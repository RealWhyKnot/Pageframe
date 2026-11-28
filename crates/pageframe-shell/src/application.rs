use crate::event::{PageframeShellEvent, PageframeShellProxy};

use anyrender::WindowRenderer;
use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::WindowId;

use winit::{
    data_transfer::{DataTransferSendBuilder, TypeHint},
    event_loop::AsyncRequestSerial,
};

#[cfg(target_os = "macos")]
use winit::platform::macos::ApplicationHandlerExtMacOS;

use crate::{View, WindowConfig};

pub struct PageframeApplication<Rend: WindowRenderer> {
    pub windows: HashMap<WindowId, View<Rend>>,
    pub pending_windows: Vec<WindowConfig<Rend>>,
    pub proxy: PageframeShellProxy,
    pub event_queue: Receiver<PageframeShellEvent>,
    pending_clipboard: HashMap<AsyncRequestSerial, WindowId>,
    #[cfg(feature = "devtools")]
    pub(crate) devtools: crate::devtools::DevtoolsState,
}

/// Whether a document or (recursively) any of its sub-documents has the
/// given id
fn document_contains_id(doc: &pageframe_dom::BaseDocument, doc_id: usize) -> bool {
    if doc.id() == doc_id {
        return true;
    }
    doc.sub_document_node_ids().iter().any(|&node_id| {
        doc.get_node(node_id)
            .and_then(|node| node.subdoc())
            .is_some_and(|sub_doc| document_contains_id(&sub_doc.inner(), doc_id))
    })
}

impl<Rend: WindowRenderer + 'static> PageframeApplication<Rend> {
    pub fn new(proxy: PageframeShellProxy, event_queue: Receiver<PageframeShellEvent>) -> Self {
        #[allow(unused_mut)]
        let mut app = PageframeApplication {
            windows: HashMap::new(),
            pending_windows: Vec::new(),
            proxy,
            event_queue,
            pending_clipboard: HashMap::new(),
            #[cfg(feature = "devtools")]
            devtools: Default::default(),
        };

        #[cfg(feature = "devtools")]
        app.start_cdp_server_from_env();

        app
    }

    pub fn add_window(&mut self, window_config: WindowConfig<Rend>) {
        self.pending_windows.push(window_config);
    }

    fn window_mut_by_doc_id(&mut self, doc_id: usize) -> Option<&mut View<Rend>> {
        self.windows
            .values_mut()
            .find(|w| document_contains_id(&w.doc.inner(), doc_id))
    }

    pub fn handle_pageframe_shell_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        event: PageframeShellEvent,
    ) {
        match event {
            PageframeShellEvent::SetClipboardText { text } => {
                let data = DataTransferSendBuilder::new(text)
                    .with_type(TypeHint::Plaintext, |text, _| Some(text.clone()))
                    .build();
                if let Err(_error) = event_loop.set_clipboard(data) {
                    #[cfg(feature = "tracing")]
                    tracing::error!("Failed to set clipboard: {_error}");
                }
            }
            PageframeShellEvent::RequestClipboardText { window_id } => {
                if let Ok(Some(id)) = event_loop.clipboard()
                    && let Ok(serial) = event_loop.fetch_data_transfer(id, &TypeHint::Plaintext)
                {
                    self.pending_clipboard.insert(serial, window_id);
                }
            }
            PageframeShellEvent::Poll { window_id } => {
                if let Some(window) = self.windows.get_mut(&window_id) {
                    window.poll();
                };
            }
            PageframeShellEvent::CloseWindow { window_id } => {
                // Drop window before exiting event loop
                // See https://github.com/rust-windowing/winit/issues/4135
                let window = self.windows.remove(&window_id);
                drop(window);
                if self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            PageframeShellEvent::ResumeReady { window_id } => {
                if let Some(window) = self.windows.get_mut(&window_id) {
                    window.complete_resume();
                }
            }
            PageframeShellEvent::RequestRedraw { doc_id } => {
                // TODO: Handle multiple documents per window
                if let Some(window) = self.window_mut_by_doc_id(doc_id) {
                    window.request_redraw();
                }
            }

            #[cfg(feature = "accessibility")]
            PageframeShellEvent::Accessibility { window_id, data } => {
                if let Some(window) = self.windows.get_mut(&window_id) {
                    match &*data {
                        accesskit_xplat::WindowEvent::InitialTreeRequested => {
                            window.build_accessibility_tree();
                        }
                        accesskit_xplat::WindowEvent::AccessibilityDeactivated => {
                            // TODO
                        }
                        accesskit_xplat::WindowEvent::ActionRequested(_req) => {
                            // TODO
                        }
                    }
                }
            }
            #[cfg(feature = "devtools")]
            PageframeShellEvent::DevtoolsPoll => self.process_devtools_messages(),
            #[cfg(feature = "devtools")]
            PageframeShellEvent::OpenDevtools { doc_id } => self.open_devtools(doc_id),
            PageframeShellEvent::Embedder(_) => {
                // Do nothing. Should be handled by embedders (if required).
            }
            PageframeShellEvent::Navigate(_opts) => {
                // Do nothing. Should be handled by embedders (if required).
            }
            PageframeShellEvent::NavigationLoad { .. } => {
                // Do nothing. Should be handled by embedders (if required).
            }
            #[cfg(target_arch = "wasm32")]
            PageframeShellEvent::ResizeSettleCheck { window_id } => {
                if let Some(window) = self.windows.get_mut(&window_id) {
                    window.apply_pending_resize_if_settled();
                }
            }
        }
    }
}

impl<Rend: WindowRenderer + 'static> ApplicationHandler for PageframeApplication<Rend> {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        // Resume existing windows
        for view in self.windows.values_mut() {
            view.resume();
        }

        // Initialise pending windows. The renderer's resume is non-blocking —
        // on native it finishes inline, on wasm32 it spawns a future that will
        // dispatch PageframeShellEvent::ResumeReady when init completes. Either way
        // we insert the view immediately so the event handler can find it.
        for window_config in self.pending_windows.drain(..) {
            let mut view = View::init(window_config, event_loop, &self.proxy);
            view.resume();
            self.windows.insert(view.window_id(), view);
        }
    }

    fn destroy_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {
        for view in self.windows.values_mut() {
            view.suspend();
        }
    }

    fn resumed(&mut self, _event_loop: &dyn ActiveEventLoop) {
        // TODO
    }

    fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
        // TODO
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::DataTransferReceived { serial, value, .. } = &event
            && let Some(&requesting_window) = self.pending_clipboard.get(serial)
        {
            let result = value.try_as_string();
            if result
                .as_ref()
                .is_err_and(|error| error.kind() == std::io::ErrorKind::WouldBlock)
            {
                return;
            }
            self.pending_clipboard.remove(serial);
            if let Ok(text) = result
                && let Some(window) = self.windows.get_mut(&requesting_window)
            {
                window
                    .doc
                    .handle_ui_event(pageframe_traits::events::UiEvent::Paste(text));
                window.request_redraw();
                self.proxy.send_event(PageframeShellEvent::Poll {
                    window_id: requesting_window,
                });
            }
            return;
        }
        // Exit the app when window close is requested.
        if matches!(event, WindowEvent::CloseRequested) {
            // Drop window before exiting event loop
            // See https://github.com/rust-windowing/winit/issues/4135
            let window = self.windows.remove(&window_id);
            drop(window);
            if self.windows.is_empty() {
                event_loop.exit();
            }
            return;
        }

        #[cfg(feature = "devtools")]
        if self.handle_picker_event(window_id, &event) {
            return;
        }

        if let Some(window) = self.windows.get_mut(&window_id) {
            window.handle_winit_event(event);
        }
        self.proxy
            .send_event(PageframeShellEvent::Poll { window_id });
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        while let Ok(event) = self.event_queue.try_recv() {
            self.handle_pageframe_shell_event(event_loop, event);
        }
    }

    #[cfg(target_os = "macos")]
    fn macos_handler(&mut self) -> Option<&mut dyn ApplicationHandlerExtMacOS> {
        Some(self)
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        let now = Instant::now();
        let mut next = None;
        for view in self.windows.values_mut() {
            if let Some(at) = view.retry_render(now) {
                next = Some(next.map_or(at, |other: Instant| other.min(at)));
            }
            #[cfg(target_os = "ios")]
            if view.ios_request_redraw.get() {
                view.window.request_redraw();
            }
        }
        event_loop.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}

#[cfg(target_os = "macos")]
impl<Rend: WindowRenderer + 'static> ApplicationHandlerExtMacOS for PageframeApplication<Rend> {
    fn standard_key_binding(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        action: &str,
    ) {
        if let Some(window) = self.windows.get_mut(&window_id) {
            window.handle_apple_standard_keybinding(action);
            self.proxy
                .send_event(PageframeShellEvent::Poll { window_id });
        }
    }
}
