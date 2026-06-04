use serde_json::{json, Value};

/// Outils OpenAI function-calling de l'agent prompter (équivalent `tools=[...]` en Python).
pub fn prompter_tools_schema() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "display_cli_message",
                "description": "Display a message to the user in the CLI. Use this to ask for clarification, show progress, or request missing information. The message will be printed to the terminal and the agent will wait for the user's response.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "The message to display to the user"
                        },
                        "field": {
                            "type": "string",
                            "description": "The API parameter or concept this message is about"
                        },
                        "choices": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Suggested choices for the user (optional)"
                        },
                        "wait_for_input": {
                            "type": "boolean",
                            "description": "Whether to wait for user input after displaying the message (default: true)"
                        }
                    },
                    "required": ["message"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "finalize_prompt",
                "description": "Submit the final generation API order: prompt text and API parameters. Call this when all information is gathered and the prompt is ready.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "professional_prompt": {
                            "type": "string",
                            "description": "Exact prompt sent to the generation model (image/video/voice/music)"
                        },
                        "parameters": {
                            "type": "object",
                            "description": "API parameters for this provider (include api_model)"
                        }
                    },
                    "required": ["professional_prompt", "parameters"]
                }
            }
        }
    ])
}
