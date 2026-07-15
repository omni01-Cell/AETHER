use crate::bridge::BridgeGenerationProvider;
use crate::composite_prompt_maker::CompositePromptMaker;
use crate::mock::MockProvider;
use crate::prompt::{PromptMaker, PromptMakerContext};
use crate::provider::GenerationProvider;
use crate::registry::ModelRegistry;
use crate::routing_provider::RoutingGenerationProvider;
use aether_core::{AetherError, GenerationJob, GenerationRequest, GenerationStatus};
use std::path::PathBuf;
use std::time::SystemTime;

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

        // 1. Model Selection (primary + optional fallback)
        let requested_model_str = request.model.as_deref();
        let route = self
            .model_registry
            .resolve_route(request.kind, requested_model_str)
            .ok_or_else(|| {
                AetherError::OperationFailed(format!(
                    "No enabled model found for kind {:?}",
                    request.kind
                ))
            })?;
        let mut resolved_model = route.primary.model.clone();

        // 2. Prompt Context and Security Validation
        let locale = request
            .options
            .get("locale")
            .and_then(|v| v.as_str())
            .map(String::from);
        let style_hint = request
            .options
            .get("style")
            .and_then(|v| v.as_str())
            .map(String::from);
        let project_summary = request
            .options
            .get("project_summary")
            .and_then(|v| v.as_str())
            .map(String::from);

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
                        if meta
                            .get("restricted")
                            .and_then(|r| r.as_bool())
                            .unwrap_or(false)
                        {
                            // Check allowed_providers
                            if let Some(allowed) =
                                meta.get("allowed_providers").and_then(|a| a.as_array())
                            {
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
                            if let Some(disallowed) =
                                meta.get("disallowed_providers").and_then(|d| d.as_array())
                            {
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

        let explicit_options = request.options.clone();
        let base_context = PromptMakerContext {
            project_summary,
            locale,
            style_hint,
            vault_context,
            target_model_id: None,
            explicit_options: explicit_options.clone(),
        };

        let mut request = request;

        // 3. Prepare initial job (prompt filled after successful submit)
        let mut job = GenerationJob {
            job_ref: request.job_ref,
            kind: request.kind,
            status: GenerationStatus::Queued,
            requested_model: request.model.clone(),
            resolved_model: None,
            provider_job_id: None,
            prompt: None,
            inputs: request.inputs.clone(),
            artifacts: Vec::new(),
            error: None,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            options: request.options.clone(),
        };

        // 4. Submit: primary (gpt-image-2) puis fallback (Nano Banana) — prompter relancé par modèle
        job.status = GenerationStatus::Submitted;
        let mut submit_res = None;
        let mut last_err: Option<AetherError> = None;
        let mut final_prompt: Option<aether_core::ProfessionalPrompt> = None;
        let candidates: Vec<&crate::routing::RoutedModel> = route.candidates();

        for candidate in candidates {
            let mut ctx = base_context.clone();
            ctx.target_model_id = Some(candidate.model.id.clone());

            let prompt =
                match self
                    .prompt_maker
                    .make_prompt(request.kind, &request.user_request, &ctx)
                {
                    Ok(p) => p,
                    Err(AetherError::PrompterNeedsClarification(payload)) => {
                        job.status = GenerationStatus::AwaitingClarification;
                        job.resolved_model = Some(candidate.model.clone());
                        if let Ok(clar) = serde_json::from_str::<serde_json::Value>(&payload) {
                            if let serde_json::Value::Object(ref mut map) = job.options {
                                map.insert("prompter_clarifications".to_string(), clar);
                            } else {
                                job.options =
                                    serde_json::json!({ "prompter_clarifications": clar });
                            }
                        }
                        job.error =
                            Some("Prompter needs clarification before image API call".to_string());
                        job.updated_at_ms = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map(|d| d.as_millis() as u64)
                            .unwrap_or(0);
                        return Ok(job);
                    }
                    Err(e) => return Err(e),
                };

            let mut req_for_provider = request.clone();
            if !req_for_provider.options.is_object() {
                req_for_provider.options = serde_json::json!({});
            }
            if let serde_json::Value::Object(ref mut map) = req_for_provider.options {
                if let Some(api_opts) = prompt.technical.get("api_options") {
                    merge_api_options(map, api_opts);
                }
                if let Some(clar) = prompt.technical.get("clarifications") {
                    map.insert("prompter_clarifications".to_string(), clar.clone());
                }
                map.insert(
                    "resolved_model".to_string(),
                    serde_json::to_value(&candidate.model).unwrap_or(serde_json::Value::Null),
                );
                map.insert(
                    "bridge_handler".to_string(),
                    serde_json::Value::String(candidate.bridge_handler.clone()),
                );
                map.insert(
                    "professional_prompt".to_string(),
                    serde_json::Value::String(prompt.professional_prompt.clone()),
                );
                if route.fallback.as_ref().map(|f| &f.model.id) == Some(&candidate.model.id) {
                    map.insert("used_fallback_model".to_string(), serde_json::json!(true));
                }
            }

            match self.provider.submit(&req_for_provider) {
                Ok(res) => {
                    resolved_model = candidate.model.clone();
                    submit_res = Some(res);
                    final_prompt = Some(prompt);
                    request.options = req_for_provider.options;
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }

        let submit_res = submit_res.ok_or_else(|| {
            last_err.unwrap_or_else(|| {
                AetherError::OperationFailed("Generation submit failed".to_string())
            })
        })?;

        job.prompt = final_prompt;
        job.resolved_model = Some(resolved_model);
        job.options = request.options.clone();
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

fn merge_api_options(
    target: &mut serde_json::Map<String, serde_json::Value>,
    api_opts: &serde_json::Value,
) {
    if let Some(obj) = api_opts.as_object() {
        for (provider, params) in obj {
            match target.get_mut(provider) {
                Some(serde_json::Value::Object(existing)) => {
                    if let Some(overlay) = params.as_object() {
                        for (k, v) in overlay {
                            existing.insert(k.clone(), v.clone());
                        }
                    }
                }
                _ => {
                    target.insert(provider.clone(), params.clone());
                }
            }
        }
    }
}

pub struct DefaultGenerationRuntime {
    pub runtime: GenerationRuntime<RoutingGenerationProvider, CompositePromptMaker>,
}

impl DefaultGenerationRuntime {
    /// Mock-only path (alias for `new` when bridge is unavailable).
    pub fn mock(output_dir: PathBuf) -> Self {
        Self::new(output_dir)
    }

    /// Runtime with MockProvider + optional TypeScript API bridge when built and discoverable.
    pub fn new(output_dir: PathBuf) -> Self {
        let mock = MockProvider::new(output_dir.clone());
        let bridge = BridgeGenerationProvider::from_env(output_dir).ok();
        DefaultGenerationRuntime {
            runtime: GenerationRuntime {
                provider: RoutingGenerationProvider { mock, bridge },
                prompt_maker: CompositePromptMaker::new(),
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
    use aether_core::{GenerationKind, GenerationStatus, Ref};

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
        assert!(prompt_obj
            .professional_prompt
            .contains("[AI Generation Mode: StoryboardScratch]"));
        assert!(prompt_obj
            .professional_prompt
            .contains("(Style: cinematic)"));
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
