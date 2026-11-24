use std::sync::Arc;
use std::task::{Context as TaskContext, Waker};
use std::time::Instant;

use pageframe_dom::Document;
use pageframe_script::{IpcMessage, ScriptDocument};
use pageframe_traits::events::{MouseEventButton, UiEvent};
use pageframe_traits::net::NetProvider;
use pageframe_traits::shell::{ColorScheme, Viewport};
use serde_json::Value;
use url::Url;

use super::handle::{HostHandle, Shared, Signal, waker};
use super::input::{HostInputEvent, PointerState};
use super::load::{Loaded, Loader, URL_CHANNEL};
use super::{HostConfig, HostEvent, HostSource, IpcReply, IpcRequest, same_origin};
use crate::error::Error;

type IpcHandler = Box<dyn FnMut(&IpcRequest) -> IpcReply>;
type PostHandler = Box<dyn FnMut(&str)>;
type NavigationPolicy = Box<dyn FnMut(&Url) -> bool>;

const MAX_IPC_ROUNDS: usize = 64;
const MAX_EVENTS: usize = 256;

pub struct HostSession {
    doc: ScriptDocument,
    url: Option<Url>,
    loader: Loader,
    shared: Arc<Shared>,
    waker: Waker,
    width: u32,
    height: u32,
    scale: f64,
    color_scheme: ColorScheme,
    pointer: PointerState,
    started: Instant,
    ipc: Option<IpcHandler>,
    post: Option<PostHandler>,
    policy: Option<NavigationPolicy>,
    events: Vec<HostEvent>,
    dirty: bool,
    fresh: bool,
    hidden: bool,
}

const MAX_SURFACE: u32 = 16_384;

fn physical(logical: f64, scale: f64) -> u32 {
    let size = (logical * scale).round().clamp(0.0, f64::from(MAX_SURFACE));
    num_traits::cast(size).unwrap_or(0)
}

fn json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| String::from("null"))
}

impl HostSession {
    pub fn new(config: HostConfig) -> Result<Self, Error> {
        let HostConfig {
            source,
            width,
            height,
            scale_factor,
            color_scheme,
            http,
            net_provider,
            wake,
        } = config;
        let timer_thread = wake.is_some();
        let shared = Shared::new(wake);
        let http = http.unwrap_or_default();
        let net = net_provider.unwrap_or_else(|| {
            let provider: Arc<dyn NetProvider> = Arc::new(http.clone());
            provider
        });
        let loader = Loader {
            http,
            net,
            shared: Arc::clone(&shared),
            timer_thread,
        };
        let scale = if valid_scale(scale_factor) {
            scale_factor
        } else {
            1.0
        };
        let (width, height) = (physical(width, scale), physical(height, scale));
        let viewport = Viewport::new(width, height, narrow(scale), color_scheme);
        let loaded = loader.load(&source, viewport, false)?;
        Ok(Self {
            doc: loaded.doc,
            url: loaded.url,
            loader,
            waker: waker(&shared),
            shared,
            width,
            height,
            scale,
            color_scheme,
            pointer: PointerState::default(),
            started: Instant::now(),
            ipc: None,
            post: None,
            policy: None,
            events: Vec::new(),
            dirty: true,
            fresh: true,
            hidden: false,
        })
    }

    pub fn handle(&self) -> HostHandle {
        HostHandle::new(&self.shared)
    }

    pub fn document(&self) -> &ScriptDocument {
        &self.doc
    }

    pub fn document_mut(&mut self) -> &mut ScriptDocument {
        &mut self.doc
    }

    pub fn url(&self) -> Option<&Url> {
        self.url.as_ref()
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale
    }

    pub fn on_ipc(&mut self, handler: impl FnMut(&IpcRequest) -> IpcReply + 'static) {
        self.ipc = Some(Box::new(handler));
    }

    pub fn on_post_message(&mut self, handler: impl FnMut(&str) + 'static) {
        self.post = Some(Box::new(handler));
    }

    pub fn on_navigation(&mut self, policy: impl FnMut(&Url) -> bool + 'static) {
        self.policy = Some(Box::new(policy));
    }

    pub fn emit(&mut self, channel: &str, payload: &Value) {
        self.doc.emit_ipc(channel, &json(payload));
        self.drain_ipc();
    }

    pub fn reply(&mut self, id: u64, result: Result<Value, String>) {
        match result {
            Ok(value) => self.doc.settle_ipc(id, Ok(&json(&value))),
            Err(message) => self.doc.settle_ipc(id, Err(&message)),
        }
        self.drain_ipc();
    }

    pub fn eval(&mut self, code: &str) {
        self.doc.eval(code);
        self.drain_ipc();
    }

    pub fn set_hidden(&mut self, hidden: bool) -> bool {
        self.hidden = hidden;
        let changed = self.doc.set_hidden(hidden);
        if changed {
            self.drain_ipc();
        }
        changed
    }

    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    pub fn take_events(&mut self) -> Vec<HostEvent> {
        self.process_signals();
        std::mem::take(&mut self.events)
    }

    pub fn navigate(&mut self, url: Url) -> Result<(), Error> {
        let loaded =
            self.loader
                .load(&HostSource::Url(url.clone()), self.viewport(), self.hidden)?;
        self.install(loaded, url);
        Ok(())
    }

    fn install(&mut self, loaded: Loaded, requested: Url) {
        self.doc = loaded.doc;
        self.push_event(HostEvent::Navigated(
            loaded.url.clone().unwrap_or(requested),
        ));
        self.url = loaded.url;
        self.dirty = true;
        self.drain_ipc();
    }

    fn push_event(&mut self, event: HostEvent) {
        if matches!(
            event,
            HostEvent::TitleChanged(_)
                | HostEvent::CursorChanged(_)
                | HostEvent::ImeEnabled(_)
                | HostEvent::ImeCursorArea { .. }
        ) {
            let kind = std::mem::discriminant(&event);
            self.events
                .retain(|queued| std::mem::discriminant(queued) != kind);
        }
        if self.events.len() >= MAX_EVENTS {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    pub fn handle_input(&mut self, event: HostInputEvent) -> bool {
        match event {
            HostInputEvent::Resized { width, height } => {
                self.width = width.min(MAX_SURFACE);
                self.height = height.min(MAX_SURFACE);
                self.apply_viewport();
            }
            HostInputEvent::ScaleFactorChanged(scale) => {
                if valid_scale(scale) {
                    self.scale = scale;
                }
                self.apply_viewport();
            }
            HostInputEvent::PointerMoved { x, y } => {
                self.pointer.moved(x, y);
                self.pointer_event(UiEvent::PointerMove, MouseEventButton::Main);
            }
            HostInputEvent::PointerDown { x, y, button } => {
                if self.pointer.moved(x, y) {
                    self.pointer_event(UiEvent::PointerMove, MouseEventButton::Main);
                }
                self.pointer.press(button);
                self.pointer_event(UiEvent::PointerDown, button);
            }
            HostInputEvent::PointerUp { x, y, button } => {
                if self.pointer.moved(x, y) {
                    self.pointer_event(UiEvent::PointerMove, MouseEventButton::Main);
                }
                self.pointer.release(button);
                self.pointer_event(UiEvent::PointerUp, button);
            }
            HostInputEvent::PointerLeft => {
                self.pointer_event(UiEvent::PointerLeave, MouseEventButton::Main);
            }
            HostInputEvent::Wheel(delta) => {
                let event = self.pointer.wheel(delta, self.scale, self.scroll());
                self.ui_event(event);
            }
            HostInputEvent::KeyDown(key) => self.ui_event(UiEvent::KeyDown(key)),
            HostInputEvent::KeyUp(key) => self.ui_event(UiEvent::KeyUp(key)),
            HostInputEvent::Ime(ime) => self.ui_event(UiEvent::Ime(ime)),
            HostInputEvent::Paste(text) => self.ui_event(UiEvent::Paste(text)),
            HostInputEvent::ModifiersChanged(mods) => {
                self.pointer.set_modifiers(mods);
                return false;
            }
            HostInputEvent::Focused(focused) => {
                self.doc
                    .dispatch_window_event(if focused { "focus" } else { "blur" });
                self.drain_ipc();
            }
        }
        self.take_redraw()
    }

    pub fn poll(&mut self) -> bool {
        self.fresh = false;
        self.process_signals();
        let waker = self.waker.clone();
        let ran = self.doc.poll(Some(TaskContext::from_waker(&waker)));
        self.drain_ipc();
        let redraw = self.take_redraw();
        ran || redraw
    }

    pub fn next_wake(&self) -> Option<Instant> {
        if self.fresh || self.shared.has_pending() {
            return Some(Instant::now());
        }
        self.doc.next_timer_deadline()
    }

    pub fn take_redraw(&mut self) -> bool {
        let requested = self.shared.take_redraw();
        std::mem::take(&mut self.dirty) || requested
    }

    pub fn resolve(&mut self) {
        let time = self.animation_time();
        self.doc.inner_mut().resolve(time);
    }

    pub(super) fn animation_time(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    pub fn request_redraw(&self) {
        self.shared.request_redraw();
    }

    fn viewport(&self) -> Viewport {
        Viewport::new(
            self.width,
            self.height,
            narrow(self.scale),
            self.color_scheme,
        )
    }

    fn apply_viewport(&mut self) {
        let viewport = self.viewport();
        self.doc.inner_mut().set_viewport(viewport);
        self.dirty = true;
    }

    fn scroll(&self) -> (f64, f64) {
        let scroll = self.doc.inner().viewport_scroll();
        (scroll.x, scroll.y)
    }

    fn ui_event(&mut self, event: UiEvent) {
        self.resolve();
        self.doc.handle_ui_event(event);
        self.drain_ipc();
    }

    fn pointer_event(
        &mut self,
        wrap: fn(pageframe_traits::events::PageframePointerEvent) -> UiEvent,
        button: MouseEventButton,
    ) {
        self.resolve();
        let event = self.pointer.pointer(button, self.scale, self.scroll());
        self.ui_event(wrap(event));
    }

    fn process_signals(&mut self) {
        for signal in self.shared.take() {
            match signal {
                Signal::Emit { channel, payload } => self.emit(&channel, &payload),
                Signal::Reply { id, result } => self.reply(id, result),
                Signal::Navigate(url) => self.request_navigation(url),
                Signal::Event(event) => self.push_event(event),
            }
        }
    }

    fn request_navigation(&mut self, url: Url) {
        if self.is_fragment_change(&url) {
            self.eval(&format!(
                "location.assign({})",
                json(&Value::String(url.into()))
            ));
            self.dirty = true;
            return;
        }
        if !self.allows(&url) {
            self.push_event(HostEvent::NavigationBlocked(url));
            return;
        }
        let (html, landed) = match self.loader.read(&url) {
            Ok(fetched) => fetched,
            Err(error) => {
                self.push_event(HostEvent::LoadFailed {
                    url,
                    reason: error.to_string(),
                });
                return;
            }
        };
        if landed != url && !self.allows(&landed) {
            self.push_event(HostEvent::NavigationBlocked(landed));
            return;
        }
        let loaded = self
            .loader
            .build(&html, Some(landed), self.viewport(), self.hidden);
        self.install(loaded, url);
    }

    fn allows(&mut self, url: &Url) -> bool {
        match (&mut self.policy, &self.url) {
            (Some(policy), _) => policy(url),
            (None, Some(base)) => same_origin(base, url),
            (None, None) => false,
        }
    }

    fn is_fragment_change(&self, target: &Url) -> bool {
        let Some(current) = &self.url else {
            return false;
        };
        let mut left = current.clone();
        let mut right = target.clone();
        left.set_fragment(None);
        right.set_fragment(None);
        left == right && target.fragment().is_some()
    }

    fn url_changed(&mut self, next: &Url) {
        let Some(current) = &self.url else {
            return;
        };
        if current == next || !same_authority(current, next) {
            return;
        }
        self.url = Some(next.clone());
        self.push_event(HostEvent::UrlChanged(next.clone()));
    }

    fn drain_ipc(&mut self) {
        for _ in 0..MAX_IPC_ROUNDS {
            for url in self.doc.take_url_changes() {
                self.url_changed(&url);
            }
            let messages = self.doc.take_ipc();
            if messages.is_empty() {
                return;
            }
            for message in messages {
                self.dispatch_ipc(message);
            }
        }
        self.fresh = true;
        self.shared.wake();
    }

    fn dispatch_ipc(&mut self, message: IpcMessage) {
        match message {
            IpcMessage::Invoke { id, channel, .. } if channel == URL_CHANNEL => {
                self.doc.settle_ipc(id, Err("reserved IPC channel"));
            }
            IpcMessage::Post(text) => {
                if let Some(post) = &mut self.post {
                    post(&text);
                }
            }
            IpcMessage::Invoke {
                id,
                channel,
                payload,
            } => {
                let request = IpcRequest {
                    id,
                    channel,
                    payload: serde_json::from_str(&payload).unwrap_or(Value::Null),
                };
                let reply = match &mut self.ipc {
                    Some(handler) => handler(&request),
                    None => IpcReply::Err(format!("no IPC handler for `{}`", request.channel)),
                };
                match reply {
                    IpcReply::Ok(value) => self.doc.settle_ipc(id, Ok(&json(&value))),
                    IpcReply::Err(message) => self.doc.settle_ipc(id, Err(&message)),
                    IpcReply::Later => {}
                }
            }
        }
    }
}

fn same_authority(current: &Url, next: &Url) -> bool {
    current.scheme() == next.scheme()
        && current.username() == next.username()
        && current.password() == next.password()
        && current.host() == next.host()
        && current.port_or_known_default() == next.port_or_known_default()
}

fn narrow(value: f64) -> f32 {
    num_traits::cast(value).unwrap_or(1.0)
}

pub(super) fn valid_scale(scale: f64) -> bool {
    scale.is_finite() && scale > 0.0 && scale <= 64.0
}
