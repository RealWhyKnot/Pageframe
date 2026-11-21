mod handle;
mod input;
mod load;
mod session;
#[cfg(any(feature = "gpu", feature = "software"))]
mod window;

use std::sync::Arc;

use pageframe_traits::net::NetProvider;
use pageframe_traits::shell::ColorScheme;
use url::Url;

pub use cursor_icon::CursorIcon;
pub use handle::HostHandle;
pub use input::HostInputEvent;
pub use pageframe_script::HttpClient;
pub use serde_json::Value;
pub use session::HostSession;
#[cfg(any(feature = "gpu", feature = "software"))]
pub use window::PageframeHost;

pub type WakeCallback = Arc<dyn Fn() + Send + Sync>;

pub enum HostSource {
    Url(Url),
    Html { html: String, base_url: Option<Url> },
}

pub struct HostConfig {
    pub source: HostSource,
    pub width: f64,
    pub height: f64,
    pub scale_factor: f64,
    pub color_scheme: ColorScheme,
    pub http: Option<HttpClient>,
    pub net_provider: Option<Arc<dyn NetProvider>>,
    pub wake: Option<WakeCallback>,
}

impl HostConfig {
    pub fn url(url: Url) -> Self {
        Self::new(HostSource::Url(url))
    }

    pub fn html(html: &str, base_url: Option<Url>) -> Self {
        Self::new(HostSource::Html {
            html: html.to_owned(),
            base_url,
        })
    }

    fn new(source: HostSource) -> Self {
        Self {
            source,
            width: 800.0,
            height: 600.0,
            scale_factor: 1.0,
            color_scheme: ColorScheme::Light,
            http: None,
            net_provider: None,
            wake: None,
        }
    }

    #[must_use]
    pub fn size(mut self, width: f64, height: f64) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    #[must_use]
    pub fn scale_factor(mut self, scale_factor: f64) -> Self {
        self.scale_factor = scale_factor;
        self
    }

    #[must_use]
    pub fn color_scheme(mut self, color_scheme: ColorScheme) -> Self {
        self.color_scheme = color_scheme;
        self
    }

    #[must_use]
    pub fn http_client(mut self, http: HttpClient) -> Self {
        self.http = Some(http);
        self
    }

    #[must_use]
    pub fn net_provider(mut self, provider: Arc<dyn NetProvider>) -> Self {
        self.net_provider = Some(provider);
        self
    }

    #[must_use]
    pub fn wake(mut self, wake: impl Fn() + Send + Sync + 'static) -> Self {
        self.wake = Some(Arc::new(wake));
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct IpcRequest {
    pub id: u64,
    pub channel: String,
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IpcReply {
    Ok(Value),
    Err(String),
    Later,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HostEvent {
    Navigated(Url),
    NavigationBlocked(Url),
    TitleChanged(String),
    CursorChanged(Option<CursorIcon>),
    ImeEnabled(bool),
    ImeCursorArea {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    ClipboardText(String),
    CloseRequested,
    LoadFailed {
        url: Url,
        reason: String,
    },
}

pub fn same_origin(base: &Url, target: &Url) -> bool {
    target.as_str() == "about:blank" || base.origin() == target.origin()
}
