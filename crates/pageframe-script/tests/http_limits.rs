use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use pageframe_script::{HttpClient, HttpRequest};
use url::Url;

fn read_request(stream: &TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone test stream"));
    let mut line = String::new();
    while reader.read_line(&mut line).expect("read test request") > 0 {
        if line == "\r\n" {
            return;
        }
        line.clear();
    }
}

fn serve_once(respond: impl FnOnce(TcpStream) + Send + 'static) -> Url {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let port = listener.local_addr().expect("test server address").port();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept test connection");
        read_request(&stream);
        respond(stream);
    });
    Url::parse(&format!("http://127.0.0.1:{port}/")).expect("test server url")
}

fn hold_open(mut stream: TcpStream) {
    let _ = stream.read(&mut [0; 1]);
}

#[test]
fn request_gives_up_when_the_server_stops_answering() {
    let url = serve_once(hold_open);
    let client = HttpClient::new().with_read_timeout(Duration::from_millis(200));
    let started = Instant::now();
    let result = client.request(&HttpRequest::get(url));
    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn request_gives_up_when_the_body_stalls() {
    let url = serve_once(|mut stream| {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabc");
        hold_open(stream);
    });
    let client = HttpClient::new().with_read_timeout(Duration::from_millis(200));
    let started = Instant::now();
    let result = client.request(&HttpRequest::get(url));
    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(10));
}

fn stream_after_signal(release: Receiver<()>) -> impl FnOnce(TcpStream) + Send + 'static {
    move |mut stream| {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n");
        let _ = stream.flush();
        if release.recv().is_ok() {
            let _ = stream.write_all(b"data: late\n\n");
        }
    }
}

#[test]
fn open_stream_outlives_the_read_timeout() {
    let (release, wait) = channel();
    let url = serve_once(stream_after_signal(wait));
    let client = HttpClient::new().with_read_timeout(Duration::from_millis(100));
    let (response, mut body) = client
        .open(&HttpRequest::get(url))
        .expect("open event stream");
    assert_eq!(response.status, 200);
    std::thread::sleep(Duration::from_millis(400));
    release.send(()).expect("release test server");
    let mut text = String::new();
    body.read_to_string(&mut text).expect("read late event");
    assert_eq!(text, "data: late\n\n");
}

fn cookie_reply(cookie: &'static str) -> impl FnOnce(TcpStream) + Send + 'static {
    move |mut stream| {
        let reply = format!(
            "HTTP/1.1 200 OK\r\nSet-Cookie: {cookie}\r\n{}",
            "Content-Length: 0\r\nConnection: close\r\n\r\n"
        );
        let _ = stream.write_all(reply.as_bytes());
    }
}

#[test]
fn cookie_domains_must_match_the_host() {
    let cases = [
        ("a=1; Domain=127.0.0.1", Some("a=1")),
        ("b=2; Domain=0.1", None),
        ("c=3; Domain=example.com", None),
        ("d=4; Domain=com", None),
    ];
    for (cookie, expected) in cases {
        let url = serve_once(cookie_reply(cookie));
        let client = HttpClient::new();
        client
            .request(&HttpRequest::get(url.clone()))
            .expect("load cookie response");
        assert_eq!(client.cookie_header(&url).as_deref(), expected, "{cookie}");
    }
}

#[test]
fn informational_responses_are_skipped() {
    let url = serve_once(|mut stream| {
        let _ = stream.write_all(
            concat!(
                "HTTP/1.1 100 Continue\r\n\r\n",
                "HTTP/1.1 103 Early Hints\r\nLink: </a.css>\r\n\r\n",
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok",
            )
            .as_bytes(),
        );
    });
    let response = HttpClient::new()
        .request(&HttpRequest::get(url))
        .expect("load response after 1xx");
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"ok");
}

#[test]
fn oversized_response_headers_are_refused() {
    let url = serve_once(|mut stream| {
        let filler = "x".repeat(70 * 1024);
        let reply = format!("HTTP/1.1 200 OK\r\nX-Filler: {filler}\r\nContent-Length: 0\r\n\r\n");
        let _ = stream.write_all(reply.as_bytes());
    });
    let result = HttpClient::new().request(&HttpRequest::get(url));
    assert!(result.is_err());
}

#[test]
fn methods_that_are_not_tokens_are_refused() {
    let url = Url::parse("http://127.0.0.1:9/").expect("test url");
    let mut request = HttpRequest::get(url);
    request.method = "GET / HTTP/1.1\r\nX: y".to_string();
    assert!(HttpClient::new().request(&request).is_err());
}
