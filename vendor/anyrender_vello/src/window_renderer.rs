use anyrender::{
    RegisterResourceErrorKind, RenderContext, ResourceId, WindowHandle, WindowRenderer,
};
use debug_timer::debug_timer;
use futures_channel::oneshot;
use peniko::{Color, ImageData};
use rustc_hash::FxHashMap;
use std::future::Future;
use std::sync::Arc;
use vello::{AaConfig, RenderParams, Scene as VelloScene};
use wgpu::{
    Backends, CompositeAlphaMode, CreateSurfaceError, Features, Instance, Limits, PipelineCache,
    PresentMode, Surface, Texture, TextureFormat, TextureUsages,
};
use wgpu_context::{
    AlphaConversion, DeviceHandle, SurfaceRenderer, SurfaceRendererConfiguration,
    TextureConfiguration, WgpuContextError,
};

use crate::VelloScenePainter;
use crate::backends::{BackendChoice, create_instance};
use crate::shared::{DevicePool, PoolKey, PooledHandle, RendererKey, SharedRenderer, lock};

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
    renderer: Arc<SharedRenderer>,
    render_surface: SurfaceRenderer<'static>,
}

struct InitOutput {
    active: ActiveRenderState,
    pool: Arc<DevicePool>,
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
        let wgpu_context = GpuContext::new(&config);
        Self::with_context(config, wgpu_context)
    }

    fn with_context(config: VelloRendererOptions, wgpu_context: GpuContext) -> Self {
        Self {
            render_state: RenderState::Suspended,
            wgpu_context,
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

impl Drop for VelloWindowRenderer {
    fn drop(&mut self) {
        self.suspend();
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
                .insert(id, lock(&active.renderer.vello).register_texture(*texture));
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
            lock(&active.renderer.vello).unregister_texture(handle);
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
    pool: Arc<DevicePool>,
    fallback_backends: Option<Backends>,
    extra_features: Option<Features>,
    override_limits: Option<Limits>,
}

impl GpuContext {
    fn new(config: &VelloRendererOptions) -> Self {
        Self::with_pool(config, BackendChoice::from_env(), DevicePool::shared)
    }

    fn with_pool(
        config: &VelloRendererOptions,
        backends: BackendChoice,
        pool: impl FnOnce(PoolKey) -> Arc<DevicePool>,
    ) -> Self {
        let key = pool_key(config, backends);
        Self {
            fallback_backends: backends.fallback,
            extra_features: Some(key.features),
            override_limits: key.limits.clone(),
            pool: pool(key),
        }
    }

    fn adopt(&mut self, pool: Arc<DevicePool>) {
        if !Arc::ptr_eq(&self.pool, &pool) {
            self.pool = pool;
            self.fallback_backends = None;
        }
    }
}

fn pool_key(config: &VelloRendererOptions, backends: BackendChoice) -> PoolKey {
    PoolKey {
        backends,
        features: config.features.unwrap_or_default()
            | Features::CLEAR_TEXTURE
            | Features::PIPELINE_CACHE,
        limits: config.limits.clone(),
    }
}

fn renderer_key(config: &VelloRendererOptions) -> RendererKey {
    RendererKey {
        antialiasing: config.antialiasing_method,
        pipeline_cache: config.pipeline_cache.clone(),
    }
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
        let pool = Arc::clone(&self.wgpu_context.pool);
        let surface = pool.instance().create_surface(window_handle);
        let instance = pool.instance().clone();
        let extra_features = self.wgpu_context.extra_features;
        let override_limits = self.wgpu_context.override_limits.clone();
        let renderer_key = renderer_key(&self.config);
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
        let desired_maximum_frame_latency = self.config.desired_maximum_frame_latency;
        let existing_device = surface
            .as_ref()
            .ok()
            .and_then(|surface| pool.compatible_device(surface));

        spawn_init(async move {
            let (surface, device) = match (surface, existing_device) {
                (Ok(surface), Some(device)) => (surface, device),
                (surface, _) => {
                    let (surface, device_handle) = connect_surface(
                        instance,
                        surface,
                        fallback,
                        extra_features,
                        override_limits,
                    )
                    .await
                    .expect("Error creating DeviceHandle");
                    (surface, PooledHandle::watch(device_handle))
                }
            };
            let pool = pool.for_instance(device.handle.instance.clone());
            let (device_handle, renderer) = pool.attach(device, &surface, &renderer_key);

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

            let _ = sender.send(InitOutput {
                active: ActiveRenderState {
                    renderer,
                    render_surface,
                },
                pool,
            });
            on_ready();
        });
    }

    fn complete_resume(&mut self) -> bool {
        match &mut self.render_state {
            RenderState::Active { .. } => true,
            RenderState::Suspended => false,
            RenderState::Pending { receiver } => match receiver.try_recv() {
                Ok(Some(InitOutput { active, pool })) => {
                    self.wgpu_context.adopt(pool);
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
            let mut renderer = lock(&active.renderer.vello);
            for (_id, handle) in self.texture_handles.drain() {
                renderer.unregister_texture(handle);
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
            renderer: Some(&state.renderer.vello),
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

        {
            let mut renderer = lock(&state.renderer.vello);
            for handle in self.texture_handles.values() {
                renderer.mark_override_image_dirty(handle);
            }
            renderer
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
        }
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
    use std::sync::Mutex;

    use super::*;
    use crate::shared::{Registry, WeakList};

    fn assert_send<T: Send>() {}

    #[test]
    fn a_window_renderer_can_move_to_another_thread() {
        assert_send::<VelloWindowRenderer>();
    }

    const DEFAULT_CHOICE: BackendChoice = BackendChoice {
        primary: Backends::PRIMARY,
        fallback: Some(Backends::GL),
    };

    #[test]
    fn windows_with_the_same_device_settings_ask_for_the_same_pool() {
        let default = VelloRendererOptions::default();
        let key = pool_key(&default, DEFAULT_CHOICE);
        assert_eq!(
            key.features,
            Features::CLEAR_TEXTURE | Features::PIPELINE_CACHE
        );
        let per_window = default
            .clone()
            .base_color(Color::BLACK)
            .desired_maximum_frame_latency(2)
            .composite_alpha_mode(anyrender::CompositeAlphaMode::Opaque)
            .antialiasing_method(AaConfig::Area);
        assert_eq!(pool_key(&per_window, DEFAULT_CHOICE), key);
    }

    #[test]
    fn windows_with_other_features_limits_or_backends_ask_for_another_pool() {
        let default = VelloRendererOptions::default();
        let key = pool_key(&default, DEFAULT_CHOICE);
        let features = default.clone().features(Features::TIMESTAMP_QUERY);
        assert_ne!(pool_key(&features, DEFAULT_CHOICE), key);
        let limits = default.clone().limits(Limits::default());
        assert_ne!(pool_key(&limits, DEFAULT_CHOICE), key);
        let backends = BackendChoice {
            primary: Backends::DX12,
            fallback: None,
        };
        assert_ne!(pool_key(&default, backends), key);
    }

    #[test]
    fn windows_only_share_a_renderer_built_for_their_antialiasing() {
        let default = VelloRendererOptions::default();
        let key = renderer_key(&default);
        assert_eq!(key.antialiasing, AaConfig::Msaa16);
        assert_eq!(renderer_key(&default.clone().base_color(Color::BLACK)), key);
        assert_ne!(
            renderer_key(&default.clone().antialiasing_method(AaConfig::Area)),
            key
        );
    }

    #[test]
    fn adopting_a_pool_on_another_instance_switches_to_it_without_sharing_it() {
        static REGISTRY: Registry = Mutex::new(WeakList::new());
        let choice = BackendChoice {
            primary: Backends::PRIMARY,
            fallback: Some(Backends::PRIMARY),
        };
        let options = VelloRendererOptions::default();
        let shared_pool = |key| DevicePool::shared_in(&REGISTRY, key);
        let mut context = GpuContext::with_pool(&options, choice, shared_pool);
        let shared = Arc::clone(&context.pool);
        let same = shared.for_instance(shared.instance().clone());
        assert!(Arc::ptr_eq(&same, &shared));
        context.adopt(same);
        assert!(Arc::ptr_eq(&context.pool, &shared));
        assert_eq!(context.fallback_backends, Some(Backends::PRIMARY));

        let fallback = shared.for_instance(create_instance(Backends::PRIMARY));
        assert!(!Arc::ptr_eq(&fallback, &shared));
        assert_ne!(fallback.instance(), shared.instance());
        context.adopt(Arc::clone(&fallback));
        context.adopt(Arc::clone(&fallback));
        assert!(Arc::ptr_eq(&context.pool, &fallback));
        assert_eq!(context.fallback_backends, None);

        let other = GpuContext::with_pool(&options, choice, shared_pool);
        assert!(Arc::ptr_eq(&other.pool, &shared));
        drop((other, context, fallback));
        assert_eq!(lock(&REGISTRY).live(), 1);
        drop(shared);
        assert_eq!(lock(&REGISTRY).live(), 0);
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
    }

    #[cfg(target_os = "windows")]
    mod gpu {
        use std::ffi::c_void;
        use std::num::NonZeroIsize;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Barrier, Weak};
        use std::thread;

        use kurbo::{Affine, Circle, Rect};
        use peniko::{Blob, Fill, FontData, Gradient, ImageAlphaType, ImageBrush, ImageFormat};
        use vello::Glyph;
        use wgpu::rwh::{
            DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
            Win32WindowHandle, WindowHandle as RawHandle,
        };

        use super::*;

        const WIDTH: u32 = 320;
        const HEIGHT: u32 = 240;
        const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;

        #[link(name = "user32")]
        unsafe extern "system" {
            fn CreateWindowExW(
                ex_style: u32,
                class_name: *const u16,
                window_name: *const u16,
                style: u32,
                x: i32,
                y: i32,
                width: i32,
                height: i32,
                parent: isize,
                menu: isize,
                instance: isize,
                param: *mut c_void,
            ) -> isize;
            fn IsWindowVisible(window: isize) -> i32;
        }

        struct HiddenWindow(NonZeroIsize);

        impl HiddenWindow {
            fn new() -> Arc<Self> {
                let class: Vec<u16> = "STATIC".encode_utf16().chain([0]).collect();
                // SAFETY: `class` is a NUL-terminated UTF-16 string that outlives the call and every other argument is a plain value or null.
                let window = unsafe {
                    CreateWindowExW(
                        0,
                        class.as_ptr(),
                        class.as_ptr(),
                        WS_OVERLAPPEDWINDOW,
                        0,
                        0,
                        WIDTH as i32,
                        HEIGHT as i32,
                        0,
                        0,
                        0,
                        std::ptr::null_mut(),
                    )
                };
                let window = NonZeroIsize::new(window).expect("a hidden test window");
                // SAFETY: `window` is a valid window handle this thread just created.
                assert_eq!(unsafe { IsWindowVisible(window.get()) }, 0);
                Arc::new(Self(window))
            }
        }

        impl HasWindowHandle for HiddenWindow {
            fn window_handle(&self) -> Result<RawHandle<'_>, HandleError> {
                let raw = RawWindowHandle::Win32(Win32WindowHandle::new(self.0));
                // SAFETY: test windows are never destroyed before their creating thread exits, which outlives every renderer that holds this handle.
                Ok(unsafe { RawHandle::borrow_raw(raw) })
            }
        }

        impl HasDisplayHandle for HiddenWindow {
            fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
                Ok(DisplayHandle::windows())
            }
        }

        fn open(registry: &'static Registry, config: VelloRendererOptions) -> VelloWindowRenderer {
            let context = GpuContext::with_pool(&config, BackendChoice::from_env(), |key| {
                DevicePool::shared_in(registry, key)
            });
            let mut renderer = VelloWindowRenderer::with_context(config, context);
            renderer.resume(HiddenWindow::new(), WIDTH, HEIGHT, || {});
            assert!(renderer.complete_resume());
            renderer
        }

        fn reopen(renderer: &mut VelloWindowRenderer) {
            renderer.suspend();
            assert!(!renderer.is_active());
            let window = renderer
                .window_handle
                .clone()
                .expect("a window to resume on");
            renderer.resume(window, WIDTH, HEIGHT, || {});
            assert!(renderer.complete_resume());
        }

        fn active(renderer: &VelloWindowRenderer) -> &ActiveRenderState {
            match &renderer.render_state {
                RenderState::Active(active) => active,
                RenderState::Pending { .. } | RenderState::Suspended => {
                    panic!("the window renderer is not active")
                }
            }
        }

        fn device(renderer: &VelloWindowRenderer) -> &DeviceHandle {
            &active(renderer).render_surface.device_handle
        }

        fn shares_renderer(first: &VelloWindowRenderer, second: &VelloWindowRenderer) -> bool {
            Arc::ptr_eq(&active(first).renderer, &active(second).renderer)
        }

        struct Content {
            font: FontData,
            image: ImageData,
            color: Color,
            seed: u8,
        }

        fn pixels(seed: u8) -> Blob<u8> {
            let pixels: Vec<u8> = (0..32u8)
                .flat_map(|y| (0..32u8).flat_map(move |x| [x * 8, y * 8, seed, 255]))
                .collect();
            Blob::new(Arc::new(pixels))
        }

        fn image(seed: u8) -> ImageData {
            ImageData {
                data: pixels(seed),
                format: ImageFormat::Rgba8,
                alpha_type: ImageAlphaType::Alpha,
                width: 32,
                height: 32,
            }
        }

        fn content(seed: u8) -> Content {
            let font = std::fs::read(r"C:\Windows\Fonts\arial.ttf").expect("Arial to draw with");
            Content {
                font: FontData::new(Blob::new(Arc::new(font)), 0),
                image: image(seed),
                color: Color::from_rgb8(seed, 120, 255 - seed),
                seed,
            }
        }

        fn scene(content: &Content) -> VelloScene {
            let mut scene = VelloScene::new();
            let panel = Rect::new(10.0, 10.0, 150.0, 90.0);
            scene.fill(Fill::NonZero, Affine::IDENTITY, content.color, None, &panel);
            let gradient = Gradient::new_linear((160.0, 0.0), (320.0, 0.0))
                .with_stops([Color::from_rgb8(255, 0, 0), content.color]);
            let circle = Circle::new((230.0, 70.0), 55.0);
            scene.fill(Fill::NonZero, Affine::IDENTITY, &gradient, None, &circle);
            let kept = ImageBrush::new(content.image.clone());
            scene.draw_image(&kept, Affine::translate((20.0, 110.0)).pre_scale(2.0));
            let fresh = ImageBrush::new(image(content.seed));
            scene.draw_image(&fresh, Affine::translate((100.0, 110.0)));
            let glyphs = (0..14u32).map(|index| Glyph {
                id: 36 + index + u32::from(content.seed % 7),
                x: 16.0 + index as f32 * 20.0,
                y: 220.0,
            });
            scene
                .draw_glyphs(&content.font)
                .font_size(24.0)
                .brush(Color::BLACK)
                .draw(Fill::NonZero, glyphs);
            scene
        }

        fn present(renderer: &mut VelloWindowRenderer, scene: &VelloScene) {
            renderer.render(|painter| painter.inner.append(scene, None));
        }

        fn read_back(renderer: &VelloWindowRenderer, scene: &VelloScene) -> Vec<u8> {
            let active = active(renderer);
            let DeviceHandle { device, queue, .. } = &active.render_surface.device_handle;
            let size = wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            lock(&active.renderer.vello)
                .render_to_texture(
                    device,
                    queue,
                    scene,
                    &view,
                    &RenderParams {
                        base_color: renderer.config.base_color,
                        width: WIDTH,
                        height: HEIGHT,
                        antialiasing_method: renderer.config.antialiasing_method,
                    },
                )
                .expect("render to the readback texture");
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: u64::from(WIDTH * HEIGHT * 4),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(WIDTH * 4),
                        rows_per_image: Some(HEIGHT),
                    },
                },
                size,
            );
            queue.submit([encoder.finish()]);
            buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .expect("wait for the readback");
            buffer
                .slice(..)
                .get_mapped_range()
                .expect("the mapped readback")
                .to_vec()
        }

        fn alone(registry: &'static Registry, content: &Content) -> Vec<u8> {
            let mut renderer = open(registry, VelloRendererOptions::default());
            let scene = scene(content);
            present(&mut renderer, &scene);
            let pixels = read_back(&renderer, &scene);
            assert!(pixels.chunks(4).any(|pixel| pixel != [255, 255, 255, 255]));
            pixels
        }

        #[test]
        #[ignore = "needs a GPU adapter"]
        fn compatible_windows_share_one_instance_device_and_renderer() {
            static ALONE: Registry = Mutex::new(WeakList::new());
            static REGISTRY: Registry = Mutex::new(WeakList::new());
            let (red, blue) = (content(40), content(200));
            let expected = [alone(&ALONE, &red), alone(&ALONE, &blue)];
            assert_ne!(expected[0], expected[1]);

            let mut first = open(&REGISTRY, VelloRendererOptions::default());
            let mut second = open(&REGISTRY, VelloRendererOptions::default());
            assert!(Arc::ptr_eq(
                &first.wgpu_context.pool,
                &second.wgpu_context.pool
            ));
            assert_eq!(lock(&REGISTRY).live(), 1);
            assert_eq!(first.wgpu_context.pool.device_count(), 1);
            assert_eq!(device(&first).instance, device(&second).instance);
            assert_eq!(device(&first).device, device(&second).device);
            assert!(shares_renderer(&first, &second));
            assert_ne!(
                active(&first).render_surface.surface,
                active(&second).render_surface.surface
            );

            for _ in 0..3 {
                for (renderer, content, expected) in [
                    (&mut first, &red, &expected[0]),
                    (&mut second, &blue, &expected[1]),
                ] {
                    let scene = scene(content);
                    present(renderer, &scene);
                    assert!(read_back(renderer, &scene) == *expected);
                }
            }
        }

        #[test]
        #[ignore = "needs a GPU adapter"]
        fn a_window_with_other_antialiasing_shares_the_device_but_not_the_renderer() {
            static REGISTRY: Registry = Mutex::new(WeakList::new());
            let msaa = open(&REGISTRY, VelloRendererOptions::default());
            let area = VelloRendererOptions::default().antialiasing_method(AaConfig::Area);
            let mut area = open(&REGISTRY, area);
            assert!(Arc::ptr_eq(
                &msaa.wgpu_context.pool,
                &area.wgpu_context.pool
            ));
            assert_eq!(device(&msaa).device, device(&area).device);
            assert!(!shares_renderer(&msaa, &area));
            present(&mut area, &scene(&content(90)));
        }

        #[test]
        #[ignore = "needs a GPU adapter"]
        fn a_window_with_other_limits_gets_its_own_instance_and_device() {
            static REGISTRY: Registry = Mutex::new(WeakList::new());
            let default = open(&REGISTRY, VelloRendererOptions::default());
            let limits = Limits {
                max_inter_stage_shader_variables: 15,
                ..Limits::default()
            };
            let limited = open(&REGISTRY, VelloRendererOptions::default().limits(limits));
            assert!(!Arc::ptr_eq(
                &default.wgpu_context.pool,
                &limited.wgpu_context.pool
            ));
            assert_eq!(lock(&REGISTRY).live(), 2);
            assert_ne!(device(&default).instance, device(&limited).instance);
            assert_eq!(default.wgpu_context.pool.device_count(), 1);
            assert_eq!(limited.wgpu_context.pool.device_count(), 1);
            assert!(!shares_renderer(&default, &limited));
        }

        #[test]
        #[ignore = "needs a GPU adapter"]
        fn the_last_window_releases_the_shared_state_and_a_new_window_builds_it_again() {
            static ALONE: Registry = Mutex::new(WeakList::new());
            static REGISTRY: Registry = Mutex::new(WeakList::new());
            let green = content(120);
            let expected = alone(&ALONE, &green);

            let mut first = open(&REGISTRY, VelloRendererOptions::default());
            let mut second = open(&REGISTRY, VelloRendererOptions::default());
            let pool = Arc::downgrade(&first.wgpu_context.pool);
            let renderer = Arc::downgrade(&active(&first).renderer);
            first.suspend();
            assert!(renderer.upgrade().is_some());
            second.suspend();
            assert!(renderer.upgrade().is_none());
            assert!(pool.upgrade().is_some());
            reopen(&mut first);
            let renderer = Arc::downgrade(&active(&first).renderer);
            drop(first);
            assert!(pool.upgrade().is_some());
            drop(second);
            assert!(pool.upgrade().is_none());
            assert!(renderer.upgrade().is_none());
            assert_eq!(lock(&REGISTRY).live(), 0);

            let mut third = open(&REGISTRY, VelloRendererOptions::default());
            assert_eq!(lock(&REGISTRY).live(), 1);
            let scene = scene(&green);
            present(&mut third, &scene);
            assert!(read_back(&third, &scene) == expected);
        }

        #[test]
        #[ignore = "needs a GPU adapter"]
        fn two_threads_render_through_one_renderer_and_keep_their_pixels() {
            static ALONE: Registry = Mutex::new(WeakList::new());
            static REGISTRY: Registry = Mutex::new(WeakList::new());
            const FRAMES: usize = 60;
            let contents = [content(10), content(230)];
            let expected = [alone(&ALONE, &contents[0]), alone(&ALONE, &contents[1])];
            let windows = [
                open(&REGISTRY, VelloRendererOptions::default()),
                open(&REGISTRY, VelloRendererOptions::default()),
            ];
            assert!(shares_renderer(&windows[0], &windows[1]));
            let start = Barrier::new(2);
            thread::scope(|scope| {
                for ((mut renderer, content), expected) in
                    windows.into_iter().zip(&contents).zip(&expected)
                {
                    let start = &start;
                    scope.spawn(move || {
                        start.wait();
                        for frame in 0..FRAMES {
                            let scene = scene(content);
                            present(&mut renderer, &scene);
                            assert!(
                                read_back(&renderer, &scene) == *expected,
                                "frame {frame} of seed {} changed",
                                content.seed
                            );
                        }
                    });
                }
            });
        }

        #[test]
        #[ignore = "needs a GPU adapter"]
        fn suspending_and_resuming_one_window_leaves_the_other_rendering() {
            static ALONE: Registry = Mutex::new(WeakList::new());
            static REGISTRY: Registry = Mutex::new(WeakList::new());
            const CYCLES: usize = 8;
            let (steady, cycled) = (content(60), content(180));
            let expected = [alone(&ALONE, &steady), alone(&ALONE, &cycled)];
            let mut first = open(&REGISTRY, VelloRendererOptions::default());
            let mut second = open(&REGISTRY, VelloRendererOptions::default());
            let shared: Weak<SharedRenderer> = Arc::downgrade(&active(&first).renderer);
            let done = AtomicBool::new(false);
            let frames = thread::scope(|scope| {
                let rendering = scope.spawn(|| {
                    let mut frames = 0;
                    while !done.load(Ordering::Acquire) || frames < 20 {
                        let scene = scene(&steady);
                        present(&mut first, &scene);
                        assert!(read_back(&first, &scene) == expected[0], "frame {frames}");
                        frames += 1;
                    }
                    frames
                });
                for _ in 0..CYCLES {
                    reopen(&mut second);
                    assert!(Weak::ptr_eq(
                        &shared,
                        &Arc::downgrade(&active(&second).renderer)
                    ));
                    let scene = scene(&cycled);
                    present(&mut second, &scene);
                    assert!(read_back(&second, &scene) == expected[1]);
                }
                done.store(true, Ordering::Release);
                rendering.join().expect("the steady window kept rendering")
            });
            assert!(frames >= 20);
        }

        #[test]
        #[ignore = "needs a GPU adapter"]
        fn a_lost_device_is_not_handed_to_the_next_window() {
            static ALONE: Registry = Mutex::new(WeakList::new());
            static REGISTRY: Registry = Mutex::new(WeakList::new());
            let yellow = content(250);
            let expected = alone(&ALONE, &yellow);
            let first = open(&REGISTRY, VelloRendererOptions::default());
            let lost = device(&first).device.clone();
            lost.destroy();
            let _ = lost.poll(wgpu::PollType::wait_indefinitely());
            let mut second = open(&REGISTRY, VelloRendererOptions::default());
            assert!(Arc::ptr_eq(
                &first.wgpu_context.pool,
                &second.wgpu_context.pool
            ));
            assert_ne!(device(&second).device, lost);
            assert!(!shares_renderer(&first, &second));
            assert_eq!(second.wgpu_context.pool.device_count(), 1);
            let scene = scene(&yellow);
            present(&mut second, &scene);
            assert!(read_back(&second, &scene) == expected);
        }

        #[test]
        #[ignore = "needs a GPU adapter"]
        fn a_device_lost_while_a_window_attaches_does_not_return_to_the_pool() {
            static REGISTRY: Registry = Mutex::new(WeakList::new());
            let options = VelloRendererOptions::default();
            let first = open(&REGISTRY, options.clone());
            let pool = Arc::clone(&first.wgpu_context.pool);
            let surface = pool
                .instance()
                .create_surface(HiddenWindow::new())
                .expect("a surface for the second window");
            let device = pool
                .compatible_device(&surface)
                .expect("the first window's device");
            device.handle.device.destroy();
            let _ = device
                .handle
                .device
                .poll(wgpu::PollType::wait_indefinitely());
            assert!(pool.compatible_device(&surface).is_none());
            let _attached = pool.attach(device, &surface, &renderer_key(&options));
            assert!(pool.compatible_device(&surface).is_none());
            assert_eq!(pool.device_count(), 0);
        }
    }
}
