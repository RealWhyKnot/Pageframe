//! JS `Event` objects dispatched to script event listeners.

use std::cell::Cell;

use boa_engine::object::JsObject;
use boa_engine::value::JsValue;
use boa_engine::{Context, Finalize, JsData, JsResult, Trace};
use keyboard_types::Modifiers;
use pageframe_traits::events::{
    DomEventData, PageframeKeyEvent, PageframePointerEvent, PageframeWheelDelta,
};

use super::{define_accessor, define_method, define_value, js_str, to_rust_string};
use crate::state::DomCtx;

/// Native data attached to JS `Event` objects. Tracks the flags set by
/// `preventDefault` / `stopPropagation` so they can be read back after dispatch.
#[derive(Default, Trace, Finalize, JsData)]
pub(crate) struct EventRef {
    #[unsafe_ignore_trace]
    pub prevented: Cell<bool>,
    #[unsafe_ignore_trace]
    pub stopped: Cell<bool>,
    #[unsafe_ignore_trace]
    pub stopped_immediate: Cell<bool>,
    #[unsafe_ignore_trace]
    pub cancelable: Cell<bool>,
    #[unsafe_ignore_trace]
    pub dispatching: Cell<bool>,
}

pub(crate) fn init_event_proto(proto: &JsObject, context: &mut Context) {
    define_method(proto, "preventDefault", 0, prevent_default, context);
    define_method(proto, "stopPropagation", 0, stop_propagation, context);
    define_method(proto, "composedPath", 0, composed_path, context);
    define_accessor(
        proto,
        "cancelBubble",
        Some(cancel_bubble),
        Some(set_cancel_bubble),
        context,
    );
    define_accessor(
        proto,
        "returnValue",
        Some(return_value),
        Some(set_return_value),
        context,
    );
    define_method(
        proto,
        "stopImmediatePropagation",
        0,
        stop_immediate_propagation,
        context,
    );
    define_accessor(
        proto,
        "defaultPrevented",
        Some(default_prevented),
        None,
        context,
    );
    // `Event.getModifierState(key)`. React's synthetic keyboard/mouse events
    // call `nativeEvent.getModifierState(...)`; without it, dispatching a
    // key/mouse event into React throws "not a callable function". Reads back
    // the event's own `ctrlKey`/`shiftKey`/`altKey`/`metaKey` fields.
    define_method(proto, "getModifierState", 1, get_modifier_state, context);
}

fn get_modifier_state(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let key = to_rust_string(args.first().unwrap_or(&JsValue::undefined()), context)?;
    let prop = match key.as_str() {
        "Control" => "ctrlKey",
        "Shift" => "shiftKey",
        "Alt" => "altKey",
        "Meta" => "metaKey",
        _ => return Ok(JsValue::from(false)),
    };
    let state = this
        .as_object()
        .and_then(|obj| obj.get(boa_engine::JsString::from(prop), context).ok())
        .map(|value| value.to_boolean())
        .unwrap_or(false);
    Ok(JsValue::from(state))
}

fn event_ref<T>(this: &JsValue, f: impl FnOnce(&EventRef) -> T) -> Option<T> {
    this.as_object()
        .and_then(|obj| obj.downcast_ref::<EventRef>().map(|event| f(&event)))
}

fn prevent_default(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    event_ref(this, |event| {
        if event.cancelable.get() {
            event.prevented.set(true);
        }
    });
    Ok(JsValue::undefined())
}

fn cancel_bubble(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::from(
        event_ref(this, |event| event.stopped.get()).unwrap_or(false),
    ))
}

fn set_cancel_bubble(this: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    if args.first().is_some_and(JsValue::to_boolean) {
        event_ref(this, |event| event.stopped.set(true));
    }
    Ok(JsValue::undefined())
}

fn return_value(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::from(
        !event_ref(this, |event| event.prevented.get()).unwrap_or(false),
    ))
}

fn set_return_value(this: &JsValue, args: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    if !args.first().is_some_and(JsValue::to_boolean) {
        prevent_default(this, args, c)?;
    }
    Ok(JsValue::undefined())
}

fn composed_path(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    use boa_engine::object::builtins::JsArray;
    let Some(event) = this.as_object() else {
        return Ok(JsArray::new(context)?.into());
    };
    let current = event.get(boa_engine::js_string!("currentTarget"), context)?;
    let target = event.get(boa_engine::js_string!("target"), context)?;
    if current.is_null_or_undefined() {
        return Ok(JsArray::new(context)?.into());
    }
    let ctx = super::dom_ctx(context)?;
    let mut path: Vec<JsValue> = Vec::new();
    match super::node_id_of_value(&target) {
        Some(node_id) => {
            let (chain, connected) = super::dispatch::event_path(&ctx, node_id);
            for id in chain {
                path.push(super::node_wrapper(&ctx, id, context).into());
            }
            if connected {
                path.push(context.global_object().into());
            }
        }
        None => path.push(target),
    }
    Ok(JsArray::from_iter(path, context).into())
}

pub(crate) fn new_event(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = super::dom_ctx(context)?;
    let event_type = to_rust_string(args.first().unwrap_or(&JsValue::undefined()), context)?;
    let flag = |index: usize| args.get(index).is_some_and(JsValue::to_boolean);
    let event = create_event(
        &ctx,
        &event_type,
        flag(1),
        flag(2),
        &JsValue::null(),
        context,
    );
    if let Some(state) = event.downcast_ref::<EventRef>() {
        state.cancelable.set(flag(2));
    }
    define_value(&event, "composed", JsValue::from(flag(3)), context);
    define_value(&event, "isTrusted", JsValue::from(false), context);
    define_value(&event, "eventPhase", JsValue::from(0), context);
    Ok(event.into())
}

pub(crate) fn event_flags(_: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    let value = args.first().cloned().unwrap_or_default();
    let bits = event_ref(&value, |event| {
        u8::from(event.prevented.get())
            | (u8::from(event.stopped.get()) << 1)
            | (u8::from(event.stopped_immediate.get()) << 2)
            | (u8::from(event.dispatching.get()) << 3)
    })
    .unwrap_or(0);
    Ok(JsValue::from(bits))
}

pub(crate) fn adopt_interface(event: &JsObject, name: &str, context: &mut Context) {
    let proto = context
        .global_object()
        .get(boa_engine::JsString::from(name), context)
        .ok()
        .and_then(|iface| iface.as_object())
        .and_then(|iface| iface.get(boa_engine::js_string!("prototype"), context).ok())
        .and_then(|proto| proto.as_object());
    if let Some(proto) = proto {
        event.set_prototype(Some(proto));
    }
}

fn stop_propagation(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    event_ref(this, |event| event.stopped.set(true));
    Ok(JsValue::undefined())
}

fn stop_immediate_propagation(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    event_ref(this, |event| {
        event.stopped.set(true);
        event.stopped_immediate.set(true);
    });
    Ok(JsValue::undefined())
}

fn default_prevented(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::from(
        event_ref(this, |event| event.prevented.get()).unwrap_or(false),
    ))
}

/// Create a JS event object with the standard `Event` fields
pub(crate) fn create_event(
    ctx: &DomCtx,
    event_type: &str,
    bubbles: bool,
    cancelable: bool,
    target: &JsValue,
    context: &mut Context,
) -> JsObject {
    let proto = ctx.state.borrow().protos().event.clone();
    let data = EventRef::default();
    data.cancelable.set(true);
    let event = JsObject::from_proto_and_data(Some(proto), data);
    define_value(&event, "type", js_str(event_type), context);
    define_value(&event, "target", target.clone(), context);
    define_value(&event, "srcElement", target.clone(), context);
    define_value(&event, "currentTarget", JsValue::null(), context);
    define_value(&event, "bubbles", JsValue::from(bubbles), context);
    define_value(&event, "cancelable", JsValue::from(cancelable), context);
    define_value(&event, "composed", JsValue::from(false), context);
    define_value(&event, "isTrusted", JsValue::from(true), context);
    define_value(&event, "eventPhase", JsValue::from(2), context);
    let timestamp = ctx.state.borrow().high_res_now();
    define_value(&event, "timeStamp", JsValue::from(timestamp), context);
    event
}

fn add_modifiers(event: &JsObject, mods: Modifiers, context: &mut Context) {
    define_value(
        event,
        "ctrlKey",
        JsValue::from(mods.contains(Modifiers::CONTROL)),
        context,
    );
    define_value(
        event,
        "shiftKey",
        JsValue::from(mods.contains(Modifiers::SHIFT)),
        context,
    );
    define_value(
        event,
        "altKey",
        JsValue::from(mods.contains(Modifiers::ALT)),
        context,
    );
    define_value(
        event,
        "metaKey",
        JsValue::from(mods.contains(Modifiers::META)),
        context,
    );
}

fn add_pointer_fields(event: &JsObject, data: &PageframePointerEvent, context: &mut Context) {
    define_value(
        event,
        "clientX",
        JsValue::from(data.client_x() as f64),
        context,
    );
    define_value(
        event,
        "clientY",
        JsValue::from(data.client_y() as f64),
        context,
    );
    define_value(event, "x", JsValue::from(data.client_x() as f64), context);
    define_value(event, "y", JsValue::from(data.client_y() as f64), context);
    define_value(event, "pageX", JsValue::from(data.page_x() as f64), context);
    define_value(event, "pageY", JsValue::from(data.page_y() as f64), context);
    define_value(
        event,
        "screenX",
        JsValue::from(data.screen_x() as f64),
        context,
    );
    define_value(
        event,
        "screenY",
        JsValue::from(data.screen_y() as f64),
        context,
    );
    define_value(
        event,
        "offsetX",
        JsValue::from(data.element_x() as f64),
        context,
    );
    define_value(
        event,
        "offsetY",
        JsValue::from(data.element_y() as f64),
        context,
    );
    define_value(event, "button", JsValue::from(data.button as u8), context);
    define_value(
        event,
        "buttons",
        JsValue::from(data.buttons.bits()),
        context,
    );
    define_value(event, "detail", JsValue::from(1), context);
    add_modifiers(event, data.mods, context);
}

fn add_key_fields(event: &JsObject, data: &PageframeKeyEvent, context: &mut Context) {
    define_value(event, "key", js_str(&data.key.to_string()), context);
    define_value(event, "code", js_str(&data.code.to_string()), context);
    define_value(
        event,
        "location",
        JsValue::from(data.location as u32),
        context,
    );
    define_value(
        event,
        "repeat",
        JsValue::from(data.is_auto_repeating),
        context,
    );
    define_value(
        event,
        "isComposing",
        JsValue::from(data.is_composing),
        context,
    );
    add_modifiers(event, data.modifiers, context);
}

/// Create a JS event object for a Pageframe [`DomEventData`], populating
/// type-specific fields (mouse coordinates, key names, etc)
pub(crate) fn create_event_for_dom_event(
    ctx: &DomCtx,
    data: &DomEventData,
    bubbles: bool,
    cancelable: bool,
    target: &JsValue,
    context: &mut Context,
) -> JsObject {
    let event = create_event(ctx, data.name(), bubbles, cancelable, target, context);
    adopt_interface(&event, interface_name(data), context);
    if let Some(pointer) = pointer_event_data(data) {
        add_pointer_identity(&event, pointer, context);
    }

    match data {
        DomEventData::PointerMove(pointer)
        | DomEventData::PointerDown(pointer)
        | DomEventData::PointerUp(pointer)
        | DomEventData::PointerCancel(pointer)
        | DomEventData::PointerEnter(pointer)
        | DomEventData::PointerLeave(pointer)
        | DomEventData::PointerOver(pointer)
        | DomEventData::PointerOut(pointer)
        | DomEventData::MouseMove(pointer)
        | DomEventData::MouseDown(pointer)
        | DomEventData::MouseUp(pointer)
        | DomEventData::MouseEnter(pointer)
        | DomEventData::MouseLeave(pointer)
        | DomEventData::MouseOver(pointer)
        | DomEventData::MouseOut(pointer)
        | DomEventData::TouchStart(pointer)
        | DomEventData::TouchMove(pointer)
        | DomEventData::TouchEnd(pointer)
        | DomEventData::TouchCancel(pointer)
        | DomEventData::Click(pointer)
        | DomEventData::ContextMenu(pointer)
        | DomEventData::DoubleClick(pointer) => {
            add_pointer_fields(&event, pointer, context);
        }

        DomEventData::KeyPress(key) | DomEventData::KeyDown(key) | DomEventData::KeyUp(key) => {
            add_key_fields(&event, key, context);
        }

        DomEventData::Wheel(wheel) => {
            let (delta_x, delta_y, delta_mode) = match wheel.delta {
                PageframeWheelDelta::Lines(x, y) => (x, y, 1),
                PageframeWheelDelta::Pixels(x, y) => (x, y, 0),
            };
            define_value(&event, "deltaX", JsValue::from(delta_x), context);
            define_value(&event, "deltaY", JsValue::from(delta_y), context);
            define_value(&event, "deltaZ", JsValue::from(0.0), context);
            define_value(&event, "deltaMode", JsValue::from(delta_mode), context);
            add_modifiers(&event, wheel.mods, context);
        }

        DomEventData::Input(input) => {
            define_value(&event, "data", js_str(&input.value), context);
            define_value(&event, "inputType", js_str("insertText"), context);
        }

        _ => {}
    }

    event
}

fn interface_name(data: &DomEventData) -> &'static str {
    match data {
        DomEventData::PointerMove(_)
        | DomEventData::PointerDown(_)
        | DomEventData::PointerUp(_)
        | DomEventData::PointerCancel(_)
        | DomEventData::PointerEnter(_)
        | DomEventData::PointerLeave(_)
        | DomEventData::PointerOver(_)
        | DomEventData::PointerOut(_)
        | DomEventData::Click(_)
        | DomEventData::ContextMenu(_) => "PointerEvent",
        DomEventData::MouseMove(_)
        | DomEventData::MouseDown(_)
        | DomEventData::MouseUp(_)
        | DomEventData::MouseEnter(_)
        | DomEventData::MouseLeave(_)
        | DomEventData::MouseOver(_)
        | DomEventData::MouseOut(_)
        | DomEventData::DoubleClick(_) => "MouseEvent",
        DomEventData::TouchStart(_)
        | DomEventData::TouchMove(_)
        | DomEventData::TouchEnd(_)
        | DomEventData::TouchCancel(_) => "TouchEvent",
        DomEventData::Wheel(_) => "WheelEvent",
        DomEventData::KeyPress(_) | DomEventData::KeyDown(_) | DomEventData::KeyUp(_) => {
            "KeyboardEvent"
        }
        DomEventData::Input(_) => "InputEvent",
        DomEventData::Ime(_) => "CompositionEvent",
        DomEventData::Focus(_)
        | DomEventData::Blur(_)
        | DomEventData::FocusIn(_)
        | DomEventData::FocusOut(_) => "FocusEvent",
        DomEventData::Scroll(_) | DomEventData::AppleStandardKeybinding(_) => "Event",
    }
}

fn pointer_event_data(data: &DomEventData) -> Option<&PageframePointerEvent> {
    match data {
        DomEventData::PointerMove(pointer)
        | DomEventData::PointerDown(pointer)
        | DomEventData::PointerUp(pointer)
        | DomEventData::PointerCancel(pointer)
        | DomEventData::PointerEnter(pointer)
        | DomEventData::PointerLeave(pointer)
        | DomEventData::PointerOver(pointer)
        | DomEventData::PointerOut(pointer)
        | DomEventData::Click(pointer)
        | DomEventData::ContextMenu(pointer) => Some(pointer),
        _ => None,
    }
}

fn add_pointer_identity(event: &JsObject, data: &PageframePointerEvent, context: &mut Context) {
    use pageframe_traits::events::PageframePointerId;
    let (id, kind) = match data.id {
        PageframePointerId::Mouse => (1.0, "mouse"),
        PageframePointerId::Pen => (2.0, "pen"),
        PageframePointerId::Finger(finger) => (finger as f64 + 3.0, "touch"),
    };
    define_value(event, "pointerId", JsValue::from(id), context);
    define_value(event, "pointerType", js_str(kind), context);
    define_value(event, "isPrimary", JsValue::from(data.is_primary), context);
    define_value(event, "width", JsValue::from(1), context);
    define_value(event, "height", JsValue::from(1), context);
    define_value(
        event,
        "pressure",
        JsValue::from(data.details.pressure),
        context,
    );
    define_value(
        event,
        "tiltX",
        JsValue::from(i32::from(data.details.tilt_x)),
        context,
    );
    define_value(
        event,
        "tiltY",
        JsValue::from(i32::from(data.details.tilt_y)),
        context,
    );
    define_value(
        event,
        "twist",
        JsValue::from(u32::from(data.details.twist)),
        context,
    );
}
