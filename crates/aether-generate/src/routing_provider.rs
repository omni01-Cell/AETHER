use aether_core::{
    AetherError, GenerationArtifact, GenerationJob, GenerationRequest, ProviderModel,
};

use crate::bridge::BridgeGenerationProvider;
use crate::mock::MockProvider;
use crate::provider::{GenerationProvider, ProviderSubmitResult};

/// Routes generation to MockProvider or BridgeGenerationProvider based on resolved model.
pub struct RoutingGenerationProvider {
    pub mock: MockProvider,
    pub bridge: Option<BridgeGenerationProvider>,
}

impl RoutingGenerationProvider {
    fn delegate<'a>(
        &'a self,
        model: &ProviderModel,
    ) -> Result<&'a dyn GenerationProvider, AetherError> {
        if model.provider == "mock" {
            return Ok(&self.mock);
        }
        if model.provider == "openai"
            || model.provider == "google"
            || model.provider == "bytedance"
            || model.provider == "kuaishou"
        {
            let bridge = self.bridge.as_ref().ok_or_else(|| {
                AetherError::OperationFailed(
                    "Real provider requested but API bridge is not configured (build services/aether-api-bridge)"
                        .to_string(),
                )
            })?;
            return Ok(bridge);
        }
        Err(AetherError::OperationFailed(format!(
            "No provider backend for model provider '{}'",
            model.provider
        )))
    }
}

impl GenerationProvider for RoutingGenerationProvider {
    fn provider_name(&self) -> &'static str {
        "routing"
    }

    fn supports(&self, _model: &ProviderModel) -> bool {
        true
    }

    fn submit(&self, request: &GenerationRequest) -> Result<ProviderSubmitResult, AetherError> {
        let model = request
            .options
            .get("resolved_model")
            .and_then(|v| serde_json::from_value::<ProviderModel>(v.clone()).ok())
            .ok_or_else(|| {
                AetherError::OperationFailed(
                    "RoutingProvider submit requires options.resolved_model".to_string(),
                )
            })?;
        self.delegate(&model)?.submit(request)
    }

    fn status(&self, job: &GenerationJob) -> Result<aether_core::GenerationStatus, AetherError> {
        let model = job.resolved_model.as_ref().ok_or_else(|| {
            AetherError::OperationFailed("Job missing resolved_model".to_string())
        })?;
        self.delegate(model)?.status(job)
    }

    fn download(&self, job: &GenerationJob) -> Result<Vec<GenerationArtifact>, AetherError> {
        let model = job.resolved_model.as_ref().ok_or_else(|| {
            AetherError::OperationFailed("Job missing resolved_model".to_string())
        })?;
        self.delegate(model)?.download(job)
    }

    fn cancel(&self, job: &GenerationJob) -> Result<(), AetherError> {
        let model = job.resolved_model.as_ref().ok_or_else(|| {
            AetherError::OperationFailed("Job missing resolved_model".to_string())
        })?;
        self.delegate(model)?.cancel(job)
    }
}
