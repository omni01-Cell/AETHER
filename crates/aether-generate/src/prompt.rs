use aether_core::{AetherError, GenerationKind, ProfessionalPrompt, PromptContext};

#[derive(Debug, Clone)]
pub struct PromptMakerContext {
    pub project_summary: Option<String>,
    pub locale: Option<String>,
    pub style_hint: Option<String>,
    pub vault_context: Option<PromptContext>,
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

        let mut rules = Vec::new();
        let mut prompt_snippets = Vec::new();
        let mut negative_prompts = Vec::new();

        if let Some(ref vc) = context.vault_context {
            for v_ctx in &vc.vault_context {
                for r in &v_ctx.rules {
                    rules.push(r.clone());
                }
                for s in &v_ctx.prompt_snippets {
                    prompt_snippets.push(s.clone());
                }
                for np in &v_ctx.negative_prompts {
                    negative_prompts.push(np.clone());
                }
            }
        }

        if !rules.is_empty() {
            enriched.push_str(&format!(" [Rules: {}]", rules.join(". ")));
        }
        if !prompt_snippets.is_empty() {
            enriched.push_str(&format!(" [Snippets: {}]", prompt_snippets.join(". ")));
        }

        let mut neg_list = vec!["low quality, blurry, distorted".to_string()];
        if !negative_prompts.is_empty() {
            neg_list.extend(negative_prompts);
        }

        Ok(ProfessionalPrompt {
            original_request: user_request.to_string(),
            professional_prompt: enriched,
            negative_prompt: Some(neg_list.join(", ")),
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
            vault_context: None,
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

