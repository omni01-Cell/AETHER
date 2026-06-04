use aether_core::AetherError;
use serde_json::{json, Map, Value};

use super::guide::{ModelPromptGuide, ParameterSpec};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PrompterClarification {
    pub field: String,
    pub question: String,
    pub suggested: Vec<String>,
    pub default_applied: Value,
}

#[derive(Debug, Clone)]
pub struct PrompterOutput {
    pub professional_prompt: String,
    pub api_options: Value,
    pub clarifications: Vec<PrompterClarification>,
    pub prompter_context: Value,
}

/// Fusionne les paramètres renvoyés par l'LLM avec le guide (validation + defaults + overlay explicite).
pub fn merge_params_with_guide(
    guide: &ModelPromptGuide,
    mut llm_params: Map<String, Value>,
    explicit_options: &Value,
) -> Result<Map<String, Value>, AetherError> {
    llm_params
        .entry("api_model".to_string())
        .or_insert_with(|| json!(guide.api_model));

    for (name, spec) in &guide.parameters {
        if let Some(v) = llm_params.get(name) {
            validate_param(name, spec, v)?;
        } else if !spec.default.is_null() {
            llm_params.insert(name.clone(), spec.default.clone());
        }
    }

    if let Some(obj) = explicit_options.get(&guide.provider).and_then(|v| v.as_object()) {
        for (k, v) in obj {
            llm_params.insert(k.clone(), v.clone());
        }
    }
    if let Some(obj) = explicit_options.as_object() {
        for (k, v) in obj {
            if guide.parameters.contains_key(k) {
                llm_params.insert(k.clone(), v.clone());
            }
        }
    }

    Ok(llm_params)
}

fn validate_param(name: &str, spec: &ParameterSpec, value: &Value) -> Result<(), AetherError> {
    if spec.allowed.is_empty() {
        return Ok(());
    }
    let ok = spec.allowed.iter().any(|a| a == value);
    if !ok {
        return Err(AetherError::OperationFailed(format!(
            "Paramètre '{}' = {} hors allowed {:?}",
            name, value, spec.allowed
        )));
    }
    Ok(())
}
