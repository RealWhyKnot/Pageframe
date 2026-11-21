use boa_engine::object::{FunctionObjectBuilder, JsObject};
use boa_engine::property::{PropertyDescriptor, PropertyKey};
use boa_engine::value::JsValue;
use boa_engine::{Context, JsResult, JsString, NativeFunction, js_string};

use super::{dom_ctx, this_node_id, to_rust_string};
use crate::state::{DomCtx, Listener, ListenerMap};

fn listener_options(args: &[JsValue], context: &mut Context) -> JsResult<(bool, bool)> {
    match args.get(2) {
        Some(value) if value.is_object() => {
            let options = value.as_object().expect("checked object");
            Ok((
                options.get(js_string!("capture"), context)?.to_boolean(),
                options.get(js_string!("once"), context)?.to_boolean(),
            ))
        }
        Some(options) => Ok((options.to_boolean(), false)),
        None => Ok((false, false)),
    }
}

pub(crate) fn add_event_listener(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let event_type = to_rust_string(args.first().unwrap_or(&JsValue::undefined()), context)?;
    let Some(callback) = args.get(1).and_then(JsValue::as_object) else {
        return Ok(JsValue::undefined());
    };
    if !callback.is_callable() {
        return Ok(JsValue::undefined());
    }
    let (capture, once) = listener_options(args, context)?;
    let mut state = ctx.state.borrow_mut();
    let listeners = state
        .node_listeners
        .entry(node_id)
        .or_default()
        .entry(event_type)
        .or_default();
    let duplicate = listeners.iter().any(|listener| {
        !listener.handler
            && JsObject::equals(&listener.callback, &callback)
            && listener.capture == capture
    });
    if !duplicate {
        listeners.push(Listener {
            callback,
            capture,
            once,
            handler: false,
        });
    }
    Ok(JsValue::undefined())
}

pub(crate) fn remove_event_listener(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let event_type = to_rust_string(args.first().unwrap_or(&JsValue::undefined()), context)?;
    let Some(callback) = args.get(1).and_then(JsValue::as_object) else {
        return Ok(JsValue::undefined());
    };
    let (capture, _) = listener_options(args, context)?;
    let mut state = ctx.state.borrow_mut();
    if let Some(listeners) = state
        .node_listeners
        .get_mut(&node_id)
        .and_then(|map| map.get_mut(&event_type))
    {
        listeners.retain(|listener| {
            listener.handler
                || !(JsObject::equals(&listener.callback, &callback) && listener.capture == capture)
        });
    }
    Ok(JsValue::undefined())
}

#[derive(Clone, Copy)]
pub(crate) enum HandlerOwner {
    Node,
    Window,
}

fn with_map<R>(
    ctx: &DomCtx,
    owner: HandlerOwner,
    this: &JsValue,
    action: impl FnOnce(&mut ListenerMap) -> R,
) -> JsResult<R> {
    let mut state = ctx.state.borrow_mut();
    let map = match owner {
        HandlerOwner::Window => &mut state.window_listeners,
        HandlerOwner::Node => state.node_listeners.entry(this_node_id(this)?).or_default(),
    };
    Ok(action(map))
}

fn get_handler(
    owner: HandlerOwner,
    name: &'static str,
    this: &JsValue,
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    with_map(&ctx, owner, this, |map| {
        map.get(name)
            .and_then(|listeners| listeners.iter().find(|listener| listener.handler))
            .map_or_else(JsValue::null, |listener| listener.callback.clone().into())
    })
}

fn set_handler(
    owner: HandlerOwner,
    name: &'static str,
    this: &JsValue,
    value: Option<JsObject>,
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    with_map(&ctx, owner, this, |map| {
        let listeners = map.entry(name.to_string()).or_default();
        let slot = listeners.iter_mut().find(|listener| listener.handler);
        match (slot, value) {
            (Some(slot), Some(callback)) => slot.callback = callback,
            (None, Some(callback)) => listeners.push(Listener {
                callback,
                capture: false,
                once: false,
                handler: true,
            }),
            (_, None) => listeners.retain(|listener| !listener.handler),
        }
    })?;
    Ok(JsValue::undefined())
}

pub(crate) fn install_handler_accessors(
    target: &JsObject,
    owner: HandlerOwner,
    names: &[&'static str],
    context: &mut Context,
) {
    for &name in names {
        let property = format!("on{name}");
        let getter = FunctionObjectBuilder::new(
            context.realm(),
            NativeFunction::from_copy_closure(move |this, _, context| {
                get_handler(owner, name, this, context)
            }),
        )
        .name(JsString::from(format!("get {property}")))
        .length(0)
        .build();
        let setter = FunctionObjectBuilder::new(
            context.realm(),
            NativeFunction::from_copy_closure(move |this, args, context| {
                let value = args.first().and_then(JsValue::as_object);
                set_handler(owner, name, this, value, context)
            }),
        )
        .name(JsString::from(format!("set {property}")))
        .length(1)
        .build();
        target
            .define_property_or_throw(
                PropertyKey::from(JsString::from(property)),
                PropertyDescriptor::builder()
                    .get(getter)
                    .set(setter)
                    .enumerable(true)
                    .configurable(true)
                    .build(),
                context,
            )
            .expect("failed to define event handler accessor");
    }
}

pub(crate) fn own_data_handler(
    object: &JsObject,
    name: &str,
    context: &mut Context,
) -> Option<JsObject> {
    let key = JsString::from(format!("on{name}"));
    if !object.has_own_property(key.clone(), context).ok()? {
        return None;
    }
    let constructor = context.intrinsics().constructors().object().constructor();
    let describe = constructor
        .get(js_string!("getOwnPropertyDescriptor"), context)
        .ok()?
        .as_object()
        .filter(|describe| describe.is_callable())?;
    let descriptor = describe
        .call(
            &constructor.into(),
            &[object.clone().into(), key.into()],
            context,
        )
        .ok()?
        .as_object()?;
    if !descriptor
        .has_own_property(js_string!("value"), context)
        .ok()?
    {
        return None;
    }
    descriptor
        .get(js_string!("value"), context)
        .ok()?
        .as_object()
        .filter(|handler| handler.is_callable())
}

pub(crate) fn cancel_if_false(result: &JsValue, event: &JsObject, context: &mut Context) {
    if result.as_boolean() != Some(false) {
        return;
    }
    let Ok(prevent) = event.get(js_string!("preventDefault"), context) else {
        return;
    };
    if let Some(prevent) = prevent.as_object().filter(|prevent| prevent.is_callable()) {
        let _ = prevent.call(&event.clone().into(), &[], context);
    }
}
