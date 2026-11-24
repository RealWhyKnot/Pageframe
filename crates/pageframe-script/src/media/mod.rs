mod shared;
#[cfg(windows)]
mod source;
#[cfg(not(windows))]
mod stub;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
use self::stub as backend;
#[cfg(windows)]
use self::windows as backend;

use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::task::Waker;
use std::thread::JoinHandle;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsResult, JsString, JsValue, NativeFunction};
use pageframe_dom::{BaseDocument, NodeId, local_name};
use url::Url;
use web_time::{Duration, Instant};

use self::shared::{
    Command, Failure, Frame, MEDIA_ERR_ABORTED, MEDIA_ERR_SRC_NOT_SUPPORTED, Metadata, Shared,
};
use crate::dom::{dom_ctx, js_str, node_id_of_value, node_wrapper, to_rust_string};
use crate::fetch::document_may_load;
use crate::http::HttpClient;
use crate::state::DomCtx;

const PLAYING_POLL: Duration = Duration::from_millis(50);
const LOADING_POLL: Duration = Duration::from_millis(100);

struct Player {
    shared: Arc<Shared>,
    commands: Option<Sender<Command>>,
    worker: Option<JoinHandle<()>>,
}

impl Player {
    fn open(url: Url, http: HttpClient, waker: Arc<Mutex<Option<Waker>>>) -> Self {
        let shared = Arc::new(Shared::new(waker));
        let (commands, receiver) = channel();
        let worker = match backend::spawn(url, http, receiver, Arc::clone(&shared)) {
            Ok(worker) => Some(worker),
            Err(error) => {
                shared.fail(Failure::new(
                    MEDIA_ERR_ABORTED,
                    format!("could not start the media worker: {error}"),
                ));
                None
            }
        };
        Self {
            shared,
            commands: Some(commands),
            worker,
        }
    }

    fn refuse(url: &Url, waker: Arc<Mutex<Option<Waker>>>) -> Self {
        let shared = Arc::new(Shared::new(waker));
        shared.fail(Failure::new(
            MEDIA_ERR_SRC_NOT_SUPPORTED,
            format!("this page cannot load {url}"),
        ));
        Self {
            shared,
            commands: None,
            worker: None,
        }
    }

    fn send(&self, command: Command) {
        if let Some(commands) = &self.commands {
            let _ = commands.send(command);
        }
    }

    fn halt(&mut self) {
        self.shared.stop();
        self.commands = None;
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.halt();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Entry {
    player: Player,
    connected: bool,
    ready: bool,
    failed: bool,
    shown: bool,
}

#[derive(Default)]
pub(crate) struct MediaRegistry {
    entries: HashMap<NodeId, Entry>,
    pending: Vec<NodeId>,
    listener: Option<JsObject>,
    serial: u64,
}

impl MediaRegistry {
    pub(crate) fn mark(&mut self, node_id: NodeId) {
        if !self.pending.contains(&node_id) {
            self.pending.push(node_id);
        }
    }

    pub(crate) fn take_all(&mut self) -> Vec<NodeId> {
        self.pending.clear();
        self.listener = None;
        let ids: Vec<NodeId> = self.entries.keys().copied().collect();
        for id in &ids {
            if let Some(entry) = self.entries.get(id) {
                entry.player.shared.stop();
            }
        }
        ids
    }

    pub(crate) fn deadline(&self, now: Instant) -> Option<Instant> {
        let mut deadline: Option<Instant> = None;
        for entry in self.entries.values() {
            if entry.failed {
                continue;
            }
            let wait = if !entry.ready {
                LOADING_POLL
            } else if entry.player.shared.timeline().playing {
                PLAYING_POLL
            } else {
                continue;
            };
            let at = now + wait;
            deadline = Some(deadline.map_or(at, |current| current.min(at)));
        }
        deadline
    }
}

pub(crate) fn is_media(doc: &BaseDocument, node_id: NodeId) -> bool {
    doc.get_node(node_id)
        .and_then(|node| node.element_data())
        .is_some_and(|element| {
            element.name.local == local_name!("video") || element.name.local == local_name!("audio")
        })
}

pub(crate) fn note_attribute(ctx: &DomCtx, node_id: NodeId, name: &str) {
    if name != "src" {
        return;
    }
    if is_media(&ctx.doc.borrow(), node_id) {
        ctx.state.borrow_mut().media.mark(node_id);
    }
}

pub(crate) fn scan(ctx: &DomCtx) {
    let found: Vec<NodeId> = {
        let doc = ctx.doc.borrow();
        let mut found = Vec::new();
        let mut stack = vec![doc.root_node().id];
        while let Some(id) = stack.pop() {
            let Some(node) = doc.get_node(id) else {
                continue;
            };
            if is_media(&doc, id) {
                found.push(id);
            }
            stack.extend(node.children.iter().rev().copied());
        }
        found
    };
    let mut state = ctx.state.borrow_mut();
    for id in found {
        state.media.mark(id);
    }
}

pub(crate) fn close_all(ctx: &DomCtx) {
    let ids = ctx.state.borrow_mut().media.take_all();
    let mut entries: Vec<Entry> = {
        let mut state = ctx.state.borrow_mut();
        ids.iter()
            .filter_map(|id| state.media.entries.remove(id))
            .collect()
    };
    for entry in &mut entries {
        entry.player.halt();
    }
    drop(entries);
}

pub(crate) fn init(context: &mut Context) {
    register(context, "__pageframe_media_open", 2, open);
    register(context, "__pageframe_media_close", 1, close);
    register(context, "__pageframe_media_control", 3, control);
    register(context, "__pageframe_media_time", 1, time);
    register(context, "__pageframe_media_can_play", 1, can_play);
    register(context, "__pageframe_media_listen", 1, listen);
}

fn register(
    context: &mut Context,
    name: &str,
    length: usize,
    body: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
) {
    let _ = context.register_global_callable(
        JsString::from(name),
        length,
        NativeFunction::from_fn_ptr(body),
    );
}

fn node_arg(args: &[JsValue]) -> Option<NodeId> {
    args.first().and_then(node_id_of_value)
}

fn remove_entry(ctx: &DomCtx, node_id: NodeId) {
    let entry = ctx.state.borrow_mut().media.entries.remove(&node_id);
    if let Some(entry) = entry {
        if entry.shown {
            pageframe_dom::media::clear_frame(&mut ctx.doc.borrow_mut(), node_id);
        }
        drop(entry);
    }
}

fn open(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let Some(node_id) = node_arg(args) else {
        return Ok(JsValue::from(false));
    };
    let raw = match args.get(1) {
        Some(value) => to_rust_string(value, context)?,
        None => String::new(),
    };
    remove_entry(&ctx, node_id);
    let resolved = {
        let state = ctx.state.borrow();
        let base = state.base_url.as_ref();
        let url = match base {
            Some(base) => base.join(&raw).ok(),
            None => Url::parse(&raw).ok(),
        };
        url.map(|url| {
            let allowed = document_may_load(base, &url);
            (url, allowed)
        })
    };
    let Some((url, allowed)) = resolved else {
        return Ok(JsValue::from(false));
    };
    let connected = ctx
        .doc
        .borrow()
        .get_node(node_id)
        .is_some_and(|node| node.flags.is_in_document());
    let waker = Arc::clone(&ctx.state.borrow().net.waker);
    let player = if allowed {
        let http = ctx
            .state
            .borrow()
            .fetcher
            .as_ref()
            .and_then(|fetcher| fetcher.borrow().http_client())
            .unwrap_or_default();
        Player::open(url, http, waker)
    } else {
        Player::refuse(&url, waker)
    };
    ctx.state.borrow_mut().media.entries.insert(
        node_id,
        Entry {
            player,
            connected,
            ready: false,
            failed: false,
            shown: false,
        },
    );
    Ok(JsValue::from(true))
}

fn close(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    if let Some(node_id) = node_arg(args) {
        remove_entry(&ctx, node_id);
    }
    Ok(JsValue::undefined())
}

fn control(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let Some(node_id) = node_arg(args) else {
        return Ok(JsValue::undefined());
    };
    let op = match args.get(1) {
        Some(value) => to_rust_string(value, context)?,
        None => String::new(),
    };
    let value = match args.get(2) {
        Some(value) => value.to_number(context)?,
        None => 0.0,
    };
    let mut state = ctx.state.borrow_mut();
    let registry = &mut state.media;
    let Some(entry) = registry.entries.get(&node_id) else {
        return Ok(JsValue::undefined());
    };
    let shared = &entry.player.shared;
    match op.as_str() {
        "play" => {
            shared.timeline().play();
            entry.player.send(Command::Play);
        }
        "pause" => {
            shared.timeline().pause();
            entry.player.send(Command::Pause);
        }
        "seek" => {
            registry.serial += 1;
            let serial = registry.serial;
            let target = if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            };
            shared.timeline().seek(target);
            #[cfg(windows)]
            entry.player.send(Command::Seek(target, serial));
            return Ok(count(serial));
        }
        "rate" => {
            if value.is_finite() && value > 0.0 {
                shared.timeline().set_rate(value);
                entry.player.send(Command::Rate);
            }
        }
        "gain" => {
            let gain = if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            };
            shared.set_gain(narrow(gain));
            entry.player.send(Command::Gain);
        }
        #[cfg(windows)]
        "loop" => entry.player.send(Command::Loop(value != 0.0)),
        _ => {}
    }
    Ok(JsValue::undefined())
}

fn narrow(value: f64) -> f32 {
    num_traits::cast::<f64, f32>(value).unwrap_or(0.0)
}

fn time(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let state = ctx.state.borrow();
    let now = node_arg(args)
        .and_then(|node_id| state.media.entries.get(&node_id))
        .map_or(f64::NAN, |entry| entry.player.shared.timeline().now());
    Ok(JsValue::from(now))
}

fn listen(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let listener = args
        .first()
        .and_then(JsValue::as_object)
        .filter(JsObject::is_callable);
    ctx.state.borrow_mut().media.listener = listener;
    Ok(JsValue::undefined())
}

fn can_play(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let mime = match args.first() {
        Some(value) => to_rust_string(value, context)?,
        None => String::new(),
    };
    Ok(js_str(if backend::SUPPORTED {
        can_play_type(&mime)
    } else {
        ""
    }))
}

pub(crate) fn can_play_type(mime: &str) -> &'static str {
    let mut parts = mime.split(';');
    let essence = parts.next().unwrap_or("").trim().to_ascii_lowercase();
    let codecs: Vec<String> = parts
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            (key.trim().eq_ignore_ascii_case("codecs"))
                .then(|| value.trim().trim_matches('"').to_ascii_lowercase())
        })
        .flat_map(|list| {
            list.split(',')
                .map(|codec| codec.trim().to_string())
                .collect::<Vec<_>>()
        })
        .filter(|codec| !codec.is_empty())
        .collect();
    let container = match essence.as_str() {
        "video/mp4" | "audio/mp4" | "audio/x-m4a" | "video/x-m4v" | "video/quicktime" => {
            Container::Mp4
        }
        "audio/mpeg" | "audio/mp3" => Container::Mp3,
        "audio/aac" | "audio/aacp" => Container::Aac,
        "audio/wav" | "audio/x-wav" | "audio/wave" | "audio/vnd.wave" => Container::Wav,
        "video/webm" | "audio/webm" => Container::Webm,
        _ => return "",
    };
    if codecs.is_empty() {
        return "maybe";
    }
    let supported = codecs.iter().all(|codec| container.supports(codec));
    if supported { "probably" } else { "" }
}

enum Container {
    Mp4,
    Mp3,
    Aac,
    Wav,
    Webm,
}

impl Container {
    fn supports(&self, codec: &str) -> bool {
        match self {
            Self::Mp4 => {
                ["avc1", "avc3", "hev1", "hvc1", "mp4a"]
                    .iter()
                    .any(|prefix| codec.starts_with(prefix))
                    || codec == "mp3"
            }
            Self::Mp3 => codec == "mp3",
            Self::Aac => codec.starts_with("mp4a") || codec == "aac",
            Self::Wav => codec == "1",
            Self::Webm => ["vp8", "vp9", "vp09", "opus", "vorbis"]
                .iter()
                .any(|prefix| codec.starts_with(prefix)),
        }
    }
}

struct Update {
    node_id: NodeId,
    metadata: Option<Metadata>,
    ready: bool,
    frame: Option<(f64, u32, u32, u64)>,
    seeked: Option<u64>,
    looped: bool,
    ended: bool,
    error: Option<Failure>,
    detached: bool,
    playing: bool,
    time: f64,
}

impl Update {
    fn is_empty(&self) -> bool {
        self.metadata.is_none()
            && !self.ready
            && self.frame.is_none()
            && self.seeked.is_none()
            && !self.looped
            && !self.ended
            && self.error.is_none()
            && !self.detached
            && !self.playing
    }
}

fn collect(ctx: &DomCtx) -> (Vec<Update>, bool) {
    let mut updates = Vec::new();
    let mut frames: Vec<(NodeId, Frame)> = Vec::new();
    let mut dropped = Vec::new();
    {
        let doc = ctx.doc.borrow();
        let mut state = ctx.state.borrow_mut();
        for (&node_id, entry) in &mut state.media.entries {
            let alive = is_media(&doc, node_id);
            let connected = alive
                && doc
                    .get_node(node_id)
                    .is_some_and(|node| node.flags.is_in_document());
            let detached = !alive || (entry.connected && !connected);
            entry.connected = connected;
            let shared = &entry.player.shared;
            let mut inbox = {
                let mut inbox = shared.inbox();
                let presented = inbox.presented;
                let taken = std::mem::take(&mut *inbox);
                inbox.presented = presented;
                taken
            };
            let (playing, time) = {
                let timeline = shared.timeline();
                (timeline.advancing(), timeline.now())
            };
            if inbox.ready {
                entry.ready = true;
            }
            if inbox.error.is_some() {
                entry.failed = true;
            }
            let presented = inbox.presented;
            let frame = inbox.frame.take().map(|frame| {
                let info = (frame.media_time, frame.width, frame.height, presented);
                frames.push((node_id, frame));
                info
            });
            if detached {
                dropped.push(node_id);
            }
            updates.push(Update {
                node_id,
                metadata: inbox.metadata,
                ready: inbox.ready,
                frame,
                seeked: inbox.seeked,
                looped: inbox.looped,
                ended: inbox.ended,
                error: inbox.error,
                detached,
                playing: playing && !detached,
                time,
            });
        }
    }
    let redraw = !frames.is_empty();
    {
        let mut doc = ctx.doc.borrow_mut();
        let mut state = ctx.state.borrow_mut();
        for (node_id, frame) in frames {
            if dropped.contains(&node_id) {
                continue;
            }
            pageframe_dom::media::present_frame(
                &mut doc,
                node_id,
                frame.width,
                frame.height,
                frame.rgba,
            );
            if let Some(entry) = state.media.entries.get_mut(&node_id) {
                entry.shown = true;
            }
        }
    }
    for node_id in dropped {
        remove_entry(ctx, node_id);
    }
    updates.retain(|update| !update.is_empty());
    (updates, redraw)
}

fn set(object: &JsObject, key: &str, value: JsValue, context: &mut Context) {
    let _ = object.create_data_property_or_throw(JsString::from(key), value, context);
}

fn count(value: u64) -> JsValue {
    JsValue::from(u32::try_from(value).map_or(f64::from(u32::MAX), f64::from))
}

fn update_object(update: Update, context: &mut Context) -> JsObject {
    let object = JsObject::with_object_proto(context.intrinsics());
    if let Some(metadata) = update.metadata {
        let value = JsObject::with_object_proto(context.intrinsics());
        set(
            &value,
            "duration",
            JsValue::from(metadata.duration),
            context,
        );
        set(&value, "width", JsValue::from(metadata.width), context);
        set(&value, "height", JsValue::from(metadata.height), context);
        set(
            &value,
            "hasAudio",
            JsValue::from(metadata.has_audio),
            context,
        );
        set(
            &value,
            "hasVideo",
            JsValue::from(metadata.has_video),
            context,
        );
        set(&object, "metadata", value.into(), context);
    }
    if update.ready {
        set(&object, "ready", JsValue::from(true), context);
    }
    if let Some((media_time, width, height, presented)) = update.frame {
        let value = JsObject::with_object_proto(context.intrinsics());
        set(&value, "mediaTime", JsValue::from(media_time), context);
        set(&value, "width", JsValue::from(width), context);
        set(&value, "height", JsValue::from(height), context);
        set(&value, "presented", count(presented), context);
        set(&object, "frame", value.into(), context);
    }
    if let Some(serial) = update.seeked {
        set(&object, "seeked", count(serial), context);
    }
    if update.looped {
        set(&object, "looped", JsValue::from(true), context);
    }
    if update.ended {
        set(&object, "ended", JsValue::from(true), context);
    }
    if let Some(error) = update.error {
        let value = JsObject::with_object_proto(context.intrinsics());
        set(&value, "code", JsValue::from(error.code), context);
        set(&value, "message", js_str(&error.message), context);
        set(&object, "error", value.into(), context);
    }
    if update.detached {
        set(&object, "detached", JsValue::from(true), context);
    }
    set(&object, "playing", JsValue::from(update.playing), context);
    set(&object, "time", JsValue::from(update.time), context);
    object
}

pub(crate) fn deliver(ctx: &DomCtx, context: &mut Context) -> (bool, bool) {
    let pending = std::mem::take(&mut ctx.state.borrow_mut().media.pending);
    let (updates, redraw) = collect(ctx);
    let listener = ctx.state.borrow().media.listener.clone();
    let Some(listener) = listener else {
        return (false, redraw);
    };
    if pending.is_empty() && updates.is_empty() {
        return (false, redraw);
    }
    for node_id in pending {
        if !is_media(&ctx.doc.borrow(), node_id) {
            continue;
        }
        let element = node_wrapper(ctx, node_id, context);
        let update = JsObject::with_object_proto(context.intrinsics());
        set(&update, "attach", JsValue::from(true), context);
        if let Err(error) = listener.call(
            &JsValue::undefined(),
            &[element.into(), update.into()],
            context,
        ) {
            crate::runtime::events::report_js_error(ctx, context, "media element", &error);
        }
    }
    for update in updates {
        let element = node_wrapper(ctx, update.node_id, context);
        let update = update_object(update, context);
        if let Err(error) = listener.call(
            &JsValue::undefined(),
            &[element.into(), update.into()],
            context,
        ) {
            crate::runtime::events::report_js_error(ctx, context, "media event", &error);
        }
    }
    (true, redraw)
}
