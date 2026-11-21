use boa_engine::object::builtins::JsArray;
use boa_engine::{Context, JsResult, JsValue};

use crate::dom::{dom_ctx, node_wrapper, this_node_id};

pub(super) fn get_options(
    this: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let options = {
        let doc = ctx.doc.borrow();
        if !doc.is_select(node_id) {
            return Ok(JsValue::undefined());
        }
        doc.select_options(node_id)
    };
    let wrappers: Vec<JsValue> = options
        .into_iter()
        .map(|option| node_wrapper(&ctx, option, context).into())
        .collect();
    Ok(JsArray::from_iter(wrappers, context).into())
}

pub(super) fn get_selected_index(
    this: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let doc = ctx.doc.borrow();
    if !doc.is_select(node_id) {
        return Ok(JsValue::undefined());
    }
    let index = doc
        .selected_index(node_id)
        .and_then(|index| i32::try_from(index).ok())
        .unwrap_or(-1);
    Ok(JsValue::from(index))
}

pub(super) fn set_selected_index(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let index = args.first().cloned().unwrap_or_default().to_i32(context)?;
    let mut doc = ctx.doc.borrow_mut();
    if doc.is_select(node_id) {
        doc.set_selected_index(node_id, usize::try_from(index).ok());
    }
    Ok(JsValue::undefined())
}
