use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use num_traits::ToPrimitive;
use url::Url;
use windows::Win32::Media::MediaFoundation::{
    IMFMediaType, IMFSinkWriter, MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE,
    MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
    MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
    MF_SINK_WRITER_DISABLE_THROTTLING, MF_VERSION, MFAudioFormat_AAC, MFAudioFormat_PCM,
    MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
    MFCreateSinkWriterFromURL, MFMediaType_Audio, MFMediaType_Video, MFSTARTUP_FULL, MFShutdown,
    MFStartup, MFVideoFormat_H264, MFVideoFormat_RGB32, MFVideoInterlace_Progressive,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::core::HSTRING;

const CLIP_WIDTH: u32 = 160;
const CLIP_HEIGHT: u32 = 120;
const FPS: u32 = 10;
const CLIP_FRAMES: u32 = 20;
const SAMPLE_RATE: u32 = 48_000;
const TICKS_PER_SECOND: i64 = 10_000_000;

struct Fixtures {
    clip: Vec<u8>,
    tone: Vec<u8>,
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("media_playback")
}

pub(crate) fn fixture_dir() -> PathBuf {
    fixtures();
    fixture_root()
}

fn fixtures() -> &'static Fixtures {
    static FIXTURES: OnceLock<Fixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let dir = fixture_root();
        fs::create_dir_all(&dir).expect("create the fixture directory");
        let clip_path = dir.join("clip.mp4");
        write_clip(&clip_path);
        let clip = fs::read(&clip_path).expect("read the generated clip");
        Fixtures {
            clip,
            tone: wav_tone(),
        }
    })
}

fn sine(frame: u32) -> i16 {
    let phase = f64::from(frame) * 440.0 * std::f64::consts::TAU / f64::from(SAMPLE_RATE);
    (phase.sin() * 300.0)
        .round()
        .to_i16()
        .expect("the tone fits in 16 bits")
}

fn wav_tone() -> Vec<u8> {
    let frames = SAMPLE_RATE;
    let data_len = frames * 4;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 4).to_le_bytes());
    wav.extend_from_slice(&4_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for frame in 0..frames {
        let value = sine(frame).to_le_bytes();
        wav.extend_from_slice(&value);
        wav.extend_from_slice(&value);
    }
    wav
}

fn pack(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

fn media_type(major: &windows::core::GUID, subtype: &windows::core::GUID) -> IMFMediaType {
    // SAFETY: Media Foundation is started on this thread and the GUID references outlive the calls.
    unsafe {
        let media_type = MFCreateMediaType().expect("create a media type");
        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, major)
            .expect("set the major type");
        media_type
            .SetGUID(&MF_MT_SUBTYPE, subtype)
            .expect("set the subtype");
        media_type
    }
}

fn video_type(subtype: &windows::core::GUID) -> IMFMediaType {
    let media_type = media_type(&MFMediaType_Video, subtype);
    // SAFETY: the media type is a live COM object created on this thread.
    unsafe {
        media_type
            .SetUINT32(
                &MF_MT_INTERLACE_MODE,
                MFVideoInterlace_Progressive.0.cast_unsigned(),
            )
            .expect("set progressive video");
        media_type
            .SetUINT64(&MF_MT_FRAME_SIZE, pack(CLIP_WIDTH, CLIP_HEIGHT))
            .expect("set the frame size");
        media_type
            .SetUINT64(&MF_MT_FRAME_RATE, pack(FPS, 1))
            .expect("set the frame rate");
        media_type
            .SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))
            .expect("set square pixels");
    }
    media_type
}

fn audio_type(subtype: &windows::core::GUID, bytes_per_second: u32) -> IMFMediaType {
    let media_type = media_type(&MFMediaType_Audio, subtype);
    // SAFETY: the media type is a live COM object created on this thread.
    unsafe {
        media_type
            .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)
            .expect("set the sample size");
        media_type
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, SAMPLE_RATE)
            .expect("set the sample rate");
        media_type
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)
            .expect("set the channel count");
        media_type
            .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, bytes_per_second)
            .expect("set the byte rate");
    }
    media_type
}

fn write_sample(writer: &IMFSinkWriter, stream: u32, bytes: &[u8], time: i64, duration: i64) {
    let len = u32::try_from(bytes.len()).expect("the sample fits in a buffer");
    // SAFETY: the buffer is locked for exactly `len` bytes while they are copied in, then unlocked.
    unsafe {
        let buffer = MFCreateMemoryBuffer(len).expect("create a sample buffer");
        let mut data = std::ptr::null_mut();
        buffer
            .Lock(&raw mut data, None, None)
            .expect("lock the sample buffer");
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        buffer.Unlock().expect("unlock the sample buffer");
        buffer.SetCurrentLength(len).expect("set the sample length");
        let sample = MFCreateSample().expect("create a sample");
        sample.AddBuffer(&buffer).expect("attach the buffer");
        sample.SetSampleTime(time).expect("set the sample time");
        sample
            .SetSampleDuration(duration)
            .expect("set the sample duration");
        writer
            .WriteSample(stream, &sample)
            .expect("write the sample");
    }
}

fn frame_pixels(red: bool) -> Vec<u8> {
    let pixel: [u8; 4] = if red {
        [0, 0, 255, 255]
    } else {
        [255, 0, 0, 255]
    };
    let count = usize::try_from(CLIP_WIDTH * CLIP_HEIGHT).expect("the frame size fits");
    pixel.repeat(count)
}

fn audio_chunk(start: u32, frames: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    for frame in start..start + frames {
        let value = sine(frame).to_le_bytes();
        bytes.extend_from_slice(&value);
        bytes.extend_from_slice(&value);
    }
    bytes
}

fn write_clip(path: &Path) {
    let _ = fs::remove_file(path);
    // SAFETY: COM and Media Foundation are initialised and shut down on this thread around the writer's lifetime.
    unsafe {
        let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        MFStartup(MF_VERSION, MFSTARTUP_FULL).expect("start Media Foundation");
        {
            let mut attributes = None;
            MFCreateAttributes(&raw mut attributes, 1).expect("create writer attributes");
            let attributes = attributes.expect("writer attributes exist");
            attributes
                .SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)
                .expect("disable throttling");
            let writer =
                MFCreateSinkWriterFromURL(&HSTRING::from(path.as_os_str()), None, &attributes)
                    .expect("create the sink writer");
            let video_out = video_type(&MFVideoFormat_H264);
            video_out
                .SetUINT32(&MF_MT_AVG_BITRATE, 400_000)
                .expect("set the bitrate");
            let video = writer.AddStream(&video_out).expect("add the video stream");
            let video_in = video_type(&MFVideoFormat_RGB32);
            video_in
                .SetUINT32(&MF_MT_DEFAULT_STRIDE, CLIP_WIDTH * 4)
                .expect("set a top-down stride");
            writer
                .SetInputMediaType(video, &video_in, None)
                .expect("accept RGB32 input");
            let audio_out = audio_type(&MFAudioFormat_AAC, 16_000);
            let audio = writer.AddStream(&audio_out).expect("add the audio stream");
            let audio_in = audio_type(&MFAudioFormat_PCM, SAMPLE_RATE * 4);
            audio_in
                .SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)
                .expect("set the block alignment");
            writer
                .SetInputMediaType(audio, &audio_in, None)
                .expect("accept PCM input");
            writer.BeginWriting().expect("begin writing");
            let frame_ticks = TICKS_PER_SECOND / i64::from(FPS);
            let chunk_frames = SAMPLE_RATE / FPS;
            for index in 0..CLIP_FRAMES {
                let time = i64::from(index) * frame_ticks;
                write_sample(
                    &writer,
                    video,
                    &frame_pixels(index < CLIP_FRAMES / 2),
                    time,
                    frame_ticks,
                );
                write_sample(
                    &writer,
                    audio,
                    &audio_chunk(index * chunk_frames, chunk_frames),
                    time,
                    frame_ticks,
                );
            }
            writer.Finalize().expect("finalize the clip");
        }
        MFShutdown().expect("shut down Media Foundation");
        if com {
            CoUninitialize();
        }
    }
}

#[derive(Default)]
struct ServerLog {
    requests: Vec<(String, Option<u64>)>,
}

pub(crate) struct Server {
    pub(crate) base: Url,
    log: Arc<Mutex<ServerLog>>,
}

impl Server {
    pub(crate) fn requests(&self) -> Vec<(String, Option<u64>)> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .requests
            .clone()
    }
}

fn range_start(header: &str) -> Option<u64> {
    let (name, value) = header.split_once(':')?;
    if !name.trim().eq_ignore_ascii_case("range") {
        return None;
    }
    let spec = value.trim().strip_prefix("bytes=")?;
    spec.split('-').next()?.trim().parse().ok()
}

fn respond(mut stream: TcpStream, log: &Mutex<ServerLog>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone the stream"));
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let mut start = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim_end().is_empty() {
            break;
        }
        start = start.or_else(|| range_start(&header));
    }
    log.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .requests
        .push((path.clone(), start));
    let fixtures = fixtures();
    let body: Option<(&[u8], &str)> = match path.as_str() {
        "/clip.mp4" => Some((&fixtures.clip, "video/mp4")),
        "/tone.wav" => Some((&fixtures.tone, "audio/wav")),
        _ => None,
    };
    let Some((body, kind)) = body else {
        let _ = stream.write_all(
            b"HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: 7\r\nConnection: close\r\n\r\nmissing",
        );
        return;
    };
    let total = body.len();
    let from = start
        .and_then(|start| usize::try_from(start).ok())
        .unwrap_or(0)
        .min(total);
    let head = match start {
        Some(_) => format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Type: {kind}\r\nAccept-Ranges: bytes\r\nContent-Range: bytes {from}-{}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            total.saturating_sub(1),
            total - from
        ),
        None => format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nAccept-Ranges: bytes\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n"
        ),
    };
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    for chunk in body[from..].chunks(16 * 1024) {
        if stream.write_all(chunk).is_err() {
            return;
        }
    }
}

pub(crate) fn serve() -> Server {
    fixtures();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let port = listener.local_addr().expect("read the local port").port();
    let log = Arc::new(Mutex::new(ServerLog::default()));
    let shared = Arc::clone(&log);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let log = Arc::clone(&shared);
            std::thread::spawn(move || respond(stream, &log));
        }
    });
    Server {
        base: Url::parse(&format!("http://127.0.0.1:{port}/")).expect("a valid local URL"),
        log,
    }
}
