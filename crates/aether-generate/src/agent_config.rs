use std::collections::HashMap;
use std::path::Path;

use aether_core::AetherError;
use serde::Deserialize;

use crate::prompter::guide::resolve_guides_dir;

/// Configuration d'un agent LLM (prompter, planner, …).
#[derive(Debug, Clone, Deserialize)]
pub struct AgentModelConfig {
    pub provider: String,
    pub model_id: String,
    pub api_model: String,
    pub bridge_handler: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentsConfigFile {
    pub schema_version: u32,
    pub agents: HashMap<String, AgentModelConfig>,
}

impl AgentsConfigFile {
    pub fn load_from_dir(guides_dir: &Path) -> Result<Self, AetherError> {
        let path = guides_dir.join("agents.v1.json");
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| AetherError::IoError(path.display().to_string(), e.to_string()))?;
        let cfg: AgentsConfigFile = serde_json::from_str(&raw).map_err(|e| {
            AetherError::OperationFailed(format!(
                "Invalid agents config {}: {}",
                path.display(),
                e
            ))
        })?;
        if cfg.schema_version != 1 {
            return Err(AetherError::OperationFailed(format!(
                "Unsupported agents schema_version {} in {}",
                cfg.schema_version,
                path.display()
            )));
        }
        Ok(cfg)
    }

    pub fn load() -> Result<Self, AetherError> {
        Self::load_from_dir(&resolve_guides_dir()?)
    }

    pub fn agent(&self, name: &str) -> Result<&AgentModelConfig, AetherError> {
        self.agents.get(name).ok_or_else(|| {
            AetherError::OperationFailed(format!(
                "Unknown agent '{}' in agents.v1.json",
                name
            ))
        })
    }
}

pub fn load_agent_config(name: &str) -> Result<AgentModelConfig, AetherError> {
    AgentsConfigFile::load()?.agent(name).cloned()
}
