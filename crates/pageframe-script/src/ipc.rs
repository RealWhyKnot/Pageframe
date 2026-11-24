use std::sync::atomic::{AtomicU64, Ordering};

use boa_engine::{Context, JsResult, JsString, JsValue};

use crate::dom::dom_ctx;
use crate::runtime::ScriptRuntime;
use crate::runtime::events::report_js_error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IpcMessage {
    Invoke {
        id: u64,
        channel: String,
        payload: String,
    },
    Post(String),
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn arg_string(args: &[JsValue], index: usize, context: &mut Context) -> JsResult<String> {
    let value = args.get(index).cloned().unwrap_or_default();
    Ok(value.to_string(context)?.to_std_string_lossy())
}

pub(crate) fn send(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let kind = arg_string(args, 0, context)?;
    let channel = arg_string(args, 1, context)?;
    let payload = arg_string(args, 2, context)?;
    let (message, reply) = if kind == "invoke" {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let message = IpcMessage::Invoke {
            id,
            channel,
            payload,
        };
        (message, JsValue::from(id))
    } else {
        (IpcMessage::Post(payload), JsValue::undefined())
    };
    ctx.state.borrow_mut().ipc_outbound.push(message);
    Ok(reply)
}

pub(crate) fn call_hook(runtime: &mut ScriptRuntime, name: &str, args: &[JsValue]) {
    let context = &mut runtime.context;
    let result = context
        .global_object()
        .get(JsString::from(name), context)
        .and_then(|hook| match hook.as_object() {
            Some(hook) if hook.is_callable() => hook.call(&JsValue::undefined(), args, context),
            _ => Ok(JsValue::undefined()),
        });
    if let Err(error) = result {
        report_js_error(&runtime.ctx, &mut runtime.context, name, &error);
    }
    runtime.run_jobs(name);
}

pub(crate) fn settle(runtime: &mut ScriptRuntime, id: u64, result: Result<&str, &str>) {
    let (ok, value) = match result {
        Ok(json) => (true, json),
        Err(message) => (false, message),
    };
    let args = [
        JsValue::from(id),
        JsValue::from(ok),
        JsString::from(value).into(),
    ];
    call_hook(runtime, "__pageframe_ipc_settle", &args);
}

pub(crate) fn emit(runtime: &mut ScriptRuntime, channel: &str, json: &str) {
    let args = [JsString::from(channel).into(), JsString::from(json).into()];
    call_hook(runtime, "__pageframe_ipc_emit", &args);
}

pub(crate) fn dispatch_document_event(runtime: &mut ScriptRuntime, name: &str) {
    let args = [JsString::from(name).into()];
    call_hook(runtime, "__pageframe_document_event", &args);
}

pub(crate) fn dispatch_window_event(runtime: &mut ScriptRuntime, name: &str) {
    let args = [JsString::from(name).into()];
    call_hook(runtime, "__pageframe_window_event", &args);
}
