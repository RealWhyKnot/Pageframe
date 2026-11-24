mod mixer;
mod stream;

use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use num_traits::ToPrimitive;
use url::Url;
use windows::Win32::Media::MediaFoundation::{
    IMF2DBuffer, IMFMediaSource, IMFSample, IMFSourceReader, MF_MT_AUDIO_NUM_CHANNELS,
    MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE,
    MF_MT_MINIMUM_DISPLAY_APERTURE, MF_MT_SUBTYPE, MF_OBJECT_INVALID, MF_PD_DURATION,
    MF_RESOLUTION_CONTENT_DOES_NOT_HAVE_TO_MATCH_EXTENSION_OR_MIME_TYPE, MF_RESOLUTION_MEDIASOURCE,
    MF_RESOLUTION_READ, MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING,
    MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    MF_SOURCE_READER_MEDIASOURCE, MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR, MF_VERSION, MFAudioFormat_Float,
    MFCreateAttributes, MFCreateMFByteStreamOnStream, MFCreateMediaType,
    MFCreateSourceReaderFromMediaSource, MFCreateSourceResolver, MFMediaType_Audio,
    MFMediaType_Video, MFSTARTUP_LITE, MFShutdown, MFStartup, MFVideoFormat_RGB32,
};
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0, PropVariantClear,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::Variant::{VT_I8, VT_UI8};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows_core::{GUID, HSTRING, Interface};

use self::mixer::{MIX_RATE, Track};
use super::shared::{
    Command, Failure, Frame, MEDIA_ERR_ABORTED, MEDIA_ERR_DECODE, MEDIA_ERR_SRC_NOT_SUPPORTED,
    Metadata, Shared,
};
use super::source;

pub(crate) const SUPPORTED: bool = true;
const TICKS: f64 = 10_000_000.0;
const AUDIO_AHEAD: f64 = 0.5;
const AUDIO_BEHIND: f64 = 0.2;
const IDLE_STEP: Duration = Duration::from_millis(20);
const MAX_STEP: Duration = Duration::from_millis(50);
const MIN_STEP: Duration = Duration::from_millis(1);

pub(crate) fn spawn(
    url: Url,
    commands: Receiver<Command>,
    shared: Arc<Shared>,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("pageframe-media".to_string())
        .spawn(move || run(&url, &commands, &shared))
}

fn run(url: &Url, commands: &Receiver<Command>, shared: &Arc<Shared>) {
    // SAFETY: this worker thread initialises COM once and balances it with CoUninitialize before exiting.
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    // SAFETY: MFStartup is balanced by MFShutdown below on the same thread after every MF object is released.
    let started = unsafe { MFStartup(MF_VERSION, MFSTARTUP_LITE) }.is_ok();
    if started {
        let outcome = Session::open(url, shared).and_then(|mut session| session.run(commands));
        if let Err(failure) = outcome {
            if !shared.is_stopped() {
                shared.fail(shared.take_network().unwrap_or(failure));
            }
        }
        // SAFETY: balances the successful MFStartup above; the session and its readers were dropped already.
        let _ = unsafe { MFShutdown() };
    } else {
        shared.fail(Failure::new(
            MEDIA_ERR_SRC_NOT_SUPPORTED,
            "Media Foundation is not available",
        ));
    }
    if com {
        // SAFETY: balances the successful CoInitializeEx above on the same thread.
        unsafe { CoUninitialize() };
    }
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

fn stream_index(index: i32) -> u32 {
    index.cast_unsigned()
}

fn seconds(ticks: i64) -> f64 {
    ticks.to_f64().unwrap_or(0.0) / TICKS
}

fn ticks(seconds: f64) -> i64 {
    (seconds * TICKS).round().to_i64().unwrap_or(0)
}

fn open_reader(url: &Url, shared: &Arc<Shared>) -> Result<IMFSourceReader, Failure> {
    let mut source = source::open(url, shared)?;
    let hint = HSTRING::from(format!(
        "media{}",
        source::sniff_extension(source.as_mut(), url)
    ));
    proceed(shared)?;
    let stream = stream::SourceStream::wrap(source, Arc::clone(shared));
    let flags = MF_RESOLUTION_MEDIASOURCE.0
        | MF_RESOLUTION_CONTENT_DOES_NOT_HAVE_TO_MATCH_EXTENSION_OR_MIME_TYPE.0
        | MF_RESOLUTION_READ.0;
    // SAFETY: every pointer passed below is a live local or COM object owned by this thread for the call.
    unsafe {
        let bytes = MFCreateMFByteStreamOnStream(&stream).map_err(|error| unsupported(&error))?;
        let resolver = MFCreateSourceResolver().map_err(|error| unsupported(&error))?;
        let mut kind = MF_OBJECT_INVALID;
        let mut object = None;
        resolver
            .CreateObjectFromByteStream(
                &bytes,
                &hint,
                flags.cast_unsigned(),
                None::<&IPropertyStore>,
                &raw mut kind,
                &raw mut object,
            )
            .map_err(|error| unsupported(&error))?;
        let media: IMFMediaSource = object
            .ok_or_else(|| Failure::new(MEDIA_ERR_SRC_NOT_SUPPORTED, "no media source"))?
            .cast()
            .map_err(|error| unsupported(&error))?;
        let mut attributes = None;
        MFCreateAttributes(&raw mut attributes, 1).map_err(|error| unsupported(&error))?;
        let attributes = attributes
            .ok_or_else(|| Failure::new(MEDIA_ERR_SRC_NOT_SUPPORTED, "no reader attributes"))?;
        attributes
            .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
            .map_err(|error| unsupported(&error))?;
        MFCreateSourceReaderFromMediaSource(&media, &attributes)
            .map_err(|error| unsupported(&error))
    }
}

fn select(reader: &IMFSourceReader, stream: i32, major: &GUID, subtype: &GUID) -> bool {
    // SAFETY: the reader is a live IMFSourceReader and the GUID references outlive each call.
    unsafe {
        if reader
            .SetStreamSelection(stream_index(MF_SOURCE_READER_ALL_STREAMS.0), false)
            .is_err()
        {
            return false;
        }
        if reader
            .SetStreamSelection(stream_index(stream), true)
            .is_err()
        {
            return false;
        }
        let Ok(media_type) = MFCreateMediaType() else {
            return false;
        };
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, major).is_ok()
            && media_type.SetGUID(&MF_MT_SUBTYPE, subtype).is_ok()
            && reader
                .SetCurrentMediaType(stream_index(stream), None, &media_type)
                .is_ok()
    }
}

fn has_stream(reader: &IMFSourceReader, stream: i32) -> bool {
    // SAFETY: the reader is a live IMFSourceReader owned by this thread.
    unsafe { reader.GetNativeMediaType(stream_index(stream), 0).is_ok() }
}

fn duration(reader: &IMFSourceReader) -> f64 {
    // SAFETY: the reader is live; the PROPVARIANT is read by its tag and cleared after use.
    unsafe {
        let Ok(mut value) = reader.GetPresentationAttribute(
            stream_index(MF_SOURCE_READER_MEDIASOURCE.0),
            &MF_PD_DURATION,
        ) else {
            return f64::INFINITY;
        };
        let inner = &value.Anonymous.Anonymous;
        let result = if inner.vt == VT_UI8 {
            inner.Anonymous.uhVal.to_f64().unwrap_or(0.0) / TICKS
        } else {
            f64::INFINITY
        };
        let _ = PropVariantClear(&raw mut value);
        if result > 0.0 { result } else { f64::INFINITY }
    }
}

fn set_position(reader: &IMFSourceReader, seconds: f64) -> windows_core::Result<()> {
    let value = PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_I8,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: PROPVARIANT_0_0_0 {
                    hVal: ticks(seconds),
                },
            }),
        },
    };
    // SAFETY: `value` is a VT_I8 PROPVARIANT that lives across the call and owns no resources.
    unsafe { reader.SetCurrentPosition(&GUID::zeroed(), &raw const value) }
}

enum Sample {
    Data(IMFSample, f64),
    Changed,
    End,
}

fn read_sample(reader: &IMFSourceReader, stream: i32) -> windows_core::Result<Sample> {
    loop {
        let mut flags = 0_u32;
        let mut time = 0_i64;
        let mut sample = None;
        // SAFETY: the out pointers reference locals that live across the synchronous ReadSample call.
        unsafe {
            reader.ReadSample(
                stream_index(stream),
                0,
                None,
                Some(&raw mut flags),
                Some(&raw mut time),
                Some(&raw mut sample),
            )?;
        }
        let flags = flags.cast_signed();
        if flags & MF_SOURCE_READERF_ERROR.0 != 0 {
            return Err(windows::Win32::Foundation::E_FAIL.into());
        }
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 != 0 {
            return Ok(Sample::End);
        }
        let changed = flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 != 0;
        match sample {
            Some(sample) => {
                if changed {
                    drop(sample);
                    return Ok(Sample::Changed);
                }
                return Ok(Sample::Data(sample, seconds(time)));
            }
            None if changed => return Ok(Sample::Changed),
            None => {}
        }
    }
}

#[derive(Clone, Copy)]
struct VideoFormat {
    width: u32,
    height: u32,
    stride: i32,
    crop: (u32, u32, u32, u32),
}

impl VideoFormat {
    fn read(reader: &IMFSourceReader) -> windows_core::Result<Self> {
        let stream = stream_index(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0);
        // SAFETY: the reader and the media type it returns are live COM objects; the blob buffer is local.
        unsafe {
            let media_type = reader.GetCurrentMediaType(stream)?;
            let size = media_type.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let width = u32::try_from(size >> 32).unwrap_or(0);
            let height = u32::try_from(size & 0xffff_ffff).unwrap_or(0);
            let packed = i32::try_from(width.saturating_mul(4)).unwrap_or(i32::MAX);
            let stride = media_type
                .GetUINT32(&MF_MT_DEFAULT_STRIDE)
                .map(u32::cast_signed)
                .unwrap_or(packed);
            let mut area = [0_u8; 16];
            let crop = match media_type.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut area, None) {
                Ok(()) => aperture(&area, width, height),
                Err(_) => (0, 0, width, height),
            };
            Ok(Self {
                width,
                height,
                stride,
                crop,
            })
        }
    }
}

fn aperture(area: &[u8; 16], width: u32, height: u32) -> (u32, u32, u32, u32) {
    let x = i16::from_le_bytes([area[2], area[3]]);
    let y = i16::from_le_bytes([area[6], area[7]]);
    let w = i32::from_le_bytes([area[8], area[9], area[10], area[11]]);
    let h = i32::from_le_bytes([area[12], area[13], area[14], area[15]]);
    let x = u32::try_from(x).unwrap_or(0).min(width);
    let y = u32::try_from(y).unwrap_or(0).min(height);
    let w = u32::try_from(w).unwrap_or(0).min(width - x);
    let h = u32::try_from(h).unwrap_or(0).min(height - y);
    if w == 0 || h == 0 {
        (0, 0, width, height)
    } else {
        (x, y, w, h)
    }
}

fn convert_rows(first_row: *const u8, pitch: isize, format: &VideoFormat) -> Option<Vec<u8>> {
    let (cx, cy, cw, ch) = format.crop;
    let row_bytes = usize::try_from(cw).ok()? * 4;
    let mut rgba = Vec::with_capacity(row_bytes * usize::try_from(ch).ok()?);
    let left = isize::try_from(cx).ok()? * 4;
    for y in 0..ch {
        let offset = isize::try_from(cy + y)
            .ok()?
            .checked_mul(pitch)?
            .checked_add(left)?;
        // SAFETY: the caller guarantees `first_row` addresses `height` rows of `pitch` bytes covering the crop.
        let row = unsafe { std::slice::from_raw_parts(first_row.offset(offset), row_bytes) };
        for pixel in row.as_chunks::<4>().0 {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
        }
    }
    Some(rgba)
}

fn frame_pixels(sample: &IMFSample, format: &VideoFormat) -> windows_core::Result<Vec<u8>> {
    let invalid = || windows_core::Error::from(windows::Win32::Foundation::E_UNEXPECTED);
    // SAFETY: buffers stay locked while read and bounds are checked against the locked length.
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer()?;
        if let Ok(planar) = buffer.cast::<IMF2DBuffer>() {
            let mut scanline = std::ptr::null_mut();
            let mut pitch = 0_i32;
            planar.Lock2D(&raw mut scanline, &raw mut pitch)?;
            let pixels = isize::try_from(pitch)
                .ok()
                .filter(|pitch| {
                    pitch.unsigned_abs() >= usize::try_from(format.width).unwrap_or(0) * 4
                })
                .and_then(|pitch| convert_rows(scanline, pitch, format));
            planar.Unlock2D()?;
            return pixels.ok_or_else(invalid);
        }
        let mut data = std::ptr::null_mut();
        let mut length = 0_u32;
        buffer.Lock(&raw mut data, None, Some(&raw mut length))?;
        let stride = isize::try_from(format.stride).unwrap_or(0);
        let span = stride.unsigned_abs();
        let rows = usize::try_from(format.height).unwrap_or(0);
        let needed = span * rows.saturating_sub(1) + usize::try_from(format.width).unwrap_or(0) * 4;
        let pixels = if usize::try_from(length).unwrap_or(0) < needed || rows == 0 {
            None
        } else if stride < 0 {
            let last = isize::try_from(span * (rows - 1)).unwrap_or(0);
            convert_rows(data.offset(last), stride, format)
        } else {
            convert_rows(data, stride, format)
        };
        buffer.Unlock()?;
        pixels.ok_or_else(invalid)
    }
}

fn audio_frames(sample: &IMFSample, channels: usize) -> windows_core::Result<Vec<[f32; 2]>> {
    // SAFETY: the buffer is locked while its `length` bytes are copied and unlocked afterwards.
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer()?;
        let mut data = std::ptr::null_mut();
        let mut length = 0_u32;
        buffer.Lock(&raw mut data, None, Some(&raw mut length))?;
        let bytes =
            std::slice::from_raw_parts(data.cast_const(), usize::try_from(length).unwrap_or(0));
        let frame_bytes = channels.max(1) * 4;
        let frames = bytes
            .chunks_exact(frame_bytes)
            .map(|frame| {
                let left = f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
                let right = if channels >= 2 {
                    f32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]])
                } else {
                    left
                };
                [left, right]
            })
            .collect();
        buffer.Unlock()?;
        Ok(frames)
    }
}

struct Video {
    reader: IMFSourceReader,
    format: VideoFormat,
    pending: Option<(IMFSample, f64)>,
    last: f64,
    eos: bool,
}

impl Video {
    fn next(&mut self) -> windows_core::Result<()> {
        self.pending = None;
        while !self.eos {
            match read_sample(&self.reader, MF_SOURCE_READER_FIRST_VIDEO_STREAM.0)? {
                Sample::Data(sample, time) => {
                    self.last = self.last.max(time);
                    self.pending = Some((sample, time));
                    return Ok(());
                }
                Sample::Changed => self.format = VideoFormat::read(&self.reader)?,
                Sample::End => self.eos = true,
            }
        }
        Ok(())
    }
}

struct Audio {
    reader: IMFSourceReader,
    channels: usize,
    rate: f64,
    track: Arc<Track>,
    eos: bool,
    attached: bool,
}

impl Audio {
    fn open(reader: IMFSourceReader, shared: &Arc<Shared>) -> windows_core::Result<Self> {
        let stream = stream_index(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0);
        // SAFETY: the reader and the media type it returns are live COM objects owned by this thread.
        let (channels, rate) = unsafe {
            let media_type = reader.GetCurrentMediaType(stream)?;
            (
                media_type.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)?,
                media_type.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)?,
            )
        };
        Ok(Self {
            reader,
            channels: usize::try_from(channels).unwrap_or(2).max(1),
            rate: f64::from(if rate == 0 { MIX_RATE } else { rate }),
            track: Track::new(Arc::clone(shared)),
            eos: false,
            attached: false,
        })
    }

    fn fill(&mut self, now: f64) -> windows_core::Result<()> {
        self.track.queue().trim_before(now - AUDIO_BEHIND);
        while !self.eos && self.track.queue().end_time() < now + AUDIO_AHEAD {
            match read_sample(&self.reader, MF_SOURCE_READER_FIRST_AUDIO_STREAM.0)? {
                Sample::Data(sample, time) => {
                    let frames = audio_frames(&sample, self.channels)?;
                    let mut queue = self.track.queue();
                    queue.push(time, self.rate, &frames);
                    queue.trim_before(now - AUDIO_BEHIND);
                }
                Sample::Changed => {
                    let stream = stream_index(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0);
                    // SAFETY: the reader and the media type it returns are live COM objects.
                    unsafe {
                        let media_type = self.reader.GetCurrentMediaType(stream)?;
                        let channels = media_type.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)?;
                        let rate = media_type.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)?;
                        self.channels = usize::try_from(channels).unwrap_or(2).max(1);
                        self.rate = f64::from(if rate == 0 { MIX_RATE } else { rate });
                    }
                }
                Sample::End => self.eos = true,
            }
        }
        Ok(())
    }

    fn seek(&mut self, to: f64) -> windows_core::Result<()> {
        set_position(&self.reader, to)?;
        self.eos = false;
        self.track.queue().clear(to);
        self.fill(to)?;
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

struct Session<'a> {
    shared: &'a Arc<Shared>,
    video: Option<Video>,
    audio: Option<Audio>,
    duration: f64,
    looping: bool,
}

impl<'a> Session<'a> {
    fn open(url: &Url, shared: &'a Arc<Shared>) -> Result<Self, Failure> {
        let first = open_reader(url, shared)?;
        proceed(shared)?;
        let duration = duration(&first);
        let has_audio = has_stream(&first, MF_SOURCE_READER_FIRST_AUDIO_STREAM.0);
        let has_video = has_stream(&first, MF_SOURCE_READER_FIRST_VIDEO_STREAM.0)
            && select(
                &first,
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0,
                &MFMediaType_Video,
                &MFVideoFormat_RGB32,
            );
        let (video, audio_reader) = if has_video {
            let format = VideoFormat::read(&first).map_err(|error| unsupported(&error))?;
            let video = Video {
                reader: first,
                format,
                pending: None,
                last: 0.0,
                eos: false,
            };
            let audio_reader = if has_audio && !shared.is_stopped() {
                open_reader(url, shared).ok()
            } else {
                None
            };
            (Some(video), audio_reader)
        } else {
            (None, has_audio.then_some(first))
        };
        let audio = audio_reader
            .filter(|reader| {
                select(
                    reader,
                    MF_SOURCE_READER_FIRST_AUDIO_STREAM.0,
                    &MFMediaType_Audio,
                    &MFAudioFormat_Float,
                )
            })
            .and_then(|reader| Audio::open(reader, shared).ok());
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
        let mut session = Self {
            shared,
            video,
            audio,
            duration,
            looping: false,
        };
        proceed(shared)?;
        session.prime(0.0).map_err(|error| unsupported(&error))?;
        shared.update(|inbox| inbox.ready = true);
        Ok(session)
    }

    fn present(
        &self,
        sample: &IMFSample,
        time: f64,
        format: &VideoFormat,
    ) -> windows_core::Result<()> {
        let rgba = frame_pixels(sample, format)?;
        let frame = Frame {
            width: format.crop.2,
            height: format.crop.3,
            rgba: Arc::new(rgba),
            media_time: time,
        };
        self.shared.update(|inbox| {
            inbox.frame = Some(frame);
            inbox.presented += 1;
        });
        Ok(())
    }

    fn prime(&mut self, to: f64) -> windows_core::Result<()> {
        if let Some(video) = self.video.as_mut() {
            if to > 0.0 {
                set_position(&video.reader, to)?;
            }
            video.eos = false;
            video.next()?;
            let mut shown = video.pending.take();
            video.next()?;
            while let Some((_, time)) = &video.pending {
                if *time > to + 0.001 {
                    break;
                }
                shown = video.pending.take();
                video.next()?;
            }
            let format = video.format;
            if let Some((sample, time)) = shown {
                self.present(&sample, time, &format)?;
            }
        }
        if let Some(audio) = self.audio.as_mut() {
            audio.seek(to)?;
        }
        self.shared.timeline().resume();
        Ok(())
    }

    fn run(&mut self, commands: &Receiver<Command>) -> Result<(), Failure> {
        self.sync_audio();
        loop {
            if self.shared.is_stopped() {
                return Ok(());
            }
            let received = match self.wait() {
                Some(wait) => commands.recv_timeout(wait),
                None => commands.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            let mut pending = Vec::new();
            match received {
                Ok(command) => pending.push(command),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
            pending.extend(commands.try_iter());
            let mut seek = None;
            for command in pending {
                match command {
                    Command::Seek(to, serial) => seek = Some((to, serial)),
                    Command::Loop(looping) => self.looping = looping,
                    Command::Play | Command::Pause | Command::Rate | Command::Gain => {}
                }
            }
            if self.shared.is_stopped() {
                return Ok(());
            }
            if let Some((to, serial)) = seek {
                self.prime(to).map_err(|error| decode_error(&error))?;
                self.shared.update(|inbox| inbox.seeked = Some(serial));
            }
            self.sync_audio();
            self.step().map_err(|error| decode_error(&error))?;
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
            .map(|(_, time)| Duration::from_secs_f64(((time - now) / rate.max(0.01)).max(0.0)));
        let wait = match (video_due, &self.audio) {
            (Some(due), Some(_)) => due.min(IDLE_STEP),
            (Some(due), None) => due,
            (None, _) => IDLE_STEP,
        };
        Some(wait.clamp(MIN_STEP, MAX_STEP))
    }

    fn step(&mut self) -> windows_core::Result<()> {
        let (now, advancing) = {
            let timeline = self.shared.timeline();
            (timeline.now(), timeline.advancing())
        };
        if !advancing {
            return Ok(());
        }
        let mut due = None;
        let mut format = None;
        if let Some(video) = self.video.as_mut() {
            while video
                .pending
                .as_ref()
                .is_some_and(|(_, time)| *time <= now + 0.002)
            {
                due = video.pending.take();
                video.next()?;
            }
            format = Some(video.format);
        }
        if let (Some((sample, time)), Some(format)) = (due, format) {
            self.present(&sample, time, &format)?;
        }
        if let Some(audio) = self.audio.as_mut() {
            audio.fill(now)?;
        }
        if self.finished(now) {
            if self.looping {
                self.shared.timeline().seek(0.0);
                self.prime(0.0)?;
                self.shared.update(|inbox| inbox.looped = true);
            } else {
                let end = self.end_time();
                self.shared.timeline().finish(end);
                self.sync_audio();
                self.shared.update(|inbox| inbox.ended = true);
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
