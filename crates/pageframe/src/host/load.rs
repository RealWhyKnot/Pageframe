use std::sync::Arc;

use pageframe_dom::DocumentConfig;
use pageframe_script::{HttpClient, HttpRequest, ScriptDocument};
use pageframe_traits::net::NetProvider;
use pageframe_traits::shell::Viewport;
use url::Url;

use super::HostSource;
use super::handle::{NavigationBridge, Shared, ShellBridge};
use crate::error::Error;

pub(super) struct Loader {
    pub(super) http: HttpClient,
    pub(super) net: Arc<dyn NetProvider>,
    pub(super) shared: Arc<Shared>,
    pub(super) timer_thread: bool,
}

pub(super) const URL_CHANNEL: &str = "__pageframe_url_changed";

fn url_hook() -> String {
    format!(
        r#"{{
    const set = __pageframe_set_document_url;
    const send = __pageframe_ipc_send;
    globalThis.__pageframe_set_document_url = (href) => {{
        set(href);
        send("invoke", "{URL_CHANNEL}", String(href));
    }};
}}"#
    )
}

pub(super) struct Loaded {
    pub(super) doc: ScriptDocument,
    pub(super) url: Option<Url>,
}

fn load_error(url: &Url, reason: &dyn std::fmt::Display) -> Error {
    Error::Load {
        url: url.to_string(),
        reason: reason.to_string(),
    }
}

impl Loader {
    fn fetch(&self, url: &Url) -> Result<(String, Url), Error> {
        match url.scheme() {
            "http" | "https" => {
                let response = self
                    .http
                    .request(&HttpRequest::get(url.clone()))
                    .map_err(|error| load_error(url, &error))?;
                let body = String::from_utf8_lossy(&response.body).into_owned();
                Ok((body, response.url))
            }
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|()| load_error(url, &"not a local file path"))?;
                let body =
                    std::fs::read_to_string(&path).map_err(|error| load_error(url, &error))?;
                Ok((body, url.clone()))
            }
            "about" if url.path() == "blank" => Ok((String::new(), url.clone())),
            scheme => Err(load_error(url, &format!("unsupported scheme `{scheme}`"))),
        }
    }

    pub(super) fn load(&self, source: &HostSource, viewport: Viewport) -> Result<Loaded, Error> {
        let (html, url) = match source {
            HostSource::Html { html, base_url } => (html.clone(), base_url.clone()),
            HostSource::Url(url) => {
                let (html, final_url) = self.fetch(url)?;
                (html, Some(final_url))
            }
        };
        Ok(self.build(&html, url, viewport))
    }

    pub(super) fn read(&self, url: &Url) -> Result<(String, Url), Error> {
        self.fetch(url)
    }

    pub(super) fn build(&self, html: &str, url: Option<Url>, viewport: Viewport) -> Loaded {
        let config = DocumentConfig {
            viewport: Some(viewport),
            base_url: url.as_ref().map(Url::to_string),
            net_provider: Some(Arc::clone(&self.net)),
            navigation_provider: Some(Arc::new(NavigationBridge(Arc::clone(&self.shared)))),
            shell_provider: Some(Arc::new(ShellBridge(Arc::clone(&self.shared)))),
            ..DocumentConfig::default()
        };
        let mut doc = ScriptDocument::from_html(html, config).with_fetcher(self.http.clone());
        if !self.timer_thread {
            doc = doc.without_timer_thread();
        }
        doc.eval(&url_hook());
        doc.execute_scripts();
        Loaded { doc, url }
    }
}
