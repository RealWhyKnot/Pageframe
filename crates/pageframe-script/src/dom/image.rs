use boa_engine::object::JsObject;
use boa_engine::value::JsValue;
use boa_engine::{Context, JsResult, JsString};
use pageframe_dom::node::{ImageData, SpecialElementData};
use pageframe_dom::{BaseDocument, NodeId};
use url::Url;

use super::element::{read_attr, write_attr};
use super::{define_accessor, dom_ctx, js_str, this_node_id, to_rust_string};
use crate::state::DomCtx;

const SRC_ELEMENTS: &[&str] = &[
    "img", "script", "iframe", "embed", "video", "audio", "source", "track", "input", "frame",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ImageState {
    Pending,
    Loaded(u32, u32),
    Broken,
}

pub(crate) fn init_image_accessors(proto: &JsObject, context: &mut Context) {
    define_accessor(proto, "src", Some(get_src), Some(set_src), context);
    define_accessor(proto, "currentSrc", Some(get_current_src), None, context);
    define_accessor(proto, "complete", Some(get_complete), None, context);
    define_accessor(
        proto,
        "naturalWidth",
        Some(get_natural_width),
        None,
        context,
    );
    define_accessor(
        proto,
        "naturalHeight",
        Some(get_natural_height),
        None,
        context,
    );
}

fn local_name(ctx: &DomCtx, node_id: NodeId) -> Option<String> {
    let doc = ctx.doc.borrow();
    let element = doc.get_node(node_id)?.element_data()?;
    Some(element.name.local.to_string())
}

pub(crate) fn is_image(doc: &BaseDocument, node_id: NodeId) -> bool {
    doc.get_node(node_id)
        .and_then(|node| node.element_data())
        .is_some_and(|element| element.name.local == pageframe_dom::local_name!("img"))
}

pub(crate) fn image_state(doc: &BaseDocument, node_id: NodeId) -> ImageState {
    let Some(element) = doc.get_node(node_id).and_then(|node| node.element_data()) else {
        return ImageState::Broken;
    };
    let has_src = element
        .attr(pageframe_dom::local_name!("src"))
        .is_some_and(|src| !src.is_empty());
    match &element.special_data {
        SpecialElementData::Image(image) => {
            if let ImageData::Raster(raster) = image.as_ref() {
                ImageState::Loaded(raster.width, raster.height)
            } else if matches!(image.as_ref(), ImageData::None) {
                ImageState::Broken
            } else {
                ImageState::Loaded(0, 0)
            }
        }
        SpecialElementData::ImageFallback(fallback) if fallback.loading && has_src => {
            ImageState::Pending
        }
        _ => ImageState::Broken,
    }
}

fn resolve(ctx: &DomCtx, value: &str) -> Option<Url> {
    match ctx.state.borrow().base_url.as_ref() {
        Some(base) => base.join(value).ok(),
        None => Url::parse(value).ok(),
    }
}

fn get_src(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let reflects = local_name(&ctx, node_id).is_some_and(|name| SRC_ELEMENTS.contains(&&*name));
    if !reflects {
        return Ok(JsValue::undefined());
    }
    let Some(src) = read_attr(&ctx, node_id, "src") else {
        return Ok(js_str(""));
    };
    if src.is_empty() {
        return Ok(js_str(""));
    }
    let resolved = resolve(&ctx, &src).map(|url| url.to_string());
    Ok(js_str(&resolved.unwrap_or(src)))
}

fn set_src(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let reflects = local_name(&ctx, node_id).is_some_and(|name| SRC_ELEMENTS.contains(&&*name));
    let value = args.first().cloned().unwrap_or_default();
    if !reflects {
        if let Some(object) = this.as_object() {
            object.create_data_property_or_throw(JsString::from("src"), value, context)?;
        }
        return Ok(JsValue::undefined());
    }
    let value = to_rust_string(&value, context)?;
    write_attr(context, &ctx, node_id, "src", &value);
    Ok(JsValue::undefined())
}

fn img_state(this: &JsValue, context: &mut Context) -> JsResult<Option<ImageState>> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let doc = ctx.doc.borrow();
    if !is_image(&doc, node_id) {
        return Ok(None);
    }
    Ok(Some(image_state(&doc, node_id)))
}

fn get_current_src(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    match img_state(this, context)? {
        Some(ImageState::Loaded(..)) => get_src(this, args, context),
        Some(_) => Ok(js_str("")),
        None => Ok(JsValue::undefined()),
    }
}

fn get_complete(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    Ok(match img_state(this, context)? {
        Some(state) => JsValue::from(state != ImageState::Pending),
        None => JsValue::undefined(),
    })
}

fn natural(this: &JsValue, context: &mut Context, pick: fn(u32, u32) -> u32) -> JsResult<JsValue> {
    Ok(match img_state(this, context)? {
        Some(ImageState::Loaded(width, height)) => JsValue::from(pick(width, height)),
        Some(_) => JsValue::from(0),
        None => JsValue::undefined(),
    })
}

fn get_natural_width(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    natural(this, context, |width, _| width)
}

fn get_natural_height(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    natural(this, context, |_, height| height)
}
