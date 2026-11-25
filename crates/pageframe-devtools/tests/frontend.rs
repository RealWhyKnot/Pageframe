//! Tests for serving the embedded DevTools frontend
#![cfg(feature = "embedded-frontend")]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use pageframe_devtools::{CdpServer, DevtoolsWaker, frontend_url};

struct NoopWaker;
impl DevtoolsWaker for NoopWaker {
    fn wake(&self) {}
}

/// GET `path`, returning the status line, headers and body
fn get(addr: SocketAddr, path: &str) -> (String, String, Vec<u8>) {
    let mut stream = TcpStream::connect(addr).unwrap();
    write!(stream, "GET {path} HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let head_end = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8(response[..head_end].to_vec()).unwrap();
    let (status, headers) = head.split_once("\r\n").unwrap_or((&head, ""));
    (
        status.to_string(),
        headers.to_string(),
        response[head_end + 4..].to_vec(),
    )
}

#[test]
fn serves_the_frontend() {
    let mut server = CdpServer::new(Arc::new(NoopWaker));
    server.start_listening("127.0.0.1:0");
    let addr = server
        .wait_for_local_addr(Duration::from_secs(10))
        .expect("server should start listening");

    let url = frontend_url(addr, "7.42");
    assert_eq!(
        url,
        format!("http://{addr}/devtools_app.html?ws={addr}/devtools/page/7.42")
    );

    // The page (requested with its query string), which loads the entrypoint
    let (status, headers, body) = get(addr, url.strip_prefix(&format!("http://{addr}")).unwrap());
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(headers.contains("Content-Type: text/html"));
    let script = "entrypoints/devtools_app/devtools_app.js";
    assert!(String::from_utf8(body).unwrap().contains(script));

    // Module scripts only run when served with a JavaScript MIME type
    let (status, headers, body) = get(addr, &format!("/{script}"));
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(headers.contains("Content-Type: text/javascript"));
    assert!(!body.is_empty());

    let (status, headers, _) = get(addr, "/core/i18n/locales/en-US.json");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(headers.contains("Content-Type: application/json"));

    assert_eq!(get(addr, "/missing.js").0, "HTTP/1.1 404 Not Found");
    assert_eq!(get(addr, "/../Cargo.toml").0, "HTTP/1.1 404 Not Found");
}

fn status_for(addr: SocketAddr, head: &str) -> String {
    let mut stream = TcpStream::connect(addr).expect("connect to the CDP server");
    stream
        .write_all(format!("{head}\r\n").as_bytes())
        .expect("send the request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("read the response");
    let text = String::from_utf8_lossy(&response).into_owned();
    text.lines().next().unwrap_or_default().to_string()
}

#[test]
fn refuses_requests_from_web_pages() {
    let mut server = CdpServer::new(Arc::new(NoopWaker));
    server.start_listening("127.0.0.1:0");
    let addr = server
        .wait_for_local_addr(Duration::from_secs(10))
        .expect("server should start listening");
    let forbidden = "HTTP/1.1 403 Forbidden";
    let upgrade = "Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13";

    assert_eq!(
        status_for(
            addr,
            "GET /json/list HTTP/1.1\r\nHost: evil.example:9222\r\n"
        ),
        forbidden
    );
    assert_eq!(status_for(addr, "GET /json/list HTTP/1.1\r\n"), forbidden);
    assert_eq!(
        status_for(
            addr,
            &format!(
                "GET /json/version HTTP/1.1\r\nHost: {addr}\r\nOrigin: https://evil.example\r\n"
            )
        ),
        forbidden
    );
    assert_eq!(
        status_for(
            addr,
            &format!(
                "GET /devtools/page/1 HTTP/1.1\r\nHost: {addr}\r\nOrigin: https://evil.example\r\n{upgrade}\r\n"
            )
        ),
        forbidden
    );
    assert_eq!(
        status_for(
            addr,
            &format!("GET /json/version HTTP/1.1\r\nHost: {addr}\r\n")
        ),
        "HTTP/1.1 200 OK"
    );
    assert_eq!(
        status_for(
            addr,
            &format!("GET /json/version HTTP/1.1\r\nHost: {addr}\r\nOrigin: http://{addr}\r\n")
        ),
        "HTTP/1.1 200 OK"
    );
}
