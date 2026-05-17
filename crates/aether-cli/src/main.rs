use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use serde_json;
use aether_core::{Command, CommandResult, Snapshot};

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
    let tokens = tokenize(line)?;
    if tokens.is_empty() {
        return Err("Empty command".to_string());
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
        "undo" => Ok(Command::Undo),
        "redo" => Ok(Command::Redo),
        "snapshot" => Ok(Command::Snapshot),
        other => Err(format!("Unknown command '{}'", other)),
    }
}

async fn get_connection(sock_path: &Path) -> Result<UnixStream, std::io::Error> {
    match UnixStream::connect(sock_path).await {
        Ok(stream) => Ok(stream),
        Err(_e) => {
            // Auto-start daemon in the background
            println!("Daemon not running. Auto-starting AETHER daemon...");
            let daemon_binary = "cargo";
            let _child = std::process::Command::new(daemon_binary)
                .args(["run", "--bin", "aether-daemon", "--quiet"])
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = std::env::current_dir()?;
    let aether_dir = current_dir.join(".aether");
    if !aether_dir.exists() {
        fs::create_dir_all(&aether_dir)?;
    }
    let sock_path = aether_dir.join("aether.sock");

    // Gather args beyond CLI executable
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut stream = get_connection(&sock_path).await?;

    if !args.is_empty() {
        // One-shot mode
        let full_command = args.join(" ");
        match parse_dsl(&full_command) {
            Ok(cmd) => {
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
            Err(e) => eprintln!("\x1b[31mDSL Parse Error:\x1b[0m {}", e),
        }
    } else {
        // REPL Mode
        println!("=======================================================");
        println!(" AETHER Headless Media Engine - Interactive CLI (REPL)");
        println!(" Enter commands below. Type 'help' or 'exit' to quit.");
        println!("=======================================================");

        let mut rl = DefaultEditor::new()?;
        let _ = rl.load_history(&aether_dir.join("repl_history.txt"));

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
                        println!("  init [<fps>] [<width>x<height>] [<colorspace>]");
                        println!("  import <path>");
                        println!("  trim <ref> <start> <end>");
                        println!("  mix <ref> <volume_lufs>");
                        println!("  composite <base_ref> <overlay_ref> <at> <x> <y>");
                        println!("  canvas <width> <height> <color>");
                        println!("  draw_text <ref> \"<text>\" <font> <size> <x> <y>");
                        println!("  export <ref> <format> [<codec>] [<quality>]");
                        println!("  undo");
                        println!("  redo");
                        println!("  snapshot");
                        continue;
                    }

                    match parse_dsl(trimmed) {
                        Ok(cmd) => {
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
                                Err(e) => {
                                    eprintln!("\x1b[31mCommunication Error:\x1b[0m {:?}", e);
                                    // Try reconnecting
                                    if let Ok(new_stream) = get_connection(&sock_path).await {
                                        stream = new_stream;
                                        println!("Reconnected to AETHER daemon.");
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
        let _ = rl.save_history(&aether_dir.join("repl_history.txt"));
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
}
