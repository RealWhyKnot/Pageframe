#[cfg(feature = "gpu")]
use anyrender_vello::VelloWindowRenderer as WindowRenderer;
#[cfg(not(feature = "gpu"))]
use anyrender_vello_cpu::VelloCpuWindowRenderer as WindowRenderer;
use pageframe_shell::{
    PageframeApplication, PageframeShellProxy, WindowConfig, create_default_event_loop,
};
use winit::dpi::LogicalSize;
use winit::window::WindowAttributes;

use super::App;
use crate::error::Error;

impl<S: 'static> App<S> {
    pub fn run(self) -> Result<(), Error> {
        let attributes = WindowAttributes::default()
            .with_title(self.title.clone())
            .with_surface_size(LogicalSize::new(self.width, self.height));
        let page = self.build(None)?;
        let event_loop = create_default_event_loop();
        let (proxy, receiver) = PageframeShellProxy::new(event_loop.create_proxy());
        let mut application = PageframeApplication::new(proxy, receiver);
        application.add_window(WindowConfig::with_attributes(
            Box::new(page),
            WindowRenderer::new(),
            attributes,
        ));
        event_loop.run_app(application)?;
        Ok(())
    }
}
