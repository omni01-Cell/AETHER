use aether_core::{
    AetherError, GenerationArtifact, GenerationJob, GenerationRequest,
    GenerationStatus, ProviderModel,
};

#[derive(Debug, Clone)]
pub struct ProviderSubmitResult {
    pub provider_job_id: String,
    pub status: GenerationStatus,
}

pub trait GenerationProvider: Send + Sync {
    /// Invariant: must return a static string representing the provider's unique name.
    fn provider_name(&self) -> &'static str;

    /// Invariant: must return true if the specified model is supported by this provider, and false otherwise.
    fn supports(&self, model: &ProviderModel) -> bool;

    /// Invariant: must submit a generation request to the provider backend and return the submission result, or return a proper AetherError on failure.
    fn submit(&self, request: &GenerationRequest) -> Result<ProviderSubmitResult, AetherError>;

    /// Invariant: must query the provider backend for the current status of the specified job and return it, preserving system state.
    fn status(&self, job: &GenerationJob) -> Result<GenerationStatus, AetherError>;

    /// Invariant: must download the generated artifacts of the completed job to the local filesystem and return the metadata list.
    fn download(&self, job: &GenerationJob) -> Result<Vec<GenerationArtifact>, AetherError>;

    /// Invariant: must request cancellation of the specified active job on the provider backend, transitioning its state to Cancelled if successful.
    fn cancel(&self, job: &GenerationJob) -> Result<(), AetherError>;
}
