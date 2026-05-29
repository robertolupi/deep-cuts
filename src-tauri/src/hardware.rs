use std::process::Command;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppleSiliconProfile {
    pub is_arm64: bool,
    pub p_cores: usize,
    pub e_cores: usize,
}

impl AppleSiliconProfile {
    pub fn discover() -> Self {
        let is_arm64 = Self::query_sysctl_bool("hw.optional.arm64");

        let p_cores = Self::query_sysctl_u32("hw.perflevel0.physicalcpu")
            .map(|v| v as usize)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| (n.get() / 2).max(1))
                    .unwrap_or(4)
            });

        let e_cores = Self::query_sysctl_u32("hw.perflevel1.physicalcpu")
            .map(|v| v as usize)
            .unwrap_or(0);

        Self { is_arm64, p_cores, e_cores }
    }

    fn query_sysctl_u32(key: &str) -> Option<u32> {
        let output = Command::new("sysctl").arg("-n").arg(key).output().ok()?;
        let val_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        val_str.parse::<u32>().ok()
    }

    fn query_sysctl_bool(key: &str) -> bool {
        Self::query_sysctl_u32(key).unwrap_or(0) == 1
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineConfig {
    pub use_coreml: bool,
    pub decode_threads: usize,
    pub intra_threads: usize,
}

impl PipelineConfig {
    pub fn auto_tune() -> Self {
        let profile = AppleSiliconProfile::discover();

        if profile.is_arm64 {
            #[cfg(feature = "coreml")]
            {
                let decode_threads = match profile.p_cores {
                    16.. => 8,
                    8..=15 => 4,
                    _ => 2,
                };
                return Self {
                    use_coreml: true,
                    decode_threads,
                    intra_threads: 1,
                };
            }
            #[cfg(not(feature = "coreml"))]
            {
                let intra_threads = (profile.p_cores / 2).max(1).min(4);
                let decode_threads = (profile.p_cores / 2).max(1);
                Self {
                    use_coreml: false,
                    decode_threads,
                    intra_threads,
                }
            }
        } else {
            Self {
                use_coreml: false,
                decode_threads: 2,
                intra_threads: 2,
            }
        }
    }
}
