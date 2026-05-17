use std::fs;
use std::path::PathBuf;
use aether_core::Command;
use aether_daemon::SessionManager;

fn temp_project_dir() -> PathBuf {
    let unique_dir = format!("test_e2e_project_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos());
    std::env::temp_dir().join(unique_dir)
}

#[test]
fn test_e2e_full_scenario_and_crash_resilience() {
    let dir = temp_project_dir();

    // 1. Create first session (simulates the running daemon)
    {
        let session = SessionManager::new(&dir).unwrap();

        // Initialize project
        let cmd_init = Command::Init {
            fps: Some(30.0),
            resolution: Some("1920x1080".to_string()),
            colorspace: Some("srgb".to_string()),
        };
        let res = session.execute(cmd_init).unwrap();
        assert!(res.success);

        // Create canvas asset
        let cmd_canvas = Command::Canvas {
            width: 300,
            height: 200,
            color: "blue".to_string(),
        };
        let res_canvas = session.execute(cmd_canvas).unwrap();
        assert!(res_canvas.success);
        let canvas_ref = res_canvas.affected_ref.unwrap();

        // Draw text on the canvas
        let cmd_text = Command::DrawText {
            r: canvas_ref,
            text: "Aether Engine E2E".to_string(),
            font: "LiberationSans-Regular".to_string(),
            size: 20.0,
            x: 20,
            y: 20,
        };
        let res_text = session.execute(cmd_text).unwrap();
        assert!(res_text.success);
        let text_ref = res_text.affected_ref.unwrap();

        // Verify active registry and SQLite state
        let snap = session.get_snapshot().unwrap();
        assert_eq!(snap.settings.fps, 30.0);
        assert_eq!(snap.settings.width, 1920);
        assert_eq!(snap.settings.colorspace, "srgb");
        assert_eq!(snap.assets.len(), 2);
        assert_eq!(snap.history_len, 3);
        assert_eq!(snap.history_cursor, 3);

        // Undo DrawText command
        let res_undo = session.execute(Command::Undo).unwrap();
        assert!(res_undo.success);
        let snap_undo = session.get_snapshot().unwrap();
        assert_eq!(snap_undo.assets.len(), 1); // Only Canvas should be active
        assert_eq!(snap_undo.history_cursor, 2);

        // Verify that the drawn text asset is no longer resolved in active registry
        assert!(session.get_snapshot().unwrap().assets.iter().any(|a| a.r == canvas_ref));
        assert!(!session.get_snapshot().unwrap().assets.iter().any(|a| a.r == text_ref));

        // Redo DrawText command
        let res_redo = session.execute(Command::Redo).unwrap();
        assert!(res_redo.success);
        let snap_redo = session.get_snapshot().unwrap();
        assert_eq!(snap_redo.assets.len(), 2); // Both should be active again
        assert_eq!(snap_redo.history_cursor, 3);

        // Session drops here, simulating a shutdown or crash.
    }

    // 2. Re-open session in the same directory (simulates daemon restart after crash/restart)
    {
        println!("Restarting SessionManager to test crash resilience and SQLite reconstruction...");
        let session = SessionManager::new(&dir).unwrap();

        // Verify state was reconstructed correctly from SQLite DB & memory sync!
        let snap = session.get_snapshot().unwrap();
        assert_eq!(snap.settings.fps, 30.0);
        assert_eq!(snap.settings.width, 1920);
        assert_eq!(snap.settings.colorspace, "srgb");
        assert_eq!(snap.assets.len(), 2);
        assert_eq!(snap.history_len, 3);
        assert_eq!(snap.history_cursor, 3);

        println!("Crash resilience test passed. Reconstructed AETHER state matches perfectly!");
    }

    let _ = fs::remove_dir_all(&dir);
}
