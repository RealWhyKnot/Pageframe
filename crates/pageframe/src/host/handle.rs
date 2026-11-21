use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Wake, Waker};

use cursor_icon::CursorIcon;
use pageframe_traits::navigation::{NavigationOptions, NavigationProvider};
use pageframe_traits::shell::{ClipboardError, ShellProvider};
use serde_json::Value;
use url::Url;

use super::{HostEvent, WakeCallback};

pub(super) enum Signal {
    Emit {
        channel: String,
        payload: Value,
    },
    Reply {
        id: u64,
        result: Result<Value, String>,
    },
    Navigate(Url),
    Event(HostEvent),
}

pub(super) struct Shared {
    signals: Mutex<Vec<Signal>>,
    redraw: AtomicBool,
    wake: Option<WakeCallback>,
}

impl Shared {
    pub(super) fn new(wake: Option<WakeCallback>) -> Arc<Self> {
        Arc::new(Self {
            signals: Mutex::new(Vec::new()),
            redraw: AtomicBool::new(true),
            wake,
        })
    }

    pub(super) fn push(&self, signal: Signal) {
        self.signals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(signal);
        self.wake();
    }

    pub(super) fn take(&self) -> Vec<Signal> {
        std::mem::take(&mut *self.signals.lock().unwrap_or_else(PoisonError::into_inner))
    }

    pub(super) fn has_pending(&self) -> bool {
        !self
            .signals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    }

    pub(super) fn request_redraw(&self) {
        if !self.redraw.swap(true, Ordering::AcqRel) {
            self.wake();
        }
    }

    pub(super) fn take_redraw(&self) -> bool {
        self.redraw.swap(false, Ordering::AcqRel)
    }

    pub(super) fn wake(&self) {
        if let Some(wake) = &self.wake {
            wake();
        }
    }
}

#[derive(Clone)]
pub struct HostHandle {
    shared: Arc<Shared>,
}

impl HostHandle {
    pub(super) fn new(shared: &Arc<Shared>) -> Self {
        Self {
            shared: Arc::clone(shared),
        }
    }

    pub fn emit(&self, channel: &str, payload: Value) {
        self.shared.push(Signal::Emit {
            channel: channel.to_owned(),
            payload,
        });
    }

    pub fn reply(&self, id: u64, result: Result<Value, String>) {
        self.shared.push(Signal::Reply { id, result });
    }

    pub fn navigate(&self, url: Url) {
        self.shared.push(Signal::Navigate(url));
    }

    pub fn request_redraw(&self) {
        self.shared.request_redraw();
    }

    pub fn wake(&self) {
        self.shared.wake();
    }
}

pub(super) struct ShellBridge(pub(super) Arc<Shared>);

impl ShellBridge {
    fn event(&self, event: HostEvent) {
        self.0.push(Signal::Event(event));
    }
}

impl ShellProvider for ShellBridge {
    fn request_redraw(&self) {
        self.0.request_redraw();
    }

    fn set_cursor(&self, icon: Option<CursorIcon>) {
        self.event(HostEvent::CursorChanged(icon));
    }

    fn set_window_title(&self, title: String) {
        self.event(HostEvent::TitleChanged(title));
    }

    fn set_ime_enabled(&self, is_enabled: bool) {
        self.event(HostEvent::ImeEnabled(is_enabled));
    }

    fn set_ime_cursor_area(&self, x: f32, y: f32, width: f32, height: f32) {
        self.event(HostEvent::ImeCursorArea {
            x,
            y,
            width,
            height,
        });
    }

    fn request_clipboard_text(&self) -> Result<(), ClipboardError> {
        self.event(HostEvent::ClipboardRequested);
        Ok(())
    }

    fn set_clipboard_text(&self, text: String) -> Result<(), ClipboardError> {
        self.event(HostEvent::ClipboardText(text));
        Ok(())
    }

    fn request_window_close(&self) {
        self.event(HostEvent::CloseRequested);
    }
}

pub(super) struct NavigationBridge(pub(super) Arc<Shared>);

impl NavigationProvider for NavigationBridge {
    fn navigate_to(&self, options: NavigationOptions) {
        self.0.push(Signal::Navigate(options.url));
    }
}

struct WakeBridge(Arc<Shared>);

impl Wake for WakeBridge {
    fn wake(self: Arc<Self>) {
        self.0.wake();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.wake();
    }
}

pub(super) fn waker(shared: &Arc<Shared>) -> Waker {
    Waker::from(Arc::new(WakeBridge(Arc::clone(shared))))
}
