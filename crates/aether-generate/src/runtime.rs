use std::path::PathBuf;
use std::time::SystemTime;
use aether_core::{
    AetherError, GenerationJob, GenerationRequest, GenerationStatus,
};
use crate::mock::MockProvider;
use crate::prompt::{PromptMaker, PromptMakerContext, RuleBasedPromptMaker};
use crate::provider::GenerationProvider;
use crate::registry::ModelRegistry;

pub struct GenerationRuntime<P, M>
where
    P: GenerationProvider,
    M: PromptMaker,
{
    pub provider: P,
    pub prompt_maker: M,
    pub model_registry: ModelRegistry,
}

impl<P, M> GenerationRuntime<P, M>
where
    P: GenerationProvider,
    M: PromptMaker,
{
    /// Invariant: must execute the complete generation pipeline synchronously (prompt make -> model select -> submit -> status check -> artifact download) and return the finalized GenerationJob.
    pub fn run_to_completion(
        &self,
        request: GenerationRequest,
    ) -> Result<GenerationJob, AetherError> {
        let now_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // 1. Model Selection
        let requested_model_str = request.model.as_deref();
        let resolved_model = self
            .model_registry
            .find_model(request.kind, requested_model_str)
            .ok_or_else(|| {
                AetherError::OperationFailed(format!(
                    "No enabled model found for kind {:?}",
                    request.kind
                ))
            })?;

        // 2. Prompt Context and Security Validation
        let locale = request.options.get("locale").and_then(|v| v.as_str()).map(String::from);
        let style_hint = request.options.get("style").and_then(|v| v.as_str()).map(String::from);
        let project_summary = request.options.get("project_summary").and_then(|v| v.as_str()).map(String::from);

        let vault_context = request
            .options
            .get("vault_context")
            .cloned()
            .and_then(|v| serde_json::from_value::<aether_core::PromptContext>(v).ok());

        // Safety validation of restricted vault assets against provider
        if let Some(ref vc) = vault_context {
            for v_ctx in &vc.vault_context {
                for asset in &v_ctx.reference_assets {
                    if let serde_json::Value::Object(ref meta) = asset.metadata {
                        if meta.get("restricted").and_then(|r| r.as_bool()).unwrap_or(false) {
                            // Check allowed_providers
                            if let Some(allowed) = meta.get("allowed_providers").and_then(|a| a.as_array()) {
                                let allowed_str: Vec<String> = allowed
                                    .iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect();
                                if !allowed_str.contains(&resolved_model.provider) {
                                    return Err(AetherError::OperationFailed(format!(
                                        "Security violation: Vault asset '{}' is restricted and not allowed on provider '{}'",
                                        asset.name, resolved_model.provider
                                    )));
                                }
                            }
                            // Check disallowed_providers
                            if let Some(disallowed) = meta.get("disallowed_providers").and_then(|d| d.as_array()) {
                                let disallowed_str: Vec<String> = disallowed
                                    .iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect();
                                if disallowed_str.contains(&resolved_model.provider) {
                                    return Err(AetherError::OperationFailed(format!(
                                        "Security violation: Vault asset '{}' is restricted and disallowed on provider '{}'",
                                        asset.name, resolved_model.provider
                                    )));
                                }
                            }
                        }
                    }
                }
            }
        }

        let context = PromptMakerContext {
            project_summary,
            locale,
            style_hint,
            vault_context,
        };

        let prompt = self
            .prompt_maker
            .make_prompt(request.kind, &request.user_request, &context)?;

        // 3. Prepare initial job
        let mut job = GenerationJob {
            job_ref: request.job_ref,
            kind: request.kind,
            status: GenerationStatus::Queued,
            requested_model: request.model.clone(),
            resolved_model: Some(resolved_model),
            provider_job_id: None,
            prompt: Some(prompt),
            inputs: request.inputs.clone(),
            artifacts: Vec::new(),
            error: None,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            options: request.options.clone(),
        };

        // 4. Submit
        job.status = GenerationStatus::Submitted;
        let submit_res = self.provider.submit(&request)?;
        job.provider_job_id = Some(submit_res.provider_job_id);
        job.status = submit_res.status;
        job.updated_at_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // 5. Poll to completion (immediate for mock)
        let status = self.provider.status(&job)?;
        job.status = status;

        if job.status == GenerationStatus::Ready {
            job.status = GenerationStatus::Downloading;
            let artifacts = self.provider.download(&job)?;
            job.artifacts = artifacts;
            job.status = GenerationStatus::Ready;
        }

        job.updated_at_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Ok(job)
    }

    /// Invariant: must cancel the specified job via the provider and update its status to Cancelled.
    pub fn cancel(&self, job: &mut GenerationJob) -> Result<(), AetherError> {
        self.provider.cancel(job)?;
        job.status = GenerationStatus::Cancelled;
        job.updated_at_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Ok(())
    }
}

pub struct DefaultGenerationRuntime {
    pub runtime: GenerationRuntime<MockProvider, RuleBasedPromptMaker>,
}

impl DefaultGenerationRuntime {
    /// Invariant: must construct a DefaultGenerationRuntime with MockProvider storing artifacts in the specified path, RuleBasedPromptMaker, and pre-populated ModelRegistry.
    pub fn mock(output_dir: PathBuf) -> Self {
        DefaultGenerationRuntime {
            runtime: GenerationRuntime {
                provider: MockProvider::new(output_dir),
                prompt_maker: RuleBasedPromptMaker,
                model_registry: ModelRegistry::with_builtin_placeholders(),
            },
        }
    }

    /// Invariant: must delegate run_to_completion execution to the inner runtime.
    pub fn run_to_completion(
        &self,
        request: GenerationRequest,
    ) -> Result<GenerationJob, AetherError> {
        self.runtime.run_to_completion(request)
    }

    /// Invariant: must delegate cancel execution to the inner runtime.
    pub fn cancel(&self, job: &mut GenerationJob) -> Result<(), AetherError> {
        self.runtime.cancel(job)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::{Ref, GenerationKind, GenerationStatus};

    #[test]
    fn test_generation_runtime_run_to_completion() {
        let temp_dir = std::env::temp_dir().join("aether_runtime_test");
        let runtime = DefaultGenerationRuntime::mock(temp_dir.clone());

        let job_ref = "@g10".parse::<Ref>().unwrap();
        let request = GenerationRequest {
            job_ref,
            kind: GenerationKind::StoryboardScratch,
            user_request: "commercial for perfume".to_string(),
            model: None,
            inputs: Vec::new(),
            options: serde_json::json!({
                "style": "cinematic",
                "locale": "fr"
            }),
        };

        let job = runtime.run_to_completion(request).unwrap();
        assert_eq!(job.job_ref, job_ref);
        assert_eq!(job.status, GenerationStatus::Ready);
        assert_eq!(job.resolved_model.as_ref().unwrap().id, "mock/storyboard");
        assert!(job.prompt.is_some());
        let prompt_obj = job.prompt.as_ref().unwrap();
        assert!(prompt_obj.professional_prompt.contains("[AI Generation Mode: StoryboardScratch]"));
        assert!(prompt_obj.professional_prompt.contains("(Style: cinematic)"));
        assert_eq!(prompt_obj.locale, Some("fr".to_string()));

        assert_eq!(job.artifacts.len(), 1);
        let art = &job.artifacts[0];
        assert_eq!(art.kind, aether_core::GeneratedArtifactKind::StoryboardJson);
        assert!(art.path.exists());

        // Test cancel
        let mut job_to_cancel = job.clone();
        runtime.cancel(&mut job_to_cancel).unwrap();
        assert_eq!(job_to_cancel.status, GenerationStatus::Cancelled);

        // Cleanup
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}

