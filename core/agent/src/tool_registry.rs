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
                    name: "file.list".to_string(),
                    description: "List files in the current project (read-only)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"}},\"additionalProperties\":false}".to_string(),
                },
                Tool {
                    name: "file.read_text".to_string(),
                    description: "Read a text file from the current project (read-only)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"}},\"required\":[\"path\"]}".to_string(),
                },
                Tool {
                    name: "file.read_csv".to_string(),
                    description: "Read a CSV file from the current project (read-only, preview rows)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"},\"limit\":{\"type\":\"integer\",\"minimum\":1}},\"required\":[\"path\"]}".to_string(),
                },
                Tool {
                    name: "file.read_json".to_string(),
                    description: "Read and parse a JSON file from the current project (read-only)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"}},\"required\":[\"path\"]}".to_string(),
                },
                Tool {
                    name: "file.search_text".to_string(),
                    description: "Search project files for a text query (read-only, scoped)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"query\":{\"type\":\"string\"},\"path\":{\"type\":\"string\"},\"limit\":{\"type\":\"integer\",\"minimum\":1}},\"required\":[\"query\"]}".to_string(),
                },
                Tool {
                    name: "file.stat".to_string(),
                    description: "Get metadata for a project-scoped file or directory (read-only)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"}},\"required\":[\"path\"]}".to_string(),
                },
                Tool {
                    name: "file.write_text".to_string(),
                    description: "Write a text file under the current project root (consent required)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"},\"content\":{\"type\":\"string\"}},\"required\":[\"path\",\"content\"]}".to_string(),
                },
                Tool {
                    name: "file.append_text".to_string(),
                    description: "Append text to a file under the current project root (consent required)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"},\"content\":{\"type\":\"string\"}},\"required\":[\"path\",\"content\"]}".to_string(),
                },
                Tool {
                    name: "file.mkdir".to_string(),
                    description: "Create a directory under the current project root (consent required)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"}},\"required\":[\"path\"]}".to_string(),
                },
                Tool {
                    name: "mcp.tool_call".to_string(),
                    description: "Call a tool on a running MCP server (consent required)".to_string(),
                    input_json_schema: "{\"type\":\"object\",\"properties\":{\"server_id\":{\"type\":\"string\"},\"tool_name\":{\"type\":\"string\"},\"arguments\":{\"type\":\"object\"}},\"required\":[\"server_id\",\"tool_name\"],\"additionalProperties\":false}".to_string(),
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

    pub fn from_tools(tools: Vec<Tool>) -> Self {
        Self { tools }
    }

    pub fn list(&self) -> Vec<Tool> {
        self.tools.clone()
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }
}
