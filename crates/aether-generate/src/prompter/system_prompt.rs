use std::path::Path;

use super::guide::{load_system_template, ModelPromptGuide};

const MODEL_PLACEHOLDER: &str = "{model}";

/// Message `system` = `system.md` avec `{model}` remplacé par le bloc du JSON modèle.
/// Les tools sont envoyés séparément dans l'appel API (comme en Python : `messages` + `tools`).
pub fn build_system_message(
    guide: &ModelPromptGuide,
    guides_dir: &Path,
) -> Result<String, aether_core::AetherError> {
    let template = load_system_template(guides_dir)?;
    if !template.contains(MODEL_PLACEHOLDER) {
        return Err(aether_core::AetherError::OperationFailed(
            "system.md must contain the {model} placeholder".to_string(),
        ));
    }
    let model_block = guide.format_model_block();
    Ok(template.replace(MODEL_PLACEHOLDER, &model_block))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompter::guide::{load_guide_for_model, resolve_guides_dir};

    #[test]
    fn system_md_substitutes_model_placeholder() {
        let guides_dir = resolve_guides_dir().unwrap();
        let guide =
            load_guide_for_model("google/gemini-3.1-flash-image-preview", &guides_dir).unwrap();
        let sys = build_system_message(&guide, &guides_dir).unwrap();
        assert!(sys.contains("You are the AETHER"));
        assert!(!sys.contains("{model}"));
        assert!(sys.contains("Nano Banana"));
        assert!(sys.contains("aspect_ratio"));
    }
}
