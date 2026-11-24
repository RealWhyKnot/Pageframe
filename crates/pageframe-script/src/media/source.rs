use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;

use url::Url;

use super::shared::{Failure, MEDIA_ERR_NETWORK, MEDIA_ERR_SRC_NOT_SUPPORTED, Shared};
use crate::http::{HttpClient, HttpRequest, TrackedSocket};

const SKIP_LIMIT: u64 = 512 * 1024;
const PREFIX_LEN: u64 = 4096;

pub(crate) trait ByteSource: Read + Send {
    fn len(&self) -> u64;
    fn position(&self) -> u64;
    fn set_position(&mut self, position: u64);
    fn content_type(&self) -> Option<&str>;
}

pub(crate) fn open(url: &Url, shared: &Shared) -> Result<Box<dyn ByteSource>, Failure> {
    match url.scheme() {
        "http" => Ok(Box::new(HttpSource::open(url, shared.socket())?)),
        "file" => Ok(Box::new(FileSource::open(url)?)),
        scheme => Err(Failure::new(
            MEDIA_ERR_SRC_NOT_SUPPORTED,
            format!("media URLs with the {scheme}: scheme are not supported"),
        )),
    }
}

struct FileSource {
    file: File,
    len: u64,
    position: u64,
    seeked: bool,
}

impl FileSource {
    fn open(url: &Url) -> Result<Self, Failure> {
        let missing = || Failure::new(MEDIA_ERR_SRC_NOT_SUPPORTED, format!("cannot open {url}"));
        let path = url.to_file_path().map_err(|()| missing())?;
        let file = File::open(&path).map_err(|error| {
            Failure::new(
                MEDIA_ERR_SRC_NOT_SUPPORTED,
                format!("open {}: {error}", path.display()),
            )
        })?;
        let len = file
            .metadata()
            .map_err(|error| {
                Failure::new(
                    MEDIA_ERR_SRC_NOT_SUPPORTED,
                    format!("read {}: {error}", path.display()),
                )
            })?
            .len();
        Ok(Self {
            file,
            len,
            position: 0,
            seeked: false,
        })
    }
}

impl Read for FileSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.seeked {
            self.file.seek(SeekFrom::Start(self.position))?;
            self.seeked = false;
        }
        let read = self.file.read(buf)?;
        self.position += u64::try_from(read).unwrap_or(0);
        Ok(read)
    }
}

impl ByteSource for FileSource {
    fn len(&self) -> u64 {
        self.len
    }

    fn position(&self) -> u64 {
        self.position
    }

    fn set_position(&mut self, position: u64) {
        if position != self.position {
            self.position = position;
            self.seeked = true;
        }
    }

    fn content_type(&self) -> Option<&str> {
        None
    }
}

struct Body {
    at: u64,
    reader: Box<dyn Read + Send>,
}

struct HttpSource {
    client: HttpClient,
    url: Url,
    socket: Arc<TrackedSocket>,
    len: u64,
    position: u64,
    ranges: bool,
    content_type: Option<String>,
    body: Option<Body>,
    prefix: Vec<u8>,
}

fn read_prefix(reader: &mut dyn Read, len: u64) -> Vec<u8> {
    let want = usize::try_from(len.min(PREFIX_LEN)).unwrap_or(0);
    let mut prefix = vec![0; want];
    let mut filled = 0;
    while filled < want {
        match reader.read(&mut prefix[filled..]) {
            Ok(0) | Err(_) => break,
            Ok(read) => filled += read,
        }
    }
    prefix.truncate(filled);
    prefix
}

fn total_length(content_range: &str) -> Option<u64> {
    content_range.rsplit('/').next()?.trim().parse().ok()
}

impl HttpSource {
    fn open(url: &Url, socket: Arc<TrackedSocket>) -> Result<Self, Failure> {
        let client = HttpClient::new();
        let mut request = HttpRequest::get(url.clone());
        request
            .headers
            .push(("Range".to_string(), "bytes=0-".to_string()));
        let (response, reader) = client.open_tracked(&request, &socket).map_err(|error| {
            Failure::new(MEDIA_ERR_SRC_NOT_SUPPORTED, format!("load {url}: {error}"))
        })?;
        let (len, ranges) = match response.status {
            206 => {
                let len = response.header("content-range").and_then(total_length);
                (len, true)
            }
            200 => {
                let len = response
                    .header("content-length")
                    .and_then(|value| value.trim().parse::<u64>().ok());
                let ranges = response
                    .header("accept-ranges")
                    .is_some_and(|value| value.to_ascii_lowercase().contains("bytes"));
                (len, ranges)
            }
            status => {
                return Err(Failure::new(
                    MEDIA_ERR_SRC_NOT_SUPPORTED,
                    format!("load {url}: HTTP {status} {}", response.status_text),
                ));
            }
        };
        let Some(len) = len else {
            return Err(Failure::new(
                MEDIA_ERR_SRC_NOT_SUPPORTED,
                format!("load {url}: the server did not report the media length"),
            ));
        };
        let content_type = response.header("content-type").map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        });
        let mut reader = reader;
        let prefix = read_prefix(&mut *reader, len);
        Ok(Self {
            client,
            url: response.url,
            socket,
            len,
            position: 0,
            ranges,
            content_type,
            body: Some(Body {
                at: u64::try_from(prefix.len()).unwrap_or(0),
                reader,
            }),
            prefix,
        })
    }

    fn reconnect(&mut self, from: u64) -> io::Result<()> {
        self.body = None;
        let start = if self.ranges { from } else { 0 };
        let mut request = HttpRequest::get(self.url.clone());
        if self.ranges {
            request
                .headers
                .push(("Range".to_string(), format!("bytes={start}-")));
        }
        let (response, reader) = self
            .client
            .open_tracked(&request, &self.socket)
            .map_err(|error| io::Error::other(format!("load {}: {error}", self.url)))?;
        let accepted = match response.status {
            206 => true,
            200 => start == 0,
            _ => false,
        };
        if !accepted {
            return Err(io::Error::other(format!(
                "load {}: HTTP {} for bytes {start}-",
                self.url, response.status
            )));
        }
        self.body = Some(Body { at: start, reader });
        Ok(())
    }

    fn align(&mut self) -> io::Result<()> {
        let position = self.position;
        let reusable = self
            .body
            .as_ref()
            .is_some_and(|body| body.at <= position && position - body.at <= SKIP_LIMIT);
        if !reusable {
            self.reconnect(position)?;
        }
        let Some(body) = self.body.as_mut() else {
            return Err(io::Error::other("the media connection closed"));
        };
        let mut scratch = [0_u8; 16 * 1024];
        while body.at < position {
            let want = usize::try_from((position - body.at).min(16 * 1024)).unwrap_or(16 * 1024);
            let read = body.reader.read(&mut scratch[..want])?;
            if read == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            body.at += u64::try_from(read).unwrap_or(0);
        }
        Ok(())
    }
}

impl Read for HttpSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.len || buf.is_empty() {
            return Ok(0);
        }
        if let Some(cached) = usize::try_from(self.position)
            .ok()
            .and_then(|start| self.prefix.get(start..))
            .filter(|cached| !cached.is_empty())
        {
            let count = cached.len().min(buf.len());
            buf[..count].copy_from_slice(&cached[..count]);
            self.position += u64::try_from(count).unwrap_or(0);
            return Ok(count);
        }
        for attempt in 0..2 {
            if self.socket.is_closed() {
                return Err(io::ErrorKind::ConnectionAborted.into());
            }
            if attempt > 0
                || self
                    .body
                    .as_ref()
                    .is_none_or(|body| body.at != self.position)
            {
                if attempt > 0 {
                    self.body = None;
                }
                self.align()?;
            }
            let Some(body) = self.body.as_mut() else {
                continue;
            };
            let read = body.reader.read(buf)?;
            if read > 0 {
                let advanced = u64::try_from(read).unwrap_or(0);
                body.at += advanced;
                self.position += advanced;
                return Ok(read);
            }
        }
        Err(io::ErrorKind::UnexpectedEof.into())
    }
}

impl ByteSource for HttpSource {
    fn len(&self) -> u64 {
        self.len
    }

    fn position(&self) -> u64 {
        self.position
    }

    fn set_position(&mut self, position: u64) {
        self.position = position.min(self.len);
    }

    fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }
}

pub(crate) fn network_failure(error: &io::Error) -> Failure {
    Failure::new(MEDIA_ERR_NETWORK, format!("media read failed: {error}"))
}

pub(crate) fn sniff_extension(source: &mut dyn ByteSource, url: &Url) -> &'static str {
    let mut head = [0_u8; 16];
    let mut filled = 0;
    while filled < head.len() {
        match source.read(&mut head[filled..]) {
            Ok(0) | Err(_) => break,
            Ok(read) => filled += read,
        }
    }
    source.set_position(0);
    let head = &head[..filled];
    if let Some(extension) = sniff(head) {
        return extension;
    }
    if let Some(extension) = source.content_type().and_then(mime_extension) {
        return extension;
    }
    let path = url.path().to_ascii_lowercase();
    let extension = path.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
    match extension {
        "wav" => ".wav",
        "mp3" => ".mp3",
        "aac" => ".aac",
        "m4a" => ".m4a",
        "webm" => ".webm",
        "mkv" => ".mkv",
        "flac" => ".flac",
        "mov" => ".mov",
        _ => ".mp4",
    }
}

fn sniff(head: &[u8]) -> Option<&'static str> {
    if head.len() >= 8 && &head[4..8] == b"ftyp" {
        return Some(".mp4");
    }
    if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WAVE" {
        return Some(".wav");
    }
    if head.starts_with(b"ID3") {
        return Some(".mp3");
    }
    if head.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        return Some(".webm");
    }
    if head.starts_with(b"fLaC") {
        return Some(".flac");
    }
    if head.starts_with(b"OggS") {
        return Some(".ogg");
    }
    match head {
        [0xff, second, ..] if second & 0xf6 == 0xf0 => Some(".aac"),
        [0xff, second, ..] if second & 0xe0 == 0xe0 => Some(".mp3"),
        _ => None,
    }
}

fn mime_extension(mime: &str) -> Option<&'static str> {
    Some(match mime {
        "video/mp4" | "audio/mp4" | "video/x-m4v" | "audio/x-m4a" => ".mp4",
        "video/quicktime" => ".mov",
        "audio/wav" | "audio/x-wav" | "audio/wave" | "audio/vnd.wave" => ".wav",
        "audio/mpeg" | "audio/mp3" => ".mp3",
        "audio/aac" | "audio/aacp" => ".aac",
        "video/webm" | "audio/webm" => ".webm",
        "video/x-matroska" => ".mkv",
        "audio/flac" | "audio/x-flac" => ".flac",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::thread;

    use url::Url;

    use super::{ByteSource, HttpSource};
    use crate::http::TrackedSocket;

    const TOTAL: u64 = 4 << 30;

    fn pattern(offset: u64) -> u8 {
        u8::try_from(offset % 251).expect("a remainder below 251 fits in a byte")
    }

    fn matches(bytes: &[u8], start: u64) -> bool {
        bytes
            .iter()
            .zip(start..)
            .all(|(byte, offset)| *byte == pattern(offset))
    }

    fn serve(stream: TcpStream, starts: &Mutex<Vec<u64>>) {
        let mut reader = BufReader::new(stream.try_clone().expect("clone the test stream"));
        let mut start = 0;
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            let header = line.trim().to_ascii_lowercase();
            if header.is_empty() {
                break;
            }
            if let Some(value) = header.strip_prefix("range:") {
                start = value
                    .trim()
                    .trim_start_matches("bytes=")
                    .trim_end_matches('-')
                    .parse()
                    .expect("the client sends an open-ended byte range");
            }
        }
        starts.lock().expect("range log").push(start);
        let mut stream = stream;
        let head = format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Type: video/mp4\r\nContent-Range: bytes {start}-{}/{TOTAL}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            TOTAL - 1,
            TOTAL - start
        );
        if stream.write_all(head.as_bytes()).is_err() {
            return;
        }
        let mut offset = start;
        let mut chunk = vec![0_u8; 64 * 1024];
        while offset < TOTAL {
            for (byte, at) in chunk.iter_mut().zip(offset..) {
                *byte = pattern(at);
            }
            let size = usize::try_from((TOTAL - offset).min(64 * 1024)).expect("chunk size");
            if stream.write_all(&chunk[..size]).is_err() {
                return;
            }
            offset += u64::try_from(size).expect("chunk size");
        }
    }

    #[test]
    fn http_source_streams_ranges_without_downloading_the_file() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a test port");
        let address = listener.local_addr().expect("test address");
        let starts = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&starts);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let log = Arc::clone(&log);
                thread::spawn(move || serve(stream, &log));
            }
        });
        let url = Url::parse(&format!("http://{address}/huge.mp4")).expect("test url");
        let socket = Arc::new(TrackedSocket::default());
        let mut source = HttpSource::open(&url, Arc::clone(&socket)).expect("open the source");
        assert_eq!(source.len(), TOTAL);
        assert_eq!(source.content_type(), Some("video/mp4"));

        let mut head = [0_u8; 8192];
        source.read_exact(&mut head).expect("read the head");
        assert!(matches(&head, 0));

        source.set_position(0);
        let mut again = [0_u8; 16];
        source.read_exact(&mut again).expect("reread the head");
        assert!(matches(&again, 0));

        let deep = TOTAL - 1_000_000;
        source.set_position(deep);
        let mut tail = [0_u8; 4096];
        source
            .read_exact(&mut tail)
            .expect("read deep into the file");
        assert!(matches(&tail, deep));
        assert_eq!(source.position(), deep + 4096);

        let near = deep + 4096 + 100_000;
        source.set_position(near);
        let mut skip = [0_u8; 512];
        source
            .read_exact(&mut skip)
            .expect("read after a short skip");
        assert!(matches(&skip, near));

        assert_eq!(*starts.lock().expect("range log"), vec![0, deep]);
        socket.close();
    }
}
