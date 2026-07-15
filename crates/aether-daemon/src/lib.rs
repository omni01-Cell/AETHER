pub mod observation;

use aether_core::{
    AetherError, AssetKind, BlendMode, Clip, Command, CommandResult, CompositionGraph,
    Connection as GraphConnection, GenerationKind, GenerationStatus, Node, NodeKind,
    ProjectSettings, Ref, RefKind, RefRegistry, SmartObservation, Snapshot, TelemetryData,
    Timeline, Track, TrackKind, TransitionKind,
};
use aether_generate::DefaultGenerationRuntime;
use aether_persistence::DbManager;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

pub struct SessionManager {
    registry: RefRegistry,
    settings: RwLock<ProjectSettings>,
    db: Mutex<DbManager>,
    current_branch: RwLock<String>,
    current_commit: RwLock<String>,
    cache_dir: PathBuf,
    project_dir: PathBuf,
    graph: RwLock<CompositionGraph>,
    timeline: RwLock<Timeline>,
    runtime: DefaultGenerationRuntime,
}

impl SessionManager {
    /// Creates a new SessionManager, initializing the persistence DB and loading existing state.
    pub fn new<P: AsRef<Path>>(project_dir: P) -> Result<Self, AetherError> {
        // Invariant: Initializes a new SessionManager, creating all cache/db paths and loading state from DB.
        let p_dir = project_dir.as_ref().to_path_buf();
        let aether_dir = p_dir.join(".aether");
        if !aether_dir.exists() {
            fs::create_dir_all(&aether_dir).map_err(|e| {
                AetherError::IoError(aether_dir.to_string_lossy().to_string(), e.to_string())
            })?;
        }

        let db = DbManager::new(&aether_dir)?;
        let cache_dir = aether_dir.join("cache");
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(|e| {
                AetherError::IoError(cache_dir.to_string_lossy().to_string(), e.to_string())
            })?;
        }

        // Load settings, branch and current commit from DB
        let (settings, current_branch, current_commit) = db.load_settings()?;
        let graph = db.load_graph()?;
        let timeline = db.load_timeline()?;

        let runtime = DefaultGenerationRuntime::new(cache_dir.join("generated"));

        let manager = SessionManager {
            registry: RefRegistry::new(),
            settings: RwLock::new(settings),
            db: Mutex::new(db),
            current_branch: RwLock::new(current_branch),
            current_commit: RwLock::new(current_commit),
            cache_dir,
            project_dir: p_dir,
            graph: RwLock::new(graph),
            timeline: RwLock::new(timeline),
            runtime,
        };

        // Load all saved assets from DB into memory registry
        let saved_assets = manager.db.lock().unwrap().load_assets()?;
        for asset in saved_assets {
            let _ = manager.registry.register(asset.r, asset);
        }

        Ok(manager)
    }

    /// Internal helper to retrieve snapshot using already-locked DbManager connection.
    fn get_snapshot_with_db(&self, db: &DbManager) -> Result<Snapshot, AetherError> {
        // Invariant: Constructs a Snapshot reflecting the current settings, assets, history, and graph states.
        let settings = self.settings.read().unwrap().clone();
        let assets = self.registry.list_assets();
        let history = db.load_history()?;
        let cur_commit = self.current_commit.read().unwrap().clone();

        // Trace ancestors of the current commit
        let mut ancestors = Vec::new();
        let mut current = cur_commit.clone();
        while !current.is_empty() {
            if let Some(pos) = history.iter().position(|h| h.0 == current) {
                ancestors.push(current.clone());
                current = history[pos].1.clone().unwrap_or_default();
            } else {
                break;
            }
        }
        let history_len = ancestors.len();
        let history_cursor = history_len;

        let graph = self.graph.read().unwrap().clone();
        let timeline = self.timeline.read().unwrap().clone();

        let generation_jobs = db.load_all_generation_jobs()?;

        Ok(Snapshot {
            settings,
            assets,
            history_len,
            history_cursor,
            graph,
            timeline,
            generation_jobs,
        })
    }

    /// Retrieves the current snapshot of the project.
    pub fn get_snapshot(&self) -> Result<Snapshot, AetherError> {
        // Invariant: Obtains a new DB lock and builds a Snapshot of the active project state.
        let db = self.db.lock().unwrap();
        self.get_snapshot_with_db(&db)
    }

    /// Restructures the in-memory state and updates SQLite active tables to exactly match the target commit checkpoint.
    fn checkout_commit(&self, db: &DbManager, commit_hash: &str) -> Result<(), AetherError> {
        // Invariant: Restores the project state from the specified checkpoint, syncs in-memory models and active SQLite tables.
        if !commit_hash.is_empty() {
            let (timeline, graph, assets, settings) = db.load_checkpoint(commit_hash)?;

            // Overwrite memory
            *self.timeline.write().unwrap() = timeline.clone();
            *self.graph.write().unwrap() = graph.clone();
            *self.settings.write().unwrap() = settings.clone();

            // Overwrite registry assets
            let mut registry_map = HashMap::new();
            for asset in assets {
                registry_map.insert(asset.r, asset);
            }
            let (counters, _) = self.registry.get_state();
            self.registry.set_state((counters, registry_map));

            // Overwrite active SQLite tables
            db.save_timeline(&timeline)?;
            db.save_graph(&graph)?;
            db.clear_assets()?;
            for asset in self.registry.list_assets() {
                db.save_asset(&asset)?;
            }
        } else {
            // Revert to empty state
            *self.timeline.write().unwrap() = Timeline::default();
            *self.graph.write().unwrap() = CompositionGraph::default();
            *self.settings.write().unwrap() = ProjectSettings::default();
            let (counters, _) = self.registry.get_state();
            self.registry.set_state((counters, HashMap::new()));

            db.save_timeline(&Timeline::default())?;
            db.save_graph(&CompositionGraph::default())?;
            db.clear_assets()?;
        }

        *self.current_commit.write().unwrap() = commit_hash.to_string();
        db.save_settings(
            &self.settings.read().unwrap(),
            &self.current_branch.read().unwrap(),
            commit_hash,
        )?;
        Ok(())
    }

    /// Invariant: This function must preserve the invariant that a generative request is processed, the job status transitions are saved in SQLite (including event logs), and only real non-mock media files (not ending in .json) are registered as assets, returning a success message.
    fn execute_generation_request(
        &self,
        db: &DbManager,
        kind: GenerationKind,
        user_request: String,
        model: Option<String>,
        inputs: Vec<Ref>,
        options: serde_json::Value,
        affected_ref: &mut Option<Ref>,
    ) -> Result<String, AetherError> {
        let job_ref = self.registry.reserve_next(RefKind::Generated);
        *affected_ref = Some(job_ref);

        db.add_generation_event(&job_ref, &GenerationStatus::Queued, "Job queued in daemon")?;

        let mut options_with_vault = options.clone();
        if !options_with_vault.is_object() {
            options_with_vault = serde_json::json!({});
        }

        // Compile attached vaults context and inject it into request options
        if let Ok(vault_mgr) = aether_vault::VaultManager::load_default() {
            if let Ok(prompt_ctx) = vault_mgr.compile_prompt_context(&self.project_dir) {
                if let Ok(ctx_json) = serde_json::to_value(&prompt_ctx) {
                    if let serde_json::Value::Object(ref mut map) = options_with_vault {
                        map.insert("vault_context".to_string(), ctx_json);
                    }
                }
            }
        }

        let mut input_asset_paths: Vec<String> = Vec::new();
        for input_ref in &inputs {
            let asset = self.registry.resolve(input_ref)?;
            input_asset_paths.push(asset.path.to_string_lossy().into_owned());
        }
        if let serde_json::Value::Object(ref mut map) = options_with_vault {
            if !input_asset_paths.is_empty() {
                map.insert(
                    "input_asset_paths".to_string(),
                    serde_json::json!(input_asset_paths),
                );
            }
        }

        let req = aether_core::GenerationRequest {
            job_ref,
            kind,
            user_request,
            model,
            inputs,
            options: options_with_vault,
        };

        db.add_generation_event(
            &job_ref,
            &GenerationStatus::Running,
            "Job processing started",
        )?;
        let job = self.runtime.run_to_completion(req)?;
        db.save_generation_job(&job)?;
        db.add_generation_event(&job_ref, &job.status, "Job execution completed")?;

        let mut registered = Vec::new();
        for art in &job.artifacts {
            // ONLY register real media files (no mock JSON files)
            let path_str = art.path.to_string_lossy();
            if path_str.ends_with(".json") || path_str.contains(".mock-") {
                continue;
            }

            match art.kind {
                aether_core::GeneratedArtifactKind::Image => {
                    let asset_ref = self.registry.reserve_next(RefKind::Image);
                    let asset = aether_core::Asset {
                        r: asset_ref,
                        kind: AssetKind::Image,
                        path: art.path.clone(),
                        hash: blake3::hash(&std::fs::read(&art.path).unwrap_or_default())
                            .to_hex()
                            .to_string(),
                        metadata: art.metadata.clone(),
                    };
                    self.registry.register(asset_ref, asset.clone())?;
                    db.save_asset(&asset)?;
                    registered.push(asset_ref.to_string());
                }
                aether_core::GeneratedArtifactKind::Audio
                | aether_core::GeneratedArtifactKind::Music => {
                    let asset_ref = self.registry.reserve_next(RefKind::Audio);
                    let asset = aether_core::Asset {
                        r: asset_ref,
                        kind: AssetKind::Audio,
                        path: art.path.clone(),
                        hash: blake3::hash(&std::fs::read(&art.path).unwrap_or_default())
                            .to_hex()
                            .to_string(),
                        metadata: art.metadata.clone(),
                    };
                    self.registry.register(asset_ref, asset.clone())?;
                    db.save_asset(&asset)?;
                    registered.push(asset_ref.to_string());
                }
                aether_core::GeneratedArtifactKind::Video => {
                    let asset_ref = self.registry.reserve_next(RefKind::Video);
                    let asset = aether_core::Asset {
                        r: asset_ref,
                        kind: AssetKind::Video,
                        path: art.path.clone(),
                        hash: blake3::hash(&std::fs::read(&art.path).unwrap_or_default())
                            .to_hex()
                            .to_string(),
                        metadata: art.metadata.clone(),
                    };
                    self.registry.register(asset_ref, asset.clone())?;
                    db.save_asset(&asset)?;
                    registered.push(asset_ref.to_string());
                }
                _ => {}
            }
        }

        let mut msg = if registered.is_empty() {
            format!(
                "Generation {} completed with status {:?} using model {:?}",
                job_ref,
                job.status,
                job.resolved_model.as_ref().map(|m| &m.id)
            )
        } else {
            format!(
                "Generation {} completed with status {:?}. Registered assets: {}",
                job_ref,
                job.status,
                registered.join(", ")
            )
        };

        if job.status == aether_core::GenerationStatus::AwaitingClarification {
            msg = format!(
                "Generation {} awaiting prompter clarification (model {:?})",
                job_ref,
                job.resolved_model.as_ref().map(|m| &m.id)
            );
        }

        if let Some(clar) = job
            .options
            .get("prompter_clarifications")
            .and_then(|v| v.as_array())
        {
            if !clar.is_empty() {
                msg.push_str("\nPrompter (Maître d'Hôtel) needs answers:");
                for item in clar {
                    if let (Some(field), Some(question)) = (
                        item.get("field").and_then(|v| v.as_str()),
                        item.get("question").and_then(|v| v.as_str()),
                    ) {
                        msg.push_str(&format!("\n- [{}] {}", field, question));
                    }
                }
            }
        }

        Ok(msg)
    }

    /// Dispatches and executes a command, managing transaction history and persistence.
    pub fn execute(&self, command: Command) -> Result<CommandResult, AetherError> {
        // Invariant: Performs an atomic two-phase commit execution of the command, ensuring rollback of all states in case of failure.
        let db = self.db.lock().unwrap();

        // 1. Isolate in-memory states by cloning/saving them
        let orig_registry_state = self.registry.get_state();
        let orig_branch = self.current_branch.read().unwrap().clone();
        let orig_commit = self.current_commit.read().unwrap().clone();

        // 2. We handle Branch and Checkout specifically, as they do not require standard mutation/rendering but manipulate commits/branches directly.
        match &command {
            Command::Branch { name } => {
                // Check if branch already exists
                let head = db.load_branch_head(name)?;
                if head.is_some() {
                    return Err(AetherError::OperationFailed(format!(
                        "Branch '{}' already exists",
                        name
                    )));
                }
                // Save new branch pointing to current commit
                db.save_branch(name, &orig_commit)?;

                let snap = self.get_snapshot_with_db(&db)?;
                return Ok(CommandResult {
                    success: true,
                    affected_ref: None,
                    message: format!("Branch '{}' created successfully", name),
                    snapshot: Some(snap),
                });
            }
            Command::Checkout { name } => {
                // Check if we are checking out a branch name or a commit hash
                let target_commit = if let Some(head) = db.load_branch_head(name)? {
                    // Update current branch
                    *self.current_branch.write().unwrap() = name.clone();
                    head
                } else {
                    // Assume name is a commit hash
                    // Let's verify if the checkpoint exists
                    let check_exists = db.load_checkpoint(name);
                    if check_exists.is_err() {
                        return Err(AetherError::OperationFailed(format!(
                            "Branch or commit '{}' not found",
                            name
                        )));
                    }
                    *self.current_branch.write().unwrap() = "detached".to_string();
                    name.clone()
                };

                // Checkout the target commit
                self.checkout_commit(&db, &target_commit)?;

                let snap = self.get_snapshot_with_db(&db)?;
                return Ok(CommandResult {
                    success: true,
                    affected_ref: None,
                    message: format!("Switched to branch or commit '{}'", name),
                    snapshot: Some(snap),
                });
            }
            _ => {}
        }

        // 3. For all other commands, execute them. If an error is returned, we revert memory.
        let mut affected_ref = None;
        let msg;

        let run_result = self.process_command(&db, &command, &mut affected_ref);

        match run_result {
            Ok(success_msg) => {
                msg = success_msg;
            }
            Err(err) => {
                // Transaction rollback (Two-Phase Commit): Restore physical SQLite database and memory state completely to orig_commit
                *self.current_branch.write().unwrap() = orig_branch;
                let _ = self.checkout_commit(&db, &orig_commit);
                self.registry.set_state(orig_registry_state);

                return Err(err);
            }
        }

        // 4. Mutation Success: Commit Phase
        // Mutation logging to SQLite history and checkpointing
        if !matches!(
            command,
            Command::Undo
                | Command::Redo
                | Command::Snapshot
                | Command::Inspect { .. }
                | Command::KeyframeList { .. }
                | Command::Export { .. }
                | Command::ExportOtio { .. }
                | Command::ExportEdl { .. }
        ) {
            let snapshot_after = self.get_snapshot_with_db(&db)?;
            let commit_hash = blake3::hash(&serde_json::to_vec(&snapshot_after).unwrap())
                .to_hex()
                .to_string();

            let current_branch = self.current_branch.read().unwrap().clone();

            // Add commit to history
            db.add_history_entry(&commit_hash, Some(&orig_commit), &current_branch, &command)?;

            // Save state checkpoint
            let assets_list = self.registry.list_assets();
            let active_timeline = self.timeline.read().unwrap().clone();
            let active_graph = self.graph.read().unwrap().clone();
            let active_settings = self.settings.read().unwrap().clone();

            db.save_checkpoint(
                &commit_hash,
                &active_timeline,
                &active_graph,
                &assets_list,
                &active_settings,
            )?;

            // Update branch head and settings
            db.save_branch(&current_branch, &commit_hash)?;
            *self.current_commit.write().unwrap() = commit_hash.clone();
            db.save_settings(&active_settings, &current_branch, &commit_hash)?;
        }

        let final_snapshot = self.get_snapshot_with_db(&db)?;

        Ok(CommandResult {
            success: true,
            affected_ref,
            message: msg,
            snapshot: Some(final_snapshot),
        })
    }

    /// Performs standard mutations on the in-memory states and saves new assets to SQLite.
    fn process_command(
        &self,
        db: &DbManager,
        command: &Command,
        affected_ref: &mut Option<aether_core::Ref>,
    ) -> Result<String, AetherError> {
        // Invariant: Modifies the in-memory states and saves assets/timelines to SQLite active tables.
        let msg;
        match command {
            Command::Init {
                fps,
                resolution,
                colorspace,
            } => {
                let mut settings = self.settings.write().unwrap();
                if let Some(f) = fps {
                    settings.fps = *f;
                }
                if let Some(res) = resolution {
                    let parts: Vec<&str> = res.split('x').collect();
                    if parts.len() == 2 {
                        if let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                            settings.width = w;
                            settings.height = h;
                        }
                    }
                }
                if let Some(cs) = colorspace {
                    settings.colorspace = cs.clone();
                }
                db.save_settings(
                    &settings,
                    &self.current_branch.read().unwrap(),
                    &self.current_commit.read().unwrap(),
                )?;
                msg = "Project settings initialized successfully".to_string();
            }
            Command::Import { path } => {
                let p = Path::new(path);
                if !p.exists() {
                    return Err(AetherError::IoError(
                        path.clone(),
                        "File does not exist".to_string(),
                    ));
                }

                let ext = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let asset = match ext.as_str() {
                    "wav" | "mp3" | "ogg" | "flac" | "aac" => {
                        let r = self.registry.reserve_next(RefKind::Audio);
                        *affected_ref = Some(r);
                        aether_audio::import_audio(p, r, &self.cache_dir)?
                    }
                    "png" | "jpg" | "jpeg" => {
                        let r = self.registry.reserve_next(RefKind::Image);
                        *affected_ref = Some(r);
                        aether_image::import_image(p, r, &self.cache_dir)?
                    }
                    _ => {
                        let r = self.registry.reserve_next(RefKind::Video);
                        *affected_ref = Some(r);
                        aether_video::import_video(p, r, &self.cache_dir)?
                    }
                };

                self.registry.register(asset.r, asset.clone())?;
                db.save_asset(&asset)?;
                msg = format!("Imported asset successfully registered as {}", asset.r);
            }
            Command::Trim { r, start, end } => {
                let original = self.registry.resolve(r)?;
                let new_ref = self.registry.reserve_next(r.kind);
                *affected_ref = Some(new_ref);

                let trimmed = match original.kind {
                    AssetKind::Video => {
                        aether_video::trim_video(&original, start, end, new_ref, &self.cache_dir)?
                    }
                    AssetKind::Audio => {
                        aether_audio::trim_audio(&original, start, end, new_ref, &self.cache_dir)?
                    }
                    AssetKind::Image => {
                        return Err(AetherError::InvalidCommand(
                            "Cannot trim image asset".to_string(),
                        ))
                    }
                    AssetKind::Animation => {
                        return Err(AetherError::InvalidCommand(
                            "Cannot trim animation asset".to_string(),
                        ))
                    }
                };

                self.registry.register(trimmed.r, trimmed.clone())?;
                db.save_asset(&trimmed)?;
                msg = format!("Trimmed asset successfully registered as {}", trimmed.r);
            }
            Command::Mix { r, volume } => {
                let original = self.registry.resolve(r)?;
                if original.kind != AssetKind::Audio {
                    return Err(AetherError::InvalidCommand(
                        "Mix command is only supported for audio assets".to_string(),
                    ));
                }
                let new_ref = self.registry.reserve_next(RefKind::Audio);
                *affected_ref = Some(new_ref);

                let mixed = aether_audio::normalize_audio(
                    &original,
                    *volume,
                    -1.0,
                    new_ref,
                    &self.cache_dir,
                )?;
                self.registry.register(mixed.r, mixed.clone())?;
                db.save_asset(&mixed)?;
                msg = format!("Mixed audio asset successfully registered as {}", mixed.r);
            }
            Command::Composite {
                base,
                overlay,
                at,
                x,
                y,
            } => {
                let base_asset = self.registry.resolve(base)?;
                let overlay_asset = self.registry.resolve(overlay)?;

                let new_ref = self.registry.reserve_next(RefKind::Video);
                *affected_ref = Some(new_ref);

                let composited = aether_video::composite_video(
                    &base_asset,
                    &overlay_asset,
                    at,
                    *x,
                    *y,
                    new_ref,
                    &self.cache_dir,
                )?;
                self.registry.register(composited.r, composited.clone())?;
                db.save_asset(&composited)?;
                msg = format!(
                    "Composited asset successfully registered as {}",
                    composited.r
                );
            }
            Command::Canvas {
                width,
                height,
                color,
            } => {
                let r = self.registry.reserve_next(RefKind::Image);
                *affected_ref = Some(r);

                let canvas =
                    aether_image::create_canvas(*width, *height, color, r, &self.cache_dir)?;
                self.registry.register(canvas.r, canvas.clone())?;
                db.save_asset(&canvas)?;
                msg = format!("Canvas successfully registered as {}", canvas.r);
            }
            Command::DrawText {
                r,
                text,
                font,
                size,
                x,
                y,
            } => {
                let original = self.registry.resolve(r)?;
                if original.kind != AssetKind::Image {
                    return Err(AetherError::InvalidCommand(
                        "DrawText is only supported on image assets".to_string(),
                    ));
                }
                let new_ref = self.registry.reserve_next(RefKind::Image);
                *affected_ref = Some(new_ref);

                let text_overlay = aether_image::draw_text(
                    &original,
                    text,
                    font,
                    *size,
                    *x as f32,
                    *y as f32,
                    "white",
                    new_ref,
                    &self.cache_dir,
                )?;
                self.registry
                    .register(text_overlay.r, text_overlay.clone())?;
                db.save_asset(&text_overlay)?;
                msg = format!(
                    "Text overlay asset successfully registered as {}",
                    text_overlay.r
                );
            }
            Command::Export {
                r,
                format,
                codec,
                quality,
            } => {
                let asset = self.registry.resolve(r)?;
                let export_dir = self.project_dir.join("export");
                if !export_dir.exists() {
                    fs::create_dir_all(&export_dir).map_err(|e| {
                        AetherError::IoError(
                            export_dir.to_string_lossy().to_string(),
                            e.to_string(),
                        )
                    })?;
                }
                let dest_file = export_dir.join(format!("{}.{}", asset.hash, format));

                match asset.kind {
                    AssetKind::Video => {
                        aether_video::render_video(&asset, format, codec, quality, &dest_file)?;
                    }
                    AssetKind::Image => {
                        aether_image::export_image(&asset, &dest_file)?;
                    }
                    _ => {
                        return Err(AetherError::InvalidCommand(
                            "Export only supported for Video and Image assets".to_string(),
                        ));
                    }
                }
                msg = format!(
                    "Asset successfully exported to {}",
                    dest_file.to_string_lossy()
                );
            }
            Command::Concat {
                refs,
                transition,
                duration_ms,
            } => {
                let mut timeline = self.timeline.write().unwrap();
                let mut clips = Vec::new();
                let mut current_offset = 0u64;

                let trans_kind = match transition.as_deref() {
                    Some("Crossfade") => Some(TransitionKind::Crossfade),
                    Some("FadeToBlack") | Some("Dissolve") => Some(TransitionKind::Dissolve),
                    _ => None,
                };

                let mut track_kind = TrackKind::Video;

                for r in refs {
                    let asset = self.registry.resolve(r)?;
                    let duration = match asset.kind {
                        AssetKind::Video => {
                            track_kind = TrackKind::Video;
                            asset
                                .metadata
                                .get("duration")
                                .and_then(|v| v.as_f64())
                                .map(|d| (d * 1000.0) as u64)
                                .unwrap_or(5000)
                        }
                        AssetKind::Audio => {
                            track_kind = TrackKind::Audio;
                            asset
                                .metadata
                                .get("duration")
                                .and_then(|v| v.as_f64())
                                .map(|d| (d * 1000.0) as u64)
                                .unwrap_or(5000)
                        }
                        AssetKind::Image => {
                            track_kind = TrackKind::Video;
                            duration_ms.as_ref().map(|&d| d as u64).unwrap_or(5000)
                        }
                        AssetKind::Animation => {
                            track_kind = TrackKind::Video;
                            duration_ms.as_ref().map(|&d| d as u64).unwrap_or(5000)
                        }
                    };

                    clips.push(Clip {
                        asset_ref: *r,
                        in_point_ms: 0,
                        out_point_ms: duration,
                        track_offset_ms: current_offset,
                        transition: trans_kind,
                    });
                    current_offset += duration;
                }

                let track_name = format!("Concat Track {}", timeline.tracks.len() + 1);
                timeline.tracks.push(Track {
                    name: track_name,
                    kind: track_kind,
                    clips,
                });

                db.save_timeline(&timeline)?;
                msg = "Concat track created successfully".to_string();
            }
            Command::Overlay {
                base,
                overlay,
                x: _,
                y: _,
                blend,
                opacity,
            } => {
                let _base_asset = self.registry.resolve(base)?;
                let _overlay_asset = self.registry.resolve(overlay)?;

                let mut graph = self.graph.write().unwrap();
                let base_node_id = graph.nodes.keys().max().copied().unwrap_or(0) + 1;
                let overlay_node_id = base_node_id + 1;
                let blend_node_id = base_node_id + 2;
                let output_node_id = base_node_id + 3;

                let mode = match blend.as_deref() {
                    Some("Multiply") => BlendMode::Multiply,
                    Some("Screen") => BlendMode::Screen,
                    Some("Overlay") => BlendMode::Overlay,
                    Some("SoftLight") => BlendMode::SoftLight,
                    _ => BlendMode::Normal,
                };
                let op = opacity.unwrap_or(1.0);

                graph.add_node(Node {
                    id: base_node_id,
                    kind: NodeKind::Source(*base),
                });
                graph.add_node(Node {
                    id: overlay_node_id,
                    kind: NodeKind::Source(*overlay),
                });
                graph.add_node(Node {
                    id: blend_node_id,
                    kind: NodeKind::Blend { mode, opacity: op },
                });
                graph.add_node(Node {
                    id: output_node_id,
                    kind: NodeKind::Output,
                });

                graph.connect(GraphConnection {
                    from_node: base_node_id,
                    from_port: 0,
                    to_node: blend_node_id,
                    to_port: 0,
                })?;
                graph.connect(GraphConnection {
                    from_node: overlay_node_id,
                    from_port: 0,
                    to_node: blend_node_id,
                    to_port: 1,
                })?;
                graph.connect(GraphConnection {
                    from_node: blend_node_id,
                    from_port: 0,
                    to_node: output_node_id,
                    to_port: 0,
                })?;
                graph.output_node = Some(output_node_id);

                db.save_graph(&graph)?;
                msg = format!(
                    "Composite overlay graph created successfully with output node {}",
                    output_node_id
                );
            }
            Command::Speed { r, factor } => {
                let original = self.registry.resolve(r)?;
                let new_ref = self.registry.reserve_next(RefKind::Animation);
                *affected_ref = Some(new_ref);

                let speed_asset = aether_video::transitions::change_speed(
                    &original,
                    *factor,
                    new_ref,
                    &self.cache_dir,
                )?;

                self.registry.register(speed_asset.r, speed_asset.clone())?;
                db.save_asset(&speed_asset)?;
                msg = format!(
                    "Speed-adjusted asset successfully registered as {}",
                    speed_asset.r
                );
            }
            Command::Inspect { r, start, end } => {
                if let Some(asset_ref) = r {
                    let asset = self.registry.resolve(asset_ref)?;

                    let start_t = start
                        .as_ref()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let end_t = end
                        .as_ref()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(2.0);
                    let mid_t = (start_t + end_t) / 2.0;

                    let duration_sec = match asset.kind {
                        AssetKind::Video | AssetKind::Audio => asset
                            .metadata
                            .get("duration")
                            .and_then(|v| v.as_f64())
                            .map(|d| d as f32)
                            .unwrap_or(0.0),
                        AssetKind::Image | AssetKind::Animation => 0.0,
                    };

                    let mut proxy_video_path = None;
                    let mut proxy_audio_path = None;
                    let mut contact_sheet_path = None;
                    let mut rms = Vec::new();
                    let anomalies;
                    let mut audio_peaks_sec = Vec::new();

                    match asset.kind {
                        AssetKind::Video => {
                            let proxy_vid_p =
                                self.cache_dir.join(format!("proxy_vid_{}.mp4", asset.hash));
                            if proxy_vid_p.exists() {
                                proxy_video_path = Some(proxy_vid_p.to_string_lossy().to_string());
                            } else {
                                if let Ok(p) =
                                    observation::generate_video_proxy(&asset.path, &proxy_vid_p)
                                {
                                    proxy_video_path = Some(p.to_string_lossy().to_string());
                                }
                            }

                            let proxy_aud_p =
                                self.cache_dir.join(format!("proxy_aud_{}.mp3", asset.hash));
                            if proxy_aud_p.exists() {
                                proxy_audio_path = Some(proxy_aud_p.to_string_lossy().to_string());
                            } else {
                                if let Ok(p) =
                                    observation::generate_audio_proxy(&asset.path, &proxy_aud_p)
                                {
                                    proxy_audio_path = Some(p.to_string_lossy().to_string());
                                }
                            }

                            let output_dir = self
                                .cache_dir
                                .join(format!("inspect_frames_{}", asset.hash));
                            let times = vec![start_t, mid_t, end_t];
                            let mut frames = Vec::new();
                            if let Ok(f) =
                                observation::extract_keyframes(&asset.path, &times, &output_dir)
                            {
                                frames = f;
                                let contact_p = self
                                    .cache_dir
                                    .join(format!("contact_sheet_{}.png", asset.hash));
                                if observation::generate_contact_sheet(&frames, 3, 1, &contact_p)
                                    .is_ok()
                                {
                                    contact_sheet_path =
                                        Some(contact_p.to_string_lossy().to_string());
                                }
                            }

                            rms = observation::analyze_audio_rms(&asset.path).unwrap_or_default();
                            anomalies =
                                observation::detect_anomalies(&rms, &frames).unwrap_or_default();
                            audio_peaks_sec = observation::detect_audio_transients(&rms, 10.0);
                        }
                        AssetKind::Audio => {
                            let proxy_aud_p =
                                self.cache_dir.join(format!("proxy_aud_{}.mp3", asset.hash));
                            if proxy_aud_p.exists() {
                                proxy_audio_path = Some(proxy_aud_p.to_string_lossy().to_string());
                            } else {
                                if let Ok(p) =
                                    observation::generate_audio_proxy(&asset.path, &proxy_aud_p)
                                {
                                    proxy_audio_path = Some(p.to_string_lossy().to_string());
                                }
                            }

                            rms = observation::analyze_audio_rms(&asset.path).unwrap_or_default();
                            anomalies =
                                observation::detect_anomalies(&rms, &[]).unwrap_or_default();
                            audio_peaks_sec = observation::detect_audio_transients(&rms, 10.0);
                        }
                        AssetKind::Image | AssetKind::Animation => {
                            let frames = vec![asset.path.clone()];
                            anomalies =
                                observation::detect_anomalies(&[], &frames).unwrap_or_default();
                        }
                    }

                    let obs = SmartObservation {
                        asset_ref: *asset_ref,
                        asset_kind: asset.kind,
                        duration_sec,
                        proxy_video_path,
                        proxy_audio_path,
                        contact_sheet_path,
                        telemetry: TelemetryData {
                            anomalies,
                            audio_peaks_sec,
                            rms_levels: rms,
                        },
                    };

                    msg = serde_json::to_string_pretty(&obs).unwrap_or_default();
                } else {
                    msg = "No asset reference specified for inspection".to_string();
                }
            }
            Command::Eq {
                r,
                filter_type,
                freq_hz,
                gain_db,
                q,
            } => {
                let original = self.registry.resolve(r)?;
                if original.kind != AssetKind::Audio {
                    return Err(AetherError::InvalidCommand(
                        "Eq is only supported for audio assets".to_string(),
                    ));
                }
                let new_ref = self.registry.reserve_next(RefKind::Audio);
                *affected_ref = Some(new_ref);

                let eq_asset = aether_audio::apply_eq(
                    &original,
                    filter_type,
                    *freq_hz,
                    *gain_db,
                    *q,
                    new_ref,
                    &self.cache_dir,
                )?;

                self.registry.register(eq_asset.r, eq_asset.clone())?;
                db.save_asset(&eq_asset)?;
                msg = format!("EQ filtered audio asset registered as {}", eq_asset.r);
            }
            Command::Compress {
                r,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
            } => {
                let original = self.registry.resolve(r)?;
                if original.kind != AssetKind::Audio {
                    return Err(AetherError::InvalidCommand(
                        "Compress is only supported for audio assets".to_string(),
                    ));
                }
                let new_ref = self.registry.reserve_next(RefKind::Audio);
                *affected_ref = Some(new_ref);

                let comp_asset = aether_audio::apply_compressor(
                    &original,
                    *threshold_db,
                    *ratio,
                    *attack_ms,
                    *release_ms,
                    new_ref,
                    &self.cache_dir,
                )?;

                self.registry.register(comp_asset.r, comp_asset.clone())?;
                db.save_asset(&comp_asset)?;
                msg = format!("Compressed audio asset registered as {}", comp_asset.r);
            }
            Command::MixTracks {
                refs,
                volumes,
                pans,
            } => {
                let new_ref = self.registry.reserve_next(RefKind::Audio);
                *affected_ref = Some(new_ref);

                let mut assets = Vec::new();
                for r in refs {
                    assets.push(self.registry.resolve(r)?.clone());
                }

                let mixed_asset =
                    aether_audio::mix_tracks(&assets, volumes, pans, new_ref, &self.cache_dir)?;

                self.registry.register(mixed_asset.r, mixed_asset.clone())?;
                db.save_asset(&mixed_asset)?;
                msg = format!("Mixed track audio asset registered as {}", mixed_asset.r);
            }
            Command::KeyframeSet {
                r,
                property,
                time_ms,
                value,
                easing,
            } => {
                let ease_str = easing.as_deref().unwrap_or("Linear");
                db.save_keyframe(&r.to_string(), property, *time_ms, *value, ease_str)?;
                msg = format!(
                    "Keyframe set successfully: {} = {} at {}ms",
                    property, value, time_ms
                );
            }
            Command::KeyframeList { r, property } => {
                let kfs = db.load_keyframes(&r.to_string(), property)?;
                let mut list = Vec::new();
                for k in kfs {
                    list.push(format!("{}ms: {} ({})", k.0, k.1, k.2));
                }
                msg = format!(
                    "Keyframes for {} (property: {}):\n{}",
                    r,
                    property,
                    list.join("\n")
                );
            }
            Command::ExportEdl { output_path } => {
                let timeline = self.timeline.read().unwrap();
                aether_persistence::edl::export_edl(&timeline, &self.registry, output_path)?;
                msg = format!("EDL successfully exported to {}", output_path);
            }
            Command::ExportOtio { output_path } => {
                let timeline = self.timeline.read().unwrap();
                aether_persistence::otio::export_otio(&timeline, output_path)?;
                msg = format!("OTIO successfully exported to {}", output_path);
            }
            Command::Undo => {
                let cur_commit = self.current_commit.read().unwrap().clone();
                if cur_commit.is_empty() {
                    return Err(AetherError::OperationFailed(
                        "Already at first history state".to_string(),
                    ));
                }
                let history = db.load_history()?;
                let parent = if let Some(pos) = history.iter().position(|h| h.0 == cur_commit) {
                    history[pos].1.clone().unwrap_or_default()
                } else {
                    "".to_string()
                };

                self.checkout_commit(db, &parent)?;
                msg = format!("Successfully rolled back project to state '{}'", parent);
            }
            Command::Redo => {
                let cur_commit = self.current_commit.read().unwrap().clone();
                let history = db.load_history()?;
                let cur_branch = self.current_branch.read().unwrap().clone();
                let next_commit = history
                    .iter()
                    .find(|h| h.1.as_deref() == Some(&cur_commit) && h.2 == cur_branch);

                if let Some(h) = next_commit {
                    self.checkout_commit(db, &h.0)?;
                    msg = format!("Successfully fast-forwarded project to state '{}'", h.0);
                } else {
                    return Err(AetherError::OperationFailed(
                        "Already at newest history state".to_string(),
                    ));
                }
            }
            Command::GenerateStoryboardScratch {
                request,
                model,
                options,
            } => {
                msg = self.execute_generation_request(
                    db,
                    GenerationKind::StoryboardScratch,
                    request.clone(),
                    model.clone(),
                    Vec::new(),
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::GenerateDialogue {
                request,
                model,
                options,
            } => {
                msg = self.execute_generation_request(
                    db,
                    GenerationKind::Dialogue,
                    request.clone(),
                    model.clone(),
                    Vec::new(),
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::GenerateImage {
                request,
                model,
                inputs,
                options,
            } => {
                // Validate inputs are existing assets
                for input in inputs {
                    self.registry.resolve(input).map_err(|_| {
                        AetherError::InvalidCommand(format!("Input reference {} not found", input))
                    })?;
                }
                msg = self.execute_generation_request(
                    db,
                    GenerationKind::Image,
                    request.clone(),
                    model.clone(),
                    inputs.clone(),
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::EditImage {
                target,
                request,
                model,
                options,
            } => {
                // Validate target is an existing image asset
                let asset = self.registry.resolve(target).map_err(|_| {
                    AetherError::InvalidCommand(format!("Image reference {} not found", target))
                })?;
                if asset.kind != AssetKind::Image {
                    return Err(AetherError::InvalidCommand(format!(
                        "Reference {} is not an image asset",
                        target
                    )));
                }

                msg = self.execute_generation_request(
                    db,
                    GenerationKind::ImageEdit,
                    request.clone(),
                    model.clone(),
                    vec![*target],
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::GenerateVoice {
                text,
                voice,
                model,
                options,
            } => {
                msg = self.execute_generation_request(
                    db,
                    GenerationKind::Voice,
                    format!("voice: {:?}, text: {}", voice, text),
                    model.clone(),
                    Vec::new(),
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::CloneVoice {
                sample,
                name,
                model,
                options,
            } => {
                // Validate sample is an existing audio asset
                let asset = self.registry.resolve(sample).map_err(|_| {
                    AetherError::InvalidCommand(format!("Audio reference {} not found", sample))
                })?;
                if asset.kind != AssetKind::Audio {
                    return Err(AetherError::InvalidCommand(format!(
                        "Reference {} is not an audio asset",
                        sample
                    )));
                }

                msg = self.execute_generation_request(
                    db,
                    GenerationKind::VoiceClone,
                    format!("name: {:?}", name),
                    model.clone(),
                    vec![*sample],
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::GenerateSceneAudio {
                request,
                model,
                options,
            } => {
                msg = self.execute_generation_request(
                    db,
                    GenerationKind::SceneAudio,
                    request.clone(),
                    model.clone(),
                    Vec::new(),
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::GenerateMusic {
                request,
                model,
                options,
            } => {
                msg = self.execute_generation_request(
                    db,
                    GenerationKind::Music,
                    request.clone(),
                    model.clone(),
                    Vec::new(),
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::GenerateVideoFromText {
                request,
                model,
                options,
            } => {
                msg = self.execute_generation_request(
                    db,
                    GenerationKind::VideoText,
                    request.clone(),
                    model.clone(),
                    Vec::new(),
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::GenerateVideoFromFrame {
                frame,
                request,
                model,
                options,
            } => {
                // Validate frame is an existing image asset
                let asset = self.registry.resolve(frame).map_err(|_| {
                    AetherError::InvalidCommand(format!("Image reference {} not found", frame))
                })?;
                if asset.kind != AssetKind::Image {
                    return Err(AetherError::InvalidCommand(format!(
                        "Reference {} is not an image asset",
                        frame
                    )));
                }

                msg = self.execute_generation_request(
                    db,
                    GenerationKind::VideoFrame,
                    request.clone(),
                    model.clone(),
                    vec![*frame],
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::GenerateVideoFromIngredients {
                inputs,
                request,
                model,
                options,
            } => {
                // Validate inputs are existing assets
                for input in inputs {
                    self.registry.resolve(input).map_err(|_| {
                        AetherError::InvalidCommand(format!("Input reference {} not found", input))
                    })?;
                }
                msg = self.execute_generation_request(
                    db,
                    GenerationKind::VideoIngredients,
                    request.clone(),
                    model.clone(),
                    inputs.clone(),
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::EditVideo {
                target,
                request,
                model,
                options,
            } => {
                // Validate target is an existing video asset
                let asset = self.registry.resolve(target).map_err(|_| {
                    AetherError::InvalidCommand(format!("Video reference {} not found", target))
                })?;
                if asset.kind != AssetKind::Video {
                    return Err(AetherError::InvalidCommand(format!(
                        "Reference {} is not a video asset",
                        target
                    )));
                }

                msg = self.execute_generation_request(
                    db,
                    GenerationKind::VideoEdit,
                    request.clone(),
                    model.clone(),
                    vec![*target],
                    options.clone(),
                    affected_ref,
                )?;
            }
            Command::CancelGeneration { r } => {
                let mut job = db.load_generation_job(r)?;
                self.runtime.cancel(&mut job)?;
                db.save_generation_job(&job)?;
                db.add_generation_event(r, &GenerationStatus::Cancelled, "Job cancelled by user")?;
                msg = format!("Successfully requested cancellation for job {}", r);
            }
            Command::GenerationStatus { r } => {
                msg = match r {
                    Some(job_ref) => {
                        let job = db.load_generation_job(job_ref)?;
                        serde_json::to_string_pretty(&job).map_err(|e| {
                            AetherError::OperationFailed(format!(
                                "Serialize GenerationJob failed: {}",
                                e
                            ))
                        })?
                    }
                    None => {
                        let all_jobs = db.load_all_generation_jobs()?;
                        serde_json::to_string_pretty(&all_jobs).map_err(|e| {
                            AetherError::OperationFailed(format!(
                                "Serialize Vec<GenerationJob> failed: {}",
                                e
                            ))
                        })?
                    }
                };
            }
            Command::Snapshot => {
                msg = "Retrieved snapshot".to_string();
            }
            Command::Shutdown => {
                msg = "Daemon shutting down...".to_string();
            }
            _ => {
                return Err(AetherError::InvalidCommand(
                    "Unsupported command".to_string(),
                ));
            }
        }
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project_dir() -> PathBuf {
        let unique_dir = format!(
            "test_project_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        std::env::temp_dir().join(unique_dir)
    }

    #[test]
    fn test_daemon_session_initialization() {
        let dir = temp_project_dir();
        let session = SessionManager::new(&dir).unwrap();

        let snap = session.get_snapshot().unwrap();
        assert_eq!(snap.settings, ProjectSettings::default());
        assert_eq!(snap.assets.len(), 0);
        assert_eq!(snap.history_len, 0);
        assert_eq!(snap.history_cursor, 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_daemon_init_settings_command() {
        let dir = temp_project_dir();
        let session = SessionManager::new(&dir).unwrap();

        let cmd = Command::Init {
            fps: Some(60.0),
            resolution: Some("3840x2160".to_string()),
            colorspace: Some("rec2020".to_string()),
        };

        let result = session.execute(cmd).unwrap();
        assert!(result.success);

        let snap = session.get_snapshot().unwrap();
        assert_eq!(snap.settings.fps, 60.0);
        assert_eq!(snap.settings.width, 3840);
        assert_eq!(snap.settings.height, 2160);
        assert_eq!(snap.settings.colorspace, "rec2020");
        assert_eq!(snap.history_len, 1);
        assert_eq!(snap.history_cursor, 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_daemon_canvas_and_draw_text_with_undo_redo() {
        let dir = temp_project_dir();
        let session = SessionManager::new(&dir).unwrap();

        // 1. Create canvas
        let cmd_canvas = Command::Canvas {
            width: 100,
            height: 100,
            color: "red".to_string(),
        };
        let res_canvas = session.execute(cmd_canvas).unwrap();
        assert!(res_canvas.success);
        let canvas_ref = res_canvas.affected_ref.unwrap();

        let snap = session.get_snapshot().unwrap();
        assert_eq!(snap.assets.len(), 1);
        assert_eq!(snap.history_len, 1);
        assert_eq!(snap.history_cursor, 1);

        // 2. Draw text
        let cmd_text = Command::DrawText {
            r: canvas_ref,
            text: "Hello".to_string(),
            font: "LiberationSans-Regular".to_string(),
            size: 16.0,
            x: 10,
            y: 10,
        };
        let res_text = session.execute(cmd_text).unwrap();
        assert!(res_text.success);
        let text_ref = res_text.affected_ref.unwrap();

        let snap = session.get_snapshot().unwrap();
        assert_eq!(snap.assets.len(), 2);
        assert_eq!(snap.history_len, 2);
        assert_eq!(snap.history_cursor, 2);

        // 3. Undo
        let res_undo = session.execute(Command::Undo).unwrap();
        assert!(res_undo.success);

        let snap = session.get_snapshot().unwrap();
        assert_eq!(snap.assets.len(), 1); // text draw undone
        assert_eq!(snap.history_cursor, 1);

        // Verify the memory registry contains ONLY the canvas
        assert!(session.registry.resolve(&canvas_ref).is_ok());
        assert!(session.registry.resolve(&text_ref).is_err());

        // 4. Redo
        let res_redo = session.execute(Command::Redo).unwrap();
        assert!(res_redo.success);

        let snap = session.get_snapshot().unwrap();
        assert_eq!(snap.assets.len(), 2); // text draw redone
        assert_eq!(snap.history_cursor, 2);
        assert!(session.registry.resolve(&text_ref).is_ok());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_phase2_complete_capabilities() {
        let dir = temp_project_dir();
        let session = SessionManager::new(&dir).unwrap();

        // 1. Create Canvas (Image Asset @img1) to act as base
        let cmd_canvas1 = Command::Canvas {
            width: 100,
            height: 100,
            color: "blue".to_string(),
        };
        let res_canvas1 = session.execute(cmd_canvas1).unwrap();
        let canvas1_ref = res_canvas1.affected_ref.unwrap();

        // 2. Create another Canvas (Image Asset @img2) to act as overlay
        let cmd_canvas2 = Command::Canvas {
            width: 100,
            height: 100,
            color: "green".to_string(),
        };
        let res_canvas2 = session.execute(cmd_canvas2).unwrap();
        let canvas2_ref = res_canvas2.affected_ref.unwrap();

        // 3. Test Overlay
        let cmd_overlay = Command::Overlay {
            base: canvas1_ref,
            overlay: canvas2_ref,
            x: 0,
            y: 0,
            blend: Some("Multiply".to_string()),
            opacity: Some(0.8),
        };
        let res_overlay = session.execute(cmd_overlay).unwrap();
        assert!(res_overlay.success);

        let snap = session.get_snapshot().unwrap();
        assert_eq!(snap.graph.nodes.len(), 4);
        assert!(snap.graph.output_node.is_some());

        // 4. Test Concat
        let cmd_concat = Command::Concat {
            refs: vec![canvas1_ref, canvas2_ref],
            transition: Some("Crossfade".to_string()),
            duration_ms: Some(3000),
        };
        let res_concat = session.execute(cmd_concat).unwrap();
        assert!(res_concat.success);

        let snap = session.get_snapshot().unwrap();
        assert_eq!(snap.timeline.tracks.len(), 1);
        assert_eq!(snap.timeline.tracks[0].clips.len(), 2);
        assert_eq!(snap.timeline.tracks[0].clips[0].asset_ref, canvas1_ref);

        // 5. Test Speed
        let cmd_speed = Command::Speed {
            r: canvas1_ref,
            factor: 2.0,
        };
        let res_speed = session.execute(cmd_speed).unwrap();
        assert!(res_speed.success);
        let speed_ref = res_speed.affected_ref.unwrap();
        let speed_asset = session.registry.resolve(&speed_ref).unwrap();
        assert_eq!(
            speed_asset
                .metadata
                .get("speed_factor")
                .and_then(|v| v.as_f64())
                .unwrap(),
            2.0
        );

        // 6. Test Keyframes
        let cmd_kf_set = Command::KeyframeSet {
            r: canvas1_ref,
            property: "opacity".to_string(),
            time_ms: 1000,
            value: 0.5,
            easing: Some("EaseInOut".to_string()),
        };
        let res_kf = session.execute(cmd_kf_set).unwrap();
        assert!(res_kf.success);

        let cmd_kf_list = Command::KeyframeList {
            r: canvas1_ref,
            property: "opacity".to_string(),
        };
        let res_list = session.execute(cmd_kf_list).unwrap();
        assert!(res_list.success);
        assert!(res_list.message.contains("1000ms: 0.5 (EaseInOut)"));

        // 7. Test Export EDL and OTIO
        let edl_path = dir.join("export.edl");
        let cmd_edl = Command::ExportEdl {
            output_path: edl_path.to_string_lossy().to_string(),
        };
        let res_edl = session.execute(cmd_edl).unwrap();
        assert!(res_edl.success);
        assert!(edl_path.exists());

        let otio_path = dir.join("export.otio");
        let cmd_otio = Command::ExportOtio {
            output_path: otio_path.to_string_lossy().to_string(),
        };
        let res_otio = session.execute(cmd_otio).unwrap();
        assert!(res_otio.success);
        assert!(otio_path.exists());

        // 8. Test Branch & Checkout
        // Create new branch "dev" pointing to current state
        let cmd_branch = Command::Branch {
            name: "dev".to_string(),
        };
        let res_branch = session.execute(cmd_branch).unwrap();
        assert!(res_branch.success);

        // Switch back to "main" (which was detached or active)
        let cmd_checkout = Command::Checkout {
            name: "dev".to_string(),
        };
        let res_checkout = session.execute(cmd_checkout).unwrap();
        assert!(res_checkout.success);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_observation_engine_and_inspect() {
        let dir = temp_project_dir();
        let session = SessionManager::new(&dir).unwrap();

        // 1. Create Canvas
        let cmd_canvas = Command::Canvas {
            width: 100,
            height: 100,
            color: "red".to_string(),
        };
        let res_canvas = session.execute(cmd_canvas).unwrap();
        let canvas_ref = res_canvas.affected_ref.unwrap();

        // 2. Inspect the Canvas Asset
        let cmd_inspect = Command::Inspect {
            r: Some(canvas_ref),
            start: Some("0.0".to_string()),
            end: Some("2.0".to_string()),
        };
        let res_inspect = session.execute(cmd_inspect).unwrap();
        assert!(res_inspect.success);
        let obs: SmartObservation = serde_json::from_str(&res_inspect.message)
            .expect("Failed to parse SmartObservation JSON");
        assert_eq!(obs.asset_kind, AssetKind::Image);
        assert!(obs.telemetry.anomalies.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_failure_rollback() {
        let dir = temp_project_dir();
        let session = SessionManager::new(&dir).unwrap();

        // 1. Create a valid Canvas (Image Asset)
        let cmd_canvas = Command::Canvas {
            width: 100,
            height: 100,
            color: "blue".to_string(),
        };
        let res_canvas = session.execute(cmd_canvas).unwrap();
        assert!(res_canvas.success);
        let canvas_ref = res_canvas.affected_ref.unwrap();

        // Save original commit hash
        let orig_commit = session.current_commit.read().unwrap().clone();

        // 2. Trigger an expected error path in process_command (trim an Image asset, which is invalid)
        let cmd_invalid = Command::Trim {
            r: canvas_ref,
            start: "0.0".to_string(),
            end: "1.0".to_string(),
        };
        let res_invalid = session.execute(cmd_invalid);
        assert!(
            res_invalid.is_err(),
            "Expected command to fail and return an Err"
        );

        // 3. Verify in-memory state is completely rolled back
        let snap_after = session.get_snapshot().unwrap();
        assert_eq!(session.current_commit.read().unwrap().clone(), orig_commit);
        assert_eq!(snap_after.assets.len(), 1);

        // 4. Verify that physical SQLite database is also fully rolled back (active tables have no invalid/partially-created assets)
        {
            let db = session.db.lock().unwrap();
            let active_assets = db.load_checkpoint(&orig_commit).unwrap().2;
            assert_eq!(active_assets.len(), 1);
            assert_eq!(active_assets[0].r, canvas_ref);
        }

        // 5. Verify the registry counter was rolled back: the subsequent valid command must succeed perfectly using the reclaimed reference ID 2
        let cmd_draw = Command::DrawText {
            r: canvas_ref,
            text: "Hello".to_string(),
            font: "Roboto".to_string(),
            size: 12.0,
            x: 10,
            y: 10,
        };
        let res_draw = session.execute(cmd_draw).unwrap();
        assert!(res_draw.success);
        assert_eq!(res_draw.affected_ref.unwrap().id, 2); // Perfectly reused reclaimed ID 2!

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_branching_and_checkout() {
        let dir = temp_project_dir();
        let session = SessionManager::new(&dir).unwrap();

        // 1. Create a canvas on "main" branch
        let cmd_canvas = Command::Canvas {
            width: 100,
            height: 100,
            color: "red".to_string(),
        };
        let res_canvas = session.execute(cmd_canvas).unwrap();
        let canvas_ref = res_canvas.affected_ref.unwrap();

        let snap_main_init = session.get_snapshot().unwrap();
        assert_eq!(session.current_branch.read().unwrap().clone(), "main");
        assert_eq!(snap_main_init.assets.len(), 1);

        // 2. Create new branch "feature-edit"
        let cmd_branch = Command::Branch {
            name: "feature-edit".to_string(),
        };
        let res_branch = session.execute(cmd_branch).unwrap();
        assert!(res_branch.success);

        // 3. Switch to "feature-edit" branch
        let cmd_checkout_feature = Command::Checkout {
            name: "feature-edit".to_string(),
        };
        let res_checkout_feature = session.execute(cmd_checkout_feature).unwrap();
        assert!(res_checkout_feature.success);
        assert_eq!(
            session.current_branch.read().unwrap().clone(),
            "feature-edit"
        );

        // 4. Modify asset on "feature-edit" branch by drawing text
        let cmd_draw = Command::DrawText {
            r: canvas_ref,
            text: "Branch Edit".to_string(),
            font: "Roboto".to_string(),
            size: 16.0,
            x: 20,
            y: 20,
        };
        let res_draw = session.execute(cmd_draw).unwrap();
        assert!(res_draw.success);
        let text_ref = res_draw.affected_ref.unwrap();

        let snap_feature = session.get_snapshot().unwrap();
        assert_eq!(
            session.current_branch.read().unwrap().clone(),
            "feature-edit"
        );
        assert_eq!(snap_feature.assets.len(), 2);
        assert!(session.registry.resolve(&text_ref).is_ok());

        // 5. Checkout "main" branch again
        let cmd_checkout_main = Command::Checkout {
            name: "main".to_string(),
        };
        let res_checkout_main = session.execute(cmd_checkout_main).unwrap();
        assert!(res_checkout_main.success);

        let snap_main_switched = session.get_snapshot().unwrap();
        assert_eq!(session.current_branch.read().unwrap().clone(), "main");
        assert_eq!(snap_main_switched.assets.len(), 1); // Only original canvas remains
        assert!(session.registry.resolve(&text_ref).is_err()); // Not resolved on main!

        // Verify database active assets contains only 1 asset on main branch
        {
            let db = session.db.lock().unwrap();
            let active_assets = db
                .load_checkpoint(&session.current_commit.read().unwrap().clone())
                .unwrap()
                .2;
            assert_eq!(active_assets.len(), 1);
        }

        // 6. Checkout "feature-edit" branch again
        let res_checkout_feature_again = session
            .execute(Command::Checkout {
                name: "feature-edit".to_string(),
            })
            .unwrap();
        assert!(res_checkout_feature_again.success);

        let snap_feature_again = session.get_snapshot().unwrap();
        assert_eq!(
            session.current_branch.read().unwrap().clone(),
            "feature-edit"
        );
        assert_eq!(snap_feature_again.assets.len(), 2); // Both are restored!
        assert!(session.registry.resolve(&text_ref).is_ok());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_generation_commands_execution() {
        let dir = temp_project_dir();
        let session = SessionManager::new(&dir).unwrap();

        // 1. Generate image using mock provider
        let cmd_img = Command::GenerateImage {
            request: "A realistic blue dragon".to_string(),
            model: Some("mock/image".to_string()),
            inputs: Vec::new(),
            options: serde_json::json!({}),
        };
        let res_img = session.execute(cmd_img).unwrap();
        assert!(res_img.success);
        let job_ref = res_img.affected_ref.unwrap();

        // 2. Query status
        let cmd_status = Command::GenerationStatus { r: Some(job_ref) };
        let res_status = session.execute(cmd_status).unwrap();
        assert!(res_status.success);
        assert!(res_status.message.contains("Ready"));

        // 3. Cancel a job
        let cmd_cancel = Command::CancelGeneration { r: job_ref };
        let res_cancel = session.execute(cmd_cancel).unwrap();
        assert!(res_cancel.success);

        // Verify it was marked Cancelled or updated
        let snap = session.get_snapshot().unwrap();
        assert_eq!(snap.generation_jobs.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }
}
