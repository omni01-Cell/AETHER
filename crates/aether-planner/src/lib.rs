use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use uuid::Uuid;

use aether_core::{
    AetherError, AetherPlan, AetherPlanStep, PlanEvidence, PlanStatus, PlanStepStatus, Ref, RefKind,
};
use aether_project::ProjectManager;

/// Appel LLM planner (`agents.v1.json` → bridge TS). Activé via `AETHER_PLANNER_LLM=1`.
pub use aether_generate::agents::{planner_llm_enabled, PlannerCall};

pub struct PlannerManager {
    project_dir: PathBuf,
}

impl PlannerManager {
    /// Initialize PlannerManager for a specific project directory.
    pub fn new(project_dir: PathBuf) -> Self {
        PlannerManager { project_dir }
    }

    /// Load PlannerManager using the current active project directory from ProjectManager.
    pub fn load_active() -> Result<Self, AetherError> {
        let pm = ProjectManager::load_default()?;
        let active_dir = pm.resolve_for_command(None).map_err(|_| {
            AetherError::OperationFailed("No active AETHER project found".to_string())
        })?;
        Ok(PlannerManager::new(active_dir))
    }

    /// Resolves the path to the plan file inside the project directory.
    fn plan_path(&self, plan_id: &str) -> PathBuf {
        self.project_dir
            .join(".aether")
            .join("plans")
            .join(format!("{}.json", plan_id))
    }

    /// Helper to ensure the plans directory exists.
    fn ensure_plans_dir(&self) -> Result<(), AetherError> {
        let path = self.project_dir.join(".aether").join("plans");
        if !path.exists() {
            fs::create_dir_all(&path).map_err(|e| {
                AetherError::IoError(path.to_string_lossy().to_string(), e.to_string())
            })?;
        }
        Ok(())
    }

    /// Creates a new AETHER Plan.
    /// - If `plan_json` is provided, parses it directly.
    /// - Otherwise, generates a structured template customized with attached Vault constraints.
    pub fn create_plan(
        &self,
        objective: &str,
        plan_json: Option<&str>,
    ) -> Result<AetherPlan, AetherError> {
        self.ensure_plans_dir()?;

        let plan = if let Some(json_content) = plan_json {
            let mut p: AetherPlan = serde_json::from_str(json_content).map_err(|e| {
                AetherError::OperationFailed(format!("Failed to parse plan JSON: {}", e))
            })?;
            p.plan_id = Uuid::new_v4().to_string();
            p
        } else {
            let mut assumptions = vec![
                "Implicit sandbox isolation active".to_string(),
                "Mock generators enabled by default".to_string(),
            ];

            // Integrate AETHER Vault constraints
            if let Ok(vault_mgr) = aether_vault::VaultManager::load_default() {
                if let Ok(links) = aether_vault::VaultManager::load_project_links(&self.project_dir)
                {
                    for link in links.attached_vaults {
                        assumptions.push(format!(
                            "Attached Vault constraint: '{}' linked as '{}'",
                            link.vault_id, link.alias
                        ));
                        if let Ok(assets) = vault_mgr.load_assets(&link.vault_id) {
                            for asset in assets {
                                if asset.kind == aether_core::VaultAssetKind::DesignRulebook {
                                    if let Some(txt) =
                                        asset.metadata.get("text").and_then(|t| t.as_str())
                                    {
                                        assumptions.push(format!(
                                            "Design rule from vault ({}): {}",
                                            link.alias, txt
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let steps = vec![
                AetherPlanStep {
                    id: "S1".to_string(),
                    title: "Initialize Project Canvas".to_string(),
                    status: PlanStepStatus::Ready, // S1 has no dependencies, so it is Ready
                    command: "init 30.0 1920x1080 srgb".to_string(),
                    creates: vec![],
                    depends_on: vec![],
                    validation: "snapshot settings fps=30.0 width=1920 height=1080".to_string(),
                    evidence: None,
                },
                AetherPlanStep {
                    id: "S2".to_string(),
                    title: "Generate Storyboard Scratch".to_string(),
                    status: PlanStepStatus::Pending,
                    command: format!("generate storyboard-scratch \"Commercial sequence for {}\"", objective),
                    creates: vec![RefKind::Generated],
                    depends_on: vec!["S1".to_string()],
                    validation: "inspect @g1".to_string(),
                    evidence: None,
                },
                AetherPlanStep {
                    id: "S3".to_string(),
                    title: "Generate Campaign Voiceover".to_string(),
                    status: PlanStepStatus::Pending,
                    command: format!("generate voice \"Voiceover for {}\"", objective),
                    creates: vec![RefKind::Audio],
                    depends_on: vec!["S2".to_string()],
                    validation: "inspect @a1".to_string(),
                    evidence: None,
                },
                AetherPlanStep {
                    id: "S4".to_string(),
                    title: "Generate Cinematic Music".to_string(),
                    status: PlanStepStatus::Pending,
                    command: format!("generate music \"Cinematic ambiance for {}\"", objective),
                    creates: vec![RefKind::Audio],
                    depends_on: vec!["S2".to_string()],
                    validation: "inspect @a2".to_string(),
                    evidence: None,
                },
                AetherPlanStep {
                    id: "S5".to_string(),
                    title: "Compile Video from ingredients".to_string(),
                    status: PlanStepStatus::Pending,
                    command: "generate video-ingredients @g1 @a1 @a2 --prompt \"Compile high resolution final cut\"".to_string(),
                    creates: vec![RefKind::Video],
                    depends_on: vec!["S3".to_string(), "S4".to_string()],
                    validation: "inspect @v1".to_string(),
                    evidence: None,
                },
                AetherPlanStep {
                    id: "S6".to_string(),
                    title: "Export Final Render".to_string(),
                    status: PlanStepStatus::Pending,
                    command: "export @v1 mp4".to_string(),
                    creates: vec![],
                    depends_on: vec!["S5".to_string()],
                    validation: "export output file check".to_string(),
                    evidence: None,
                },
            ];

            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;

            AetherPlan {
                plan_id: Uuid::new_v4().to_string(),
                objective: objective.to_string(),
                status: PlanStatus::Pending,
                assumptions,
                required_capabilities: vec![
                    "init".to_string(),
                    "generate".to_string(),
                    "inspect".to_string(),
                    "export".to_string(),
                ],
                steps,
                validation_plan: vec![
                    "Verify S1 canvas configurations".to_string(),
                    "Inspect generated storyboard @g1".to_string(),
                    "Inspect compiled video @v1 prior to rendering".to_string(),
                ],
                risks: vec!["Generation latency for video rendering".to_string()],
                created_at_ms: now,
                updated_at_ms: now,
            }
        };

        self.save_plan(&plan)?;
        Ok(plan)
    }

    /// Load a plan from project storage.
    pub fn get_plan(&self, plan_id: &str) -> Result<AetherPlan, AetherError> {
        let path = self.plan_path(plan_id);
        if !path.exists() {
            return Err(AetherError::OperationFailed(format!(
                "Plan '{}' not found in this project",
                plan_id
            )));
        }
        let content = fs::read_to_string(&path)
            .map_err(|e| AetherError::IoError(path.to_string_lossy().to_string(), e.to_string()))?;
        serde_json::from_str(&content).map_err(|e| {
            AetherError::OperationFailed(format!("Failed to deserialize plan JSON: {}", e))
        })
    }

    /// Save a plan to project storage.
    pub fn save_plan(&self, plan: &AetherPlan) -> Result<(), AetherError> {
        self.ensure_plans_dir()?;
        let path = self.plan_path(&plan.plan_id);
        let content = serde_json::to_string_pretty(plan).map_err(|e| {
            AetherError::OperationFailed(format!("Failed to serialize plan: {}", e))
        })?;
        fs::write(&path, content)
            .map_err(|e| AetherError::IoError(path.to_string_lossy().to_string(), e.to_string()))?;
        Ok(())
    }

    /// Revise a plan with custom instructions.
    /// Updates command parameters deterministically if instructions contain specific hints.
    pub fn revise_plan(&self, plan_id: &str, instruction: &str) -> Result<AetherPlan, AetherError> {
        let mut plan = self.get_plan(plan_id)?;

        plan.assumptions
            .push(format!("Revision Instruction: {}", instruction));
        plan.updated_at_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        plan.status = PlanStatus::NeedsRevision;

        // Apply changes based on hints
        if instruction.contains("24fps") || instruction.contains("24 fps") {
            if let Some(s1) = plan.steps.iter_mut().find(|s| s.id == "S1") {
                s1.command = "init 24.0 1920x1080 srgb".to_string();
                s1.validation = "snapshot settings fps=24.0 width=1920 height=1080".to_string();
            }
            plan.status = PlanStatus::Running; // Reset to running after revision applied
        } else if instruction.contains("vertical") || instruction.contains("9:16") {
            if let Some(s1) = plan.steps.iter_mut().find(|s| s.id == "S1") {
                s1.command = "init 30.0 1080x1920 srgb".to_string();
                s1.validation = "snapshot settings fps=30.0 width=1080 height=1920".to_string();
            }
            plan.status = PlanStatus::Running;
        }

        self.save_plan(&plan)?;
        Ok(plan)
    }

    /// Check/Mark a step as done and supply optional evidence reference.
    /// Automatically propagates state changes to make downstream steps `Ready`.
    pub fn check_step(
        &self,
        plan_id: &str,
        step_id: &str,
        evidence_ref: Option<Ref>,
    ) -> Result<AetherPlan, AetherError> {
        let mut plan = self.get_plan(plan_id)?;

        // Find target step
        let step_idx = plan
            .steps
            .iter()
            .position(|s| s.id == step_id)
            .ok_or_else(|| {
                AetherError::OperationFailed(format!("Step '{}' not found in plan", step_id))
            })?;

        // 1. Verify dependencies are complete
        for dep in &plan.steps[step_idx].depends_on {
            if let Some(dep_step) = plan.steps.iter().find(|s| s.id == *dep) {
                if dep_step.status != PlanStepStatus::Done {
                    return Err(AetherError::OperationFailed(format!(
                        "Cannot check step '{}': Dependency '{}' is not complete",
                        step_id, dep
                    )));
                }
            }
        }

        // 2. Set step status to Done and add evidence
        plan.steps[step_idx].status = PlanStepStatus::Done;
        plan.steps[step_idx].evidence = Some(PlanEvidence {
            step_id: step_id.to_string(),
            evidence_ref,
            comment: Some("Completed via AETHER Planner CLI".to_string()),
        });

        // 3. Propagate states: find other pending steps whose dependencies are now fully completed
        loop {
            let mut changed = false;
            for i in 0..plan.steps.len() {
                if plan.steps[i].status == PlanStepStatus::Pending {
                    let all_deps_done = plan.steps[i].depends_on.iter().all(|dep_id| {
                        plan.steps
                            .iter()
                            .find(|s| s.id == *dep_id)
                            .map(|s| s.status == PlanStepStatus::Done)
                            .unwrap_or(false)
                    });

                    if all_deps_done {
                        plan.steps[i].status = PlanStepStatus::Ready;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        // 4. Update overall plan status
        let all_done = plan.steps.iter().all(|s| s.status == PlanStepStatus::Done);
        plan.status = if all_done {
            PlanStatus::Done
        } else {
            PlanStatus::Running
        };

        plan.updated_at_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        self.save_plan(&plan)?;
        Ok(plan)
    }

    /// Uncheck/Reset a step to Pending.
    /// Cascades downward: resets any steps that directly or indirectly depend on this step.
    pub fn uncheck_step(&self, plan_id: &str, step_id: &str) -> Result<AetherPlan, AetherError> {
        let mut plan = self.get_plan(plan_id)?;

        // Recursive reset list
        let mut to_reset = std::collections::HashSet::new();
        to_reset.insert(step_id.to_string());

        loop {
            let mut added = false;
            for step in &plan.steps {
                if !to_reset.contains(&step.id) {
                    // If this step depends on any step slated to be reset, it must be reset too
                    let depends_on_reset = step.depends_on.iter().any(|dep| to_reset.contains(dep));
                    if depends_on_reset {
                        to_reset.insert(step.id.clone());
                        added = true;
                    }
                }
            }
            if !added {
                break;
            }
        }

        // Reset all selected steps
        for step in &mut plan.steps {
            if to_reset.contains(&step.id) {
                step.status = PlanStepStatus::Pending;
                step.evidence = None;
            }
        }

        // Evaluate if any step has become ready again (its dependencies are Done)
        for i in 0..plan.steps.len() {
            if plan.steps[i].status == PlanStepStatus::Pending {
                let all_deps_done = plan.steps[i].depends_on.iter().all(|dep_id| {
                    plan.steps
                        .iter()
                        .find(|s| s.id == *dep_id)
                        .map(|s| s.status == PlanStepStatus::Done)
                        .unwrap_or(true) // No deps means Done
                });
                if all_deps_done {
                    plan.steps[i].status = PlanStepStatus::Ready;
                }
            }
        }

        plan.status = PlanStatus::Running;
        plan.updated_at_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        self.save_plan(&plan)?;
        Ok(plan)
    }

    /// Returns the next ready step that can be executed.
    pub fn next_step(&self, plan_id: &str) -> Result<Option<AetherPlanStep>, AetherError> {
        let plan = self.get_plan(plan_id)?;
        let next = plan
            .steps
            .iter()
            .find(|s| s.status == PlanStepStatus::Ready)
            .cloned();
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_project(test_name: &str) -> (PathBuf, PlannerManager) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let test_dir = workspace_root
            .join("target")
            .join("test_projects")
            .join(test_name);

        if test_dir.exists() {
            let _ = fs::remove_dir_all(&test_dir);
        }
        fs::create_dir_all(&test_dir).unwrap();

        (test_dir.clone(), PlannerManager::new(test_dir))
    }

    #[test]
    fn test_planner_lifecycle() {
        let (_proj_dir, planner) = setup_test_project("test_planner_lifecycle");

        // 1. Create a plan
        let plan = planner
            .create_plan("Promote AETHER Multimedia Engine", None)
            .unwrap();

        assert_eq!(plan.status, PlanStatus::Pending);
        assert_eq!(plan.steps.len(), 6);
        assert_eq!(plan.steps[0].id, "S1");
        assert_eq!(plan.steps[0].status, PlanStepStatus::Ready);
        assert_eq!(plan.steps[1].status, PlanStepStatus::Pending);

        // Load plan
        let loaded = planner.get_plan(&plan.plan_id).unwrap();
        assert_eq!(loaded.plan_id, plan.plan_id);

        // 2. Check S1 (Ready) -> S2 should become Ready
        let evidence_ref = "@v1".parse::<Ref>().ok();
        let plan = planner
            .check_step(&plan.plan_id, "S1", evidence_ref)
            .unwrap();
        assert_eq!(plan.steps[0].status, PlanStepStatus::Done);
        assert_eq!(plan.steps[1].status, PlanStepStatus::Ready); // Propagated S1 -> S2

        // Checking S3 should fail because S2 is not Done
        let res = planner.check_step(&plan.plan_id, "S3", None);
        assert!(res.is_err());

        // Check S2 -> S3 and S4 should become Ready
        let plan = planner.check_step(&plan.plan_id, "S2", None).unwrap();
        assert_eq!(plan.steps[1].status, PlanStepStatus::Done);
        assert_eq!(plan.steps[2].status, PlanStepStatus::Ready);
        assert_eq!(plan.steps[3].status, PlanStepStatus::Ready);

        // 3. Uncheck S2 -> should cascade and reset S2, S3, S4 back to Pending
        let plan = planner.uncheck_step(&plan.plan_id, "S2").unwrap();
        assert_eq!(plan.steps[1].status, PlanStepStatus::Ready); // S2 becomes ready again because S1 is Done
        assert_eq!(plan.steps[2].status, PlanStepStatus::Pending);
        assert_eq!(plan.steps[3].status, PlanStepStatus::Pending);

        // 4. Revise plan with hint
        let plan = planner
            .revise_plan(&plan.plan_id, "Please use 24fps vertical video")
            .unwrap();
        assert_eq!(
            plan.steps[0].command,
            "init 24.0 1920x1080 srgb".to_string()
        );
    }
}
