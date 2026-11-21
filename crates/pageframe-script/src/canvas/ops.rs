use boa_engine::object::JsObject;
use boa_engine::object::builtins::JsArray;
use boa_engine::value::JsValue;
use boa_engine::{Context, JsNativeError, JsResult, JsString};
use pageframe_dom::canvas::{Affine, Point, Vec2};
use pageframe_dom::canvas::{Canvas2D, FillRule, Radii};

use super::content::{
    context_attributes, create_conic_gradient, create_linear_gradient, create_pattern,
    create_radial_gradient, draw_image, fill_text, get_line_dash, measure_text, read_pixels,
    set_line_dash, stroke_text, write_pixels,
};
use super::{canvas_of, register};
use crate::dom::{NativeFnPtr, define_method, dom_exception, to_rust_string};

pub(super) fn init_methods(proto: &JsObject, context: &mut Context) {
    let table: [(&str, usize, NativeFnPtr); 38] = [
        ("save", 0, save),
        ("restore", 0, restore),
        ("reset", 0, reset),
        ("scale", 2, scale),
        ("rotate", 1, rotate),
        ("translate", 2, translate),
        ("transform", 6, transform),
        ("setTransform", 0, set_transform),
        ("resetTransform", 0, reset_transform),
        ("beginPath", 0, begin_path),
        ("closePath", 0, close_path),
        ("moveTo", 2, move_to),
        ("lineTo", 2, line_to),
        ("quadraticCurveTo", 4, quadratic_curve_to),
        ("bezierCurveTo", 6, bezier_curve_to),
        ("arc", 5, arc),
        ("arcTo", 5, arc_to),
        ("ellipse", 7, ellipse),
        ("rect", 4, rect),
        ("roundRect", 4, round_rect),
        ("fill", 0, fill),
        ("stroke", 0, stroke),
        ("clip", 0, clip),
        ("isPointInPath", 2, always_false),
        ("isPointInStroke", 2, always_false),
        ("fillRect", 4, fill_rect),
        ("strokeRect", 4, stroke_rect),
        ("clearRect", 4, clear_rect),
        ("fillText", 3, fill_text),
        ("strokeText", 3, stroke_text),
        ("measureText", 1, measure_text),
        ("setLineDash", 1, set_line_dash),
        ("getLineDash", 0, get_line_dash),
        ("createLinearGradient", 4, create_linear_gradient),
        ("createRadialGradient", 6, create_radial_gradient),
        ("createConicGradient", 3, create_conic_gradient),
        ("createPattern", 2, create_pattern),
        ("drawImage", 3, draw_image),
    ];
    for (name, length, body) in table {
        define_method(proto, name, length, body, context);
    }
    define_method(
        proto,
        "getContextAttributes",
        0,
        context_attributes,
        context,
    );
    register(context, "__pageframe_canvas_read", 5, read_pixels);
    register(context, "__pageframe_canvas_write", 10, write_pixels);
    register(context, "__pageframe_canvas_matrix", 1, matrix);
}

pub(super) fn arg(args: &[JsValue], i: usize) -> JsValue {
    args.get(i).cloned().unwrap_or_default()
}

pub(super) fn nums<const N: usize>(args: &[JsValue], context: &mut Context) -> JsResult<[f64; N]> {
    let mut out = [0.0; N];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = arg(args, i).to_number(context)?;
    }
    Ok(out)
}

pub(super) fn required(args: &[JsValue], n: usize, name: &str) -> JsResult<()> {
    if args.len() < n {
        return Err(JsNativeError::typ()
            .with_message(format!(
                "Failed to execute '{name}' on 'CanvasRenderingContext2D': {n} arguments required, but only {} present.",
                args.len()
            ))
            .into());
    }
    Ok(())
}

pub(super) fn apply(this: &JsValue, f: impl FnOnce(&mut Canvas2D)) -> JsResult<JsValue> {
    let canvas = canvas_of(this)?;
    f(&mut canvas.borrow_mut());
    Ok(JsValue::undefined())
}

pub(super) fn index_size(context: &mut Context, message: &str) -> boa_engine::JsError {
    dom_exception(context, "IndexSizeError", message)
}

fn save(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    apply(this, Canvas2D::save)
}

fn restore(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    apply(this, Canvas2D::restore)
}

fn reset(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    apply(this, Canvas2D::reset)
}

pub(super) fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

fn scale(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 2, "scale")?;
    let [x, y] = nums(args, context)?;
    apply(this, |c| {
        if finite(&[x, y]) {
            c.transform(Affine::scale_non_uniform(x, y));
        }
    })
}

fn rotate(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 1, "rotate")?;
    let [angle] = nums(args, context)?;
    apply(this, |c| {
        if angle.is_finite() {
            c.transform(Affine::rotate(angle));
        }
    })
}

fn translate(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 2, "translate")?;
    let [x, y] = nums(args, context)?;
    apply(this, |c| {
        if finite(&[x, y]) {
            c.transform(Affine::translate((x, y)));
        }
    })
}

fn transform(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 6, "transform")?;
    let m: [f64; 6] = nums(args, context)?;
    apply(this, |c| {
        if finite(&m) {
            c.transform(Affine::new(m));
        }
    })
}

fn matrix_object(value: &JsValue, context: &mut Context) -> JsResult<Option<[f64; 6]>> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    let keys = [("a", "m11", 1.0), ("b", "m12", 0.0), ("c", "m21", 0.0)];
    let more = [("d", "m22", 1.0), ("e", "m41", 0.0), ("f", "m42", 0.0)];
    let mut out = [0.0; 6];
    for (slot, (short, long, default)) in out.iter_mut().zip(keys.into_iter().chain(more)) {
        let mut value = object.get(JsString::from(short), context)?;
        if value.is_undefined() {
            value = object.get(JsString::from(long), context)?;
        }
        *slot = if value.is_undefined() {
            default
        } else {
            value.to_number(context)?
        };
    }
    Ok(Some(out))
}

fn set_transform(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let m = if args.len() >= 6 {
        nums::<6>(args, context)?
    } else if let Some(m) = matrix_object(&arg(args, 0), context)? {
        m
    } else if args.is_empty() || arg(args, 0).is_null_or_undefined() {
        [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
    } else {
        required(args, 6, "setTransform")?;
        return Ok(JsValue::undefined());
    };
    apply(this, |c| c.set_transform(Affine::new(m)))
}

fn reset_transform(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    apply(this, |c| c.set_transform(Affine::IDENTITY))
}

fn matrix(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let canvas = canvas_of(&arg(args, 0))?;
    let coeffs = canvas.borrow().state().transform.as_coeffs();
    let values: Vec<JsValue> = coeffs.into_iter().map(JsValue::from).collect();
    Ok(JsArray::from_iter(values, context).into())
}

fn begin_path(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    apply(this, Canvas2D::begin_path)
}

fn close_path(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    apply(this, Canvas2D::close_path)
}

fn move_to(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 2, "moveTo")?;
    let [x, y] = nums(args, context)?;
    apply(this, |c| c.move_to(x, y))
}

fn line_to(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 2, "lineTo")?;
    let [x, y] = nums(args, context)?;
    apply(this, |c| c.line_to(x, y))
}

fn quadratic_curve_to(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    required(args, 4, "quadraticCurveTo")?;
    let [cx, cy, x, y] = nums(args, context)?;
    apply(this, |c| c.quadratic_curve_to(cx, cy, x, y))
}

fn bezier_curve_to(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 6, "bezierCurveTo")?;
    let [a, b, cx, d, x, y] = nums(args, context)?;
    apply(this, |c| c.bezier_curve_to(a, b, cx, d, x, y))
}

fn arc(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 5, "arc")?;
    let [x, y, r, start, end] = nums(args, context)?;
    let ccw = arg(args, 5).to_boolean();
    let ok = canvas_of(this)?.borrow_mut().arc(x, y, r, start, end, ccw);
    if !ok {
        return Err(index_size(
            context,
            &format!("The radius provided ({r}) is negative."),
        ));
    }
    Ok(JsValue::undefined())
}

fn arc_to(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 5, "arcTo")?;
    let [x1, y1, x2, y2, r] = nums(args, context)?;
    let ok = canvas_of(this)?.borrow_mut().arc_to(x1, y1, x2, y2, r);
    if !ok {
        return Err(index_size(
            context,
            &format!("The radius provided ({r}) is negative."),
        ));
    }
    Ok(JsValue::undefined())
}

fn ellipse(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 7, "ellipse")?;
    let [x, y, rx, ry, rotation, start, end] = nums(args, context)?;
    let ccw = arg(args, 7).to_boolean();
    let ok = canvas_of(this)?.borrow_mut().ellipse(
        Point::new(x, y),
        Vec2::new(rx, ry),
        rotation,
        (start, end),
        ccw,
    );
    if !ok {
        let r = if rx < 0.0 { rx } else { ry };
        return Err(index_size(
            context,
            &format!("The radius provided ({r}) is negative."),
        ));
    }
    Ok(JsValue::undefined())
}

fn rect(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 4, "rect")?;
    let [x, y, w, h] = nums(args, context)?;
    apply(this, |c| c.rect(x, y, w, h))
}

fn radius(value: &JsValue, context: &mut Context) -> JsResult<Radii> {
    if let Some(object) = value.as_object() {
        let x = object
            .get(JsString::from("x"), context)?
            .to_number(context)?;
        let y = object
            .get(JsString::from("y"), context)?
            .to_number(context)?;
        let (x, y) = (
            if x.is_nan() { 0.0 } else { x },
            if y.is_nan() { 0.0 } else { y },
        );
        return Ok(Radii { x, y });
    }
    let r = value.to_number(context)?;
    Ok(Radii { x: r, y: r })
}

fn round_rect(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 4, "roundRect")?;
    let [x, y, w, h] = nums(args, context)?;
    let value = arg(args, 4);
    let mut radii = Vec::new();
    if value.is_undefined() {
        radii.push(Radii { x: 0.0, y: 0.0 });
    } else if let Some(array) = value.as_object().filter(|o| o.is_array()) {
        let array = JsArray::from_object(array.clone())?;
        for i in 0..array.length(context)?.min(5) {
            radii.push(radius(&array.get(i, context)?, context)?);
        }
    } else {
        radii.push(radius(&value, context)?);
    }
    if !canvas_of(this)?.borrow_mut().round_rect(x, y, w, h, &radii) {
        return Err(JsNativeError::range()
            .with_message(
                "Failed to execute 'roundRect' on 'CanvasRenderingContext2D': invalid radii.",
            )
            .into());
    }
    Ok(JsValue::undefined())
}

fn rule(args: &[JsValue], context: &mut Context) -> JsResult<FillRule> {
    let value = args.iter().find(|v| v.is_string()).cloned();
    Ok(match value {
        Some(v) if to_rust_string(&v, context)? == "evenodd" => FillRule::EvenOdd,
        _ => FillRule::NonZero,
    })
}

fn fill(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let rule = rule(args, context)?;
    apply(this, |c| c.fill(rule))
}

fn stroke(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    apply(this, Canvas2D::stroke)
}

fn clip(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let rule = rule(args, context)?;
    apply(this, |c| c.clip(rule))
}

fn always_false(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::from(false))
}

fn fill_rect(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 4, "fillRect")?;
    let [x, y, w, h] = nums(args, context)?;
    apply(this, |c| c.fill_rect(x, y, w, h))
}

fn stroke_rect(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 4, "strokeRect")?;
    let [x, y, w, h] = nums(args, context)?;
    apply(this, |c| c.stroke_rect(x, y, w, h))
}

fn clear_rect(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    required(args, 4, "clearRect")?;
    let [x, y, w, h] = nums(args, context)?;
    apply(this, |c| c.clear_rect(x, y, w, h))
}
