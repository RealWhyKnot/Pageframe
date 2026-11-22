use wgpu::{
    BackendOptions, Backends, Instance, InstanceDescriptor, InstanceFlags, MemoryBudgetThresholds,
    PowerPreference,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BackendChoice {
    pub(crate) primary: Backends,
    pub(crate) fallback: Option<Backends>,
}

#[cfg(target_os = "windows")]
const DEFAULT_CHOICE: BackendChoice = BackendChoice {
    primary: Backends::VULKAN.union(Backends::DX12),
    fallback: Some(Backends::GL),
};

#[cfg(not(target_os = "windows"))]
const DEFAULT_CHOICE: BackendChoice = BackendChoice {
    primary: Backends::all(),
    fallback: None,
};

const EVERY_BACKEND: BackendChoice = BackendChoice {
    primary: Backends::all(),
    fallback: None,
};

impl BackendChoice {
    pub(crate) fn from_env() -> Self {
        let adapter_name = std::env::var("WGPU_ADAPTER_NAME").ok();
        Self::new(
            Backends::from_env(),
            adapter_name.as_deref(),
            PowerPreference::from_env(),
        )
    }

    fn new(
        requested: Option<Backends>,
        adapter_name: Option<&str>,
        power_preference: Option<PowerPreference>,
    ) -> Self {
        match (requested, adapter_name, power_preference) {
            (Some(primary), _, _) => Self {
                primary,
                fallback: None,
            },
            (None, Some(_), _) => EVERY_BACKEND,
            (None, None, Some(PowerPreference::LowPower | PowerPreference::HighPerformance)) => {
                EVERY_BACKEND
            }
            (None, None, Some(PowerPreference::None) | None) => DEFAULT_CHOICE,
        }
    }
}

pub(crate) fn create_instance(backends: Backends) -> Instance {
    Instance::new(InstanceDescriptor {
        backends,
        flags: InstanceFlags::from_build_config().with_env(),
        backend_options: BackendOptions::from_env_or_default(),
        memory_budget_thresholds: MemoryBudgetThresholds::default(),
        display: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const POWER_PREFERENCES: [Option<PowerPreference>; 4] = [
        None,
        Some(PowerPreference::None),
        Some(PowerPreference::LowPower),
        Some(PowerPreference::HighPerformance),
    ];

    #[test]
    fn requested_backends_are_used_as_is_without_a_fallback() {
        for (value, backends) in [
            ("gl", Backends::GL),
            ("vulkan", Backends::VULKAN),
            ("dx12, vulkan", Backends::VULKAN | Backends::DX12),
            ("vk,gl", Backends::VULKAN | Backends::GL),
            (" VK , Dx12 ", Backends::VULKAN | Backends::DX12),
            ("gl,bogus", Backends::GL),
            ("", Backends::empty()),
            ("bogus", Backends::empty()),
        ] {
            for adapter_name in [None, Some("NVIDIA")] {
                for power_preference in POWER_PREFERENCES {
                    assert_eq!(
                        BackendChoice::new(
                            Some(Backends::from_comma_list(value)),
                            adapter_name,
                            power_preference,
                        ),
                        BackendChoice {
                            primary: backends,
                            fallback: None,
                        },
                        "WGPU_BACKEND={value:?} WGPU_ADAPTER_NAME={adapter_name:?} {power_preference:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn an_adapter_name_without_backends_searches_every_backend() {
        for adapter_name in ["ANGLE", ""] {
            for power_preference in POWER_PREFERENCES {
                assert_eq!(
                    BackendChoice::new(None, Some(adapter_name), power_preference),
                    BackendChoice {
                        primary: Backends::all(),
                        fallback: None,
                    },
                    "WGPU_ADAPTER_NAME={adapter_name:?} {power_preference:?}"
                );
            }
        }
    }

    #[test]
    fn a_low_or_high_power_preference_without_backends_searches_every_backend() {
        for power_preference in [PowerPreference::LowPower, PowerPreference::HighPerformance] {
            assert_eq!(
                BackendChoice::new(None, None, Some(power_preference)),
                BackendChoice {
                    primary: Backends::all(),
                    fallback: None,
                },
                "{power_preference:?}"
            );
        }
    }

    #[test]
    fn no_power_preference_keeps_the_platform_default() {
        for power_preference in [None, Some(PowerPreference::None)] {
            assert_eq!(
                BackendChoice::new(None, None, power_preference),
                DEFAULT_CHOICE,
                "{power_preference:?}"
            );
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_defaults_to_vulkan_and_dx12_and_falls_back_to_gl() {
        assert_eq!(
            BackendChoice::new(None, None, None),
            BackendChoice {
                primary: Backends::VULKAN | Backends::DX12,
                fallback: Some(Backends::GL),
            }
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn other_platforms_default_to_every_backend() {
        assert_eq!(
            BackendChoice::new(None, None, None),
            BackendChoice {
                primary: Backends::all(),
                fallback: None,
            }
        );
    }
}
