use ipc::{ToolCall, ToolResult};
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait ActionBackend {
    fn platform_name(&self) -> &'static str;
    fn execute_tool(&self, tool_call: &ToolCall) -> ToolResult;
}

#[derive(Clone, Debug)]
pub struct StubActionBackend {
    platform: &'static str,
    project_root: Option<PathBuf>,
}

impl StubActionBackend {
    pub fn new(platform: &'static str) -> Self {
        Self {
            platform,
            project_root: None,
        }
    }

    pub fn with_project_root(platform: &'static str, project_root: Option<PathBuf>) -> Self {
        Self {
            platform,
            project_root,
        }
    }

    fn scoped_path(&self, requested: Option<&str>) -> Result<PathBuf, String> {
        let root = self
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| "unable to resolve project root".to_string())?;

        let rel = requested.unwrap_or(".").trim();
        let rel = if rel.is_empty() { "." } else { rel };

        let requested_path = Path::new(rel);
        let candidate = if requested_path.is_absolute() {
            requested_path.to_path_buf()
        } else {
            root.join(requested_path)
        };

        let normalized = normalize_path(candidate);
        let normalized_root = normalize_path(root.clone());
        if !normalized.starts_with(&normalized_root) {
            return Err("path_outside_project_scope".to_string());
        }
        Ok(normalized)
    }

    fn project_root_display(&self) -> String {
        self.project_root
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".".to_string())
    }
}

impl ActionBackend for StubActionBackend {
    fn platform_name(&self) -> &'static str {
        self.platform
    }

    fn execute_tool(&self, tool_call: &ToolCall) -> ToolResult {
        let args = serde_json::from_str::<Value>(&tool_call.arguments_json).unwrap_or(Value::Null);

        if tool_call.name == "time.now" {
            let unix_seconds = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            return ToolResult {
                name: tool_call.name.clone(),
                result_json: format!(
                    "{{\"status\":\"ok\",\"platform\":\"{}\",\"unix_seconds\":{}}}",
                    self.platform, unix_seconds
                ),
                evidence: crate::evidence::action_evidence(
                    format!("Read local time on {}", self.platform),
                    format!("stub://{}/time.now", self.platform),
                ),
            };
        }

        if tool_call.name == "echo" {
            let input = args.get("input").and_then(Value::as_str).unwrap_or_default();
            return ToolResult {
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "input": input
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Echoed local payload on {}", self.platform),
                    format!("stub://{}/echo", self.platform),
                ),
            };
        }

        if tool_call.name == "text.uppercase" {
            let text = args.get("text").and_then(Value::as_str).unwrap_or_default();
            return ToolResult {
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "text": text,
                    "uppercased": text.to_uppercase()
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Uppercased text locally on {}", self.platform),
                    format!("stub://{}/text.uppercase", self.platform),
                ),
            };
        }

        if tool_call.name == "math.add" {
            let a = args.get("a").and_then(Value::as_f64).unwrap_or(0.0);
            let b = args.get("b").and_then(Value::as_f64).unwrap_or(0.0);
            return ToolResult {
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "a": a,
                    "b": b,
                    "sum": a + b
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Computed local addition on {}", self.platform),
                    format!("stub://{}/math.add", self.platform),
                ),
            };
        }

        if tool_call.name == "file.list" {
            let requested = args.get("path").and_then(Value::as_str);
            let dir = match self.scoped_path(requested) {
                Ok(p) => p,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        err,
                        "file.list",
                        self.project_root_display(),
                    )
                }
            };
            let read_dir = match fs::read_dir(&dir) {
                Ok(d) => d,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        format!("read_dir_failed:{err}"),
                        "file.list",
                        dir.display().to_string(),
                    )
                }
            };
            let mut entries = read_dir
                .filter_map(Result::ok)
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let path = e.path();
                    let kind = if path.is_dir() { "dir" } else { "file" };
                    json!({ "name": name, "kind": kind })
                })
                .collect::<Vec<_>>();
            entries.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
            return ToolResult {
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "project_root": self.project_root_display(),
                    "path": dir.display().to_string(),
                    "entries": entries
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Listed files in {}", dir.display()),
                    format!("stub://{}/file.list", self.platform),
                ),
            };
        }

        if tool_call.name == "file.read_text" {
            let requested = args.get("path").and_then(Value::as_str);
            let path = match self.scoped_path(requested) {
                Ok(p) => p,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        err,
                        "file.read_text",
                        self.project_root_display(),
                    )
                }
            };
            let raw = match fs::read_to_string(&path) {
                Ok(v) => v,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        format!("read_failed:{err}"),
                        "file.read_text",
                        path.display().to_string(),
                    )
                }
            };
            let preview = truncate_chars(&raw, 2000);
            return ToolResult {
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "project_root": self.project_root_display(),
                    "path": path.display().to_string(),
                    "bytes": raw.len(),
                    "text": preview,
                    "truncated": raw.len() > preview.len(),
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Read text file {}", path.display()),
                    format!("stub://{}/file.read_text", self.platform),
                ),
            };
        }

        if tool_call.name == "file.read_csv" {
            let requested = args.get("path").and_then(Value::as_str);
            let path = match self.scoped_path(requested) {
                Ok(p) => p,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        err,
                        "file.read_csv",
                        self.project_root_display(),
                    )
                }
            };
            let raw = match fs::read_to_string(&path) {
                Ok(v) => v,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        format!("read_failed:{err}"),
                        "file.read_csv",
                        path.display().to_string(),
                    )
                }
            };
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20).min(200) as usize;
            let mut lines = raw.lines();
            let header_line = lines.next().unwrap_or("");
            let headers = split_csv_line(header_line);
            let mut rows = Vec::new();
            for line in lines.take(limit) {
                let cols = split_csv_line(line);
                let mut obj = serde_json::Map::new();
                for (idx, value) in cols.iter().enumerate() {
                    let key = headers
                        .get(idx)
                        .cloned()
                        .filter(|h| !h.is_empty())
                        .unwrap_or_else(|| format!("col_{}", idx + 1));
                    obj.insert(key, Value::String(value.clone()));
                }
                rows.push(Value::Object(obj));
            }
            return ToolResult {
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "project_root": self.project_root_display(),
                    "path": path.display().to_string(),
                    "headers": headers,
                    "rows_preview": rows,
                    "rows_preview_limit": limit
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Read CSV file {} (preview)", path.display()),
                    format!("stub://{}/file.read_csv", self.platform),
                ),
            };
        }

        if tool_call.name == "desktop.open_url" {
            let url = args.get("url").and_then(Value::as_str).unwrap_or("about:blank");
            return ToolResult {
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "url": url,
                    "note": "stub_only_not_opened"
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Approved open_url stub for '{}' on {}", url, self.platform),
                    format!("stub://{}/desktop.open_url", self.platform),
                ),
            };
        }

        if tool_call.name == "desktop.app.list" {
            let filter = args.get("filter").and_then(Value::as_str).unwrap_or_default();
            return ToolResult {
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "apps": [
                        {"id":"app-1","name":"Browser","state":"running"},
                        {"id":"app-2","name":"Editor","state":"running"}
                    ],
                    "filter": filter,
                    "note": "stub_only_not_real_process_enumeration"
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Listed desktop apps stub on {}", self.platform),
                    format!("stub://{}/desktop.app.list", self.platform),
                ),
            };
        }

        if tool_call.name == "desktop.app.activate" {
            let app = args.get("app").and_then(Value::as_str).unwrap_or("unknown");
            return ToolResult {
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "app": app,
                    "note": "stub_only_not_real_window_activation"
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Activated desktop app stub '{}' on {}", app, self.platform),
                    format!("stub://{}/desktop.app.activate", self.platform),
                ),
            };
        }

        ToolResult {
            name: tool_call.name.clone(),
            result_json: format!(
                "{{\"status\":\"ok\",\"platform\":\"{}\",\"arguments\":{}}}",
                self.platform, tool_call.arguments_json
            ),
            evidence: crate::evidence::action_evidence(
                format!("Executed stub action '{}' on {}", tool_call.name, self.platform),
                format!("stub://{}/{}", self.platform, tool_call.name),
            ),
        }
    }
}

fn tool_error(
    tool_name: &str,
    platform: &str,
    code: impl Into<String>,
    op: &str,
    artifact: String,
) -> ToolResult {
    let code = code.into();
    ToolResult {
        name: tool_name.to_string(),
        result_json: json!({
            "status": "error",
            "platform": platform,
            "error": code
        })
        .to_string(),
        evidence: crate::evidence::action_evidence(
            format!("{} failed on {}: {}", op, platform, code),
            artifact,
        ),
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let mut chars = input.chars();
    let out: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{out}...")
    } else {
        out
    }
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn split_csv_line(line: &str) -> Vec<String> {
    line.split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .collect()
}
