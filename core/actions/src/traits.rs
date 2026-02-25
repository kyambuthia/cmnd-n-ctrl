use ipc::{ToolCall, ToolResult};

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
