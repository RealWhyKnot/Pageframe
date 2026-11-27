use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use anyrender::{ImageRenderer, RenderContext, ResourceId};
use peniko::ImageData;
use rustc_hash::FxHashMap;
use vello::{Renderer as VelloRenderer, RendererOptions, Scene as VelloScene};
use wgpu::{Backend, PipelineCache, PowerPreference, TextureUsages};
use wgpu_context::{BufferRenderer, BufferRendererConfig, DeviceHandle};

use crate::backends::{BackendChoice, create_instance};
use crate::shared::{PooledHandle, lock};
use crate::{DEFAULT_THREADS, VelloScenePainter};

#[derive(PartialEq)]
struct AdapterSelection {
    choice: BackendChoice,
    adapter_name: Option<String>,
    power_preference: Option<PowerPreference>,
}

struct HeadlessContext {
    renderer: Mutex<VelloRenderer>,
    device: PooledHandle,
}

static HEADLESS_CONTEXT: Mutex<Option<(AdapterSelection, Arc<HeadlessContext>)>> = Mutex::new(None);

impl HeadlessContext {
    fn open_device(choice: BackendChoice) -> DeviceHandle {
        let primary = pollster::block_on(DeviceHandle::new_from_compatible_surface(
            create_instance(choice.primary),
            None,
            None,
            None,
        ));
        match (primary, choice.fallback) {
            (Ok(handle), _) => handle,
            (Err(_), Some(fallback)) => {
                pollster::block_on(DeviceHandle::new_from_compatible_surface(
                    create_instance(fallback),
                    None,
                    None,
                    None,
                ))
                .expect("No compatible device found")
            }
            (Err(err), None) => panic!("No compatible device found: {err:?}"),
        }
    }

    fn new(choice: BackendChoice, pipeline_cache: Option<PipelineCache>) -> Self {
        let handle = Self::open_device(choice);
        let num_init_threads = if handle.adapter.get_info().backend == Backend::Dx12 {
            NonZeroUsize::new(1)
        } else {
            DEFAULT_THREADS
        };
        let renderer = VelloRenderer::new(
            &handle.device,
            RendererOptions {
                use_cpu: false,
                num_init_threads,
                antialiasing_support: vello::AaSupport::area_only(),
                pipeline_cache,
            },
        )
        .expect("Got non-Send/Sync error from creating renderer");

        Self {
            renderer: Mutex::new(renderer),
            device: PooledHandle::watch(handle),
        }
    }

    fn acquire(selection: AdapterSelection, pipeline_cache: Option<PipelineCache>) -> Arc<Self> {
        let mut cached = lock(&HEADLESS_CONTEXT);
        if pipeline_cache.is_some() {
            return Arc::new(Self::new(selection.choice, pipeline_cache));
        }
        if let Some((key, context)) = cached.as_ref()
            && *key == selection
            && context.device.is_usable()
        {
            return Arc::clone(context);
        }
        let context = Arc::new(Self::new(selection.choice, None));
        *cached = Some((selection, Arc::clone(&context)));
        context
    }
}

pub struct VelloImageRenderer {
    buffer_renderer: BufferRenderer,
    context: Arc<HeadlessContext>,
    scene: VelloScene,
    texture_handles: FxHashMap<ResourceId, ImageData>,
}

impl VelloImageRenderer {
    /// Like [`ImageRenderer::new`], but reusing compiled pipelines from a cache
    /// wgpu built earlier. Creating the cache, persisting its data and deciding
    /// when it is stale are all the caller's responsibility.
    pub fn with_pipeline_cache(
        width: u32,
        height: u32,
        pipeline_cache: Option<PipelineCache>,
    ) -> Self {
        let choice = BackendChoice::from_env();
        let context = HeadlessContext::acquire(
            AdapterSelection {
                choice,
                adapter_name: std::env::var("WGPU_ADAPTER_NAME").ok(),
                power_preference: PowerPreference::from_env(),
            },
            pipeline_cache,
        );

        let buffer_renderer = BufferRenderer::new(
            BufferRendererConfig {
                width,
                height,
                usage: TextureUsages::STORAGE_BINDING,
            },
            context.device.handle.clone(),
            0,
        );

        Self {
            buffer_renderer,
            context,
            scene: VelloScene::new(),
            texture_handles: FxHashMap::default(),
        }
    }
}

impl Drop for VelloImageRenderer {
    fn drop(&mut self) {
        if !self.texture_handles.is_empty() {
            let mut renderer = lock(&self.context.renderer);
            for (_, handle) in self.texture_handles.drain() {
                renderer.unregister_texture(handle);
            }
        }
    }
}

impl RenderContext for VelloImageRenderer {}
impl ImageRenderer for VelloImageRenderer {
    type ScenePainter<'a>
        = VelloScenePainter<'a, 'a>
    where
        Self: 'a;

    fn new(width: u32, height: u32) -> Self {
        Self::with_pipeline_cache(width, height, None)
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.buffer_renderer.resize(width, height);
    }

    fn reset(&mut self) {
        self.scene.reset();
    }

    fn render_to_vec<F: FnOnce(&mut Self::ScenePainter<'_>)>(
        &mut self,
        draw_fn: F,
        cpu_buffer: &mut Vec<u8>,
    ) {
        let size = self.buffer_renderer.size();
        cpu_buffer.resize((size.width * size.height * 4) as usize, 0);
        self.render(draw_fn, cpu_buffer);
    }

    fn render<F: FnOnce(&mut Self::ScenePainter<'_>)>(
        &mut self,
        draw_fn: F,
        cpu_buffer: &mut [u8],
    ) {
        draw_fn(&mut VelloScenePainter {
            inner: &mut self.scene,
            renderer: Some(&self.context.renderer),
            device_handle: Some(&self.buffer_renderer.device_handle),
            texture_handles: Some(&mut self.texture_handles),
        });

        let mut vello_renderer = lock(&self.context.renderer);
        for handle in self.texture_handles.values() {
            vello_renderer.mark_override_image_dirty(handle);
        }

        let size = self.buffer_renderer.size();
        vello_renderer
            .render_to_texture(
                self.buffer_renderer.device(),
                self.buffer_renderer.queue(),
                &self.scene,
                &self.buffer_renderer.target_texture_view(),
                &vello::RenderParams {
                    base_color: vello::peniko::Color::TRANSPARENT,
                    width: size.width,
                    height: size.height,
                    antialiasing_method: vello::AaConfig::Area,
                },
            )
            .expect("Got non-Send/Sync error from rendering");
        drop(vello_renderer);

        self.buffer_renderer.copy_texture_to_buffer(cpu_buffer);

        // Empty the Vello scene (memory optimisation)
        self.scene.reset();
    }
}
