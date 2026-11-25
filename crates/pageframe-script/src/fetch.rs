//! Synchronous fetching of external script sources (`<script src="...">`)

use std::fmt;
use std::io::Read;

use url::Url;

use crate::http::{HttpClient, HttpRequest, HttpResponse};

#[derive(Debug)]
pub enum FetchError {
    UnsupportedScheme(String),
    /// The requesting document is not allowed to load this URL
    /// (see [`document_may_load`])
    Forbidden(Url),
    Io(std::io::Error),
    InvalidData(String),
    Status(u16, Url),
    HttpsUnsupported(Url),
    TooLarge(Url, u64),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedScheme(scheme) => {
                write!(f, "unsupported URL scheme for script: {scheme}")
            }
            Self::Forbidden(url) => write!(f, "not allowed to load {url} from this document"),
            Self::Io(error) => write!(f, "IO error fetching script: {error}"),
            Self::InvalidData(msg) => write!(f, "invalid script data: {msg}"),
            Self::Status(status, url) => write!(f, "{url} answered with HTTP {status}"),
            Self::HttpsUnsupported(url) => write!(
                f,
                "cannot load {url}: Pageframe's HTTP client only speaks plain http, not https"
            ),
            Self::TooLarge(url, limit) => write!(f, "{url} is larger than {limit} bytes"),
        }
    }
}

impl std::error::Error for FetchError {}

/// Trait for synchronously fetching external script sources.
///
/// Scripts are fetched synchronously because the HTML spec requires classic
/// scripts to execute in document order, blocking parsing.
pub trait ScriptFetcher: 'static {
    fn fetch(&self, url: &Url) -> Result<String, FetchError>;

    fn request(&self, request: &HttpRequest) -> Result<HttpResponse, FetchError> {
        if !matches!(request.method.as_str(), "GET" | "HEAD") {
            return Err(FetchError::UnsupportedScheme(format!(
                "{} {}",
                request.method,
                request.url.scheme()
            )));
        }
        let text = self.fetch(&request.url)?;
        Ok(HttpResponse::ok(request.url.clone(), text.into_bytes()))
    }

    fn http_client(&self) -> Option<HttpClient> {
        None
    }
}

/// Whether a document at `document_url` may load `url` (via `<script src>`,
/// `import` or `fetch()`). `file:` URLs may only be loaded by `file:` documents,
/// so that remote pages cannot read local files.
pub fn document_may_load(document_url: Option<&Url>, url: &Url) -> bool {
    url.scheme() != "file"
        || document_url.is_some_and(|document_url| document_url.scheme() == "file")
}

pub(crate) const MAX_LOCAL_FILE_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn read_local_file(url: &Url, path: &std::path::Path) -> Result<Vec<u8>, FetchError> {
    let file = std::fs::File::open(path).map_err(FetchError::Io)?;
    let mut bytes = Vec::new();
    file.take(MAX_LOCAL_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(FetchError::Io)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_LOCAL_FILE_BYTES {
        return Err(FetchError::TooLarge(url.clone(), MAX_LOCAL_FILE_BYTES));
    }
    Ok(bytes)
}

/// The default [`ScriptFetcher`]: supports `file:` and `data:` URLs.
pub struct DefaultScriptFetcher;

impl ScriptFetcher for DefaultScriptFetcher {
    fn fetch(&self, url: &Url) -> Result<String, FetchError> {
        match url.scheme() {
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|_| FetchError::InvalidData(format!("invalid file URL: {url}")))?;
                let bytes = read_local_file(url, &path)?;
                String::from_utf8(bytes).map_err(|err| FetchError::InvalidData(format!("{err:?}")))
            }
            "data" => {
                let data_url = data_url::DataUrl::process(url.as_str())
                    .map_err(|err| FetchError::InvalidData(format!("{err:?}")))?;
                let (bytes, _) = data_url
                    .decode_to_vec()
                    .map_err(|err| FetchError::InvalidData(format!("{err:?}")))?;
                String::from_utf8(bytes).map_err(|err| FetchError::InvalidData(format!("{err:?}")))
            }
            scheme => Err(FetchError::UnsupportedScheme(scheme.to_string())),
        }
    }
}
