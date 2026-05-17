use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde::{Deserialize, Serialize};
use aether_core::{Command, CommandResult};
use aether_daemon::SessionManager;

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum IncomingRequest {
    JsonRpc {
        jsonrpc: String,
        method: String,
        params: Command,
        id: serde_json::Value,
    },
    Direct(Command),
}

#[derive(Serialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    result: Option<CommandResult>,
    error: Option<String>,
    id: serde_json::Value,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    println!("=== AETHER Headless Media Engine Daemon starting ===");

    // Determine project directory (default: current directory)
    let project_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    println!("Project directory: {}", project_dir.to_string_lossy());

    // Initialize session manager
    let session = Arc::new(SessionManager::new(&project_dir)?);
    println!("Session manager and sqlite database successfully initialized.");

    // Setup UDS Socket
    let aether_dir = project_dir.join(".aether");
    if !aether_dir.exists() {
        fs::create_dir_all(&aether_dir)?;
    }
    let sock_path = aether_dir.join("aether.sock");
    if sock_path.exists() {
        let _ = fs::remove_file(&sock_path);
    }

    let listener = UnixListener::bind(&sock_path)?;
    println!("UDS server listening on socket: {}", sock_path.to_string_lossy());

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let session_clone = Arc::clone(&session);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, session_clone).await {
                        eprintln!("Error handling connection: {:?}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {:?}", e);
            }
        }
    }
}

async fn handle_connection(mut stream: UnixStream, session: Arc<SessionManager>) -> Result<(), Box<dyn std::error::Error>> {
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    while buf_reader.read_line(&mut line).await? > 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        // Parse command request
        let req_parsed: Result<IncomingRequest, _> = serde_json::from_str(trimmed);
        
        let response_bytes = match req_parsed {
            Ok(IncomingRequest::JsonRpc { jsonrpc, method, params, id }) => {
                if method != "execute" {
                    let err_resp = JsonRpcResponse {
                        jsonrpc,
                        result: None,
                        error: Some("Unknown JSON-RPC method. Only 'execute' is supported.".to_string()),
                        id,
                    };
                    serde_json::to_vec(&err_resp)?
                } else {
                    match session.execute(params) {
                        Ok(res) => {
                            let resp = JsonRpcResponse {
                                jsonrpc,
                                result: Some(res),
                                error: None,
                                id,
                            };
                            serde_json::to_vec(&resp)?
                        }
                        Err(e) => {
                            let err_resp = JsonRpcResponse {
                                jsonrpc,
                                result: None,
                                error: Some(e.to_string()),
                                id,
                            };
                            serde_json::to_vec(&err_resp)?
                        }
                    }
                }
            }
            Ok(IncomingRequest::Direct(cmd)) => {
                match session.execute(cmd) {
                    Ok(res) => serde_json::to_vec(&res)?,
                    Err(e) => {
                        let err_res = CommandResult {
                            success: false,
                            affected_ref: None,
                            message: e.to_string(),
                            snapshot: None,
                        };
                        serde_json::to_vec(&err_res)?
                    }
                }
            }
            Err(err) => {
                // If parsing completely failed, return a direct error CommandResult
                let err_res = CommandResult {
                    success: false,
                    affected_ref: None,
                    message: format!("Failed to parse JSON request: {}", err),
                    snapshot: None,
                };
                serde_json::to_vec(&err_res)?
            }
        };

        // Write response followed by a newline (framing)
        writer.write_all(&response_bytes).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        line.clear();
    }

    Ok(())
}
