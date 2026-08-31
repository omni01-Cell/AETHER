use aether_core::{AetherError, GenerationKind, ProfessionalPrompt};

use crate::prompter::LlmPrompterAgent;
use crate::prompt::{PromptMaker, PromptMakerContext, RuleBasedPromptMaker};

/// Agent LLM Maître d'Hôtel quand un modèle guidé est ciblé, sinon RuleBasedPromptMaker.
pub struct CompositePromptMaker {
    pub llm_prompter: Option<LlmPrompterAgent>,
    pub rule_based: RuleBasedPromptMaker,
}

impl Default for CompositePromptMaker {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositePromptMaker {
    pub fn new() -> Self {
        CompositePromptMaker {
            llm_prompter: LlmPrompterAgent::new().ok(),
            rule_based: RuleBasedPromptMaker,
        }
    }
}

impl PromptMaker for CompositePromptMaker {
    fn make_prompt(
        &self,
        kind: GenerationKind,
        user_request: &str,
        context: &PromptMakerContext,
    ) -> Result<ProfessionalPrompt, AetherError> {
        if context.target_model_id.is_some() {
            if let Some(ref prompter) = self.llm_prompter {
                return prompter.make_prompt(kind, user_request, context);
            }
        }
        self.rule_based.make_prompt(kind, user_request, context)
    }
}
