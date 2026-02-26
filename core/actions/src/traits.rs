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
                tool_call_id: None,
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
                tool_call_id: None,
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
                tool_call_id: None,
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
                tool_call_id: None,
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
                tool_call_id: None,
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
                tool_call_id: None,
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
                tool_call_id: None,
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

        if tool_call.name == "file.read_json" {
            let requested = args.get("path").and_then(Value::as_str);
            let path = match self.scoped_path(requested) {
                Ok(p) => p,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        err,
                        "file.read_json",
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
                        "file.read_json",
                        path.display().to_string(),
                    )
                }
            };
            let parsed = match serde_json::from_str::<Value>(&raw) {
                Ok(v) => v,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        format!("json_parse_failed:{err}"),
                        "file.read_json",
                        path.display().to_string(),
                    )
                }
            };
            let preview = preview_json(&parsed);
            return ToolResult {
                tool_call_id: None,
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "project_root": self.project_root_display(),
                    "path": path.display().to_string(),
                    "json_preview": preview
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Read JSON file {}", path.display()),
                    format!("stub://{}/file.read_json", self.platform),
                ),
            };
        }

        if tool_call.name == "file.search_text" {
            let query = args.get("query").and_then(Value::as_str).unwrap_or("").trim();
            if query.is_empty() {
                return tool_error(
                    &tool_call.name,
                    self.platform,
                    "missing_query",
                    "file.search_text",
                    self.project_root_display(),
                );
            }
            let requested = args.get("path").and_then(Value::as_str);
            let root = match self.scoped_path(requested) {
                Ok(p) => p,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        err,
                        "file.search_text",
                        self.project_root_display(),
                    )
                }
            };
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(25).min(200) as usize;
            let mut matches = Vec::new();
            let mut scanned_files = 0usize;
            search_text_recursive(&root, query, limit, &mut matches, &mut scanned_files);
            return ToolResult {
                tool_call_id: None,
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "project_root": self.project_root_display(),
                    "path": root.display().to_string(),
                    "query": query,
                    "scanned_files": scanned_files,
                    "limit": limit,
                    "matches": matches
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Searched project text for '{}' under {}", query, root.display()),
                    format!("stub://{}/file.search_text", self.platform),
                ),
            };
        }

        if tool_call.name == "file.stat" {
            let requested = args.get("path").and_then(Value::as_str);
            let path = match self.scoped_path(requested) {
                Ok(p) => p,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        err,
                        "file.stat",
                        self.project_root_display(),
                    )
                }
            };
            let meta = match fs::metadata(&path) {
                Ok(v) => v,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        format!("stat_failed:{err}"),
                        "file.stat",
                        path.display().to_string(),
                    )
                }
            };
            let modified_unix = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            return ToolResult {
                tool_call_id: None,
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "project_root": self.project_root_display(),
                    "path": path.display().to_string(),
                    "is_dir": meta.is_dir(),
                    "is_file": meta.is_file(),
                    "len": meta.len(),
                    "modified_unix_seconds": modified_unix
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Read metadata for {}", path.display()),
                    format!("stub://{}/file.stat", self.platform),
                ),
            };
        }

        if tool_call.name == "file.write_text" {
            let requested = args.get("path").and_then(Value::as_str);
            let content = args.get("content").and_then(Value::as_str).unwrap_or("");
            let path = match self.scoped_path(requested) {
                Ok(p) => p,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        err,
                        "file.write_text",
                        self.project_root_display(),
                    )
                }
            };
            if let Some(parent) = path.parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        format!("mkdir_failed:{err}"),
                        "file.write_text",
                        path.display().to_string(),
                    );
                }
            }
            if let Err(err) = fs::write(&path, content) {
                return tool_error(
                    &tool_call.name,
                    self.platform,
                    format!("write_failed:{err}"),
                    "file.write_text",
                    path.display().to_string(),
                );
            }
            return ToolResult {
                tool_call_id: None,
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "project_root": self.project_root_display(),
                    "path": path.display().to_string(),
                    "bytes_written": content.len(),
                    "note": "file written under project scope"
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Wrote text file {} ({} bytes)", path.display(), content.len()),
                    format!("stub://{}/file.write_text", self.platform),
                ),
            };
        }

        if tool_call.name == "file.append_text" {
            let requested = args.get("path").and_then(Value::as_str);
            let content = args.get("content").and_then(Value::as_str).unwrap_or("");
            let path = match self.scoped_path(requested) {
                Ok(p) => p,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        err,
                        "file.append_text",
                        self.project_root_display(),
                    )
                }
            };
            if let Some(parent) = path.parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        format!("mkdir_failed:{err}"),
                        "file.append_text",
                        path.display().to_string(),
                    );
                }
            }
            let mut existing = if path.exists() {
                fs::read_to_string(&path).unwrap_or_default()
            } else {
                String::new()
            };
            existing.push_str(content);
            if let Err(err) = fs::write(&path, &existing) {
                return tool_error(
                    &tool_call.name,
                    self.platform,
                    format!("append_failed:{err}"),
                    "file.append_text",
                    path.display().to_string(),
                );
            }
            return ToolResult {
                tool_call_id: None,
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "project_root": self.project_root_display(),
                    "path": path.display().to_string(),
                    "bytes_appended": content.len(),
                    "bytes_total": existing.len(),
                    "note": "file appended under project scope"
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Appended text file {} ({} bytes)", path.display(), content.len()),
                    format!("stub://{}/file.append_text", self.platform),
                ),
            };
        }

        if tool_call.name == "file.mkdir" {
            let requested = args.get("path").and_then(Value::as_str);
            let path = match self.scoped_path(requested) {
                Ok(p) => p,
                Err(err) => {
                    return tool_error(
                        &tool_call.name,
                        self.platform,
                        err,
                        "file.mkdir",
                        self.project_root_display(),
                    )
                }
            };
            if let Err(err) = fs::create_dir_all(&path) {
                return tool_error(
                    &tool_call.name,
                    self.platform,
                    format!("mkdir_failed:{err}"),
                    "file.mkdir",
                    path.display().to_string(),
                );
            }
            return ToolResult {
                tool_call_id: None,
                name: tool_call.name.clone(),
                result_json: json!({
                    "status": "ok",
                    "platform": self.platform,
                    "project_root": self.project_root_display(),
                    "path": path.display().to_string(),
                    "note": "directory created under project scope"
                })
                .to_string(),
                evidence: crate::evidence::action_evidence(
                    format!("Created directory {}", path.display()),
                    format!("stub://{}/file.mkdir", self.platform),
                ),
            };
        }

        if tool_call.name == "desktop.open_url" {
            let url = args.get("url").and_then(Value::as_str).unwrap_or("about:blank");
            return ToolResult {
                tool_call_id: None,
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
                tool_call_id: None,
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
                tool_call_id: None,
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
            tool_call_id: None,
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
        tool_call_id: None,
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

fn preview_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (idx, (k, v)) in map.iter().enumerate() {
                if idx >= 20 {
                    out.insert("_truncated".to_string(), Value::Bool(true));
                    break;
                }
                out.insert(k.clone(), preview_json_scalar_or_shallow(v));
            }
            Value::Object(out)
        }
        Value::Array(arr) => {
            let mut out = Vec::new();
            for (idx, v) in arr.iter().enumerate() {
                if idx >= 20 {
                    out.push(json!({"_truncated": true}));
                    break;
                }
                out.push(preview_json_scalar_or_shallow(v));
            }
            Value::Array(out)
        }
        _ => preview_json_scalar_or_shallow(value),
    }
}

fn preview_json_scalar_or_shallow(value: &Value) -> Value {
    match value {
        Value::String(s) => Value::String(truncate_chars(s, 500)),
        Value::Array(arr) => json!({"type":"array","len":arr.len()}),
        Value::Object(obj) => json!({"type":"object","keys": obj.keys().take(10).cloned().collect::<Vec<_>>() }),
        _ => value.clone(),
    }
}

fn search_text_recursive(
    root: &Path,
    query: &str,
    limit: usize,
    matches_out: &mut Vec<Value>,
    scanned_files: &mut usize,
) {
    if matches_out.len() >= limit {
        return;
    }
    let Ok(read_dir) = fs::read_dir(root) else {
        return;
    };
    for entry in read_dir.flatten() {
        if matches_out.len() >= limit {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            search_text_recursive(&path, query, limit, matches_out, scanned_files);
            continue;
        }
        if !path.is_file() {
            continue;
        }
        *scanned_files += 1;
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.len() > 512 * 1024 {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        for (line_no, line) in raw.lines().enumerate() {
            if line.contains(query) {
                matches_out.push(json!({
                    "path": path.display().to_string(),
                    "line": line_no + 1,
                    "snippet": truncate_chars(line, 220)
                }));
                if matches_out.len() >= limit {
                    return;
                }
            }
        }
    }
}
