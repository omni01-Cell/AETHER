use aether_core::{PlanStatus, PlanStepStatus, Ref, VaultAssetKind, VaultKind, VaultUsage};
use aether_planner::PlannerManager;
use aether_project::{ProjectCreateSpec, ProjectManager};
use aether_vault::VaultManager;
use std::fs;
use std::path::PathBuf;

fn setup_test_project(test_name: &str) -> (PathBuf, PathBuf) {
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

    let mock_home = test_dir.join("mock_home");
    fs::create_dir_all(&mock_home).unwrap();
    std::env::set_var("HOME", &mock_home);

    (test_dir, mock_home)
}

#[test]
fn test_planner_full_lifecycle_e2e() {
    let (test_dir, _mock_home) = setup_test_project("test_planner_full_lifecycle_e2e");

    // 1. Create a project
    let pm = ProjectManager::load_default().expect("Failed to load ProjectManager");
    let proj_dir = test_dir.join("my_creative_project");
    let spec = ProjectCreateSpec {
        name: "Marketing Video Teaser".to_string(),
        dir: Some(proj_dir.clone()),
        adopt: false,
        force: false,
    };
    let project = pm.create(spec).expect("Failed to create project");
    assert_eq!(project.name, "Marketing Video Teaser");

    // Verify it is the current active project
    let current_proj = pm
        .current()
        .expect("Failed to get current project")
        .unwrap();
    assert_eq!(current_proj.project_id, project.project_id);

    // 2. Create a Vault
    let vm = VaultManager::load_default().expect("Failed to load VaultManager");
    let vault = vm
        .create_vault(
            "Teaser Brand Kit",
            VaultKind::Brand,
            Some("Teaser Campaign assets & design rules".to_string()),
        )
        .expect("Failed to create vault");
    assert_eq!(vault.vault_id, "teaser_brand_kit");

    // 3. Add brand constraints to the Vault
    let text_rule = "Mandatory: Always include mint-green accent color #00FFCC in video assets.";
    let rule_asset = vm
        .add_text_asset(
            &vault.vault_id,
            "Mint Accent Color Rule",
            VaultAssetKind::DesignRulebook,
            text_rule,
            vec![VaultUsage::Storyboard, VaultUsage::PromptMaker],
            vec!["branding".to_string(), "color".to_string()],
            serde_json::json!({}),
        )
        .expect("Failed to add rule to vault");
    assert_eq!(rule_asset.name, "Mint Accent Color Rule");

    // 4. Attach the Vault to the project
    vm.attach_vault(&proj_dir, &vault.vault_id, "brand_kit")
        .expect("Failed to attach vault to project");

    // Verify the links are correct
    let links = VaultManager::load_project_links(&proj_dir).expect("Failed to load project links");
    assert_eq!(links.attached_vaults.len(), 1);
    assert_eq!(links.attached_vaults[0].vault_id, "teaser_brand_kit");
    assert_eq!(links.attached_vaults[0].alias, "brand_kit");

    // 5. Initialize the Planner and generate a Plan
    let planner = PlannerManager::load_active().expect("Failed to load active planner");
    let plan = planner
        .create_plan("Teaser Promo for AETHER", None)
        .expect("Failed to create plan");

    assert_eq!(plan.status, PlanStatus::Pending);
    assert_eq!(plan.objective, "Teaser Promo for AETHER");
    assert_eq!(plan.steps.len(), 6);

    // Check that vault constraints were injected into plan assumptions
    let has_vault_assumption = plan.assumptions.iter().any(|assumption| {
        assumption.contains("teaser_brand_kit") && assumption.contains("brand_kit")
    });
    assert!(
        has_vault_assumption,
        "Vault metadata links should be part of plan assumptions"
    );

    let has_rule_assumption = plan
        .assumptions
        .iter()
        .any(|assumption| assumption.contains("Mandatory: Always include mint-green accent color"));
    assert!(
        has_rule_assumption,
        "Brand rules from vault should be injected into plan assumptions"
    );

    // 6. Check step states sequentially
    // Step S1 has no dependencies -> should be Ready
    assert_eq!(plan.steps[0].id, "S1");
    assert_eq!(plan.steps[0].status, PlanStepStatus::Ready);

    // All downstream steps should be Pending
    for step in &plan.steps[1..] {
        assert_eq!(step.status, PlanStepStatus::Pending);
    }

    // Checking S2 before S1 is completed should fail
    let check_s2_fail = planner.check_step(&plan.plan_id, "S2", None);
    assert!(check_s2_fail.is_err());

    // Complete S1 (Ready) -> S2 should transition to Ready
    let evidence_r1 = "@v1".parse::<Ref>().ok();
    let plan = planner
        .check_step(&plan.plan_id, "S1", evidence_r1)
        .expect("Failed to check step S1");
    assert_eq!(plan.steps[0].status, PlanStepStatus::Done);
    assert_eq!(plan.steps[1].status, PlanStepStatus::Ready);
    assert_eq!(plan.steps[2].status, PlanStepStatus::Pending);

    // Complete S2 -> S3 and S4 should transition to Ready (parallel execution branches!)
    let plan = planner
        .check_step(&plan.plan_id, "S2", None)
        .expect("Failed to check step S2");
    assert_eq!(plan.steps[1].status, PlanStepStatus::Done);
    assert_eq!(plan.steps[2].status, PlanStepStatus::Ready); // S3 Ready
    assert_eq!(plan.steps[3].status, PlanStepStatus::Ready); // S4 Ready
    assert_eq!(plan.steps[4].status, PlanStepStatus::Pending); // S5 is Pending (depends on S3, S4)

    // Complete S3 -> S5 is still Pending because S4 is not complete
    let plan = planner
        .check_step(&plan.plan_id, "S3", None)
        .expect("Failed to check step S3");
    assert_eq!(plan.steps[2].status, PlanStepStatus::Done);
    assert_eq!(plan.steps[4].status, PlanStepStatus::Pending);

    // Complete S4 -> S5 transitions to Ready since all dependencies (S3 & S4) are now Done
    let plan = planner
        .check_step(&plan.plan_id, "S4", None)
        .expect("Failed to check step S4");
    assert_eq!(plan.steps[3].status, PlanStepStatus::Done);
    assert_eq!(plan.steps[4].status, PlanStepStatus::Ready);

    // 7. Test revise_plan functionality
    let plan = planner
        .revise_plan(
            &plan.plan_id,
            "Revise frame: Please use 24fps vertical video",
        )
        .expect("Failed to revise plan");

    // S1 command parameters should be revised accordingly
    assert_eq!(plan.steps[0].command, "init 24.0 1920x1080 srgb");
    assert!(plan.steps[0].validation.contains("fps=24.0"));

    // 8. Test cascading uncheck (resetting a parent step cascades down to children)
    // Currently, S1, S2, S3, S4 are Done, S5 is Ready, S6 is Pending.
    // If we uncheck S2:
    let plan = planner
        .uncheck_step(&plan.plan_id, "S2")
        .expect("Failed to uncheck step S2");

    // S2 transitions back to Ready (dependencies: S1 is still Done)
    assert_eq!(plan.steps[1].status, PlanStepStatus::Ready);

    // Downstream steps S3, S4, S5, S6 must transition back to Pending
    assert_eq!(plan.steps[2].status, PlanStepStatus::Pending);
    assert_eq!(plan.steps[3].status, PlanStepStatus::Pending);
    assert_eq!(plan.steps[4].status, PlanStepStatus::Pending);
    assert_eq!(plan.steps[5].status, PlanStepStatus::Pending);

    // Evidence for unchecked steps must be cleared
    assert!(plan.steps[1].evidence.is_none());
    assert!(plan.steps[2].evidence.is_none());

    println!("E2E Planner Lifecycle validation succeeded cleanly!");
}
