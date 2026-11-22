use anyrender::{
    RegisterResourceErrorKind, RenderContext, ResourceId, WindowHandle, WindowRenderer,
};
use debug_timer::debug_timer;
use futures_channel::oneshot;
use peniko::{Color, ImageData};
use rustc_hash::FxHashMap;
use std::future::Future;
use std::sync::Arc;
use vello::{
    AaConfig, AaSupport, RenderParams, Renderer as VelloRenderer, RendererOptions,
    Scene as VelloScene,
};
use wgpu::{
    Backends, CompositeAlphaMode, CreateSurfaceError, Features, Instance, Limits, PipelineCache,
    PresentMode, Surface, Texture, TextureFormat, TextureUsages,
};
use wgpu_context::{
    AlphaConversion, DeviceHandle, SurfaceRenderer, SurfaceRendererConfiguration,
    TextureConfiguration, WgpuContextError,
};

use crate::backends::{BackendChoice, create_instance};
use crate::{DEFAULT_THREADS, VelloScenePainter};

/// Drive the wgpu init future. On wasm32 we spawn it onto the JS microtask
/// queue (blocking is not allowed). On native we drive it inline with
/// `pollster::block_on` — there's no ambient async runtime to spawn onto, and
/// `on_ready` then fires before `resume` returns.
#[cfg(target_arch = "wasm32")]
fn spawn_init<F: Future<Output = ()> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(f);
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_init<F: Future<Output = ()>>(f: F) {
    pollster::block_on(f);
}

struct ActiveRenderState {
    renderer: VelloRenderer,
    render_surface: SurfaceRenderer<'static>,
}

struct InitOutput {
    active: ActiveRenderState,
}

#[allow(clippy::large_enum_variant)]
enum RenderState {
    Suspended,
    Pending {
        receiver: oneshot::Receiver<InitOutput>,
    },
    Active(ActiveRenderState),
}

#[derive(Clone)]
#[non_exhaustive]
pub struct VelloRendererOptions {
    pub features: Option<Features>,
    pub limits: Option<Limits>,
    pub base_color: Color,
    pub antialiasing_method: AaConfig,
    /// Alpha mode used when compositing the window surface.
    pub composite_alpha_mode: anyrender::CompositeAlphaMode,
    /// Cache that wgpu may reuse compiled pipelines from, avoiding shader
    /// compilation on start-up. Creating it, persisting its data and deciding
    /// when it is stale are all the caller's responsibility.
    pub pipeline_cache: Option<PipelineCache>,
    /// Maximum number of frames the presentation engine may queue ahead of the
    /// display. Each queued frame requires a window-sized swapchain surface, so
    /// lower values reduce memory usage and input latency at the cost of less
    /// buffering to absorb slow frames.
    pub desired_maximum_frame_latency: u32,
}

impl Default for VelloRendererOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl VelloRendererOptions {
    pub const fn new() -> Self {
        Self {
            features: None,
            limits: None,
            base_color: Color::WHITE,
            antialiasing_method: AaConfig::Msaa16,
            composite_alpha_mode: anyrender::CompositeAlphaMode::Auto,
            pipeline_cache: None,
            desired_maximum_frame_latency: 1,
        }
    }

    pub fn features(self, features: Features) -> Self {
        Self {
            features: Some(features),
            ..self
        }
    }

    pub fn limits(self, limits: Limits) -> Self {
        Self {
            limits: Some(limits),
            ..self
        }
    }

    pub fn base_color(self, base_color: Color) -> Self {
        Self { base_color, ..self }
    }

    pub fn antialiasing_method(self, antialiasing_method: AaConfig) -> Self {
        Self {
            antialiasing_method,
            ..self
        }
    }

    pub fn composite_alpha_mode(
        self,
        composite_alpha_mode: anyrender::types::CompositeAlphaMode,
    ) -> Self {
        Self {
            composite_alpha_mode,
            ..self
        }
    }

    pub fn pipeline_cache(self, pipeline_cache: PipelineCache) -> Self {
        Self {
            pipeline_cache: Some(pipeline_cache),
            ..self
        }
    }

    pub fn desired_maximum_frame_latency(self, desired_maximum_frame_latency: u32) -> Self {
        Self {
            desired_maximum_frame_latency,
            ..self
        }
    }
}

impl From<anyrender::RendererConfig> for VelloRendererOptions {
    fn from(config: anyrender::RendererConfig) -> Self {
        Self {
            base_color: config.base_color.unwrap_or(Color::WHITE),
            composite_alpha_mode: config.composite_alpha_mode.unwrap_or_default(),
            ..Default::default()
        }
    }
}

pub struct VelloWindowRenderer {
    // The fields MUST be in this order, so that the surface is dropped before the window
    // Window is cached even when suspended so that it can be reused when the app is resumed after being suspended
    render_state: RenderState,
    window_handle: Option<Arc<dyn WindowHandle>>,

    wgpu_context: GpuContext,
    scene: VelloScene,
    config: VelloRendererOptions,

    // Resources
    texture_handles: FxHashMap<ResourceId, ImageData>,
}

impl VelloWindowRenderer {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self::with_options(VelloRendererOptions::default())
    }

    pub fn with_options(config: impl Into<VelloRendererOptions>) -> Self {
        let config = config.into();
        Self {
            render_state: RenderState::Suspended,
            wgpu_context: GpuContext::new(&config),
            config,
            window_handle: None,
            scene: VelloScene::new(),
            texture_handles: FxHashMap::default(),
        }
    }

    pub fn current_device_handle(&self) -> Option<&DeviceHandle> {
        match &self.render_state {
            RenderState::Active(active) => Some(&active.render_surface.device_handle),
            _ => None,
        }
    }
}

impl RenderContext for VelloWindowRenderer {
    fn try_register_custom_resource(
        &mut self,
        resource: Box<dyn std::any::Any>,
    ) -> Result<ResourceId, anyrender::RegisterResourceError> {
        let RenderState::Active(active) = &mut self.render_state else {
            return Err(RegisterResourceErrorKind::NotActive.into());
        };

        if let Ok(texture) = resource.downcast::<Texture>() {
            let id = ResourceId::new();
            self.texture_handles
                .insert(id, active.renderer.register_texture(*texture));
            Ok(id)
        } else {
            Err(anyrender::RegisterResourceErrorKind::UnsupportedResourceKind.into())
        }
    }

    fn unregister_resource(&mut self, resource_id: ResourceId) {
        let RenderState::Active(active) = &mut self.render_state else {
            return;
        };

        if let Some(handle) = self.texture_handles.remove(&resource_id) {
            active.renderer.unregister_texture(handle);
        }
    }

    fn renderer_specific_context(&self) -> Option<Box<dyn std::any::Any>> {
        match &self.render_state {
            RenderState::Active(active) => {
                Some(Box::new(active.render_surface.device_handle.clone()))
            }
            RenderState::Pending { .. } => None,
            RenderState::Suspended => None,
        }
    }
}

struct GpuContext {
    instance: Instance,
    fallback_backends: Option<Backends>,
    device_pool: Vec<DeviceHandle>,
    extra_features: Option<Features>,
    override_limits: Option<Limits>,
}

impl GpuContext {
    fn new(config: &VelloRendererOptions) -> Self {
        Self::with_backends(config, BackendChoice::from_env())
    }

    fn with_backends(config: &VelloRendererOptions, backends: BackendChoice) -> Self {
        let features = config.features.unwrap_or_default()
            | Features::CLEAR_TEXTURE
            | Features::PIPELINE_CACHE;
        Self {
            instance: create_instance(backends.primary),
            fallback_backends: backends.fallback,
            device_pool: Vec::new(),
            extra_features: Some(features),
            override_limits: config.limits.clone(),
        }
    }

    fn find_compatible_device_handle(&self, surface: &Surface<'_>) -> Option<DeviceHandle> {
        self.device_pool
            .iter()
            .filter(|device_handle| device_handle.instance == self.instance)
            .find(|device_handle| device_handle.adapter.is_surface_supported(surface))
            .cloned()
    }

    fn adopt(&mut self, device_handle: DeviceHandle) {
        if device_handle.instance != self.instance {
            self.instance = device_handle.instance.clone();
            self.fallback_backends = None;
            let instance = &self.instance;
            self.device_pool
                .retain(|pooled| pooled.instance == *instance);
        }
        if !self.device_pool.iter().any(|pooled| {
            pooled.instance == device_handle.instance && pooled.device == device_handle.device
        }) {
            self.device_pool.push(device_handle);
        }
    }
}

fn antialiasing_support(method: AaConfig) -> AaSupport {
    std::iter::once(method).collect()
}

type ConnectError = (WgpuContextError, Option<WgpuContextError>);

async fn connect_surface(
    instance: Instance,
    primary_surface: Result<Surface<'static>, CreateSurfaceError>,
    fallback: Option<(Backends, Arc<dyn WindowHandle>)>,
    extra_features: Option<Features>,
    override_limits: Option<Limits>,
) -> Result<(Surface<'static>, DeviceHandle), ConnectError> {
    let primary = match primary_surface {
        Ok(surface) => DeviceHandle::new_from_compatible_surface(
            instance,
            Some(&surface),
            extra_features,
            override_limits.clone(),
        )
        .await
        .map(|device_handle| (surface, device_handle)),
        Err(error) => Err(WgpuContextError::from(error)),
    };
    let fallback = fallback.map(|(backends, window_handle)| {
        connect_fallback(backends, window_handle, extra_features, override_limits)
    });
    or_fallback(primary, fallback).await
}

async fn or_fallback<T, E>(
    primary: Result<T, E>,
    fallback: Option<impl Future<Output = Result<T, E>>>,
) -> Result<T, (E, Option<E>)> {
    let primary = match primary {
        Ok(connected) => return Ok(connected),
        Err(error) => error,
    };
    let Some(fallback) = fallback else {
        return Err((primary, None));
    };
    fallback.await.map_err(|fallback| (primary, Some(fallback)))
}

async fn connect_fallback(
    backends: Backends,
    window_handle: Arc<dyn WindowHandle>,
    extra_features: Option<Features>,
    override_limits: Option<Limits>,
) -> Result<(Surface<'static>, DeviceHandle), WgpuContextError> {
    let instance = create_instance(backends);
    let surface = instance.create_surface(window_handle)?;
    let device_handle = DeviceHandle::new_from_compatible_surface(
        instance,
        Some(&surface),
        extra_features,
        override_limits,
    )
    .await?;
    Ok((surface, device_handle))
}

impl WindowRenderer for VelloWindowRenderer {
    type ScenePainter<'a>
        = VelloScenePainter<'a, 'a>
    where
        Self: 'a;

    fn is_active(&self) -> bool {
        matches!(self.render_state, RenderState::Active { .. })
    }

    fn is_pending(&self) -> bool {
        matches!(self.render_state, RenderState::Pending { .. })
    }

    fn resume<F: FnOnce() + 'static>(
        &mut self,
        window_handle: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
        on_ready: F,
    ) {
        // Each `resume` must be preceded by `suspend` (or be the first call after
        // construction).
        if !matches!(self.render_state, RenderState::Suspended) {
            // #[cfg(feature = "tracing")]
            // tracing::warn!("WindowRenderer::resume called from non-Suspended state");
            return;
        }

        let (sender, receiver) = oneshot::channel();
        self.render_state = RenderState::Pending { receiver };
        self.window_handle = Some(window_handle.clone());

        let fallback = self
            .wgpu_context
            .fallback_backends
            .map(|backends| (backends, Arc::clone(&window_handle)));
        let surface = self.wgpu_context.instance.create_surface(window_handle);
        let instance = self.wgpu_context.instance.clone();
        let extra_features = self.wgpu_context.extra_features;
        let override_limits = self.wgpu_context.override_limits.clone();
        let antialiasing_support = antialiasing_support(self.config.antialiasing_method);
        let mut composite_alpha_mode = match self.config.composite_alpha_mode {
            anyrender::CompositeAlphaMode::Auto => CompositeAlphaMode::Auto,
            anyrender::CompositeAlphaMode::Opaque => CompositeAlphaMode::Opaque,
            anyrender::CompositeAlphaMode::Transparent => {
                #[cfg(target_vendor = "apple")]
                {
                    // wgpu is lying in apple's case it uses PreMultiplied in reality
                    // (do not modify shaders for PostMultiplied)
                    CompositeAlphaMode::PostMultiplied
                }
                #[cfg(not(target_vendor = "apple"))]
                {
                    CompositeAlphaMode::PreMultiplied
                }
            }
        };
        let pipeline_cache = self.config.pipeline_cache.clone();
        let desired_maximum_frame_latency = self.config.desired_maximum_frame_latency;
        let existing_device_handle = surface
            .as_ref()
            .ok()
            .and_then(|surface| self.wgpu_context.find_compatible_device_handle(surface));

        spawn_init(async move {
            let (surface, device_handle) = match (surface, existing_device_handle) {
                (Ok(surface), Some(device_handle)) => (surface, device_handle),
                (surface, _) => {
                    connect_surface(instance, surface, fallback, extra_features, override_limits)
                        .await
                        .expect("Error creating DeviceHandle")
                }
            };

            let adapter = &device_handle.adapter;
            let caps = surface.get_capabilities(adapter);
            let mut alpha_modes = caps.alpha_modes;

            if !alpha_modes.contains(&composite_alpha_mode) {
                alpha_modes.sort_unstable_by(
                    |first: &CompositeAlphaMode, second: &CompositeAlphaMode| {
                        let first_num = match *first {
                            CompositeAlphaMode::PreMultiplied => 0,
                            CompositeAlphaMode::PostMultiplied => 1,
                            CompositeAlphaMode::Opaque
                            | CompositeAlphaMode::Inherit
                            | CompositeAlphaMode::Auto => 2,
                        };
                        let second_num = match *second {
                            CompositeAlphaMode::PreMultiplied => 0,
                            CompositeAlphaMode::PostMultiplied => 1,
                            CompositeAlphaMode::Opaque
                            | CompositeAlphaMode::Inherit
                            | CompositeAlphaMode::Auto => 2,
                        };
                        first_num.cmp(&second_num)
                    },
                );
                composite_alpha_mode = alpha_modes
                    .first()
                    .copied()
                    .expect("Surface didn't report any alpha modes");
            }

            #[cfg(not(target_vendor = "apple"))]
            let texture_config = Some(TextureConfiguration {
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
                format: TextureFormat::Rgba8Unorm,
                alpha_conversion: (composite_alpha_mode == CompositeAlphaMode::PreMultiplied)
                    .then_some(AlphaConversion::Premultiply),
            });
            // TODO: Remove below once gfx-rs/wgpu#9896 gets fixed
            #[cfg(target_vendor = "apple")]
            let texture_config = Some(TextureConfiguration {
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
                format: TextureFormat::Rgba8Unorm,
                alpha_conversion: (composite_alpha_mode == CompositeAlphaMode::PostMultiplied
                    || composite_alpha_mode == CompositeAlphaMode::PreMultiplied)
                    .then_some(AlphaConversion::Premultiply),
            });

            let render_surface = SurfaceRenderer::new(
                surface,
                SurfaceRendererConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    formats: vec![TextureFormat::Rgba8Unorm, TextureFormat::Bgra8Unorm],
                    width,
                    height,
                    present_mode: PresentMode::AutoVsync,
                    desired_maximum_frame_latency,
                    alpha_mode: composite_alpha_mode,
                    view_formats: vec![],
                },
                texture_config,
                device_handle,
            )
            .expect("Error creating SurfaceRenderer");

            let renderer = VelloRenderer::new(
                render_surface.device(),
                RendererOptions {
                    antialiasing_support,
                    use_cpu: false,
                    num_init_threads: DEFAULT_THREADS,
                    pipeline_cache,
                },
            )
            .unwrap();

            let _ = sender.send(InitOutput {
                active: ActiveRenderState {
                    renderer,
                    render_surface,
                },
            });
            on_ready();
        });
    }

    fn complete_resume(&mut self) -> bool {
        match &mut self.render_state {
            RenderState::Active { .. } => true,
            RenderState::Suspended => false,
            RenderState::Pending { receiver } => match receiver.try_recv() {
                Ok(Some(InitOutput { active })) => {
                    let device_handle = active.render_surface.device_handle.clone();
                    self.wgpu_context.adopt(device_handle);
                    self.render_state = RenderState::Active(active);
                    true
                }
                _ => false,
            },
        }
    }

    fn suspend(&mut self) {
        if let RenderState::Active(active) = &mut self.render_state {
            // Unregister all textures on suspend
            for (_id, handle) in self.texture_handles.drain() {
                active.renderer.unregister_texture(handle);
            }
        }
        self.render_state = RenderState::Suspended;
    }

    fn set_size(&mut self, width: u32, height: u32) {
        if let RenderState::Active(active) = &mut self.render_state {
            active.render_surface.resize(width, height);
        };
    }

    fn render<F: FnOnce(&mut Self::ScenePainter<'_>)>(&mut self, draw_fn: F) {
        let RenderState::Active(state) = &mut self.render_state else {
            return;
        };

        let render_surface = &mut state.render_surface;

        debug_timer!(timer, feature = "log_frame_times");

        // Regenerate the vello scene
        draw_fn(&mut VelloScenePainter {
            inner: &mut self.scene,
            renderer: Some(&mut state.renderer),
            device_handle: Some(&render_surface.device_handle),
            texture_handles: Some(&mut self.texture_handles),
        });
        timer.record_time("cmd");

        let Ok(texture_view) = render_surface.target_texture_view() else {
            // Skip frame in case of error trying to get current surface texture
            render_surface.clear_surface_texture();
            self.scene.reset();
            return;
        };

        for handle in self.texture_handles.values() {
            state.renderer.mark_override_image_dirty(handle);
        }

        state
            .renderer
            .render_to_texture(
                render_surface.device(),
                render_surface.queue(),
                &self.scene,
                &texture_view,
                &RenderParams {
                    base_color: self.config.base_color,
                    width: render_surface.config.width,
                    height: render_surface.config.height,
                    antialiasing_method: self.config.antialiasing_method,
                },
            )
            .expect("failed to render to texture");
        timer.record_time("render");

        drop(texture_view);

        if render_surface.maybe_blit_and_present().is_err() {
            self.scene.reset();
            return;
        }
        timer.record_time("present");

        render_surface
            .device()
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();

        timer.record_time("wait");
        timer.print_times("vello: ");

        // Empty the Vello scene (memory optimisation)
        self.scene.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_configured_antialiasing_method_is_compiled() {
        assert_eq!(
            antialiasing_support(VelloRendererOptions::default().antialiasing_method),
            AaSupport {
                area: false,
                msaa8: false,
                msaa16: true,
            }
        );
        assert_eq!(antialiasing_support(AaConfig::Area), AaSupport::area_only());
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod native {
        use std::cell::Cell;
        use std::future::ready;

        use super::*;

        fn connect(
            primary: Result<u32, &'static str>,
            fallback: Option<Result<u32, &'static str>>,
        ) -> Result<u32, (&'static str, Option<&'static str>)> {
            pollster::block_on(or_fallback(primary, fallback.map(ready)))
        }

        #[test]
        fn a_working_primary_connection_skips_the_fallback() {
            let fallback_ran = Cell::new(false);
            let fallback = async {
                fallback_ran.set(true);
                Ok(2)
            };
            assert_eq!(
                pollster::block_on(or_fallback::<u32, &str>(Ok(1), Some(fallback))),
                Ok(1)
            );
            assert!(!fallback_ran.get());
        }

        #[test]
        fn a_failed_primary_connection_without_a_fallback_reports_the_primary_error() {
            assert_eq!(connect(Err("primary"), None), Err(("primary", None)));
        }

        #[test]
        fn a_failed_primary_connection_uses_the_fallback() {
            assert_eq!(connect(Err("primary"), Some(Ok(2))), Ok(2));
        }

        #[test]
        fn a_failed_fallback_reports_both_errors() {
            assert_eq!(
                connect(Err("primary"), Some(Err("fallback"))),
                Err(("primary", Some("fallback")))
            );
        }

        fn gpu_device(context: &GpuContext, instance: Instance) -> DeviceHandle {
            pollster::block_on(DeviceHandle::new_from_compatible_surface(
                instance,
                None,
                context.extra_features,
                context.override_limits.clone(),
            ))
            .expect("a GPU adapter on the requested backends")
        }

        fn adopt_a_fallback_gpu_device(choice: BackendChoice) {
            let fallback_backends = choice.fallback.expect("a fallback to switch to");
            let mut context = GpuContext::with_backends(&VelloRendererOptions::default(), choice);
            assert_eq!(context.fallback_backends, Some(fallback_backends));
            let primary = gpu_device(&context, context.instance.clone());
            context.adopt(primary.clone());
            context.adopt(primary.clone());
            assert_eq!(context.instance, primary.instance);
            assert_eq!(context.device_pool.len(), 1);
            assert_eq!(context.fallback_backends, Some(fallback_backends));

            let fallback = gpu_device(&context, create_instance(fallback_backends));
            assert_ne!(fallback.instance, primary.instance);
            context.adopt(fallback.clone());
            context.adopt(fallback.clone());
            assert_eq!(context.instance, fallback.instance);
            assert_eq!(context.fallback_backends, None);
            assert_eq!(context.device_pool.len(), 1);
            assert_eq!(context.device_pool[0].instance, fallback.instance);
            assert_eq!(context.device_pool[0].device, fallback.device);
            assert!(
                context
                    .device_pool
                    .iter()
                    .all(|pooled| pooled.instance != primary.instance)
            );
        }

        #[test]
        #[ignore = "needs a GPU adapter"]
        fn adopting_a_device_from_another_instance_switches_instance_and_prunes_the_pool() {
            adopt_a_fallback_gpu_device(BackendChoice {
                primary: Backends::PRIMARY,
                fallback: Some(Backends::PRIMARY),
            });
        }
    }
}
