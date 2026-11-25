use std::sync::{Arc, Mutex, PoisonError};

use pageframe_dom::{Document, DocumentConfig};
use pageframe_script::ScriptDocument;
use pageframe_traits::net::{NetHandler, NetProvider, Request};
use url::Url;

#[derive(Default)]
struct Recorder {
    requested: Mutex<Vec<String>>,
    refused: Arc<Mutex<Vec<String>>>,
}

struct Refusals(Arc<Mutex<Vec<String>>>);

impl NetHandler for Refusals {
    fn bytes(self: Box<Self>, _resolved_url: String, _bytes: pageframe_traits::net::Bytes) {}

    fn error(self: Box<Self>, url: String, _error: String) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(url);
    }
}

impl NetProvider for Recorder {
    fn fetch(&self, _doc_id: usize, request: Request, _handler: Box<dyn NetHandler>) {
        self.requested
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.url.to_string());
    }
}

fn local_url(name: &str) -> Url {
    Url::from_file_path(std::env::temp_dir().join(name)).expect("temp dir is absolute")
}

fn requested_by(base: &str, html: &str) -> Vec<String> {
    let recorder = Arc::new(Recorder::default());
    let provider: Arc<dyn NetProvider> = Arc::<Recorder>::clone(&recorder);
    let config = DocumentConfig {
        base_url: Some(base.to_string()),
        net_provider: Some(provider),
        ..Default::default()
    };
    let mut doc = ScriptDocument::from_html(html, config).without_timer_thread();
    doc.execute_scripts();
    doc.poll(None);
    drop(doc);
    recorder
        .requested
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

fn subresources() -> (Url, Url, Url, String) {
    let image = local_url("pageframe-secret.png");
    let sheet = local_url("pageframe-secret.css");
    let frame = local_url("pageframe-secret.html");
    let html = format!(
        r#"<html><head><link rel="stylesheet" href="{sheet}"></head>
        <body><img src="{image}"><iframe src="{frame}"></iframe></body></html>"#
    );
    (image, sheet, frame, html)
}

#[test]
fn a_web_page_cannot_load_local_files() {
    let (image, sheet, frame, html) = subresources();
    let requested = requested_by("http://127.0.0.1:9/page.html", &html);
    for url in [&image, &sheet, &frame] {
        assert!(
            !requested.contains(&url.to_string()),
            "{url} was requested: {requested:?}"
        );
    }
}

#[test]
fn a_local_page_still_loads_local_files() {
    let (image, sheet, frame, html) = subresources();
    let page = local_url("pageframe-page.html");
    let requested = requested_by(page.as_str(), &html);
    for url in [&image, &sheet, &frame] {
        assert!(
            requested.contains(&url.to_string()),
            "{url} was not requested: {requested:?}"
        );
    }
}

#[test]
fn an_app_page_still_loads_local_files() {
    let (image, sheet, frame, html) = subresources();
    let requested = requested_by("pageframe://app/index.html", &html);
    for url in [&image, &sheet, &frame] {
        assert!(
            requested.contains(&url.to_string()),
            "{url} was not requested: {requested:?}"
        );
    }
}

#[test]
fn pages_without_a_file_or_app_url_cannot_load_local_files() {
    let (image, sheet, frame, html) = subresources();
    for base in [
        "data:text/html,page",
        "about:blank",
        "blob:http://127.0.0.1:9/id",
        "custom://host/page.html",
    ] {
        let requested = requested_by(base, &html);
        for url in [&image, &sheet, &frame] {
            assert!(
                !requested.contains(&url.to_string()),
                "{url} was requested from {base}: {requested:?}"
            );
        }
    }
}

#[test]
fn a_page_without_a_base_url_cannot_load_local_files() {
    let (image, sheet, frame, html) = subresources();
    let recorder = Arc::new(Recorder::default());
    let provider: Arc<dyn NetProvider> = Arc::<Recorder>::clone(&recorder);
    let config = DocumentConfig {
        net_provider: Some(provider),
        ..Default::default()
    };
    let mut doc = ScriptDocument::from_html(&html, config).without_timer_thread();
    doc.execute_scripts();
    doc.poll(None);
    drop(doc);
    let requested = recorder
        .requested
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    for url in [&image, &sheet, &frame] {
        assert!(
            !requested.contains(&url.to_string()),
            "{url} was requested: {requested:?}"
        );
    }
}

#[test]
fn a_secure_web_page_cannot_load_local_files() {
    let (image, sheet, frame, html) = subresources();
    let requested = requested_by("https://example.com/page.html", &html);
    for url in [&image, &sheet, &frame] {
        assert!(
            !requested.contains(&url.to_string()),
            "{url} was requested: {requested:?}"
        );
    }
}

#[test]
fn a_web_page_still_loads_web_subresources() {
    let html = r#"<html><head><link rel="stylesheet" href="/site.css"></head>
        <body><img src="/logo.png"><iframe src="/frame.html"></iframe></body></html>"#;
    let requested = requested_by("http://127.0.0.1:9/page.html", html);
    for url in [
        "http://127.0.0.1:9/site.css",
        "http://127.0.0.1:9/logo.png",
        "http://127.0.0.1:9/frame.html",
    ] {
        assert!(
            requested.contains(&url.to_string()),
            "{url} was not requested: {requested:?}"
        );
    }
}

#[test]
fn a_refused_local_file_reports_an_error_to_its_handler() {
    let recorder = Arc::new(Recorder::default());
    let provider: Arc<dyn NetProvider> = Arc::<Recorder>::clone(&recorder);
    let config = DocumentConfig {
        base_url: Some("http://127.0.0.1:9/page.html".to_string()),
        net_provider: Some(provider),
        ..Default::default()
    };
    let doc = ScriptDocument::from_html("<p>hi</p>", config).without_timer_thread();
    let secret = local_url("pageframe-secret.bin");
    doc.inner().net_provider.fetch(
        0,
        Request::get(secret.clone()),
        Box::new(Refusals(Arc::clone(&recorder.refused))),
    );
    assert_eq!(
        *recorder
            .refused
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        vec![secret.to_string()]
    );
    assert!(
        recorder
            .requested
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    );
}
