use ipc::Tool;

#[derive(Clone, Debug, Default)]
pub struct ToolRegistry {
    tools: Vec<Tool>,
}

impl ToolRegistry {
    pub fn new_default() -> Self {
        Self {
            tools: vec![
                Tool {
                    name: "echo".to_string(),
                    description: "Echo a payload for testing tool orchestration".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"input\":{\"type\":\"string\"}},\"required\":[\"input\"]}".to_string(),
                },
                Tool {
                    name: "desktop.open_url".to_string(),
                    description: "Open a URL using the platform shell (stubbed)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"url\":{\"type\":\"string\"}},\"required\":[\"url\"]}".to_string(),
                },
            ],
        }
    }

    pub fn list(&self) -> Vec<Tool> {
        self.tools.clone()
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }
}
