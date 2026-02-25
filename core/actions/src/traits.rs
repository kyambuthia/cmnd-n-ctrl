use ipc::{ToolCall, ToolResult};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait ActionBackend {
    fn platform_name(&self) -> &'static str;
    fn execute_tool(&self, tool_call: &ToolCall) -> ToolResult;
}

#[derive(Clone, Debug)]
pub struct StubActionBackend {
    platform: &'static str,
}

impl StubActionBackend {
    pub fn new(platform: &'static str) -> Self {
        Self { platform }
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
