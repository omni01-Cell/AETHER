use std::path::Path;

use aether_core::{AetherError, GenerationKind};
use serde::Deserialize;

use crate::prompter::guide::resolve_guides_dir;
use crate::registry::ModelRegistry;
use crate::routing::{ModelRoute, RoutedModel};

#[derive(Debug, Clone, Deserialize)]
pub struct RoutedModelRef {
    pub model_id: String,
    pub bridge_handler: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RouteEntry {
    pub function: String,
    pub kinds: Vec<String>,
    pub primary: RoutedModelRef,
    #[serde(default)]
    pub fallback: Option<RoutedModelRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoutingConfig {
    pub schema_version: u32,
    pub routes: Vec<RouteEntry>,
}

impl RoutingConfig {
    pub fn load_from_dir(guides_dir: &Path) -> Result<Self, AetherError> {
        let path = guides_dir.join("routing.v1.json");
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| AetherError::IoError(path.display().to_string(), e.to_string()))?;
        let cfg: RoutingConfig = serde_json::from_str(&raw).map_err(|e| {
            AetherError::OperationFailed(format!(
                "Invalid routing config {}: {}",
                path.display(),
                e
            ))
        })?;
        if cfg.schema_version != 1 {
            return Err(AetherError::OperationFailed(format!(
                "Unsupported routing schema_version {} in {}",
                cfg.schema_version,
                path.display()
            )));
        }
        Ok(cfg)
    }

    pub fn load() -> Result<Self, AetherError> {
        Self::load_from_dir(&resolve_guides_dir()?)
    }

    pub fn function_for_kind(&self, kind: GenerationKind) -> Option<&RouteEntry> {
        let kind_str = format!("{:?}", kind);
        self.routes
            .iter()
            .find(|r| r.kinds.iter().any(|k| k == &kind_str))
    }

    fn resolve_routed(
        registry: &ModelRegistry,
        kind: GenerationKind,
        spec: &RoutedModelRef,
    ) -> Option<RoutedModel> {
        let model = registry.find_model(kind, Some(&spec.model_id))?;
        Some(RoutedModel {
            model,
            bridge_handler: spec.bridge_handler.clone(),
        })
    }

    pub fn resolve_route(
        &self,
        registry: &ModelRegistry,
        kind: GenerationKind,
        requested_model: Option<&str>,
    ) -> Option<ModelRoute> {
        if let Some(req) = requested_model {
            if let Some(primary) = registry.find_model(kind, Some(req)) {
                let entry = self.function_for_kind(kind);
                let bridge_handler = entry
                    .and_then(|e| {
                        if e.primary.model_id == req {
                            Some(e.primary.bridge_handler.clone())
                        } else {
                            e.fallback
                                .as_ref()
                                .filter(|f| f.model_id == req)
                                .map(|f| f.bridge_handler.clone())
                        }
                    })
                    .unwrap_or_else(|| {
                        primary
                            .capabilities
                            .get("bridge_handler")
                            .and_then(|v| v.as_str())
                            .unwrap_or("openai-image-edit")
                            .to_string()
                    });
                let fallback = entry.and_then(|e| {
                    e.fallback
                        .as_ref()
                        .and_then(|f| Self::resolve_routed(registry, kind, f))
                });
                return Some(ModelRoute {
                    primary: RoutedModel {
                        model: primary,
                        bridge_handler,
                    },
                    fallback,
                });
            }
        }

        let entry = self.function_for_kind(kind)?;
        let primary = Self::resolve_routed(registry, kind, &entry.primary)?;
        let fallback = entry
            .fallback
            .as_ref()
            .and_then(|f| Self::resolve_routed(registry, kind, f));
        Some(ModelRoute { primary, fallback })
    }
}

pub fn resolve_routing_config() -> Result<RoutingConfig, AetherError> {
    RoutingConfig::load()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_config_loads() {
        let dir = resolve_guides_dir().unwrap();
        let cfg = RoutingConfig::load_from_dir(&dir).unwrap();
        let edit = cfg.function_for_kind(GenerationKind::ImageEdit).unwrap();
        assert_eq!(edit.function, "edit-image");
        assert_eq!(edit.primary.model_id, "openai/gpt-image-2");
        assert_eq!(edit.primary.bridge_handler, "openai-image-edit");
        assert_eq!(
            edit.fallback.as_ref().map(|f| f.model_id.as_str()),
            Some("google/gemini-3.1-flash-image-preview")
        );
        assert_eq!(
            edit.fallback.as_ref().map(|f| f.bridge_handler.as_str()),
            Some("nano-banana-image-edit")
        );
    }
}
