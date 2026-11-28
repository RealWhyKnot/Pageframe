//! Runtime construction, script evaluation, and microtask execution.
//!
//! API installation lives in `bootstrap`; event dispatch, timers, module
//! loading, and window/CSS/fetch bindings have separate implementations.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{Context, Finalize, JsResult, Source, Trace};
use boa_runtime::Console;
use boa_runtime::console::{ConsoleState, Logger};
use pageframe_dom::BaseDocument;
use url::Url;
use web_time::{Duration, Instant};

use crate::dom::dom_ctx;
use crate::fetch::ScriptFetcher;
use crate::state::{DomCtx, ReadyState};
use bootstrap::{BOOTSTRAP_JS, GEOMETRY_JS};
use events::report_js_error;
use modules::PageframeModuleLoader;

mod bootstrap;
mod css;
pub(crate) mod events;
mod media;
mod modules;
mod net;
mod timers;
mod window;

const JS_LOG_BURST: u8 = 5;
const JS_LOG_WINDOW: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(crate) struct JsLogLimiter {
    resets_at: Option<Instant>,
    emitted: u8,
    suppressed: u64,
}

impl JsLogLimiter {
    fn admit(&mut self, now: Instant) -> Option<u64> {
        if self.resets_at.is_none_or(|deadline| now >= deadline) {
            self.resets_at = Some(now + JS_LOG_WINDOW);
            self.emitted = 0;
        }
        if self.emitted == JS_LOG_BURST {
            self.suppressed = self.suppressed.saturating_add(1);
            return None;
        }
        self.emitted += 1;
        Some(std::mem::take(&mut self.suppressed))
    }
}

#[derive(Debug, Trace, Finalize)]
struct TracingLogger;

impl Logger for TracingLogger {
    fn log(&self, msg: String, _state: &ConsoleState, _context: &mut Context) -> JsResult<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: "js_console", "{msg}");
        #[cfg(not(feature = "tracing"))]
        drop(msg);
        Ok(())
    }

    fn info(&self, msg: String, state: &ConsoleState, context: &mut Context) -> JsResult<()> {
        self.log(msg, state, context)
    }

    fn warn(&self, msg: String, _: &ConsoleState, context: &mut Context) -> JsResult<()> {
        let ctx = dom_ctx(context)?;
        let admitted = ctx.state.borrow_mut().js_logs.admit(Instant::now());
        if let Some(suppressed) = admitted {
            tracing::warn!(target: "js_console", suppressed, "{msg}");
        }
        Ok(())
    }

    fn error(&self, msg: String, _: &ConsoleState, context: &mut Context) -> JsResult<()> {
        let ctx = dom_ctx(context)?;
        let admitted = ctx.state.borrow_mut().js_logs.admit(Instant::now());
        if let Some(suppressed) = admitted {
            tracing::error!(target: "js_console", suppressed, "{msg}");
        }
        Ok(())
    }
}

pub(crate) struct ScriptRuntime {
    pub context: Context,
    pub ctx: DomCtx,
    module_loader: Rc<PageframeModuleLoader>,
}

impl ScriptRuntime {
    pub fn new(
        doc: Rc<RefCell<BaseDocument>>,
        base_url: Option<&Url>,
        fetcher: Rc<RefCell<Box<dyn ScriptFetcher>>>,
    ) -> Self {
        let module_loader = Rc::new(PageframeModuleLoader::new(
            Rc::clone(&fetcher),
            base_url.cloned(),
        ));
        let ctx = DomCtx::new(doc);
        {
            let mut state = ctx.state.borrow_mut();
            state.base_url = base_url.cloned();
            state.fetcher = Some(fetcher);
            state.time_origin = Some(state.clock.now());
        }
        // Share the runtime's clock with boa so that `Date` observes the same
        // (possibly virtual) time as timers
        let clock = ctx.state.borrow().clock.clone();
        let mut context = Context::builder()
            .module_loader(module_loader.clone())
            .clock(Rc::new(crate::clock::BoaClockAdapter::new(clock)))
            .build()
            .expect("failed to build JS context");
        context.insert_data(ctx.clone());

        Console::register_with_logger(TracingLogger, &mut context)
            .expect("failed to register console");

        // Register boa_runtime's web-API extensions: atob/btoa, TextEncoder/
        // TextDecoder, structuredClone, queueMicrotask and URL.
        //
        // Deliberately NOT registered:
        // - TimeoutExtension: pageframe-script has its own setTimeout/setInterval/
        //   requestAnimationFrame implementation integrated with the document's
        //   event loop and timer thread
        // - FetchExtension/AbortControllerExtension: fetch should go through
        //   the embedder's net provider, not an internal HTTP client
        boa_runtime::register_extensions(
            (
                boa_runtime::extensions::Base64Extension,
                boa_runtime::extensions::EncodingExtension,
                boa_runtime::extensions::StructuredCloneExtension,
                boa_runtime::extensions::MicrotaskExtension,
                boa_runtime::extensions::UrlExtension,
            ),
            None,
            &mut context,
        )
        .expect("failed to register boa_runtime extensions");

        crate::dom::init_protos(&ctx, &mut context);

        bootstrap::register_globals(&ctx, &mut context);
        crate::canvas::init(&ctx, &mut context);
        crate::media::init(&mut context);

        let mut runtime = Self {
            context,
            ctx,
            module_loader,
        };

        // Install JavaScript APIs after the native bindings.
        runtime.eval_internal(BOOTSTRAP_JS, "<pageframe-bootstrap>");
        runtime.eval_internal(GEOMETRY_JS, "<pageframe-geometry>");

        runtime
    }

    /// Evaluate a script, logging (but not propagating) any uncaught errors,
    /// then drain the microtask queue.
    pub fn eval(&mut self, code: &str, description: &str) {
        self.eval_internal(code, description);
        self.run_jobs(description);
    }

    fn eval_internal(&mut self, code: &str, description: &str) {
        if let Err(error) = self.context.eval(Source::from_bytes(code)) {
            report_js_error(&self.ctx, &mut self.context, description, &error);
        }
    }

    /// Run pending promise jobs (microtasks)
    pub fn run_jobs(&mut self, description: &str) {
        if let Err(error) = self.context.run_jobs() {
            report_js_error(&self.ctx, &mut self.context, description, &error);
        }
    }

    /// Set the value exposed as `document.readyState`
    pub fn set_ready_state(&mut self, ready_state: ReadyState) {
        self.ctx.state.borrow_mut().ready_state = ready_state;
    }
}

#[cfg(test)]
mod tests {
    use super::{Duration, Instant, JS_LOG_BURST, JS_LOG_WINDOW, JsLogLimiter};

    #[test]
    fn javascript_logs_cap_bursts_and_report_suppressed_messages() {
        let start = Instant::now();
        let mut limiter = JsLogLimiter::default();
        for _ in 0..JS_LOG_BURST {
            assert_eq!(limiter.admit(start), Some(0));
        }
        for _ in 0..100 {
            assert_eq!(limiter.admit(start + Duration::from_millis(999)), None);
        }
        assert_eq!(limiter.admit(start + JS_LOG_WINDOW), Some(100));
        assert_eq!(limiter.admit(start + JS_LOG_WINDOW), Some(0));
    }
}
