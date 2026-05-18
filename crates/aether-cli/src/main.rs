use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use serde_json;
use aether_core::{Command, CommandResult, Snapshot};
use aether_project::{ProjectManager, ProjectCreateSpec, DeleteMode};

fn tokenize(line: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            in_quotes = !in_quotes;
        } else if c.is_whitespace() && !in_quotes {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
    }

    if in_quotes {
        return Err("Unclosed double quote".to_string());
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

pub fn parse_dsl(line: &str) -> Result<Command, String> {
    let mut tokens = tokenize(line)?;
    if tokens.is_empty() {
        return Err("Empty command".to_string());
    }

    // Pre-process two-word command tokens into canonical single tokens
    if tokens.len() >= 2 {
        let first = tokens[0].to_lowercase();
        let second = tokens[1].to_lowercase();
        match (first.as_str(), second.as_str()) {
            ("generate", "storyboard-scratch") | ("generate", "storyboard_scratch") => {
                tokens[0] = "generate_storyboard_scratch".to_string();
                tokens.remove(1);
            }
            ("generate", "dialogue") => {
                tokens[0] = "generate_dialogue".to_string();
                tokens.remove(1);
            }
            ("generate", "image") => {
                tokens[0] = "generate_image".to_string();
                tokens.remove(1);
            }
            ("edit", "image") => {
                tokens[0] = "edit_image".to_string();
                tokens.remove(1);
            }
            ("generate", "voice") => {
                tokens[0] = "generate_voice".to_string();
                tokens.remove(1);
            }
            ("clone", "voice") => {
                tokens[0] = "clone_voice".to_string();
                tokens.remove(1);
            }
            ("generate", "scene-audio") | ("generate", "scene_audio") => {
                tokens[0] = "generate_scene_audio".to_string();
                tokens.remove(1);
            }
            ("generate", "music") => {
                tokens[0] = "generate_music".to_string();
                tokens.remove(1);
            }
            ("generate", "video-text") | ("generate", "video_text") | ("generate", "video") => {
                tokens[0] = "generate_video".to_string();
                tokens.remove(1);
            }
            ("generate", "video-frame") | ("generate", "video_frame") => {
                tokens[0] = "generate_video_frame".to_string();
                tokens.remove(1);
            }
            ("generate", "video-ingredients") | ("generate", "video_ingredients") => {
                tokens[0] = "generate_video_ingredients".to_string();
                tokens.remove(1);
            }
            ("edit", "video") => {
                tokens[0] = "edit_video".to_string();
                tokens.remove(1);
            }
            ("generation", "status") => {
                tokens[0] = "generation_status".to_string();
                tokens.remove(1);
            }
            ("generation", "cancel") => {
                tokens[0] = "cancel_generation".to_string();
                tokens.remove(1);
            }
            ("project", "create") => {
                tokens[0] = "project_create".to_string();
                tokens.remove(1);
            }
            ("project", "open") => {
                tokens[0] = "project_open".to_string();
                tokens.remove(1);
            }
            ("project", "current") => {
                tokens[0] = "project_current".to_string();
                tokens.remove(1);
            }
            ("project", "close") => {
                tokens[0] = "project_close".to_string();
                tokens.remove(1);
            }
            ("project", "list") => {
                tokens[0] = "project_list".to_string();
                tokens.remove(1);
            }
            ("project", "delete") => {
                tokens[0] = "project_delete".to_string();
                tokens.remove(1);
            }
            _ => {}
        }
    }

    let cmd_name = tokens[0].to_lowercase();
    match cmd_name.as_str() {
        "init" => {
            let fps = tokens.get(1).and_then(|s| s.parse::<f32>().ok());
            let resolution = tokens.get(2).cloned();
            let colorspace = tokens.get(3).cloned();
            Ok(Command::Init { fps, resolution, colorspace })
        }
        "import" => {
            let path = tokens.get(1).ok_or("Missing path for import")?.clone();
            Ok(Command::Import { path })
        }
        "trim" => {
            let r_str = tokens.get(1).ok_or("Missing reference for trim")?;
            let r = r_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            let start = tokens.get(2).ok_or("Missing start time for trim")?.clone();
            let end = tokens.get(3).ok_or("Missing end time for trim")?.clone();
            Ok(Command::Trim { r, start, end })
        }
        "mix" => {
            let r_str = tokens.get(1).ok_or("Missing reference for mix")?;
            let r = r_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            let volume = tokens.get(2).ok_or("Missing volume for mix")?
                .parse::<f32>().map_err(|_| "Invalid volume float".to_string())?;
            Ok(Command::Mix { r, volume })
        }
        "composite" => {
            let base_str = tokens.get(1).ok_or("Missing base reference")?;
            let base = base_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            let overlay_str = tokens.get(2).ok_or("Missing overlay reference")?;
            let overlay = overlay_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            let at = tokens.get(3).ok_or("Missing timestamp")?.clone();
            let x = tokens.get(4).ok_or("Missing x coordinate")?
                .parse::<i32>().map_err(|_| "Invalid x".to_string())?;
            let y = tokens.get(5).ok_or("Missing y coordinate")?
                .parse::<i32>().map_err(|_| "Invalid y".to_string())?;
            Ok(Command::Composite { base, overlay, at, x, y })
        }
        "canvas" => {
            let width = tokens.get(1).ok_or("Missing width")?
                .parse::<u32>().map_err(|_| "Invalid width".to_string())?;
            let height = tokens.get(2).ok_or("Missing height")?
                .parse::<u32>().map_err(|_| "Invalid height".to_string())?;
            let color = tokens.get(3).ok_or("Missing color")?.clone();
            Ok(Command::Canvas { width, height, color })
        }
        "draw_text" => {
            let r_str = tokens.get(1).ok_or("Missing reference")?;
            let r = r_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            let text = tokens.get(2).ok_or("Missing text")?.clone();
            let font = tokens.get(3).ok_or("Missing font")?.clone();
            let size = tokens.get(4).ok_or("Missing size")?
                .parse::<f32>().map_err(|_| "Invalid size".to_string())?;
            let x = tokens.get(5).ok_or("Missing x")?
                .parse::<i32>().map_err(|_| "Invalid x".to_string())?;
            let y = tokens.get(6).ok_or("Missing y")?
                .parse::<i32>().map_err(|_| "Invalid y".to_string())?;
            Ok(Command::DrawText { r, text, font, size, x, y })
        }
        "export" => {
            let r_str = tokens.get(1).ok_or("Missing reference")?;
            let r = r_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            let format = tokens.get(2).ok_or("Missing format")?.clone();
            let codec = tokens.get(3).cloned().unwrap_or_else(|| "h264".to_string());
            let quality = tokens.get(4).cloned().unwrap_or_else(|| "high".to_string());
            Ok(Command::Export { r, format, codec, quality })
        }
        "inspect" => {
            let r_str = tokens.get(1).ok_or("Missing reference for inspect")?;
            let r = r_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            let start = tokens.get(2).cloned();
            let end = tokens.get(3).cloned();
            Ok(Command::Inspect { r: Some(r), start, end })
        }
        "generate_storyboard_scratch" | "storyboard_scratch" => {
            let request = tokens.get(1).ok_or("Missing request prompt")?.clone();
            let model = tokens.get(2).cloned();
            Ok(Command::GenerateStoryboardScratch { request, model, options: serde_json::json!({}) })
        }
        "generate_dialogue" | "dialogue" => {
            let request = tokens.get(1).ok_or("Missing request prompt")?.clone();
            let model = tokens.get(2).cloned();
            Ok(Command::GenerateDialogue { request, model, options: serde_json::json!({}) })
        }
        "generate_image" | "image" => {
            let request = tokens.get(1).ok_or("Missing request prompt")?.clone();
            let model = tokens.get(2).cloned();
            Ok(Command::GenerateImage { request, model, inputs: Vec::new(), options: serde_json::json!({}) })
        }
        "edit_image" => {
            let target_str = tokens.get(1).ok_or("Missing target image reference")?;
            let target = target_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            let request = tokens.get(2).ok_or("Missing edit request prompt")?.clone();
            let model = tokens.get(3).cloned();
            Ok(Command::EditImage { target, request, model, options: serde_json::json!({}) })
        }
        "generate_voice" | "voice" => {
            let text = tokens.get(1).ok_or("Missing text for voice generation")?.clone();
            let voice = tokens.get(2).cloned();
            let model = tokens.get(3).cloned();
            Ok(Command::GenerateVoice { text, voice, model, options: serde_json::json!({}) })
        }
        "clone_voice" => {
            let sample_str = tokens.get(1).ok_or("Missing voice sample reference")?;
            let sample = sample_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            let name = tokens.get(2).cloned();
            let model = tokens.get(3).cloned();
            Ok(Command::CloneVoice { sample, name, model, options: serde_json::json!({}) })
        }
        "generate_scene_audio" | "scene_audio" => {
            let request = tokens.get(1).ok_or("Missing scene audio description")?.clone();
            let model = tokens.get(2).cloned();
            Ok(Command::GenerateSceneAudio { request, model, options: serde_json::json!({}) })
        }
        "generate_music" | "music" => {
            let request = tokens.get(1).ok_or("Missing music description")?.clone();
            let model = tokens.get(2).cloned();
            Ok(Command::GenerateMusic { request, model, options: serde_json::json!({}) })
        }
        "generate_video" | "video" => {
            let request = tokens.get(1).ok_or("Missing video description")?.clone();
            let model = tokens.get(2).cloned();
            Ok(Command::GenerateVideoFromText { request, model, options: serde_json::json!({}) })
        }
        "generate_video_frame" | "video_frame" => {
            let frame_str = tokens.get(1).ok_or("Missing frame reference")?;
            let frame = frame_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            let request = tokens.get(2).ok_or("Missing animation prompt")?.clone();
            let model = tokens.get(3).cloned();
            Ok(Command::GenerateVideoFromFrame { frame, request, model, options: serde_json::json!({}) })
        }
        "generate_video_ingredients" => {
            // Find `--prompt`
            let prompt_idx = tokens.iter().position(|t| t == "--prompt")
                .ok_or("Missing --prompt flag for video ingredients generation")?;
            if prompt_idx < 1 {
                return Err("Missing input references for video ingredients".to_string());
            }
            let mut inputs = Vec::new();
            for token in &tokens[1..prompt_idx] {
                let r = token.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
                inputs.push(r);
            }
            let request = tokens.get(prompt_idx + 1).ok_or("Missing prompt request after --prompt")?.clone();
            let model = tokens.get(prompt_idx + 2).cloned();
            Ok(Command::GenerateVideoFromIngredients { inputs, request, model, options: serde_json::json!({}) })
        }
        "edit_video" => {
            let target_str = tokens.get(1).ok_or("Missing target video reference to edit")?;
            let target = target_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            let request = tokens.get(2).ok_or("Missing edit request prompt")?.clone();
            let model = tokens.get(3).cloned();
            Ok(Command::EditVideo { target, request, model, options: serde_json::json!({}) })
        }
        "generation_status" | "status" => {
            let r = match tokens.get(1) {
                Some(s) => Some(s.parse::<aether_core::Ref>().map_err(|e| e.to_string())?),
                None => None,
            };
            Ok(Command::GenerationStatus { r })
        }
        "cancel_generation" | "cancel" => {
            let r_str = tokens.get(1).ok_or("Missing generation reference to cancel")?;
            let r = r_str.parse::<aether_core::Ref>().map_err(|e| e.to_string())?;
            Ok(Command::CancelGeneration { r })
        }
        "undo" => Ok(Command::Undo),
        "redo" => Ok(Command::Redo),
        "snapshot" => Ok(Command::Snapshot),
        "project_create" => {
            let name = tokens.get(1).ok_or("Missing project name")?.clone();
            let mut dir = None;
            let mut adopt = false;
            let mut force = false;
            
            let mut i = 2;
            while i < tokens.len() {
                match tokens[i].as_str() {
                    "--dir" => {
                        let path_str = tokens.get(i + 1).ok_or("Missing value for --dir")?;
                        dir = Some(PathBuf::from(path_str));
                        i += 2;
                    }
                    "--adopt" => {
                        adopt = true;
                        i += 1;
                    }
                    "--force" => {
                        force = true;
                        i += 1;
                    }
                    other => return Err(format!("Unknown flag '{}' for project create", other)),
                }
            }
            Ok(Command::ProjectCreate { name, dir, adopt, force })
        }
        "project_open" => {
            let target = tokens.get(1).ok_or("Missing project name or path to open")?.clone();
            Ok(Command::ProjectOpen { target })
        }
        "project_current" => {
            Ok(Command::ProjectCurrent)
        }
        "project_close" => {
            let target = tokens.get(1).cloned();
            Ok(Command::ProjectClose { target })
        }
        "project_list" => {
            Ok(Command::ProjectList)
        }
        "project_delete" => {
            let target = tokens.get(1).ok_or("Missing project name or path to delete")?.clone();
            let mut force = false;
            let mut archive = false;
            
            let mut i = 2;
            while i < tokens.len() {
                match tokens[i].as_str() {
                    "--force" => {
                        force = true;
                        i += 1;
                    }
                    "--archive" => {
                        archive = true;
                        i += 1;
                    }
                    other => return Err(format!("Unknown flag '{}' for project delete", other)),
                }
            }
            Ok(Command::ProjectDelete { target, force, archive })
        }
        "shutdown" => {
            Ok(Command::Shutdown)
        }
        other => Err(format!("Unknown command '{}'", other)),
    }
}

async fn get_connection(project_dir: &Path, sock_path: &Path) -> Result<UnixStream, std::io::Error> {
    match UnixStream::connect(sock_path).await {
        Ok(stream) => Ok(stream),
        Err(_e) => {
            println!("Daemon not running. Auto-starting AETHER daemon for project '{}'...", project_dir.to_string_lossy());
            let daemon_binary = "cargo";
            let _child = std::process::Command::new(daemon_binary)
                .args(["run", "--bin", "aether-daemon", "--quiet", "--", project_dir.to_str().unwrap_or(".")])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            
            // Wait for socket file to be created
            for _ in 0..15 {
                tokio::time::sleep(Duration::from_millis(300)).await;
                if sock_path.exists() {
                    if let Ok(stream) = UnixStream::connect(sock_path).await {
                        return Ok(stream);
                    }
                }
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Timed out waiting for AETHER daemon to start",
            ))
        }
    }
}

async fn send_command(stream: &mut UnixStream, cmd: Command) -> Result<CommandResult, anyhow::Error> {
    let req_bytes = serde_json::to_vec(&cmd)?;
    stream.write_all(&req_bytes).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;

    let res: CommandResult = serde_json::from_str(&response_line)?;
    Ok(res)
}

fn print_snapshot(snap: &Snapshot) {
    println!("--- PROJECT SNAPSHOT ---");
    println!("Settings: fps={}, resolution={}x{}, colorspace={}",
        snap.settings.fps, snap.settings.width, snap.settings.height, snap.settings.colorspace
    );
    println!("History Cursor: {} / {}", snap.history_cursor, snap.history_len);
    println!("Registered Assets ({}):", snap.assets.len());
    for asset in &snap.assets {
        println!("  - {} [{:?}] path={}", asset.r, asset.kind, asset.path.to_string_lossy());
    }
    println!("Active/Completed Generation Jobs ({}):", snap.generation_jobs.len());
    for job in &snap.generation_jobs {
        println!("  - {} [{:?}] status={:?} model={:?}",
            job.job_ref, job.kind, job.status, job.resolved_model.as_ref().map(|m| &m.id)
        );
    }
}

async fn execute_local_command(cmd: Command) -> Result<(), Box<dyn std::error::Error>> {
    let pm = ProjectManager::load_default()?;
    match cmd {
        Command::ProjectCreate { name, dir, adopt, force } => {
            match pm.create(ProjectCreateSpec { name, dir, adopt, force }) {
                Ok(meta) => {
                    println!("\x1b[32mSuccess:\x1b[0m Created and activated project '{}'", meta.name);
                    println!("{}", serde_json::to_string_pretty(&meta)?);
                }
                Err(e) => eprintln!("\x1b[31mError:\x1b[0m Failed to create project: {}", e),
            }
        }
        Command::ProjectOpen { target } => {
            match pm.open(&target) {
                Ok(meta) => {
                    println!("\x1b[32mSuccess:\x1b[0m Opened and activated project '{}'", meta.name);
                    println!("{}", serde_json::to_string_pretty(&meta)?);
                }
                Err(e) => eprintln!("\x1b[31mError:\x1b[0m Failed to open project: {}", e),
            }
        }
        Command::ProjectCurrent => {
            match pm.current() {
                Ok(Some(meta)) => {
                    println!("Current active project: '{}'", meta.name);
                    println!("{}", serde_json::to_string_pretty(&meta)?);
                }
                Ok(None) => println!("No active project."),
                Err(e) => eprintln!("\x1b[31mError:\x1b[0m Failed to get current project: {}", e),
            }
        }
        Command::ProjectClose { target } => {
            match pm.close(target.as_deref()) {
                Ok(_) => println!("\x1b[32mSuccess:\x1b[0m Project closed."),
                Err(e) => eprintln!("\x1b[31mError:\x1b[0m Failed to close project: {}", e),
            }
        }
        Command::ProjectList => {
            match pm.list() {
                Ok(projects) => {
                    println!("Registered AETHER Projects:");
                    println!("{}", serde_json::to_string_pretty(&projects)?);
                }
                Err(e) => eprintln!("\x1b[31mError:\x1b[0m Failed to list projects: {}", e),
            }
        }
        Command::ProjectDelete { target, force, archive } => {
            // Stop daemon if running for this project!
            let resolved_dir = pm.resolve_for_command(Some(&target)).ok();
            if let Some(ref root) = resolved_dir {
                let sock_path = root.join(".aether/aether.sock");
                if sock_path.exists() {
                    if let Ok(mut stream) = UnixStream::connect(&sock_path).await {
                        println!("Project daemon is running. Sending Shutdown command...");
                        let _ = send_command(&mut stream, Command::Shutdown).await;
                    }
                }
            }
            
            let mode = if archive { DeleteMode::Archive } else if force { DeleteMode::Force } else {
                eprintln!("\x1b[31mError:\x1b[0m Refusing to delete project. Use --force or --archive.");
                return Ok(());
            };
            
            match pm.delete(&target, mode) {
                Ok(_) => println!("\x1b[32mSuccess:\x1b[0m Project deleted."),
                Err(e) => eprintln!("\x1b[31mError:\x1b[0m Failed to delete project: {}", e),
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn is_project_command(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::ProjectCreate { .. }
            | Command::ProjectOpen { .. }
            | Command::ProjectCurrent
            | Command::ProjectClose { .. }
            | Command::ProjectList
            | Command::ProjectDelete { .. }
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut explicit_project = None;
    
    // Parse global --project or -p flag
    if let Some(pos) = args.iter().position(|arg| arg == "--project" || arg == "-p") {
        if pos + 1 < args.len() {
            explicit_project = Some(args[pos + 1].clone());
            args.remove(pos + 1);
            args.remove(pos);
        } else {
            eprintln!("\x1b[31mError:\x1b[0m Missing value for --project flag");
            std::process::exit(1);
        }
    }

    let pm = ProjectManager::load_default()?;

    if !args.is_empty() {
        // One-shot mode
        let full_command = args.join(" ");
        match parse_dsl(&full_command) {
            Ok(cmd) => {
                if is_project_command(&cmd) {
                    execute_local_command(cmd).await?;
                } else {
                    // Resolve project context
                    let project_dir = match pm.resolve_for_command(explicit_project.as_deref()) {
                        Ok(d) => d,
                        Err(e) => {
                            eprintln!("\x1b[31mError:\x1b[0m {}", e);
                            std::process::exit(1);
                        }
                    };
                    
                    let sock_path = project_dir.join(".aether/aether.sock");
                    let mut stream = match get_connection(&project_dir, &sock_path).await {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("\x1b[31mError connecting to daemon:\x1b[0m {:?}", e);
                            std::process::exit(1);
                        }
                    };
                    
                    match send_command(&mut stream, cmd).await {
                        Ok(result) => {
                            if result.success {
                                println!("\x1b[32mSuccess:\x1b[0m {}", result.message);
                                if let Some(r) = result.affected_ref {
                                    println!("Affected Ref: {}", r);
                                }
                                if let Some(snap) = result.snapshot {
                                    print_snapshot(&snap);
                                }
                            } else {
                                eprintln!("\x1b[31mError:\x1b[0m {}", result.message);
                            }
                        }
                        Err(e) => eprintln!("\x1b[31mCommunication Error:\x1b[0m {:?}", e),
                    }
                }
            }
            Err(e) => eprintln!("\x1b[31mDSL Parse Error:\x1b[0m {}", e),
        }
    } else {
        // REPL Mode
        println!("=======================================================");
        println!(" AETHER Headless Media Engine - Interactive CLI (REPL)");
        println!(" Enter commands below. Type 'help' or 'exit' to quit.");
        println!("=======================================================");

        // Try to show current active project
        let mut active_project_dir = match pm.resolve_for_command(explicit_project.as_deref()) {
            Ok(d) => Some(d),
            Err(_) => None,
        };

        if let Some(ref dir) = active_project_dir {
            println!("Active project context: {}", dir.to_string_lossy());
        } else {
            println!("Warning: No active project context. Run 'project create <name>' or 'project open <name>' to start.");
        }

        let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/home/omni"));
        let aether_cli_dir = home.join(".config/aether");
        let _ = fs::create_dir_all(&aether_cli_dir);
        let mut rl = DefaultEditor::new()?;
        let _ = rl.load_history(&aether_cli_dir.join("repl_history.txt"));

        let mut stream = None;

        loop {
            let readline = rl.readline("\x1b[35mAETHER >>\x1b[0m ");
            match readline {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed.to_lowercase() == "exit" || trimmed.to_lowercase() == "quit" {
                        break;
                    }
                    let _ = rl.add_history_entry(trimmed);

                    if trimmed.to_lowercase() == "help" {
                        println!("Available DSL Commands:");
                        println!("  project create <name> [--dir <path>] [--adopt] [--force]");
                        println!("  project open <name-or-path>");
                        println!("  project current");
                        println!("  project close");
                        println!("  project list");
                        println!("  project delete <name-or-path> [--force] [--archive]");
                        println!("  shutdown");
                        println!("  init [<fps>] [<width>x<height>] [<colorspace>]");
                        println!("  import <path>");
                        println!("  trim <ref> <start> <end>");
                        println!("  mix <ref> <volume_lufs>");
                        println!("  composite <base_ref> <overlay_ref> <at> <x> <y>");
                        println!("  canvas <width> <height> <color>");
                        println!("  draw_text <ref> \"<text>\" <font> <size> <x> <y>");
                        println!("  export <ref> <format> [<codec>] [<quality>]");
                        println!("  inspect <ref> [<start>] [<end>]");
                        println!("  generate storyboard-scratch \"<prompt>\"");
                        println!("  generate dialogue \"<prompt>\"");
                        println!("  generate image \"<prompt>\"");
                        println!("  edit image <image_ref> \"<prompt>\"");
                        println!("  generate voice \"<text>\"");
                        println!("  clone voice <audio_ref>");
                        println!("  generate scene-audio \"<prompt>\"");
                        println!("  generate music \"<prompt>\"");
                        println!("  generate video \"<prompt>\"");
                        println!("  generate video-frame <image_ref> \"<prompt>\"");
                        println!("  generate video-ingredients <refs...> --prompt \"<prompt>\"");
                        println!("  edit video <video_ref> \"<prompt>\"");
                        println!("  generation status [<gen_ref>]");
                        println!("  generation cancel <gen_ref>");
                        println!("  undo");
                        println!("  redo");
                        println!("  snapshot");
                        continue;
                    }

                    match parse_dsl(trimmed) {
                        Ok(cmd) => {
                            if is_project_command(&cmd) {
                                let old_active = active_project_dir.clone();
                                let _ = execute_local_command(cmd).await;
                                
                                // Re-resolve active project
                                active_project_dir = match pm.resolve_for_command(None) {
                                    Ok(d) => Some(d),
                                    Err(_) => None,
                                };
                                
                                if active_project_dir != old_active {
                                    stream = None; // Reset stream so we connect to the new project daemon
                                    if let Some(ref dir) = active_project_dir {
                                        println!("Active project context changed to: {}", dir.to_string_lossy());
                                    } else {
                                        println!("No active project context.");
                                    }
                                }
                            } else {
                                // For non-project commands, make sure we have a resolved project directory
                                let project_dir = match &active_project_dir {
                                    Some(d) => d.clone(),
                                    None => {
                                        match pm.resolve_for_command(None) {
                                            Ok(d) => {
                                                active_project_dir = Some(d.clone());
                                                d
                                            }
                                            Err(e) => {
                                                eprintln!("\x1b[31mError:\x1b[0m {}", e);
                                                continue;
                                            }
                                        }
                                    }
                                };
                                
                                let sock_path = project_dir.join(".aether/aether.sock");
                                
                                // Get connection if not already connected
                                let current_stream = match &mut stream {
                                    Some(s) => s,
                                    None => {
                                        match get_connection(&project_dir, &sock_path).await {
                                            Ok(s) => {
                                                stream = Some(s);
                                                stream.as_mut().unwrap()
                                            }
                                            Err(e) => {
                                                eprintln!("\x1b[31mError connecting to daemon:\x1b[0m {:?}", e);
                                                continue;
                                            }
                                        }
                                    }
                                };

                                let is_shutdown = matches!(cmd, Command::Shutdown);
                                
                                match send_command(current_stream, cmd).await {
                                    Ok(result) => {
                                        if result.success {
                                            println!("\x1b[32mSuccess:\x1b[0m {}", result.message);
                                            if let Some(r) = result.affected_ref {
                                                println!("Affected Ref: {}", r);
                                            }
                                            if let Some(snap) = result.snapshot {
                                                print_snapshot(&snap);
                                            }
                                        } else {
                                            eprintln!("\x1b[31mError:\x1b[0m {}", result.message);
                                        }
                                        if is_shutdown {
                                            stream = None;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("\x1b[31mCommunication Error:\x1b[0m {:?}", e);
                                        // Try reconnecting
                                        if let Ok(new_stream) = get_connection(&project_dir, &sock_path).await {
                                            stream = Some(new_stream);
                                            println!("Reconnected to AETHER daemon.");
                                        } else {
                                            stream = None;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => eprintln!("\x1b[31mDSL Parse Error:\x1b[0m {}", e),
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("CTRL-C");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    println!("CTRL-D");
                    break;
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    break;
                }
            }
        }
        let _ = rl.save_history(&aether_cli_dir.join("repl_history.txt"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer_simple() {
        let tokens = tokenize("init 30 1920x1080 srgb").unwrap();
        assert_eq!(tokens, vec!["init", "30", "1920x1080", "srgb"]);
    }

    #[test]
    fn test_tokenizer_quotes() {
        let tokens = tokenize("draw_text @img1 \"Hello World\" Arial 24").unwrap();
        assert_eq!(tokens, vec!["draw_text", "@img1", "Hello World", "Arial", "24"]);
    }

    #[test]
    fn test_parser_init() {
        let cmd = parse_dsl("init 25 1280x720 rec2020").unwrap();
        assert_eq!(cmd, Command::Init {
            fps: Some(25.0),
            resolution: Some("1280x720".to_string()),
            colorspace: Some("rec2020".to_string())
        });
    }

    #[test]
    fn test_parser_canvas() {
        let cmd = parse_dsl("canvas 640 480 green").unwrap();
        assert_eq!(cmd, Command::Canvas {
            width: 640,
            height: 480,
            color: "green".to_string()
        });
    }

    #[test]
    fn test_parser_generative_commands() {
        let cmd = parse_dsl("image \"A cute kitten\" mock/image").unwrap();
        assert!(matches!(cmd, Command::GenerateImage { .. }));

        let cmd2 = parse_dsl("status @g123").unwrap();
        assert!(matches!(cmd2, Command::GenerationStatus { .. }));

        let cmd3 = parse_dsl("cancel @g123").unwrap();
        assert!(matches!(cmd3, Command::CancelGeneration { .. }));

        // Two-word aligned grammar tests
        let cmd4 = parse_dsl("generate storyboard-scratch \"test prompt\"").unwrap();
        assert!(matches!(cmd4, Command::GenerateStoryboardScratch { .. }));

        let cmd5 = parse_dsl("generate image \"test prompt\"").unwrap();
        assert!(matches!(cmd5, Command::GenerateImage { .. }));

        let cmd6 = parse_dsl("edit image @img1 \"test prompt\"").unwrap();
        assert!(matches!(cmd6, Command::EditImage { .. }));

        let cmd7 = parse_dsl("generate video-ingredients @img1 @a1 --prompt \"test prompt\"").unwrap();
        assert!(matches!(cmd7, Command::GenerateVideoFromIngredients { .. }));

        let cmd8 = parse_dsl("edit video @v1 \"test prompt\"").unwrap();
        assert!(matches!(cmd8, Command::EditVideo { .. }));

        let cmd9 = parse_dsl("generation status @g1").unwrap();
        assert!(matches!(cmd9, Command::GenerationStatus { .. }));

        let cmd10 = parse_dsl("generation cancel @g1").unwrap();
        assert!(matches!(cmd10, Command::CancelGeneration { .. }));
    }
}
