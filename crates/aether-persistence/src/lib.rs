use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::collections::HashMap;
use rusqlite::{params, Connection, OptionalExtension};
use aether_core::{
    AetherError, Ref, Asset, AssetKind, ProjectSettings, Command,
    CompositionGraph, Node, Connection as GraphConnection, NodeId, NodeKind,
    Timeline, Track, Clip, TransitionKind, TrackKind
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
        // 1. Settings Table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS project_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                fps REAL NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                colorspace TEXT NOT NULL,
                history_cursor INTEGER NOT NULL DEFAULT 0
            );",
            [],
        ).map_err(|e| AetherError::DatabaseError(format!("Create project_settings failed: {}", e)))?;

        // Insert default settings if empty
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM project_settings",
            [],
            |row| row.get(0),
        ).map_err(|e| AetherError::DatabaseError(format!("Check settings count failed: {}", e)))?;

        if count == 0 {
            let defaults = ProjectSettings::default();
            self.conn.execute(
                "INSERT INTO project_settings (id, fps, width, height, colorspace, history_cursor)
                 VALUES (1, ?1, ?2, ?3, ?4, 0)",
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
                seq_id INTEGER PRIMARY KEY AUTOINCREMENT,
                command TEXT NOT NULL,
                hash_before TEXT,
                hash_after TEXT,
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

        Ok(())
    }

    /// Saves the current project settings and history cursor.
    pub fn save_settings(&self, settings: &ProjectSettings, history_cursor: usize) -> Result<(), AetherError> {
        self.conn.execute(
            "UPDATE project_settings
             SET fps = ?1, width = ?2, height = ?3, colorspace = ?4, history_cursor = ?5
             WHERE id = 1",
            params![
                settings.fps,
                settings.width,
                settings.height,
                settings.colorspace,
                history_cursor as i64
            ],
        ).map_err(|e| AetherError::DatabaseError(format!("Update settings failed: {}", e)))?;
        Ok(())
    }

    /// Loads the project settings and history cursor.
    pub fn load_settings(&self) -> Result<(ProjectSettings, usize), AetherError> {
        self.conn.query_row(
            "SELECT fps, width, height, colorspace, history_cursor FROM project_settings WHERE id = 1",
            [],
            |row| {
                let fps: f64 = row.get(0)?;
                let width: u32 = row.get(1)?;
                let height: u32 = row.get(2)?;
                let colorspace: String = row.get(3)?;
                let history_cursor: i64 = row.get(4)?;
                Ok((
                    ProjectSettings {
                        fps: fps as f32,
                        width,
                        height,
                        colorspace,
                    },
                    history_cursor as usize,
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

            let r = Ref::from_str(&ref_str)?;
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
        command: &Command,
        hash_before: Option<&str>,
        hash_after: Option<&str>,
    ) -> Result<usize, AetherError> {
        let cmd_str = serde_json::to_string(command)
            .map_err(|e| AetherError::DatabaseError(format!("Serialize command failed: {}", e)))?;

        self.conn.execute(
            "INSERT INTO history (command, hash_before, hash_after)
             VALUES (?1, ?2, ?3)",
            params![cmd_str, hash_before, hash_after],
        ).map_err(|e| AetherError::DatabaseError(format!("Insert history failed: {}", e)))?;

        let id: i64 = self.conn.last_insert_rowid();
        Ok(id as usize)
    }

    /// Loads the complete execution history list.
    pub fn load_history(&self) -> Result<Vec<(usize, Command, Option<String>, Option<String>)>, AetherError> {
        let mut stmt = self.conn.prepare(
            "SELECT seq_id, command, hash_before, hash_after FROM history ORDER BY seq_id ASC",
        ).map_err(|e| AetherError::DatabaseError(format!("Prepare history query failed: {}", e)))?;

        let history_iter = stmt.query_map([], |row| {
            let seq_id: i64 = row.get(0)?;
            let cmd_str: String = row.get(1)?;
            let hash_before: Option<String> = row.get(2)?;
            let hash_after: Option<String> = row.get(3)?;
            Ok((seq_id as usize, cmd_str, hash_before, hash_after))
        }).map_err(|e| AetherError::DatabaseError(format!("Query history failed: {}", e)))?;

        let mut history = Vec::new();
        for item in history_iter {
            let (seq_id, cmd_str, hash_before, hash_after) = item
                .map_err(|e| AetherError::DatabaseError(format!("Read history row failed: {}", e)))?;

            let command: Command = serde_json::from_str(&cmd_str)
                .map_err(|e| AetherError::DatabaseError(format!("Deserialize command failed: {}", e)))?;

            history.push((seq_id, command, hash_before, hash_after));
        }

        Ok(history)
    }

    /// Clears the history log entirely or trims it up to a sequence ID.
    pub fn truncate_history_after(&self, seq_id: usize) -> Result<(), AetherError> {
        self.conn.execute(
            "DELETE FROM history WHERE seq_id > ?1",
            params![seq_id as i64],
        ).map_err(|e| AetherError::DatabaseError(format!("Truncate history failed: {}", e)))?;
        Ok(())
    }

    /// Clears the history log entirely.
    pub fn clear_history(&self) -> Result<(), AetherError> {
        self.conn.execute("DELETE FROM history", [])
            .map_err(|e| AetherError::DatabaseError(format!("Clear history failed: {}", e)))?;
        Ok(())
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

                let asset_ref = Ref::from_str(&asset_ref_str)?;
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

    /// Helper to get DB file path.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::{RefKind, ProjectSettings, BlendMode};

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
        let (settings, cursor) = db.load_settings().unwrap();
        assert_eq!(settings, ProjectSettings::default());
        assert_eq!(cursor, 0);

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

        db.save_settings(&new_settings, 5).unwrap();

        let (loaded_settings, cursor) = db.load_settings().unwrap();
        assert_eq!(loaded_settings, new_settings);
        assert_eq!(cursor, 5);

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

        let id1 = db.add_history_entry(&cmd1, None, Some("hash_state_1")).unwrap();
        let id2 = db.add_history_entry(&cmd2, Some("hash_state_1"), Some("hash_state_2")).unwrap();

        let history = db.load_history().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0, id1);
        assert_eq!(history[0].1, cmd1);
        assert_eq!(history[0].2, None);
        assert_eq!(history[0].3, Some("hash_state_1".to_string()));

        assert_eq!(history[1].0, id2);
        assert_eq!(history[1].1, cmd2);
        assert_eq!(history[1].2, Some("hash_state_1".to_string()));
        assert_eq!(history[1].3, Some("hash_state_2".to_string()));

        // Truncate history
        db.truncate_history_after(id1).unwrap();
        let history = db.load_history().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].0, id1);

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
}

