use std::collections::HashMap;

use data_url::DataUrl;
use pageframe_traits::net::{Bytes, NetHandler, NetProvider, Request};
use url::Url;

pub(crate) const EMBEDDED_BASE: &str = "pageframe://app/";

pub(crate) struct LocalNet {
    assets: HashMap<String, &'static [u8]>,
}

impl LocalNet {
    pub(crate) fn new(assets: HashMap<String, &'static [u8]>) -> Self {
        Self { assets }
    }

    fn load(&self, url: &Url) -> Result<Bytes, String> {
        match url.scheme() {
            "data" => {
                let data = DataUrl::process(url.as_str())
                    .map_err(|error| format!("parse data URL: {error:?}"))?;
                let (body, _) = data
                    .decode_to_vec()
                    .map_err(|error| format!("decode data URL: {error:?}"))?;
                Ok(Bytes::from(body))
            }
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|()| format!("{url} is not a local file path"))?;
                std::fs::read(&path)
                    .map(Bytes::from)
                    .map_err(|error| format!("read {}: {error}", path.display()))
            }
            "pageframe" => {
                let name = url.path().trim_start_matches('/');
                self.assets
                    .get(name)
                    .map(|bytes| Bytes::from_static(bytes))
                    .ok_or_else(|| format!("no embedded asset named {name}"))
            }
            other => Err(format!("{other} URLs are not loaded")),
        }
    }
}

impl NetProvider for LocalNet {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url.to_string();
        match self.load(&request.url) {
            Ok(bytes) => handler.bytes(url, bytes),
            Err(message) => {
                tracing::warn!(%url, %message, "resource not loaded");
                handler.error(url, message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use url::Url;

    use super::{EMBEDDED_BASE, LocalNet};

    fn net() -> LocalNet {
        LocalNet::new(HashMap::from([("img/logo.svg".to_owned(), &b"<svg/>"[..])]))
    }

    #[test]
    fn embedded_asset_resolves_against_base() {
        let url = Url::parse(EMBEDDED_BASE)
            .unwrap()
            .join("img/logo.svg")
            .unwrap();
        assert_eq!(net().load(&url).unwrap().as_ref(), b"<svg/>");
    }

    #[test]
    fn data_url_decodes() {
        let url = Url::parse("data:text/plain;base64,aGk=").unwrap();
        assert_eq!(net().load(&url).unwrap().as_ref(), b"hi");
    }

    #[test]
    fn missing_asset_and_remote_urls_fail() {
        let missing = Url::parse(EMBEDDED_BASE).unwrap().join("nope.css").unwrap();
        assert!(net().load(&missing).is_err());
        let remote = Url::parse("https://example.com/a.css").unwrap();
        assert!(net().load(&remote).is_err());
    }
}
