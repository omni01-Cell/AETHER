pub mod otio;
pub mod edl;

use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use rusqlite::{params, Connection, OptionalExtension};
use aether_core::{
    AetherError, Ref, Asset, AssetKind, ProjectSettings, Command,
    CompositionGraph, Node, Connection as GraphConnection, NodeId, NodeKind,
    Timeline, Track, Clip, TransitionKind, TrackKind,
    GenerationJob, GenerationStatus
};

/// Database manager for the AETHER persistence layer.
pub struct DbManager {
    conn: Connection,
    db_path: PathBuf,
}

impl DbManager {
    /// Initializes and opens the database at the specified directory path.
    /// Creates the directory (e.g. `.aether/`) if it does not exist.
    pub fn new<P: AsRef<Path>>(dir_path: P) -> Result<Self, AetherError> {
        let dir = dir_path.as_ref();
        if !dir.exists() {
            fs::create_dir_all(dir)
                .map_err(|e| AetherError::IoError(dir.to_string_lossy().to_string(), e.to_string()))?;
        }

        let db_path = dir.join("metadata.db");
        let conn = Connection::open(&db_path)
            .map_err(|e| AetherError::DatabaseError(format!("Failed to open DB: {}", e)))?;

        let manager = DbManager { conn, db_path };
        manager.initialize_schema()?;
        Ok(manager)
    }

    /// Initializes all required tables and default settings if not already present.
    fn initialize_schema(&self) -> Result<(), AetherError> {
        // Invariant: Initializes all required database tables and default project settings, ensuring the schema matches the specifications exactly.

        // 1. Settings Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS project_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                fps REAL NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                colorspace TEXT NOT NULL,
                current_branch TEXT NOT NULL DEFAULT 'main',
                current_commit TEXT NOT NULL DEFAULT ''
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create project_settings failed: {}", e)))?;

        // 1b. Branches Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS branches (
                name TEXT PRIMARY KEY,
                head_commit TEXT NOT NULL
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create branches failed: {}", e)))?;

        // Insert default branch if empty
        let count_branches: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM branches",
            [],
            |row| row.get(0),
        ).map_err(|e| AetherError::DatabaseError(format!("Check branches count failed: {}", e)))?;

        if count_branches == 0 {
            self.conn.execute(
                "INSERT INTO branches (name, head_commit) VALUES ('main', '')",
                [],
            ).map_err(|e| AetherError::DatabaseError(format!("Insert default branch failed: {}", e)))?;
        }

        // Insert default settings if empty
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM project_settings",
            [],
            |row| row.get(0),
        ).map_err(|e| AetherError::DatabaseError(format!("Check settings count failed: {}", e)))?;

        if count == 0 {
            let defaults = ProjectSettings::default();
            self.conn.execute(
                "INSERT INTO project_settings (id, fps, width, height, colorspace, current_branch, current_commit)
                 VALUES (1, ?1, ?2, ?3, ?4, 'main', '')",
                params![defaults.fps, defaults.width, defaults.height, defaults.colorspace],
            ).map_err(|e| AetherError::DatabaseError(format!("Insert default settings failed: {}", e)))?;
        }

        // 2. Assets Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS assets (
                ref_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                path TEXT NOT NULL,
                hash TEXT NOT NULL,
                metadata TEXT NOT NULL
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create assets failed: {}", e)))?;

        // 3. History Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS history (
                commit_hash TEXT PRIMARY KEY,
                parent_hash TEXT,
                branch TEXT NOT NULL,
                command TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create history failed: {}", e)))?;

        // 4. Nodes Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS nodes (
                id INTEGER PRIMARY KEY,
                kind TEXT NOT NULL,
                params TEXT NOT NULL
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create nodes failed: {}", e)))?;

        // 5. Connections Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS connections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_node INTEGER NOT NULL,
                from_port INTEGER NOT NULL,
                to_node INTEGER NOT NULL,
                to_port INTEGER NOT NULL
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create connections failed: {}", e)))?;

        // 6. Graph State Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS graph_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                output_node INTEGER
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create graph_state failed: {}", e)))?;

        // 7. Tracks Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                position INTEGER NOT NULL
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create tracks failed: {}", e)))?;

        // 8. Clips Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS clips (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id INTEGER NOT NULL,
                asset_ref TEXT NOT NULL,
                in_point_ms INTEGER NOT NULL,
                out_point_ms INTEGER NOT NULL,
                offset_ms INTEGER NOT NULL,
                transition TEXT,
                FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create clips failed: {}", e)))?;

        // 9. Keyframes Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS keyframes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                asset_ref TEXT NOT NULL,
                property TEXT NOT NULL,
                time_ms INTEGER NOT NULL,
                value REAL NOT NULL,
                easing TEXT NOT NULL
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create keyframes failed: {}", e)))?;

        // 10. State Checkpoints Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS state_checkpoints (
                commit_hash TEXT PRIMARY KEY,
                timeline TEXT NOT NULL,
                graph TEXT NOT NULL,
                assets TEXT NOT NULL,
                settings TEXT NOT NULL
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create state_checkpoints failed: {}", e)))?;

        // 11. Generation Jobs Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS generation_jobs (
                ref_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                status TEXT NOT NULL,
                requested_model TEXT,
                resolved_model TEXT,
                provider_job_id TEXT,
                prompt TEXT,
                inputs TEXT NOT NULL,
                artifacts TEXT NOT NULL,
                error TEXT,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                options TEXT NOT NULL
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create generation_jobs failed: {}", e)))?;

        // 12. Generation Events Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS generation_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_ref TEXT NOT NULL,
                status TEXT NOT NULL,
                message TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create generation_events failed: {}", e)))?;

        Ok(())
    }

    /// Saves the current project settings, current branch, and current commit.
    pub fn save_settings(&self, settings: &ProjectSettings, current_branch: &str, current_commit: &str) -> Result<(), AetherError> {
        // Invariant: Updates the settings, current branch, and current commit in the database, guaranteeing they persist on disk.
        self.conn.execute(
            "UPDATE project_settings
             SET fps = ?1, width = ?2, height = ?3, colorspace = ?4, current_branch = ?5, current_commit = ?6
             WHERE id = 1",
            params![
                settings.fps,
                settings.width,
                settings.height,
                settings.colorspace,
                current_branch,
                current_commit
            ],
        ).map_err(|e| AetherError::DatabaseError(format!("Update settings failed: {}", e)))?;
        Ok(())
    }

    /// Loads the project settings, current branch, and current commit.
    pub fn load_settings(&self) -> Result<(ProjectSettings, String, String), AetherError> {
        // Invariant: Reconstructs and returns the settings, current branch, and current commit from the database table.
        self.conn.query_row(
            "SELECT fps, width, height, colorspace, current_branch, current_commit FROM project_settings WHERE id = 1",
            [],
            |row| {
                let fps: f64 = row.get(0)?;
                let width: u32 = row.get(1)?;
                let height: u32 = row.get(2)?;
                let colorspace: String = row.get(3)?;
                let current_branch: String = row.get(4)?;
                let current_commit: String = row.get(5)?;
                Ok((
                    ProjectSettings {
                        fps: fps as f32,
                        width,
                        height,
                        colorspace,
                    },
                    current_branch,
                    current_commit,
                ))
            },
        ).map_err(|e| AetherError::DatabaseError(format!("Load settings failed: {}", e)))
    }

    /// Registers a new asset in the database.
    pub fn save_asset(&self, asset: &Asset) -> Result<(), AetherError> {
        let ref_str = asset.r.to_string();
        let kind_str = match asset.kind {
            AssetKind::Video => "Video",
            AssetKind::Audio => "Audio",
            AssetKind::Image => "Image",
            AssetKind::Animation => "Animation",
        };
        let path_str = asset.path.to_string_lossy().to_string();
        let metadata_str = serde_json::to_string(&asset.metadata)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize asset metadata failed: {}", e)))?;

        self.conn.execute(
            "INSERT OR REPLACE INTO assets (ref_id, kind, path, hash, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![ref_str, kind_str, path_str, asset.hash, metadata_str],
        ).map_err(|e| AetherError::DatabaseError(format!("Insert asset failed: {}", e)))?;
        Ok(())
    }

    /// Deletes an asset from the database.
    pub fn delete_asset(&self, r: &Ref) -> Result<(), AetherError> {
        self.conn.execute(
            "DELETE FROM assets WHERE ref_id = ?1",
            params![r.to_string()],
        ).map_err(|e| AetherError::DatabaseError(format!("Delete asset failed: {}", e)))?;
        Ok(())
    }

    /// Loads all registered assets.
    pub fn load_assets(&self) -> Result<Vec<Asset>, AetherError> {
        let mut stmt = self.conn.prepare("SELECT ref_id, kind, path, hash, metadata FROM assets")
            .map_err(|e| AetherError::DatabaseError(format!("Prepare assets query failed: {}", e)))?;

        let asset_iter = stmt.query_map([], |row| {
            let ref_str: String = row.get(0)?;
            let kind_str: String = row.get(1)?;
            let path_str: String = row.get(2)?;
            let hash: String = row.get(3)?;
            let metadata_str: String = row.get(4)?;

            Ok((ref_str, kind_str, path_str, hash, metadata_str))
        }).map_err(|e| AetherError::DatabaseError(format!("Query assets failed: {}", e)))?;

        let mut assets = Vec::new();
        for item in asset_iter {
            let (ref_str, kind_str, path_str, hash, metadata_str) = item
                .map_err(|e| AetherError::DatabaseError(format!("Read asset row failed: {}", e)))?;

            let r = ref_str.parse::<Ref>()?;
            let kind = match kind_str.as_str() {
                "Video" => AssetKind::Video,
                "Audio" => AssetKind::Audio,
                "Image" => AssetKind::Image,
                "Animation" => AssetKind::Animation,
                other => {
                    return Err(AetherError::DatabaseError(format!(
                        "Unknown asset kind '{}' in database",
                        other
                    )));
                }
            };
            let metadata = serde_json::from_str(&metadata_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize metadata failed: {}", e)))?;

            assets.push(Asset {
                r,
                kind,
                path: PathBuf::from(path_str),
                hash,
                metadata,
            });
        }

        Ok(assets)
    }

    /// Clears all assets from the database.
    pub fn clear_assets(&self) -> Result<(), AetherError> {
        self.conn.execute("DELETE FROM assets", [])
            .map_err(|e| AetherError::DatabaseError(format!("Clear assets failed: {}", e)))?;
        Ok(())
    }

    /// Appends a new command to the project execution history.
    pub fn add_history_entry(
        &self,
        commit_hash: &str,
        parent_hash: Option<&str>,
        branch: &str,
        command: &Command,
    ) -> Result<(), AetherError> {
        // Invariant: Inserts a new commit entry into the history graph with its hash, parent, branch, and command.
        let cmd_str = serde_json::to_string(command)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize command failed: {}", e)))?;

        self.conn.execute(
            "INSERT OR REPLACE INTO history (commit_hash, parent_hash, branch, command)
             VALUES (?1, ?2, ?3, ?4)",
            params![commit_hash, parent_hash, branch, cmd_str],
        ).map_err(|e| AetherError::DatabaseError(format!("Insert history failed: {}", e)))?;

        Ok(())
    }

    /// Loads the complete execution history list.
    pub fn load_history(&self) -> Result<Vec<(String, Option<String>, String, Command)>, AetherError> {
        // Invariant: Retrieves and returns all commit entries in the history graph sorted by their timestamp.
        let mut stmt = self.conn.prepare(
            "SELECT commit_hash, parent_hash, branch, command FROM history ORDER BY timestamp ASC",
        ).map_err(|e| AetherError::DatabaseError(format!("Prepare history query failed: {}", e)))?;

        let history_iter = stmt.query_map([], |row| {
            let commit_hash: String = row.get(0)?;
            let parent_hash: Option<String> = row.get(1)?;
            let branch: String = row.get(2)?;
            let cmd_str: String = row.get(3)?;
            Ok((commit_hash, parent_hash, branch, cmd_str))
        }).map_err(|e| AetherError::DatabaseError(format!("Query history failed: {}", e)))?;

        let mut history = Vec::new();
        for item in history_iter {
            let (commit_hash, parent_hash, branch, cmd_str) = item
                .map_err(|e| AetherError::DatabaseError(format!("Read history row failed: {}", e)))?;

            let command: Command = serde_json::from_str(&cmd_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize command failed: {}", e)))?;

            history.push((commit_hash, parent_hash, branch, command));
        }

        Ok(history)
    }

    /// Clears the history log entirely.
    pub fn clear_history(&self) -> Result<(), AetherError> {
        self.conn.execute("DELETE FROM history", [])
            .map_err(|e| AetherError::DatabaseError(format!("Clear history failed: {}", e)))?;
        Ok(())
    }

    /// Saves a branch head commit.
    pub fn save_branch(&self, name: &str, head_commit: &str) -> Result<(), AetherError> {
        // Invariant: Registers or updates a branch with its head commit in the branches table.
        self.conn.execute(
            "INSERT OR REPLACE INTO branches (name, head_commit) VALUES (?1, ?2)",
            params![name, head_commit],
        ).map_err(|e| AetherError::DatabaseError(format!("Save branch failed: {}", e)))?;
        Ok(())
    }

    /// Loads a branch head commit.
    pub fn load_branch_head(&self, name: &str) -> Result<Option<String>, AetherError> {
        // Invariant: Retrieves the head commit of the given branch name if it exists.
        let head: Option<String> = self.conn.query_row(
            "SELECT head_commit FROM branches WHERE name = ?1",
            params![name],
            |row| row.get(0),
        ).optional().map_err(|e| AetherError::DatabaseError(format!("Load branch head failed: {}", e)))?;
        Ok(head)
    }

    /// Loads all branches.
    pub fn load_all_branches(&self) -> Result<Vec<(String, String)>, AetherError> {
        // Invariant: Returns all existing branch records with their names and head commits.
        let mut stmt = self.conn.prepare("SELECT name, head_commit FROM branches")
            .map_err(|e| AetherError::DatabaseError(format!("Prepare branches query failed: {}", e)))?;

        let branch_iter = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let head_commit: String = row.get(1)?;
            Ok((name, head_commit))
        }).map_err(|e| AetherError::DatabaseError(format!("Query branches failed: {}", e)))?;

        let mut branches = Vec::new();
        for item in branch_iter {
            branches.push(item.map_err(|e| AetherError::DatabaseError(format!("Read branch failed: {}", e)))?);
        }
        Ok(branches)
    }

    /// Saves a full state checkpoint for a commit.
    pub fn save_checkpoint(
        &self,
        commit_hash: &str,
        timeline: &Timeline,
        graph: &CompositionGraph,
        assets: &[Asset],
        settings: &ProjectSettings,
    ) -> Result<(), AetherError> {
        // Invariant: Persists a serialized full state checkpoint of the timeline, graph, active assets, and settings under the specified commit hash.
        let timeline_str = serde_json::to_string(timeline)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize timeline failed: {}", e)))?;
        let graph_str = serde_json::to_string(graph)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize graph failed: {}", e)))?;
        let assets_str = serde_json::to_string(assets)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize assets failed: {}", e)))?;
        let settings_str = serde_json::to_string(settings)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize settings failed: {}", e)))?;

        self.conn.execute(
            "INSERT OR REPLACE INTO state_checkpoints (commit_hash, timeline, graph, assets, settings)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![commit_hash, timeline_str, graph_str, assets_str, settings_str],
        ).map_err(|e| AetherError::DatabaseError(format!("Save checkpoint failed: {}", e)))?;
        Ok(())
    }

    /// Loads a full state checkpoint for a commit.
    pub fn load_checkpoint(
        &self,
        commit_hash: &str,
    ) -> Result<(Timeline, CompositionGraph, Vec<Asset>, ProjectSettings), AetherError> {
        // Invariant: Retrieves and deserializes the full state checkpoint (timeline, graph, assets, settings) corresponding to the given commit hash.
        self.conn.query_row(
            "SELECT timeline, graph, assets, settings FROM state_checkpoints WHERE commit_hash = ?1",
            params![commit_hash],
            |row| {
                let timeline_str: String = row.get(0)?;
                let graph_str: String = row.get(1)?;
                let assets_str: String = row.get(2)?;
                let settings_str: String = row.get(3)?;
                Ok((timeline_str, graph_str, assets_str, settings_str))
            },
        ).map_err(|e| AetherError::DatabaseError(format!("Query checkpoint failed: {}", e)))
        .and_then(|(timeline_str, graph_str, assets_str, settings_str)| {
            let timeline = serde_json::from_str(&timeline_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize timeline checkpoint failed: {}", e)))?;
            let graph = serde_json::from_str(&graph_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize graph checkpoint failed: {}", e)))?;
            let assets = serde_json::from_str(&assets_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize assets checkpoint failed: {}", e)))?;
            let settings = serde_json::from_str(&settings_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize settings checkpoint failed: {}", e)))?;
            Ok((timeline, graph, assets, settings))
        })
    }

    /// Saves the composition graph.
    pub fn save_graph(&self, graph: &CompositionGraph) -> Result<(), AetherError> {
        // Invariant: The database graph tables (nodes, connections, graph_state) are updated to exactly match the provided CompositionGraph state.
        self.conn.execute("DELETE FROM nodes", [])
            .map_err(|e| AetherError::DatabaseError(format!("Failed to clear nodes: {}", e)))?;
        self.conn.execute("DELETE FROM connections", [])
            .map_err(|e| AetherError::DatabaseError(format!("Failed to clear connections: {}", e)))?;
        self.conn.execute("DELETE FROM graph_state", [])
            .map_err(|e| AetherError::DatabaseError(format!("Failed to clear graph_state: {}", e)))?;

        for node in graph.nodes.values() {
            let kind_str = match &node.kind {
                NodeKind::Source(_) => "Source",
                NodeKind::Blend { .. } => "Blend",
                NodeKind::Transition { .. } => "Transition",
                NodeKind::Filter { .. } => "Filter",
                NodeKind::Output => "Output",
            };
            let params_str = serde_json::to_string(&node.kind)
                .map_err(|e| AetherError::DatabaseError(format!("Failed to serialize NodeKind: {}", e)))?;

            self.conn.execute(
                "INSERT INTO nodes (id, kind, params) VALUES (?1, ?2, ?3)",
                params![node.id as i64, kind_str, params_str],
            ).map_err(|e| AetherError::DatabaseError(format!("Failed to insert node: {}", e)))?;
        }

        for conn_val in &graph.connections {
            self.conn.execute(
                "INSERT INTO connections (from_node, from_port, to_node, to_port) VALUES (?1, ?2, ?3, ?4)",
                params![conn_val.from_node as i64, conn_val.from_port as i32, conn_val.to_node as i64, conn_val.to_port as i32],
            ).map_err(|e| AetherError::DatabaseError(format!("Failed to insert connection: {}", e)))?;
        }

        if let Some(out_id) = graph.output_node {
            self.conn.execute(
                "INSERT INTO graph_state (id, output_node) VALUES (1, ?1)",
                params![out_id as i64],
            ).map_err(|e| AetherError::DatabaseError(format!("Failed to insert graph_state: {}", e)))?;
        }

        Ok(())
    }

    /// Loads the composition graph.
    pub fn load_graph(&self) -> Result<CompositionGraph, AetherError> {
        // Invariant: The function returns the exact CompositionGraph reconstructed from the database tables (nodes, connections, graph_state).
        let mut stmt = self.conn.prepare("SELECT id, params FROM nodes")
            .map_err(|e| AetherError::DatabaseError(format!("Prepare nodes query failed: {}", e)))?;
        let node_iter = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let params_str: String = row.get(1)?;
            Ok((id, params_str))
        }).map_err(|e| AetherError::DatabaseError(format!("Query nodes failed: {}", e)))?;

        let mut nodes = HashMap::new();
        for item in node_iter {
            let (id, params_str) = item
                .map_err(|e| AetherError::DatabaseError(format!("Read node row failed: {}", e)))?;
            let kind: NodeKind = serde_json::from_str(&params_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize NodeKind failed: {}", e)))?;
            nodes.insert(id as NodeId, Node { id: id as NodeId, kind });
        }

        let mut stmt = self.conn.prepare("SELECT from_node, from_port, to_node, to_port FROM connections")
            .map_err(|e| AetherError::DatabaseError(format!("Prepare connections query failed: {}", e)))?;
        let conn_iter = stmt.query_map([], |row| {
            let from_node: i64 = row.get(0)?;
            let from_port: i32 = row.get(1)?;
            let to_node: i64 = row.get(2)?;
            let to_port: i32 = row.get(3)?;
            Ok(GraphConnection {
                from_node: from_node as NodeId,
                from_port: from_port as u8,
                to_node: to_node as NodeId,
                to_port: to_port as u8,
            })
        }).map_err(|e| AetherError::DatabaseError(format!("Query connections failed: {}", e)))?;

        let mut connections = Vec::new();
        for item in conn_iter {
            let conn_val = item
                .map_err(|e| AetherError::DatabaseError(format!("Read connection row failed: {}", e)))?;
            connections.push(conn_val);
        }

        let output_node: Option<i64> = self.conn.query_row(
            "SELECT output_node FROM graph_state WHERE id = 1",
            [],
            |row| row.get(0),
        ).optional().map_err(|e| AetherError::DatabaseError(format!("Get output_node failed: {}", e)))?;

        Ok(CompositionGraph {
            nodes,
            connections,
            output_node: output_node.map(|x| x as NodeId),
        })
    }

    /// Saves the timeline state.
    pub fn save_timeline(&self, timeline: &Timeline) -> Result<(), AetherError> {
        // Invariant: The database timeline tables (tracks, clips) are updated to exactly match the provided Timeline state.
        self.conn.execute("DELETE FROM tracks", [])
            .map_err(|e| AetherError::DatabaseError(format!("Failed to clear tracks: {}", e)))?;
        self.conn.execute("DELETE FROM clips", [])
            .map_err(|e| AetherError::DatabaseError(format!("Failed to clear clips: {}", e)))?;

        for (pos, track) in timeline.tracks.iter().enumerate() {
            let kind_str = match track.kind {
                TrackKind::Video => "Video",
                TrackKind::Audio => "Audio",
            };
            self.conn.execute(
                "INSERT INTO tracks (name, kind, position) VALUES (?1, ?2, ?3)",
                params![track.name, kind_str, pos as i32],
            ).map_err(|e| AetherError::DatabaseError(format!("Failed to insert track: {}", e)))?;

            let track_db_id = self.conn.last_insert_rowid();

            for clip in &track.clips {
                let trans_str = clip.transition.map(|t| serde_json::to_string(&t).unwrap());
                self.conn.execute(
                    "INSERT INTO clips (track_id, asset_ref, in_point_ms, out_point_ms, offset_ms, transition)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        track_db_id,
                        clip.asset_ref.to_string(),
                        clip.in_point_ms as i64,
                        clip.out_point_ms as i64,
                        clip.track_offset_ms as i64,
                        trans_str,
                    ],
                ).map_err(|e| AetherError::DatabaseError(format!("Failed to insert clip: {}", e)))?;
            }
        }

        Ok(())
    }

    /// Loads the timeline state.
    pub fn load_timeline(&self) -> Result<Timeline, AetherError> {
        // Invariant: The function returns the exact Timeline reconstructed from the database tables (tracks, clips).
        let mut stmt = self.conn.prepare("SELECT id, name, kind FROM tracks ORDER BY position ASC")
            .map_err(|e| AetherError::DatabaseError(format!("Prepare tracks query failed: {}", e)))?;

        let track_iter = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let kind_str: String = row.get(2)?;
            Ok((id, name, kind_str))
        }).map_err(|e| AetherError::DatabaseError(format!("Query tracks failed: {}", e)))?;

        let mut tracks = Vec::new();
        for item in track_iter {
            let (track_id, name, kind_str) = item
                .map_err(|e| AetherError::DatabaseError(format!("Read track row failed: {}", e)))?;

            let kind = match kind_str.as_str() {
                "Video" => TrackKind::Video,
                "Audio" => TrackKind::Audio,
                other => return Err(AetherError::DatabaseError(format!("Unknown track kind '{}'", other))),
            };

            // Query clips for this track
            let mut clip_stmt = self.conn.prepare(
                "SELECT asset_ref, in_point_ms, out_point_ms, offset_ms, transition FROM clips WHERE track_id = ?1"
            ).map_err(|e| AetherError::DatabaseError(format!("Prepare clips query failed: {}", e)))?;

            let clip_iter = clip_stmt.query_map(params![track_id], |row| {
                let asset_ref_str: String = row.get(0)?;
                let in_point_ms: i64 = row.get(1)?;
                let out_point_ms: i64 = row.get(2)?;
                let offset_ms: i64 = row.get(3)?;
                let trans_str: Option<String> = row.get(4)?;
                Ok((asset_ref_str, in_point_ms, out_point_ms, offset_ms, trans_str))
            }).map_err(|e| AetherError::DatabaseError(format!("Query clips failed: {}", e)))?;

            let mut clips = Vec::new();
            for c_item in clip_iter {
                let (asset_ref_str, in_point, out_point, offset, trans_str) = c_item
                    .map_err(|e| AetherError::DatabaseError(format!("Read clip row failed: {}", e)))?;

                let asset_ref = asset_ref_str.parse::<Ref>()?;
                let transition = match trans_str {
                    Some(s) => Some(serde_json::from_str::<TransitionKind>(&s)
                        .map_err(|e| AetherError::DatabaseError(format!("Deserialize transition failed: {}", e)))?),
                    None => None,
                };

                clips.push(Clip {
                    asset_ref,
                    in_point_ms: in_point as u64,
                    out_point_ms: out_point as u64,
                    track_offset_ms: offset as u64,
                    transition,
                });
            }

            tracks.push(Track { name, kind, clips });
        }

        Ok(Timeline { tracks })
    }

    pub fn save_keyframe(&self, asset_ref: &str, property: &str, time_ms: u64, value: f32, easing: &str) -> Result<(), AetherError> {
        self.conn.execute(
            "DELETE FROM keyframes WHERE asset_ref = ?1 AND property = ?2 AND time_ms = ?3",
            params![asset_ref, property, time_ms as i64],
        ).map_err(|e| AetherError::DatabaseError(format!("Delete keyframe failed: {}", e)))?;

        self.conn.execute(
            "INSERT INTO keyframes (asset_ref, property, time_ms, value, easing) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![asset_ref, property, time_ms as i64, value as f64, easing],
        ).map_err(|e| AetherError::DatabaseError(format!("Insert keyframe failed: {}", e)))?;
        Ok(())
    }

    pub fn load_keyframes(&self, asset_ref: &str, property: &str) -> Result<Vec<(u64, f32, String)>, AetherError> {
        let mut stmt = self.conn.prepare(
            "SELECT time_ms, value, easing FROM keyframes WHERE asset_ref = ?1 AND property = ?2 ORDER BY time_ms ASC"
        ).map_err(|e| AetherError::DatabaseError(format!("Prepare keyframes query failed: {}", e)))?;

        let kf_iter = stmt.query_map(params![asset_ref, property], |row| {
            let time_ms: i64 = row.get(0)?;
            let value: f64 = row.get(1)?;
            let easing: String = row.get(2)?;
            Ok((time_ms as u64, value as f32, easing))
        }).map_err(|e| AetherError::DatabaseError(format!("Query keyframes failed: {}", e)))?;

        let mut list = Vec::new();
        for item in kf_iter {
            list.push(item.map_err(|e| AetherError::DatabaseError(format!("Read keyframe failed: {}", e)))?);
        }
        Ok(list)
    }

    /// Invariant: must save or update the specified GenerationJob in the database, serializing all sub-structures to JSON and validating parameters.
    pub fn save_generation_job(&self, job: &GenerationJob) -> Result<(), AetherError> {
        let ref_id = job.job_ref.to_string();
        let kind_str = serde_json::to_string(&job.kind)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize GenerationKind failed: {}", e)))?;
        let status_str = serde_json::to_string(&job.status)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize GenerationStatus failed: {}", e)))?;
        let requested_model_str = job.requested_model.clone();
        let resolved_model_str = job.resolved_model.as_ref()
            .map(|m| serde_json::to_string(m).unwrap());
        let provider_job_id_str = job.provider_job_id.clone();
        let prompt_str = job.prompt.as_ref()
            .map(|p| serde_json::to_string(p).unwrap());
        let inputs_str = serde_json::to_string(&job.inputs)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize job inputs failed: {}", e)))?;
        let artifacts_str = serde_json::to_string(&job.artifacts)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize job artifacts failed: {}", e)))?;
        let error_str = job.error.clone();
        let created_at_ms = job.created_at_ms as i64;
        let updated_at_ms = job.updated_at_ms as i64;
        let options_str = serde_json::to_string(&job.options)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize job options failed: {}", e)))?;

        self.conn.execute(
            "INSERT OR REPLACE INTO generation_jobs (
                ref_id, kind, status, requested_model, resolved_model, provider_job_id,
                prompt, inputs, artifacts, error, created_at_ms, updated_at_ms, options
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                ref_id,
                kind_str,
                status_str,
                requested_model_str,
                resolved_model_str,
                provider_job_id_str,
                prompt_str,
                inputs_str,
                artifacts_str,
                error_str,
                created_at_ms,
                updated_at_ms,
                options_str,
            ],
        ).map_err(|e| AetherError::DatabaseError(format!("Save generation job failed: {}", e)))?;

        Ok(())
    }

    /// Invariant: must load and deserialize the GenerationJob associated with the given Ref from the database.
    pub fn load_generation_job(&self, r: &Ref) -> Result<GenerationJob, AetherError> {
        let ref_id = r.to_string();
        self.conn.query_row(
            "SELECT ref_id, kind, status, requested_model, resolved_model, provider_job_id,
                    prompt, inputs, artifacts, error, created_at_ms, updated_at_ms, options
             FROM generation_jobs WHERE ref_id = ?1",
            params![ref_id],
            |row| {
                let ref_id_str: String = row.get(0)?;
                let kind_str: String = row.get(1)?;
                let status_str: String = row.get(2)?;
                let requested_model: Option<String> = row.get(3)?;
                let resolved_model_str: Option<String> = row.get(4)?;
                let provider_job_id: Option<String> = row.get(5)?;
                let prompt_str: Option<String> = row.get(6)?;
                let inputs_str: String = row.get(7)?;
                let artifacts_str: String = row.get(8)?;
                let error: Option<String> = row.get(9)?;
                let created_at_ms: i64 = row.get(10)?;
                let updated_at_ms: i64 = row.get(11)?;
                let options_str: String = row.get(12)?;

                Ok((
                    ref_id_str,
                    kind_str,
                    status_str,
                    requested_model,
                    resolved_model_str,
                    provider_job_id,
                    prompt_str,
                    inputs_str,
                    artifacts_str,
                    error,
                    created_at_ms,
                    updated_at_ms,
                    options_str,
                ))
            },
        ).map_err(|e| AetherError::DatabaseError(format!("Query generation job failed: {}", e)))
        .and_then(|(ref_id_str, kind_str, status_str, requested_model, resolved_model_str, provider_job_id, prompt_str, inputs_str, artifacts_str, error, created_at_ms, updated_at_ms, options_str)| {
            let job_ref = ref_id_str.parse::<Ref>()?;
            let kind = serde_json::from_str(&kind_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize kind failed: {}", e)))?;
            let status = serde_json::from_str(&status_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize status failed: {}", e)))?;
            let resolved_model = match resolved_model_str {
                Some(s) => Some(serde_json::from_str(&s)
                    .map_err(|e| AetherError::DatabaseError(format!("Deserialize resolved_model failed: {}", e)))?),
                None => None,
            };
            let prompt = match prompt_str {
                Some(s) => Some(serde_json::from_str(&s)
                    .map_err(|e| AetherError::DatabaseError(format!("Deserialize prompt failed: {}", e)))?),
                None => None,
            };
            let inputs = serde_json::from_str(&inputs_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize inputs failed: {}", e)))?;
            let artifacts = serde_json::from_str(&artifacts_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize artifacts failed: {}", e)))?;
            let options = serde_json::from_str(&options_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize options failed: {}", e)))?;

            Ok(GenerationJob {
                job_ref,
                kind,
                status,
                requested_model,
                resolved_model,
                provider_job_id,
                prompt,
                inputs,
                artifacts,
                error,
                created_at_ms: created_at_ms as u64,
                updated_at_ms: updated_at_ms as u64,
                options,
            })
        })
    }

    /// Invariant: must load and deserialize all GenerationJobs from the database.
    pub fn load_all_generation_jobs(&self) -> Result<Vec<GenerationJob>, AetherError> {
        let mut stmt = self.conn.prepare(
            "SELECT ref_id FROM generation_jobs"
        ).map_err(|e| AetherError::DatabaseError(format!("Prepare generation jobs query failed: {}", e)))?;

        let ref_iter = stmt.query_map([], |row| {
            let ref_id_str: String = row.get(0)?;
            Ok(ref_id_str)
        }).map_err(|e| AetherError::DatabaseError(format!("Query generation jobs failed: {}", e)))?;

        let mut jobs = Vec::new();
        for item in ref_iter {
            let ref_id_str = item
                .map_err(|e| AetherError::DatabaseError(format!("Read generation job row failed: {}", e)))?;
            let r = ref_id_str.parse::<Ref>()?;
            let job = self.load_generation_job(&r)?;
            jobs.push(job);
        }

        Ok(jobs)
    }

    /// Invariant: must delete the GenerationJob associated with the given Ref from the database.
    pub fn delete_generation_job(&self, r: &Ref) -> Result<(), AetherError> {
        self.conn.execute(
            "DELETE FROM generation_jobs WHERE ref_id = ?1",
            params![r.to_string()],
        ).map_err(|e| AetherError::DatabaseError(format!("Delete generation job failed: {}", e)))?;
        Ok(())
    }

    /// Invariant: This function must preserve the invariant that a new event is successfully recorded in the SQLite `generation_events` table matching the provided job reference, status, and message, or else it returns a structured `AetherError` on failure.
    pub fn add_generation_event(&self, job_ref: &Ref, status: &GenerationStatus, message: &str) -> Result<(), AetherError> {
        let job_ref_str = job_ref.to_string();
        let status_str = serde_json::to_string(status)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize status failed: {}", e)))?;
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        self.conn.execute(
            "INSERT INTO generation_events (job_ref, status, message, timestamp_ms) VALUES (?1, ?2, ?3, ?4)",
            params![
                job_ref_str,
                status_str,
                message.to_string(),
                timestamp_ms,
            ],
        ).map_err(|e| AetherError::DatabaseError(format!("Add generation event failed: {}", e)))?;

        Ok(())
    }

    /// Helper to get DB file path.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::{RefKind, ProjectSettings, BlendMode, GenerationArtifact, ProviderModel, GenerationKind, GenerationStatus};

    fn temp_db_dir() -> PathBuf {
        let unique_dir = format!("test_aether_{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos());
        std::env::temp_dir().join(unique_dir)
    }

    #[test]
    fn test_db_initialization() {
        let dir = temp_db_dir();
        let db = DbManager::new(&dir).unwrap();
        assert!(db.db_path().exists());

        // Check loaded default settings
        let (settings, branch, commit) = db.load_settings().unwrap();
        assert_eq!(settings, ProjectSettings::default());
        assert_eq!(branch, "main");
        assert_eq!(commit, "");

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_and_load_settings() {
        let dir = temp_db_dir();
        let db = DbManager::new(&dir).unwrap();

        let new_settings = ProjectSettings {
            fps: 60.0,
            width: 3840,
            height: 2160,
            colorspace: "rec2020".to_string(),
        };

        db.save_settings(&new_settings, "main", "commit1").unwrap();

        let (loaded_settings, branch, commit) = db.load_settings().unwrap();
        assert_eq!(loaded_settings, new_settings);
        assert_eq!(branch, "main");
        assert_eq!(commit, "commit1");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_and_load_assets() {
        let dir = temp_db_dir();
        let db = DbManager::new(&dir).unwrap();

        let r1 = Ref { kind: RefKind::Video, id: 1 };
        let asset1 = Asset {
            r: r1,
            kind: AssetKind::Video,
            path: PathBuf::from("video1.mp4"),
            hash: "hash123".to_string(),
            metadata: serde_json::json!({ "duration": 5.0 }),
        };

        let r2 = Ref { kind: RefKind::Audio, id: 2 };
        let asset2 = Asset {
            r: r2,
            kind: AssetKind::Audio,
            path: PathBuf::from("audio1.wav"),
            hash: "hash456".to_string(),
            metadata: serde_json::json!({ "channels": 2 }),
        };

        db.save_asset(&asset1).unwrap();
        db.save_asset(&asset2).unwrap();

        let assets = db.load_assets().unwrap();
        assert_eq!(assets.len(), 2);
        assert!(assets.contains(&asset1));
        assert!(assets.contains(&asset2));

        // Delete asset
        db.delete_asset(&r1).unwrap();
        let assets = db.load_assets().unwrap();
        assert_eq!(assets.len(), 1);
        assert!(!assets.contains(&asset1));
        assert!(assets.contains(&asset2));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_history_management() {
        let dir = temp_db_dir();
        let db = DbManager::new(&dir).unwrap();

        let cmd1 = Command::Import { path: "source.mov".to_string() };
        let cmd2 = Command::Trim {
            r: Ref { kind: RefKind::Video, id: 1 },
            start: "00:01".to_string(),
            end: "00:05".to_string(),
        };

        db.add_history_entry("hash1", None, "main", &cmd1).unwrap();
        db.add_history_entry("hash2", Some("hash1"), "main", &cmd2).unwrap();

        let history = db.load_history().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0, "hash1");
        assert_eq!(history[0].1, None);
        assert_eq!(history[0].2, "main");
        assert_eq!(history[0].3, cmd1);

        assert_eq!(history[1].0, "hash2");
        assert_eq!(history[1].1, Some("hash1".to_string()));
        assert_eq!(history[1].2, "main");
        assert_eq!(history[1].3, cmd2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_load_graph() {
        let dir = temp_db_dir();
        let db = DbManager::new(&dir).unwrap();

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

        graph.connect(GraphConnection { from_node: 1, from_port: 0, to_node: 3, to_port: 0 }).unwrap();
        graph.connect(GraphConnection { from_node: 2, from_port: 0, to_node: 3, to_port: 1 }).unwrap();
        graph.connect(GraphConnection { from_node: 3, from_port: 0, to_node: 4, to_port: 0 }).unwrap();
        graph.output_node = Some(4);

        db.save_graph(&graph).unwrap();

        let loaded = db.load_graph().unwrap();
        assert_eq!(loaded, graph);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_load_timeline() {
        let dir = temp_db_dir();
        let db = DbManager::new(&dir).unwrap();

        let r1 = "@v1".parse::<Ref>().unwrap();
        let clip = Clip {
            asset_ref: r1,
            in_point_ms: 10,
            out_point_ms: 5000,
            track_offset_ms: 100,
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

        db.save_timeline(&timeline).unwrap();

        let loaded = db.load_timeline().unwrap();
        assert_eq!(loaded, timeline);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_load_generation_jobs() {
        let dir = temp_db_dir();
        let db = DbManager::new(&dir).unwrap();

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
            prompt: Some(aether_core::ProfessionalPrompt {
                original_request: "a cat".to_string(),
                professional_prompt: "a beautiful cat".to_string(),
                negative_prompt: None,
                locale: None,
                style: None,
                technical: serde_json::json!({}),
            }),
            inputs: Vec::new(),
            artifacts: vec![GenerationArtifact {
                kind: aether_core::GeneratedArtifactKind::Image,
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

        db.save_generation_job(&job).unwrap();

        let loaded = db.load_generation_job(&job_ref).unwrap();
        assert_eq!(loaded, job);

        let all = db.load_all_generation_jobs().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0], job);

        db.add_generation_event(&job_ref, &GenerationStatus::Ready, "Test event log").unwrap();

        db.delete_generation_job(&job_ref).unwrap();
        let all_after = db.load_all_generation_jobs().unwrap();
        assert!(all_after.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }
}

