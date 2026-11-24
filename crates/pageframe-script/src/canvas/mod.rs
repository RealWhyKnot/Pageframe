mod content;
mod encode;
mod ops;
mod props;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::object::builtins::JsArrayBuffer;
use boa_engine::object::{FunctionObjectBuilder, JsObject};
use boa_engine::property::{PropertyDescriptor, PropertyKey};
use boa_engine::value::JsValue;
use boa_engine::{
    Context, Finalize, JsData, JsNativeError, JsResult, JsString, NativeFunction, Trace,
};
use pageframe_dom::canvas::{Canvas2D, Gradient};
use pageframe_dom::{NodeId, local_name};

use crate::dom::element::read_attr;
use crate::dom::{define_method, dom_ctx, node_id_of_value, node_wrapper, to_rust_string};
use crate::state::DomCtx;

#[derive(Trace, Finalize, JsData)]
pub(crate) struct ContextData {
    #[unsafe_ignore_trace]
    pub(crate) canvas: Rc<RefCell<Canvas2D>>,
}

#[derive(Trace, Finalize, JsData)]
pub(crate) struct GradientData {
    #[unsafe_ignore_trace]
    pub(crate) gradient: Rc<RefCell<Gradient>>,
}

#[derive(Default)]
pub(crate) struct CanvasRegistry {
    context_proto: Option<JsObject>,
    gradient_proto: Option<JsObject>,
    contexts: HashMap<NodeId, (JsObject, Rc<RefCell<Canvas2D>>)>,
}

pub(crate) fn init(ctx: &DomCtx, context: &mut Context) {
    let context_proto = JsObject::with_object_proto(context.intrinsics());
    ops::init_methods(&context_proto, context);
    props::init_accessors(&context_proto, context);
    let gradient_proto = JsObject::with_object_proto(context.intrinsics());
    define_method(
        &gradient_proto,
        "addColorStop",
        2,
        content::add_color_stop,
        context,
    );
    install_interface("CanvasRenderingContext2D", &context_proto, context);
    install_interface("CanvasGradient", &gradient_proto, context);
    let element_proto = ctx
        .state
        .borrow()
        .protos
        .as_ref()
        .map(|p| p.element.clone());
    if let Some(proto) = element_proto {
        define_method(&proto, "getContext", 1, get_context, context);
    }
    register(context, "__pageframe_canvas_create", 3, create_detached);
    register(context, "__pageframe_canvas_resize", 3, resize_detached);
    register(context, "__pageframe_canvas_snapshot", 1, snapshot);
    register(
        context,
        "__pageframe_canvas_data_url",
        3,
        encode::to_data_url,
    );
    register(context, "__pageframe_canvas_encode", 3, encode::to_bytes);
    let mut state = ctx.state.borrow_mut();
    state.canvas.context_proto = Some(context_proto);
    state.canvas.gradient_proto = Some(gradient_proto);
}

fn install_interface(name: &str, proto: &JsObject, context: &mut Context) {
    let ctor = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|_, _, _| {
            Err(JsNativeError::typ()
                .with_message("Illegal constructor")
                .into())
        }),
    )
    .name(JsString::from(name))
    .length(0)
    .constructor(true)
    .build();
    let hidden = |value: JsValue| {
        PropertyDescriptor::builder()
            .value(value)
            .writable(true)
            .enumerable(false)
            .configurable(true)
            .build()
    };
    let _ = ctor.define_property_or_throw(
        PropertyKey::from(JsString::from("prototype")),
        hidden(proto.clone().into()),
        context,
    );
    let _ = proto.define_property_or_throw(
        PropertyKey::from(JsString::from("constructor")),
        hidden(ctor.clone().into()),
        context,
    );
    let _ = context.register_global_property(
        JsString::from(name),
        ctor,
        boa_engine::property::Attribute::WRITABLE | boa_engine::property::Attribute::CONFIGURABLE,
    );
}

fn register(
    context: &mut Context,
    name: &str,
    length: usize,
    body: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
) {
    let _ = context.register_global_callable(
        JsString::from(name),
        length,
        NativeFunction::from_fn_ptr(body),
    );
}

pub(crate) fn canvas_of(value: &JsValue) -> JsResult<Rc<RefCell<Canvas2D>>> {
    value
        .as_object()
        .and_then(|obj| {
            obj.downcast_ref::<ContextData>()
                .map(|d| Rc::clone(&d.canvas))
        })
        .ok_or_else(|| {
            JsNativeError::typ()
                .with_message("not a CanvasRenderingContext2D")
                .into()
        })
}

pub(crate) fn gradient_object(ctx: &DomCtx, gradient: Gradient, context: &mut Context) -> JsValue {
    wrap_gradient(ctx, Rc::new(RefCell::new(gradient)), context)
}

pub(crate) fn wrap_gradient(
    ctx: &DomCtx,
    gradient: Rc<RefCell<Gradient>>,
    context: &mut Context,
) -> JsValue {
    let proto = ctx.state.borrow().canvas.gradient_proto.clone();
    let proto = proto.unwrap_or_else(|| JsObject::with_object_proto(context.intrinsics()));
    JsObject::from_proto_and_data(Some(proto), GradientData { gradient }).into()
}

fn new_context(
    ctx: &DomCtx,
    canvas: Rc<RefCell<Canvas2D>>,
    owner: JsValue,
    context: &mut Context,
) -> JsObject {
    let proto = ctx.state.borrow().canvas.context_proto.clone();
    let object = JsObject::from_proto_and_data(proto, ContextData { canvas });
    let _ = object.define_property_or_throw(
        PropertyKey::from(JsString::from("canvas")),
        PropertyDescriptor::builder()
            .value(owner)
            .writable(false)
            .enumerable(false)
            .configurable(true)
            .build(),
        context,
    );
    object
}

fn dimension(ctx: &DomCtx, node_id: NodeId, name: &str, default: u32) -> u32 {
    read_attr(ctx, node_id, name)
        .and_then(|v| {
            let digits: String = v.trim().chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u32>().ok()
        })
        .filter(|v| *v <= 2_147_483_647)
        .unwrap_or(default)
}

fn bitmap_size(ctx: &DomCtx, node_id: NodeId) -> (u32, u32) {
    (
        dimension(ctx, node_id, "width", 300),
        dimension(ctx, node_id, "height", 150),
    )
}

fn get_context(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let Some(node_id) = node_id_of_value(this) else {
        return Ok(JsValue::null());
    };
    let is_canvas = ctx
        .doc
        .borrow()
        .get_node(node_id)
        .and_then(|n| n.element_data())
        .is_some_and(|e| e.name.local == local_name!("canvas"));
    if !is_canvas {
        return Ok(JsValue::undefined());
    }
    let kind = match args.first() {
        Some(value) => to_rust_string(value, context)?,
        None => String::new(),
    };
    if kind != "2d" {
        return Ok(JsValue::null());
    }
    if let Some((object, _)) = ctx.state.borrow().canvas.contexts.get(&node_id) {
        return Ok(object.clone().into());
    }
    let (w, h) = bitmap_size(&ctx, node_id);
    let canvas = Rc::new(RefCell::new(Canvas2D::new(w, h)));
    let owner = node_wrapper(&ctx, node_id, context);
    let object = new_context(&ctx, Rc::clone(&canvas), owner.into(), context);
    ctx.state
        .borrow_mut()
        .canvas
        .contexts
        .insert(node_id, (object.clone(), canvas));
    Ok(object.into())
}

fn size_args(args: &[JsValue], context: &mut Context) -> JsResult<(u32, u32)> {
    let mut size = [0u32; 2];
    for (i, slot) in size.iter_mut().enumerate() {
        let n = args
            .get(i + 1)
            .cloned()
            .unwrap_or_default()
            .to_number(context)?;
        *slot = if n.is_finite() {
            n.clamp(0.0, 4_294_967_295.0) as u32
        } else {
            0
        };
    }
    Ok((size[0], size[1]))
}

fn create_detached(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let (w, h) = size_args(args, context)?;
    let canvas = Rc::new(RefCell::new(Canvas2D::new(w, h)));
    let owner = args.first().cloned().unwrap_or_default();
    Ok(new_context(&ctx, canvas, owner, context).into())
}

fn resize_detached(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let canvas = canvas_of(&args.first().cloned().unwrap_or_default())?;
    let (w, h) = size_args(args, context)?;
    canvas.borrow_mut().resize(w, h);
    Ok(JsValue::undefined())
}

fn snapshot(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let source = canvas_of(&args.first().cloned().unwrap_or_default())?;
    let copy = source.borrow().clone();
    source.borrow_mut().reset();
    Ok(new_context(&ctx, Rc::new(RefCell::new(copy)), JsValue::null(), context).into())
}

pub(crate) fn reset_for_attribute(ctx: &DomCtx, node_id: NodeId, name: &str) {
    if name != "width" && name != "height" {
        return;
    }
    let canvas = ctx
        .state
        .borrow()
        .canvas
        .contexts
        .get(&node_id)
        .map(|(_, canvas)| Rc::clone(canvas));
    if let Some(canvas) = canvas {
        let (w, h) = bitmap_size(ctx, node_id);
        canvas.borrow_mut().resize(w, h);
    }
}

pub(crate) fn flush(ctx: &DomCtx) {
    let dirty: Vec<(NodeId, Rc<RefCell<Canvas2D>>)> = ctx
        .state
        .borrow()
        .canvas
        .contexts
        .iter()
        .filter(|(_, (_, canvas))| canvas.borrow().is_dirty())
        .map(|(id, (_, canvas))| (*id, Rc::clone(canvas)))
        .collect();
    if dirty.is_empty() {
        return;
    }
    let mut doc = ctx.doc.borrow_mut();
    for (node_id, canvas) in dirty {
        let mut canvas = canvas.borrow_mut();
        canvas.take_dirty();
        pageframe_dom::canvas::present(&mut doc, node_id, &canvas);
    }
}

pub(crate) fn bytes_value(bytes: &[u8], context: &mut Context) -> JsValue {
    let Ok(buffer) = JsArrayBuffer::new(bytes.len(), context) else {
        return JsValue::null();
    };
    if let Some(mut data) = buffer.data_mut() {
        data.copy_from_slice(bytes);
    }
    buffer.into()
}
