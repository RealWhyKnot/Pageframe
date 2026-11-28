#[cfg(windows)]
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::Waker;
use std::time::Instant;

use crate::http::TrackedSocket;

pub(crate) const MEDIA_ERR_ABORTED: u16 = 1;
#[cfg(windows)]
pub(crate) const MEDIA_ERR_NETWORK: u16 = 2;
#[cfg(windows)]
pub(crate) const MEDIA_ERR_DECODE: u16 = 3;
pub(crate) const MEDIA_ERR_SRC_NOT_SUPPORTED: u16 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Metadata {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub has_audio: bool,
    pub has_video: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
    pub media_time: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Failure {
    pub code: u16,
    pub message: String,
}

impl Failure {
    pub(crate) fn new(code: u16, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Default)]
pub(crate) struct Inbox {
    pub metadata: Option<Metadata>,
    pub frame: Option<Frame>,
    pub presented: u64,
    pub ready: bool,
    pub seeked: Option<u64>,
    pub looped: bool,
    pub ended: bool,
    pub error: Option<Failure>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Timeline {
    pub position: f64,
    pub anchor: Option<Instant>,
    pub rate: f64,
    pub playing: bool,
    pub ready: bool,
    pub duration: f64,
    pub seek_serial: Option<u64>,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            position: 0.0,
            anchor: None,
            rate: 1.0,
            playing: false,
            ready: false,
            duration: f64::INFINITY,
            seek_serial: None,
        }
    }
}

impl Timeline {
    pub(crate) fn advancing(&self) -> bool {
        self.playing && self.ready
    }

    pub(crate) fn now(&self) -> f64 {
        let position = match self.anchor {
            Some(anchor) if self.advancing() => {
                self.position + anchor.elapsed().as_secs_f64() * self.rate
            }
            _ => self.position,
        };
        if self.duration.is_finite() {
            position.clamp(0.0, self.duration)
        } else {
            position.max(0.0)
        }
    }

    fn rebase(&mut self) {
        self.position = self.now();
        self.anchor = self.advancing().then(Instant::now);
    }

    pub(crate) fn play(&mut self) {
        self.rebase();
        self.playing = true;
        self.rebase();
    }

    pub(crate) fn pause(&mut self) {
        self.rebase();
        self.playing = false;
        self.anchor = None;
    }

    pub(crate) fn seek(&mut self, position: f64, serial: Option<u64>) {
        self.position = position;
        self.seek_serial = serial;
        self.ready = false;
        self.anchor = None;
    }

    pub(crate) fn set_rate(&mut self, rate: f64) {
        self.rebase();
        self.rate = rate;
        self.rebase();
    }

    #[cfg(windows)]
    pub(crate) fn resume(&mut self) {
        self.ready = true;
        self.anchor = self.advancing().then(Instant::now);
    }

    pub(crate) fn finish(&mut self, position: f64) {
        self.position = position;
        self.playing = false;
        self.anchor = None;
    }
}

pub(crate) enum Command {
    Play,
    Pause,
    #[cfg(windows)]
    Seek(f64, u64),
    Rate,
    Gain,
    #[cfg(windows)]
    Loop(bool),
}

pub(crate) struct Shared {
    timeline: Mutex<Timeline>,
    inbox: Mutex<Inbox>,
    gain: Mutex<f32>,
    stopped: AtomicBool,
    sockets: Mutex<Vec<Arc<TrackedSocket>>>,
    #[cfg(windows)]
    network: Mutex<Option<Failure>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    pub(crate) fn new(waker: Arc<Mutex<Option<Waker>>>) -> Self {
        Self {
            timeline: Mutex::new(Timeline::default()),
            inbox: Mutex::new(Inbox::default()),
            gain: Mutex::new(1.0),
            stopped: AtomicBool::new(false),
            sockets: Mutex::new(Vec::new()),
            #[cfg(windows)]
            network: Mutex::new(None),
            waker,
        }
    }

    #[cfg(windows)]
    pub(crate) fn record_network(&self, failure: Failure) {
        if !self.is_stopped() {
            lock(&self.network).get_or_insert(failure);
        }
    }

    #[cfg(windows)]
    pub(crate) fn take_network(&self) -> Option<Failure> {
        lock(&self.network).take()
    }

    pub(crate) fn timeline(&self) -> MutexGuard<'_, Timeline> {
        lock(&self.timeline)
    }

    pub(crate) fn inbox(&self) -> MutexGuard<'_, Inbox> {
        lock(&self.inbox)
    }

    #[cfg(windows)]
    pub(crate) fn gain(&self) -> f32 {
        *lock(&self.gain)
    }

    pub(crate) fn set_gain(&self, gain: f32) {
        *lock(&self.gain) = gain;
    }

    pub(crate) fn update(&self, change: impl FnOnce(&mut Inbox)) {
        change(&mut self.inbox());
        self.notify();
    }

    pub(crate) fn fail(&self, failure: Failure) {
        self.publish_failure(&mut self.timeline(), failure);
        self.notify();
    }

    #[cfg(windows)]
    pub(crate) fn finish_read(
        &self,
        serial: Option<u64>,
        result: Result<(), Failure>,
    ) -> ControlFlow<()> {
        let network = self.take_network();
        let Err(failure) = result else {
            return ControlFlow::Continue(());
        };
        let mut timeline = self.timeline();
        if self.is_stopped() || timeline.seek_serial != serial {
            return ControlFlow::Continue(());
        }
        self.publish_failure(&mut timeline, network.unwrap_or(failure));
        drop(timeline);
        self.notify();
        ControlFlow::Break(())
    }

    fn publish_failure(&self, timeline: &mut Timeline, failure: Failure) {
        timeline.finish(timeline.now());
        tracing::warn!(code = failure.code, reason = %failure.message, "Media playback failed");
        self.inbox().error.get_or_insert(failure);
    }

    pub(crate) fn notify(&self) {
        let waker = lock(&self.waker).clone();
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    #[cfg(windows)]
    pub(crate) fn socket(&self) -> Arc<TrackedSocket> {
        let socket = Arc::new(TrackedSocket::default());
        let mut sockets = lock(&self.sockets);
        if self.is_stopped() {
            socket.close();
        } else {
            sockets.push(Arc::clone(&socket));
        }
        socket
    }

    #[cfg(windows)]
    pub(crate) fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        let sockets = std::mem::take(&mut *lock(&self.sockets));
        for socket in sockets {
            socket.close();
        }
    }
}
