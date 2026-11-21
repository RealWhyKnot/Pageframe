use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use pageframe_dom::{Document, DocumentConfig};
use pageframe_script::{HttpClient, ScriptDocument};
use url::Url;

fn read_request(stream: &TcpStream) -> String {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line.trim_end().is_empty() {
            break;
        }
    }
    first.split_whitespace().nth(1).unwrap_or("").to_string()
}

fn serve(closed: Sender<()>) -> Url {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let closed = closed.clone();
            std::thread::spawn(move || {
                let path = read_request(&stream);
                if path == "/binary" {
                    let body = [0xff_u8, 0x00, 0x80, 0x41];
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(&body);
                    return;
                }
                let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: ready\n\n";
                let _ = stream.write_all(head.as_bytes());
                let mut buffer = [0_u8; 16];
                loop {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
                let _ = closed.send(());
            });
        }
    });
    Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap()
}

fn page(base: &Url, html: &str) -> ScriptDocument {
    let config = DocumentConfig {
        base_url: Some(base.to_string()),
        ..Default::default()
    };
    let mut doc = ScriptDocument::from_html(html, config)
        .without_timer_thread()
        .with_fetcher(HttpClient::new());
    doc.execute_scripts();
    doc
}

fn pump(doc: &mut ScriptDocument) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        doc.poll(None);
        let text = {
            let inner = doc.inner();
            let id = inner.query_selector("#out").unwrap().unwrap();
            inner.get_node(id).unwrap().text_content()
        };
        if !text.is_empty() || Instant::now() > deadline {
            return text;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn fetch_keeps_binary_bodies_intact() {
    let (sender, _receiver) = channel();
    let base = serve(sender);
    let mut doc = page(
        &base,
        r#"<div id="out"></div><script>
            Promise.all([
                fetch("/binary").then((r) => r.arrayBuffer()),
                fetch("/binary").then((r) => r.bytes()),
                fetch("/binary").then((r) => r.blob()),
                fetch("/binary").then((r) => r.text()),
            ]).then(([buffer, bytes, blob, text]) => {
                document.getElementById("out").textContent = [
                    [...new Uint8Array(buffer)].join("-"),
                    bytes instanceof Uint8Array, [...bytes].join("-"),
                    blob.size, blob.type, text.length, text.charCodeAt(0), text.at(-1),
                ].join(",");
            });
        </script>"#,
    );
    assert_eq!(
        pump(&mut doc),
        "255-0-128-65,true,255-0-128-65,4,application/octet-stream,4,65533,A"
    );
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn response_constructor_accepts_binary_bodies() {
    let (sender, _receiver) = channel();
    let base = serve(sender);
    let mut doc = page(
        &base,
        r#"<div id="out"></div><script>
            new Response(new Uint8Array([1, 2, 250])).arrayBuffer().then((buffer) => {
                document.getElementById("out").textContent = [...new Uint8Array(buffer)].join("-");
            });
        </script>"#,
    );
    assert_eq!(pump(&mut doc), "1-2-250");
}

#[test]
fn event_source_close_releases_the_connection_promptly() {
    let (sender, receiver) = channel();
    let base = serve(sender);
    let mut doc = page(
        &base,
        r#"<div id="out"></div><script>
            const source = new EventSource("/events");
            source.onmessage = (event) => {
                source.close();
                document.getElementById("out").textContent = event.data + " " + source.readyState;
            };
        </script>"#,
    );
    assert_eq!(pump(&mut doc), "ready 2");
    let started = Instant::now();
    assert!(receiver.recv_timeout(Duration::from_secs(3)).is_ok());
    assert!(started.elapsed() < Duration::from_secs(3));
}
