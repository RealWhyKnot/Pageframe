//! Window location, named elements, viewport, scrolling, and embedder messages.

use boa_engine::{Context, JsResult, JsString, JsValue};
use pageframe_dom::NodeId;

use super::ScriptRuntime;
use crate::dom::{dom_ctx, node_wrapper};

impl ScriptRuntime {
    pub fn set_hidden(&mut self, hidden: bool) -> bool {
        let changed = self.ctx.doc.borrow_mut().set_page_hidden(hidden);
        if changed {
            crate::ipc::dispatch_document_event(self, "visibilitychange");
        }
        changed
    }

    /// HTML named element access: expose elements with an `id` attribute as
    /// properties of the global (`window`) object, without overriding existing
    /// globals. Should be called before evaluating scripts so that newly
    /// parsed/created ids are visible.
    pub fn sync_named_element_globals(&mut self) {
        use boa_engine::property::{PropertyDescriptor, PropertyKey};

        let ids: Vec<(String, NodeId)> = {
            let doc = self.ctx.doc.borrow();
            let mut ids = Vec::new();
            let mut stack = vec![doc.root_node().id];
            while let Some(node_id) = stack.pop() {
                let Some(node) = doc.get_node(node_id) else {
                    continue;
                };
                if let Some(id) = node
                    .element_data()
                    .and_then(|element| element.attr(pageframe_dom::local_name!("id")))
                {
                    if !id.is_empty() {
                        ids.push((id.to_string(), node_id));
                    }
                }
                stack.extend(node.children.iter().rev().copied());
            }
            ids
        };

        let global = self.context.global_object();
        for (id, node_id) in ids {
            let key = PropertyKey::from(JsString::from(id));
            let already_defined = global
                .has_own_property(key.clone(), &mut self.context)
                .unwrap_or(true);
            if already_defined {
                continue;
            }
            let wrapper = node_wrapper(&self.ctx, node_id, &mut self.context);
            let _ = global.define_property_or_throw(
                key,
                PropertyDescriptor::builder()
                    .value(wrapper)
                    .writable(true)
                    .enumerable(false)
                    .configurable(true)
                    .build(),
                &mut self.context,
            );
        }
    }
}

// === Viewport dimensions ===

pub(super) fn inner_width(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let doc = ctx.doc.borrow();
    let viewport = doc.viewport();
    Ok(JsValue::from(
        viewport.window_size.0 as f64 / viewport.scale() as f64,
    ))
}

pub(super) fn inner_height(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let doc = ctx.doc.borrow();
    let viewport = doc.viewport();
    Ok(JsValue::from(
        viewport.window_size.1 as f64 / viewport.scale() as f64,
    ))
}

pub(super) fn device_pixel_ratio(
    _: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let scale = ctx.doc.borrow().viewport().scale();
    Ok(JsValue::from(scale as f64))
}

// === Viewport scrolling ===

pub(super) fn scroll_x(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    Ok(JsValue::from(ctx.doc.borrow().viewport_scroll().x))
}

pub(super) fn scroll_y(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    Ok(JsValue::from(ctx.doc.borrow().viewport_scroll().y))
}

/// `window.scrollTo`/`window.scroll`: scroll the viewport, which pageframe-dom
/// models as a programmatic scroll of the root element.
pub(super) fn window_scroll_to(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parsed = crate::dom::element::parse_scroll_to_args(args, context)?;
    let ctx = dom_ctx(context)?;
    let mut doc = ctx.doc.borrow_mut();
    doc.resolve(0.0);
    let Some(root_id) = doc.try_root_element().map(|root| root.id) else {
        return Ok(JsValue::undefined());
    };
    let current = doc.viewport_scroll();
    doc.scroll_to(
        root_id,
        parsed.left.unwrap_or(current.x),
        parsed.top.unwrap_or(current.y),
        parsed.behavior,
    );
    Ok(JsValue::undefined())
}

pub(super) fn window_scroll_by(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parsed = crate::dom::element::parse_scroll_to_args(args, context)?;
    let ctx = dom_ctx(context)?;
    let mut doc = ctx.doc.borrow_mut();
    doc.resolve(0.0);
    let Some(root_id) = doc.try_root_element().map(|root| root.id) else {
        return Ok(JsValue::undefined());
    };
    doc.scroll_by(
        root_id,
        parsed.left.unwrap_or(0.0),
        parsed.top.unwrap_or(0.0),
        parsed.behavior,
    );
    Ok(JsValue::undefined())
}

pub(super) fn document_hidden(
    _: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let hidden = ctx.doc.borrow().is_page_hidden();
    Ok(hidden.into())
}

pub(super) fn clipboard_write_text(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let text = args
        .first()
        .unwrap_or(&JsValue::undefined())
        .to_string(context)?
        .to_std_string_lossy();
    let shell = std::sync::Arc::clone(&ctx.doc.borrow().shell_provider);
    Ok(shell.set_clipboard_text(text).is_ok().into())
}

pub(super) fn send_message(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let message = args
        .first()
        .unwrap_or(&JsValue::undefined())
        .to_string(context)?
        .to_std_string_lossy();
    ctx.state.borrow_mut().outbound_messages.push(message);
    Ok(JsValue::undefined())
}
