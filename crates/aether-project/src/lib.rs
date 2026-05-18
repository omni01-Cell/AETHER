use std::path::{Path, PathBuf};
use aether_core::{AetherError, ProjectMetadata, ProjectRegistry, ProjectRegistryEntry, ProjectStatus};

#[derive(Debug, Clone)]
pub struct ProjectCreateSpec {
    pub name: String,
    pub dir: Option<PathBuf>,
    pub adopt: bool,
    pub force: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteMode {
    Archive,
    Force,
}

pub struct ProjectManager {
    registry_path: PathBuf,
}

impl ProjectManager {
    /// Loads the ProjectManager using the default global registry path `~/.config/aether/projects.json`.
    pub fn load_default() -> Result<Self, AetherError> {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/home/omni"));
        let registry_path = home.join(".config/aether/projects.json");
        Ok(ProjectManager { registry_path })
    }

    /// Loads the ProjectManager using a custom registry path (primarily for testing).
    pub fn with_registry_path(registry_path: PathBuf) -> Self {
        ProjectManager { registry_path }
    }

    /// Loads the global project registry from disk.
    pub fn load_registry(&self) -> Result<ProjectRegistry, AetherError> {
        if !self.registry_path.exists() {
            return Ok(ProjectRegistry {
                schema_version: 1,
                active_project_id: None,
                projects: Vec::new(),
            });
        }
        let content = std::fs::read_to_string(&self.registry_path).map_err(|e| {
            AetherError::IoError(self.registry_path.to_string_lossy().to_string(), e.to_string())
        })?;
        serde_json::from_str(&content).map_err(|e| {
            AetherError::OperationFailed(format!("Failed to parse registry: {}", e))
        })
    }

    /// Saves the project registry atomically to disk.
    pub fn save_registry(&self, registry: &ProjectRegistry) -> Result<(), AetherError> {
        let parent = self.registry_path.parent().ok_or_else(|| {
            AetherError::IoError(
                self.registry_path.to_string_lossy().to_string(),
                "No parent directory".to_string(),
            )
        })?;
        std::fs::create_dir_all(parent).map_err(|e| {
            AetherError::IoError(parent.to_string_lossy().to_string(), e.to_string())
        })?;
        
        let temp_path = self.registry_path.with_extension("tmp");
        let content = serde_json::to_string_pretty(registry).map_err(|e| {
            AetherError::OperationFailed(format!("Failed to serialize registry: {}", e))
        })?;
        
        std::fs::write(&temp_path, content).map_err(|e| {
            AetherError::IoError(temp_path.to_string_lossy().to_string(), e.to_string())
        })?;
        
        std::fs::rename(&temp_path, &self.registry_path).map_err(|e| {
            AetherError::IoError(self.registry_path.to_string_lossy().to_string(), e.to_string())
        })?;
        
        Ok(())
    }

    /// Loads the metadata file from the specified project directory.
    pub fn load_project_json(project_dir: &Path) -> Result<ProjectMetadata, AetherError> {
        let path = project_dir.join(".aether/project.json");
        if !path.exists() {
            return Err(AetherError::IoError(
                path.to_string_lossy().to_string(),
                "project.json does not exist".to_string(),
            ));
        }
        let content = std::fs::read_to_string(&path).map_err(|e| {
            AetherError::IoError(path.to_string_lossy().to_string(), e.to_string())
        })?;
        serde_json::from_str(&content).map_err(|e| {
            AetherError::OperationFailed(format!("Failed to parse project.json: {}", e))
        })
    }

    /// Saves the project metadata file inside the project directory.
    pub fn save_project_json(project_dir: &Path, meta: &ProjectMetadata) -> Result<(), AetherError> {
        let aether_dir = project_dir.join(".aether");
        std::fs::create_dir_all(&aether_dir).map_err(|e| {
            AetherError::IoError(aether_dir.to_string_lossy().to_string(), e.to_string())
        })?;
        let path = aether_dir.join("project.json");
        let content = serde_json::to_string_pretty(meta).map_err(|e| {
            AetherError::OperationFailed(format!("Failed to serialize project.json: {}", e))
        })?;
        std::fs::write(&path, content).map_err(|e| {
            AetherError::IoError(path.to_string_lossy().to_string(), e.to_string())
        })?;
        Ok(())
    }

    /// Creates a new AETHER project.
    pub fn create(&self, spec: ProjectCreateSpec) -> Result<ProjectMetadata, AetherError> {
        let root_dir = match &spec.dir {
            Some(d) => {
                std::fs::create_dir_all(d).map_err(|e| AetherError::IoError(d.to_string_lossy().to_string(), e.to_string()))?;
                std::fs::canonicalize(d).map_err(|e| AetherError::IoError(d.to_string_lossy().to_string(), e.to_string()))?
            }
            None => {
                let d = std::env::current_dir().map_err(|e| AetherError::IoError("current_dir".to_string(), e.to_string()))?;
                std::fs::canonicalize(d).map_err(|e| AetherError::IoError("current_dir".to_string(), e.to_string()))?
            }
        };

        let has_aether = root_dir.join(".aether").exists();
        let has_db = root_dir.join(".aether/metadata.db").exists();
        let has_meta = root_dir.join(".aether/project.json").exists();
        
        if has_meta && !spec.force {
            let existing = Self::load_project_json(&root_dir)?;
            return Err(AetherError::OperationFailed(format!(
                "Directory already contains AETHER project '{}' ({})",
                existing.name, existing.project_id
            )));
        }
        
        if has_db && !has_meta && !spec.adopt && !spec.force {
            return Err(AetherError::OperationFailed(
                "Directory contains an unmanaged/legacy AETHER database. Use --adopt or --force.".to_string()
            ));
        }
        
        if !has_aether {
            let is_empty = std::fs::read_dir(&root_dir)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(true);
            if !is_empty && !spec.adopt && !spec.force {
                return Err(AetherError::OperationFailed(
                    "Directory is not empty. Use --adopt or --force to create project here.".to_string()
                ));
            }
        }

        let aether_dir = root_dir.join(".aether");
        if spec.force && aether_dir.exists() {
            let db_path = aether_dir.join("metadata.db");
            if db_path.exists() {
                let _ = std::fs::remove_file(&db_path);
            }
        }
        
        // This will create the .aether/ directory and metadata.db, running schema setups
        let _db = aether_persistence::DbManager::new(&aether_dir)?;

        let project_id = uuid::Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let meta = ProjectMetadata {
            schema_version: 1,
            project_id: project_id.clone(),
            name: spec.name.clone(),
            root: root_dir.clone(),
            created_at_ms: now,
            updated_at_ms: now,
            aether_version: "0.1.0".to_string(),
            description: None,
        };

        // Write local project.json
        Self::save_project_json(&root_dir, &meta)?;

        // Update global registry
        let mut registry = self.load_registry()?;
        registry.projects.retain(|p| p.root != root_dir && p.name != spec.name);
        registry.projects.push(ProjectRegistryEntry {
            project_id: project_id.clone(),
            name: spec.name.clone(),
            root: root_dir.clone(),
            last_opened_at_ms: now,
            status: ProjectStatus::Open,
        });
        registry.active_project_id = Some(project_id);
        self.save_registry(&registry)?;

        Ok(meta)
    }

    /// Opens a project by name, ID, or path.
    pub fn open(&self, target: &str) -> Result<ProjectMetadata, AetherError> {
        let resolved_dir = if Path::new(target).exists() {
            let root = std::fs::canonicalize(target).map_err(|e| {
                AetherError::IoError(target.to_string(), e.to_string())
            })?;
            if !root.join(".aether/project.json").exists() {
                return Err(AetherError::OperationFailed(format!(
                    "Directory '{}' does not contain an AETHER project (missing project.json)",
                    root.to_string_lossy()
                )));
            }
            root
        } else {
            let registry = self.load_registry()?;
            let entry = registry.projects.iter().find(|p| p.name == target || p.project_id == target)
                .ok_or_else(|| AetherError::OperationFailed(format!("Project '{}' not found in registry", target)))?;
            if !entry.root.exists() {
                return Err(AetherError::OperationFailed(format!(
                    "Project root directory '{}' does not exist anymore",
                    entry.root.to_string_lossy()
                )));
            }
            entry.root.clone()
        };

        let meta = Self::load_project_json(&resolved_dir)?;
        let mut registry = self.load_registry()?;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        if let Some(entry) = registry.projects.iter_mut().find(|p| p.project_id == meta.project_id) {
            entry.status = ProjectStatus::Open;
            entry.last_opened_at_ms = now;
        } else {
            registry.projects.push(ProjectRegistryEntry {
                project_id: meta.project_id.clone(),
                name: meta.name.clone(),
                root: resolved_dir.clone(),
                last_opened_at_ms: now,
                status: ProjectStatus::Open,
            });
        }
        registry.active_project_id = Some(meta.project_id.clone());
        self.save_registry(&registry)?;

        Ok(meta)
    }

    /// Returns the currently active project metadata if present.
    pub fn current(&self) -> Result<Option<ProjectMetadata>, AetherError> {
        let registry = self.load_registry()?;
        if let Some(active_id) = &registry.active_project_id {
            if let Some(entry) = registry.projects.iter().find(|p| p.project_id == *active_id) {
                if entry.root.exists() {
                    let meta = Self::load_project_json(&entry.root)?;
                    return Ok(Some(meta));
                }
            }
        }
        Ok(None)
    }

    /// Closes a specific project or the currently active project.
    pub fn close(&self, target: Option<&str>) -> Result<(), AetherError> {
        let mut registry = self.load_registry()?;
        let target_id = if let Some(t) = target {
            let entry = if Path::new(t).exists() {
                let canon = std::fs::canonicalize(t).map_err(|e| AetherError::IoError(t.to_string(), e.to_string()))?;
                registry.projects.iter().find(|p| p.root == canon)
            } else {
                registry.projects.iter().find(|p| p.name == t || p.project_id == t)
            };
            match entry {
                Some(e) => Some(e.project_id.clone()),
                None => return Err(AetherError::OperationFailed(format!("Project '{}' not found", t))),
            }
        } else {
            registry.active_project_id.clone()
        };

        if let Some(id) = target_id {
            if let Some(entry) = registry.projects.iter_mut().find(|p| p.project_id == id) {
                entry.status = ProjectStatus::Closed;
            }
            if registry.active_project_id == Some(id) {
                registry.active_project_id = None;
            }
            self.save_registry(&registry)?;
        } else {
            return Err(AetherError::OperationFailed("No active project to close".to_string()));
        }
        Ok(())
    }

    /// Lists all projects in the registry, updating their statuses dynamically.
    pub fn list(&self) -> Result<Vec<ProjectRegistryEntry>, AetherError> {
        let mut registry = self.load_registry()?;
        for entry in &mut registry.projects {
            if !entry.root.exists() {
                entry.status = ProjectStatus::Missing;
            }
        }
        Ok(registry.projects)
    }

    /// Deletes or archives a project.
    pub fn delete(&self, target: &str, mode: DeleteMode) -> Result<(), AetherError> {
        let mut registry = self.load_registry()?;
        
        let (index, entry) = if Path::new(target).exists() {
            let canon = std::fs::canonicalize(target).map_err(|e| AetherError::IoError(target.to_string(), e.to_string()))?;
            registry.projects.iter().enumerate().find(|(_, p)| p.root == canon)
                .map(|(i, p)| (i, p.clone()))
                .ok_or_else(|| AetherError::OperationFailed(format!("Project path '{}' not found in registry", target)))?
        } else {
            registry.projects.iter().enumerate().find(|(_, p)| p.name == target || p.project_id == target)
                .map(|(i, p)| (i, p.clone()))
                .ok_or_else(|| AetherError::OperationFailed(format!("Project '{}' not found in registry", target)))?
        };

        let project_dir = &entry.root;
        let project_dir_canon = std::fs::canonicalize(project_dir).map_err(|e| {
            AetherError::IoError(project_dir.to_string_lossy().to_string(), e.to_string())
        })?;
        
        // SAFETY CHECKS
        if project_dir_canon.parent().is_none() {
            return Err(AetherError::OperationFailed("Refusing to delete root directory '/'".to_string()));
        }
        
        if let Ok(home) = std::env::var("HOME") {
            if let Ok(home_canon) = std::fs::canonicalize(&home) {
                if project_dir_canon == home_canon {
                    return Err(AetherError::OperationFailed("Refusing to delete home directory $HOME".to_string()));
                }
            }
        }
        
        if project_dir_canon.join("Cargo.toml").exists() && project_dir_canon.join("crates").exists() {
            return Err(AetherError::OperationFailed("Refusing to delete the AETHER repository itself".to_string()));
        }
        
        let project_json_path = project_dir_canon.join(".aether/project.json");
        if !project_json_path.exists() {
            return Err(AetherError::OperationFailed(
                "Target folder is not a valid AETHER project (missing .aether/project.json)".to_string()
            ));
        }
        
        let disk_meta = Self::load_project_json(&project_dir_canon)?;
        if disk_meta.project_id != entry.project_id {
            return Err(AetherError::OperationFailed(
                "Project ID mismatch between registry and disk".to_string()
            ));
        }

        if registry.active_project_id == Some(entry.project_id.clone()) {
            registry.active_project_id = None;
        }
        
        registry.projects.remove(index);
        self.save_registry(&registry)?;

        match mode {
            DeleteMode::Archive => {
                let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/home/omni"));
                let trash_dir = home.join(".local/share/aether/trash").join(format!("{}-{}", entry.name, entry.project_id));
                
                if let Some(parent) = trash_dir.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        AetherError::IoError(parent.to_string_lossy().to_string(), e.to_string())
                    })?;
                }
                
                std::fs::rename(&project_dir_canon, &trash_dir).map_err(|e| {
                    AetherError::IoError(project_dir_canon.to_string_lossy().to_string(), e.to_string())
                })?;
            }
            DeleteMode::Force => {
                std::fs::remove_dir_all(&project_dir_canon).map_err(|e| {
                    AetherError::IoError(project_dir_canon.to_string_lossy().to_string(), e.to_string())
                })?;
            }
        }

        Ok(())
    }

    /// Resolves the active project folder path for a command execution.
    pub fn resolve_for_command(&self, explicit: Option<&str>) -> Result<PathBuf, AetherError> {
        // 1. Explicit option
        if let Some(target) = explicit {
            return self.resolve_path(target);
        }
        
        // 2. Env variable AETHER_PROJECT
        if let Ok(target) = std::env::var("AETHER_PROJECT") {
            if !target.is_empty() {
                return self.resolve_path(&target);
            }
        }
        
        // 3. Active project of global registry
        let registry = self.load_registry()?;
        if let Some(active_id) = &registry.active_project_id {
            if let Some(entry) = registry.projects.iter().find(|p| p.project_id == *active_id) {
                if entry.root.exists() {
                    return Ok(entry.root.clone());
                }
            }
        }
        
        // 4. Current dir if <cwd>/.aether exists
        let cwd = std::env::current_dir().map_err(|e| {
            AetherError::IoError("current_dir".to_string(), e.to_string())
        })?;
        if cwd.join(".aether").exists() {
            return Ok(cwd);
        }
        
        // 5. Error
        Err(AetherError::OperationFailed(
            "No active AETHER project. Run 'aether project create <name>' or 'aether project open <name>'".to_string()
        ))
    }

    fn resolve_path(&self, target: &str) -> Result<PathBuf, AetherError> {
        if Path::new(target).exists() {
            let canon = std::fs::canonicalize(target).map_err(|e| {
                AetherError::IoError(target.to_string(), e.to_string())
            })?;
            if canon.join(".aether").exists() {
                return Ok(canon);
            }
        }
        
        let registry = self.load_registry()?;
        if let Some(entry) = registry.projects.iter().find(|p| p.name == target || p.project_id == target) {
            if entry.root.exists() {
                return Ok(entry.root.clone());
            }
        }
        
        Err(AetherError::OperationFailed(format!(
            "Could not resolve project target '{}'",
            target
        )))
    }
}
