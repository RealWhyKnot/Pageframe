//! Global registration and the ordered JavaScript bootstrap.

use boa_engine::object::ObjectInitializer;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsResult, JsString, JsValue, NativeFunction, js_string};

use super::{css, events, media, net, timers, window};
use crate::dom::node_wrapper;
use crate::state::DomCtx;

pub(super) const GEOMETRY_JS: &str = include_str!("../geometry.js");

// Concatenate into one IIFE to preserve shared lexical bindings, installation
// order, and short-circuiting on errors. Geometry remains a separate evaluation.
pub(super) const BOOTSTRAP_JS: &str = concat!(
    "\n(function () {\n",
    include_str!("bootstrap/url_search_params.js"),
    "\n",
    include_str!("bootstrap/window.js"),
    "\n",
    include_str!("bootstrap/location.js"),
    "\n",
    include_str!("bootstrap/events.js"),
    "\n",
    include_str!("bootstrap/web_apis.js"),
    "\n",
    include_str!("bootstrap/fetch.js"),
    "\n",
    include_str!("bootstrap/event_source.js"),
    "\n",
    include_str!("bootstrap/dom_exception.js"),
    "\n",
    include_str!("bootstrap/style.js"),
    "\n",
    include_str!("bootstrap/interfaces.js"),
    "\n",
    include_str!("bootstrap/element_interfaces.js"),
    "\n",
    include_str!("bootstrap/elements.js"),
    "\n",
    include_str!("bootstrap/observers.js"),
    "\n",
    include_str!("bootstrap/mutation_observer.js"),
    "\n",
    include_str!("bootstrap/ipc.js"),
    "\n",
    include_str!("bootstrap/css.js"),
    "\n",
    include_str!("bootstrap/stylesheets.js"),
    "\n",
    include_str!("bootstrap/fonts.js"),
    "\n",
    include_str!("bootstrap/node_constants.js"),
    "\n",
    include_str!("bootstrap/canvas.js"),
    "})();\n",
);

pub(super) fn register_globals(ctx: &DomCtx, context: &mut Context) {
    register_global_fn(
        context,
        "__pageframe_parse_search_params",
        1,
        parse_search_params,
    );
    // `document`
    let root_id = ctx.doc.borrow().root_node().id;
    let document_wrapper = node_wrapper(ctx, root_id, context);
    register_global(context, "document", document_wrapper.into());
    let mut window_handlers: Vec<&'static str> = crate::dom::ON_EVENT_TYPES.to_vec();
    for name in crate::dom::WINDOW_REFLECTING_BODY_EVENTS {
        if !window_handlers.contains(name) {
            window_handlers.push(name);
        }
    }
    crate::dom::listeners::install_handler_accessors(
        &context.global_object(),
        crate::dom::listeners::HandlerOwner::Window,
        &window_handlers,
        context,
    );

    // `window` and friends (aliases for the global object)
    let global: JsValue = context.global_object().into();
    register_global(context, "window", global.clone());
    register_global(context, "self", global.clone());
    // There is only ever a single frame, so `parent` and `top` refer to the
    // window itself and `opener` is null
    register_global(context, "parent", global.clone());
    register_global(context, "top", global);
    register_global(context, "opener", JsValue::null());

    // `navigator`
    let navigator = ObjectInitializer::new(context)
        .property(
            js_string!("userAgent"),
            js_string!("Mozilla/5.0 (compatible; Pageframe)"),
            Attribute::all(),
        )
        .build();
    register_global(context, "navigator", navigator.into());

    // Timers and window event listeners
    register_global_fn(context, "setTimeout", 2, timers::set_timeout);
    register_global_fn(context, "clearTimeout", 1, timers::clear_timer);
    register_global_fn(context, "setInterval", 2, timers::set_interval);
    register_global_fn(context, "clearInterval", 1, timers::clear_timer);
    register_global_fn(
        context,
        "requestAnimationFrame",
        1,
        timers::request_animation_frame,
    );
    register_global_fn(context, "cancelAnimationFrame", 1, timers::clear_timer);
    register_global_fn(
        context,
        "addEventListener",
        2,
        events::window_add_event_listener,
    );
    register_global_fn(
        context,
        "removeEventListener",
        2,
        events::window_remove_event_listener,
    );

    register_global_fn(
        context,
        "dispatchEvent",
        1,
        crate::dom::dispatch::window_dispatch_event,
    );
    register_global_fn(
        context,
        "__pageframe_new_event",
        4,
        crate::dom::event::new_event,
    );
    register_global_fn(
        context,
        "__pageframe_event_flags",
        1,
        crate::dom::event::event_flags,
    );
    register_global_fn(
        context,
        "__pageframe_performance_now",
        0,
        media::performance_now,
    );
    register_global_fn(context, "__pageframe_document_url", 0, media::document_url);
    register_global_fn(
        context,
        "__pageframe_set_document_url",
        1,
        media::set_document_url,
    );
    register_global_fn(context, "__pageframe_navigate", 1, media::navigate);
    register_global_fn(context, "__pageframe_media_env", 0, media::media_env);
    register_global_fn(context, "__pageframe_random_bytes", 1, media::random_bytes);

    // Embedder message channel (see `ScriptDocument::take_messages`)
    register_global_fn(context, "__pageframe_send_message", 1, window::send_message);
    register_global_fn(
        context,
        "__pageframe_document_hidden",
        0,
        window::document_hidden,
    );
    register_global_fn(
        context,
        "__pageframe_clipboard_write_text",
        1,
        window::clipboard_write_text,
    );
    register_mutation_natives(context);
    register_global_fn(context, "__pageframe_ipc_send", 3, crate::ipc::send);

    register_global_fn(context, "__pageframe_fetch_start", 4, net::fetch_start);
    register_global_fn(
        context,
        "__pageframe_event_source_open",
        2,
        net::event_source_open,
    );
    register_global_fn(
        context,
        "__pageframe_event_source_close",
        1,
        net::event_source_close,
    );

    // CSS property support check, used by the style Proxy's `has` trap
    register_global_fn(
        context,
        "__pageframe_css_property_names",
        0,
        css::css_property_names,
    );
    register_global_fn(
        context,
        "__pageframe_css_property_supported",
        1,
        css::css_property_supported,
    );

    // `getComputedStyle`
    register_global_fn(context, "getComputedStyle", 1, css::get_computed_style);
    register_global_fn(
        context,
        "getSelection",
        0,
        crate::dom::selection::get_selection,
    );
    register_global_fn(
        context,
        "__pageframe_parse_transform",
        1,
        css::parse_transform,
    );

    // CSSOM stylesheet natives (`__pageframe_sheet_*`), used by the bootstrap's
    // `CSSStyleSheet` / `CSSRule` implementation
    crate::dom::stylesheet::register(context);

    // Viewport dimensions
    register_global_accessor(context, "innerWidth", window::inner_width);
    register_global_accessor(context, "innerHeight", window::inner_height);
    register_global_accessor(context, "outerWidth", window::inner_width);
    register_global_accessor(context, "outerHeight", window::inner_height);
    register_global_accessor(context, "devicePixelRatio", window::device_pixel_ratio);

    // Viewport scrolling
    register_global_accessor(context, "scrollX", window::scroll_x);
    register_global_accessor(context, "scrollY", window::scroll_y);
    register_global_accessor(context, "pageXOffset", window::scroll_x);
    register_global_accessor(context, "pageYOffset", window::scroll_y);
    register_global_fn(context, "scroll", 2, window::window_scroll_to);
    register_global_fn(context, "scrollTo", 2, window::window_scroll_to);
    register_global_fn(context, "scrollBy", 2, window::window_scroll_by);

    // The `CSS` namespace object. `CSS.escape` is defined in the JS bootstrap.
    let css_namespace = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(css::css_supports),
            js_string!("supports"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(css::css_register_property),
            js_string!("registerProperty"),
            1,
        )
        .build();
    register_global(context, "CSS", css_namespace.into());
}

fn register_mutation_natives(context: &mut Context) {
    use crate::dom::mutations;
    register_global_fn(
        context,
        "__pageframe_mutation_observing",
        1,
        mutations::set_observing,
    );
    register_global_fn(context, "__pageframe_mutation_begin", 0, mutations::begin);
    register_global_fn(context, "__pageframe_mutation_end", 0, mutations::end);
    register_global_fn(context, "__pageframe_take_mutations", 0, mutations::take);
}

fn parse_search_params(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    use boa_engine::object::builtins::JsArray;

    let input = args[0].to_string(context)?.to_std_string_lossy();
    let pairs = url::form_urlencoded::parse(input.as_bytes())
        .map(|(name, value)| {
            JsArray::from_iter(
                [
                    JsValue::from(JsString::from(name.as_ref())),
                    JsValue::from(JsString::from(value.as_ref())),
                ],
                context,
            )
            .into()
        })
        .collect::<Vec<JsValue>>();
    Ok(JsArray::from_iter(pairs, context).into())
}

fn register_global(context: &mut Context, name: &str, value: JsValue) {
    context
        .register_global_property(
            JsString::from(name),
            value,
            Attribute::WRITABLE.union(Attribute::CONFIGURABLE),
        )
        .expect("failed to register global");
}

fn register_global_accessor(
    context: &mut Context,
    name: &str,
    getter: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
) {
    use boa_engine::object::FunctionObjectBuilder;
    use boa_engine::property::{PropertyDescriptor, PropertyKey};

    let getter_fn =
        FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(getter))
            .name(JsString::from(format!("get {name}")))
            .length(0)
            .build();
    context
        .global_object()
        .define_property_or_throw(
            PropertyKey::from(JsString::from(name)),
            PropertyDescriptor::builder()
                .get(getter_fn)
                .enumerable(false)
                .configurable(true)
                .build(),
            context,
        )
        .expect("failed to register global accessor");
}

fn register_global_fn(
    context: &mut Context,
    name: &str,
    length: usize,
    body: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
) {
    context
        .register_global_callable(
            JsString::from(name),
            length,
            NativeFunction::from_fn_ptr(body),
        )
        .expect("failed to register global function");
}
