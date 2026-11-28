mod mixer;
mod pixels;
mod reader;
mod stream;

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use url::Url;
use windows::Win32::Media::MediaFoundation::{
    IMFSample, IMFSourceReader, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
    MF_MT_SUBTYPE, MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    MF_VERSION, MFAudioFormat_Float, MFMediaType_Audio, MFMediaType_Video, MFSTARTUP_LITE,
    MFShutdown, MFStartup, MFVideoFormat_RGB32,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

use self::mixer::Track;
use self::pixels::{VideoFormat, audio_frames, frame_pixels};
use self::reader::{Reader, duration, has_stream, open_reader, select, set_position, stream_index};
use super::shared::{
    Command, Failure, Frame, MEDIA_ERR_ABORTED, MEDIA_ERR_DECODE, MEDIA_ERR_SRC_NOT_SUPPORTED,
    Metadata, Shared,
};
use super::source;
use crate::http::HttpClient;

pub(crate) const SUPPORTED: bool = true;
const AUDIO_AHEAD: f64 = 0.5;
const AUDIO_BEHIND: f64 = 0.2;
const IDLE_STEP: Duration = Duration::from_millis(20);
const MAX_STEP: Duration = Duration::from_millis(50);
const MIN_STEP: Duration = Duration::from_millis(1);

pub(crate) fn spawn(
    url: Url,
    http: HttpClient,
    commands: Receiver<Command>,
    shared: Arc<Shared>,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("pageframe-media".to_string())
        .spawn(move || run(&url, &http, &commands, &shared))
}

fn run(url: &Url, http: &HttpClient, commands: &Receiver<Command>, shared: &Arc<Shared>) {
    // SAFETY: this worker thread initialises COM once and balances it with CoUninitialize before exiting.
    if let Err(error) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        shared.fail(Failure::new(
            MEDIA_ERR_SRC_NOT_SUPPORTED,
            format!("initialize media COM: {error}"),
        ));
        return;
    }
    // SAFETY: a successful startup is balanced by shutdown on this thread after the session is dropped.
    if let Err(error) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_LITE) } {
        shared.fail(Failure::new(
            MEDIA_ERR_SRC_NOT_SUPPORTED,
            format!("start Media Foundation: {error}"),
        ));
    } else {
        match Session::open(url, http, shared) {
            Ok(mut session) => {
                if session.prime(0.0, None, Prime::Initial).is_continue() {
                    let _ = session.run(commands);
                }
            }
            Err(failure) if !shared.is_stopped() => {
                shared.fail(shared.take_network().unwrap_or(failure));
            }
            Err(_) => {}
        }
        // SAFETY: balances the successful MFStartup above; the session and its readers were dropped already.
        if let Err(error) = unsafe { MFShutdown() } {
            tracing::warn!(%error, "Media Foundation shutdown failed");
        }
    }
    // SAFETY: balances the successful CoInitializeEx above on the same thread.
    unsafe { CoUninitialize() };
}

fn proceed(shared: &Shared) -> Result<(), Failure> {
    if shared.is_stopped() {
        Err(Failure::new(MEDIA_ERR_ABORTED, "media loading stopped"))
    } else {
        Ok(())
    }
}

fn unsupported(error: &windows_core::Error) -> Failure {
    Failure::new(
        MEDIA_ERR_SRC_NOT_SUPPORTED,
        format!("open media: {}", error.message()),
    )
}

fn decode_error(error: &windows_core::Error) -> Failure {
    Failure::new(
        MEDIA_ERR_DECODE,
        format!("decode media: {}", error.message()),
    )
}

struct VideoSample {
    sample: IMFSample,
    time: f64,
    format: VideoFormat,
}

impl VideoSample {
    fn into_frame(self) -> windows_core::Result<Frame> {
        let rgba = frame_pixels(&self.sample, &self.format)?;
        Ok(Frame {
            width: self.format.crop.2,
            height: self.format.crop.3,
            rgba: Arc::new(rgba),
            media_time: self.time,
        })
    }
}

struct Video {
    reader: Reader,
    format: VideoFormat,
    pending: Option<VideoSample>,
    last: f64,
    eos: bool,
}

impl Video {
    fn open(reader: IMFSourceReader) -> Result<Self, Failure> {
        select(
            &reader,
            MF_SOURCE_READER_FIRST_VIDEO_STREAM.0,
            &MFMediaType_Video,
            &MFVideoFormat_RGB32,
        )
        .map_err(|error| {
            Failure::new(
                MEDIA_ERR_DECODE,
                format!("configure RGB32 video output: {error}"),
            )
        })?;
        let format = VideoFormat::read(&reader).map_err(|error| decode_error(&error))?;
        Ok(Self {
            reader: Reader::new(reader),
            format,
            pending: None,
            last: 0.0,
            eos: false,
        })
    }

    fn next(&mut self, shared: &Shared, serial: Option<u64>) -> windows_core::Result<()> {
        self.pending = None;
        while !self.eos && shared.timeline().seek_serial == serial {
            let sample = self
                .reader
                .read_sample(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0, shared)?;
            self.eos = sample.end;
            if sample.changed {
                self.format = VideoFormat::read(self.reader.get()?)?;
            }
            if let Some((sample, time)) = sample.data {
                self.last = self.last.max(time);
                self.pending = Some(VideoSample {
                    sample,
                    time,
                    format: self.format,
                });
                break;
            }
        }
        Ok(())
    }
}

struct Audio {
    reader: Reader,
    channels: usize,
    rate: f64,
    track: Arc<Track>,
    eos: bool,
    attached: bool,
}

impl Audio {
    fn configure(reader: &IMFSourceReader) -> windows_core::Result<(usize, f64)> {
        select(
            reader,
            MF_SOURCE_READER_FIRST_AUDIO_STREAM.0,
            &MFMediaType_Audio,
            &MFAudioFormat_Float,
        )?;
        Self::format(reader)
    }

    fn format(reader: &IMFSourceReader) -> windows_core::Result<(usize, f64)> {
        let stream = stream_index(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0);
        // SAFETY: the reader and the media type it returns are live COM objects owned by this thread.
        unsafe {
            let media_type = reader.GetCurrentMediaType(stream)?;
            if media_type.GetGUID(&MF_MT_SUBTYPE)? != MFAudioFormat_Float {
                return Err(windows::Win32::Foundation::E_UNEXPECTED.into());
            }
            let channels = media_type.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)?;
            let rate = media_type.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)?;
            if channels == 0 || rate == 0 {
                return Err(windows::Win32::Foundation::E_UNEXPECTED.into());
            }
            Ok((usize::try_from(channels).unwrap_or(2), f64::from(rate)))
        }
    }

    fn open(reader: IMFSourceReader, shared: &Arc<Shared>) -> windows_core::Result<Self> {
        let (channels, rate) = Self::configure(&reader)?;
        Ok(Self {
            reader: Reader::new(reader),
            channels,
            rate,
            track: Track::new(Arc::clone(shared)),
            eos: false,
            attached: false,
        })
    }

    fn fill(&mut self, now: f64, shared: &Shared, serial: Option<u64>) -> windows_core::Result<()> {
        self.track.queue().trim_before(now - AUDIO_BEHIND);
        while !self.eos && shared.timeline().seek_serial == serial {
            if self.track.queue().end_time() >= now + AUDIO_AHEAD {
                break;
            }
            let sample = self
                .reader
                .read_sample(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0, shared)?;
            self.eos = sample.end;
            if sample.changed {
                (self.channels, self.rate) = Self::format(self.reader.get()?)?;
            }
            if let Some((sample, time)) = sample.data {
                let frames = audio_frames(&sample, self.channels)?;
                let mut queue = self.track.queue();
                queue.push(time, self.rate, &frames);
                queue.trim_before(now - AUDIO_BEHIND);
            }
        }
        Ok(())
    }

    fn prime(
        &mut self,
        to: f64,
        serial: Option<u64>,
        shared: &Shared,
        reposition: bool,
    ) -> windows_core::Result<()> {
        if reposition {
            set_position(self.reader.get()?, to)?;
        }
        self.eos = false;
        self.track.queue().clear(to);
        self.fill(to, shared, serial)?;
        self.track.queue().trim_before(to);
        Ok(())
    }

    fn set_attached(&mut self, attached: bool) {
        if attached == self.attached {
            return;
        }
        self.attached = attached;
        if attached {
            mixer::attach(&self.track);
        } else {
            mixer::detach(&self.track);
        }
    }

    fn drained(&self) -> bool {
        self.eos
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        mixer::detach(&self.track);
    }
}

#[derive(PartialEq, Eq)]
enum Prime {
    Initial,
    Seek,
    Loop,
}

struct Session<'a> {
    shared: &'a Arc<Shared>,
    url: &'a Url,
    http: &'a HttpClient,
    video: Option<Video>,
    audio: Option<Audio>,
    duration: f64,
    looping: bool,
    loaded: bool,
}

impl<'a> Session<'a> {
    fn open(url: &'a Url, http: &'a HttpClient, shared: &'a Arc<Shared>) -> Result<Self, Failure> {
        let first = open_reader(url, http, shared)?;
        proceed(shared)?;
        let duration = duration(&first);
        let has_audio = has_stream(&first, MF_SOURCE_READER_FIRST_AUDIO_STREAM.0);
        let has_video = has_stream(&first, MF_SOURCE_READER_FIRST_VIDEO_STREAM.0);
        let (video, audio_reader) = if has_video {
            let video = Video::open(first)?;
            let audio_reader = if has_audio {
                proceed(shared)?;
                match open_reader(url, http, shared) {
                    Ok(reader) => Some(reader),
                    Err(failure) => {
                        proceed(shared)?;
                        let failure = shared.take_network().unwrap_or(failure);
                        tracing::warn!(reason = %failure.message, "Media audio reader could not open");
                        None
                    }
                }
            } else {
                None
            };
            (Some(video), audio_reader)
        } else {
            (None, has_audio.then_some(first))
        };
        let audio = if let Some(reader) = audio_reader {
            match Audio::open(reader, shared) {
                Ok(audio) => Some(audio),
                Err(error) if has_video => {
                    tracing::warn!(%error, "Media audio format could not open");
                    None
                }
                Err(error) => return Err(unsupported(&error)),
            }
        } else {
            None
        };
        if video.is_none() && audio.is_none() {
            return Err(Failure::new(
                MEDIA_ERR_SRC_NOT_SUPPORTED,
                "the media has no playable audio or video",
            ));
        }
        let (width, height) = video
            .as_ref()
            .map_or((0, 0), |video| (video.format.crop.2, video.format.crop.3));
        let metadata = Metadata {
            duration,
            width,
            height,
            has_audio: audio.is_some(),
            has_video: video.is_some(),
        };
        shared.timeline().duration = duration;
        shared.update(|inbox| inbox.metadata = Some(metadata));
        proceed(shared)?;
        Ok(Self {
            shared,
            url,
            http,
            video,
            audio,
            duration,
            looping: false,
            loaded: false,
        })
    }

    fn restore_readers(&mut self) -> Result<(), Failure> {
        if let Some(video) = self.video.as_mut()
            && video.reader.is_retired()
        {
            *video = Video::open(open_reader(self.url, self.http, self.shared)?)?;
        }
        if let Some(audio) = self.audio.as_mut()
            && audio.reader.is_retired()
        {
            let reader = open_reader(self.url, self.http, self.shared)?;
            (audio.channels, audio.rate) =
                Audio::configure(&reader).map_err(|error| decode_error(&error))?;
            audio.reader = Reader::new(reader);
        }
        Ok(())
    }

    fn prime(&mut self, to: f64, serial: Option<u64>, kind: Prime) -> ControlFlow<()> {
        if self.shared.timeline().seek_serial != serial {
            return ControlFlow::Continue(());
        }
        let result = self.restore_readers().and_then(|()| {
            self.prime_current(to, serial, kind)
                .map_err(|error| decode_error(&error))
        });
        self.shared.finish_read(serial, result)
    }

    fn prime_current(
        &mut self,
        to: f64,
        serial: Option<u64>,
        kind: Prime,
    ) -> windows_core::Result<()> {
        if self.shared.timeline().seek_serial != serial {
            return Ok(());
        }
        let mut shown = None;
        if let Some(video) = self.video.as_mut() {
            if kind != Prime::Initial {
                video.pending = None;
                set_position(video.reader.get()?, to)?;
            }
            video.last = 0.0;
            video.eos = false;
            video.next(self.shared, serial)?;
            shown = video.pending.take();
            video.next(self.shared, serial)?;
            while video
                .pending
                .as_ref()
                .is_some_and(|sample| sample.time <= to + 0.001)
            {
                shown = video.pending.take();
                video.next(self.shared, serial)?;
            }
        }
        if self.shared.timeline().seek_serial != serial {
            return Ok(());
        }
        if let Some(audio) = self.audio.as_mut() {
            audio.prime(to, serial, self.shared, kind != Prime::Initial)?;
        }
        let frame = shown.map(VideoSample::into_frame).transpose()?;
        {
            let mut timeline = self.shared.timeline();
            if self.shared.is_stopped() || timeline.seek_serial != serial {
                return Ok(());
            }
            let mut inbox = self.shared.inbox();
            if let Some(frame) = frame {
                inbox.frame = Some(frame);
                inbox.presented += 1;
            }
            inbox.ready |= !self.loaded;
            self.loaded = true;
            match kind {
                Prime::Initial => {}
                Prime::Seek => inbox.seeked = serial,
                Prime::Loop => inbox.looped = true,
            }
            timeline.resume();
        }
        self.shared.notify();
        Ok(())
    }

    fn run(&mut self, commands: &Receiver<Command>) -> ControlFlow<()> {
        self.sync_audio();
        loop {
            if self.shared.is_stopped() {
                return ControlFlow::Continue(());
            }
            let received = match self.wait() {
                Some(wait) => commands.recv_timeout(wait),
                None => commands.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            let first = match received {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return ControlFlow::Continue(()),
            };
            let mut seek = None;
            for command in first.into_iter().chain(commands.try_iter()) {
                match command {
                    Command::Seek(to, serial) => seek = Some((to, serial)),
                    Command::Loop(looping) => self.looping = looping,
                    Command::Play | Command::Pause | Command::Rate | Command::Gain => {}
                }
            }
            if self.shared.is_stopped() {
                return ControlFlow::Continue(());
            }
            if let Some((to, serial)) = seek {
                self.prime(to.min(self.duration), Some(serial), Prime::Seek)?;
            }
            self.sync_audio();
            self.step()?;
        }
    }

    fn sync_audio(&mut self) {
        let advancing = self.shared.timeline().advancing();
        if let Some(audio) = self.audio.as_mut() {
            audio.set_attached(advancing);
        }
    }

    fn wait(&self) -> Option<Duration> {
        let (now, rate, advancing) = {
            let timeline = self.shared.timeline();
            (timeline.now(), timeline.rate, timeline.advancing())
        };
        if !advancing {
            return None;
        }
        let video_due = self
            .video
            .as_ref()
            .and_then(|video| video.pending.as_ref())
            .map(|sample| {
                Duration::from_secs_f64(
                    ((sample.time - now) / rate.max(0.01)).clamp(0.0, MAX_STEP.as_secs_f64()),
                )
            });
        let wait = match (video_due, &self.audio) {
            (Some(due), Some(_)) => due.min(IDLE_STEP),
            (Some(due), None) => due,
            (None, _) => IDLE_STEP,
        };
        Some(wait.clamp(MIN_STEP, MAX_STEP))
    }

    fn step(&mut self) -> ControlFlow<()> {
        let (now, serial) = {
            let timeline = self.shared.timeline();
            if !timeline.advancing() {
                return ControlFlow::Continue(());
            }
            (timeline.now(), timeline.seek_serial)
        };
        let result = self
            .step_current(now, serial)
            .map_err(|error| decode_error(&error));
        self.shared.finish_read(serial, result)
    }

    fn step_current(&mut self, now: f64, serial: Option<u64>) -> windows_core::Result<()> {
        let mut due = None;
        if let Some(video) = self.video.as_mut() {
            while video
                .pending
                .as_ref()
                .is_some_and(|sample| sample.time <= now + 0.002)
            {
                due = video.pending.take();
                video.next(self.shared, serial)?;
            }
        }
        if let Some(frame) = due.map(VideoSample::into_frame).transpose()? {
            let timeline = self.shared.timeline();
            if self.shared.is_stopped() || timeline.seek_serial != serial {
                return Ok(());
            }
            {
                let mut inbox = self.shared.inbox();
                inbox.frame = Some(frame);
                inbox.presented += 1;
            }
            drop(timeline);
            self.shared.notify();
        }
        if let Some(audio) = self.audio.as_mut() {
            audio.fill(now, self.shared, serial)?;
        }
        if self.finished(now) {
            let mut timeline = self.shared.timeline();
            if !timeline.advancing() || timeline.seek_serial != serial {
                return Ok(());
            }
            if self.looping {
                timeline.seek(0.0, serial);
                drop(timeline);
                self.prime_current(0.0, serial, Prime::Loop)?;
            } else {
                timeline.finish(self.end_time());
                self.shared.inbox().ended = true;
                drop(timeline);
                self.sync_audio();
                self.shared.notify();
            }
        }
        Ok(())
    }

    fn end_time(&self) -> f64 {
        if self.duration.is_finite() {
            return self.duration;
        }
        let video = self.video.as_ref().map_or(0.0, |video| video.last);
        let audio = self
            .audio
            .as_ref()
            .map_or(0.0, |audio| audio.track.queue().end_time());
        video.max(audio)
    }

    fn finished(&self, now: f64) -> bool {
        if self.duration.is_finite() && now >= self.duration {
            return true;
        }
        let video_done = self
            .video
            .as_ref()
            .is_none_or(|video| video.eos && video.pending.is_none());
        let audio_done = self.audio.as_ref().is_none_or(Audio::drained);
        video_done && audio_done && now >= self.end_time()
    }
}

#[cfg(test)]
mod tests {
    use super::{Failure, Shared};
    use crate::media::shared::{MEDIA_ERR_DECODE, MEDIA_ERR_NETWORK};
    use std::sync::{Arc, Mutex};

    #[test]
    fn failing_superseded_seek_discards_its_decode_and_network_errors() {
        let shared = Shared::new(Arc::new(Mutex::new(None)));
        shared.timeline().seek(2.0, Some(1));
        shared.record_network(Failure::new(MEDIA_ERR_NETWORK, "first seek read failed"));
        shared.timeline().seek(4.0, Some(2));

        let failure = Failure::new(MEDIA_ERR_DECODE, "first seek decode failed");
        assert!(shared.finish_read(Some(1), Err(failure)).is_continue());
        assert_eq!(shared.take_network(), None);
        assert_eq!(shared.inbox().error, None);
        assert!(shared.finish_read(Some(2), Ok(())).is_continue());

        let failure = Failure::new(MEDIA_ERR_DECODE, "second seek decode failed");
        assert!(shared.finish_read(Some(2), Err(failure.clone())).is_break());
        assert_eq!(shared.inbox().error, Some(failure));
    }
}
