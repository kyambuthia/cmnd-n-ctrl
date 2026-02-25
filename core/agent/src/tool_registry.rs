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
                    name: "time.now".to_string(),
                    description: "Return the current UTC timestamp from the local runtime".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{},\"additionalProperties\":false}".to_string(),
                },
                Tool {
                    name: "echo".to_string(),
                    description: "Echo a payload for testing tool orchestration".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"input\":{\"type\":\"string\"}},\"required\":[\"input\"]}".to_string(),
                },
                Tool {
                    name: "text.uppercase".to_string(),
                    description: "Uppercase a provided string locally".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"text\":{\"type\":\"string\"}},\"required\":[\"text\"]}".to_string(),
                },
                Tool {
                    name: "math.add".to_string(),
                    description: "Add two numbers locally".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"number\"},\"b\":{\"type\":\"number\"}},\"required\":[\"a\",\"b\"]}".to_string(),
                },
                Tool {
                    name: "desktop.open_url".to_string(),
                    description: "Open a URL using the platform shell (stubbed)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"url\":{\"type\":\"string\"}},\"required\":[\"url\"]}".to_string(),
                },
                Tool {
                    name: "desktop.app.list".to_string(),
                    description: "List desktop applications/windows (stubbed)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"filter\":{\"type\":\"string\"}},\"additionalProperties\":false}".to_string(),
                },
                Tool {
                    name: "desktop.app.activate".to_string(),
                    description: "Activate/focus a desktop application/window (stubbed)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"app\":{\"type\":\"string\"}},\"required\":[\"app\"]}".to_string(),
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
