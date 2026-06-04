use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU32, Ordering};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod keyframes;

/// Trait representing a rendering backend for composition graphs.
pub trait RenderBackend: Send + Sync {
    /// Render a composition graph to a raw RGBA buffer of width * height * 4 bytes.
    fn render(
        &self,
        graph: &CompositionGraph,
        width: u32,
        height: u32,
        registry: &RefRegistry,
    ) -> Result<Vec<u8>, AetherError>;
}

/// Error type for the AETHER multimedia engine.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AetherError {
    #[error("Invalid Reference '{0}': {1}")]
    InvalidRef(String, String),

    #[error("Reference '{0}' not found in registry")]
    RefNotFound(Ref),

    #[error("Reference '{0}' already registered")]
    RefAlreadyExists(Ref),

    #[error("Invalid Command: {0}")]
    InvalidCommand(String),

    #[error("IO Error on path '{0}': {1}")]
    IoError(String, String),

    #[error("Database Error: {0}")]
    DatabaseError(String),

    #[error("Multimedia Processing Error: {0}")]
    MediaError(String),

    #[error("Operation Failed: {0}")]
    OperationFailed(String),

    #[error("Vault Error: {0}")]
    VaultError(String),

    #[error("Prompter needs clarification: {0}")]
    PrompterNeedsClarification(String),
}

/// The kind of multimedia asset reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RefKind {
    Video,
    Audio,
    Image,
    Animation,
    Generated,
}

/// A semantic reference to a multimedia asset (e.g. `@v1`, `@a2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ref {
    pub kind: RefKind,
    pub id: u32,
}

impl fmt::Display for Ref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.kind {
            RefKind::Video => "v",
            RefKind::Audio => "a",
            RefKind::Image => "img",
            RefKind::Animation => "anim",
            RefKind::Generated => "g",
        };
        write!(f, "@{}{}", prefix, self.id)
    }
}

impl FromStr for Ref {
    type Err = AetherError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('@') {
            return Err(AetherError::InvalidRef(
                s.to_string(),
                "Must start with '@'".to_string(),
            ));
        }
        let rest = &s[1..];
        let mut prefix = String::new();
        let mut num_str = String::new();

        for c in rest.chars() {
            if c.is_ascii_alphabetic() {
                if !num_str.is_empty() {
                    return Err(AetherError::InvalidRef(
                        s.to_string(),
                        "Letters cannot follow digits".to_string(),
                    ));
                }
                prefix.push(c);
            } else if c.is_ascii_digit() {
                num_str.push(c);
            } else {
                return Err(AetherError::InvalidRef(
                    s.to_string(),
                    format!("Invalid character '{}' in reference", c),
                ));
            }
        }

        if prefix.is_empty() {
            return Err(AetherError::InvalidRef(
                s.to_string(),
                "Missing alphabetic prefix".to_string(),
            ));
        }
        if num_str.is_empty() {
            return Err(AetherError::InvalidRef(
                s.to_string(),
                "Missing numeric identifier".to_string(),
            ));
        }

        let id = num_str
            .parse::<u32>()
            .map_err(|e| AetherError::InvalidRef(s.to_string(), e.to_string()))?;

        let kind = match prefix.as_str() {
            "v" => RefKind::Video,
            "a" => RefKind::Audio,
            "img" => RefKind::Image,
            "anim" => RefKind::Animation,
            "g" => RefKind::Generated,
            other => {
                return Err(AetherError::InvalidRef(
                    s.to_string(),
                    format!("Unknown reference prefix '{}'", other),
                ));
            }
        };

        Ok(Ref { kind, id })
    }
}

impl Serialize for Ref {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Ref {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RefVisitor;

        impl<'vi> serde::de::Visitor<'vi> for RefVisitor {
            type Value = Ref;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a semantic reference starting with '@'")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                v.parse::<Ref>().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(RefVisitor)
    }
}

/// The category of the underlying asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    Video,
    Audio,
    Image,
    Animation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartObservation {
    pub asset_ref: Ref,
    pub asset_kind: AssetKind,
    pub duration_sec: f32,
    // Médias légers (Le Cerveau Droit)
    pub proxy_video_path: Option<String>,
    pub proxy_audio_path: Option<String>,
    pub contact_sheet_path: Option<String>,
    // Télémétrie absolue (Le Cerveau Gauche)
    pub telemetry: TelemetryData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryData {
    pub anomalies: Vec<String>,
    pub audio_peaks_sec: Vec<f32>, // Timestamps des beats/transients
    pub rms_levels: Vec<f32>,
}

/// Metadata and path of a registered asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub r: Ref,
    pub kind: AssetKind,
    pub path: PathBuf,
    pub hash: String,
    pub metadata: serde_json::Value,
}

/// Thread-safe registry for allocating, registering, and resolving references.
#[derive(Debug)]
pub struct RefRegistry {
    // Atomic counters for each RefKind (starting from 1)
    counters: HashMap<RefKind, AtomicU32>,
    resolved: RwLock<HashMap<Ref, Asset>>,
}

impl RefRegistry {
    /// Creates a new empty `RefRegistry`.
    pub fn new() -> Self {
        let mut counters = HashMap::new();
        counters.insert(RefKind::Video, AtomicU32::new(1));
        counters.insert(RefKind::Audio, AtomicU32::new(1));
        counters.insert(RefKind::Image, AtomicU32::new(1));
        counters.insert(RefKind::Animation, AtomicU32::new(1));
        counters.insert(RefKind::Generated, AtomicU32::new(1));

        RefRegistry {
            counters,
            resolved: RwLock::new(HashMap::new()),
        }
    }

    /// Allocates a new unused reference of the specified kind.
    pub fn allocate(&self, kind: RefKind) -> Ref {
        let counter = self.counters.get(&kind).expect("Counter initialized");
        let id = counter.fetch_add(1, Ordering::SeqCst);
        Ref { kind, id }
    }

    /// Reserves the next reference of the specified kind without permanently registering it yet.
    pub fn reserve_next(&self, kind: RefKind) -> Ref {
        // Invariant: Reserves the next available Ref ID by allocating it; if the transaction rolls back, registry state restoration will reclaim it.
        self.allocate(kind)
    }

    /// Registers an asset to a specific reference.
    pub fn register(&self, r: Ref, asset: Asset) -> Result<(), AetherError> {
        let mut map = self.resolved.write().unwrap();
        if map.contains_key(&r) {
            return Err(AetherError::RefAlreadyExists(r));
        }
        map.insert(r, asset);
        Ok(())
    }

    /// Resolves an asset reference.
    pub fn resolve(&self, r: &Ref) -> Result<Asset, AetherError> {
        let map = self.resolved.read().unwrap();
        map.get(r).cloned().ok_reachable(*r)
    }

    /// Frees an asset reference.
    pub fn free(&self, r: &Ref) -> bool {
        let mut map = self.resolved.write().unwrap();
        map.remove(r).is_some()
    }

    /// Gets a snapshot list of all registered assets.
    pub fn list_assets(&self) -> Vec<Asset> {
        let map = self.resolved.read().unwrap();
        map.values().cloned().collect()
    }

    /// Returns a copy of the registry's current state (counters and resolved assets).
    pub fn get_state(&self) -> (HashMap<RefKind, u32>, HashMap<Ref, Asset>) {
        // Invariant: Returns a complete clone of the registry's internal counters and resolved assets map.
        let mut counters_val = HashMap::new();
        for (&kind, atom) in &self.counters {
            counters_val.insert(kind, atom.load(Ordering::SeqCst));
        }
        let resolved_val = self.resolved.read().unwrap().clone();
        (counters_val, resolved_val)
    }

    /// Overwrites the registry's state with the provided counters and resolved assets.
    pub fn set_state(&self, state: (HashMap<RefKind, u32>, HashMap<Ref, Asset>)) {
        // Invariant: Overwrites both the atomic counters and the resolved assets map with the provided state.
        let (counters_val, resolved_val) = state;
        for (kind, val) in counters_val {
            if let Some(atom) = self.counters.get(&kind) {
                atom.store(val, Ordering::SeqCst);
            }
        }
        *self.resolved.write().unwrap() = resolved_val;
    }
}

impl Default for RefRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper extension trait to resolve Option into Result for references.
trait ResolveExt {
    fn ok_reachable(self, r: Ref) -> Result<Asset, AetherError>;
}

impl ResolveExt for Option<Asset> {
    fn ok_reachable(self, r: Ref) -> Result<Asset, AetherError> {
        match self {
            Some(a) => Ok(a),
            None => Err(AetherError::RefNotFound(r)),
        }
    }
}

/// Global settings of an AETHER project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub fps: f32,
    pub width: u32,
    pub height: u32,
    pub colorspace: String,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        ProjectSettings {
            fps: 30.0,
            width: 1920,
            height: 1080,
            colorspace: "srgb".to_string(),
        }
    }
}

/// Metadata and settings of a specific AETHER project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub schema_version: u32,
    pub project_id: String,
    pub name: String,
    pub root: PathBuf,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub aether_version: String,
    pub description: Option<String>,
}

/// The run-time status of a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectStatus {
    Open,
    Closed,
    Missing,
}

/// An entry in the global user projects registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectRegistryEntry {
    pub project_id: String,
    pub name: String,
    pub root: PathBuf,
    pub last_opened_at_ms: i64,
    pub status: ProjectStatus,
}

/// The global registry of all AETHER projects known to the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectRegistry {
    pub schema_version: u32,
    pub active_project_id: Option<String>,
    pub projects: Vec<ProjectRegistryEntry>,
}

/// Unique identifier for a graph node.
pub type NodeId = u64;

/// Represents a blend mode for overlapping visual elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    SoftLight,
}

/// Represents a type of visual transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionKind {
    Crossfade,
    Dissolve,
    WipeLeft,
    WipeRight,
    WipeUp,
    WipeDown,
    SlideLeft,
    SlideRight,
}

/// Represents a type of filter applied to a node.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FilterKind {
    GaussianBlur { radius: f32 },
    Contrast { factor: f32 },
    Brightness { delta: f32 },
}

/// Represents the kind of a composition node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Source(Ref),
    Blend { mode: BlendMode, opacity: f32 },
    Transition { kind: TransitionKind, duration_ms: u32 },
    Filter { kind: FilterKind },
    Output,
}

/// Represents a single composition node with its kind and ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
}

/// A directed connection between two ports in the composition graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub from_node: NodeId,
    pub from_port: u8,
    pub to_node: NodeId,
    pub to_port: u8,
}

/// A Directed Acyclic Graph (DAG) for image and video composition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CompositionGraph {
    pub nodes: HashMap<NodeId, Node>,
    pub connections: Vec<Connection>,
    pub output_node: Option<NodeId>,
}

impl CompositionGraph {
    /// Creates a new empty CompositionGraph.
    pub fn new() -> Self {
        // Invariant: The function returns an empty graph instance with default mappings.
        Self::default()
    }

    /// Adds a node to the graph.
    pub fn add_node(&mut self, node: Node) {
        // Invariant: The node is successfully added to the graph, and the graph's nodes mapping contains the new node.
        self.nodes.insert(node.id, node);
    }

    /// Checks if a directed path exists between start and end nodes.
    pub fn is_reachable(&self, start: NodeId, end: NodeId) -> bool {
        // Invariant: The function returns true if and only if there exists a directed path from start to end in the current graph connections, preserving the visited tracking state.
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            if node == end {
                return true;
            }
            if visited.insert(node) {
                for conn in &self.connections {
                    if conn.from_node == node {
                        stack.push(conn.to_node);
                    }
                }
            }
        }
        false
    }

    /// Connects two nodes. Returns an error if a cycle would be introduced.
    pub fn connect(&mut self, conn: Connection) -> Result<(), AetherError> {
        // Invariant: If successful, the connection is added and the graph remains a Directed Acyclic Graph (DAG); on failure, the graph's connections list remains unchanged.
        if !self.nodes.contains_key(&conn.from_node) {
            return Err(AetherError::OperationFailed(format!("Source node {} not found", conn.from_node)));
        }
        if !self.nodes.contains_key(&conn.to_node) {
            return Err(AetherError::OperationFailed(format!("Target node {} not found", conn.to_node)));
        }
        if self.is_reachable(conn.to_node, conn.from_node) {
            return Err(AetherError::OperationFailed("Connecting these nodes would introduce a cycle".to_string()));
        }
        self.connections.push(conn);
        Ok(())
    }

    /// Removes a node and any connections associated with it.
    pub fn remove_node(&mut self, id: NodeId) {
        // Invariant: The specified node and all connections referencing it are completely removed from the graph.
        self.nodes.remove(&id);
        self.connections.retain(|c| c.from_node != id && c.to_node != id);
        if self.output_node == Some(id) {
            self.output_node = None;
        }
    }

    /// Performs a topological sort on the graph nodes. Returns a list of sorted node IDs or a cycle error.
    pub fn topological_sort(&self) -> Result<Vec<NodeId>, AetherError> {
        // Invariant: Returns a list of node IDs in topological order if the graph is a DAG; otherwise, returns a cycle error, ensuring the graph itself remains unmodified.
        let mut in_degree = HashMap::new();
        for &id in self.nodes.keys() {
            in_degree.insert(id, 0);
        }

        for conn in &self.connections {
            if let Some(deg) = in_degree.get_mut(&conn.to_node) {
                *deg += 1;
            }
        }

        let mut queue = Vec::new();
        for (&id, &deg) in &in_degree {
            if deg == 0 {
                queue.push(id);
            }
        }

        queue.sort_unstable();

        let mut sorted = Vec::new();
        while let Some(node) = queue.pop() {
            sorted.push(node);
            for conn in &self.connections {
                if conn.from_node == node {
                    if let Some(deg) = in_degree.get_mut(&conn.to_node) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(conn.to_node);
                        }
                    }
                }
            }
            queue.sort_unstable();
        }

        if sorted.len() != self.nodes.len() {
            return Err(AetherError::OperationFailed("Cycle detected during topological sorting".to_string()));
        }

        Ok(sorted)
    }
}

/// The playback track kind (Video or Audio).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
}

/// A clip positioned in time on a timeline track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub asset_ref: Ref,
    pub in_point_ms: u64,
    pub out_point_ms: u64,
    pub track_offset_ms: u64,
    pub transition: Option<TransitionKind>,
}

/// A track containing multiple sequential or overlapping clips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}

/// A multi-track timeline representing the layout of the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Timeline {
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GenerationKind {
    StoryboardScratch,
    Dialogue,
    Image,
    ImageEdit,
    Voice,
    VoiceClone,
    SceneAudio,
    Music,
    VideoText,
    VideoFrame,
    VideoIngredients,
    VideoEdit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenerationStatus {
    Queued,
    Prompting,
    AwaitingClarification,
    Submitted,
    Running,
    Downloading,
    Ready,
    Failed,
    Cancelled,
    Recovering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeneratedArtifactKind {
    StoryboardJson,
    DialogueJson,
    Image,
    Audio,
    Music,
    Video,
    PromptJson,
    MetadataJson,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderModel {
    pub id: String,
    pub provider: String,
    pub kind: GenerationKind,
    pub enabled: bool,
    pub capabilities: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfessionalPrompt {
    pub original_request: String,
    pub professional_prompt: String,
    pub negative_prompt: Option<String>,
    pub locale: Option<String>,
    pub style: Option<String>,
    pub technical: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub job_ref: Ref,
    pub kind: GenerationKind,
    pub user_request: String,
    pub model: Option<String>,
    pub inputs: Vec<Ref>,
    pub options: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationArtifact {
    pub kind: GeneratedArtifactKind,
    pub path: PathBuf,
    pub asset_ref: Option<Ref>,
    pub mime_type: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationJob {
    pub job_ref: Ref,
    pub kind: GenerationKind,
    pub status: GenerationStatus,
    pub requested_model: Option<String>,
    pub resolved_model: Option<ProviderModel>,
    pub provider_job_id: Option<String>,
    pub prompt: Option<ProfessionalPrompt>,
    pub inputs: Vec<Ref>,
    pub artifacts: Vec<GenerationArtifact>,
    pub error: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub options: serde_json::Value,
}

/// Condensed, LLM-friendly view of the project state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub settings: ProjectSettings,
    pub assets: Vec<Asset>,
    pub history_len: usize,
    pub history_cursor: usize,
    pub graph: CompositionGraph,
    pub timeline: Timeline,
    pub generation_jobs: Vec<GenerationJob>,
}

/// Represents the executable Domain Specific Language commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Init {
        fps: Option<f32>,
        resolution: Option<String>,
        colorspace: Option<String>,
    },
    Import {
        path: String,
    },
    Trim {
        r: Ref,
        start: String,
        end: String,
    },
    Mix {
        r: Ref,
        volume: f32,
    },
    Composite {
        base: Ref,
        overlay: Ref,
        at: String,
        x: i32,
        y: i32,
    },
    Canvas {
        width: u32,
        height: u32,
        color: String,
    },
    DrawText {
        r: Ref,
        text: String,
        font: String,
        size: f32,
        x: i32,
        y: i32,
    },
    Export {
        r: Ref,
        format: String,
        codec: String,
        quality: String,
    },
    Undo,
    Redo,
    Snapshot,
    // Timeline commands
    Concat {
        refs: Vec<Ref>,
        transition: Option<String>,
        duration_ms: Option<u32>,
    },
    Overlay {
        base: Ref,
        overlay: Ref,
        x: i32,
        y: i32,
        blend: Option<String>,
        opacity: Option<f32>,
    },
    Speed {
        r: Ref,
        factor: f32,
    },
    // Observation
    Inspect {
        r: Option<Ref>,
        start: Option<String>,
        end: Option<String>,
    },
    // Audio DSP
    Eq {
        r: Ref,
        filter_type: String,
        freq_hz: f32,
        gain_db: f32,
        q: f32,
    },
    Compress {
        r: Ref,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
    },
    MixTracks {
        refs: Vec<Ref>,
        volumes: Vec<f32>,
        pans: Vec<f32>,
    },
    // Keyframes & Animation
    KeyframeSet {
        r: Ref,
        property: String,
        time_ms: u64,
        value: f32,
        easing: Option<String>,
    },
    KeyframeList {
        r: Ref,
        property: String,
    },
    // Interopérabilité
    ExportOtio {
        output_path: String,
    },
    ExportEdl {
        output_path: String,
    },
    // Creative Version Control
    Branch {
        name: String,
    },
    Checkout {
        name: String,
    },
    GenerateStoryboardScratch {
        request: String,
        model: Option<String>,
        options: serde_json::Value,
    },
    GenerateDialogue {
        request: String,
        model: Option<String>,
        options: serde_json::Value,
    },
    GenerateImage {
        request: String,
        model: Option<String>,
        inputs: Vec<Ref>,
        options: serde_json::Value,
    },
    EditImage {
        target: Ref,
        request: String,
        model: Option<String>,
        options: serde_json::Value,
    },
    GenerateVoice {
        text: String,
        voice: Option<String>,
        model: Option<String>,
        options: serde_json::Value,
    },
    CloneVoice {
        sample: Ref,
        name: Option<String>,
        model: Option<String>,
        options: serde_json::Value,
    },
    GenerateSceneAudio {
        request: String,
        model: Option<String>,
        options: serde_json::Value,
    },
    GenerateMusic {
        request: String,
        model: Option<String>,
        options: serde_json::Value,
    },
    GenerateVideoFromText {
        request: String,
        model: Option<String>,
        options: serde_json::Value,
    },
    GenerateVideoFromFrame {
        frame: Ref,
        request: String,
        model: Option<String>,
        options: serde_json::Value,
    },
    GenerateVideoFromIngredients {
        inputs: Vec<Ref>,
        request: String,
        model: Option<String>,
        options: serde_json::Value,
    },
    EditVideo {
        target: Ref,
        request: String,
        model: Option<String>,
        options: serde_json::Value,
    },
    GenerationStatus {
        r: Option<Ref>,
    },
    CancelGeneration {
        r: Ref,
    },
    // Project management commands
    ProjectCreate {
        name: String,
        dir: Option<PathBuf>,
        adopt: bool,
        force: bool,
    },
    ProjectOpen {
        target: String,
    },
    ProjectCurrent,
    ProjectClose {
        target: Option<String>,
    },
    ProjectList,
        ProjectDelete {
        target: String,
        force: bool,
        archive: bool,
    },
    VaultCreate {
        name: String,
        kind: VaultKind,
        description: Option<String>,
    },
    VaultList,
    VaultShow {
        vault_id: String,
    },
    VaultAdd {
        vault_id: String,
        asset_name: String,
        asset_kind: VaultAssetKind,
        source_file: Option<PathBuf>,
        text_content: Option<String>,
        usage: Vec<VaultUsage>,
        tags: Vec<String>,
        metadata: serde_json::Value,
    },
    VaultAttach {
        vault_id: String,
        alias: String,
    },
    VaultDetach {
        vault_id: String,
    },
    VaultAttached,
    // AETHER Planner commands
    PlanCreate {
        objective: String,
        plan_json: Option<String>,
    },
    PlanShow {
        plan_id: String,
    },
    PlanRevise {
        plan_id: String,
        instruction: String,
    },
    PlanNext {
        plan_id: String,
    },
    PlanCheck {
        plan_id: String,
        step_id: String,
        evidence_ref: Option<Ref>,
    },
    PlanUncheck {
        plan_id: String,
        step_id: String,
    },
    PlanStatus {
        plan_id: String,
    },
    Shutdown,
}

/// Standardized output structure after executing a command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub affected_ref: Option<Ref>,
    pub message: String,
    pub snapshot: Option<Snapshot>,
}

/// The kind of Vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VaultKind {
    Brand,
    Character,
    Product,
    Campaign,
    Studio,
    Client,
    General,
}

impl fmt::Display for VaultKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            VaultKind::Brand => "brand",
            VaultKind::Character => "character",
            VaultKind::Product => "product",
            VaultKind::Campaign => "campaign",
            VaultKind::Studio => "studio",
            VaultKind::Client => "client",
            VaultKind::General => "general",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for VaultKind {
    type Err = AetherError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "brand" => Ok(VaultKind::Brand),
            "character" => Ok(VaultKind::Character),
            "product" => Ok(VaultKind::Product),
            "campaign" => Ok(VaultKind::Campaign),
            "studio" => Ok(VaultKind::Studio),
            "client" => Ok(VaultKind::Client),
            "general" => Ok(VaultKind::General),
            other => Err(AetherError::VaultError(format!("Unknown Vault kind: '{}'", other))),
        }
    }
}

/// A Vault structure representing a creative library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vault {
    pub schema_version: u32,
    pub vault_id: String,
    pub name: String,
    pub kind: VaultKind,
    pub root: PathBuf,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub description: Option<String>,
    pub default_language: Option<String>,
    pub owner: Option<String>,
}

/// The kind of asset stored in a Vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VaultAssetKind {
    Logo,
    LogoVariant,
    Watermark,
    ColorPalette,
    Typography,
    DesignRulebook,
    Persona,
    CharacterReference,
    ProductReference,
    VoiceReference,
    MusicReference,
    LegalGuideline,
    PromptSnippet,
    NegativePrompt,
    ExampleOutput,
}

impl fmt::Display for VaultAssetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            VaultAssetKind::Logo => "logo",
            VaultAssetKind::LogoVariant => "logo-variant",
            VaultAssetKind::Watermark => "watermark",
            VaultAssetKind::ColorPalette => "color-palette",
            VaultAssetKind::Typography => "typography",
            VaultAssetKind::DesignRulebook => "design-rulebook",
            VaultAssetKind::Persona => "persona",
            VaultAssetKind::CharacterReference => "character-reference",
            VaultAssetKind::ProductReference => "product-reference",
            VaultAssetKind::VoiceReference => "voice-reference",
            VaultAssetKind::MusicReference => "music-reference",
            VaultAssetKind::LegalGuideline => "legal-guideline",
            VaultAssetKind::PromptSnippet => "prompt-snippet",
            VaultAssetKind::NegativePrompt => "negative-prompt",
            VaultAssetKind::ExampleOutput => "example-output",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for VaultAssetKind {
    type Err = AetherError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('_', "-").as_str() {
            "logo" => Ok(VaultAssetKind::Logo),
            "logo-variant" => Ok(VaultAssetKind::LogoVariant),
            "watermark" => Ok(VaultAssetKind::Watermark),
            "color-palette" | "palette" => Ok(VaultAssetKind::ColorPalette),
            "typography" | "font" => Ok(VaultAssetKind::Typography),
            "design-rulebook" | "rule" => Ok(VaultAssetKind::DesignRulebook),
            "persona" => Ok(VaultAssetKind::Persona),
            "character-reference" | "character-ref" => Ok(VaultAssetKind::CharacterReference),
            "product-reference" | "product-ref" => Ok(VaultAssetKind::ProductReference),
            "voice-reference" | "voice-ref" => Ok(VaultAssetKind::VoiceReference),
            "music-reference" | "music-ref" => Ok(VaultAssetKind::MusicReference),
            "legal-guideline" => Ok(VaultAssetKind::LegalGuideline),
            "prompt-snippet" => Ok(VaultAssetKind::PromptSnippet),
            "negative-prompt" => Ok(VaultAssetKind::NegativePrompt),
            "example-output" => Ok(VaultAssetKind::ExampleOutput),
            other => Err(AetherError::VaultError(format!("Unknown Vault asset kind: '{}'", other))),
        }
    }
}

/// Contexts of generation where the asset is allowed to be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VaultUsage {
    GenerateImage,
    EditImage,
    GenerateVideo,
    EditVideo,
    GenerateVoice,
    CloneVoice,
    GenerateMusic,
    GenerateSceneAudio,
    Storyboard,
    PromptMaker,
    ExportBranding,
}

impl fmt::Display for VaultUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            VaultUsage::GenerateImage => "generate-image",
            VaultUsage::EditImage => "edit-image",
            VaultUsage::GenerateVideo => "generate-video",
            VaultUsage::EditVideo => "edit-video",
            VaultUsage::GenerateVoice => "generate-voice",
            VaultUsage::CloneVoice => "clone-voice",
            VaultUsage::GenerateMusic => "generate-music",
            VaultUsage::GenerateSceneAudio => "generate-scene-audio",
            VaultUsage::Storyboard => "storyboard",
            VaultUsage::PromptMaker => "prompt-maker",
            VaultUsage::ExportBranding => "export-branding",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for VaultUsage {
    type Err = AetherError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('_', "-").as_str() {
            "generate-image" => Ok(VaultUsage::GenerateImage),
            "edit-image" => Ok(VaultUsage::EditImage),
            "generate-video" => Ok(VaultUsage::GenerateVideo),
            "edit-video" => Ok(VaultUsage::EditVideo),
            "generate-voice" => Ok(VaultUsage::GenerateVoice),
            "clone-voice" => Ok(VaultUsage::CloneVoice),
            "generate-music" => Ok(VaultUsage::GenerateMusic),
            "generate-scene-audio" => Ok(VaultUsage::GenerateSceneAudio),
            "storyboard" => Ok(VaultUsage::Storyboard),
            "prompt-maker" => Ok(VaultUsage::PromptMaker),
            "export-branding" => Ok(VaultUsage::ExportBranding),
            other => Err(AetherError::VaultError(format!("Unknown Vault usage: '{}'", other))),
        }
    }
}

/// Struct representing an asset indexed in a Vault.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultAsset {
    pub vault_asset_id: String,
    pub vault_id: String,
    pub kind: VaultAssetKind,
    pub name: String,
    pub path: PathBuf,
    pub hash: String,
    pub tags: Vec<String>,
    pub usage: Vec<VaultUsage>,
    pub metadata: serde_json::Value,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Reference details of a Vault asset to pass in prompt context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultAssetRef {
    pub vault_asset_id: String,
    pub name: String,
    pub kind: VaultAssetKind,
    pub path: PathBuf,
    pub hash: String,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
}

/// Prompt context representing the assets, rules, and snippets compiled from a single Vault.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultPromptContext {
    pub vault_id: String,
    pub name: String,
    pub kind: VaultKind,
    pub rules: Vec<String>,
    pub prompt_snippets: Vec<String>,
    pub negative_prompts: Vec<String>,
    pub reference_assets: Vec<VaultAssetRef>,
}

/// Full prompt context passed down to generative agents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptContext {
    pub project_summary: Option<String>,
    pub style_hints: Vec<String>,
    pub negative_constraints: Vec<String>,
    pub vault_context: Vec<VaultPromptContext>,
}

/// Representation of `vault_links.json` entry in a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultLink {
    pub vault_id: String,
    pub alias: String,
    pub scope: String,
    pub locked_version: Option<String>,
}

/// Representation of `vault_links.json` in a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultLinks {
    pub schema_version: u32,
    pub attached_vaults: Vec<VaultLink>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanStatus {
    Pending,
    Running,
    Done,
    Failed,
    NeedsRevision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanStepStatus {
    Pending,
    Ready,
    Running,
    Done,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanEvidence {
    pub step_id: String,
    pub evidence_ref: Option<Ref>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AetherPlanStep {
    pub id: String,
    pub title: String,
    pub status: PlanStepStatus,
    pub command: String,
    pub creates: Vec<RefKind>,
    pub depends_on: Vec<String>,
    pub validation: String,
    pub evidence: Option<PlanEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AetherPlan {
    pub plan_id: String,
    pub objective: String,
    pub status: PlanStatus,
    pub assumptions: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub steps: Vec<AetherPlanStep>,
    pub validation_plan: Vec<String>,
    pub risks: Vec<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ref_parsing_valid() {
        let r = "@v1".parse::<Ref>().unwrap();
        assert_eq!(r.kind, RefKind::Video);
        assert_eq!(r.id, 1);
        assert_eq!(r.to_string(), "@v1");

        let r = "@a42".parse::<Ref>().unwrap();
        assert_eq!(r.kind, RefKind::Audio);
        assert_eq!(r.id, 42);
        assert_eq!(r.to_string(), "@a42");

        let r = "@img999".parse::<Ref>().unwrap();
        assert_eq!(r.kind, RefKind::Image);
        assert_eq!(r.id, 999);
        assert_eq!(r.to_string(), "@img999");

        let r = "@anim10".parse::<Ref>().unwrap();
        assert_eq!(r.kind, RefKind::Animation);
        assert_eq!(r.id, 10);
        assert_eq!(r.to_string(), "@anim10");

        let r = "@g123".parse::<Ref>().unwrap();
        assert_eq!(r.kind, RefKind::Generated);
        assert_eq!(r.id, 123);
        assert_eq!(r.to_string(), "@g123");
    }

    #[test]
    fn test_ref_parsing_invalid() {
        assert!(matches!("@v".parse::<Ref>(), Err(AetherError::InvalidRef(_, _))));
        assert!(matches!("v1".parse::<Ref>(), Err(AetherError::InvalidRef(_, _))));
        assert!(matches!("@v1a".parse::<Ref>(), Err(AetherError::InvalidRef(_, _))));
        assert!(matches!("@unknown123".parse::<Ref>(), Err(AetherError::InvalidRef(_, _))));
    }

    #[test]
    fn test_ref_serde_roundtrip() {
        let r = Ref {
            kind: RefKind::Video,
            id: 15,
        };
        let serialized = serde_json::to_string(&r).unwrap();
        assert_eq!(serialized, "\"@v15\"");

        let deserialized: Ref = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, r);
    }

    #[test]
    fn test_ref_registry_allocation() {
        let registry = RefRegistry::new();
        let r1 = registry.allocate(RefKind::Video);
        let r2 = registry.allocate(RefKind::Video);
        let r3 = registry.allocate(RefKind::Audio);

        assert_eq!(r1, Ref { kind: RefKind::Video, id: 1 });
        assert_eq!(r2, Ref { kind: RefKind::Video, id: 2 });
        assert_eq!(r3, Ref { kind: RefKind::Audio, id: 1 });
    }

    #[test]
    fn test_ref_registry_register_and_resolve() {
        let registry = RefRegistry::new();
        let r = registry.allocate(RefKind::Video);
        let asset = Asset {
            r,
            kind: AssetKind::Video,
            path: PathBuf::from("test.mp4"),
            hash: "blake3_hash_here".to_string(),
            metadata: serde_json::json!({ "duration": 10.0 }),
        };

        // Resolution fails before registration
        assert_eq!(registry.resolve(&r), Err(AetherError::RefNotFound(r)));

        // Registration succeeds
        assert_eq!(registry.register(r, asset.clone()), Ok(()));

        // Registering same reference twice fails
        assert_eq!(registry.register(r, asset.clone()), Err(AetherError::RefAlreadyExists(r)));

        // Resolution succeeds after registration
        assert_eq!(registry.resolve(&r), Ok(asset));

        // Freeing reference succeeds
        assert!(registry.free(&r));
        assert_eq!(registry.resolve(&r), Err(AetherError::RefNotFound(r)));
    }

    #[test]
    fn test_composition_graph_dag() {
        let mut graph = CompositionGraph::new();
        let r1 = "@v1".parse::<Ref>().unwrap();
        let r2 = "@v2".parse::<Ref>().unwrap();

        let n1 = Node { id: 1, kind: NodeKind::Source(r1) };
        let n2 = Node { id: 2, kind: NodeKind::Source(r2) };
        let n3 = Node { id: 3, kind: NodeKind::Blend { mode: BlendMode::Normal, opacity: 1.0 } };
        let n4 = Node { id: 4, kind: NodeKind::Output };

        graph.add_node(n1);
        graph.add_node(n2);
        graph.add_node(n3);
        graph.add_node(n4);

        assert_eq!(graph.connect(Connection { from_node: 1, from_port: 0, to_node: 3, to_port: 0 }), Ok(()));
        assert_eq!(graph.connect(Connection { from_node: 2, from_port: 0, to_node: 3, to_port: 1 }), Ok(()));
        assert_eq!(graph.connect(Connection { from_node: 3, from_port: 0, to_node: 4, to_port: 0 }), Ok(()));

        // Attempting to introduce a cycle: 4 -> 1
        assert!(graph.connect(Connection { from_node: 4, from_port: 0, to_node: 1, to_port: 0 }).is_err());

        let sorted = graph.topological_sort().unwrap();
        let pos1 = sorted.iter().position(|&x| x == 1).unwrap();
        let pos2 = sorted.iter().position(|&x| x == 2).unwrap();
        let pos3 = sorted.iter().position(|&x| x == 3).unwrap();
        let pos4 = sorted.iter().position(|&x| x == 4).unwrap();

        assert!(pos1 < pos3);
        assert!(pos2 < pos3);
        assert!(pos3 < pos4);

        // Remove node 3
        graph.remove_node(3);
        assert!(!graph.nodes.contains_key(&3));
        // All connections touching 3 should be gone
        assert!(graph.connections.is_empty());
    }

    #[test]
    fn test_timeline_serialization() {
        let r1 = "@v1".parse::<Ref>().unwrap();
        let clip = Clip {
            asset_ref: r1,
            in_point_ms: 0,
            out_point_ms: 5000,
            track_offset_ms: 0,
            transition: Some(TransitionKind::Crossfade),
        };
        let track = Track {
            name: "Video Track 1".to_string(),
            kind: TrackKind::Video,
            clips: vec![clip],
        };
        let timeline = Timeline {
            tracks: vec![track],
        };

        let serialized = serde_json::to_string(&timeline).unwrap();
        let deserialized: Timeline = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, timeline);
    }

    #[test]
    fn test_generation_serde_roundtrip() {
        let job_ref = "@g1".parse::<Ref>().unwrap();
        let job = GenerationJob {
            job_ref,
            kind: GenerationKind::Image,
            status: GenerationStatus::Ready,
            requested_model: Some("mock/image".to_string()),
            resolved_model: Some(ProviderModel {
                id: "mock/image".to_string(),
                provider: "mock".to_string(),
                kind: GenerationKind::Image,
                enabled: true,
                capabilities: serde_json::json!({}),
            }),
            provider_job_id: Some("mock-job-1".to_string()),
            prompt: Some(ProfessionalPrompt {
                original_request: "a cat".to_string(),
                professional_prompt: "a beautiful cat".to_string(),
                negative_prompt: None,
                locale: None,
                style: None,
                technical: serde_json::json!({}),
            }),
            inputs: Vec::new(),
            artifacts: vec![GenerationArtifact {
                kind: GeneratedArtifactKind::Image,
                path: PathBuf::from("/tmp/art.png"),
                asset_ref: None,
                mime_type: Some("image/png".to_string()),
                metadata: serde_json::json!({}),
            }],
            error: None,
            created_at_ms: 123456789,
            updated_at_ms: 123456799,
            options: serde_json::json!({}),
        };

        let serialized = serde_json::to_string(&job).unwrap();
        let deserialized: GenerationJob = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, job);
    }
}

