use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use pageframe_dom::{Document, DocumentConfig};
use pageframe_script::{HttpClient, HttpRequest, ScriptDocument};
use url::Url;

struct Received {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Received {
    fn header(&self, name: &str) -> &str {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .unwrap_or("")
    }
}

fn serve(handler: fn(&Received) -> String) -> Url {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let mut parts = line.split_whitespace();
                let method = parts.next().unwrap_or("").to_string();
                let path = parts.next().unwrap_or("").to_string();
                let mut headers = Vec::new();
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    let header = header.trim_end();
                    if header.is_empty() {
                        break;
                    }
                    let (name, value) = header.split_once(':').unwrap();
                    headers.push((name.trim().to_string(), value.trim().to_string()));
                }
                let length = headers
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    .map(|(_, value)| value.parse::<usize>().unwrap())
                    .unwrap_or(0);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let received = Received {
                    method,
                    path,
                    headers,
                    body: String::from_utf8(body).unwrap(),
                };
                let _ = stream.write_all(handler(&received).as_bytes());
            });
        }
    });
    Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap()
}

fn reply(status: &str, extra: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
        body.len()
    )
}

fn page(base: &Url, html: &str, client: HttpClient) -> ScriptDocument {
    let config = DocumentConfig {
        base_url: Some(base.to_string()),
        ..Default::default()
    };
    let mut doc = ScriptDocument::from_html(html, config)
        .without_timer_thread()
        .with_fetcher(client);
    doc.execute_scripts();
    doc
}

fn text_of(doc: &ScriptDocument, selector: &str) -> String {
    let inner = doc.inner();
    let id = inner.query_selector(selector).unwrap().unwrap();
    inner.get_node(id).unwrap().text_content()
}

fn pump_until(doc: &mut ScriptDocument, selector: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        doc.poll(None);
        let text = text_of(doc, selector);
        if !text.is_empty() || Instant::now() > deadline {
            return text;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn login_server(request: &Received) -> String {
    match request.path.as_str() {
        "/?t=secret" => reply(
            "302 Found",
            "Location: /\r\nSet-Cookie: sid=abc; Path=/; HttpOnly\r\n",
            "",
        ),
        "/" if request.header("cookie") == "sid=abc" => reply("200 OK", "", "home"),
        "/app.js" if request.header("cookie") == "sid=abc" => reply(
            "200 OK",
            "Content-Type: text/javascript\r\n",
            "document.getElementById('out').textContent = 'loaded';",
        ),
        _ => reply("401 Unauthorized", "", "no"),
    }
}

#[test]
fn http_scripts_load_after_cookie_redirect() {
    let base = serve(login_server);
    let client = HttpClient::new();
    let landing = client
        .request(&HttpRequest::get(base.join("/?t=secret").unwrap()))
        .unwrap();
    assert_eq!(landing.status, 200);
    assert!(landing.redirected);
    assert_eq!(landing.url, base);
    assert_eq!(String::from_utf8(landing.body).unwrap(), "home");
    let mut doc = page(
        &base,
        r#"<div id="out"></div><script src="/app.js"></script>"#,
        client,
    );
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
    assert_eq!(text_of(&doc, "#out"), "loaded");
}

fn echo_server(request: &Received) -> String {
    let body = format!(
        "{{\"method\":\"{}\",\"type\":\"{}\",\"custom\":\"{}\",\"body\":{}}}",
        request.method,
        request.header("content-type"),
        request.header("x-custom"),
        request.body
    );
    reply(
        "201 Created",
        "Content-Type: application/json\r\nX-Reply: yes\r\n",
        &body,
    )
}

#[test]
fn fetch_posts_json_without_blocking() {
    let base = serve(echo_server);
    let mut doc = page(
        &base,
        r#"<div id="out"></div><div id="sync"></div><script>
            let settled = false;
            fetch("/api/echo", {
                method: "POST",
                headers: new Headers({ "Content-Type": "application/json", "X-Custom": "42" }),
                body: JSON.stringify({ a: 1 }),
                signal: AbortSignal.timeout(60000),
            })
                .then((r) => r.json().then((json) => [r.status, r.statusText, r.headers.get("x-reply"), json]))
                .then(([status, text, header, json]) => {
                    settled = true;
                    document.getElementById("out").textContent = [
                        status, text, header, json.method, json.type, json.custom, json.body.a,
                    ].join("|");
                });
            document.getElementById("sync").textContent = String(settled);
        </script>"#,
        HttpClient::new(),
    );
    assert_eq!(text_of(&doc, "#sync"), "false");
    assert_eq!(
        pump_until(&mut doc, "#out"),
        "201|Created|yes|POST|application/json|42|1"
    );
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn aborted_fetch_rejects_with_abort_error() {
    let base = serve(echo_server);
    let mut doc = page(
        &base,
        r#"<div id="out"></div><script>
            const controller = new AbortController();
            const pending = fetch("/slow", { signal: controller.signal });
            controller.abort();
            pending.catch((e) => {
                document.getElementById("out").textContent = e.name + " " + controller.signal.aborted;
            });
        </script>"#,
        HttpClient::new(),
    );
    assert_eq!(pump_until(&mut doc, "#out"), "AbortError true");
}

fn events_server(request: &Received) -> String {
    if request.header("accept") != "text/event-stream" {
        return reply("406 Not Acceptable", "", "");
    }
    let stream = ": hi\nretry: 50\nevent: hello\ndata: one\ndata: two\nid: 7\n\ndata: plain\n\n";
    reply("200 OK", "Content-Type: text/event-stream\r\n", stream)
}

#[test]
fn event_source_dispatches_named_and_default_messages() {
    let base = serve(events_server);
    let mut doc = page(
        &base,
        r##"<div id="out"></div><script>
            const seen = [];
            const source = new EventSource("/events");
            source.onopen = () => seen.push("open " + source.readyState);
            source.addEventListener("hello", (e) => seen.push(e.data + "#" + e.lastEventId));
            source.onmessage = (e) => {
                seen.push(e.data);
                source.close();
                document.getElementById("out").textContent = seen.join(",") + " " + source.readyState;
            };
        </script>"##,
        HttpClient::new(),
    );
    assert_eq!(pump_until(&mut doc, "#out"), "open 1,one\ntwo#7,plain 2");
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn fetch_rejects_methods_and_headers_that_would_split_the_request() {
    let base = serve(echo_server);
    let mut doc = page(
        &base,
        r#"<div id="out"></div><script>
            const seen = [];
            fetch("/api/echo", { method: "GET /x HTTP/1.1\r\nX-Evil: 1\r\n" })
                .catch((e) => seen.push(e.name))
                .then(() => fetch("/api/echo", { method: "POST", headers: [["X-Bad:Name", "1"], ["X-Custom", "ok"]], body: "{}" }))
                .then((r) => r.json())
                .then((json) => {
                    seen.push(json.method + " " + json.custom);
                    document.getElementById("out").textContent = seen.join("|");
                });
        </script>"#,
        HttpClient::new(),
    );
    assert_eq!(pump_until(&mut doc, "#out"), "TypeError|POST ok");
}

#[test]
fn a_post_survives_a_server_that_answers_after_one_read() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let port = listener.local_addr().expect("the port").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0; 8192];
            if stream.read(&mut buffer).is_err() {
                continue;
            }
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
        }
    });
    let url = Url::parse(&format!("http://127.0.0.1:{port}/notice")).expect("a valid URL");
    let client = HttpClient::new();
    let failures: Vec<String> = (0..300)
        .filter_map(|_| {
            let request = HttpRequest {
                method: "POST".to_string(),
                body: b"{}".to_vec(),
                ..HttpRequest::get(url.clone())
            };
            match client.request(&request) {
                Ok(response) if response.body == b"ok" => None,
                Ok(response) => Some(format!("{} {:?}", response.status, response.body)),
                Err(error) => Some(error.to_string()),
            }
        })
        .collect();
    assert_eq!(failures, Vec::<String>::new());
}
