use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ModelFile {
    pub filename: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ModelGroup {
    pub label: String,
    pub files: Vec<ModelFile>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ModelManifest {
    pub manifest_version: u32,
    pub min_app_version: String,
    pub update_notice: Option<String>,
    pub models: HashMap<String, ModelGroup>,
}

impl ModelManifest {
    /// Deserializes the manifest from a JSON string.
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Failed to parse model manifest: {}", e))
    }

    /// Returns the compiled-in fallback manifest.
    pub fn fallback() -> Self {
        const FALLBACK_JSON: &str = include_str!("../../models/manifest.json");
        Self::parse(FALLBACK_JSON).expect("Failed to parse fallback manifest; this is a developer bug")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_manifest_parses_successfully() {
        let manifest = ModelManifest::fallback();
        assert_eq!(manifest.manifest_version, 1);
        assert_eq!(manifest.min_app_version, "0.1.0");
        assert!(manifest.models.contains_key("qwen"));
        assert!(manifest.models.contains_key("clap"));
        assert!(manifest.models.contains_key("sentence"));
        assert!(manifest.models.contains_key("essentia"));

        // Verify some properties of Qwen model
        let qwen_group = manifest.models.get("qwen").unwrap();
        assert_eq!(qwen_group.label, "Qwen Audio LLM");
        assert_eq!(qwen_group.files.len(), 2);
        assert_eq!(qwen_group.files[0].filename, "Qwen2-Audio-7B-Instruct.Q4_K_M.gguf");
        assert_eq!(qwen_group.files[0].size_bytes, 4720000000);
    }
}
