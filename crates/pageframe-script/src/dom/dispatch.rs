use boa_engine::object::JsObject;
use boa_engine::{Context, JsNativeError, JsResult, JsString, JsValue, js_string};
use pageframe_dom::NodeId;

use super::event::EventRef;
use super::{define_value, dom_ctx, dom_exception, node_wrapper, this_node_id};
use crate::runtime::events::report_js_error;
use crate::state::{DomCtx, Listener};

const AT_TARGET: i32 = 2;
const BUBBLING_PHASE: i32 = 3;

pub(crate) fn event_flag(event: &JsObject, read: fn(&EventRef) -> bool) -> bool {
    event
        .downcast_ref::<EventRef>()
        .map(|event| read(&event))
        .unwrap_or(false)
}

fn take_listeners(listeners: Option<&mut Vec<Listener>>) -> Vec<Listener> {
    match listeners {
        Some(listeners) => {
            let cloned = listeners.clone();
            listeners.retain(|listener| !listener.once);
            cloned
        }
        None => Vec::new(),
    }
}

fn handler_property(object: &JsObject, name: &str, context: &mut Context) -> Option<JsObject> {
    object
        .get(JsString::from(format!("on{name}")), context)
        .ok()
        .and_then(|handler| handler.as_object())
        .filter(|handler| handler.is_callable())
}

pub(crate) fn dispatch_on_window(
    ctx: &DomCtx,
    context: &mut Context,
    name: &str,
    event: &JsObject,
) -> bool {
    let listeners = take_listeners(ctx.state.borrow_mut().window_listeners.get_mut(name));
    let global = context.global_object();
    let handler = handler_property(&global, name, context);
    if listeners.is_empty() && handler.is_none() {
        return false;
    }
    let global: JsValue = global.into();
    define_value(event, "currentTarget", global.clone(), context);
    let callbacks = listeners
        .into_iter()
        .map(|listener| listener.callback)
        .chain(handler);
    for callback in callbacks {
        if let Err(error) = callback.call(&global, &[event.clone().into()], context) {
            report_js_error(ctx, context, "event listener", &error);
        }
        if event_flag(event, |event| event.stopped_immediate.get()) {
            break;
        }
    }
    true
}

pub(crate) fn dispatch_to_chain(
    ctx: &DomCtx,
    context: &mut Context,
    chain: &[NodeId],
    name: &str,
    bubbles: bool,
    event: &JsObject,
) -> bool {
    let mut any_called = false;
    'chain: for (index, &node_id) in chain.iter().enumerate() {
        let mut callbacks: Vec<JsObject> = {
            let mut state = ctx.state.borrow_mut();
            take_listeners(
                state
                    .node_listeners
                    .get_mut(&node_id)
                    .and_then(|map| map.get_mut(name)),
            )
            .into_iter()
            .map(|listener| listener.callback)
            .collect()
        };
        let wrapper = ctx.state.borrow().node_wrappers.get(&node_id).cloned();
        if let Some(handler) = wrapper.and_then(|wrapper| handler_property(&wrapper, name, context))
        {
            callbacks.push(handler);
        }
        if callbacks.is_empty() {
            if !bubbles {
                break;
            }
            continue;
        }
        let current_target: JsValue = node_wrapper(ctx, node_id, context).into();
        define_value(event, "currentTarget", current_target.clone(), context);
        let phase = if index == 0 {
            AT_TARGET
        } else {
            BUBBLING_PHASE
        };
        define_value(event, "eventPhase", JsValue::from(phase), context);
        for callback in callbacks {
            any_called = true;
            if let Err(error) = callback.call(&current_target, &[event.clone().into()], context) {
                report_js_error(ctx, context, "event listener", &error);
            }
            if event_flag(event, |event| event.stopped_immediate.get()) {
                break 'chain;
            }
        }
        if !bubbles || event_flag(event, |event| event.stopped.get()) {
            break;
        }
    }
    any_called
}

pub(crate) fn event_path(ctx: &DomCtx, node_id: NodeId) -> (Vec<NodeId>, bool) {
    let doc = ctx.doc.borrow();
    let root = doc.root_node().id;
    let mut chain = Vec::new();
    let mut current = Some(node_id);
    while let Some(id) = current {
        chain.push(id);
        current = doc.get_node(id).and_then(|node| node.parent);
    }
    let connected = chain.last() == Some(&root);
    (chain, connected)
}

fn event_arg(args: &[JsValue]) -> JsResult<JsObject> {
    args.first()
        .and_then(JsValue::as_object)
        .filter(|object| object.downcast_ref::<EventRef>().is_some())
        .ok_or_else(|| {
            JsNativeError::typ()
                .with_message(
                    "Failed to execute 'dispatchEvent' on 'EventTarget': parameter 1 is not of type 'Event'.",
                )
                .into()
        })
}

fn begin(event: &JsObject, target: &JsValue, context: &mut Context) -> JsResult<(String, bool)> {
    if event_flag(event, |event| event.dispatching.get()) {
        return Err(dom_exception(
            context,
            "InvalidStateError",
            "Failed to execute 'dispatchEvent' on 'EventTarget': The event is already being dispatched.",
        ));
    }
    let name = event
        .get(js_string!("type"), context)?
        .to_string(context)?
        .to_std_string_lossy();
    let bubbles = event.get(js_string!("bubbles"), context)?.to_boolean();
    if let Some(state) = event.downcast_ref::<EventRef>() {
        state.stopped.set(false);
        state.stopped_immediate.set(false);
        state.dispatching.set(true);
    }
    define_value(event, "target", target.clone(), context);
    define_value(event, "srcElement", target.clone(), context);
    Ok((name, bubbles))
}

fn finish(event: &JsObject, context: &mut Context) -> JsValue {
    define_value(event, "currentTarget", JsValue::null(), context);
    define_value(event, "eventPhase", JsValue::from(0), context);
    if let Some(state) = event.downcast_ref::<EventRef>() {
        state.stopped.set(false);
        state.stopped_immediate.set(false);
        state.dispatching.set(false);
    }
    JsValue::from(!event_flag(event, |event| event.prevented.get()))
}

pub(crate) fn node_dispatch_event(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let event = event_arg(args)?;
    let (name, bubbles) = begin(&event, this, context)?;
    let (chain, connected) = event_path(&ctx, node_id);
    dispatch_to_chain(&ctx, context, &chain, &name, bubbles, &event);
    if connected && bubbles && !event_flag(&event, |event| event.stopped.get()) {
        define_value(&event, "eventPhase", JsValue::from(BUBBLING_PHASE), context);
        dispatch_on_window(&ctx, context, &name, &event);
    }
    Ok(finish(&event, context))
}

pub(crate) fn window_dispatch_event(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let event = event_arg(args)?;
    let global: JsValue = context.global_object().into();
    let (name, _) = begin(&event, &global, context)?;
    define_value(&event, "eventPhase", JsValue::from(AT_TARGET), context);
    dispatch_on_window(&ctx, context, &name, &event);
    Ok(finish(&event, context))
}
