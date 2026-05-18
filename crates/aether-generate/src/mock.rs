use std::fs;
use std::path::PathBuf;
use aether_core::{
    AetherError, GeneratedArtifactKind, GenerationArtifact, GenerationJob,
    GenerationKind, GenerationRequest, GenerationStatus, ProviderModel,
};
use crate::provider::{GenerationProvider, ProviderSubmitResult};

#[derive(Debug, Clone)]
pub struct MockProvider {
    pub output_dir: PathBuf,
}

impl MockProvider {
    /// Invariant: must create the output directory if it does not exist and return a valid MockProvider instance.
    pub fn new(output_dir: PathBuf) -> Self {
        if !output_dir.exists() {
            let _ = fs::create_dir_all(&output_dir);
        }
        MockProvider { output_dir }
    }
}

impl GenerationProvider for MockProvider {
    /// Invariant: must return a static string "mock".
    fn provider_name(&self) -> &'static str {
        "mock"
    }

    /// Invariant: must support any model whose provider field is "mock".
    fn supports(&self, model: &ProviderModel) -> bool {
        model.provider == "mock"
    }

    /// Invariant: must successfully submit a request and return a deterministic ProviderSubmitResult with Ready status and a job ID prefix.
    fn submit(&self, request: &GenerationRequest) -> Result<ProviderSubmitResult, AetherError> {
        let provider_job_id = format!("mock-job-{}", request.job_ref.id);
        Ok(ProviderSubmitResult {
            provider_job_id,
            status: GenerationStatus::Ready,
        })
    }

    /// Invariant: must return GenerationStatus::Ready deterministically for any mock job.
    fn status(&self, _job: &GenerationJob) -> Result<GenerationStatus, AetherError> {
        Ok(GenerationStatus::Ready)
    }

    /// Invariant: must write deterministic mock metadata/payload JSON files to output_dir and return a vector of generated artifacts based on the GenerationKind.
    fn download(&self, job: &GenerationJob) -> Result<Vec<GenerationArtifact>, AetherError> {
        let job_id = job.job_ref.id;
        let kind = job.kind;

        let (filename, artifact_kind, file_content) = match kind {
            GenerationKind::StoryboardScratch => (
                format!("storyboard_{}.json", job_id),
                GeneratedArtifactKind::StoryboardJson,
                r#"{"scenes": [{"panel": 1, "description": "Luxe futuriste, flacon noir", "duration": 5}]}"#.to_string(),
            ),
            GenerationKind::Dialogue => (
                format!("dialogue_{}.json", job_id),
                GeneratedArtifactKind::DialogueJson,
                r#"{"dialogue": [{"character": "Hero", "line": "Le mystère commence."}]}"#.to_string(),
            ),
            GenerationKind::Image | GenerationKind::ImageEdit => (
                format!("image_{}.mock-image.json", job_id),
                GeneratedArtifactKind::Image,
                format!(r#"{{"mock_image": true, "prompt": {:?}}}"#, job.prompt.as_ref().map(|p| &p.professional_prompt)),
            ),
            GenerationKind::Voice | GenerationKind::VoiceClone | GenerationKind::SceneAudio => (
                format!("audio_{}.mock-audio.json", job_id),
                GeneratedArtifactKind::Audio,
                r#"{"mock_audio": true, "sample_rate": 44100}"#.to_string(),
            ),
            GenerationKind::Music => (
                format!("music_{}.mock-music.json", job_id),
                GeneratedArtifactKind::Music,
                r#"{"mock_music": true, "tempo": 120}"#.to_string(),
            ),
            _ => (
                format!("video_{}.mock-video.json", job_id),
                GeneratedArtifactKind::Video,
                r#"{"mock_video": true, "fps": 30, "duration_sec": 4}"#.to_string(),
            ),
        };

        let file_path = self.output_dir.join(&filename);
        fs::write(&file_path, file_content).map_err(|e| {
            AetherError::IoError(file_path.to_string_lossy().into_owned(), e.to_string())
        })?;

        let artifact = GenerationArtifact {
            kind: artifact_kind,
            path: file_path,
            asset_ref: None,
            mime_type: Some("application/json".to_string()),
            metadata: serde_json::json!({
                "provider": "mock",
                "mock_generated": true
            }),
        };

        Ok(vec![artifact])
    }

    /// Invariant: must return Ok(()) deterministically.
    fn cancel(&self, _job: &GenerationJob) -> Result<(), AetherError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::Ref;

    #[test]
    fn test_mock_provider() {
        let temp_dir = std::env::temp_dir().join("aether_mock_provider_test");
        let provider = MockProvider::new(temp_dir.clone());

        let job_ref = "@g5".parse::<Ref>().unwrap();
        let request = GenerationRequest {
            job_ref,
            kind: GenerationKind::Image,
            user_request: "a sunset".to_string(),
            model: None,
            inputs: Vec::new(),
            options: serde_json::json!({}),
        };

        // Submit
        let submit_res = provider.submit(&request).unwrap();
        assert_eq!(submit_res.status, GenerationStatus::Ready);
        assert_eq!(submit_res.provider_job_id, "mock-job-5");

        // Download
        let job = GenerationJob {
            job_ref,
            kind: GenerationKind::Image,
            status: GenerationStatus::Ready,
            requested_model: None,
            resolved_model: None,
            provider_job_id: Some(submit_res.provider_job_id),
            prompt: None,
            inputs: Vec::new(),
            artifacts: Vec::new(),
            error: None,
            created_at_ms: 0,
            updated_at_ms: 0,
            options: serde_json::json!({}),
        };

        let artifacts = provider.download(&job).unwrap();
        assert_eq!(artifacts.len(), 1);
        let art = &artifacts[0];
        assert_eq!(art.kind, GeneratedArtifactKind::Image);
        assert!(art.path.exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}

