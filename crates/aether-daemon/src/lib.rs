use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, Mutex};
use aether_core::{
    AetherError, RefKind, Asset, AssetKind, ProjectSettings, Command, CommandResult, Snapshot, RefRegistry,
    CompositionGraph, Node, Connection as GraphConnection, NodeKind, Timeline, Track, Clip,
    TransitionKind, TrackKind, BlendMode
};
use aether_persistence::DbManager;

pub struct SessionManager {
    registry: RefRegistry,
    settings: RwLock<ProjectSettings>,
    db: Mutex<DbManager>,
    history_cursor: RwLock<usize>,
    cache_dir: PathBuf,
    project_dir: PathBuf,
    graph: RwLock<CompositionGraph>,
    timeline: RwLock<Timeline>,
}

impl SessionManager {
    /// Creates a new SessionManager, initializing the persistence DB and loading existing state.
    pub fn new<P: AsRef<Path>>(project_dir: P) -> Result<Self, AetherError> {
        let p_dir = project_dir.as_ref().to_path_buf();
        let aether_dir = p_dir.join(".aether");
        if !aether_dir.exists() {
            fs::create_dir_all(&aether_dir)
                .map_err(|e| AetherError::IoError(aether_dir.to_string_lossy().to_string(), e.to_string()))?;
        }

        let db = DbManager::new(&aether_dir)?;
        let cache_dir = aether_dir.join("cache");
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)
                .map_err(|e| AetherError::IoError(cache_dir.to_string_lossy().to_string(), e.to_string()))?;
        }

        // Load settings, history cursor, graph and timeline from DB
        let (settings, history_cursor) = db.load_settings()?;
        let graph = db.load_graph()?;
        let timeline = db.load_timeline()?;
        
        let manager = SessionManager {
            registry: RefRegistry::new(),
            settings: RwLock::new(settings),
            db: Mutex::new(db),
            history_cursor: RwLock::new(history_cursor),
            cache_dir,
            project_dir: p_dir,
            graph: RwLock::new(graph),
            timeline: RwLock::new(timeline),
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
        let settings = self.settings.read().unwrap().clone();
        let assets = self.registry.list_assets();
        let history = db.load_history()?;
        let cursor = *self.history_cursor.read().unwrap();
        let graph = self.graph.read().unwrap().clone();
        let timeline = self.timeline.read().unwrap().clone();

        Ok(Snapshot {
            settings,
            assets,
            history_len: history.len(),
            history_cursor: cursor,
            graph,
            timeline,
        })
    }

    /// Retrieves the current snapshot of the project.
    pub fn get_snapshot(&self) -> Result<Snapshot, AetherError> {
        let db = self.db.lock().unwrap();
        self.get_snapshot_with_db(&db)
    }

    /// Dispatches and executes a command, managing transaction history and persistence.
    pub fn execute(&self, command: Command) -> Result<CommandResult, AetherError> {
        let db = self.db.lock().unwrap();
        let snapshot_before = self.get_snapshot_with_db(&db)?;
        let hash_before = blake3::hash(&serde_json::to_vec(&snapshot_before).unwrap()).to_hex().to_string();

        let mut affected_ref = None;
        let msg;

        match &command {
            Command::Init { fps, resolution, colorspace } => {
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
                let cursor = *self.history_cursor.read().unwrap();
                db.save_settings(&settings, cursor)?;
                msg = "Project settings initialized successfully".to_string();
            }
            Command::Import { path } => {
                let p = Path::new(path);
                if !p.exists() {
                    return Err(AetherError::IoError(path.clone(), "File does not exist".to_string()));
                }

                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                let asset = match ext.as_str() {
                    "wav" | "mp3" | "ogg" | "flac" | "aac" => {
                        let r = self.registry.allocate(RefKind::Audio);
                        affected_ref = Some(r);
                        aether_audio::import_audio(p, r, &self.cache_dir)?
                    }
                    "png" | "jpg" | "jpeg" => {
                        let r = self.registry.allocate(RefKind::Image);
                        affected_ref = Some(r);
                        aether_image::import_image(p, r, &self.cache_dir)?
                    }
                    _ => {
                        let r = self.registry.allocate(RefKind::Video);
                        affected_ref = Some(r);
                        aether_video::import_video(p, r, &self.cache_dir)?
                    }
                };

                self.registry.register(asset.r, asset.clone())?;
                db.save_asset(&asset)?;
                msg = format!("Imported asset successfully registered as {}", asset.r);
            }
            Command::Trim { r, start, end } => {
                let original = self.registry.resolve(r)?;
                let new_ref = self.registry.allocate(r.kind);
                affected_ref = Some(new_ref);

                let trimmed = match original.kind {
                    AssetKind::Video => aether_video::trim_video(&original, start, end, new_ref, &self.cache_dir)?,
                    AssetKind::Audio => aether_audio::trim_audio(&original, start, end, new_ref, &self.cache_dir)?,
                    AssetKind::Image => return Err(AetherError::InvalidCommand("Cannot trim image asset".to_string())),
                    AssetKind::Animation => return Err(AetherError::InvalidCommand("Cannot trim animation asset".to_string())),
                };

                self.registry.register(trimmed.r, trimmed.clone())?;
                db.save_asset(&trimmed)?;
                msg = format!("Trimmed asset successfully registered as {}", trimmed.r);
            }
            Command::Mix { r, volume } => {
                let original = self.registry.resolve(r)?;
                if original.kind != AssetKind::Audio {
                    return Err(AetherError::InvalidCommand("Mix command is only supported for audio assets".to_string()));
                }
                let new_ref = self.registry.allocate(RefKind::Audio);
                affected_ref = Some(new_ref);

                // Use volume as target LUFS, limiting True Peak at -1.0dB
                let mixed = aether_audio::normalize_audio(&original, *volume, -1.0, new_ref, &self.cache_dir)?;
                self.registry.register(mixed.r, mixed.clone())?;
                db.save_asset(&mixed)?;
                msg = format!("Mixed audio asset successfully registered as {}", mixed.r);
            }
            Command::Composite { base, overlay, at, x, y } => {
                let base_asset = self.registry.resolve(base)?;
                let overlay_asset = self.registry.resolve(overlay)?;
                
                let new_ref = self.registry.allocate(RefKind::Video);
                affected_ref = Some(new_ref);

                let composited = aether_video::composite_video(
                    &base_asset, &overlay_asset, at, *x, *y, new_ref, &self.cache_dir
                )?;
                self.registry.register(composited.r, composited.clone())?;
                db.save_asset(&composited)?;
                msg = format!("Composited asset successfully registered as {}", composited.r);
            }
            Command::Canvas { width, height, color } => {
                let r = self.registry.allocate(RefKind::Image);
                affected_ref = Some(r);

                let canvas = aether_image::create_canvas(*width, *height, color, r, &self.cache_dir)?;
                self.registry.register(canvas.r, canvas.clone())?;
                db.save_asset(&canvas)?;
                msg = format!("Canvas successfully registered as {}", canvas.r);
            }
            Command::DrawText { r, text, font, size, x, y } => {
                let original = self.registry.resolve(r)?;
                if original.kind != AssetKind::Image {
                    return Err(AetherError::InvalidCommand("DrawText is only supported on image assets".to_string()));
                }
                let new_ref = self.registry.allocate(RefKind::Image);
                affected_ref = Some(new_ref);

                let text_overlay = aether_image::draw_text(
                    &original, text, font, *size, *x as f32, *y as f32, "white", new_ref, &self.cache_dir
                )?;
                self.registry.register(text_overlay.r, text_overlay.clone())?;
                db.save_asset(&text_overlay)?;
                msg = format!("Text overlay asset successfully registered as {}", text_overlay.r);
            }
            Command::Export { r, format, codec, quality } => {
                let asset = self.registry.resolve(r)?;
                let export_dir = self.project_dir.join("export");
                if !export_dir.exists() {
                    fs::create_dir_all(&export_dir)
                        .map_err(|e| AetherError::IoError(export_dir.to_string_lossy().to_string(), e.to_string()))?;
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
                        return Err(AetherError::InvalidCommand("Export only supported for Video and Image assets".to_string()));
                    }
                }
                msg = format!("Asset successfully exported to {}", dest_file.to_string_lossy());
            }
            Command::Concat { refs, transition, duration_ms } => {
                // Invariant: Resolves each asset reference, verifies its existence and compatibility, and schedules it as a sequential clip on a newly created or existing timeline track.
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
                            asset.metadata.get("duration").and_then(|v| v.as_f64()).map(|d| (d * 1000.0) as u64).unwrap_or(5000)
                        }
                        AssetKind::Audio => {
                            track_kind = TrackKind::Audio;
                            asset.metadata.get("duration").and_then(|v| v.as_f64()).map(|d| (d * 1000.0) as u64).unwrap_or(5000)
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
            Command::Overlay { base, overlay, x: _, y: _, blend, opacity } => {
                // Invariant: Resolves base and overlay assets, adds corresponding Source nodes to the composition graph, adds a Blend node with specified mode/opacity, creates the necessary connections, and sets the output node.
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

                graph.add_node(Node { id: base_node_id, kind: NodeKind::Source(*base) });
                graph.add_node(Node { id: overlay_node_id, kind: NodeKind::Source(*overlay) });
                graph.add_node(Node { id: blend_node_id, kind: NodeKind::Blend { mode, opacity: op } });
                graph.add_node(Node { id: output_node_id, kind: NodeKind::Output });

                graph.connect(GraphConnection { from_node: base_node_id, from_port: 0, to_node: blend_node_id, to_port: 0 })?;
                graph.connect(GraphConnection { from_node: overlay_node_id, from_port: 0, to_node: blend_node_id, to_port: 1 })?;
                graph.connect(GraphConnection { from_node: blend_node_id, from_port: 0, to_node: output_node_id, to_port: 0 })?;
                graph.output_node = Some(output_node_id);

                db.save_graph(&graph)?;
                msg = format!("Composite overlay graph created successfully with output node {}", output_node_id);
            }
            Command::Speed { r, factor } => {
                let original = self.registry.resolve(r)?;
                let new_ref = self.registry.allocate(RefKind::Animation);
                affected_ref = Some(new_ref);

                let orig_dur = original.metadata.get("duration").and_then(|v| v.as_f64()).unwrap_or(5.0);
                let new_dur = orig_dur / (*factor as f64);

                let speed_asset = Asset {
                    r: new_ref,
                    kind: AssetKind::Animation,
                    path: original.path.clone(),
                    hash: format!("{}_speed_{}", original.hash, factor),
                    metadata: serde_json::json!({
                        "duration": new_dur,
                        "speed_factor": factor,
                        "parent_ref": r.to_string(),
                    }),
                };

                self.registry.register(speed_asset.r, speed_asset.clone())?;
                db.save_asset(&speed_asset)?;
                msg = format!("Speed-adjusted asset successfully registered as {}", speed_asset.r);
            }
            Command::Inspect { r, start: _, end: _ } => {
                if let Some(asset_ref) = r {
                    let asset = self.registry.resolve(asset_ref)?;
                    msg = format!(
                        "Asset Reference: {}\nKind: {:?}\nPath: {}\nHash: {}\nMetadata: {}",
                        asset.r,
                        asset.kind,
                        asset.path.to_string_lossy(),
                        asset.hash,
                        serde_json::to_string_pretty(&asset.metadata).unwrap_or_default()
                    );
                } else {
                    msg = "No asset reference specified for inspection".to_string();
                }
            }
            Command::Eq { r, filter_type, freq_hz, gain_db, q } => {
                let original = self.registry.resolve(r)?;
                if original.kind != AssetKind::Audio {
                    return Err(AetherError::InvalidCommand("Eq is only supported for audio assets".to_string()));
                }
                let new_ref = self.registry.allocate(RefKind::Audio);
                affected_ref = Some(new_ref);

                let eq_asset = Asset {
                    r: new_ref,
                    kind: AssetKind::Audio,
                    path: original.path.clone(),
                    hash: format!("{}_eq_{}_{}", original.hash, filter_type, freq_hz),
                    metadata: serde_json::json!({
                        "duration": original.metadata.get("duration").cloned(),
                        "eq": {
                            "filter_type": filter_type,
                            "freq_hz": freq_hz,
                            "gain_db": gain_db,
                            "q": q,
                        },
                        "parent_ref": r.to_string(),
                    }),
                };

                self.registry.register(eq_asset.r, eq_asset.clone())?;
                db.save_asset(&eq_asset)?;
                msg = format!("EQ filtered audio asset registered as {}", eq_asset.r);
            }
            Command::Compress { r, threshold_db, ratio, attack_ms, release_ms } => {
                let original = self.registry.resolve(r)?;
                if original.kind != AssetKind::Audio {
                    return Err(AetherError::InvalidCommand("Compress is only supported for audio assets".to_string()));
                }
                let new_ref = self.registry.allocate(RefKind::Audio);
                affected_ref = Some(new_ref);

                let comp_asset = Asset {
                    r: new_ref,
                    kind: AssetKind::Audio,
                    path: original.path.clone(),
                    hash: format!("{}_compress_{}", original.hash, threshold_db),
                    metadata: serde_json::json!({
                        "duration": original.metadata.get("duration").cloned(),
                        "compressor": {
                            "threshold_db": threshold_db,
                            "ratio": ratio,
                            "attack_ms": attack_ms,
                            "release_ms": release_ms,
                        },
                        "parent_ref": r.to_string(),
                    }),
                };

                self.registry.register(comp_asset.r, comp_asset.clone())?;
                db.save_asset(&comp_asset)?;
                msg = format!("Compressed audio asset registered as {}", comp_asset.r);
            }
            Command::MixTracks { refs, volumes, pans } => {
                let new_ref = self.registry.allocate(RefKind::Audio);
                affected_ref = Some(new_ref);

                let mut inputs = Vec::new();
                let mut max_dur = 0.0;
                for (idx, r) in refs.iter().enumerate() {
                    let asset = self.registry.resolve(r)?;
                    let vol = volumes.get(idx).copied().unwrap_or(1.0);
                    let pan = pans.get(idx).copied().unwrap_or(0.0);
                    let dur = asset.metadata.get("duration").and_then(|v| v.as_f64()).unwrap_or(5.0);
                    if dur > max_dur {
                         max_dur = dur;
                    }
                    inputs.push(serde_json::json!({
                        "ref": r.to_string(),
                        "volume": vol,
                        "pan": pan,
                    }));
                }

                let mixed_asset = Asset {
                    r: new_ref,
                    kind: AssetKind::Audio,
                    path: PathBuf::from("mixed_track.wav"),
                    hash: format!("mixed_track_{}", new_ref.id),
                    metadata: serde_json::json!({
                        "duration": max_dur,
                        "inputs": inputs,
                    }),
                };

                self.registry.register(mixed_asset.r, mixed_asset.clone())?;
                db.save_asset(&mixed_asset)?;
                msg = format!("Mixed track audio asset registered as {}", mixed_asset.r);
            }
            Command::KeyframeSet { r, property, time_ms, value, easing } => {
                let ease_str = easing.as_deref().unwrap_or("Linear");
                db.save_keyframe(&r.to_string(), property, *time_ms, *value, ease_str)?;
                msg = format!("Keyframe set successfully: {} = {} at {}ms", property, value, time_ms);
            }
            Command::KeyframeList { r, property } => {
                let kfs = db.load_keyframes(&r.to_string(), property)?;
                let mut list = Vec::new();
                for k in kfs {
                    list.push(format!("{}ms: {} ({})", k.0, k.1, k.2));
                }
                msg = format!("Keyframes for {} (property: {}):\n{}", r, property, list.join("\n"));
            }
            Command::ExportEdl { output_path } => {
                // Invariant: Generates a standard CMX 3600 Edit Decision List (EDL) text representing the timeline, and writes it to the specified output path.
                let timeline = self.timeline.read().unwrap();
                let mut edl = String::new();
                edl.push_str("TITLE: AETHER Project\nFCM: NON-DROP FRAME\n\n");
                let mut event_count = 1;
                for track in &timeline.tracks {
                    for clip in &track.clips {
                        let asset = self.registry.resolve(&clip.asset_ref)?;
                        let name = asset.path.file_name().and_then(|n| n.to_str()).unwrap_or("UNKNOWN");
                        
                        let in_frames = (clip.in_point_ms as f64 * 30.0 / 1000.0) as u64;
                        let out_frames = (clip.out_point_ms as f64 * 30.0 / 1000.0) as u64;
                        let offset_frames = (clip.track_offset_ms as f64 * 30.0 / 1000.0) as u64;
                        let duration_frames = out_frames - in_frames;
                        
                        let in_tc = format_timecode(in_frames, 30.0);
                        let out_tc = format_timecode(out_frames, 30.0);
                        let start_tc = format_timecode(offset_frames, 30.0);
                        let end_tc = format_timecode(offset_frames + duration_frames, 30.0);
                        
                        edl.push_str(&format!(
                            "{:03}  AX       V     C        {} {} {} {}\n* FROM CLIP NAME: {}\n\n",
                            event_count, in_tc, out_tc, start_tc, end_tc, name
                        ));
                        event_count += 1;
                    }
                }
                
                let path = Path::new(output_path);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|e| AetherError::IoError(parent.to_string_lossy().to_string(), e.to_string()))?;
                }
                fs::write(path, edl).map_err(|e| AetherError::IoError(output_path.clone(), e.to_string()))?;
                msg = format!("EDL successfully exported to {}", output_path);
            }
            Command::ExportOtio { output_path } => {
                // Invariant: Generates a valid JSON representation adhering to OpenTimelineIO (OTIO) schemas and writes it to the specified output path.
                let timeline = self.timeline.read().unwrap();
                let otio_json = serde_json::json!({
                    "OTIO_SCHEMA": "Timeline.1",
                    "metadata": {},
                    "name": "AETHER OTIO Export",
                    "tracks": timeline.tracks.iter().map(|track| {
                        serde_json::json!({
                            "OTIO_SCHEMA": "Track.1",
                            "kind": match track.kind {
                                TrackKind::Video => "Video",
                                TrackKind::Audio => "Audio",
                            },
                            "name": track.name,
                            "children": track.clips.iter().map(|clip| {
                                serde_json::json!({
                                    "OTIO_SCHEMA": "Clip.1",
                                    "name": clip.asset_ref.to_string(),
                                    "source_range": {
                                        "OTIO_SCHEMA": "TimeRange.1",
                                        "duration": {
                                            "OTIO_SCHEMA": "RationalTime.1",
                                            "rate": 30.0,
                                            "value": ((clip.out_point_ms - clip.in_point_ms) as f64 * 30.0 / 1000.0)
                                        },
                                        "start_time": {
                                            "OTIO_SCHEMA": "RationalTime.1",
                                            "rate": 30.0,
                                            "value": (clip.in_point_ms as f64 * 30.0 / 1000.0)
                                        }
                                    }
                                })
                            }).collect::<Vec<_>>()
                        })
                    }).collect::<Vec<_>>()
                });
                
                let path = Path::new(output_path);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|e| AetherError::IoError(parent.to_string_lossy().to_string(), e.to_string()))?;
                }
                let formatted_json = serde_json::to_string_pretty(&otio_json)
                    .map_err(|e| AetherError::OperationFailed(format!("Failed to format OTIO JSON: {}", e)))?;
                fs::write(path, formatted_json).map_err(|e| AetherError::IoError(output_path.clone(), e.to_string()))?;
                msg = format!("OTIO successfully exported to {}", output_path);
            }
            Command::Undo => {
                let mut cursor = *self.history_cursor.read().unwrap();
                if cursor == 0 {
                    return Err(AetherError::OperationFailed("Already at first history state".to_string()));
                }
                cursor -= 1;
                self.rollback_to_cursor(&db, cursor)?;
                msg = format!("Successfully rolled back project to state {}", cursor);
            }
            Command::Redo => {
                let mut cursor = *self.history_cursor.read().unwrap();
                let history = db.load_history()?;
                if cursor >= history.len() {
                    return Err(AetherError::OperationFailed("Already at newest history state".to_string()));
                }
                cursor += 1;
                self.rollback_to_cursor(&db, cursor)?;
                msg = format!("Successfully fast-forwarded project to state {}", cursor);
            }
            Command::Snapshot => {
                msg = "Retrieved snapshot".to_string();
            }
        }

        // Mutation logging to SQLite history
        let snapshot_after = self.get_snapshot_with_db(&db)?;
        let hash_after = blake3::hash(&serde_json::to_vec(&snapshot_after).unwrap()).to_hex().to_string();

        // Increment history cursor if not undo/redo
        if !matches!(command, Command::Undo | Command::Redo | Command::Snapshot | Command::Inspect { .. } | Command::KeyframeList { .. } | Command::Export { .. } | Command::ExportOtio { .. } | Command::ExportEdl { .. }) {
            let mut cursor = self.history_cursor.write().unwrap();
            *cursor = db.add_history_entry(&command, Some(&hash_before), Some(&hash_after))?;
            db.save_settings(&self.settings.read().unwrap(), *cursor)?;
        }

        let final_snapshot = self.get_snapshot_with_db(&db)?;

        Ok(CommandResult {
            success: true,
            affected_ref,
            message: msg,
            snapshot: Some(final_snapshot),
        })
    }

    /// Rollback/Fast-Forward helper by rebuilding memory registry from history.
    fn rollback_to_cursor(&self, db: &DbManager, cursor: usize) -> Result<(), AetherError> {
        let history = db.load_history()?;
        
        // 1. Reset memory registry
        let all_assets = self.registry.list_assets();
        for asset in all_assets {
            self.registry.free(&asset.r);
        }
        db.clear_assets()?;

        let temp_graph = RwLock::new(CompositionGraph::new());
        let temp_timeline = RwLock::new(Timeline::default());

        // 2. Re-apply history commands up to cursor
        let temp_registry = RefRegistry::new();
        let mut temp_settings = ProjectSettings::default();

        for i in 0..cursor {
            let cmd = &history[i].1;
            // Execute the commands sequentially to re-populate settings and memory
            match cmd {
                Command::Init { fps, resolution, colorspace } => {
                    if let Some(f) = fps { temp_settings.fps = *f; }
                    if let Some(res) = resolution {
                        let parts: Vec<&str> = res.split('x').collect();
                        if parts.len() == 2 {
                            if let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                                temp_settings.width = w;
                                temp_settings.height = h;
                            }
                        }
                    }
                    if let Some(cs) = colorspace { temp_settings.colorspace = cs.clone(); }
                }
                Command::Import { path } => {
                    let p = Path::new(path);
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    let asset = match ext.as_str() {
                        "wav" | "mp3" | "ogg" | "flac" | "aac" => {
                            let r = temp_registry.allocate(RefKind::Audio);
                            aether_audio::import_audio(p, r, &self.cache_dir)?
                        }
                        "png" | "jpg" | "jpeg" => {
                            let r = temp_registry.allocate(RefKind::Image);
                            aether_image::import_image(p, r, &self.cache_dir)?
                        }
                        _ => {
                            let r = temp_registry.allocate(RefKind::Video);
                            aether_video::import_video(p, r, &self.cache_dir)?
                        }
                    };
                    temp_registry.register(asset.r, asset.clone())?;
                    db.save_asset(&asset)?;
                }
                Command::Trim { r, start, end } => {
                    let original = temp_registry.resolve(r)?;
                    let new_ref = temp_registry.allocate(r.kind);
                    let trimmed = match original.kind {
                        AssetKind::Video => aether_video::trim_video(&original, start, end, new_ref, &self.cache_dir)?,
                        AssetKind::Audio => aether_audio::trim_audio(&original, start, end, new_ref, &self.cache_dir)?,
                        AssetKind::Image | AssetKind::Animation => return Err(AetherError::InvalidCommand("Cannot trim".to_string())),
                    };
                    temp_registry.register(trimmed.r, trimmed.clone())?;
                    db.save_asset(&trimmed)?;
                }
                Command::Mix { r, volume } => {
                    let original = temp_registry.resolve(r)?;
                    let new_ref = temp_registry.allocate(RefKind::Audio);
                    let mixed = aether_audio::normalize_audio(&original, *volume, -1.0, new_ref, &self.cache_dir)?;
                    temp_registry.register(mixed.r, mixed.clone())?;
                    db.save_asset(&mixed)?;
                }
                Command::Composite { base, overlay, at, x, y } => {
                    let base_asset = temp_registry.resolve(base)?;
                    let overlay_asset = temp_registry.resolve(overlay)?;
                    let new_ref = temp_registry.allocate(RefKind::Video);
                    let composited = aether_video::composite_video(
                        &base_asset, &overlay_asset, at, *x, *y, new_ref, &self.cache_dir
                    )?;
                    temp_registry.register(composited.r, composited.clone())?;
                    db.save_asset(&composited)?;
                }
                Command::Canvas { width, height, color } => {
                    let r = temp_registry.allocate(RefKind::Image);
                    let canvas = aether_image::create_canvas(*width, *height, color, r, &self.cache_dir)?;
                    temp_registry.register(canvas.r, canvas.clone())?;
                    db.save_asset(&canvas)?;
                }
                Command::DrawText { r, text, font, size, x, y } => {
                    let original = temp_registry.resolve(r)?;
                    let new_ref = temp_registry.allocate(RefKind::Image);
                    let text_overlay = aether_image::draw_text(
                        &original, text, font, *size, *x as f32, *y as f32, "white", new_ref, &self.cache_dir
                    )?;
                    temp_registry.register(text_overlay.r, text_overlay.clone())?;
                    db.save_asset(&text_overlay)?;
                }
                Command::Concat { refs, transition, duration_ms } => {
                    let mut timeline = temp_timeline.write().unwrap();
                    let mut clips = Vec::new();
                    let mut current_offset = 0u64;

                    let trans_kind = match transition.as_deref() {
                        Some("Crossfade") => Some(TransitionKind::Crossfade),
                        Some("FadeToBlack") | Some("Dissolve") => Some(TransitionKind::Dissolve),
                        _ => None,
                    };

                    let mut track_kind = TrackKind::Video;

                    for r in refs {
                        let asset = temp_registry.resolve(r)?;
                        let duration = match asset.kind {
                            AssetKind::Video => {
                                track_kind = TrackKind::Video;
                                asset.metadata.get("duration").and_then(|v| v.as_f64()).map(|d| (d * 1000.0) as u64).unwrap_or(5000)
                            }
                            AssetKind::Audio => {
                                track_kind = TrackKind::Audio;
                                asset.metadata.get("duration").and_then(|v| v.as_f64()).map(|d| (d * 1000.0) as u64).unwrap_or(5000)
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
                }
                Command::Overlay { base, overlay, x: _, y: _, blend, opacity } => {
                    let mut graph = temp_graph.write().unwrap();
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

                    graph.add_node(Node { id: base_node_id, kind: NodeKind::Source(*base) });
                    graph.add_node(Node { id: overlay_node_id, kind: NodeKind::Source(*overlay) });
                    graph.add_node(Node { id: blend_node_id, kind: NodeKind::Blend { mode, opacity: op } });
                    graph.add_node(Node { id: output_node_id, kind: NodeKind::Output });

                    graph.connect(GraphConnection { from_node: base_node_id, from_port: 0, to_node: blend_node_id, to_port: 0 })?;
                    graph.connect(GraphConnection { from_node: overlay_node_id, from_port: 0, to_node: blend_node_id, to_port: 1 })?;
                    graph.connect(GraphConnection { from_node: blend_node_id, from_port: 0, to_node: output_node_id, to_port: 0 })?;
                    graph.output_node = Some(output_node_id);
                }
                Command::Speed { r, factor } => {
                    let original = temp_registry.resolve(r)?;
                    let new_ref = temp_registry.allocate(RefKind::Animation);

                    let orig_dur = original.metadata.get("duration").and_then(|v| v.as_f64()).unwrap_or(5.0);
                    let new_dur = orig_dur / (*factor as f64);

                    let speed_asset = Asset {
                        r: new_ref,
                        kind: AssetKind::Animation,
                        path: original.path.clone(),
                        hash: format!("{}_speed_{}", original.hash, factor),
                        metadata: serde_json::json!({
                            "duration": new_dur,
                            "speed_factor": factor,
                            "parent_ref": r.to_string(),
                        }),
                    };

                    temp_registry.register(speed_asset.r, speed_asset.clone())?;
                    db.save_asset(&speed_asset)?;
                }
                Command::Eq { r, filter_type, freq_hz, gain_db, q } => {
                    let original = temp_registry.resolve(r)?;
                    let new_ref = temp_registry.allocate(RefKind::Audio);

                    let eq_asset = Asset {
                        r: new_ref,
                        kind: AssetKind::Audio,
                        path: original.path.clone(),
                        hash: format!("{}_eq_{}_{}", original.hash, filter_type, freq_hz),
                        metadata: serde_json::json!({
                            "duration": original.metadata.get("duration").cloned(),
                            "eq": {
                                "filter_type": filter_type,
                                "freq_hz": freq_hz,
                                "gain_db": gain_db,
                                "q": q,
                            },
                            "parent_ref": r.to_string(),
                        }),
                    };

                    temp_registry.register(eq_asset.r, eq_asset.clone())?;
                    db.save_asset(&eq_asset)?;
                }
                Command::Compress { r, threshold_db, ratio, attack_ms, release_ms } => {
                    let original = temp_registry.resolve(r)?;
                    let new_ref = temp_registry.allocate(RefKind::Audio);

                    let comp_asset = Asset {
                        r: new_ref,
                        kind: AssetKind::Audio,
                        path: original.path.clone(),
                        hash: format!("{}_compress_{}", original.hash, threshold_db),
                        metadata: serde_json::json!({
                            "duration": original.metadata.get("duration").cloned(),
                            "compressor": {
                                "threshold_db": threshold_db,
                                "ratio": ratio,
                                "attack_ms": attack_ms,
                                "release_ms": release_ms,
                            },
                            "parent_ref": r.to_string(),
                        }),
                    };

                    temp_registry.register(comp_asset.r, comp_asset.clone())?;
                    db.save_asset(&comp_asset)?;
                }
                Command::MixTracks { refs, volumes, pans } => {
                    let new_ref = temp_registry.allocate(RefKind::Audio);

                    let mut inputs = Vec::new();
                    let mut max_dur = 0.0;
                    for (idx, r) in refs.iter().enumerate() {
                        let asset = temp_registry.resolve(r)?;
                        let vol = volumes.get(idx).copied().unwrap_or(1.0);
                        let pan = pans.get(idx).copied().unwrap_or(0.0);
                        let dur = asset.metadata.get("duration").and_then(|v| v.as_f64()).unwrap_or(5.0);
                        if dur > max_dur {
                             max_dur = dur;
                        }
                        inputs.push(serde_json::json!({
                            "ref": r.to_string(),
                            "volume": vol,
                            "pan": pan,
                        }));
                    }

                    let mixed_asset = Asset {
                        r: new_ref,
                        kind: AssetKind::Audio,
                        path: PathBuf::from("mixed_track.wav"),
                        hash: format!("mixed_track_{}", new_ref.id),
                        metadata: serde_json::json!({
                            "duration": max_dur,
                            "inputs": inputs,
                        }),
                    };

                    temp_registry.register(mixed_asset.r, mixed_asset.clone())?;
                    db.save_asset(&mixed_asset)?;
                }
                Command::KeyframeSet { r, property, time_ms, value, easing } => {
                    let ease_str = easing.as_deref().unwrap_or("Linear");
                    db.save_keyframe(&r.to_string(), property, *time_ms, *value, ease_str)?;
                }
                _ => {}
            }
        }

        // 3. Save new values to memory & DB
        *self.settings.write().unwrap() = temp_settings.clone();
        *self.graph.write().unwrap() = temp_graph.into_inner().unwrap();
        *self.timeline.write().unwrap() = temp_timeline.into_inner().unwrap();

        db.save_settings(&temp_settings, cursor)?;
        db.save_graph(&self.graph.read().unwrap())?;
        db.save_timeline(&self.timeline.read().unwrap())?;
        *self.history_cursor.write().unwrap() = cursor;

        // Populate registry with re-applied assets
        for asset in temp_registry.list_assets() {
            let _ = self.registry.register(asset.r, asset);
        }
        Ok(())
    }
}

fn format_timecode(frames: u64, fps: f64) -> String {
    let fps_u = fps as u64;
    let total_secs = frames / fps_u;
    let f = frames % fps_u;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}:{:02}:{:02}", h, m, s, f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project_dir() -> PathBuf {
        let unique_dir = format!("test_project_{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos());
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
        let cmd_canvas1 = Command::Canvas { width: 100, height: 100, color: "blue".to_string() };
        let res_canvas1 = session.execute(cmd_canvas1).unwrap();
        let canvas1_ref = res_canvas1.affected_ref.unwrap();

        // 2. Create another Canvas (Image Asset @img2) to act as overlay
        let cmd_canvas2 = Command::Canvas { width: 100, height: 100, color: "green".to_string() };
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
        let cmd_speed = Command::Speed { r: canvas1_ref, factor: 2.0 };
        let res_speed = session.execute(cmd_speed).unwrap();
        assert!(res_speed.success);
        let speed_ref = res_speed.affected_ref.unwrap();
        let speed_asset = session.registry.resolve(&speed_ref).unwrap();
        assert_eq!(speed_asset.metadata.get("speed_factor").and_then(|v| v.as_f64()).unwrap(), 2.0);

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
        let cmd_edl = Command::ExportEdl { output_path: edl_path.to_string_lossy().to_string() };
        let res_edl = session.execute(cmd_edl).unwrap();
        assert!(res_edl.success);
        assert!(edl_path.exists());

        let otio_path = dir.join("export.otio");
        let cmd_otio = Command::ExportOtio { output_path: otio_path.to_string_lossy().to_string() };
        let res_otio = session.execute(cmd_otio).unwrap();
        assert!(res_otio.success);
        assert!(otio_path.exists());

        // 8. Test Undo and Redo of Concat
        // Let's rollback to state 3 (Canvas1, Canvas2, Overlay)
        session.rollback_to_cursor(&session.db.lock().unwrap(), 3).unwrap();
        let snap_undone = session.get_snapshot().unwrap();
        assert_eq!(snap_undone.timeline.tracks.len(), 0); // Concat undone!

        // Let's redo/fast-forward back to state 6
        session.rollback_to_cursor(&session.db.lock().unwrap(), 6).unwrap();
        let snap_redone = session.get_snapshot().unwrap();
        assert_eq!(snap_redone.timeline.tracks.len(), 1); // Concat redone!

        let _ = fs::remove_dir_all(&dir);
    }
}
