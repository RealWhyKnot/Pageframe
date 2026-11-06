use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyrender_vello::VelloWindowRenderer;
use blitz_dom::{DocumentConfig, LocalName, QualName, UserAgentStylesheet, ns};
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_shell::{BlitzApplication, BlitzShellProxy, WindowConfig, create_default_event_loop};
use blitz_traits::events::DomEventKind;
use blitz_traits::shell::{ColorScheme, Viewport};
use url::Url;
use winit::dpi::LogicalSize;
use winit::window::WindowAttributes;

use crate::error::Error;
use crate::event::Event;
use crate::net::{EMBEDDED_BASE, LocalNet};
use crate::page::{Callback, Handler, Page};
use crate::ui::Ui;

type Start<S> = Box<dyn FnOnce(&mut S, &mut Ui<'_>)>;

struct PendingHandler<S> {
    selector: String,
    kind: DomEventKind,
    callback: Callback<S>,
}

pub struct App<S> {
    state: S,
    html: String,
    base_url: String,
    stylesheets: Vec<String>,
    assets: HashMap<String, &'static [u8]>,
    title: String,
    width: u32,
    height: u32,
    handlers: Vec<PendingHandler<S>>,
    starts: Vec<Start<S>>,
}

impl<S: 'static> App<S> {
    pub fn new(state: S, html: &str) -> Self {
        Self {
            state,
            html: html.to_owned(),
            base_url: EMBEDDED_BASE.to_owned(),
            stylesheets: Vec::new(),
            assets: HashMap::new(),
            title: String::from("Pageframe"),
            width: 800,
            height: 600,
            handlers: Vec::new(),
            starts: Vec::new(),
        }
    }

    pub fn from_file(state: S, path: &Path) -> Result<Self, Error> {
        let read_error = |source| Error::Read {
            path: path.to_owned(),
            source,
        };
        let html = std::fs::read_to_string(path).map_err(read_error)?;
        let absolute = std::path::absolute(path).map_err(read_error)?;
        let base = Url::from_file_path(&absolute).map_err(|()| Error::FileUrl {
            path: absolute.clone(),
        })?;
        let mut app = Self::new(state, &html);
        app.base_url = base.into();
        Ok(app)
    }

    #[must_use]
    pub fn stylesheet(mut self, css: &str) -> Self {
        self.stylesheets.push(css.to_owned());
        self
    }

    #[must_use]
    pub fn asset(mut self, path: &str, bytes: &'static [u8]) -> Self {
        self.assets
            .insert(path.trim_start_matches('/').to_owned(), bytes);
        self
    }

    #[must_use]
    pub fn title(mut self, title: &str) -> Self {
        title.clone_into(&mut self.title);
        self
    }

    #[must_use]
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    #[must_use]
    pub fn on(
        mut self,
        selector: &str,
        kind: DomEventKind,
        callback: impl FnMut(&mut S, &mut Ui<'_>, &mut Event<'_>) + 'static,
    ) -> Self {
        self.handlers.push(PendingHandler {
            selector: selector.to_owned(),
            kind,
            callback: Box::new(callback),
        });
        self
    }

    #[must_use]
    pub fn on_start(mut self, start: impl FnOnce(&mut S, &mut Ui<'_>) + 'static) -> Self {
        self.starts.push(Box::new(start));
        self
    }

    pub fn page(self, width: u32, height: u32) -> Result<Page<S>, Error> {
        let viewport = Viewport::new(width, height, 1.0, ColorScheme::Light);
        self.build(Some(viewport))
    }

    pub fn run(self) -> Result<(), Error> {
        let attributes = WindowAttributes::default()
            .with_title(self.title.clone())
            .with_surface_size(LogicalSize::new(self.width, self.height));
        let page = self.build(None)?;
        let event_loop = create_default_event_loop();
        let (proxy, receiver) = BlitzShellProxy::new(event_loop.create_proxy());
        let mut application = BlitzApplication::new(proxy, receiver);
        application.add_window(WindowConfig::with_attributes(
            Box::new(page),
            VelloWindowRenderer::new(),
            attributes,
        ));
        event_loop.run_app(application)?;
        Ok(())
    }

    fn build(self, viewport: Option<Viewport>) -> Result<Page<S>, Error> {
        let config = DocumentConfig {
            viewport,
            base_url: Some(self.base_url),
            ua_stylesheets: vec![UserAgentStylesheet::default_sheet()],
            net_provider: Some(Arc::new(LocalNet::new(self.assets))),
            html_parser_provider: Some(Arc::new(HtmlProvider)),
            ..Default::default()
        };
        let mut doc = HtmlDocument::from_html(&self.html, config);
        add_stylesheets(&mut doc, &self.stylesheets);

        let mut handlers = Vec::with_capacity(self.handlers.len());
        for pending in self.handlers {
            let selector = doc
                .try_parse_selector_list(&pending.selector)
                .map_err(|error| {
                    tracing::error!(selector = pending.selector, ?error, "invalid selector");
                    Error::Selector {
                        selector: pending.selector.clone(),
                    }
                })?;
            handlers.push(Handler {
                kind: pending.kind,
                selector,
                callback: pending.callback,
            });
        }

        let mut page = Page::new(doc, self.state, handlers);
        for start in self.starts {
            page.update(start);
        }
        Ok(page)
    }
}

fn add_stylesheets(doc: &mut HtmlDocument, stylesheets: &[String]) {
    if stylesheets.is_empty() {
        return;
    }
    let Ok(Some(head)) = doc.query_selector("head") else {
        tracing::error!("document has no <head> for the app stylesheets");
        return;
    };
    let mut mutator = doc.mutate();
    for css in stylesheets {
        let style = mutator.create_element(
            QualName::new(None, ns!(html), LocalName::from("style")),
            Vec::new(),
        );
        let text = mutator.create_text_node(css);
        mutator.append_children(style, &[text]);
        mutator.append_children(head, &[style]);
    }
}
