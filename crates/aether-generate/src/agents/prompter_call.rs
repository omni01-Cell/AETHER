use aether_core::AetherError;
use serde_json::{json, Value};

use crate::agent_config::{load_agent_config, AgentModelConfig};
use crate::bridge::{invoke_bridge, resolve_bridge_script, BridgeRequest, BridgeResponse};
use crate::prompter::tools::prompter_tools_schema;

use super::llm_outcome::{parse_chat_completion_response, LlmTurnOutcome};

const AGENT_NAME: &str = "prompter";

/// Appel LLM dédié à l'agent prompter — lit `agents.v1.json`, délègue au bridge TypeScript.
#[derive(Debug, Clone)]
pub struct PrompterCall {
    config: AgentModelConfig,
    bridge_script: std::path::PathBuf,
}

impl PrompterCall {
    pub fn new() -> Result<Self, AetherError> {
        Ok(PrompterCall {
            config: load_agent_config(AGENT_NAME)?,
            bridge_script: resolve_bridge_script()?,
        })
    }

    pub fn chat_with_tools(&self, system: &str, user: &str) -> Result<LlmTurnOutcome, AetherError> {
        let messages = vec![
            json!({ "role": "system", "content": system }),
            json!({ "role": "user", "content": user }),
        ];
        self.chat_with_tools_from_history_raw(&messages)
    }

    /// Appel LLM avec un historique de conversation complet (pour la boucle CLI).
    pub fn chat_with_tools_from_history(
        &self,
        conversation: &[Value],
    ) -> Result<LlmTurnOutcome, AetherError> {
        self.chat_with_tools_from_history_raw(conversation)
    }

    fn chat_with_tools_from_history_raw(
        &self,
        messages: &[Value],
    ) -> Result<LlmTurnOutcome, AetherError> {
        let tools = prompter_tools_schema();
        let req = BridgeRequest {
            version: crate::bridge::BRIDGE_VERSION,
            operation: "chat_completions".to_string(),
            bridge_handler: Some(self.config.bridge_handler.clone()),
            agent: Some(AGENT_NAME.to_string()),
            provider: Some(self.config.provider.clone()),
            model_id: self.config.model_id.clone(),
            api_model: Some(self.config.api_model.clone()),
            messages: Some(messages.to_vec()),
            tools: Some(tools),
            prompt: String::new(),
            input_image_paths: vec![],
            output_dir: String::new(),
            options: json!({ "temperature": 0.3 }),
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
                            "Bridge chat_completions: missing metadata.raw_response".to_string(),
                        )
                    })?;
                parse_chat_completion_response(raw)
            }
            BridgeResponse::Failure(f) => Err(AetherError::OperationFailed(format!(
                "Prompter bridge {} failed: {}",
                f.provider, f.error
            ))),
            BridgeResponse::Success(_) => Err(AetherError::OperationFailed(
                "Prompter bridge returned ok=false".to_string(),
            )),
        }
    }
}

pub fn prompter_llm_enabled() -> bool {
    match std::env::var("AETHER_PROMPTER_LLM").as_deref() {
        Ok("0") | Ok("false") | Ok("off") => false,
        _ => true,
    }
}
