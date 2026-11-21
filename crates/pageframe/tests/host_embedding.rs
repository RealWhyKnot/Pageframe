use std::cell::{Cell, RefCell};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use pageframe::events::{Code, Key, KeyEvent, KeyState, Location, Modifiers, MouseEventButton};
use pageframe::host::{HostConfig, HostEvent, HostInputEvent, HostSession, IpcReply};
use serde_json::json;
use url::Url;

fn route(path: &str) -> (&'static str, &'static str, String) {
    match path {
        "/" => (
            "200 OK",
            "text/html",
            String::from(
                r#"<!doctype html><html><head><title>Home</title><script src="/app.js"></script></head>
<body><p id="status">loading</p><script>ipc.postMessage("inline:" + document.getElementById("status").textContent);</script></body></html>"#,
            ),
        ),
        "/app.js" => (
            "200 OK",
            "text/javascript",
            String::from(
                r#"document.addEventListener("DOMContentLoaded", () => { document.getElementById("status").textContent = "ready"; ipc.postMessage("external:" + location.pathname); });"#,
            ),
        ),
        "/second" => (
            "200 OK",
            "text/html",
            String::from(
                r#"<!doctype html><body><script>ipc.postMessage("second:" + location.href);</script></body>"#,
            ),
        ),
        hop if hop.starts_with("/hop/") => ("302 Found", "text/plain", String::new()),
        _ => ("404 Not Found", "text/plain", String::from("missing")),
    }
}

fn serve() -> Url {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let port = listener.local_addr().expect("read the local port").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().expect("clone the stream"));
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    return;
                }
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).is_err() || header.trim_end().is_empty() {
                        break;
                    }
                }
                let (status, kind, body) = route(&path);
                let location = path
                    .strip_prefix("/hop/")
                    .map(|target| format!("Location: {target}\r\n"))
                    .unwrap_or_default();
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\n{location}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    Url::parse(&format!("http://127.0.0.1:{port}/")).expect("a valid local URL")
}

fn collect_posts(session: &mut HostSession) -> Rc<RefCell<Vec<String>>> {
    let posts = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&posts);
    session.on_post_message(move |message| sink.borrow_mut().push(message.to_owned()));
    posts
}

fn taken(posts: &Rc<RefCell<Vec<String>>>) -> Vec<String> {
    std::mem::take(&mut *posts.borrow_mut())
}

fn blank(html: &str) -> HostSession {
    let base = Url::parse("http://app.test/").expect("a valid base URL");
    HostSession::new(HostConfig::html(html, Some(base))).expect("inline HTML always loads")
}

fn assert_clean(session: &mut HostSession) {
    assert_eq!(
        session.document_mut().take_js_errors(),
        Vec::<String>::new()
    );
}

#[test]
fn loads_http_url_and_runs_inline_and_external_scripts() {
    let base = serve();
    let mut session = HostSession::new(HostConfig::url(base.clone())).unwrap();
    let posts = collect_posts(&mut session);
    assert_eq!(session.url(), Some(&base));
    assert!(session.next_wake().is_some());
    session.poll();
    assert_eq!(taken(&posts), ["inline:loading", "external:/"]);
    session.eval(
        r#"ipc.postMessage(document.title + "|" + document.getElementById("status").textContent)"#,
    );
    assert_eq!(taken(&posts), ["Home|ready"]);
    assert_clean(&mut session);
}

#[test]
fn load_errors_are_reported() {
    let base = serve();
    let missing = base.join("/nope.html").unwrap();
    let error = HostSession::new(HostConfig::url(Url::parse("ftp://example.test/").unwrap()))
        .err()
        .unwrap();
    assert!(error.to_string().contains("unsupported scheme"), "{error}");
    let session = HostSession::new(HostConfig::url(missing));
    assert!(session.is_ok());
}

#[test]
fn invoke_resolves_with_sync_replies_and_rejects_errors() {
    let mut session = blank("<p></p>");
    let posts = collect_posts(&mut session);
    session.on_ipc(|request| match request.channel.as_str() {
        "sum" => {
            let total = request.payload["a"].as_i64().unwrap_or(0)
                + request.payload["b"].as_i64().unwrap_or(0);
            IpcReply::Ok(json!({ "total": total }))
        }
        _ => IpcReply::Err(format!("unknown command {}", request.channel)),
    });
    session.eval(
        r#"
        ipc.invoke("sum", { a: 2, b: 5 }).then((value) => ipc.postMessage("sum:" + value.total));
        ipc.invoke("nope").catch((error) => ipc.postMessage("error:" + error.message));
    "#,
    );
    assert_eq!(taken(&posts), ["sum:7", "error:unknown command nope"]);
    assert_clean(&mut session);
}

#[test]
fn invoke_during_page_load_waits_for_the_handler() {
    let mut session = blank(
        r#"<script>ipc.invoke("boot", [1, 2]).then((value) => ipc.postMessage("boot:" + value));</script>"#,
    );
    let posts = collect_posts(&mut session);
    session.on_ipc(|request| IpcReply::Ok(json!(request.payload.as_array().map_or(0, Vec::len))));
    assert!(session.next_wake().is_some());
    session.poll();
    assert_eq!(taken(&posts), ["boot:2"]);
}

#[test]
fn later_replies_settle_from_the_session_and_from_other_threads() {
    let mut session = blank("<p></p>");
    let posts = collect_posts(&mut session);
    let pending = Rc::new(RefCell::new(Vec::new()));
    let ids = Rc::clone(&pending);
    session.on_ipc(move |request| {
        ids.borrow_mut().push(request.id);
        IpcReply::Later
    });
    session.eval(
        r#"
        ipc.invoke("first").then((value) => ipc.postMessage("first:" + JSON.stringify(value)));
        ipc.invoke("second").catch((error) => ipc.postMessage("second:" + error.message));
    "#,
    );
    let ids = pending.borrow().clone();
    assert_eq!(ids.len(), 2);
    assert_eq!(taken(&posts), Vec::<String>::new());
    session.reply(ids[0], Ok(json!({ "done": true })));
    assert_eq!(taken(&posts), [r#"first:{"done":true}"#]);
    let handle = session.handle();
    let second = ids[1];
    std::thread::spawn(move || handle.reply(second, Err(String::from("worker failed"))))
        .join()
        .unwrap();
    let wake = session.next_wake().unwrap();
    assert!(wake <= Instant::now());
    session.poll();
    assert_eq!(taken(&posts), ["second:worker failed"]);
    assert_clean(&mut session);
}

#[test]
fn emit_reaches_listeners_and_unsubscribe_stops_them() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&wakes);
    let base = Url::parse("http://app.test/").unwrap();
    let config = HostConfig::html("<p></p>", Some(base)).wake(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    let mut session = HostSession::new(config).unwrap();
    let posts = collect_posts(&mut session);
    session.eval(r#"globalThis.off = ipc.on("progress", (payload) => ipc.postMessage("progress:" + payload.percent));"#);
    session.emit("progress", &json!({ "percent": 40 }));
    assert_eq!(taken(&posts), ["progress:40"]);
    let before = wakes.load(Ordering::SeqCst);
    let handle = session.handle();
    std::thread::spawn(move || handle.emit("progress", json!({ "percent": 90 })))
        .join()
        .unwrap();
    assert!(wakes.load(Ordering::SeqCst) > before);
    session.poll();
    assert_eq!(taken(&posts), ["progress:90"]);
    session.eval("off()");
    session.emit("progress", &json!({ "percent": 100 }));
    assert_eq!(taken(&posts), Vec::<String>::new());
    assert_clean(&mut session);
}

#[test]
fn next_wake_tracks_script_timers() {
    let mut session = blank("<p></p>");
    let posts = collect_posts(&mut session);
    session.poll();
    assert_eq!(session.next_wake(), None);
    let start = Instant::now();
    session.eval(r#"setTimeout(() => ipc.postMessage("fired"), 120);"#);
    let deadline = session.next_wake().unwrap();
    let wait = deadline.saturating_duration_since(start);
    assert!(
        wait >= Duration::from_millis(100) && wait <= Duration::from_millis(400),
        "{wait:?}"
    );
    session.poll();
    assert_eq!(taken(&posts), Vec::<String>::new());
    std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
    assert!(session.poll());
    assert_eq!(taken(&posts), ["fired"]);
    assert_eq!(session.next_wake(), None);
}

#[test]
fn pointer_click_runs_the_page_handler_and_requests_a_redraw() {
    let base = Url::parse("http://app.test/").unwrap();
    let html = r#"<body style="margin:0">
<button id="go" style="position:absolute;left:0;top:0;width:100px;height:40px;margin:0">Go</button>
<script>
document.getElementById("go").addEventListener("click", (event) => {
    event.target.textContent = "Done";
    ipc.postMessage("click:" + event.clientX + "," + event.clientY);
});
</script></body>"#;
    let config = HostConfig::html(html, Some(base))
        .size(300.0, 200.0)
        .scale_factor(2.0);
    let mut session = HostSession::new(config).unwrap();
    let posts = collect_posts(&mut session);
    assert_eq!(session.size(), (600, 400));
    session.poll();
    session.take_redraw();
    assert!(!session.handle_input(HostInputEvent::ModifiersChanged(Default::default())));
    let down = HostInputEvent::PointerDown {
        x: 100.0,
        y: 40.0,
        button: MouseEventButton::Main,
    };
    let up = HostInputEvent::PointerUp {
        x: 100.0,
        y: 40.0,
        button: MouseEventButton::Main,
    };
    session.handle_input(down);
    assert!(session.handle_input(up));
    assert_eq!(taken(&posts), ["click:50,20"]);
    session.eval(r#"ipc.postMessage(document.getElementById("go").textContent)"#);
    assert_eq!(taken(&posts), ["Done"]);
    assert!(session.handle_input(HostInputEvent::Resized {
        width: 800,
        height: 500
    }));
    assert_eq!(session.size(), (800, 500));
    assert_clean(&mut session);
}

#[test]
fn navigation_follows_policy_and_reports_blocked_targets() {
    let base = serve();
    let mut session = HostSession::new(HostConfig::url(base.clone())).unwrap();
    let posts = collect_posts(&mut session);
    session.poll();
    taken(&posts);
    session.take_events();
    session.eval(r#"location.href = "https://elsewhere.test/page";"#);
    session.poll();
    let blocked = Url::parse("https://elsewhere.test/page").unwrap();
    assert!(
        session
            .take_events()
            .contains(&HostEvent::NavigationBlocked(blocked))
    );
    session.eval(r#"location.href = "/second";"#);
    session.poll();
    let second = base.join("/second").unwrap();
    assert!(
        session
            .take_events()
            .contains(&HostEvent::Navigated(second.clone()))
    );
    assert_eq!(session.url(), Some(&second));
    session.poll();
    assert_eq!(taken(&posts), [format!("second:{second}")]);
}

#[test]
fn fragment_link_clicks_stay_in_the_document() {
    let base = Url::parse("http://app.test/index.html").unwrap();
    let html = r##"<body style="margin:0">
<a id="link" href="#/settings" style="display:block;width:100px;height:30px">Settings</a>
<script>
globalThis.loaded = (globalThis.loaded || 0) + 1;
addEventListener("hashchange", () => ipc.postMessage("hash:" + location.hash + ":" + loaded));
</script></body>"##;
    let mut session = HostSession::new(HostConfig::html(html, Some(base))).unwrap();
    let posts = collect_posts(&mut session);
    let navigations = Rc::new(Cell::new(0));
    let count = Rc::clone(&navigations);
    session.on_navigation(move |_| {
        count.set(count.get() + 1);
        true
    });
    session.poll();
    for event in [
        HostInputEvent::PointerDown {
            x: 10.0,
            y: 10.0,
            button: MouseEventButton::Main,
        },
        HostInputEvent::PointerUp {
            x: 10.0,
            y: 10.0,
            button: MouseEventButton::Main,
        },
    ] {
        session.handle_input(event);
    }
    session.poll();
    let deadline = session.next_wake().unwrap_or_else(Instant::now);
    std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
    session.poll();
    assert_eq!(taken(&posts), ["hash:#/settings:1"]);
    assert_eq!(navigations.get(), 0);
    assert_clean(&mut session);
}

#[test]
fn redirects_to_another_origin_are_checked_against_the_policy() {
    let base = serve();
    let other = serve();
    let mut session = HostSession::new(HostConfig::url(base.clone())).unwrap();
    session.poll();
    session.take_events();
    let target = other.join("/second").unwrap();
    session.eval(&format!(r#"location.href = "/hop/{target}";"#));
    session.poll();
    assert!(
        session
            .take_events()
            .contains(&HostEvent::NavigationBlocked(target))
    );
    assert_eq!(session.url(), Some(&base));
    let local = base.join("/second").unwrap();
    session.eval(&format!(r#"location.href = "/hop/{local}";"#));
    session.poll();
    assert_eq!(session.url(), Some(&local));
}

#[test]
fn zero_sizes_and_bad_scale_factors_keep_the_session_usable() {
    let base = Url::parse("http://app.test/").unwrap();
    let config = HostConfig::html("<p>hi</p>", Some(base))
        .size(f64::NAN, -20.0)
        .scale_factor(f64::INFINITY);
    let mut session = HostSession::new(config).unwrap();
    assert_eq!(session.size(), (0, 0));
    assert!((session.scale_factor() - 1.0).abs() < f64::EPSILON);
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e9] {
        session.handle_input(HostInputEvent::ScaleFactorChanged(scale));
        assert!((session.scale_factor() - 1.0).abs() < f64::EPSILON);
    }
    session.handle_input(HostInputEvent::Resized {
        width: 0,
        height: 0,
    });
    session.handle_input(HostInputEvent::PointerMoved { x: 5.0, y: 5.0 });
    session.resolve();
    session.poll();
    session.handle_input(HostInputEvent::ScaleFactorChanged(1.5));
    assert!((session.scale_factor() - 1.5).abs() < f64::EPSILON);
    assert_clean(&mut session);
}

#[test]
fn huge_sizes_are_clamped_to_a_safe_surface() {
    let base = Url::parse("http://app.test/").unwrap();
    let config = HostConfig::html("<p>hi</p>", Some(base))
        .size(1e12, 20_000.0)
        .scale_factor(2.0);
    let mut session = HostSession::new(config).unwrap();
    assert_eq!(session.size(), (16_384, 16_384));
    session.handle_input(HostInputEvent::Resized {
        width: u32::MAX,
        height: 900,
    });
    assert_eq!(session.size(), (16_384, 900));
    session.resolve();
    assert_clean(&mut session);
}

#[test]
fn repeated_state_events_are_coalesced() {
    let mut session = blank("<title>start</title><p></p>");
    session.take_events();
    for index in 0..1000 {
        session.eval(&format!(
            r#"document.querySelector("title").textContent = "t{index}";"#
        ));
        session.handle_input(HostInputEvent::PointerMoved {
            x: f64::from(index % 50),
            y: 1.0,
        });
        session.poll();
    }
    let events = session.take_events();
    assert!(events.len() <= 2, "{events:?}");
    let titles: Vec<&HostEvent> = events
        .iter()
        .filter(|event| matches!(event, HostEvent::TitleChanged(_)))
        .collect();
    assert_eq!(titles, [&HostEvent::TitleChanged(String::from("t999"))]);
}

fn click(session: &mut HostSession, x: f64, y: f64) {
    let button = MouseEventButton::Main;
    session.handle_input(HostInputEvent::PointerDown { x, y, button });
    session.handle_input(HostInputEvent::PointerUp { x, y, button });
}

fn url_changes(session: &mut HostSession) -> Vec<Url> {
    session
        .take_events()
        .into_iter()
        .filter_map(|event| match event {
            HostEvent::UrlChanged(url) => Some(url),
            _ => None,
        })
        .collect()
}

#[test]
fn same_document_url_changes_are_reported() {
    let base = Url::parse("http://app.test/index.html").unwrap();
    let html = r##"<body style="margin:0">
<a id="link" href="#/settings" style="display:block;width:100px;height:30px">Settings</a>
</body>"##;
    let mut session = HostSession::new(HostConfig::html(html, Some(base.clone()))).unwrap();
    let navigations = Rc::new(Cell::new(0));
    let count = Rc::clone(&navigations);
    session.on_navigation(move |_| {
        count.set(count.get() + 1);
        true
    });
    session.poll();
    session.take_events();
    click(&mut session, 10.0, 10.0);
    let settings = base.join("#/settings").unwrap();
    assert_eq!(url_changes(&mut session), std::slice::from_ref(&settings));
    assert_eq!(session.url(), Some(&settings));
    session.eval(r#"location.hash = "about";"#);
    session.eval(r#"history.pushState({ tab: 2 }, "", "/library?tab=2");"#);
    session.eval(r#"history.replaceState(null, "", "/library");"#);
    let expected = ["#about", "/library?tab=2", "/library"].map(|path| base.join(path).unwrap());
    assert_eq!(url_changes(&mut session), expected);
    session.eval(
        r#"__pageframe_ipc_send("invoke", "__pageframe_url_changed", "https://evil.test/");"#,
    );
    session.eval(r#"__pageframe_ipc_send("invoke", "__pageframe_url_changed", "not a url");"#);
    assert_eq!(url_changes(&mut session), Vec::<Url>::new());
    assert_eq!(session.url(), Some(&expected[2]));
    let posts = collect_posts(&mut session);
    session.eval(
        r#"ipc.invoke("__pageframe_url_changed", "/x").catch(() => ipc.postMessage("rejected"));"#,
    );
    session.poll();
    assert_eq!(taken(&posts), ["rejected"]);
    assert_eq!(navigations.get(), 0);
    assert_clean(&mut session);
}

#[test]
fn url_changes_during_page_load_are_reported() {
    let mut session = blank(r#"<script>history.replaceState(null, "", "/start#top");</script>"#);
    session.poll();
    let start = Url::parse("http://app.test/start#top").unwrap();
    assert_eq!(url_changes(&mut session), std::slice::from_ref(&start));
    assert_eq!(session.url(), Some(&start));
    assert_clean(&mut session);
}

fn paste_shortcut() -> KeyEvent {
    let modifiers = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    KeyEvent {
        key: Key::Character("v".into()),
        code: Code::KeyV,
        modifiers,
        location: Location::Standard,
        is_auto_repeating: false,
        is_composing: false,
        state: KeyState::Pressed,
        text: None,
    }
}

#[test]
fn paste_requests_the_clipboard_and_fills_the_focused_field() {
    let mut session = blank(
        r#"<body style="margin:0">
<input id="field" style="position:absolute;left:0;top:0;width:200px;height:30px;margin:0">
<script>
const field = document.getElementById("field");
field.addEventListener("input", () => ipc.postMessage("input:" + field.value));
</script></body>"#,
    );
    let posts = collect_posts(&mut session);
    session.poll();
    click(&mut session, 20.0, 10.0);
    session.take_events();
    session.handle_input(HostInputEvent::KeyDown(paste_shortcut()));
    let events = session.take_events();
    assert!(
        events.contains(&HostEvent::ClipboardRequested),
        "{events:?}"
    );
    assert!(session.handle_input(HostInputEvent::Paste(String::from("hello world"))));
    let long = "x".repeat(100_000);
    session.handle_input(HostInputEvent::Paste(long.clone()));
    session.eval(r#"ipc.postMessage("value:" + document.getElementById("field").value.length)"#);
    let posts = taken(&posts);
    assert_eq!(posts.first().map(String::as_str), Some("input:hello world"));
    let length = "hello world".len() + long.len();
    assert_eq!(posts.last(), Some(&format!("value:{length}")));
    assert_clean(&mut session);
}
