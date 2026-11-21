use boa_engine::object::builtins::{JsArray, JsArrayBuffer};
use boa_engine::{Context, JsNativeError, JsResult, JsString, JsValue};
use url::Url;

use super::{ScriptRuntime, events::report_js_error};
use crate::dom::dom_ctx;
use crate::fetch::{FetchError, document_may_load};
use crate::http::{HttpRequest, HttpResponse};
use crate::net::NetEvent;

fn string_arg(args: &[JsValue], index: usize, context: &mut Context) -> JsResult<String> {
    match args.get(index) {
        Some(value) if !value.is_undefined() && !value.is_null() => {
            Ok(value.to_string(context)?.to_std_string_lossy())
        }
        _ => Ok(String::new()),
    }
}

fn header_pairs(value: Option<&JsValue>, context: &mut Context) -> JsResult<Vec<(String, String)>> {
    let Some(object) = value.and_then(JsValue::as_object) else {
        return Ok(Vec::new());
    };
    let list = JsArray::from_object(object.clone())?;
    let mut pairs = Vec::new();
    for index in 0..list.length(context)? {
        let Some(pair) = list.get(index, context)?.as_object() else {
            continue;
        };
        let pair = JsArray::from_object(pair.clone())?;
        let name = pair
            .get(0, context)?
            .to_string(context)?
            .to_std_string_lossy();
        let value = pair
            .get(1, context)?
            .to_string(context)?
            .to_std_string_lossy();
        pairs.push((name, value));
    }
    Ok(pairs)
}

fn resolve(input: &str, context: &mut Context) -> JsResult<(Url, Option<Url>)> {
    let ctx = dom_ctx(context)?;
    let base_url = ctx.state.borrow().base_url.clone();
    let url = match &base_url {
        Some(base) => base.join(input),
        None => Url::parse(input),
    }
    .map_err(|_| JsNativeError::typ().with_message(format!("Failed to parse URL from {input}")))?;
    if !document_may_load(base_url.as_ref(), &url) {
        return Err(JsNativeError::typ()
            .with_message(format!(
                "Failed to fetch {url}: {}",
                FetchError::Forbidden(url.clone())
            ))
            .into());
    }
    Ok((url, base_url))
}

fn bytes_value(bytes: &[u8], context: &mut Context) -> JsValue {
    let Ok(buffer) = JsArrayBuffer::new(bytes.len(), context) else {
        return JsValue::null();
    };
    if let Some(mut data) = buffer.data_mut() {
        data.copy_from_slice(bytes);
    }
    buffer.into()
}

fn body_arg(args: &[JsValue], index: usize, context: &mut Context) -> JsResult<Vec<u8>> {
    if let Some(object) = args.get(index).and_then(JsValue::as_object) {
        if let Ok(buffer) = JsArrayBuffer::from_object(object.clone()) {
            return Ok(buffer.to_vec().unwrap_or_default());
        }
    }
    Ok(string_arg(args, index, context)?.into_bytes())
}

fn response_value(response: &HttpResponse, context: &mut Context) -> JsValue {
    let headers: Vec<JsValue> = response
        .headers
        .iter()
        .map(|(name, value)| {
            JsArray::from_iter(
                [
                    JsString::from(name.as_str()).into(),
                    JsString::from(value.as_str()).into(),
                ],
                context,
            )
            .into()
        })
        .collect();
    let headers = JsArray::from_iter(headers, context);
    JsArray::from_iter(
        [
            JsValue::from(response.status),
            JsString::from(response.status_text.as_str()).into(),
            JsString::from(response.url.as_str()).into(),
            JsValue::from(response.redirected),
            headers.into(),
            bytes_value(&response.body, context),
        ],
        context,
    )
    .into()
}

pub(super) fn fetch_start(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let method = string_arg(args, 0, context)?.to_ascii_uppercase();
    if method.is_empty()
        || !method
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
    {
        return Err(JsNativeError::typ()
            .with_message(format!("'{method}' is not a valid HTTP method."))
            .into());
    }
    let input = string_arg(args, 1, context)?;
    let headers = header_pairs(args.get(2), context)?;
    let body = body_arg(args, 3, context)?;
    let (url, _) = resolve(&input, context)?;
    let ctx = dom_ctx(context)?;
    let fetcher = ctx
        .state
        .borrow()
        .fetcher
        .clone()
        .ok_or_else(|| JsNativeError::typ().with_message("fetch is unavailable"))?;
    let mut request = HttpRequest::get(url.clone());
    request.method = method;
    request.headers = headers;
    request.body = body;
    let client = fetcher.borrow().http_client();
    if let Some(client) = client.filter(|_| url.scheme() == "http") {
        let shell = Some(ctx.doc.borrow().shell_provider.clone());
        let id = ctx
            .state
            .borrow_mut()
            .net
            .start_request(client, request, shell);
        return Ok(JsValue::from(id as f64));
    }
    let result = fetcher.borrow().request(&request);
    match result {
        Ok(response) => Ok(response_value(&response, context)),
        Err(FetchError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut missing = HttpResponse::ok(url, Vec::new());
            missing.status = 404;
            missing.status_text = "Not Found".to_string();
            Ok(response_value(&missing, context))
        }
        Err(error) => Err(JsNativeError::typ()
            .with_message(format!("Failed to fetch {url}: {error}"))
            .into()),
    }
}

pub(super) fn event_source_open(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let input = string_arg(args, 0, context)?;
    let last_event_id = string_arg(args, 1, context)?;
    let (url, _) = resolve(&input, context)?;
    let ctx = dom_ctx(context)?;
    let client = ctx
        .state
        .borrow()
        .fetcher
        .as_ref()
        .and_then(|fetcher| fetcher.borrow().http_client())
        .filter(|_| url.scheme() == "http")
        .ok_or_else(|| {
            JsNativeError::typ().with_message(format!("EventSource can't load {url}"))
        })?;
    let mut request = HttpRequest::get(url.clone());
    request
        .headers
        .push(("Accept".to_string(), "text/event-stream".to_string()));
    request
        .headers
        .push(("Cache-Control".to_string(), "no-cache".to_string()));
    if !last_event_id.is_empty() {
        request
            .headers
            .push(("Last-Event-ID".to_string(), last_event_id));
    }
    let shell = Some(ctx.doc.borrow().shell_provider.clone());
    let id = ctx
        .state
        .borrow_mut()
        .net
        .open_stream(client, request, shell);
    Ok(JsValue::from(id as f64))
}

pub(super) fn event_source_close(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let id = match args.first() {
        Some(value) => value.to_number(context)?,
        None => return Ok(JsValue::undefined()),
    };
    if id.is_finite() && id > 0.0 {
        dom_ctx(context)?
            .state
            .borrow_mut()
            .net
            .close_stream(id as u64);
    }
    Ok(JsValue::undefined())
}

impl ScriptRuntime {
    pub fn pending_network_requests(&self) -> usize {
        self.ctx.state.borrow().net.in_flight()
    }

    pub fn deliver_network(&mut self) -> bool {
        let events = self.ctx.state.borrow_mut().net.drain();
        if events.is_empty() {
            return false;
        }
        let deliver = match self
            .context
            .global_object()
            .get(JsString::from("__pageframe_net_deliver"), &mut self.context)
        {
            Ok(value) => value,
            Err(error) => {
                report_js_error(&self.ctx, &mut self.context, "network delivery", &error);
                return false;
            }
        };
        let Some(deliver) = deliver.as_callable() else {
            return false;
        };
        for event in events {
            let args = self.event_args(event);
            if let Err(error) = deliver.call(&JsValue::undefined(), &args, &mut self.context) {
                report_js_error(&self.ctx, &mut self.context, "network callback", &error);
            }
            self.run_jobs("network microtasks");
        }
        true
    }

    fn event_args(&mut self, event: NetEvent) -> Vec<JsValue> {
        let text = |value: &str| -> JsValue { JsString::from(value).into() };
        match event {
            NetEvent::Response { id, result } => match result {
                Ok(response) => vec![
                    JsValue::from(id as f64),
                    text("response"),
                    response_value(&response, &mut self.context),
                ],
                Err(error) => vec![JsValue::from(id as f64), text("error"), text(&error)],
            },
            NetEvent::StreamOpen { id } => vec![JsValue::from(id as f64), text("open")],
            NetEvent::StreamMessage {
                id,
                kind,
                data,
                last_event_id,
            } => vec![
                JsValue::from(id as f64),
                text("message"),
                text(&kind),
                text(&data),
                text(&last_event_id),
            ],
            NetEvent::StreamRetry { id, millis } => vec![
                JsValue::from(id as f64),
                text("retry"),
                JsValue::from(millis as f64),
            ],
            NetEvent::StreamEnd {
                id,
                error,
                reconnect,
            } => vec![
                JsValue::from(id as f64),
                text("end"),
                text(&error),
                JsValue::from(reconnect),
            ],
        }
    }
}
