use aether_core::AetherError;

use crate::agent_config::{load_agent_config, AgentModelConfig};
use crate::bridge::{invoke_bridge, resolve_bridge_script, BridgeRequest, BridgeResponse};

use super::llm_outcome::{parse_chat_completion_response, LlmTurnOutcome};

const AGENT_NAME: &str = "planner";

/// Appel LLM dédié à l'agent planner — lit `agents.v1.json`, délègue au bridge TypeScript.
#[derive(Debug, Clone)]
pub struct PlannerCall {
    config: AgentModelConfig,
    bridge_script: std::path::PathBuf,
}

impl PlannerCall {
    pub fn new() -> Result<Self, AetherError> {
        Ok(PlannerCall {
            config: load_agent_config(AGENT_NAME)?,
            bridge_script: resolve_bridge_script()?,
        })
    }

    pub fn config(&self) -> &AgentModelConfig {
        &self.config
    }

    /// Chat simple (sans tools) — prêt pour un planner LLM futur.
    pub fn chat(&self, system: &str, user: &str) -> Result<LlmTurnOutcome, AetherError> {
        let req = BridgeRequest {
            version: crate::bridge::BRIDGE_VERSION,
            operation: "chat_completions".to_string(),
            bridge_handler: Some(self.config.bridge_handler.clone()),
            agent: Some(AGENT_NAME.to_string()),
            provider: Some(self.config.provider.clone()),
            model_id: self.config.model_id.clone(),
            api_model: Some(self.config.api_model.clone()),
            messages: Some(vec![
                serde_json::json!({ "role": "system", "content": system }),
                serde_json::json!({ "role": "user", "content": user }),
            ]),
            tools: None,
            prompt: String::new(),
            input_image_paths: vec![],
            output_dir: String::new(),
            options: serde_json::json!({ "temperature": 0.2 }),
        };

        let resp = invoke_bridge(&req, &self.bridge_script)?;
        match resp {
            BridgeResponse::Success(s) if s.ok => {
                let raw = s
                    .metadata
                    .get("raw_response")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AetherError::OperationFailed(
                            "Planner bridge: missing metadata.raw_response".to_string(),
                        )
                    })?;
                parse_chat_completion_response(raw)
            }
            BridgeResponse::Failure(f) => Err(AetherError::OperationFailed(format!(
                "Planner bridge {} failed: {}",
                f.provider, f.error
            ))),
            BridgeResponse::Success(_) => Err(AetherError::OperationFailed(
                "Planner bridge returned ok=false".to_string(),
            )),
        }
    }
}

pub fn planner_llm_enabled() -> bool {
    match std::env::var("AETHER_PLANNER_LLM").as_deref() {
        Ok("1") | Ok("true") | Ok("on") => true,
        _ => false,
    }
}
