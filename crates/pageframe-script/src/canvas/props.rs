use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::property::{PropertyDescriptor, PropertyKey};
use boa_engine::value::JsValue;
use boa_engine::{Context, JsResult, JsString};
use pageframe_dom::canvas::{
    Canvas2D, DrawState, FontSpec, Paint, Rgba, SmoothingQuality, TextAlign, TextBaseline,
};
use pageframe_dom::canvas::{Cap, Join};

use super::{GradientData, canvas_of, wrap_gradient};
use crate::dom::{NativeFnPtr, define_accessor, dom_ctx, js_str, to_rust_string};

const TEXT_ALIGNS: [(&str, TextAlign); 5] = [
    ("start", TextAlign::Start),
    ("end", TextAlign::End),
    ("left", TextAlign::Left),
    ("right", TextAlign::Right),
    ("center", TextAlign::Center),
];

const TEXT_BASELINES: [(&str, TextBaseline); 6] = [
    ("top", TextBaseline::Top),
    ("hanging", TextBaseline::Hanging),
    ("middle", TextBaseline::Middle),
    ("alphabetic", TextBaseline::Alphabetic),
    ("ideographic", TextBaseline::Ideographic),
    ("bottom", TextBaseline::Bottom),
];

const CAPS: [(&str, Cap); 3] = [
    ("butt", Cap::Butt),
    ("round", Cap::Round),
    ("square", Cap::Square),
];

const JOINS: [(&str, Join); 3] = [
    ("round", Join::Round),
    ("bevel", Join::Bevel),
    ("miter", Join::Miter),
];

const QUALITIES: [(&str, SmoothingQuality); 3] = [
    ("low", SmoothingQuality::Low),
    ("medium", SmoothingQuality::Medium),
    ("high", SmoothingQuality::High),
];

const FIXED: [(&str, &str); 7] = [
    ("direction", "inherit"),
    ("filter", "none"),
    ("letterSpacing", "0px"),
    ("wordSpacing", "0px"),
    ("fontKerning", "auto"),
    ("fontStretch", "normal"),
    ("fontVariantCaps", "normal"),
];

pub(super) fn init_accessors(proto: &JsObject, context: &mut Context) {
    let table: [(&str, NativeFnPtr, NativeFnPtr); 18] = [
        ("fillStyle", get_fill, set_fill),
        ("strokeStyle", get_stroke, set_stroke),
        ("globalAlpha", get_alpha, set_alpha),
        ("globalCompositeOperation", get_composite, set_composite),
        ("lineWidth", get_line_width, set_line_width),
        ("lineCap", get_line_cap, set_line_cap),
        ("lineJoin", get_line_join, set_line_join),
        ("miterLimit", get_miter, set_miter),
        ("lineDashOffset", get_dash_offset, set_dash_offset),
        ("font", get_font, set_font),
        ("textAlign", get_text_align, set_text_align),
        ("textBaseline", get_text_baseline, set_text_baseline),
        ("shadowColor", get_shadow_color, set_shadow_color),
        ("shadowBlur", get_shadow_blur, set_shadow_blur),
        ("shadowOffsetX", get_shadow_x, set_shadow_x),
        ("shadowOffsetY", get_shadow_y, set_shadow_y),
        ("imageSmoothingEnabled", get_smoothing, set_smoothing),
        ("imageSmoothingQuality", get_quality, set_quality),
    ];
    for (name, getter, setter) in table {
        define_accessor(proto, name, Some(getter), Some(setter), context);
    }
    for (name, value) in FIXED {
        let descriptor = PropertyDescriptor::builder()
            .value(js_str(value))
            .writable(true)
            .enumerable(true)
            .configurable(true)
            .build();
        let key = PropertyKey::from(JsString::from(name));
        let _ = proto.define_property_or_throw(key, descriptor, context);
    }
}

fn read<R>(this: &JsValue, f: impl FnOnce(&DrawState) -> R) -> JsResult<R> {
    let canvas = canvas_of(this)?;
    let canvas = canvas.borrow();
    Ok(f(canvas.state()))
}

fn write(this: &JsValue, f: impl FnOnce(&mut Canvas2D)) -> JsResult<JsValue> {
    let canvas = canvas_of(this)?;
    f(&mut canvas.borrow_mut());
    Ok(JsValue::undefined())
}

fn number(args: &[JsValue], context: &mut Context) -> JsResult<f64> {
    args.first().cloned().unwrap_or_default().to_number(context)
}

fn string(args: &[JsValue], context: &mut Context) -> JsResult<String> {
    to_rust_string(&args.first().cloned().unwrap_or_default(), context)
}

fn name_of<T: PartialEq + Copy>(table: &[(&'static str, T)], value: T) -> JsValue {
    js_str(
        table
            .iter()
            .find(|(_, v)| *v == value)
            .map_or("", |(n, _)| n),
    )
}

fn lookup<T: Copy>(table: &[(&str, T)], name: &str) -> Option<T> {
    table.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

fn paint_value(paint: &Paint, context: &mut Context) -> JsResult<JsValue> {
    Ok(match paint {
        Paint::Color(color) => js_str(&color.serialize()),
        Paint::Gradient(gradient) => {
            let ctx = dom_ctx(context)?;
            wrap_gradient(&ctx, Rc::clone(gradient), context)
        }
    })
}

fn paint_arg(args: &[JsValue], context: &mut Context) -> JsResult<Option<Paint>> {
    let value = args.first().cloned().unwrap_or_default();
    if let Some(object) = value.as_object() {
        if let Some(data) = object.downcast_ref::<GradientData>() {
            return Ok(Some(Paint::Gradient(Rc::clone(&data.gradient))));
        }
    }
    let text = to_rust_string(&value, context)?;
    Ok(Rgba::parse(&text).map(Paint::Color))
}

fn get_fill(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let paint = read(this, |s| s.fill.clone())?;
    paint_value(&paint, context)
}

fn set_fill(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let paint = paint_arg(args, context)?;
    write(this, |c| {
        if let Some(paint) = paint {
            c.state_mut().fill = paint;
        }
    })
}

fn get_stroke(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let paint = read(this, |s| s.stroke.clone())?;
    paint_value(&paint, context)
}

fn set_stroke(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let paint = paint_arg(args, context)?;
    write(this, |c| {
        if let Some(paint) = paint {
            c.state_mut().stroke = paint;
        }
    })
}

fn set_number(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
    valid: fn(f64) -> bool,
    apply: fn(&mut DrawState, f64),
) -> JsResult<JsValue> {
    let value = number(args, context)?;
    write(this, |c| {
        if value.is_finite() && valid(value) {
            apply(c.state_mut(), value);
        }
    })
}

fn get_alpha(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| JsValue::from(s.global_alpha))
}

fn set_alpha(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    set_number(
        this,
        args,
        context,
        |v| (0.0..=1.0).contains(&v),
        |s, v| s.global_alpha = v,
    )
}

fn get_composite(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| js_str(s.composite_name))
}

fn set_composite(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = string(args, context)?;
    write(this, |c| {
        c.set_composite(&name);
    })
}

fn get_line_width(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| JsValue::from(s.line_width))
}

fn set_line_width(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    set_number(this, args, context, |v| v > 0.0, |s, v| s.line_width = v)
}

fn get_line_cap(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| name_of(&CAPS, s.line_cap))
}

fn set_line_cap(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let value = lookup(&CAPS, &string(args, context)?);
    write(this, |c| {
        if let Some(cap) = value {
            c.state_mut().line_cap = cap;
        }
    })
}

fn get_line_join(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| name_of(&JOINS, s.line_join))
}

fn set_line_join(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let value = lookup(&JOINS, &string(args, context)?);
    write(this, |c| {
        if let Some(join) = value {
            c.state_mut().line_join = join;
        }
    })
}

fn get_miter(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| JsValue::from(s.miter_limit))
}

fn set_miter(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    set_number(this, args, context, |v| v > 0.0, |s, v| s.miter_limit = v)
}

fn get_dash_offset(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| JsValue::from(s.dash_offset))
}

fn set_dash_offset(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    set_number(this, args, context, |_| true, |s, v| s.dash_offset = v)
}

fn get_font(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| js_str(&s.font.serialize()))
}

fn set_font(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let spec = FontSpec::parse(&string(args, context)?);
    write(this, |c| {
        if let Some(spec) = spec {
            c.state_mut().font = spec;
        }
    })
}

fn get_text_align(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| name_of(&TEXT_ALIGNS, s.text_align))
}

fn set_text_align(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let value = lookup(&TEXT_ALIGNS, &string(args, context)?);
    write(this, |c| {
        if let Some(align) = value {
            c.state_mut().text_align = align;
        }
    })
}

fn get_text_baseline(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| name_of(&TEXT_BASELINES, s.text_baseline))
}

fn set_text_baseline(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let value = lookup(&TEXT_BASELINES, &string(args, context)?);
    write(this, |c| {
        if let Some(baseline) = value {
            c.state_mut().text_baseline = baseline;
        }
    })
}

fn get_shadow_color(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| js_str(&s.shadow_color.serialize()))
}

fn set_shadow_color(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let color = Rgba::parse(&string(args, context)?);
    write(this, |c| {
        if let Some(color) = color {
            c.state_mut().shadow_color = color;
        }
    })
}

fn get_shadow_blur(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| JsValue::from(s.shadow_blur))
}

fn set_shadow_blur(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    set_number(this, args, context, |v| v >= 0.0, |s, v| s.shadow_blur = v)
}

fn get_shadow_x(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| JsValue::from(s.shadow_offset_x))
}

fn set_shadow_x(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    set_number(this, args, context, |_| true, |s, v| s.shadow_offset_x = v)
}

fn get_shadow_y(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| JsValue::from(s.shadow_offset_y))
}

fn set_shadow_y(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    set_number(this, args, context, |_| true, |s, v| s.shadow_offset_y = v)
}

fn get_smoothing(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| JsValue::from(s.image_smoothing))
}

fn set_smoothing(this: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    let value = args.first().is_some_and(JsValue::to_boolean);
    write(this, |c| c.state_mut().image_smoothing = value)
}

fn get_quality(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    read(this, |s| name_of(&QUALITIES, s.smoothing_quality))
}

fn set_quality(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let value = lookup(&QUALITIES, &string(args, context)?);
    write(this, |c| {
        if let Some(quality) = value {
            c.state_mut().smoothing_quality = quality;
        }
    })
}
