use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aether_core::{AetherError, GenerationKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParameterSpec {
    #[serde(rename = "type", default = "default_param_type")]
    pub param_type: String,
    pub default: Value,
    #[serde(default)]
    pub allowed: Vec<Value>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_param_type() -> String {
    "string".to_string()
}

/// Guide modèle v2 : manifeste expert inline + paramètres API (injecté dans `{model}` de system.md).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelPromptGuide {
    pub schema_version: u32,
    pub model_id: String,
    pub provider: String,
    pub api_model: String,
    pub display_name: String,
    pub kinds: Vec<String>,
    /// Guide prompt complet, écrit directement dans le JSON (pas de fichier externe au runtime).
    pub model_guide: String,
    pub parameters: HashMap<String, ParameterSpec>,
}

impl ModelPromptGuide {
    pub fn supports_kind(&self, kind: GenerationKind) -> bool {
        let kind_str = format!("{:?}", kind);
        self.kinds.iter().any(|k| k == &kind_str)
    }

    /// Texte injecté dans la variable `{model}` de `system.md`.
    pub fn format_model_block(&self) -> String {
        let params_json =
            serde_json::to_string_pretty(&self.parameters).unwrap_or_else(|_| "{}".to_string());

        format!(
            r#"**{display}** (`{model_id}` → API `{api_model}`)

### Model guide

{guide}

### API parameters

Use these keys in `finalize_prompt.parameters` (include `"api_model": "{api_model}"`).

```json
{params}
```
"#,
            display = self.display_name,
            model_id = self.model_id,
            api_model = self.api_model,
            guide = self.model_guide,
            params = params_json,
        )
    }

    pub fn to_prompter_context(&self) -> Value {
        serde_json::json!({
            "schema_version": self.schema_version,
            "model_id": self.model_id,
            "provider": self.provider,
            "api_model": self.api_model,
            "display_name": self.display_name,
            "parameter_specs": self.parameters,
        })
    }
}

pub fn resolve_guides_dir() -> Result<PathBuf, AetherError> {
    if let Ok(dir) = std::env::var("AETHER_PROMPT_GUIDES_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Ok(p);
        }
    }

    let mut dir = std::env::current_dir().ok();
    while let Some(ref d) = dir {
        let candidate = d.join("crates/aether-generate/prompt-guides");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        let candidate2 = d.join("prompt-guides");
        if candidate2.is_dir() {
            return Ok(candidate2);
        }
        dir = d.parent().map(PathBuf::from);
    }

    Err(AetherError::OperationFailed(
        "prompt-guides directory not found (set AETHER_PROMPT_GUIDES_DIR)".to_string(),
    ))
}

pub fn load_system_template(guides_dir: &Path) -> Result<String, AetherError> {
    let path = guides_dir.join("system.md");
    std::fs::read_to_string(&path).map_err(|e| {
        AetherError::IoError(path.display().to_string(), e.to_string())
    })
}

pub fn load_guide_for_model(model_id: &str, guides_dir: &Path) -> Result<ModelPromptGuide, AetherError> {
    let (provider, name) = model_id.split_once('/').ok_or_else(|| {
        AetherError::OperationFailed(format!("Invalid model_id for guide load: {}", model_id))
    })?;

    let path = guides_dir.join(provider).join(format!("{}.json", name));
    if !path.is_file() {
        return Err(AetherError::OperationFailed(format!(
            "No prompt guide at {}",
            path.display()
        )));
    }

    let raw = std::fs::read_to_string(&path)
        .map_err(|e| AetherError::IoError(path.display().to_string(), e.to_string()))?;
    let guide: ModelPromptGuide = serde_json::from_str(&raw).map_err(|e| {
        AetherError::OperationFailed(format!(
            "Invalid prompt guide JSON {}: {}",
            path.display(),
            e
        ))
    })?;

    if guide.schema_version != 2 {
        return Err(AetherError::OperationFailed(format!(
            "Unsupported guide schema_version {} in {} (expected 2)",
            guide.schema_version,
            path.display()
        )));
    }

    if guide.model_guide.trim().is_empty() {
        return Err(AetherError::OperationFailed(format!(
            "model_guide must be non-empty inline text in {}",
            path.display()
        )));
    }

    Ok(guide)
}
