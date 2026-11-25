use std::sync::{Arc, Mutex, PoisonError};

use pageframe_traits::net::{Body, Bytes, NetHandler, NetProvider, Request};

use crate::fetch::{FetchError, read_local_file};
use crate::http::{HttpClient, HttpRequest};

fn local_bytes(url: &url::Url) -> Result<Vec<u8>, FetchError> {
    match url.scheme() {
        "file" => {
            let path = url
                .to_file_path()
                .map_err(|()| FetchError::InvalidData(format!("invalid file URL: {url}")))?;
            read_local_file(url, &path)
        }
        "data" => {
            let data_url = data_url::DataUrl::process(url.as_str())
                .map_err(|err| FetchError::InvalidData(format!("{err:?}")))?;
            let (bytes, _) = data_url
                .decode_to_vec()
                .map_err(|err| FetchError::InvalidData(format!("{err:?}")))?;
            Ok(bytes)
        }
        "https" => Err(FetchError::HttpsUnsupported(url.clone())),
        scheme => Err(FetchError::UnsupportedScheme(scheme.to_string())),
    }
}

fn to_http_request(request: Request) -> HttpRequest {
    let mut converted = HttpRequest::get(request.url);
    converted.method = request.method.as_str().to_string();
    converted.headers = request
        .headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect();
    if let Some(content_type) = request.content_type {
        converted
            .headers
            .push(("content-type".to_string(), content_type));
    }
    if let Body::Bytes(bytes) = request.body {
        converted.body = bytes.to_vec();
    }
    converted
}

type HandlerSlot = Arc<Mutex<Option<Box<dyn NetHandler>>>>;

fn take_handler(slot: &HandlerSlot) -> Option<Box<dyn NetHandler>> {
    slot.lock().unwrap_or_else(PoisonError::into_inner).take()
}

impl NetProvider for HttpClient {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url.clone();
        if url.scheme() != "http" {
            match local_bytes(&url) {
                Ok(bytes) => handler.bytes(url.to_string(), Bytes::from(bytes)),
                Err(error) => handler.error(url.to_string(), error.to_string()),
            }
            return;
        }
        let client = self.clone();
        let request = to_http_request(request);
        let slot: HandlerSlot = Arc::new(Mutex::new(Some(handler)));
        let worker_slot = Arc::clone(&slot);
        let spawned = std::thread::Builder::new()
            .name("pageframe-http".to_string())
            .spawn(move || {
                let Some(handler) = take_handler(&worker_slot) else {
                    return;
                };
                match client.request(&request) {
                    Ok(response) if (200..300).contains(&response.status) => {
                        handler.bytes(response.url.to_string(), Bytes::from(response.body));
                    }
                    Ok(response) => handler.error(
                        request.url.to_string(),
                        FetchError::Status(response.status, response.url).to_string(),
                    ),
                    Err(error) => handler.error(request.url.to_string(), error.to_string()),
                }
            });
        if let Err(error) = spawned
            && let Some(handler) = take_handler(&slot)
        {
            handler.error(
                url.to_string(),
                format!("could not start a thread to load {url}: {error}"),
            );
        }
    }
}
