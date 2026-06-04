use std::path::{Path, PathBuf};
use std::fs;
use aether_core::{
    AetherError, PromptContext, Vault, VaultAsset, VaultAssetKind, VaultAssetRef, VaultKind,
    VaultLink, VaultLinks, VaultPromptContext, VaultUsage,
};

pub struct VaultManager {
    registry_path: PathBuf,
    storage_root: PathBuf,
}

impl VaultManager {
    /// Loads the VaultManager using the default global registry path `~/.config/aether/vaults.json`.
    pub fn load_default() -> Result<Self, AetherError> {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/home/omni"));
        let registry_path = home.join(".config/aether/vaults.json");
        let storage_root = home.join(".local/share/aether/vaults");
        Ok(VaultManager { registry_path, storage_root })
    }

    /// Loads the VaultManager using a custom registry path (primarily for testing).
    pub fn with_registry_path(registry_path: PathBuf) -> Self {
        let storage_root = registry_path
            .parent()
            .unwrap_or(&registry_path)
            .parent()
            .map(|p| p.join("local/share/aether/vaults"))
            .unwrap_or_else(|| {
                let home = std::env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("/home/omni"));
                home.join(".local/share/aether/vaults")
            });
        VaultManager { registry_path, storage_root }
    }

    /// Loads the VaultManager using custom registry and storage root paths.
    pub fn with_paths(registry_path: PathBuf, storage_root: PathBuf) -> Self {
        VaultManager { registry_path, storage_root }
    }

    /// Helper to resolve the vault storage root path.
    pub fn vault_storage_root(&self) -> PathBuf {
        self.storage_root.clone()
    }

    /// Loads the global vault registry.
    pub fn load_registry(&self) -> Result<Vec<Vault>, AetherError> {
        if !self.registry_path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.registry_path).map_err(|e| {
            AetherError::IoError(self.registry_path.to_string_lossy().to_string(), e.to_string())
        })?;
        serde_json::from_str(&content).map_err(|e| {
            AetherError::VaultError(format!("Failed to parse vault registry: {}", e))
        })
    }

    /// Saves the global vault registry.
    pub fn save_registry(&self, vaults: &[Vault]) -> Result<(), AetherError> {
        if let Some(parent) = self.registry_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AetherError::IoError(parent.to_string_lossy().to_string(), e.to_string())
            })?;
        }
        let content = serde_json::to_string_pretty(vaults).map_err(|e| {
            AetherError::VaultError(format!("Failed to serialize vault registry: {}", e))
        })?;
        fs::write(&self.registry_path, content).map_err(|e| {
            AetherError::IoError(self.registry_path.to_string_lossy().to_string(), e.to_string())
        })?;
        Ok(())
    }

    /// Creates a new AETHER Vault.
    pub fn create_vault(&self, name: &str, kind: VaultKind, description: Option<String>) -> Result<Vault, AetherError> {
        let mut vaults = self.load_registry()?;
        let vault_id = name.to_lowercase().replace(' ', "_");

        if vaults.iter().any(|v| v.vault_id == vault_id) {
            return Err(AetherError::VaultError(format!("Vault with ID '{}' already exists", vault_id)));
        }

        let vault_dir = self.vault_storage_root().join(&vault_id);
        fs::create_dir_all(&vault_dir).map_err(|e| {
            AetherError::IoError(vault_dir.to_string_lossy().to_string(), e.to_string())
        })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let vault = Vault {
            schema_version: 1,
            vault_id: vault_id.clone(),
            name: name.to_string(),
            kind,
            root: vault_dir.clone(),
            created_at_ms: now,
            updated_at_ms: now,
            description,
            default_language: Some("en".to_string()),
            owner: None,
        };

        // Save local vault.json
        let vault_json_path = vault_dir.join("vault.json");
        let vault_content = serde_json::to_string_pretty(&vault).map_err(|e| {
            AetherError::VaultError(format!("Failed to serialize vault.json: {}", e))
        })?;
        fs::write(&vault_json_path, vault_content).map_err(|e| {
            AetherError::IoError(vault_json_path.to_string_lossy().to_string(), e.to_string())
        })?;

        // Initialize empty assets directory inside vault
        let assets_dir = vault_dir.join("assets");
        fs::create_dir_all(&assets_dir).map_err(|e| {
            AetherError::IoError(assets_dir.to_string_lossy().to_string(), e.to_string())
        })?;

        // Save to global registry
        vaults.push(vault.clone());
        self.save_registry(&vaults)?;

        Ok(vault)
    }

    /// Loads the assets list of a specific Vault.
    pub fn load_assets(&self, vault_id: &str) -> Result<Vec<VaultAsset>, AetherError> {
        let vault_dir = self.vault_storage_root().join(vault_id);
        let assets_json_path = vault_dir.join("assets.json");
        if !assets_json_path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&assets_json_path).map_err(|e| {
            AetherError::IoError(assets_json_path.to_string_lossy().to_string(), e.to_string())
        })?;
        serde_json::from_str(&content).map_err(|e| {
            AetherError::VaultError(format!("Failed to parse assets.json for vault '{}': {}", vault_id, e))
        })
    }

    /// Saves the assets list of a specific Vault.
    pub fn save_assets(&self, vault_id: &str, assets: &[VaultAsset]) -> Result<(), AetherError> {
        let vault_dir = self.vault_storage_root().join(vault_id);
        let assets_json_path = vault_dir.join("assets.json");
        let content = serde_json::to_string_pretty(assets).map_err(|e| {
            AetherError::VaultError(format!("Failed to serialize assets.json for vault '{}': {}", vault_id, e))
        })?;
        fs::write(&assets_json_path, content).map_err(|e| {
            AetherError::IoError(assets_json_path.to_string_lossy().to_string(), e.to_string())
        })?;
        Ok(())
    }

    /// Adds a file asset to the Vault.
    pub fn add_file_asset(
        &self,
        vault_id: &str,
        name: &str,
        kind: VaultAssetKind,
        source_path: &Path,
        usage: Vec<VaultUsage>,
        tags: Vec<String>,
        metadata: serde_json::Value,
    ) -> Result<VaultAsset, AetherError> {
        let vaults = self.load_registry()?;
        if !vaults.iter().any(|v| v.vault_id == vault_id) {
            return Err(AetherError::VaultError(format!("Vault '{}' not found", vault_id)));
        }

        if !source_path.exists() {
            return Err(AetherError::IoError(
                source_path.to_string_lossy().to_string(),
                "Source file does not exist".to_string(),
            ));
        }

        let file_bytes = fs::read(source_path).map_err(|e| {
            AetherError::IoError(source_path.to_string_lossy().to_string(), e.to_string())
        })?;
        let hash = blake3::hash(&file_bytes).to_hex().to_string();

        let ext = source_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");
        let dest_filename = format!("{}.{}", hash, ext);

        let vault_dir = self.vault_storage_root().join(vault_id);
        let dest_path = vault_dir.join("assets").join(&dest_filename);

        fs::write(&dest_path, file_bytes).map_err(|e| {
            AetherError::IoError(dest_path.to_string_lossy().to_string(), e.to_string())
        })?;

        let mut assets = self.load_assets(vault_id)?;
        let vault_asset_id = uuid::Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let asset = VaultAsset {
            vault_asset_id: vault_asset_id.clone(),
            vault_id: vault_id.to_string(),
            kind,
            name: name.to_string(),
            path: dest_path.clone(),
            hash,
            tags,
            usage,
            metadata,
            created_at_ms: now,
            updated_at_ms: now,
        };

        assets.retain(|a| a.name != name);
        assets.push(asset.clone());
        self.save_assets(vault_id, &assets)?;

        Ok(asset)
    }

    /// Adds a pure text/rule asset to the Vault.
    pub fn add_text_asset(
        &self,
        vault_id: &str,
        name: &str,
        kind: VaultAssetKind,
        text: &str,
        usage: Vec<VaultUsage>,
        tags: Vec<String>,
        mut metadata: serde_json::Value,
    ) -> Result<VaultAsset, AetherError> {
        let vaults = self.load_registry()?;
        if !vaults.iter().any(|v| v.vault_id == vault_id) {
            return Err(AetherError::VaultError(format!("Vault '{}' not found", vault_id)));
        }

        let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
        let vault_dir = self.vault_storage_root().join(vault_id);
        let dest_filename = format!("{}.txt", hash);
        let dest_path = vault_dir.join("assets").join(&dest_filename);

        fs::write(&dest_path, text).map_err(|e| {
            AetherError::IoError(dest_path.to_string_lossy().to_string(), e.to_string())
        })?;

        let mut assets = self.load_assets(vault_id)?;
        let vault_asset_id = uuid::Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        if let serde_json::Value::Object(ref mut map) = metadata {
            map.insert("text".to_string(), serde_json::Value::String(text.to_string()));
        }

        let asset = VaultAsset {
            vault_asset_id: vault_asset_id.clone(),
            vault_id: vault_id.to_string(),
            kind,
            name: name.to_string(),
            path: dest_path.clone(),
            hash,
            tags,
            usage,
            metadata,
            created_at_ms: now,
            updated_at_ms: now,
        };

        assets.retain(|a| a.name != name);
        assets.push(asset.clone());
        self.save_assets(vault_id, &assets)?;

        Ok(asset)
    }

    /// Resolves `vault_links.json` from the project directory.
    pub fn load_project_links(project_dir: &Path) -> Result<VaultLinks, AetherError> {
        let path = project_dir.join(".aether/vault_links.json");
        if !path.exists() {
            return Ok(VaultLinks {
                schema_version: 1,
                attached_vaults: Vec::new(),
            });
        }
        let content = fs::read_to_string(&path).map_err(|e| {
            AetherError::IoError(path.to_string_lossy().to_string(), e.to_string())
        })?;
        serde_json::from_str(&content).map_err(|e| {
            AetherError::VaultError(format!("Failed to parse vault_links.json: {}", e))
        })
    }

    /// Saves `vault_links.json` to the project directory.
    pub fn save_project_links(project_dir: &Path, links: &VaultLinks) -> Result<(), AetherError> {
        let aether_dir = project_dir.join(".aether");
        fs::create_dir_all(&aether_dir).map_err(|e| {
            AetherError::IoError(aether_dir.to_string_lossy().to_string(), e.to_string())
        })?;
        let path = aether_dir.join("vault_links.json");
        let content = serde_json::to_string_pretty(links).map_err(|e| {
            AetherError::VaultError(format!("Failed to serialize vault_links.json: {}", e))
        })?;
        fs::write(&path, content).map_err(|e| {
            AetherError::IoError(path.to_string_lossy().to_string(), e.to_string())
        })?;
        Ok(())
    }

    /// Attaches a Vault to a project.
    pub fn attach_vault(&self, project_dir: &Path, vault_id: &str, alias: &str) -> Result<(), AetherError> {
        let vaults = self.load_registry()?;
        if !vaults.iter().any(|v| v.vault_id == vault_id) {
            return Err(AetherError::VaultError(format!("Vault '{}' not found", vault_id)));
        }

        let mut links = Self::load_project_links(project_dir)?;
        if links.attached_vaults.iter().any(|l| l.vault_id == vault_id) {
            return Ok(()); // Already attached
        }

        links.attached_vaults.push(VaultLink {
            vault_id: vault_id.to_string(),
            alias: alias.to_string(),
            scope: "default".to_string(),
            locked_version: None,
        });

        Self::save_project_links(project_dir, &links)?;
        Ok(())
    }

    /// Detaches a Vault from a project.
    pub fn detach_vault(&self, project_dir: &Path, vault_id: &str) -> Result<(), AetherError> {
        let mut links = Self::load_project_links(project_dir)?;
        let before_len = links.attached_vaults.len();
        links.attached_vaults.retain(|l| l.vault_id != vault_id);

        if links.attached_vaults.len() == before_len {
            return Err(AetherError::VaultError(format!("Vault '{}' was not attached to this project", vault_id)));
        }

        Self::save_project_links(project_dir, &links)?;
        Ok(())
    }

    /// Compiles PromptContext from the Vaults attached to a project.
    pub fn compile_prompt_context(&self, project_dir: &Path) -> Result<PromptContext, AetherError> {
        let links = Self::load_project_links(project_dir)?;
        let vaults = self.load_registry()?;

        let mut vault_contexts = Vec::new();

        for link in &links.attached_vaults {
            if let Some(vault) = vaults.iter().find(|v| v.vault_id == link.vault_id) {
                let assets = self.load_assets(&link.vault_id)?;
                let mut rules = Vec::new();
                let mut prompt_snippets = Vec::new();
                let mut negative_prompts = Vec::new();
                let mut reference_assets = Vec::new();

                for asset in assets {
                    match asset.kind {
                        VaultAssetKind::DesignRulebook | VaultAssetKind::LegalGuideline => {
                            if let Some(txt) = asset.metadata.get("text").and_then(|t| t.as_str()) {
                                rules.push(txt.to_string());
                            } else {
                                rules.push(asset.name.clone());
                            }
                        }
                        VaultAssetKind::PromptSnippet => {
                            if let Some(txt) = asset.metadata.get("text").and_then(|t| t.as_str()) {
                                prompt_snippets.push(txt.to_string());
                            } else {
                                prompt_snippets.push(asset.name.clone());
                            }
                        }
                        VaultAssetKind::NegativePrompt => {
                            if let Some(txt) = asset.metadata.get("text").and_then(|t| t.as_str()) {
                                negative_prompts.push(txt.to_string());
                            } else {
                                negative_prompts.push(asset.name.clone());
                            }
                        }
                        _ => {
                            reference_assets.push(VaultAssetRef {
                                vault_asset_id: asset.vault_asset_id,
                                name: asset.name,
                                kind: asset.kind,
                                path: asset.path,
                                hash: asset.hash,
                                tags: asset.tags,
                                metadata: asset.metadata,
                            });
                        }
                    }
                }

                vault_contexts.push(VaultPromptContext {
                    vault_id: vault.vault_id.clone(),
                    name: vault.name.clone(),
                    kind: vault.kind,
                    rules,
                    prompt_snippets,
                    negative_prompts,
                    reference_assets,
                });
            }
        }

        Ok(PromptContext {
            project_summary: None,
            style_hints: Vec::new(),
            negative_constraints: Vec::new(),
            vault_context: vault_contexts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use aether_core::VaultKind;

    fn setup_test_manager(test_name: &str) -> (VaultManager, PathBuf) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap().parent().unwrap().to_path_buf();
        let test_dir = workspace_root
            .join("target")
            .join("test_vaults")
            .join(test_name);

        if test_dir.exists() {
            let _ = fs::remove_dir_all(&test_dir);
        }
        fs::create_dir_all(&test_dir).unwrap();

        let registry_path = test_dir.join("vaults.json");
        let storage_root = test_dir.join("vaults");
        (VaultManager::with_paths(registry_path, storage_root), test_dir)
    }

    #[test]
    fn test_vault_lifecycle() {
        let (vm, test_dir) = setup_test_manager("test_vault_lifecycle");

        // 1. Create Brand Vault
        let vault = vm
            .create_vault("Maison Lux Time", VaultKind::Brand, Some("Luxurious Brand Kit".to_string()))
            .expect("Failed to create vault");
        assert_eq!(vault.vault_id, "maison_lux_time");
        assert_eq!(vault.name, "Maison Lux Time");
        assert_eq!(vault.kind, VaultKind::Brand);
        assert!(vault.root.exists());
        assert!(vault.root.join("vault.json").exists());
        assert!(vault.root.join("assets").exists());

        // Registry should list it
        let registry = vm.load_registry().unwrap();
        assert_eq!(registry.len(), 1);
        assert_eq!(registry[0].vault_id, "maison_lux_time");

        // 2. Add text rule
        let rule = vm
            .add_text_asset(
                "maison_lux_time",
                "Placement Rule",
                VaultAssetKind::DesignRulebook,
                "Never stretch the logo. Keep minimum clear space of 8 percent.",
                vec![VaultUsage::PromptMaker],
                vec!["brand".to_string(), "logo".to_string()],
                serde_json::json!({}),
            )
            .expect("Failed to add text asset");
        assert_eq!(rule.name, "Placement Rule");
        assert_eq!(rule.kind, VaultAssetKind::DesignRulebook);
        assert!(rule.path.exists());

        // 3. Add file mock asset
        let mock_file = test_dir.join("logo.png");
        fs::write(&mock_file, "mock binary logo contents").unwrap();

        let logo = vm
            .add_file_asset(
                "maison_lux_time",
                "Primary Logo",
                VaultAssetKind::Logo,
                &mock_file,
                vec![VaultUsage::GenerateImage, VaultUsage::ExportBranding],
                vec!["logo".to_string(), "primary".to_string()],
                serde_json::json!({ "variant": "primary" }),
            )
            .expect("Failed to add file asset");
        assert_eq!(logo.name, "Primary Logo");
        assert_eq!(logo.kind, VaultAssetKind::Logo);
        assert!(logo.path.exists());

        // Load assets and verify
        let assets = vm.load_assets("maison_lux_time").unwrap();
        assert_eq!(assets.len(), 2);

        // 4. Project linking tests
        let project_dir = test_dir.join("test_project");
        fs::create_dir_all(&project_dir).unwrap();

        // Initially no links
        let links = VaultManager::load_project_links(&project_dir).unwrap();
        assert!(links.attached_vaults.is_empty());

        // Attach vault
        vm.attach_vault(&project_dir, "maison_lux_time", "brand").unwrap();
        let links = VaultManager::load_project_links(&project_dir).unwrap();
        assert_eq!(links.attached_vaults.len(), 1);
        assert_eq!(links.attached_vaults[0].vault_id, "maison_lux_time");
        assert_eq!(links.attached_vaults[0].alias, "brand");

        // Compile prompt context
        let context = vm.compile_prompt_context(&project_dir).unwrap();
        assert_eq!(context.vault_context.len(), 1);
        assert_eq!(context.vault_context[0].vault_id, "maison_lux_time");
        assert_eq!(context.vault_context[0].rules.len(), 1);
        assert!(context.vault_context[0].rules[0].contains("Never stretch the logo"));
        assert_eq!(context.vault_context[0].reference_assets.len(), 1);
        assert_eq!(context.vault_context[0].reference_assets[0].name, "Primary Logo");

        // Detach vault
        vm.detach_vault(&project_dir, "maison_lux_time").unwrap();
        let links = VaultManager::load_project_links(&project_dir).unwrap();
        assert!(links.attached_vaults.is_empty());
    }

    #[test]
    fn test_vault_generate_integration() {
        use aether_core::{GenerationKind, GenerationRequest, ProviderModel};
        use aether_generate::prompt::{PromptMaker, PromptMakerContext, RuleBasedPromptMaker};
        use aether_generate::runtime::DefaultGenerationRuntime;

        let (vm, test_dir) = setup_test_manager("test_vault_generate_integration");

        // 1. Create Brand Vault
        let _vault = vm
            .create_vault("Maison Lux Time", VaultKind::Brand, None)
            .unwrap();

        // 2. Add rule
        vm.add_text_asset(
            "maison_lux_time",
            "Placement Rule",
            VaultAssetKind::DesignRulebook,
            "Never stretch the logo.",
            vec![VaultUsage::PromptMaker],
            vec!["rule".to_string()],
            serde_json::json!({}),
        )
        .unwrap();

        // 3. Add negative prompt
        vm.add_text_asset(
            "maison_lux_time",
            "Muted Negative",
            VaultAssetKind::NegativePrompt,
            "no bright neon",
            vec![VaultUsage::PromptMaker],
            vec!["neg".to_string()],
            serde_json::json!({}),
        )
        .unwrap();

        // 4. Add restricted logo asset with allowed/disallowed providers
        let mock_file = test_dir.join("logo.png");
        fs::write(&mock_file, "binary").unwrap();
        vm.add_file_asset(
            "maison_lux_time",
            "Watermark Logo",
            VaultAssetKind::Logo,
            &mock_file,
            vec![VaultUsage::GenerateImage],
            vec!["logo".to_string()],
            serde_json::json!({
                "restricted": true,
                "allowed_providers": ["mock", "internal"],
                "disallowed_providers": ["public-image-api"]
            }),
        )
        .unwrap();

        // 5. Setup project and attach
        let project_dir = test_dir.join("my_proj");
        fs::create_dir_all(&project_dir).unwrap();
        vm.attach_vault(&project_dir, "maison_lux_time", "brand").unwrap();

        // 6. Compile PromptContext
        let prompt_context = vm.compile_prompt_context(&project_dir).unwrap();
        assert_eq!(prompt_context.vault_context.len(), 1);

        // 7. Test prompt maker enrichment
        let maker = RuleBasedPromptMaker;
        let pm_ctx = PromptMakerContext {
            project_summary: Some("Lux watch campaign".to_string()),
            locale: Some("en".to_string()),
            style_hint: Some("gold metallic".to_string()),
            vault_context: Some(prompt_context.clone()),
            target_model_id: None,
            explicit_options: serde_json::json!({}),
        };

        let enriched = maker
            .make_prompt(GenerationKind::Image, "a watch on desk", &pm_ctx)
            .unwrap();

        println!("DEBUG PROMPT IS: {}", enriched.professional_prompt);
        assert!(enriched.professional_prompt.contains("[AI Generation Mode: Image] a watch on desk"));
        assert!(enriched.professional_prompt.contains("[Rules: Never stretch the logo.]"));
        assert!(enriched.negative_prompt.unwrap().contains("no bright neon"));

        // 8. Test runtime provider restrictions checking
        let mut generator = DefaultGenerationRuntime::mock(test_dir.join("outputs"));

        // Safe provider: "mock" (is allowed)
        let val_context_json = serde_json::to_value(&prompt_context).unwrap();
        let options = serde_json::json!({
            "vault_context": val_context_json
        });

        let req_ok = GenerationRequest {
            job_ref: "@g1".parse().unwrap(),
            kind: GenerationKind::Image,
            user_request: "a watch on desk".to_string(),
            model: Some("mock/image".to_string()),
            inputs: vec![],
            options: options.clone(),
        };
        let res_ok = generator.run_to_completion(req_ok);
        assert!(res_ok.is_ok(), "Should allow 'mock' provider since it is explicitly allowed");

        // Unsafe provider: let's resolve to a provider not allowed or disallowed
        // Let's register a model with provider "public-image-api" which is explicitly disallowed!
        let disallowed_model = ProviderModel {
            id: "unsafe/image".to_string(),
            provider: "public-image-api".to_string(),
            kind: GenerationKind::Image,
            enabled: true,
            capabilities: serde_json::json!({}),
        };
        generator.runtime.model_registry.models.push(disallowed_model);

        let req_fail = GenerationRequest {
            job_ref: "@g2".parse().unwrap(),
            kind: GenerationKind::Image,
            user_request: "a watch on desk".to_string(),
            model: Some("unsafe/image".to_string()),
            inputs: vec![],
            options,
        };
        let res_fail = generator.run_to_completion(req_fail);
        assert!(res_fail.is_err(), "Should disallow unsafe provider 'public-image-api'");
        let err_msg = format!("{}", res_fail.unwrap_err());
        assert!(err_msg.contains("Security violation: Vault asset 'Watermark Logo' is restricted"));
    }
}

