use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use num_traits::ToPrimitive;
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, IAudioClient, IAudioRenderClient, IMMDeviceEnumerator,
    MMDeviceEnumerator, WAVEFORMATEX, eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};

use crate::media::shared::Shared;

pub(super) const MIX_RATE: u32 = 48_000;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const BUFFER_HNS: i64 = 1_000_000;
const MIX_INTERVAL: Duration = Duration::from_millis(10);
const LEAD: f64 = 0.1;
const MAX_QUEUED_FRAMES: u32 = 3_600;

pub(super) struct AudioQueue {
    frames: VecDeque<[f32; 2]>,
    rate: f64,
    start: f64,
    cursor: f64,
}

impl AudioQueue {
    fn new() -> Self {
        Self {
            frames: VecDeque::new(),
            rate: f64::from(MIX_RATE),
            start: 0.0,
            cursor: 0.0,
        }
    }

    fn frame_count(&self) -> f64 {
        self.frames.len().to_f64().unwrap_or(0.0)
    }

    pub(super) fn end_time(&self) -> f64 {
        self.start + self.frame_count() / self.rate
    }

    pub(super) fn clear(&mut self, at: f64) {
        self.frames.clear();
        self.start = at;
        self.cursor = 0.0;
    }

    pub(super) fn push(&mut self, time: f64, rate: f64, samples: &[[f32; 2]]) {
        if self.frames.is_empty() || (self.rate - rate).abs() > f64::EPSILON {
            self.frames.clear();
            self.rate = rate;
            self.start = time;
            self.cursor = 0.0;
        }
        self.frames.extend(samples.iter().copied());
    }

    pub(super) fn trim_before(&mut self, time: f64) {
        let stale = ((time - self.start) * self.rate).floor();
        let Some(stale) = stale.to_usize() else {
            return;
        };
        let stale = stale.min(self.frames.len());
        if stale == 0 {
            return;
        }
        self.frames.drain(..stale);
        self.start += stale.to_f64().unwrap_or(0.0) / self.rate;
        self.cursor = 0.0;
    }

    fn head_time(&self) -> f64 {
        self.start + self.cursor / self.rate
    }

    fn mix_into(&mut self, out: &mut [f32], gain: f32, speed: f64) -> usize {
        let step = self.rate / f64::from(MIX_RATE) * speed;
        let mut mixed = 0;
        for frame in out.as_chunks_mut::<2>().0 {
            let (Some(a), Some(b)) = (self.frames.front(), self.frames.get(1)) else {
                break;
            };
            let t = self.cursor.to_f32().unwrap_or(0.0);
            frame[0] += (a[0] + (b[0] - a[0]) * t) * gain;
            frame[1] += (a[1] + (b[1] - a[1]) * t) * gain;
            mixed += 1;
            self.cursor += step;
            while self.cursor >= 1.0 && !self.frames.is_empty() {
                self.frames.pop_front();
                self.start += 1.0 / self.rate;
                self.cursor -= 1.0;
            }
        }
        mixed
    }
}

pub(super) struct Track {
    shared: Arc<Shared>,
    queue: Mutex<AudioQueue>,
}

impl Track {
    pub(super) fn new(shared: Arc<Shared>) -> Arc<Self> {
        Arc::new(Self {
            shared,
            queue: Mutex::new(AudioQueue::new()),
        })
    }

    pub(super) fn queue(&self) -> MutexGuard<'_, AudioQueue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn mix(&self, out: &mut [f32]) -> usize {
        let timeline = self.shared.timeline();
        if self.shared.is_stopped() || !timeline.advancing() {
            return 0;
        }
        let gain = self.shared.gain();
        let mut queue = self.queue();
        if queue.head_time() > timeline.now() + LEAD {
            return 0;
        }
        queue.mix_into(out, gain, timeline.rate)
    }
}

#[derive(Default)]
struct Mixer {
    tracks: Vec<Weak<Track>>,
    running: bool,
}

static MIXER: Mutex<Mixer> = Mutex::new(Mixer {
    tracks: Vec::new(),
    running: false,
});

fn mixer() -> MutexGuard<'static, Mixer> {
    MIXER.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn attach(track: &Arc<Track>) {
    let mut mixer = mixer();
    if mixer
        .tracks
        .iter()
        .any(|known| std::ptr::eq(known.as_ptr(), Arc::as_ptr(track)))
    {
        return;
    }
    mixer.tracks.push(Arc::downgrade(track));
    if mixer.running {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("pageframe-audio".to_string())
        .spawn(run);
    match spawned {
        Ok(_) => mixer.running = true,
        Err(error) => {
            mixer.tracks.pop();
            tracing::warn!(%error, "Could not start media audio mixer");
        }
    }
}

pub(super) fn detach(track: &Arc<Track>) {
    mixer()
        .tracks
        .retain(|known| !std::ptr::eq(known.as_ptr(), Arc::as_ptr(track)));
}

fn live_tracks() -> Option<Vec<Arc<Track>>> {
    let mut mixer = mixer();
    mixer.tracks.retain(|track| track.strong_count() > 0);
    if mixer.tracks.is_empty() {
        mixer.running = false;
        return None;
    }
    Some(mixer.tracks.iter().filter_map(Weak::upgrade).collect())
}

struct Output {
    client: IAudioClient,
    render: IAudioRenderClient,
    size: u32,
}

impl Output {
    fn open() -> windows_core::Result<Self> {
        let format = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_IEEE_FLOAT,
            nChannels: 2,
            nSamplesPerSec: MIX_RATE,
            nAvgBytesPerSec: MIX_RATE * 8,
            nBlockAlign: 8,
            wBitsPerSample: 32,
            cbSize: 0,
        };
        // SAFETY: COM is initialised on this thread and `format` outlives the Initialize call that reads it.
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                BUFFER_HNS,
                0,
                &raw const format,
                None,
            )?;
            let render: IAudioRenderClient = client.GetService()?;
            let size = client.GetBufferSize()?;
            client.Start()?;
            Ok(Self {
                client,
                render,
                size,
            })
        }
    }

    fn render(&self, tracks: &[Arc<Track>], scratch: &mut Vec<f32>) -> windows_core::Result<()> {
        // SAFETY: the client was initialised and started by `open` and is only used from this thread.
        let padding = unsafe { self.client.GetCurrentPadding()? };
        let free = self.size.min(MAX_QUEUED_FRAMES).saturating_sub(padding);
        if free == 0 {
            return Ok(());
        }
        let samples = usize::try_from(free).unwrap_or(0) * 2;
        scratch.clear();
        scratch.resize(samples, 0.0);
        let mixed = tracks.iter().map(|track| track.mix(scratch)).max();
        let Some(frames) = mixed.filter(|frames| *frames > 0) else {
            return Ok(());
        };
        let count = frames.to_u32().unwrap_or(free).min(free);
        let written = usize::try_from(count).unwrap_or(0) * 2;
        // SAFETY: GetBuffer returns space for `count` stereo f32 frames, which is filled before ReleaseBuffer.
        unsafe {
            let data = self.render.GetBuffer(count)?;
            let out = std::slice::from_raw_parts_mut(data.cast::<f32>(), written);
            for (target, sample) in out.iter_mut().zip(scratch.iter()) {
                *target = sample.clamp(-1.0, 1.0);
            }
            self.render.ReleaseBuffer(count, 0)
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: the client is a live IAudioClient that was started by `open`.
        let _ = unsafe { self.client.Stop() };
    }
}

fn run() {
    // SAFETY: this thread initialises COM once and balances it with CoUninitialize before exiting.
    if let Err(error) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        tracing::warn!(%error, "Could not initialize media audio COM");
        mixer().running = false;
        return;
    }
    let mut scratch = Vec::new();
    let mut output = None;
    let mut retry_at = std::time::Instant::now();
    while let Some(tracks) = live_tracks() {
        if output.is_none() && std::time::Instant::now() >= retry_at {
            match Output::open() {
                Ok(device) => output = Some(device),
                Err(error) => {
                    tracing::warn!(%error, "Could not open media audio output");
                    retry_at = std::time::Instant::now() + Duration::from_secs(5);
                }
            }
        }
        if let Some(device) = &output
            && let Err(error) = device.render(&tracks, &mut scratch)
        {
            tracing::warn!(%error, "Media audio output failed");
            output = None;
            retry_at = std::time::Instant::now() + Duration::from_secs(5);
        }
        drop(tracks);
        std::thread::sleep(MIX_INTERVAL);
    }
    drop(output);
    // SAFETY: balances the successful CoInitializeEx above on the same thread.
    unsafe { CoUninitialize() };
}

#[cfg(test)]
mod tests {
    use super::{AudioQueue, MIX_RATE};

    #[test]
    fn mixing_reports_only_the_frames_the_queue_could_supply() {
        let mut queue = AudioQueue::new();
        queue.push(0.0, f64::from(MIX_RATE), &[[0.5, -0.5]; 11]);
        let mut out = vec![0.0_f32; 40];
        let mixed = queue.mix_into(&mut out, 2.0, 1.0);
        assert_eq!(mixed, 10);
        assert!(
            out[..20]
                .iter()
                .step_by(2)
                .all(|left| (*left - 1.0).abs() < 1e-6)
        );
        assert!(out[20..].iter().all(|sample| *sample == 0.0));
        assert_eq!(queue.mix_into(&mut out, 1.0, 1.0), 0);
    }
}
