use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use url::Url;

use crate::cookies::CookieJar;
use crate::fetch::{DefaultScriptFetcher, FetchError, ScriptFetcher};

const MAX_REDIRECTS: usize = 20;
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_CHUNK_LINE_BYTES: usize = 4096;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const READ_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
enum BodyWait<'a> {
    Timed,
    Unbounded,
    Tracked(&'a TrackedSocket),
}

#[derive(Default)]
pub(crate) struct TrackedSocket(Mutex<SocketSlot>);

#[derive(Default)]
struct SocketSlot {
    closed: bool,
    stream: Option<TcpStream>,
}

impl TrackedSocket {
    fn track(&self, stream: &TcpStream) -> Result<(), FetchError> {
        let mut slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.closed {
            return Err(FetchError::Io(std::io::ErrorKind::ConnectionAborted.into()));
        }
        slot.stream = stream.try_clone().ok();
        Ok(())
    }

    pub(crate) fn close(&self) {
        let stream = {
            let mut slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            slot.closed = true;
            slot.stream.take()
        };
        if let Some(stream) = stream {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).closed
    }
}

#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: String,
    pub url: Url,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn get(url: Url) -> Self {
        Self {
            method: "GET".to_string(),
            url,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn header(&self, name: &str) -> Option<&str> {
        find_header(&self.headers, name)
    }
}

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub url: Url,
    pub redirected: bool,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn ok(url: Url, body: Vec<u8>) -> Self {
        Self {
            status: 200,
            status_text: "OK".to_string(),
            url,
            redirected: false,
            headers: Vec::new(),
            body,
        }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        find_header(&self.headers, name)
    }
}

fn find_header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[derive(Clone)]
pub struct HttpClient {
    jar: Arc<Mutex<CookieJar>>,
    read_timeout: Duration,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self {
            jar: Arc::default(),
            read_timeout: READ_TIMEOUT,
        }
    }
}

impl HttpClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout.max(Duration::from_millis(1));
        self
    }

    pub fn cookie_header(&self, url: &Url) -> Option<String> {
        self.jar.lock().ok()?.header_for(url)
    }

    pub fn request(&self, request: &HttpRequest) -> Result<HttpResponse, FetchError> {
        let (mut response, mut body) = self.follow(request, BodyWait::Timed)?;
        let skip_body =
            request.method.eq_ignore_ascii_case("HEAD") || matches!(response.status, 204 | 304);
        if !skip_body {
            body.read_to_end(&mut response.body)
                .map_err(FetchError::Io)?;
        }
        Ok(response)
    }

    pub fn open(
        &self,
        request: &HttpRequest,
    ) -> Result<(HttpResponse, Box<dyn Read + Send>), FetchError> {
        self.follow(request, BodyWait::Unbounded)
    }

    pub(crate) fn open_tracked(
        &self,
        request: &HttpRequest,
        socket: &TrackedSocket,
    ) -> Result<(HttpResponse, Box<dyn Read + Send>), FetchError> {
        self.follow(request, BodyWait::Tracked(socket))
    }

    fn follow(
        &self,
        request: &HttpRequest,
        wait: BodyWait<'_>,
    ) -> Result<(HttpResponse, Box<dyn Read + Send>), FetchError> {
        let mut current = request.clone();
        let mut redirected = false;
        for _ in 0..=MAX_REDIRECTS {
            let (mut response, body) = self.send_once(&current, wait)?;
            if let Ok(mut jar) = self.jar.lock() {
                jar.store(&current.url, &response.headers);
            }
            let location = response
                .header("location")
                .filter(|_| matches!(response.status, 301 | 302 | 303 | 307 | 308))
                .and_then(|location| current.url.join(location).ok());
            let Some(mut next) = location else {
                response.redirected = redirected;
                return Ok((response, body));
            };
            if next.fragment().is_none() {
                next.set_fragment(current.url.fragment());
            }
            let to_get = match response.status {
                301 | 302 => current.method.eq_ignore_ascii_case("POST"),
                303 => {
                    !current.method.eq_ignore_ascii_case("GET")
                        && !current.method.eq_ignore_ascii_case("HEAD")
                }
                _ => false,
            };
            if to_get {
                current.method = "GET".to_string();
                current.body.clear();
                current
                    .headers
                    .retain(|(key, _)| !key.eq_ignore_ascii_case("content-type"));
            }
            if next.origin() != current.url.origin() {
                current
                    .headers
                    .retain(|(key, _)| !key.eq_ignore_ascii_case("authorization"));
            }
            current.url = next;
            redirected = true;
        }
        Err(FetchError::InvalidData(format!(
            "too many redirects loading {}",
            request.url
        )))
    }

    fn send_once(
        &self,
        request: &HttpRequest,
        wait: BodyWait<'_>,
    ) -> Result<(HttpResponse, Box<dyn Read + Send>), FetchError> {
        let url = &request.url;
        if url.scheme() != "http" {
            return Err(FetchError::UnsupportedScheme(url.scheme().to_string()));
        }
        if !is_token(&request.method) {
            return Err(FetchError::InvalidData(format!(
                "{:?} is not a valid HTTP method",
                request.method
            )));
        }
        let host = url
            .host_str()
            .ok_or_else(|| FetchError::InvalidData(format!("{url} has no host")))?;
        let port = url.port_or_known_default().unwrap_or(80);
        let mut stream = connect(host, port)?;
        if let BodyWait::Tracked(socket) = &wait {
            socket.track(&stream)?;
        }
        stream
            .set_read_timeout(Some(self.read_timeout))
            .and_then(|()| stream.set_write_timeout(Some(self.read_timeout)))
            .map_err(FetchError::Io)?;
        let head = request_head(request, self.cookie_header(url));
        stream
            .write_all(head.as_bytes())
            .and_then(|()| stream.write_all(&request.body))
            .and_then(|()| stream.flush())
            .map_err(FetchError::Io)?;
        let mut reader = BufReader::new(stream);
        let (status, status_text, headers) = read_head(&mut reader)?;
        if matches!(wait, BodyWait::Unbounded | BodyWait::Tracked(_)) {
            reader
                .get_ref()
                .set_read_timeout(None)
                .map_err(FetchError::Io)?;
        }
        let response = HttpResponse {
            status,
            status_text,
            url: url.clone(),
            redirected: false,
            headers,
            body: Vec::new(),
        };
        let body: Box<dyn Read + Send> = if response
            .header("transfer-encoding")
            .is_some_and(|value| value.to_ascii_lowercase().contains("chunked"))
        {
            Box::new(ChunkedReader::new(reader))
        } else if let Some(length) = response
            .header("content-length")
            .and_then(|value| value.trim().parse::<u64>().ok())
        {
            Box::new(reader.take(length))
        } else {
            Box::new(reader)
        };
        Ok((response, body))
    }
}

fn connect(host: &str, port: u16) -> Result<TcpStream, FetchError> {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let addresses = (host, port).to_socket_addrs().map_err(FetchError::Io)?;
    let mut last_error = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(FetchError::Io(last_error.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("no address for {host}"),
        )
    })))
}

fn is_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

fn push_header(head: &mut String, name: &str, value: &str) {
    head.push_str(name);
    head.push_str(": ");
    head.push_str(value);
    head.push_str("\r\n");
}

fn request_head(request: &HttpRequest, cookies: Option<String>) -> String {
    let url = &request.url;
    let mut target = url.path().to_string();
    if let Some(query) = url.query() {
        target.push('?');
        target.push_str(query);
    }
    let host = match url.port() {
        Some(port) => format!("{}:{port}", url.host_str().unwrap_or_default()),
        None => url.host_str().unwrap_or_default().to_string(),
    };
    let mut head = format!(
        "{} {target} HTTP/1.1\r\n",
        request.method.to_ascii_uppercase()
    );
    push_header(&mut head, "Host", &host);
    push_header(&mut head, "Connection", "close");
    for (name, value) in &request.headers {
        let lower = name.to_ascii_lowercase();
        let reserved = matches!(
            lower.as_str(),
            "host" | "connection" | "content-length" | "transfer-encoding" | "cookie"
        );
        if reserved || !is_token(name) || value.contains(['\r', '\n', '\0']) {
            continue;
        }
        push_header(&mut head, name, value);
    }
    if request.header("accept").is_none() {
        push_header(&mut head, "Accept", "*/*");
    }
    if let Some(cookies) = cookies {
        push_header(&mut head, "cookie", &cookies);
    }
    if !request.body.is_empty() || !matches!(request.method.as_str(), "GET" | "HEAD") {
        push_header(&mut head, "Content-Length", &request.body.len().to_string());
    }
    head.push_str("\r\n");
    head
}

type ResponseHead = (u16, String, Vec<(String, String)>);

fn read_line(
    reader: &mut impl BufRead,
    line: &mut String,
    total: &mut usize,
) -> Result<(), FetchError> {
    line.clear();
    let limit = MAX_HEADER_BYTES.saturating_sub(*total) + 1;
    let read = (&mut *reader)
        .take(u64::try_from(limit).unwrap_or(u64::MAX))
        .read_line(line)
        .map_err(FetchError::Io)?;
    *total += read;
    if read == 0 {
        return Err(FetchError::InvalidData(
            "connection closed while reading response headers".to_string(),
        ));
    }
    if *total > MAX_HEADER_BYTES {
        return Err(FetchError::InvalidData(format!(
            "response headers are larger than {MAX_HEADER_BYTES} bytes"
        )));
    }
    Ok(())
}

fn read_head(reader: &mut impl BufRead) -> Result<ResponseHead, FetchError> {
    let mut total = 0;
    let mut line = String::new();
    loop {
        loop {
            read_line(reader, &mut line, &mut total)?;
            if !line.trim().is_empty() {
                break;
            }
        }
        let status_line = line.trim_end().to_string();
        let mut status_parts = status_line.splitn(3, ' ');
        let version = status_parts.next().unwrap_or_default();
        let status = status_parts
            .next()
            .and_then(|code| code.parse::<u16>().ok())
            .filter(|_| version.starts_with("HTTP/"))
            .ok_or_else(|| {
                FetchError::InvalidData(format!("malformed status line {status_line:?}"))
            })?;
        let status_text = status_parts.next().unwrap_or_default().to_string();
        let mut headers = Vec::new();
        loop {
            read_line(reader, &mut line, &mut total)?;
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some((name, value)) = trimmed.split_once(':') {
                headers.push((name.trim().to_string(), value.trim().to_string()));
            }
        }
        if !(100..200).contains(&status) {
            return Ok((status, status_text, headers));
        }
    }
}

struct ChunkedReader<R> {
    inner: R,
    remaining: u64,
    done: bool,
}

impl<R: BufRead> ChunkedReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            remaining: 0,
            done: false,
        }
    }

    fn next_chunk(&mut self) -> std::io::Result<()> {
        let mut line = String::new();
        while line.trim().is_empty() {
            line.clear();
            let limit = u64::try_from(MAX_CHUNK_LINE_BYTES).unwrap_or(u64::MAX);
            if (&mut self.inner).take(limit).read_line(&mut line)? == 0 {
                self.done = true;
                return Ok(());
            }
        }
        let size = line.trim().split(';').next().unwrap_or_default().trim();
        self.remaining = u64::from_str_radix(size, 16).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("bad chunk size {size:?}"),
            )
        })?;
        if self.remaining == 0 {
            self.done = true;
        }
        Ok(())
    }
}

impl<R: BufRead> Read for ChunkedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while !self.done && self.remaining == 0 {
            self.next_chunk()?;
        }
        if self.done || buf.is_empty() {
            return Ok(0);
        }
        let limit = buf
            .len()
            .min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let read = self.inner.read(&mut buf[..limit])?;
        if read == 0 {
            self.done = true;
            return Ok(0);
        }
        self.remaining -= read as u64;
        Ok(read)
    }
}

impl ScriptFetcher for HttpClient {
    fn fetch(&self, url: &Url) -> Result<String, FetchError> {
        if url.scheme() != "http" {
            return DefaultScriptFetcher.fetch(url);
        }
        let response = HttpClient::request(self, &HttpRequest::get(url.clone()))?;
        if !(200..300).contains(&response.status) {
            return Err(FetchError::Status(response.status, url.clone()));
        }
        String::from_utf8(response.body).map_err(|error| FetchError::InvalidData(error.to_string()))
    }

    fn request(&self, request: &HttpRequest) -> Result<HttpResponse, FetchError> {
        if request.url.scheme() != "http" {
            return DefaultScriptFetcher.request(request);
        }
        HttpClient::request(self, request)
    }

    fn http_client(&self) -> Option<HttpClient> {
        Some(self.clone())
    }
}
