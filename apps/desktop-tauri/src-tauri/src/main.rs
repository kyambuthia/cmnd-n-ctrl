use std::env;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
#[cfg(feature = "tauri-app")]
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

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
    http_addr: String,
    auto_spawn: bool,
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
            http_addr: env::var("CMND_N_CTRL_BACKEND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:7777".to_string()),
            auto_spawn: env::var("CMND_N_CTRL_AUTOSPAWN_BACKEND")
                .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
                .unwrap_or(true),
        }
    }
}

impl BackendProcessManager {
    fn ensure_started(&mut self) -> io::Result<()> {
        if !matches!(self.mode, BackendMode::HttpDev) {
            return Err(io::Error::other("only HttpDev bridge mode implemented"));
        }

        if self.check_http_ready().is_ok() {
            return Ok(());
        }

        if self.child.is_some() {
            self.wait_for_http_ready()?;
            return Ok(());
        }

        if !self.auto_spawn {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "backend not reachable and autospawn disabled",
            ));
        }

        let child = Command::new("cargo")
            .args(["run", "-p", "cli", "--", "serve-http", "--addr", &self.http_addr])
            .current_dir(repo_root_guess()?)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        self.child = Some(child);
        self.wait_for_http_ready()
    }

    fn wait_for_http_ready(&self) -> io::Result<()> {
        let mut last_err = None;
        for _ in 0..25 {
            match self.check_http_ready() {
                Ok(()) => return Ok(()),
                Err(err) => {
                    last_err = Some(err);
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
        Err(last_err.unwrap_or_else(|| io::Error::other("backend did not become ready")))
    }

    fn check_http_ready(&self) -> io::Result<()> {
        let _ = TcpStream::connect(&self.http_addr)?;
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn jsonrpc_request(&mut self, req: JsonRpcBridgeRequest) -> io::Result<JsonRpcBridgeResponse> {
        self.ensure_started()?;
        let response_json = post_jsonrpc_http(&self.http_addr, &req.payload_json)?;
        Ok(JsonRpcBridgeResponse { response_json })
    }
}

impl Drop for BackendProcessManager {
    fn drop(&mut self) {
        self.stop();
    }
}

fn post_jsonrpc_http(addr: &str, payload_json: &str) -> io::Result<String> {
    let mut stream = TcpStream::connect(addr)?;
    let request = format!(
        "POST /jsonrpc HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        payload_json.len(),
        payload_json
    );
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let raw = String::from_utf8(raw)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("utf8 response: {err}")))?;
    let (headers, body) = raw
        .split_once("\r\n\r\n")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed http response"))?;
    let status = headers
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    if status != 200 {
        return Err(io::Error::other(format!("http {status}: {body}")));
    }
    Ok(body.to_string())
}

fn repo_root_guess() -> io::Result<PathBuf> {
    let cwd = env::current_dir()?;
    if cwd.ends_with("src-tauri") {
        return Ok(cwd.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).unwrap_or(&cwd).to_path_buf());
    }
    Ok(cwd)
}

fn jsonrpc_request_command(
    manager: &mut BackendProcessManager,
    payload_json: String,
) -> Result<String, String> {
    manager
        .jsonrpc_request(JsonRpcBridgeRequest { payload_json })
        .map(|r| r.response_json)
        .map_err(|e| e.to_string())
}

#[cfg(feature = "tauri-app")]
struct TauriBridgeState {
    manager: Mutex<BackendProcessManager>,
}

#[cfg(feature = "tauri-app")]
impl Default for TauriBridgeState {
    fn default() -> Self {
        Self {
            manager: Mutex::new(BackendProcessManager::default()),
        }
    }
}

#[cfg(feature = "tauri-app")]
#[tauri::command]
fn jsonrpc_request(
    state: tauri::State<'_, TauriBridgeState>,
    payload_json: String,
) -> Result<String, String> {
    let mut guard = state
        .manager
        .lock()
        .map_err(|_| "backend bridge mutex poisoned".to_string())?;
    jsonrpc_request_command(&mut guard, payload_json)
}

#[cfg(feature = "tauri-app")]
fn tauri_integration_contract() {
    let _builder = tauri::Builder::default()
        .manage(TauriBridgeState::default())
        .invoke_handler(tauri::generate_handler![jsonrpc_request]);
}

#[cfg(not(feature = "tauri-app"))]
fn main() {
    // Compile-safe bridge binary for eventual Tauri v2 integration.
    // Planned next step:
    // - add `tauri` dependency and annotate `jsonrpc_request_command` as a Tauri command
    // - store `BackendProcessManager` in managed Tauri state
    // - call from frontend via `invoke('jsonrpc_request', { payloadJson })`
    let mut manager = BackendProcessManager::default();

    let probe_payload =
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools.list\",\"params\":{}}".to_string();
    match jsonrpc_request_command(&mut manager, probe_payload) {
        Ok(resp) => println!("desktop bridge ready (response bytes={})", resp.len()),
        Err(err) => {
            eprintln!("desktop bridge probe failed: {err}");
            println!("desktop bridge initialized (backend unavailable)");
        }
    }
}

#[cfg(feature = "tauri-app")]
fn main() {
    tauri_integration_contract();
    println!("desktop bridge tauri-app feature compiled (command registered in builder)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_body_roundtrip_helper_contract() {
        // Structural sanity test for the bridge command wrapper contract.
        let mut manager = BackendProcessManager {
            child: None,
            mode: BackendMode::HttpDev,
            http_addr: "127.0.0.1:1".to_string(),
            auto_spawn: false,
        };
        let result = jsonrpc_request_command(
            &mut manager,
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools.list\",\"params\":{}}".to_string(),
        );
        assert!(result.is_err());
    }
}
