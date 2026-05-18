use aether_core::{AetherError, GenerationKind, ProfessionalPrompt};

#[derive(Debug, Clone)]
pub struct PromptMakerContext {
    pub project_summary: Option<String>,
    pub locale: Option<String>,
    pub style_hint: Option<String>,
}

pub trait PromptMaker: Send + Sync {
    /// Invariant: must return a ProfessionalPrompt that wraps the original_request and includes enriched professional fields without modifying the core request.
    fn make_prompt(
        &self,
        kind: GenerationKind,
        user_request: &str,
        context: &PromptMakerContext,
    ) -> Result<ProfessionalPrompt, AetherError>;
}

pub struct RuleBasedPromptMaker;

impl PromptMaker for RuleBasedPromptMaker {
    /// Invariant: must return an enriched ProfessionalPrompt wrapping the user_request according to the specific GenerationKind, incorporating context details if provided.
    fn make_prompt(
        &self,
        kind: GenerationKind,
        user_request: &str,
        context: &PromptMakerContext,
    ) -> Result<ProfessionalPrompt, AetherError> {
        let mut enriched = format!("[AI Generation Mode: {:?}] {}", kind, user_request);
        if let Some(ref style) = context.style_hint {
            enriched.push_str(&format!(" (Style: {})", style));
        }
        if let Some(ref summary) = context.project_summary {
            enriched.push_str(&format!(" [Project Summary: {}]", summary));
        }

        Ok(ProfessionalPrompt {
            original_request: user_request.to_string(),
            professional_prompt: enriched,
            negative_prompt: Some("low quality, blurry, distorted".to_string()),
            locale: context.locale.clone(),
            style: context.style_hint.clone(),
            technical: serde_json::json!({
                "generation_kind": kind,
                "enriched_by": "RuleBasedPromptMaker"
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_based_prompt_maker() {
        let maker = RuleBasedPromptMaker;
        let ctx = PromptMakerContext {
            project_summary: Some("Cyberpunk aesthetic".to_string()),
            locale: Some("en".to_string()),
            style_hint: Some("neon glowing".to_string()),
        };

        let res = maker.make_prompt(GenerationKind::Image, "cyberpunk street", &ctx).unwrap();
        assert_eq!(res.original_request, "cyberpunk street");
        assert!(res.professional_prompt.contains("[AI Generation Mode: Image]"));
        assert!(res.professional_prompt.contains("(Style: neon glowing)"));
        assert!(res.professional_prompt.contains("[Project Summary: Cyberpunk aesthetic]"));
        assert_eq!(res.style, Some("neon glowing".to_string()));
        assert_eq!(res.locale, Some("en".to_string()));
    }
}

