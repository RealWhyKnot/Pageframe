use boa_engine::object::builtins::JsArray;
use boa_engine::{Context, JsResult, JsValue, js_string};
use pageframe_traits::navigation::NavigationOptions;
use pageframe_traits::shell::ColorScheme;
use url::Url;
use web_time::{Duration, Instant};

use super::ScriptRuntime;
use crate::dom::dispatch::dispatch_to_chain;
use crate::dom::event::create_event;
use crate::dom::image::{ImageState, image_state};
use crate::dom::{dom_ctx, js_str, node_wrapper, to_rust_string};
use crate::state::DomCtx;

const IMAGE_POLL_INTERVAL: Duration = Duration::from_millis(16);

impl ScriptRuntime {
    pub fn image_poll_deadline(&self) -> Option<Instant> {
        let state = self.ctx.state.borrow();
        if state.watched_images.is_empty() || state.clock.is_virtual() {
            return None;
        }
        Some(state.clock.now() + IMAGE_POLL_INTERVAL)
    }

    pub fn media_poll_deadline(&self) -> Option<Instant> {
        let state = self.ctx.state.borrow();
        if state.clock.is_virtual() {
            return None;
        }
        state.media.deadline(state.clock.now())
    }

    pub fn deliver_media(&mut self) -> (bool, bool) {
        let ctx = self.ctx.clone();
        let (ran, redraw) = crate::media::deliver(&ctx, &mut self.context);
        if ran {
            self.run_jobs("media events");
        }
        (ran, redraw)
    }

    pub fn deliver_image_events(&mut self) -> bool {
        if self.ctx.state.borrow().watched_images.is_empty() {
            return false;
        }
        self.ctx.doc.borrow_mut().handle_messages();
        let watched = std::mem::take(&mut self.ctx.state.borrow_mut().watched_images);
        let mut settled = Vec::new();
        let mut pending = Vec::new();
        {
            let doc = self.ctx.doc.borrow();
            for node_id in watched {
                match image_state(&doc, node_id) {
                    ImageState::Pending => pending.push(node_id),
                    ImageState::Loaded(..) => settled.push((node_id, "load")),
                    ImageState::Broken => settled.push((node_id, "error")),
                }
            }
        }
        {
            let mut state = self.ctx.state.borrow_mut();
            let added = std::mem::replace(&mut state.watched_images, pending);
            for node_id in added {
                state.watch_image(node_id);
            }
        }
        if settled.is_empty() {
            return false;
        }
        let ctx = self.ctx.clone();
        for (node_id, name) in settled {
            let target: JsValue = node_wrapper(&ctx, node_id, &mut self.context).into();
            let event = create_event(&ctx, name, false, false, &target, &mut self.context);
            if let Some(state) = event.downcast_ref::<crate::dom::event::EventRef>() {
                state.cancelable.set(false);
            }
            dispatch_to_chain(&ctx, &mut self.context, &[node_id], name, false, &event);
            crate::dom::define_value(&event, "currentTarget", JsValue::null(), &mut self.context);
            crate::dom::define_value(&event, "eventPhase", JsValue::from(0), &mut self.context);
        }
        self.run_jobs("image events");
        true
    }

    pub fn deliver_media_changes(&mut self) -> bool {
        let snapshot = media_snapshot(&self.ctx);
        let previous = self.ctx.state.borrow_mut().media_snapshot.replace(snapshot);
        if previous.is_none_or(|previous| previous == snapshot) {
            return false;
        }
        let global = self.context.global_object();
        let Ok(callback) = global.get(js_string!("__pageframe_media_changed"), &mut self.context)
        else {
            return false;
        };
        let Some(callback) = callback.as_callable() else {
            return false;
        };
        if let Err(error) = callback.call(&JsValue::undefined(), &[], &mut self.context) {
            super::events::report_js_error(&self.ctx, &mut self.context, "media query", &error);
        }
        self.run_jobs("media query listeners");
        true
    }
}

fn media_snapshot(ctx: &DomCtx) -> (u32, u32, u32, bool) {
    let doc = ctx.doc.borrow();
    let viewport = doc.viewport();
    let scale = viewport.scale();
    (
        (viewport.window_size.0 as f32 / scale) as u32,
        (viewport.window_size.1 as f32 / scale) as u32,
        (scale * 1000.0) as u32,
        viewport.color_scheme == ColorScheme::Dark,
    )
}

pub(super) fn media_env(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let (width, height, scale, dark) = {
        let doc = ctx.doc.borrow();
        let viewport = doc.viewport();
        let scale = viewport.scale();
        (
            viewport.window_size.0 as f64 / scale as f64,
            viewport.window_size.1 as f64 / scale as f64,
            scale as f64,
            viewport.color_scheme == ColorScheme::Dark,
        )
    };
    let values = [
        JsValue::from(width),
        JsValue::from(height),
        JsValue::from(scale),
        JsValue::from(dark),
    ];
    Ok(JsArray::from_iter(values, context).into())
}

pub(super) fn performance_now(
    _: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    Ok(JsValue::from(ctx.state.borrow().high_res_now()))
}

pub(super) fn document_url(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let url = ctx.state.borrow().base_url.as_ref().map(Url::to_string);
    Ok(js_str(url.as_deref().unwrap_or("about:blank")))
}

pub(super) fn set_document_url(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let value = to_rust_string(args.first().unwrap_or(&JsValue::undefined()), context)?;
    let Ok(url) = Url::parse(&value) else {
        return Ok(JsValue::from(false));
    };
    let mut state = ctx.state.borrow_mut();
    let blank = Url::parse("about:blank").expect("about:blank parses");
    if !may_rewrite_url(state.base_url.as_ref().unwrap_or(&blank), &url) {
        return Ok(JsValue::from(false));
    }
    state.base_url = Some(url.clone());
    if state.url_changes.len() >= MAX_URL_CHANGES {
        state.url_changes.remove(0);
    }
    state.url_changes.push(url);
    Ok(JsValue::from(true))
}

const MAX_URL_CHANGES: usize = 256;

fn may_rewrite_url(current: &Url, target: &Url) -> bool {
    if current.scheme() != target.scheme()
        || current.username() != target.username()
        || current.password() != target.password()
        || current.host() != target.host()
        || current.port() != target.port()
    {
        return false;
    }
    match target.scheme() {
        "http" | "https" => true,
        "file" => current.path() == target.path(),
        _ => {
            let mut left = current.clone();
            let mut right = target.clone();
            left.set_fragment(None);
            right.set_fragment(None);
            left == right
        }
    }
}

pub(super) fn navigate(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let value = to_rust_string(args.first().unwrap_or(&JsValue::undefined()), context)?;
    let Ok(url) = Url::parse(&value) else {
        return Ok(JsValue::from(false));
    };
    let doc = ctx.doc.borrow();
    let options = NavigationOptions::new(url, None, doc.id());
    doc.navigation_provider.navigate_to(options);
    Ok(JsValue::from(true))
}

pub(super) fn random_bytes(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let count = match args.first() {
        Some(value) => value.to_length(context)?.min(65_536) as usize,
        None => 0,
    };
    let ctx = dom_ctx(context)?;
    let bytes = ctx.state.borrow_mut().random_bytes(count);
    let values: Vec<JsValue> = bytes.into_iter().map(JsValue::from).collect();
    Ok(JsArray::from_iter(values, context).into())
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::may_rewrite_url;

    fn allowed(current: &str, target: &str) -> bool {
        may_rewrite_url(&Url::parse(current).unwrap(), &Url::parse(target).unwrap())
    }

    #[test]
    fn document_urls_stay_within_their_origin() {
        assert!(allowed("http://app.test/a", "http://app.test/b?x#y"));
        assert!(allowed(
            "file:///C:/site/index.html",
            "file:///C:/site/index.html?q#top"
        ));
        assert!(allowed("about:blank", "about:blank#top"));
        assert!(!allowed("http://app.test/", "file:///C:/site/index.html"));
        assert!(!allowed("http://app.test/", "https://app.test/"));
        assert!(!allowed("http://app.test/", "http://app.test:8080/"));
        assert!(!allowed("http://app.test/", "http://user@app.test/"));
        assert!(!allowed("http://app.test/", "http://other.test/"));
        assert!(!allowed(
            "file:///C:/site/index.html",
            "file:///C:/site/other.html"
        ));
        assert!(!allowed("about:blank", "file:///C:/site/index.html"));
        assert!(!allowed("about:blank", "about:srcdoc"));
    }
}
