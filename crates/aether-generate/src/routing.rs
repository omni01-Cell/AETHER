use aether_core::{AetherError, GenerationKind, ProviderModel};

use crate::registry::ModelRegistry;
use crate::routing_config::RoutingConfig;

/// Modèle résolu + handler bridge TypeScript (depuis routing.v1.json).
#[derive(Debug, Clone)]
pub struct RoutedModel {
    pub model: ProviderModel,
    pub bridge_handler: String,
}

/// Primary + fallback model route for a generation kind.
#[derive(Debug, Clone)]
pub struct ModelRoute {
    pub primary: RoutedModel,
    pub fallback: Option<RoutedModel>,
}

impl ModelRoute {
    pub fn candidates(&self) -> Vec<&RoutedModel> {
        let mut out = vec![&self.primary];
        if let Some(ref fb) = self.fallback {
            out.push(fb);
        }
        out
    }
}

impl ModelRegistry {
    /// Route déclarative (`routing.v1.json`) puis fallback legacy registry.
    pub fn resolve_route(
        &self,
        kind: GenerationKind,
        requested_model: Option<&str>,
    ) -> Option<ModelRoute> {
        if let Ok(cfg) = RoutingConfig::load() {
            if let Some(route) = cfg.resolve_route(self, kind, requested_model) {
                return Some(route);
            }
        }
        self.resolve_route_legacy(kind, requested_model)
    }

    fn resolve_route_legacy(
        &self,
        kind: GenerationKind,
        requested_model: Option<&str>,
    ) -> Option<ModelRoute> {
        let primary = self.find_model(kind, requested_model)?;
        let bridge_handler = primary
            .capabilities
            .get("bridge_handler")
            .and_then(|v| v.as_str())
            .unwrap_or("openai-image-edit")
            .to_string();
        let fallback_id = primary
            .capabilities
            .get("fallback_model_id")
            .and_then(|v| v.as_str());
        let fallback = fallback_id.and_then(|id| {
            self.find_model(kind, Some(id)).map(|m| RoutedModel {
                bridge_handler: m
                    .capabilities
                    .get("bridge_handler")
                    .and_then(|v| v.as_str())
                    .unwrap_or("nano-banana-image-edit")
                    .to_string(),
                model: m,
            })
        });
        Some(ModelRoute {
            primary: RoutedModel {
                model: primary,
                bridge_handler,
            },
            fallback,
        })
    }
}

/// Fonction métier (ex. `edit-image`) pour un kind.
pub fn function_for_kind(kind: GenerationKind) -> Result<String, AetherError> {
    let cfg = RoutingConfig::load()?;
    cfg.function_for_kind(kind)
        .map(|e| e.function.clone())
        .ok_or_else(|| {
            AetherError::OperationFailed(format!("No routing entry for kind {:?}", kind))
        })
}
