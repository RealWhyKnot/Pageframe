use std::sync::Arc;
use std::time::Instant;

use anyrender::{WindowHandle, WindowRenderer};
#[cfg(feature = "gpu")]
use anyrender_vello::VelloWindowRenderer as Renderer;
#[cfg(not(feature = "gpu"))]
use anyrender_vello_cpu::VelloCpuWindowRenderer as Renderer;
use pageframe_dom::Document;
use pageframe_paint::paint_scene;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use super::{HostConfig, HostInputEvent, HostSession};
use crate::error::Error;

pub struct PageframeHost<W> {
    session: HostSession,
    window: Arc<W>,
    renderer: Renderer,
}

impl<W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static> PageframeHost<W> {
    pub fn new(window: Arc<W>, config: HostConfig) -> Result<Self, Error> {
        let session = HostSession::new(config)?;
        let mut host = Self {
            session,
            window,
            renderer: Renderer::new(),
        };
        host.resume();
        Ok(host)
    }

    pub fn window(&self) -> &Arc<W> {
        &self.window
    }

    pub fn session(&self) -> &HostSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut HostSession {
        &mut self.session
    }

    pub fn resume(&mut self) {
        if self.renderer.is_active() {
            return;
        }
        let (width, height) = self.session.size();
        let window = Arc::clone(&self.window);
        let handle: Arc<dyn WindowHandle> = window;
        self.renderer
            .resume(handle, width.max(1), height.max(1), || {});
        if self.renderer.complete_resume() {
            self.session.request_redraw();
        }
    }

    pub fn suspend(&mut self) {
        self.renderer.suspend();
    }

    pub fn handle_input(&mut self, event: HostInputEvent) -> bool {
        let resized = matches!(event, HostInputEvent::Resized { .. });
        let handled = self.session.handle_input(event);
        if resized {
            let (width, height) = self.session.size();
            self.renderer.set_size(width.max(1), height.max(1));
        }
        handled
    }

    pub fn poll(&mut self) -> bool {
        self.session.poll()
    }

    pub fn next_wake(&self) -> Option<Instant> {
        self.session.next_wake()
    }

    pub fn redraw(&mut self) {
        if !self.renderer.is_active() {
            return;
        }
        let time = self.session.animation_time();
        let animating = {
            let mut inner = self.session.document_mut().inner_mut();
            inner.resolve(time);
            if inner.has_pending_critical_resources() {
                return;
            }
            let (width, height) = inner.viewport().window_size;
            if width == 0 || height == 0 {
                return;
            }
            let scale = inner.viewport().scale_f64();
            self.renderer
                .render(|scene| paint_scene(scene, &mut inner, scale, width, height, 0, 0));
            inner.is_animating()
        };
        if animating {
            self.session.request_redraw();
        }
    }
}
