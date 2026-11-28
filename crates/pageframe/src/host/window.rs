use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
#[cfg(feature = "gpu")]
use std::time::Duration;
use std::time::Instant;

use anyrender::{WindowHandle, WindowRenderer};
#[cfg(feature = "gpu")]
use anyrender_vello::{FrameStatus, VelloWindowRenderer as Renderer};
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
    #[cfg(feature = "gpu")]
    retry_redraw_at: Option<Instant>,
}

impl<W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static> PageframeHost<W> {
    pub fn new(window: Arc<W>, config: HostConfig) -> Result<Self, Error> {
        let session = HostSession::new(config)?;
        let mut host = Self {
            session,
            window,
            renderer: Renderer::new(),
            #[cfg(feature = "gpu")]
            retry_redraw_at: None,
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

    pub fn is_active(&self) -> bool {
        self.renderer.is_active()
    }

    pub fn request_redraw(&self) {
        self.session.request_redraw();
    }

    pub fn resume(&mut self) -> bool {
        if self.renderer.is_active() {
            return true;
        }
        let (width, height) = self.session.size();
        let window = Arc::clone(&self.window);
        let handle: Arc<dyn WindowHandle> = window;
        let renderer = &mut self.renderer;
        let started = panic::catch_unwind(AssertUnwindSafe(|| {
            renderer.resume(handle, width.max(1), height.max(1), || {});
        }));
        if started.is_err() {
            self.renderer.suspend();
            return false;
        }
        let active = self.renderer.complete_resume() && self.renderer.is_active();
        if active {
            self.session.request_redraw();
        }
        active
    }

    pub fn suspend(&mut self) {
        self.renderer.suspend();
        #[cfg(feature = "gpu")]
        {
            self.retry_redraw_at = None;
        }
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

    pub fn set_hidden(&mut self, hidden: bool) -> bool {
        let changed = self.session.set_hidden(hidden);
        #[cfg(feature = "gpu")]
        if !hidden && self.retry_redraw_at.is_some() {
            self.session.request_redraw();
        }
        changed
    }

    pub fn poll(&mut self) -> bool {
        let redraw = self.session.poll();
        #[cfg(feature = "gpu")]
        if !self.session.is_hidden() && self.retry_redraw_at.is_some_and(|at| at <= Instant::now())
        {
            self.retry_redraw_at = None;
            if self.renderer.is_active() || self.resume() {
                return true;
            }
            self.retry_redraw_at = Some(Instant::now() + Duration::from_millis(250));
        }
        redraw
    }

    pub fn next_wake(&self) -> Option<Instant> {
        let wake = self.session.next_wake();
        #[cfg(feature = "gpu")]
        let wake = if self.session.is_hidden() {
            wake
        } else {
            wake.into_iter().chain(self.retry_redraw_at).min()
        };
        wake
    }

    pub fn redraw(&mut self) {
        if !self.renderer.is_active() {
            return;
        }
        let time = self.session.animation_time();
        #[cfg(feature = "gpu")]
        let status;
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
            #[cfg(feature = "gpu")]
            {
                status = self.renderer.render_frame(|scene| {
                    paint_scene(scene, &mut inner, scale, width, height, 0, 0)
                });
            }
            #[cfg(not(feature = "gpu"))]
            self.renderer
                .render(|scene| paint_scene(scene, &mut inner, scale, width, height, 0, 0));
            inner.is_animating()
        };
        #[cfg(feature = "gpu")]
        {
            self.retry_redraw_at = match status {
                FrameStatus::Presented => None,
                FrameStatus::Retry => Some(Instant::now() + Duration::from_millis(16)),
                FrameStatus::Occluded => Some(Instant::now() + Duration::from_millis(100)),
                FrameStatus::DeviceLost => {
                    self.renderer.suspend();
                    Some(Instant::now() + Duration::from_millis(250))
                }
            };
            if self.retry_redraw_at.is_some() {
                return;
            }
        }
        if animating {
            self.session.request_redraw();
        }
    }
}
