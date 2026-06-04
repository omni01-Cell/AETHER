use aether_core::AetherError;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone)]
pub enum LlmTurnOutcome {
    ToolCalls(Vec<ToolCallResult>),
    Text(String),
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: Option<String>,
    function: OpenAiFunctionCall,
}

#[derive(Debug, Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

pub fn parse_chat_completion_response(raw: &str) -> Result<LlmTurnOutcome, AetherError> {
    let parsed: OpenAiChatResponse = serde_json::from_str(raw).map_err(|e| {
        AetherError::OperationFailed(format!(
            "Chat completion invalid JSON: {} — body: {}",
            e,
            truncate(raw, 400)
        ))
    })?;

    let message = parsed
        .choices
        .first()
        .map(|c| &c.message)
        .ok_or_else(|| AetherError::OperationFailed("Chat completion: empty choices".to_string()))?;

    if let Some(ref calls) = message.tool_calls {
        if !calls.is_empty() {
            let mut out = Vec::new();
            for (i, tc) in calls.iter().enumerate() {
                let args: Value = serde_json::from_str(&tc.function.arguments).map_err(|e| {
                    AetherError::OperationFailed(format!(
                        "Tool args parse ({}): {}",
                        tc.function.name, e
                    ))
                })?;
                let id = tc
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("call_{}", i));
                out.push(ToolCallResult {
                    id,
                    name: tc.function.name.clone(),
                    arguments: args,
                });
            }
            return Ok(LlmTurnOutcome::ToolCalls(out));
        }
    }

    if let Some(ref content) = message.content {
        if !content.trim().is_empty() {
            return Ok(LlmTurnOutcome::Text(content.clone()));
        }
    }

    Err(AetherError::OperationFailed(
        "Chat completion: no tool_calls and no content".to_string(),
    ))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
