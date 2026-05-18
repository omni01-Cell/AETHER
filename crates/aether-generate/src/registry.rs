use aether_core::{GenerationKind, ProviderModel};

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

        // Real models (disabled)
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

        ModelRegistry { models }
    }

    /// Invariant: must find and return an enabled model compatible with the given GenerationKind, prioritizing the requested_model if provided and enabled.
    pub fn find_model(&self, kind: GenerationKind, requested_model: Option<&str>) -> Option<ProviderModel> {
        if let Some(req_id) = requested_model {
            if let Some(m) = self.models.iter().find(|m| m.id == req_id && m.kind == kind && m.enabled) {
                return Some(m.clone());
            }
        }
        // Fallback to first enabled model of this kind
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
}

