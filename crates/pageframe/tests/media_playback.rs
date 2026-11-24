#![cfg(windows)]

mod media_support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use media_support::serve;
use pageframe::host::{HostConfig, HostSession};
use pageframe_dom::Document as _;
use pageframe_paint::paint_scene;
use url::Url;

const WIDTH: u32 = 200;
const HEIGHT: u32 = 260;
const TIMEOUT: Duration = Duration::from_secs(20);
const MAX_WAIT: Duration = Duration::from_millis(25);

const EVENTS: &str = "['loadstart','durationchange','loadedmetadata','loadeddata','canplay','canplaythrough','play','playing','pause','seeking','seeked','ended','volumechange','ratechange','error','resize','emptied']";

struct Page {
    session: HostSession,
    wakes: Receiver<()>,
    posts: Rc<RefCell<Vec<String>>>,
}

impl Page {
    fn open(body: &str, base: &Url) -> Self {
        let html = format!(
            "<!doctype html><html><head><style>html,body{{margin:0;background:#000}}video,canvas{{display:block}}</style></head><body>{body}<script>window.log = []; for (const el of document.querySelectorAll('video,audio')) for (const type of {EVENTS}) el.addEventListener(type, () => log.push(type));</script></body></html>"
        );
        let (sender, wakes) = channel();
        let sender = Mutex::new(sender);
        let config = HostConfig::html(&html, Some(base.clone()))
            .size(f64::from(WIDTH), f64::from(HEIGHT))
            .wake(move || {
                let _ = sender
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .send(());
            });
        let mut session = HostSession::new(config).expect("the page loads");
        let posts = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&posts);
        session.on_post_message(move |message| sink.borrow_mut().push(message.to_owned()));
        let mut page = Self {
            session,
            wakes,
            posts,
        };
        page.pump();
        page
    }

    fn pump(&mut self) {
        self.session.poll();
        let wait = self
            .session
            .next_wake()
            .map_or(MAX_WAIT, |at| at.saturating_duration_since(Instant::now()))
            .min(MAX_WAIT);
        let _ = self.wakes.recv_timeout(wait);
        self.session.poll();
    }

    fn run(&mut self, code: &str) {
        self.session.eval(code);
    }

    fn query(&mut self, expression: &str) -> String {
        self.posts.borrow_mut().clear();
        self.session
            .eval(&format!("ipc.postMessage(String({expression}))"));
        let posted = self.posts.borrow_mut().pop();
        posted.unwrap_or_else(|| {
            let errors = self.session.document_mut().take_js_errors();
            panic!("evaluating {expression} posted nothing; errors: {errors:?}")
        })
    }

    fn number(&mut self, expression: &str) -> f64 {
        let text = self.query(expression);
        text.parse()
            .unwrap_or_else(|_| panic!("{expression} gave {text}, not a number"))
    }

    fn wait_for(&mut self, expression: &str) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if self.query(expression) == "true" {
                return;
            }
            if Instant::now() > deadline {
                let log = self.query("log.join(' ')");
                let errors = self.session.document_mut().take_js_errors();
                panic!("timed out waiting for {expression}; events: {log}; errors: {errors:?}");
            }
            self.pump();
        }
    }

    fn pump_for(&mut self, duration: Duration) {
        let until = Instant::now() + duration;
        while Instant::now() < until {
            self.pump();
        }
    }

    fn render(&mut self) -> Vec<u8> {
        self.session.resolve();
        let mut inner = self.session.document_mut().inner_mut();
        render_to_buffer::<VelloCpuImageRenderer, _>(
            |scene| paint_scene(scene, &mut inner, 1.0, WIDTH, HEIGHT, 0, 0),
            WIDTH,
            HEIGHT,
        )
    }

    fn assert_clean(&mut self) {
        assert_eq!(
            self.session.document_mut().take_js_errors(),
            Vec::<String>::new()
        );
    }
}

fn pixel(buffer: &[u8], x: u32, y: u32) -> [u8; 3] {
    let start = usize::try_from((y * WIDTH + x) * 4).expect("the pixel offset fits");
    [buffer[start], buffer[start + 1], buffer[start + 2]]
}

fn is_red(rgb: [u8; 3]) -> bool {
    rgb[0] > 180 && rgb[1] < 80 && rgb[2] < 80
}

fn is_blue(rgb: [u8; 3]) -> bool {
    rgb[2] > 180 && rgb[0] < 80 && rgb[1] < 80
}

const VIDEO_PAGE: &str = "<video id='v' src='clip.mp4' preload='auto'></video><canvas id='c' width='160' height='120'></canvas>";

#[test]
fn a_video_loads_metadata_and_paints_its_first_frame() {
    let server = serve();
    let mut page = Page::open(VIDEO_PAGE, &server.base);
    page.wait_for("v.readyState >= 2");
    assert_eq!(page.query("v.videoWidth + 'x' + v.videoHeight"), "160x120");
    let duration = page.number("v.duration");
    assert!((duration - 2.0).abs() < 0.15, "duration was {duration}");
    assert_eq!(
        page.query("v.currentSrc"),
        format!("{}clip.mp4", server.base)
    );
    assert_eq!(
        page.query("log.slice(0, 3).join(' ')"),
        "loadstart durationchange loadedmetadata"
    );
    assert_eq!(page.query("log.includes('loadeddata')"), "true");
    assert_eq!(page.query("v.paused"), "true");

    let frame = page.render();
    let painted = pixel(&frame, 80, 60);
    assert!(is_red(painted), "the video painted {painted:?}");
    assert!(is_red(pixel(&frame, 5, 5)));
    assert!(is_red(pixel(&frame, 154, 114)));
    let below = pixel(&frame, 80, 200);
    assert!(
        below.iter().all(|channel| *channel < 20),
        "the canvas starts black, got {below:?}"
    );

    page.run("const ctx = c.getContext('2d'); ctx.drawImage(v, 0, 0, 160, 120, 0, 0, 160, 120);");
    let copied = page.query("[...c.getContext('2d').getImageData(80, 60, 1, 1).data].join()");
    let channels: Vec<u8> = copied
        .split(',')
        .map(|part| part.parse().expect("a channel value"))
        .collect();
    assert!(
        is_red([channels[0], channels[1], channels[2]]),
        "drawImage copied {copied}"
    );
    let frame = page.render();
    assert!(
        is_red(pixel(&frame, 80, 180)),
        "the canvas shows the copied frame"
    );

    let requests = server.requests();
    assert!(
        requests
            .iter()
            .all(|(path, start)| path == "/clip.mp4" && start.is_some()),
        "every media request asks for a byte range: {requests:?}"
    );
    assert!(!requests.is_empty(), "the video loads over HTTP");
    page.assert_clean();
}

#[test]
fn playback_advances_reports_frames_seeks_and_pauses() {
    let server = serve();
    let mut page = Page::open(VIDEO_PAGE, &server.base);
    page.wait_for("v.readyState === 4");
    page.run(
        "window.frames = []; const tick = (now, meta) => { frames.push(meta); v.requestVideoFrameCallback(tick); }; v.requestVideoFrameCallback(tick); v.play().then(() => log.push('resolved'));",
    );
    page.wait_for("v.currentTime > 0.45");
    assert_eq!(page.query("v.paused"), "false");
    assert_eq!(
        page.query("log.includes('play') && log.includes('playing') && log.includes('resolved')"),
        "true"
    );
    assert_eq!(
        page.query("log.indexOf('play') < log.indexOf('playing')"),
        "true"
    );
    page.wait_for("frames.length >= 3");
    assert_eq!(
        page.query("Object.keys(frames[0]).sort().join()"),
        "expectedDisplayTime,height,mediaTime,presentationTime,presentedFrames,processingDuration,width"
    );
    assert_eq!(
        page.query("frames.every(f => f.width === 160 && f.height === 120)"),
        "true"
    );
    assert_eq!(
        page.query("frames.every((f, i) => i === 0 || (f.mediaTime > frames[i - 1].mediaTime && f.presentedFrames > frames[i - 1].presentedFrames))"),
        "true"
    );
    assert_eq!(
        page.query("frames.at(-1).mediaTime <= v.currentTime + 0.05"),
        "true"
    );

    page.run("v.currentTime = 1.5;");
    assert_eq!(page.query("v.seeking"), "true");
    page.wait_for("log.includes('seeked')");
    assert_eq!(
        page.query("log.indexOf('seeking') < log.indexOf('seeked')"),
        "true"
    );
    assert_eq!(page.query("v.seeking"), "false");
    page.run("v.pause();");
    assert_eq!(page.query("v.paused"), "true");
    let paused_at = page.number("v.currentTime");
    assert!((1.5..1.95).contains(&paused_at), "paused at {paused_at}");
    page.pump();
    assert_eq!(page.query("log.includes('pause')"), "true");
    page.pump_for(Duration::from_millis(300));
    let later = page.number("v.currentTime");
    assert!(
        (later - paused_at).abs() < 1e-6,
        "time moved from {paused_at} to {later} while paused"
    );
    let frame = page.render();
    let painted = pixel(&frame, 80, 60);
    assert!(
        is_blue(painted),
        "after seeking the video painted {painted:?}"
    );

    page.run("v.currentTime = 1.8; v.play();");
    page.wait_for("log.includes('ended')");
    assert_eq!(page.query("v.ended && v.paused"), "true");
    let end = page.number("v.currentTime");
    assert!(
        (end - page.number("v.duration")).abs() < 1e-6,
        "ended at {end}"
    );
    page.run("v.playbackRate = 2;");
    page.pump();
    assert_eq!(page.query("log.includes('ratechange')"), "true");
    page.assert_clean();
}

#[test]
fn seeking_back_to_the_start_shows_the_first_frame_again() {
    let server = serve();
    let mut page = Page::open(VIDEO_PAGE, &server.base);
    page.wait_for("v.readyState === 4");
    assert!(is_red(pixel(&page.render(), 80, 60)), "the first frame");
    page.run("v.currentTime = 1.5;");
    page.wait_for("log.filter((name) => name === 'seeked').length === 1");
    let later = pixel(&page.render(), 80, 60);
    assert!(is_blue(later), "after seeking forward {later:?}");
    page.run("v.currentTime = 0;");
    page.wait_for("log.filter((name) => name === 'seeked').length === 2");
    let start = pixel(&page.render(), 80, 60);
    assert!(is_red(start), "after seeking back {start:?}");
    page.assert_clean();
}

#[test]
fn audio_plays_through_an_audio_context_gain_chain() {
    let server = serve();
    let mut page = Page::open("<audio id='a' src='tone.wav'></audio>", &server.base);
    page.run(
        "window.ctx = new AudioContext(); window.source = ctx.createMediaElementSource(a); window.gain = ctx.createGain(); gain.gain.value = 2.0; window.chained = source.connect(gain) === gain && gain.connect(ctx.destination) === ctx.destination;",
    );
    assert_eq!(page.query("chained"), "true");
    assert_eq!(page.query("gain.gain.value"), "2");
    assert_eq!(
        page.query("ctx.state + ' ' + ctx.sampleRate"),
        "running 48000"
    );
    assert_eq!(page.query("typeof webkitAudioContext === 'function' && gain instanceof GainNode && gain.gain instanceof AudioParam"), "true");
    assert_eq!(
        page.query("(() => { try { ctx.createMediaElementSource(a); return 'reused'; } catch (e) { return e.name; } })()"),
        "InvalidStateError"
    );
    page.wait_for("a.readyState === 4");
    let duration = page.number("a.duration");
    assert!((duration - 1.0).abs() < 0.05, "duration was {duration}");
    page.run("Promise.all([ctx.resume(), a.play()]).then(() => log.push('started'));");
    page.wait_for("log.includes('started') && a.currentTime > 0.25");
    page.run("gain.gain.setValueAtTime(0.5, ctx.currentTime); a.volume = 0.5; a.muted = true; a.muted = false;");
    assert_eq!(page.query("gain.gain.value"), "0.5");
    assert_eq!(
        page.query("['x', NaN, -0.1, 1.5].map(v => { try { a.volume = v; return 'ok'; } catch (e) { return e.name; } }).join()"),
        "IndexSizeError,IndexSizeError,IndexSizeError,IndexSizeError"
    );
    page.run("a.pause(); ctx.suspend().then(() => log.push('suspended:' + ctx.state)).then(() => ctx.close()).then(() => log.push('closed:' + ctx.state));");
    page.wait_for("log.includes('closed:closed')");
    assert_eq!(page.query("log.includes('suspended:suspended')"), "true");
    assert_eq!(page.query("a.paused"), "true");
    assert_eq!(
        page.query("log.filter(t => t === 'volumechange').length"),
        "3"
    );
    page.run("ctx.resume().catch(e => log.push('resume:' + e.name));");
    page.pump();
    assert_eq!(
        page.query("log.includes('resume:InvalidStateError')"),
        "true"
    );

    assert_eq!(
        page.query("[a.canPlayType('video/mp4; codecs=\"avc1.42E01E, mp4a.40.2\"'), a.canPlayType('audio/mpeg'), a.canPlayType('audio/wav'), a.canPlayType('video/webm'), a.canPlayType('video/ogg'), a.canPlayType('video/mp4; codecs=\"theora\"')].join()"),
        "probably,maybe,maybe,maybe,,"
    );
    page.run("window.extra = new Audio('tone.wav');");
    assert_eq!(page.query("extra instanceof HTMLAudioElement && extra instanceof HTMLMediaElement && extra.src.endsWith('/tone.wav')"), "true");
    assert_eq!(page.query("extra.networkState === HTMLMediaElement.NETWORK_LOADING && extra.HAVE_ENOUGH_DATA === 4"), "true");
    page.assert_clean();
}

#[test]
fn an_audio_element_plays_the_aac_track_of_an_mp4_over_http() {
    let server = serve();
    let mut page = Page::open("<audio id='a' src='clip.mp4'></audio>", &server.base);
    page.wait_for("a.readyState === 4");
    let duration = page.number("a.duration");
    assert!((duration - 2.0).abs() < 0.15, "duration was {duration}");
    page.run("a.play();");
    page.wait_for("a.currentTime > 0.3");
    assert_eq!(page.query("a.paused"), "false");
    page.run("a.currentTime = 1.2;");
    page.wait_for("!a.seeking");
    let seeked = page.number("a.currentTime");
    assert!((1.2..1.7).contains(&seeked), "seeked to {seeked}");
    page.run("a.pause();");
    assert_eq!(page.query("a.paused"), "true");
    page.assert_clean();
}

#[test]
fn a_missing_source_reports_an_error_and_play_rejects() {
    let server = serve();
    let mut page = Page::open("<video id='v' src='missing.mp4'></video>", &server.base);
    page.wait_for("log.includes('error')");
    assert_eq!(page.query("v.error instanceof MediaError && v.error.code === MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED"), "true");
    assert_eq!(page.query("v.error.message.includes('404')"), "true");
    assert_eq!(
        page.query("v.networkState === v.NETWORK_NO_SOURCE && v.readyState === 0"),
        "true"
    );
    page.run("v.play().then(() => log.push('played'), e => log.push('rejected:' + e.name));");
    page.pump();
    assert_eq!(page.query("log.at(-1)"), "rejected:NotSupportedError");
    page.run("window.empty = document.createElement('video'); empty.play().catch(e => log.push('empty:' + e.name));");
    page.pump();
    assert_eq!(page.query("log.at(-1)"), "empty:NotSupportedError");
    page.assert_clean();
}

#[test]
fn dropping_a_playing_session_stops_media_promptly() {
    let server = serve();
    let mut page = Page::open(
        "<video id='v' src='clip.mp4' loop></video><audio id='a' src='tone.wav' loop></audio>",
        &server.base,
    );
    page.wait_for("v.readyState === 4 && a.readyState === 4");
    page.run("v.play(); a.play();");
    page.wait_for("v.currentTime > 0.2 && a.currentTime > 0.2");
    page.run("v.remove();");
    page.wait_for("v.paused");
    page.run("v.src = 'clip.mp4';");
    page.pump();
    let started = Instant::now();
    drop(page);
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(3),
        "dropping the session took {elapsed:?}"
    );
}

#[test]
fn dropping_a_session_while_media_loads_stops_promptly() {
    let server = serve();
    for pumps in 0..6 {
        let mut page = Page::open(
            "<video src='clip.mp4' preload='auto'></video>",
            &server.base,
        );
        page.run("window.extra = [new Audio('tone.wav'), new Audio('clip.mp4')];");
        for _ in 0..pumps {
            page.pump();
        }
        let started = Instant::now();
        drop(page);
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_secs(3),
            "dropping after {pumps} pumps took {elapsed:?}"
        );
    }
}
