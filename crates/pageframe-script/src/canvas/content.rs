use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::object::ObjectInitializer;
use boa_engine::object::builtins::{JsArray, JsArrayBuffer};
use boa_engine::property::Attribute;
use boa_engine::value::JsValue;
use boa_engine::{Context, JsNativeError, JsResult, JsString};
use pageframe_dom::canvas::{
    Canvas2D, Gradient, GradientKind, ImageSource, Rgba, TextMetrics, with_fonts,
};
use pageframe_dom::canvas::{MAX_AREA, Rect};
use pageframe_dom::{NodeId, local_name};

use super::ops::{apply, arg, finite, index_size, nums, required};
use super::{GradientData, bytes_value, canvas_of, gradient_object};
use crate::dom::{dom_ctx, dom_exception, node_id_of_value, to_rust_string};

fn text_args(
    args: &[JsValue],
    name: &str,
    context: &mut Context,
) -> JsResult<(String, (f64, f64), Option<f64>)> {
    required(args, 3, name)?;
    let text = to_rust_string(&arg(args, 0), context)?;
    let x = arg(args, 1).to_number(context)?;
    let y = arg(args, 2).to_number(context)?;
    let max = match args.get(3) {
        Some(v) if !v.is_undefined() => Some(v.to_number(context)?),
        _ => None,
    };
    Ok((text, (x, y), max))
}

pub(super) fn fill_text(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let (text, at, max) = text_args(args, "fillText", context)?;
    let canvas = canvas_of(this)?;
    let ctx = dom_ctx(context)?;
    let doc = ctx.doc.borrow();
    with_fonts(&doc, |fonts| {
        canvas.borrow_mut().fill_text(fonts, &text, at, max)
    });
    Ok(JsValue::undefined())
}

pub(super) fn stroke_text(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let (text, at, max) = text_args(args, "strokeText", context)?;
    let canvas = canvas_of(this)?;
    let ctx = dom_ctx(context)?;
    let doc = ctx.doc.borrow();
    with_fonts(&doc, |fonts| {
        canvas.borrow_mut().stroke_text(fonts, &text, at, max)
    });
    Ok(JsValue::undefined())
}

fn metrics_object(m: &TextMetrics, context: &mut Context) -> JsValue {
    let fields = [
        ("width", m.width),
        ("actualBoundingBoxLeft", m.actual_left),
        ("actualBoundingBoxRight", m.actual_right),
        ("actualBoundingBoxAscent", m.actual_ascent),
        ("actualBoundingBoxDescent", m.actual_descent),
        ("fontBoundingBoxAscent", m.font_ascent),
        ("fontBoundingBoxDescent", m.font_descent),
        ("emHeightAscent", m.em_ascent),
        ("emHeightDescent", m.em_descent),
        ("hangingBaseline", m.hanging_baseline),
        ("alphabeticBaseline", m.alphabetic_baseline),
        ("ideographicBaseline", m.ideographic_baseline),
    ];
    let mut init = ObjectInitializer::new(context);
    for (name, value) in fields {
        init.property(JsString::from(name), value, Attribute::all());
    }
    init.build().into()
}

pub(super) fn measure_text(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    required(args, 1, "measureText")?;
    let text = to_rust_string(&arg(args, 0), context)?;
    let canvas = canvas_of(this)?;
    let ctx = dom_ctx(context)?;
    let metrics = {
        let doc = ctx.doc.borrow();
        with_fonts(&doc, |fonts| canvas.borrow().measure_text(fonts, &text))
    };
    let metrics = metrics.unwrap_or_default();
    Ok(metrics_object(&metrics, context))
}

pub(super) fn set_line_dash(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    required(args, 1, "setLineDash")?;
    let Some(object) = arg(args, 0).as_object().filter(|o| o.is_array()) else {
        return Err(JsNativeError::typ()
            .with_message("Failed to execute 'setLineDash' on 'CanvasRenderingContext2D': The provided value cannot be converted to a sequence.")
            .into());
    };
    let array = JsArray::from_object(object.clone())?;
    let mut segments = Vec::new();
    for i in 0..array.length(context)?.min(65_536) {
        segments.push(array.get(i, context)?.to_number(context)?);
    }
    apply(this, |c| {
        c.set_line_dash(&segments);
    })
}

pub(super) fn get_line_dash(
    this: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let dash = canvas_of(this)?.borrow().state().dash.clone();
    let values: Vec<JsValue> = dash.iter().copied().map(JsValue::from).collect();
    Ok(JsArray::from_iter(values, context).into())
}

fn gradient(kind: GradientKind, context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    Ok(gradient_object(&ctx, Gradient::new(kind), context))
}

fn out_of_memory() -> boa_engine::JsError {
    JsNativeError::range()
        .with_message("Out of memory at ImageData creation")
        .into()
}

fn non_finite() -> boa_engine::JsError {
    JsNativeError::typ()
        .with_message("The provided double value is non-finite.")
        .into()
}

pub(super) fn create_linear_gradient(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    required(args, 4, "createLinearGradient")?;
    let [x0, y0, x1, y1] = nums(args, context)?;
    if !finite(&[x0, y0, x1, y1]) {
        return Err(non_finite());
    }
    gradient(GradientKind::Linear { x0, y0, x1, y1 }, context)
}

pub(super) fn create_radial_gradient(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    required(args, 6, "createRadialGradient")?;
    let [x0, y0, r0, x1, y1, r1] = nums(args, context)?;
    if !finite(&[x0, y0, r0, x1, y1, r1]) {
        return Err(non_finite());
    }
    if r0 < 0.0 || r1 < 0.0 {
        let r = if r0 < 0.0 { r0 } else { r1 };
        return Err(index_size(
            context,
            &format!("The radius provided ({r}) is negative."),
        ));
    }
    gradient(
        GradientKind::Radial {
            x0,
            y0,
            r0,
            x1,
            y1,
            r1,
        },
        context,
    )
}

pub(super) fn create_conic_gradient(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    required(args, 3, "createConicGradient")?;
    let [angle, x, y] = nums(args, context)?;
    if !finite(&[angle, x, y]) {
        return Err(non_finite());
    }
    gradient(GradientKind::Conic { angle, x, y }, context)
}

pub(super) fn create_pattern(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::null())
}

pub(super) fn add_color_stop(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let gradient = this
        .as_object()
        .and_then(|o| {
            o.downcast_ref::<GradientData>()
                .map(|d| Rc::clone(&d.gradient))
        })
        .ok_or_else(|| JsNativeError::typ().with_message("not a CanvasGradient"))?;
    if args.len() < 2 {
        return Err(JsNativeError::typ()
            .with_message(
                "Failed to execute 'addColorStop' on 'CanvasGradient': 2 arguments required.",
            )
            .into());
    }
    let offset = arg(args, 0).to_number(context)?;
    if !offset.is_finite() {
        return Err(non_finite());
    }
    if !(0.0..=1.0).contains(&offset) {
        return Err(index_size(
            context,
            &format!("The provided value ({offset}) is outside the range (0.0, 1.0)."),
        ));
    }
    let text = to_rust_string(&arg(args, 1), context)?;
    let Some(color) = Rgba::parse(&text) else {
        return Err(dom_exception(
            context,
            "SyntaxError",
            &format!("The value provided ('{text}') could not be parsed as a color."),
        ));
    };
    gradient.borrow_mut().add_stop(offset, color);
    Ok(JsValue::undefined())
}

enum Pixels {
    Canvas(Rc<RefCell<Canvas2D>>),
    Image(u32, u32, Vec<u8>),
}

fn image_source(value: &JsValue, context: &mut Context) -> JsResult<Option<Pixels>> {
    if let Ok(canvas) = canvas_of(value) {
        return Ok(Some(Pixels::Canvas(canvas)));
    }
    let Some(node_id) = node_id_of_value(value) else {
        return Err(JsNativeError::typ()
            .with_message("Failed to execute 'drawImage' on 'CanvasRenderingContext2D': The provided value is not of type '(CanvasImageSource or VideoFrame)'.")
            .into());
    };
    let ctx = dom_ctx(context)?;
    if let Some((_, canvas)) = ctx.state.borrow().canvas.contexts.get(&node_id) {
        return Ok(Some(Pixels::Canvas(Rc::clone(canvas))));
    }
    Ok(element_pixels(&ctx.doc.borrow(), node_id))
}

fn element_pixels(doc: &pageframe_dom::BaseDocument, node_id: NodeId) -> Option<Pixels> {
    let element = doc.get_node(node_id)?.element_data()?;
    if element.name.local == local_name!("canvas") {
        return None;
    }
    let raster = element.raster_image_data()?;
    let data = ImageSource::premultiply(raster.data.as_ref());
    Some(Pixels::Image(raster.width, raster.height, data))
}

fn draw_rects(args: &[JsValue], size: (f64, f64), context: &mut Context) -> JsResult<(Rect, Rect)> {
    let full = Rect::new(0.0, 0.0, size.0, size.1);
    match args.len() {
        0..=2 => Err(JsNativeError::typ()
            .with_message("Failed to execute 'drawImage' on 'CanvasRenderingContext2D': 3 arguments required.")
            .into()),
        3 | 4 => {
            let [x, y] = nums(&args[1..], context)?;
            Ok((full, Rect::new(x, y, x + size.0, y + size.1)))
        }
        5..=8 => {
            let [x, y, w, h] = nums(&args[1..], context)?;
            Ok((full, Rect::new(x, y, x + w, y + h)))
        }
        _ => {
            let [sx, sy, sw, sh, dx, dy, dw, dh] = nums(&args[1..], context)?;
            Ok((
                Rect::new(sx, sy, sx + sw, sy + sh),
                Rect::new(dx, dy, dx + dw, dy + dh),
            ))
        }
    }
}

pub(super) fn draw_image(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let target = canvas_of(this)?;
    let Some(source) = image_source(&arg(args, 0), context)? else {
        draw_rects(args, (0.0, 0.0), context)?;
        return Ok(JsValue::undefined());
    };
    let (width, height, data) = match source {
        Pixels::Canvas(canvas) => {
            let canvas = canvas.borrow();
            let pixels = canvas.premultiplied_pixels().to_vec();
            (canvas.width(), canvas.height(), pixels)
        }
        Pixels::Image(w, h, data) => (w, h, data),
    };
    let size = (f64::from(width), f64::from(height));
    let (src, dst) = draw_rects(args, size, context)?;
    let image = ImageSource {
        width,
        height,
        premultiplied: &data,
    };
    target.borrow_mut().draw_image(&image, src, dst);
    Ok(JsValue::undefined())
}

pub(super) fn context_attributes(
    _: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let mut init = ObjectInitializer::new(context);
    init.property(JsString::from("alpha"), true, Attribute::all());
    init.property(
        JsString::from("colorSpace"),
        JsString::from("srgb"),
        Attribute::all(),
    );
    init.property(JsString::from("desynchronized"), false, Attribute::all());
    init.property(
        JsString::from("willReadFrequently"),
        false,
        Attribute::all(),
    );
    Ok(init.build().into())
}

pub(super) fn read_pixels(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let canvas = canvas_of(&arg(args, 0))?;
    let [x, y, w, h] = nums(&args[1.min(args.len())..], context)?;
    let (w, h) = (w.max(0.0) as u32, h.max(0.0) as u32);
    if u64::from(w) * u64::from(h) > MAX_AREA {
        return Err(out_of_memory());
    }
    let bytes = canvas.borrow().get_image_data(x as i64, y as i64, w, h);
    Ok(bytes_value(&bytes, context))
}

pub(super) fn write_pixels(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let canvas = canvas_of(&arg(args, 0))?;
    let buffer = arg(args, 1)
        .as_object()
        .and_then(|o| JsArrayBuffer::from_object(o.clone()).ok())
        .ok_or_else(|| JsNativeError::typ().with_message("expected ArrayBuffer"))?;
    let rest = &args[2.min(args.len())..];
    let [w, h, dx, dy, x0, y0, dw, dh] = nums(rest, context)?;
    let data = buffer.data().map(|d| d.to_vec()).unwrap_or_default();
    let dirty = Rect::new(x0, y0, x0 + dw, y0 + dh);
    let size = (w.max(0.0) as u32, h.max(0.0) as u32);
    canvas
        .borrow_mut()
        .put_image_data(&data, size, (dx as i64, dy as i64), dirty);
    Ok(JsValue::undefined())
}
