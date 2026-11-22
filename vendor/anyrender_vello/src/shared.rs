use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use vello::{AaConfig, AaSupport, Renderer as VelloRenderer, RendererOptions};
use wgpu::{Device, Features, Instance, Limits, PipelineCache, Surface};
use wgpu_context::DeviceHandle;

use crate::DEFAULT_THREADS;
use crate::backends::{BackendChoice, create_instance};

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) struct WeakList<T>(Vec<Weak<T>>);

impl<T> WeakList<T> {
    pub(crate) const fn new() -> Self {
        Self(Vec::new())
    }

    pub(crate) fn find_or_insert_with(
        &mut self,
        matches: impl Fn(&T) -> bool,
        make: impl FnOnce() -> T,
    ) -> Arc<T> {
        self.0.retain(|entry| entry.strong_count() > 0);
        if let Some(found) = self
            .0
            .iter()
            .filter_map(Weak::upgrade)
            .find(|entry| matches(entry))
        {
            return found;
        }
        let made = Arc::new(make());
        self.0.push(Arc::downgrade(&made));
        made
    }

    #[cfg(test)]
    pub(crate) fn live(&self) -> usize {
        self.0
            .iter()
            .filter(|entry| entry.strong_count() > 0)
            .count()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PoolKey {
    pub(crate) backends: BackendChoice,
    pub(crate) features: Features,
    pub(crate) limits: Option<Limits>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RendererKey {
    pub(crate) antialiasing: AaConfig,
    pub(crate) pipeline_cache: Option<PipelineCache>,
}

pub(crate) fn antialiasing_support(method: AaConfig) -> AaSupport {
    std::iter::once(method).collect()
}

pub(crate) type Registry = Mutex<WeakList<DevicePool>>;

#[cfg(not(target_arch = "wasm32"))]
static POOLS: Registry = Mutex::new(WeakList::new());

pub(crate) struct SharedRenderer {
    key: RendererKey,
    pub(crate) vello: Mutex<VelloRenderer>,
}

impl SharedRenderer {
    fn new(device: &Device, key: RendererKey) -> Self {
        let vello = VelloRenderer::new(
            device,
            RendererOptions {
                antialiasing_support: antialiasing_support(key.antialiasing),
                use_cpu: false,
                num_init_threads: DEFAULT_THREADS,
                pipeline_cache: key.pipeline_cache.clone(),
            },
        )
        .expect("Error creating the vello renderer");
        Self {
            key,
            vello: Mutex::new(vello),
        }
    }
}

#[derive(Clone)]
pub(crate) struct PooledHandle {
    pub(crate) handle: DeviceHandle,
    lost: Arc<AtomicBool>,
}

impl PooledHandle {
    pub(crate) fn watch(handle: DeviceHandle) -> Self {
        let lost = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&lost);
        handle
            .device
            .set_device_lost_callback(move |_, _| flag.store(true, Ordering::Release));
        Self { handle, lost }
    }

    fn is_usable(&self) -> bool {
        !self.lost.load(Ordering::Acquire)
    }
}

struct PooledDevice {
    device: PooledHandle,
    renderers: WeakList<SharedRenderer>,
}

impl PooledDevice {
    fn is_usable(&self) -> bool {
        self.device.is_usable()
    }
}

pub(crate) struct DevicePool {
    key: Option<PoolKey>,
    instance: Instance,
    devices: Mutex<Vec<PooledDevice>>,
}

impl DevicePool {
    fn new(instance: Instance, key: Option<PoolKey>) -> Self {
        Self {
            key,
            instance,
            devices: Mutex::new(Vec::new()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn shared(key: PoolKey) -> Arc<Self> {
        Self::shared_in(&POOLS, key)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn shared(key: PoolKey) -> Arc<Self> {
        thread_local! {
            static POOLS: Registry = const { Mutex::new(WeakList::new()) };
        }
        POOLS.with(|registry| Self::shared_in(registry, key))
    }

    pub(crate) fn shared_in(registry: &Registry, key: PoolKey) -> Arc<Self> {
        lock(registry).find_or_insert_with(
            |pool| pool.key.as_ref() == Some(&key),
            || Self::new(create_instance(key.backends.primary), Some(key.clone())),
        )
    }

    pub(crate) fn instance(&self) -> &Instance {
        &self.instance
    }

    pub(crate) fn for_instance(self: &Arc<Self>, instance: Instance) -> Arc<Self> {
        if instance == self.instance {
            Arc::clone(self)
        } else {
            Arc::new(Self::new(instance, None))
        }
    }

    pub(crate) fn compatible_device(&self, surface: &Surface<'_>) -> Option<PooledHandle> {
        let mut devices = lock(&self.devices);
        devices.retain(PooledDevice::is_usable);
        devices
            .iter()
            .find(|pooled| pooled.device.handle.adapter.is_surface_supported(surface))
            .map(|pooled| pooled.device.clone())
    }

    pub(crate) fn attach(
        &self,
        device: PooledHandle,
        surface: &Surface<'_>,
        key: &RendererKey,
    ) -> (DeviceHandle, Arc<SharedRenderer>) {
        let mut devices = lock(&self.devices);
        devices.retain(PooledDevice::is_usable);
        let index = devices
            .iter()
            .position(|pooled| pooled.device.handle.device == device.handle.device)
            .or_else(|| {
                devices.iter().position(|pooled| {
                    let adapter = &pooled.device.handle.adapter;
                    adapter.is_surface_supported(surface)
                })
            })
            .unwrap_or_else(|| {
                devices.push(PooledDevice {
                    device,
                    renderers: WeakList::new(),
                });
                devices.len() - 1
            });
        let pooled = &mut devices[index];
        let renderer = pooled.renderers.find_or_insert_with(
            |renderer| renderer.key == *key,
            || SharedRenderer::new(&pooled.device.handle.device, key.clone()),
        );
        (pooled.device.handle.clone(), renderer)
    }

    #[cfg(test)]
    pub(crate) fn device_count(&self) -> usize {
        lock(&self.devices).len()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn only_the_configured_antialiasing_method_is_compiled() {
        assert_eq!(
            antialiasing_support(AaConfig::Msaa16),
            AaSupport {
                area: false,
                msaa8: false,
                msaa16: true,
            }
        );
        assert_eq!(antialiasing_support(AaConfig::Area), AaSupport::area_only());
    }

    #[test]
    fn a_live_matching_entry_is_reused_without_making_another() {
        let mut list = WeakList::new();
        let first = list.find_or_insert_with(|value: &u32| *value == 1, || 1);
        let made = Cell::new(false);
        let again = list.find_or_insert_with(
            |value| *value == 1,
            || {
                made.set(true);
                1
            },
        );
        assert!(Arc::ptr_eq(&first, &again));
        assert!(!made.get());
        assert_eq!(list.live(), 1);
    }

    #[test]
    fn an_entry_that_does_not_match_is_never_shared() {
        let mut list = WeakList::new();
        let first = list.find_or_insert_with(|value: &u32| *value == 1, || 1);
        let other = list.find_or_insert_with(|value| *value == 2, || 2);
        assert!(!Arc::ptr_eq(&first, &other));
        assert_eq!((*first, *other), (1, 2));
        assert_eq!(list.live(), 2);
    }

    #[test]
    fn the_list_keeps_nothing_alive_once_the_last_user_drops_it() {
        let mut list = WeakList::new();
        let first = list.find_or_insert_with(|value: &Vec<u8>| value.len() == 3, || vec![0; 3]);
        let weak = Arc::downgrade(&first);
        let second = list.find_or_insert_with(|value| value.len() == 3, Vec::new);
        drop(first);
        assert!(weak.upgrade().is_some());
        drop(second);
        assert!(weak.upgrade().is_none());
        assert_eq!(list.live(), 0);
        let made = Cell::new(false);
        let rebuilt = list.find_or_insert_with(
            |value| value.len() == 3,
            || {
                made.set(true);
                vec![1; 3]
            },
        );
        assert!(made.get());
        assert_eq!(*rebuilt, vec![1; 3]);
        assert_eq!(list.0.len(), 1);
    }

    #[test]
    fn renderer_keys_only_match_the_same_antialiasing_and_pipeline_cache() {
        let key = |antialiasing| RendererKey {
            antialiasing,
            pipeline_cache: None,
        };
        assert_eq!(key(AaConfig::Msaa16), key(AaConfig::Msaa16));
        assert_ne!(key(AaConfig::Msaa16), key(AaConfig::Area));
        assert_ne!(key(AaConfig::Msaa16), key(AaConfig::Msaa8));
    }
}
