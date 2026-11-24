//! Form-control values, text-input selection, and focus.

use super::attributes::{attr_getter, attr_setter, clear_attr, read_attr, store_attr, write_attr};
use crate::dom::{dom_ctx, this_node_id};
use crate::dom::{js_str, to_rust_string};
use boa_engine::{Context, JsResult, JsValue};
use pageframe_dom::NodeId;

pub(super) fn get_placeholder(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = args;
    attr_getter("placeholder", this, context)
}
pub(super) fn set_placeholder(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    attr_setter("placeholder", this, args, context)
}

pub(super) fn get_type(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = args;
    attr_getter("type", this, context)
}
pub(super) fn set_type(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    attr_setter("type", this, args, context)
}

pub(super) fn get_autofocus(
    this: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    Ok(JsValue::from(
        read_attr(&ctx, node_id, "autofocus").is_some(),
    ))
}
pub(super) fn set_autofocus(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let value = args.first().map(JsValue::to_boolean).unwrap_or(false);
    if value {
        write_attr(context, &ctx, node_id, "autofocus", "true");
    } else {
        clear_attr(context, &ctx, node_id, "autofocus");
    }
    Ok(JsValue::undefined())
}

pub(super) fn get_value(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let value = {
        let doc = ctx.doc.borrow();
        if doc.is_select(node_id) {
            return Ok(js_str(&doc.select_value(node_id).unwrap_or_default()));
        }
        if doc.is_option(node_id) {
            return Ok(js_str(&doc.option_value(node_id)));
        }
        doc.get_node(node_id)
            .and_then(|node| node.element_data())
            .map(|element| match element.text_input_data() {
                Some(input_data) => input_data.value().to_string(),
                None if element.is_range_input() => {
                    pageframe_dom::format_input_number(element.range_value())
                }
                None => element
                    .attrs()
                    .iter()
                    .find(|attr| &*attr.name.local == "value")
                    .map(|attr| attr.value.clone())
                    .unwrap_or_default(),
            })
            .unwrap_or_default()
    };
    Ok(js_str(&value))
}

pub(super) fn set_value(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    if ctx.doc.borrow().is_select(node_id) {
        let value = to_rust_string(args.first().unwrap_or(&JsValue::undefined()), context)?;
        ctx.doc.borrow_mut().set_select_value(node_id, &value);
        return Ok(JsValue::undefined());
    }
    if is_range(&ctx, node_id) {
        let value = to_rust_string(args.first().unwrap_or(&JsValue::undefined()), context)?;
        ctx.doc.borrow_mut().set_range_input_text(node_id, &value);
        return Ok(JsValue::undefined());
    }
    attr_setter("value", this, args, context)
}

fn is_range(ctx: &crate::state::DomCtx, node_id: NodeId) -> bool {
    ctx.doc
        .borrow()
        .get_node(node_id)
        .and_then(|node| node.element_data())
        .is_some_and(pageframe_dom::ElementData::is_range_input)
}

fn input_kind(ctx: &crate::state::DomCtx, node_id: NodeId) -> Option<String> {
    let doc = ctx.doc.borrow();
    let element = doc.get_node(node_id)?.element_data()?;
    if element.name.local != pageframe_dom::local_name!("input") {
        return None;
    }
    Some(
        element
            .input_type()
            .map_or_else(String::new, |kind| kind.into_owned()),
    )
}

pub(super) fn get_value_as_number(
    this: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let number = match input_kind(&ctx, node_id).as_deref() {
        Some("range") => {
            let doc = ctx.doc.borrow();
            doc.get_node(node_id)
                .and_then(|node| node.element_data())
                .map_or(f64::NAN, pageframe_dom::ElementData::range_value)
        }
        Some("number") => {
            let text = get_value(this, &[], context)?.to_string(context)?;
            let text = text.to_std_string_lossy();
            if text.trim().is_empty() {
                f64::NAN
            } else {
                js_str(&text).to_number(context)?
            }
        }
        _ => f64::NAN,
    };
    Ok(JsValue::from(number))
}

pub(super) fn set_value_as_number(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let number = args
        .first()
        .cloned()
        .unwrap_or_default()
        .to_number(context)?;
    if number.is_infinite() {
        return Err(boa_engine::JsNativeError::typ()
            .with_message(
                "Failed to set the 'valueAsNumber' property on 'HTMLInputElement': The value provided is infinite.",
            )
            .into());
    }
    match input_kind(&ctx, node_id).as_deref() {
        Some("range") => {
            ctx.doc.borrow_mut().set_range_input_value(node_id, number);
        }
        Some("number") => {
            let text = if number.is_nan() {
                String::new()
            } else {
                pageframe_dom::format_input_number(number)
            };
            write_attr(context, &ctx, node_id, "value", &text);
        }
        _ => {}
    }
    Ok(JsValue::undefined())
}

pub(super) fn get_checked(
    this: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let checked = {
        let doc = ctx.doc.borrow();
        doc.get_node(node_id)
            .and_then(|node| node.element_data())
            .map(|element| {
                element.checkbox_input_checked().unwrap_or_else(|| {
                    element
                        .attr(pageframe_dom::local_name!("checked"))
                        .is_some()
                })
            })
            .unwrap_or(false)
    };
    Ok(JsValue::from(checked))
}

pub(super) fn set_checked(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let checked = args.first().map(JsValue::to_boolean).unwrap_or(false);
    // pageframe-dom's checked handling parses the value as a boolean
    store_attr(
        &ctx,
        node_id,
        "checked",
        if checked { "true" } else { "false" },
    );
    Ok(JsValue::undefined())
}

// === Text input selection (`selectionStart`/`selectionEnd`) ===
//
// Backed by the text input's real editor selection. Offsets are in UTF-16 code
// units, per spec. Returns null for elements without a text editor. React
// snapshots these around controlled-input re-renders and restores the caret
// via `setSelectionRange()`.

/// Convert a byte offset in `text` to a UTF-16 code-unit offset
fn byte_to_utf16(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset.min(text.len())]
        .chars()
        .map(char::len_utf16)
        .sum()
}

/// Convert a UTF-16 code-unit offset to a byte offset in `text` (clamped)
fn utf16_to_byte(text: &str, utf16_offset: usize) -> usize {
    let mut units = 0;
    for (byte_offset, ch) in text.char_indices() {
        if units >= utf16_offset {
            return byte_offset;
        }
        units += ch.len_utf16();
    }
    text.len()
}

/// The current selection of the node's text editor as UTF-16 offsets
fn selection_utf16_range(
    doc: &pageframe_dom::BaseDocument,
    node_id: NodeId,
) -> Option<(usize, usize)> {
    let input = doc.get_node(node_id)?.element_data()?.text_input_data()?;
    let text = input.editor.raw_text();
    let value = input.value();
    let range = input.editor.raw_selection().text_range();
    let to_utf16 = |byte: usize| {
        let chars = text[..byte.min(text.len())].chars().count();
        let value_byte = value
            .char_indices()
            .nth(chars)
            .map_or(value.len(), |(b, _)| b);
        byte_to_utf16(value, value_byte)
    };
    Some((to_utf16(range.start), to_utf16(range.end)))
}

/// Set the node's text editor selection from UTF-16 offsets
fn set_selection_utf16_range(
    doc: &mut pageframe_dom::BaseDocument,
    node_id: NodeId,
    start: usize,
    end: usize,
) {
    let Some(value) = doc
        .get_node(node_id)
        .and_then(|node| node.element_data())
        .and_then(|element| element.text_input_data())
        .map(|input| input.value().to_string())
    else {
        return;
    };
    doc.with_text_input(node_id, |mut driver| {
        let text = driver.editor.raw_text().to_string();
        let to_byte = |utf16: usize| {
            let chars = value[..utf16_to_byte(&value, utf16)].chars().count();
            text.char_indices()
                .nth(chars)
                .map_or(text.len(), |(b, _)| b)
        };
        let start_byte = to_byte(start);
        let end_byte = to_byte(start.max(end));
        driver.select_byte_range(start_byte, end_byte);
    });
}

pub(super) fn get_selection_start(
    this: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let mut doc = ctx.doc.borrow_mut();
    // The text editor is created during layout construction
    doc.resolve(0.0);
    Ok(match selection_utf16_range(&doc, node_id) {
        Some((start, _)) => JsValue::from(start as u32),
        None => JsValue::null(),
    })
}

pub(super) fn get_selection_end(
    this: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let mut doc = ctx.doc.borrow_mut();
    // The text editor is created during layout construction
    doc.resolve(0.0);
    Ok(match selection_utf16_range(&doc, node_id) {
        Some((_, end)) => JsValue::from(end as u32),
        None => JsValue::null(),
    })
}

pub(super) fn set_selection_start(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let start = args
        .first()
        .unwrap_or(&JsValue::undefined())
        .to_number(context)?
        .max(0.0) as usize;
    let mut doc = ctx.doc.borrow_mut();
    doc.resolve(0.0);
    if let Some((_, end)) = selection_utf16_range(&doc, node_id) {
        set_selection_utf16_range(&mut doc, node_id, start, end.max(start));
    }
    Ok(JsValue::undefined())
}

pub(super) fn set_selection_end(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let end = args
        .first()
        .unwrap_or(&JsValue::undefined())
        .to_number(context)?
        .max(0.0) as usize;
    let mut doc = ctx.doc.borrow_mut();
    doc.resolve(0.0);
    if let Some((start, _)) = selection_utf16_range(&doc, node_id) {
        set_selection_utf16_range(&mut doc, node_id, start.min(end), end);
    }
    Ok(JsValue::undefined())
}

pub(super) fn set_selection_range(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    let start = args
        .first()
        .unwrap_or(&JsValue::undefined())
        .to_number(context)?
        .max(0.0) as usize;
    let end = args
        .get(1)
        .unwrap_or(&JsValue::undefined())
        .to_number(context)?
        .max(0.0) as usize;
    let mut doc = ctx.doc.borrow_mut();
    doc.resolve(0.0);
    set_selection_utf16_range(&mut doc, node_id, start, end);
    Ok(JsValue::undefined())
}

// === Focus ===

pub(super) fn focus(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let node_id = this_node_id(this)?;
    ctx.doc.borrow_mut().set_focus_to(node_id);
    Ok(JsValue::undefined())
}

pub(super) fn blur(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let _ = this_node_id(this)?;
    ctx.doc.borrow_mut().clear_focus();
    Ok(JsValue::undefined())
}
