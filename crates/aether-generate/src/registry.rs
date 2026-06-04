use aether_core::{GenerationKind, ProviderModel};

fn env_present(key: &str) -> bool {
    std::env::var(key)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

fn openai_enabled() -> bool {
    env_present("AETHER_OPENAI_API_KEY") || env_present("OPENAI_API_KEY")
}

fn google_enabled() -> bool {
    env_present("AETHER_GOOGLE_API_KEY")
        || env_present("GEMINI_API_KEY")
        || env_present("GOOGLE_API_KEY")
}

fn bytedance_enabled() -> bool {
    env_present("AETHER_BYTEDANCE_API_KEY")
        || env_present("SEEDANCE_API_KEY")
        || env_present("VOLCENGINE_API_KEY")
}

fn kuaishou_enabled() -> bool {
    env_present("AETHER_KUAISHOU_API_KEY")
        || env_present("KLING_API_KEY")
}

fn elevenlabs_enabled() -> bool {
    env_present("ELEVENLABS_API_KEY")
}

fn minimax_enabled() -> bool {
    env_present("FAL_API_KEY") || env_present("MINIMAX_API_KEY")
}

#[derive(Debug, Clone, Default)]
pub struct ModelRegistry {
    pub models: Vec<ProviderModel>,
}

impl ModelRegistry {
    /// Invariant: must construct and return a ModelRegistry pre-populated with builtin placeholder models (mock enabled, real disabled).
    pub fn with_builtin_placeholders() -> Self {
        let mut models = Vec::new();

        // Mocks (enabled)
        let mock_models = vec![
            ("mock/storyboard", GenerationKind::StoryboardScratch),
            ("mock/dialogue", GenerationKind::Dialogue),
            ("mock/image", GenerationKind::Image),
            ("mock/image-edit", GenerationKind::ImageEdit),
            ("mock/voice", GenerationKind::Voice),
            ("mock/voice-clone", GenerationKind::VoiceClone),
            ("mock/scene-audio", GenerationKind::SceneAudio),
            ("mock/music", GenerationKind::Music),
            ("mock/video-text", GenerationKind::VideoText),
            ("mock/video-frame", GenerationKind::VideoFrame),
            ("mock/video-ingredients", GenerationKind::VideoIngredients),
            ("mock/video-edit", GenerationKind::VideoEdit),
        ];

        for (id, kind) in mock_models {
            models.push(ProviderModel {
                id: id.to_string(),
                provider: "mock".to_string(),
                kind,
                enabled: true,
                capabilities: serde_json::json!({}),
            });
        }

        // Real models (disabled by default, enabled via API keys)
        let real_models = vec![
            ("google/gemini-flash-tts", "google", GenerationKind::Voice),
            ("google/lyria", "google", GenerationKind::Music),
            ("google/veo-3", "google", GenerationKind::VideoText),
            ("google/veo-3.1", "google", GenerationKind::VideoText),
            ("google/imagen", "google", GenerationKind::Image),
        ];

        for (id, provider, kind) in real_models {
            models.push(ProviderModel {
                id: id.to_string(),
                provider: provider.to_string(),
                kind,
                enabled: false,
                capabilities: serde_json::json!({}),
            });
        }

        // Seedance 2.0 (ByteDance) — video generation
        let seedance_kinds = vec![
            GenerationKind::VideoText,
            GenerationKind::VideoFrame,
            GenerationKind::VideoIngredients,
            GenerationKind::VideoEdit,
        ];
        for kind in seedance_kinds {
            models.push(ProviderModel {
                id: "bytedance/seedance-2.0".to_string(),
                provider: "bytedance".to_string(),
                kind,
                enabled: bytedance_enabled(),
                capabilities: serde_json::json!({
                    "api_model": "seedance-2.0",
                    "display_name": "Seedance 2.0",
                    "bridge_handler": "seedance-video",
                    "prompt_guide": "bytedance/seedance-2.0.json"
                }),
            });
        }

        // Kling 3.0 (Kuaishou) — video generation
        let kling_kinds = vec![
            GenerationKind::VideoText,
            GenerationKind::VideoFrame,
            GenerationKind::VideoIngredients,
        ];
        for kind in kling_kinds {
            models.push(ProviderModel {
                id: "kuaishou/kling-3.0".to_string(),
                provider: "kuaishou".to_string(),
                kind,
                enabled: kuaishou_enabled(),
                capabilities: serde_json::json!({
                    "api_model": "kling-3.0",
                    "display_name": "Kling 3.0",
                    "bridge_handler": "kling-video",
                    "prompt_guide": "kuaishou/kling-3.0.json"
                }),
            });
        }

        // ElevenLabs — voice generation (primary)
        let elevenlabs_kinds = vec![
            GenerationKind::Voice,
            GenerationKind::VoiceClone,
        ];
        for kind in elevenlabs_kinds {
            models.push(ProviderModel {
                id: "elevenlabs/eleven-v3".to_string(),
                provider: "elevenlabs".to_string(),
                kind,
                enabled: elevenlabs_enabled(),
                capabilities: serde_json::json!({
                    "api_model": "eleven_v3",
                    "display_name": "ElevenLabs Eleven v3",
                    "bridge_handler": "elevenlabs-tts",
                    "prompt_guide": "elevenlabs/eleven-v3.json"
                }),
            });
        }

        // Google Gemini TTS — voice generation (fallback)
        models.push(ProviderModel {
            id: "google/gemini-3.1-flash-tts".to_string(),
            provider: "google".to_string(),
            kind: GenerationKind::Voice,
            enabled: google_enabled(),
            capabilities: serde_json::json!({
                "api_model": "gemini-3.1-flash-tts",
                "display_name": "Google Gemini 3.1 Flash TTS",
                "bridge_handler": "gemini-tts",
                "prompt_guide": "google/gemini-3.1-flash-tts.json"
            }),
        });

        // OpenAI TTS — voice generation (fallback)
        models.push(ProviderModel {
            id: "openai/tts-1-hd".to_string(),
            provider: "openai".to_string(),
            kind: GenerationKind::Voice,
            enabled: openai_enabled(),
            capabilities: serde_json::json!({
                "api_model": "tts-1-hd",
                "display_name": "OpenAI TTS-1 HD",
                "bridge_handler": "openai-tts",
                "prompt_guide": "openai/tts-1-hd.json"
            }),
        });

        // MiniMax Music — music generation
        models.push(ProviderModel {
            id: "minimax/music-2.5".to_string(),
            provider: "minimax".to_string(),
            kind: GenerationKind::Music,
            enabled: minimax_enabled(),
            capabilities: serde_json::json!({
                "api_model": "minimax-music-v2",
                "display_name": "MiniMax Music 2.5",
                "bridge_handler": "minimax-music",
                "prompt_guide": "minimax/music-2.5.json"
            }),
        });

        // Image edit — primary GPT Image 2, fallback Gemini 3.1 flash image (guides: prompt-guides/)
        models.push(ProviderModel {
            id: "openai/gpt-image-2".to_string(),
            provider: "openai".to_string(),
            kind: GenerationKind::ImageEdit,
            enabled: openai_enabled(),
            capabilities: serde_json::json!({
                "api_model": "gpt-image-2",
                "fallback_model_id": "google/gemini-3.1-flash-image-preview",
                "bridge_handler": "openai-image-edit",
                "prompt_guide": "openai/gpt-image-2.json"
            }),
        });

        models.push(ProviderModel {
            id: "google/gemini-3.1-flash-image-preview".to_string(),
            provider: "google".to_string(),
            kind: GenerationKind::ImageEdit,
            enabled: google_enabled(),
            capabilities: serde_json::json!({
                "api_model": "gemini-3.1-flash-image-preview",
                "display_name": "Nano Banana 2",
                "bridge_handler": "nano-banana-image-edit",
                "prompt_guide": "google/gemini-3.1-flash-image-preview.json"
            }),
        });

        models.push(ProviderModel {
            id: "google/nano-banana-2".to_string(),
            provider: "google".to_string(),
            kind: GenerationKind::ImageEdit,
            enabled: google_enabled(),
            capabilities: serde_json::json!({
                "api_model": "gemini-3.1-flash-image-preview",
                "display_name": "Nano Banana 2",
                "bridge_handler": "nano-banana-image-edit",
                "prompt_guide": "google/nano-banana-2.json",
                "alias_of": "google/gemini-3.1-flash-image-preview"
            }),
        });

        ModelRegistry { models }
    }

    /// Default model id for a kind when the user does not pass `--model`.
    pub fn default_model_id(&self, kind: GenerationKind) -> Option<String> {
        match kind {
            GenerationKind::ImageEdit if openai_enabled() => {
                Some("openai/gpt-image-2".to_string())
            }
            GenerationKind::ImageEdit if google_enabled() => {
                Some("google/gemini-3.1-flash-image-preview".to_string())
            }
            GenerationKind::VideoText
            | GenerationKind::VideoFrame
            | GenerationKind::VideoIngredients
            | GenerationKind::VideoEdit => {
                if kuaishou_enabled() {
                    Some("kuaishou/kling-3.0".to_string())
                } else if bytedance_enabled() {
                    Some("bytedance/seedance-2.0".to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Invariant: must find and return an enabled model compatible with the given GenerationKind, prioritizing the requested_model if provided and enabled.
    pub fn find_model(&self, kind: GenerationKind, requested_model: Option<&str>) -> Option<ProviderModel> {
        if let Some(req_id) = requested_model {
            if let Some(m) = self.models.iter().find(|m| m.id == req_id && m.kind == kind && m.enabled) {
                return Some(m.clone());
            }
        }
        if let Some(default_id) = self.default_model_id(kind) {
            if let Some(m) = self
                .models
                .iter()
                .find(|m| m.id == default_id && m.kind == kind && m.enabled)
            {
                return Some(m.clone());
            }
        }
        // Fallback to first enabled model of this kind (mock first in list order)
        self.models.iter().find(|m| m.kind == kind && m.enabled).cloned()
    }

    /// Invariant: must return a list of all models matching the specified GenerationKind.
    pub fn list_by_kind(&self, kind: GenerationKind) -> Vec<ProviderModel> {
        self.models.iter().filter(|m| m.kind == kind).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_registry() {
        let registry = ModelRegistry::with_builtin_placeholders();
        // Check that a mock model is found and enabled
        let mock_video = registry.find_model(GenerationKind::VideoText, None).unwrap();
        assert_eq!(mock_video.provider, "mock");
        assert_eq!(mock_video.id, "mock/video-text");
        assert!(mock_video.enabled);

        // Check listing by kind
        let list = registry.list_by_kind(GenerationKind::VideoText);
        // Includes mock/video-text, google/veo-3, google/veo-3.1
        assert!(list.iter().any(|m| m.id == "mock/video-text"));
        assert!(list.iter().any(|m| m.id == "google/veo-3"));

        // Google veo-3 is disabled
        let google_veo = list.iter().find(|m| m.id == "google/veo-3").unwrap();
        assert!(!google_veo.enabled);

        // Trying to find google/veo-3 should fallback to mock/video-text because google/veo-3 is disabled
        let resolved = registry.find_model(GenerationKind::VideoText, Some("google/veo-3")).unwrap();
        assert_eq!(resolved.id, "mock/video-text");
    }

    #[test]
    fn test_image_edit_route_has_nano_banana_fallback() {
        let registry = ModelRegistry::with_builtin_placeholders();
        let route = registry
            .resolve_route(GenerationKind::ImageEdit, None)
            .expect("route when mock or real enabled");

        if route.primary.model.id == "openai/gpt-image-2" {
            let fb = route.fallback.expect("fallback model");
            assert_eq!(fb.model.id, "google/gemini-3.1-flash-image-preview");
            assert_eq!(fb.bridge_handler, "nano-banana-image-edit");
        }
    }
}

