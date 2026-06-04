use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use aether_core::{
    AetherError, GeneratedArtifactKind, GenerationArtifact, GenerationJob, GenerationKind,
    GenerationRequest, GenerationStatus, ProviderModel,
};
use serde::{Deserialize, Serialize};

use crate::provider::{GenerationProvider, ProviderSubmitResult};

pub const BRIDGE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeOperation {
    ImageEdit,
    VideoGenerate,
    VoiceGenerate,
    MusicGenerate,
}

impl BridgeOperation {
    pub fn from_kind(kind: GenerationKind) -> Option<Self> {
        match kind {
            GenerationKind::ImageEdit => Some(BridgeOperation::ImageEdit),
            GenerationKind::VideoText
            | GenerationKind::VideoFrame
            | GenerationKind::VideoIngredients
            | GenerationKind::VideoEdit => Some(BridgeOperation::VideoGenerate),
            GenerationKind::Voice | GenerationKind::VoiceClone | GenerationKind::SceneAudio => {
                Some(BridgeOperation::VoiceGenerate)
            }
            GenerationKind::Music => Some(BridgeOperation::MusicGenerate),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            BridgeOperation::ImageEdit => "image_edit",
            BridgeOperation::VideoGenerate => "video_generate",
            BridgeOperation::VoiceGenerate => "voice_generate",
            BridgeOperation::MusicGenerate => "music_generate",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeRequest {
    pub version: u32,
    pub operation: String,
    pub model_id: String,
    #[serde(default)]
    pub bridge_handler: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub api_model: Option<String>,
    #[serde(default)]
    pub messages: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub tools: Option<serde_json::Value>,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub input_image_paths: Vec<String>,
    #[serde(default)]
    pub output_dir: String,
    #[serde(default)]
    pub options: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeArtifactOut {
    pub path: String,
    pub mime_type: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeSuccess {
    pub ok: bool,
    pub provider: String,
    pub provider_job_id: String,
    pub status: String,
    pub artifacts: Vec<BridgeArtifactOut>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeFailure {
    pub ok: bool,
    pub provider: String,
    pub error: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BridgeResponse {
    Success(BridgeSuccess),
    Failure(BridgeFailure),
}

/// Invokes the TypeScript API bridge (Node) with a JSON request on stdin.
pub fn invoke_bridge(req: &BridgeRequest, script: &Path) -> Result<BridgeResponse, AetherError> {
    let node = std::env::var("AETHER_NODE").unwrap_or_else(|_| "node".to_string());
    let mut child = Command::new(&node)
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            AetherError::OperationFailed(format!(
                "Failed to spawn node bridge ({}): {}",
                script.display(),
                e
            ))
        })?;

    let payload = serde_json::to_string(req).map_err(|e| {
        AetherError::OperationFailed(format!("Bridge request serialize failed: {}", e))
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.as_bytes())
            .map_err(|e| AetherError::OperationFailed(format!("Bridge stdin write failed: {}", e)))?;
    }

    let output = child.wait_with_output().map_err(|e| {
        AetherError::OperationFailed(format!("Bridge process wait failed: {}", e))
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() && stdout.trim().is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AetherError::OperationFailed(format!(
            "Bridge exited with {}: {}",
            output.status, stderr
        )));
    }

    serde_json::from_str(stdout.trim()).map_err(|e| {
        AetherError::OperationFailed(format!(
            "Bridge response parse failed: {} (stdout: {})",
            e,
            stdout.chars().take(500).collect::<String>()
        ))
    })
}

/// Resolves path to `services/aether-api-bridge/dist/index.js` from workspace root.
pub fn resolve_bridge_script() -> Result<PathBuf, AetherError> {
    if let Ok(p) = std::env::var("AETHER_API_BRIDGE_SCRIPT") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Ok(path);
        }
        return Err(AetherError::OperationFailed(format!(
            "AETHER_API_BRIDGE_SCRIPT not found: {}",
            path.display()
        )));
    }

    let mut dir = std::env::current_dir().ok();
    while let Some(ref d) = dir {
        let candidate = d.join("services/aether-api-bridge/dist/index.js");
        if candidate.is_file() {
            return Ok(candidate);
        }
        dir = d.parent().map(PathBuf::from);
    }

    Err(AetherError::OperationFailed(
        "API bridge script not found. Run: cd services/aether-api-bridge && npm install && npm run build"
            .to_string(),
    ))
}

#[derive(Debug)]
pub struct BridgeGenerationProvider {
    pub bridge_script: PathBuf,
    pub output_dir: PathBuf,
}

impl BridgeGenerationProvider {
    pub fn from_env(output_dir: PathBuf) -> Result<Self, AetherError> {
        if !output_dir.exists() {
            fs::create_dir_all(&output_dir).map_err(|e| {
                AetherError::IoError(output_dir.to_string_lossy().into_owned(), e.to_string())
            })?;
        }
        Ok(BridgeGenerationProvider {
            bridge_script: resolve_bridge_script()?,
            output_dir,
        })
    }

    fn build_request(
        &self,
        request: &GenerationRequest,
        model: &ProviderModel,
        prompt_text: &str,
    ) -> Result<BridgeRequest, AetherError> {
        let op = BridgeOperation::from_kind(request.kind).ok_or_else(|| {
            AetherError::OperationFailed(format!(
                "Bridge does not support generation kind {:?}",
                request.kind
            ))
        })?;

        let input_paths: Vec<String> = request
            .options
            .get("input_asset_paths")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if input_paths.is_empty() {
            return Err(AetherError::OperationFailed(
                "Bridge image_edit requires options.input_asset_paths (absolute file paths)"
                    .to_string(),
            ));
        }

        let bridge_handler = request
            .options
            .get("bridge_handler")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| {
                model
                    .capabilities
                    .get("bridge_handler")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            });

        Ok(BridgeRequest {
            version: BRIDGE_VERSION,
            operation: op.as_str().to_string(),
            model_id: model.id.clone(),
            bridge_handler,
            agent: None,
            provider: Some(model.provider.clone()),
            api_model: model
                .capabilities
                .get("api_model")
                .and_then(|v| v.as_str())
                .map(String::from),
            messages: None,
            tools: None,
            prompt: prompt_text.to_string(),
            input_image_paths: input_paths,
            output_dir: self.output_dir.to_string_lossy().into_owned(),
            options: request.options.clone(),
        })
    }

    fn invoke_for_job(
        &self,
        request: &GenerationRequest,
        model: &ProviderModel,
        prompt_text: &str,
    ) -> Result<BridgeSuccess, AetherError> {
        let bridge_req = self.build_request(request, model, prompt_text)?;
        let resp = invoke_bridge(&bridge_req, &self.bridge_script)?;

        match resp {
            BridgeResponse::Success(s) if s.ok => Ok(s),
            BridgeResponse::Failure(f) => Err(AetherError::OperationFailed(format!(
                "Bridge {} failed: {}",
                f.provider, f.error
            ))),
            BridgeResponse::Success(_) => Err(AetherError::OperationFailed(
                "Bridge returned ok=false in success variant".to_string(),
            )),
        }
    }

    fn artifacts_from_success(&self, success: &BridgeSuccess) -> Vec<GenerationArtifact> {
        success
            .artifacts
            .iter()
            .map(|a| GenerationArtifact {
                kind: GeneratedArtifactKind::Image,
                path: PathBuf::from(&a.path),
                asset_ref: None,
                mime_type: Some(a.mime_type.clone()),
                metadata: serde_json::json!({
                    "provider": success.provider,
                    "bridge": true,
                    "provider_metadata": success.metadata,
                    "artifact_metadata": a.metadata,
                }),
            })
            .collect()
    }
}

impl GenerationProvider for BridgeGenerationProvider {
    fn provider_name(&self) -> &'static str {
        "bridge"
    }

    fn supports(&self, model: &ProviderModel) -> bool {
        model.provider == "openai"
            || model.provider == "google"
            || model.provider == "bytedance"
            || model.provider == "kuaishou"
            || model.provider == "elevenlabs"
            || model.provider == "minimax"
    }

    fn submit(
        &self,
        request: &GenerationRequest,
    ) -> Result<ProviderSubmitResult, AetherError> {
        let prompt_text = request
            .options
            .get("professional_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or(&request.user_request)
            .to_string();

        let model = request
            .options
            .get("resolved_model")
            .and_then(|v| serde_json::from_value::<ProviderModel>(v.clone()).ok())
            .ok_or_else(|| {
                AetherError::OperationFailed(
                    "Bridge submit requires options.resolved_model".to_string(),
                )
            })?;

        let success = self.invoke_for_job(request, &model, &prompt_text)?;
        let job_id = success.provider_job_id.clone();

        let cache_path = self.output_dir.join(format!("{}.bridge.json", job_id));
        fs::write(
            &cache_path,
            serde_json::to_string(&success).unwrap_or_default(),
        )
        .map_err(|e| AetherError::IoError(cache_path.display().to_string(), e.to_string()))?;

        Ok(ProviderSubmitResult {
            provider_job_id: job_id,
            status: GenerationStatus::Ready,
        })
    }

    fn status(&self, _job: &GenerationJob) -> Result<GenerationStatus, AetherError> {
        Ok(GenerationStatus::Ready)
    }

    fn download(&self, job: &GenerationJob) -> Result<Vec<GenerationArtifact>, AetherError> {
        let job_id = job
            .provider_job_id
            .as_deref()
            .ok_or_else(|| AetherError::OperationFailed("Missing provider_job_id".to_string()))?;

        let cache_path = self.output_dir.join(format!("{}.bridge.json", job_id));
        let raw = fs::read_to_string(&cache_path).map_err(|e| {
            AetherError::IoError(cache_path.display().to_string(), e.to_string())
        })?;
        let success: BridgeSuccess = serde_json::from_str(&raw).map_err(|e| {
            AetherError::OperationFailed(format!("Bridge cache parse failed: {}", e))
        })?;

        Ok(self.artifacts_from_success(&success))
    }

    fn cancel(&self, _job: &GenerationJob) -> Result<(), AetherError> {
        Ok(())
    }
}
