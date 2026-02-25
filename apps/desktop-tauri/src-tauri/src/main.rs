use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

#[derive(Debug, Clone)]
struct JsonRpcBridgeRequest {
    payload_json: String,
}

#[derive(Debug, Clone)]
struct JsonRpcBridgeResponse {
    response_json: String,
}

#[derive(Debug)]
struct BackendProcessManager {
    child: Option<Child>,
    mode: BackendMode,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum BackendMode {
    HttpDev,
    StdioMcp,
}

impl Default for BackendProcessManager {
    fn default() -> Self {
        Self {
            child: None,
            mode: BackendMode::HttpDev,
        }
    }
}

impl BackendProcessManager {
    fn ensure_started(&mut self) -> io::Result<()> {
        if self.child.is_some() {
            return Ok(());
        }

        // Scaffold behavior only: attempt to start the CLI local HTTP server if available.
        // In a real Tauri integration, this would use tauri::AppHandle for paths/logging and
        // expose commands like `jsonrpc_request` via `tauri::generate_handler!`.
        let child = Command::new("cargo")
            .args(["run", "-p", "cli", "--", "serve-http"])
            .current_dir(repo_root_guess()?)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        self.child = Some(child);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn jsonrpc_request_stub(&mut self, req: JsonRpcBridgeRequest) -> JsonRpcBridgeResponse {
        JsonRpcBridgeResponse {
            response_json: format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{{\"code\":-32000,\"message\":\"Tauri backend bridge not wired yet (mode={:?}). Received {} bytes.\"}}}}",
                self.mode,
                req.payload_json.len()
            ),
        }
    }
}

impl Drop for BackendProcessManager {
    fn drop(&mut self) {
        self.stop();
    }
}

fn repo_root_guess() -> io::Result<PathBuf> {
    // `apps/desktop-tauri/src-tauri` -> repo root is three levels up.
    let cwd = std::env::current_dir()?;
    Ok(cwd)
}

fn main() {
    // Compile-safe scaffold for the next stage:
    // - define bridge/process lifecycle contract
    // - keep backend binary buildable without Tauri dependencies
    //
    // Planned Tauri v2 wiring:
    // 1. add `tauri` dependency
    // 2. define command `jsonrpc_request(payload_json: String) -> String`
    // 3. manage local backend child (stdio or local socket/http) in shared state
    let mut manager = BackendProcessManager::default();

    println!("desktop-tauri backend scaffold (bridge contract placeholder)");
    let probe = manager.jsonrpc_request_stub(JsonRpcBridgeRequest {
        payload_json: "{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"tools.list\",\"params\":{}}".to_string(),
    });
    println!("jsonrpc_request stub ready ({} bytes)", probe.response_json.len());

    // Prevent unused method warnings while keeping runtime side effects minimal.
    let _ = manager.ensure_started().map_err(|err| {
        eprintln!("backend auto-start skipped in scaffold: {err}");
        err
    });
    manager.stop();
}
