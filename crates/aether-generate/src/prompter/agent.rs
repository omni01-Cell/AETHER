use std::io::{self, Write};
use std::path::PathBuf;

use aether_core::{AetherError, GenerationKind, ProfessionalPrompt};
use serde_json::{json, Map, Value};

use crate::agents::prompter_llm_enabled;
use crate::agents::{LlmTurnOutcome, PrompterCall, ToolCallResult};

use super::engine::{merge_params_with_guide, PrompterOutput};
use super::guide::{load_guide_for_model, resolve_guides_dir, ModelPromptGuide};
use super::system_prompt::build_system_message;
use crate::prompt::{PromptMaker, PromptMakerContext, RuleBasedPromptMaker};

/// Nombre max de tours CLI ( boucle display_cli_message → user input → LLM )
const MAX_CLI_TURNS: usize = 10;

/// Agent prompter : guide JSON modèle + appel LLM via `PrompterCall` (agents.v1.json → bridge TS).
/// Gère la boucle CLI : affiche des messages, attend l'entrée utilisateur, relance le LLM.
#[derive(Debug, Clone)]
pub struct LlmPrompterAgent {
    pub guides_dir: PathBuf,
    pub fallback: RuleBasedPromptMaker,
    caller: Option<PrompterCall>,
}

impl LlmPrompterAgent {
    pub fn new() -> Result<Self, AetherError> {
        let guides_dir = resolve_guides_dir()?;
        let caller = if prompter_llm_enabled() {
            Some(PrompterCall::new()?)
        } else {
            None
        };
        Ok(LlmPrompterAgent {
            guides_dir,
            fallback: RuleBasedPromptMaker,
            caller,
        })
    }

    pub fn with_guides_dir(guides_dir: PathBuf) -> Self {
        let caller = if prompter_llm_enabled() {
            PrompterCall::new().ok()
        } else {
            None
        };
        LlmPrompterAgent {
            guides_dir,
            fallback: RuleBasedPromptMaker,
            caller,
        }
    }

    fn load_guide(&self, model_id: &str) -> Result<ModelPromptGuide, AetherError> {
        load_guide_for_model(model_id, &self.guides_dir)
    }

    /// Boucle LLM + CLI : l'agent peut afficher des messages et attendre l'entrée user.
    fn run_llm_with_cli_loop(
        &self,
        caller: &PrompterCall,
        guide: &ModelPromptGuide,
        user_request: &str,
        context: &PromptMakerContext,
    ) -> Result<PrompterOutput, AetherError> {
        let system = build_system_message(guide, &self.guides_dir)?;
        let initial_user = build_user_message(user_request, context);

        // Conversation history: system + user initial
        let mut conversation: Vec<Value> = vec![
            json!({ "role": "system", "content": system }),
            json!({ "role": "user", "content": initial_user }),
        ];

        for _turn in 0..MAX_CLI_TURNS {
            let outcome = caller.chat_with_tools_from_history(&conversation)?;

            match outcome {
                LlmTurnOutcome::ToolCalls(calls) => {
                    let mut got_finalize = false;
                    let mut got_cli_display = false;

                    for call in &calls {
                        match call.name.as_str() {
                            "display_cli_message" => {
                                got_cli_display = true;
                                let message = call
                                    .arguments
                                    .get("message")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let choices: Vec<String> = call
                                    .arguments
                                    .get("choices")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let wait = call
                                    .arguments
                                    .get("wait_for_input")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(true);

                                // Afficher le message à l'utilisateur
                                eprintln!("\n🤖 Prompter Agent: {}", message);
                                if !choices.is_empty() {
                                    eprintln!("   Choix possibles:");
                                    for (i, choice) in choices.iter().enumerate() {
                                        eprintln!("   {}) {}", i + 1, choice);
                                    }
                                }

                                if wait {
                                    // Attendre l'entrée utilisateur
                                    eprint!("   > ");
                                    io::stderr().flush().unwrap_or_default();
                                    let mut input = String::new();
                                    io::stdin().read_line(&mut input).map_err(|e| {
                                        AetherError::OperationFailed(format!(
                                            "Erreur lecture stdin: {}",
                                            e
                                        ))
                                    })?;
                                    let user_response = input.trim().to_string();

                                    // Ajouter tool call + tool response à la conversation
                                    conversation.push(json!({
                                        "role": "assistant",
                                        "content": null,
                                        "tool_calls": [{
                                            "id": call.id,
                                            "type": "function",
                                            "function": {
                                                "name": "display_cli_message",
                                                "arguments": serde_json::to_string(&call.arguments).unwrap_or_default()
                                            }
                                        }]
                                    }));
                                    conversation.push(json!({
                                        "role": "tool",
                                        "tool_call_id": call.id,
                                        "content": user_response
                                    }));
                                }
                            }
                            "finalize_prompt" => {
                                got_finalize = true;
                            }
                            "ask_clarification" => {
                                // Legacy support
                            }
                            _ => {}
                        }
                    }

                    // Si on a finalize_prompt, traiter et retourner
                    if got_finalize {
                        return process_tool_calls(guide, calls, context);
                    }

                    // Si on a fait un display_cli_message sans finalize, on continue la boucle
                    if got_cli_display {
                        continue;
                    }

                    // Sinon, erreur
                    return Err(AetherError::OperationFailed(
                        "Prompter LLM: outil inconnu ou finalize manquant".to_string(),
                    ));
                }
                LlmTurnOutcome::Text(t) => {
                    return Err(AetherError::OperationFailed(format!(
                        "Prompter LLM a répondu en texte sans outil — relancez ou précisez la demande. Extrait: {}",
                        truncate(&t, 200)
                    )));
                }
            }
        }

        Err(AetherError::OperationFailed(
            "Prompter LLM: nombre max de tours CLI atteint".to_string(),
        ))
    }

    fn from_guide(
        &self,
        guide: &ModelPromptGuide,
        kind: GenerationKind,
        user_request: &str,
        context: &PromptMakerContext,
    ) -> Result<ProfessionalPrompt, AetherError> {
        let model_id = &guide.model_id;
        let is_mock = context
            .target_model_id
            .as_deref()
            .map(|id| id.starts_with("mock/"))
            .unwrap_or(false);

        if is_mock {
            return self.fallback.make_prompt(kind, user_request, context);
        }

        let caller = self.caller.as_ref().ok_or_else(|| {
            AetherError::OperationFailed(
                "Prompter Agent LLM requis (agents.v1.json + bridge TS, ou AETHER_PROMPTER_LLM=0 + mock)"
                    .to_string(),
            )
        })?;

        let out = self.run_llm_with_cli_loop(caller, guide, user_request, context)?;

        let clarifications_json: Vec<Value> = out
            .clarifications
            .iter()
            .map(|c| {
                json!({
                    "field": c.field,
                    "question": c.question,
                    "suggested": c.suggested,
                })
            })
            .collect();

        Ok(ProfessionalPrompt {
            original_request: user_request.to_string(),
            professional_prompt: out.professional_prompt,
            negative_prompt: Some(
                "low quality, blurry, distorted, wrong aspect ratio, cropped subject".to_string(),
            ),
            locale: context.locale.clone(),
            style: context.style_hint.clone(),
            technical: json!({
                "prompter_model_id": model_id,
                "generation_kind": format!("{:?}", kind),
                "prompter_context": out.prompter_context,
                "api_options": out.api_options,
                "clarifications": clarifications_json,
                "enriched_by": "PrompterAgent",
            }),
        })
    }
}

fn build_user_message(user_request: &str, context: &PromptMakerContext) -> String {
    let mut parts = vec![format!("Demande client:\n{}", user_request)];
    if let Some(ref locale) = context.locale {
        parts.push(format!("Locale projet: {}", locale));
    }
    if let Some(ref style) = context.style_hint {
        parts.push(format!("Style hint: {}", style));
    }
    if let Some(ref summary) = context.project_summary {
        parts.push(format!("Résumé projet: {}", summary));
    }
    if let Some(ref vc) = context.vault_context {
        for v_ctx in &vc.vault_context {
            if !v_ctx.rules.is_empty() {
                parts.push(format!("Règles vault: {}", v_ctx.rules.join(" | ")));
            }
            if !v_ctx.prompt_snippets.is_empty() {
                parts.push(format!(
                    "Snippets vault: {}",
                    v_ctx.prompt_snippets.join(" | ")
                ));
            }
        }
    }
    if !context.explicit_options.is_null() && context.explicit_options != json!({}) {
        parts.push(format!(
            "Options explicites (prioritaires): {}",
            context.explicit_options
        ));
    }
    parts.join("\n\n")
}

fn process_tool_calls(
    guide: &ModelPromptGuide,
    calls: Vec<ToolCallResult>,
    context: &PromptMakerContext,
) -> Result<PrompterOutput, AetherError> {
    let mut finalize: Option<(String, Map<String, Value>)> = None;

    for call in calls {
        match call.name.as_str() {
            "finalize_prompt" => {
                let prompt = call
                    .arguments
                    .get("professional_prompt")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AetherError::OperationFailed(
                            "finalize_prompt: professional_prompt manquant".to_string(),
                        )
                    })?
                    .to_string();
                let params = call
                    .arguments
                    .get("parameters")
                    .and_then(|v| v.as_object())
                    .cloned()
                    .unwrap_or_default();
                finalize = Some((prompt, params));
            }
            "display_cli_message" | "ask_clarification" => {
                // Ignoré dans cette phase (déjà traité dans la boucle CLI)
            }
            other => {
                return Err(AetherError::OperationFailed(format!(
                    "Prompter Agent: outil inconnu '{}'",
                    other
                )));
            }
        }
    }

    let (professional_prompt, llm_params) = finalize.ok_or_else(|| {
        AetherError::OperationFailed(
            "Prompter Agent: aucun finalize_prompt après tour d'outils".to_string(),
        )
    })?;

    let params = merge_params_with_guide(guide, llm_params, &context.explicit_options)?;
    let mut api_root = Map::new();
    api_root.insert(guide.provider.clone(), Value::Object(params));

    Ok(PrompterOutput {
        professional_prompt,
        api_options: Value::Object(api_root),
        clarifications: vec![],
        prompter_context: guide.to_prompter_context(),
    })
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

impl PromptMaker for LlmPrompterAgent {
    fn make_prompt(
        &self,
        kind: GenerationKind,
        user_request: &str,
        context: &PromptMakerContext,
    ) -> Result<ProfessionalPrompt, AetherError> {
        let model_id = context.target_model_id.as_deref().ok_or_else(|| {
            AetherError::OperationFailed(
                "LlmPrompterAgent requires context.target_model_id".to_string(),
            )
        })?;

        if model_id.starts_with("mock/") {
            return self.fallback.make_prompt(kind, user_request, context);
        }

        match self.load_guide(model_id) {
            Ok(guide) if guide.supports_kind(kind) => {
                self.from_guide(&guide, kind, user_request, context)
            }
            Ok(_) => self.fallback.make_prompt(kind, user_request, context),
            Err(_) if kind != GenerationKind::ImageEdit && kind != GenerationKind::Image => {
                self.fallback.make_prompt(kind, user_request, context)
            }
            Err(e) => Err(e),
        }
    }
}

/// Alias historique.
pub type ModelPrompterAgent = LlmPrompterAgent;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompter::guide::resolve_guides_dir;

    #[test]
    fn test_system_prompt_header_and_complement() {
        let guides_dir = resolve_guides_dir().unwrap();
        let guide =
            load_guide_for_model("google/gemini-3.1-flash-image-preview", &guides_dir).unwrap();
        let sys = build_system_message(&guide, &guides_dir).unwrap();
        assert!(sys.contains("You are the AETHER"));
        assert!(!sys.contains("{model}"));
        assert!(sys.contains("Nano Banana"));
    }

    #[test]
    fn test_gemini_guide_loads_without_llm() {
        let guides_dir = resolve_guides_dir().unwrap();
        let agent = LlmPrompterAgent::with_guides_dir(guides_dir);
        let ctx = PromptMakerContext {
            project_summary: None,
            locale: Some("fr".to_string()),
            style_hint: None,
            vault_context: None,
            target_model_id: Some("mock/image-edit".to_string()),
            explicit_options: serde_json::json!({}),
        };

        let res = agent
            .make_prompt(
                GenerationKind::ImageEdit,
                "portrait de Lisa style concert, format 16:9 en 2K",
                &ctx,
            )
            .unwrap();

        assert!(res.professional_prompt.contains("16:9"));
        assert_eq!(res.technical["enriched_by"], "RuleBasedPromptMaker");
    }
}
