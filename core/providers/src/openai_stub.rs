use ipc::{ChatMessage, ProviderConfig, Tool, ToolCall, ToolResult};
use serde_json::{json, Value};

use crate::provider_trait::{Provider, ProviderReply};

pub struct OpenAiStubProvider;

impl Provider for OpenAiStubProvider {
    fn name(&self) -> &'static str {
        "openai-stub"
    }

    fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[Tool],
        tool_results: &[ToolResult],
        _config: &ProviderConfig,
    ) -> ProviderReply {
        if tool_results.is_empty() {
            if let Some(last) = messages.last() {
                if let Some(call) = select_stub_tool_call(&last.content, tools) {
                    return ProviderReply::ToolCalls(vec![call]);
                }
            }
        }

        let suffix = if tool_results.is_empty() {
            "(no tools used)"
        } else {
            "(tool results incorporated)"
        };
        ProviderReply::FinalText(format!(
            "OpenAI stub response {}. TODO: integrate real API client without embedding keys.",
            suffix
        ))
    }
}

fn select_stub_tool_call(prompt: &str, tools: &[Tool]) -> Option<ToolCall> {
    if let Some(call) = select_natural_language_tool_call(prompt, tools) {
        return Some(call);
    }
    if prompt.to_ascii_lowercase().contains("tool:") {
        return select_legacy_tool_syntax_call(prompt, tools);
    }
    None
}

fn select_natural_language_tool_call(prompt: &str, tools: &[Tool]) -> Option<ToolCall> {
    if has_tool(tools, "desktop.open_url") {
        if let Some(url) = extract_url(prompt) {
            let lower = prompt.to_ascii_lowercase();
            if lower.contains("open")
                || lower.contains("launch")
                || lower.contains("navigate")
                || lower.contains("visit")
            {
                return Some(ToolCall {
                    tool_call_id: None,
                    name: "desktop.open_url".to_string(),
                    arguments_json: json!({ "url": url }).to_string(),
                });
            }
        }
    }

    if has_tool(tools, "desktop.app.activate") {
        if let Some(rest) = slice_after_case_insensitive(prompt, "activate ") {
            let app = rest.trim();
            if !app.is_empty() {
                return Some(ToolCall {
                    tool_call_id: None,
                    name: "desktop.app.activate".to_string(),
                    arguments_json: json!({ "app": app }).to_string(),
                });
            }
        }
        if let Some(rest) = slice_after_case_insensitive(prompt, "focus ") {
            let app = rest.trim();
            if !app.is_empty() {
                return Some(ToolCall {
                    tool_call_id: None,
                    name: "desktop.app.activate".to_string(),
                    arguments_json: json!({ "app": app }).to_string(),
                });
            }
        }
    }

    if has_tool(tools, "file.list") {
        let lower = prompt.to_ascii_lowercase();
        if lower.contains("list files") || lower.contains("show files") || lower.contains("what files") {
            return Some(ToolCall {
                tool_call_id: None,
                name: "file.list".to_string(),
                arguments_json: json!({ "path": "." }).to_string(),
            });
        }
    }

    if has_tool(tools, "file.read_text") {
        if let Some(rest) = slice_after_case_insensitive(prompt, "read file ") {
            let path = first_token(rest);
            if !path.is_empty() {
                return Some(ToolCall {
                    tool_call_id: None,
                    name: "file.read_text".to_string(),
                    arguments_json: json!({ "path": path }).to_string(),
                });
            }
        }
    }

    if has_tool(tools, "file.mkdir") {
        if let Some(rest) = slice_after_case_insensitive(prompt, "create directory ") {
            let path = rest.trim();
            if !path.is_empty() {
                return Some(ToolCall {
                    tool_call_id: None,
                    name: "file.mkdir".to_string(),
                    arguments_json: json!({ "path": path }).to_string(),
                });
            }
        }
        if let Some(rest) = slice_after_case_insensitive(prompt, "create folder ") {
            let path = rest.trim();
            if !path.is_empty() {
                return Some(ToolCall {
                    tool_call_id: None,
                    name: "file.mkdir".to_string(),
                    arguments_json: json!({ "path": path }).to_string(),
                });
            }
        }
    }

    if has_tool(tools, "mcp.tool_call") {
        let lower = prompt.to_ascii_lowercase();
        if lower.contains("mcp") && lower.contains("server") && lower.contains("tool") {
            let server_id = token_after(prompt, "server").unwrap_or("mcp-000001");
            let tool_name = token_after(prompt, "tool").unwrap_or("echo");
            return Some(ToolCall {
                tool_call_id: None,
                name: "mcp.tool_call".to_string(),
                arguments_json: json!({
                    "server_id": server_id,
                    "tool_name": tool_name,
                    "arguments": {}
                })
                .to_string(),
            });
        }
    }

    if prompt.to_ascii_lowercase().contains("what time") && has_tool(tools, "time.now") {
        return Some(ToolCall {
            tool_call_id: None,
            name: "time.now".to_string(),
            arguments_json: "{}".to_string(),
        });
    }

    None
}

fn select_legacy_tool_syntax_call(prompt: &str, tools: &[Tool]) -> Option<ToolCall> {
    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:open") {
        if has_tool(tools, "desktop.open_url") {
            let url = rest.trim();
            return Some(ToolCall {
                tool_call_id: None,
                name: "desktop.open_url".to_string(),
                arguments_json: json!({
                    "url": if url.is_empty() { "https://example.com" } else { url }
                })
                .to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:ls") {
        if has_tool(tools, "file.list") {
            let path = rest.trim();
            return Some(ToolCall {
                tool_call_id: None,
                name: "file.list".to_string(),
                arguments_json: json!({
                    "path": if path.is_empty() { "." } else { path }
                })
                .to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:cat") {
        if has_tool(tools, "file.read_text") {
            let path = rest.trim();
            return Some(ToolCall {
                tool_call_id: None,
                name: "file.read_text".to_string(),
                arguments_json: json!({
                    "path": if path.is_empty() { "README.md" } else { path }
                })
                .to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:csv") {
        if has_tool(tools, "file.read_csv") {
            let path = rest.trim();
            return Some(ToolCall {
                tool_call_id: None,
                name: "file.read_csv".to_string(),
                arguments_json: json!({
                    "path": if path.is_empty() { "data.csv" } else { path },
                    "limit": 10
                })
                .to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:json") {
        if has_tool(tools, "file.read_json") {
            let path = rest.trim();
            return Some(ToolCall {
                tool_call_id: None,
                name: "file.read_json".to_string(),
                arguments_json: json!({
                    "path": if path.is_empty() { "package.json" } else { path }
                })
                .to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:stat") {
        if has_tool(tools, "file.stat") {
            let path = rest.trim();
            return Some(ToolCall {
                tool_call_id: None,
                name: "file.stat".to_string(),
                arguments_json: json!({
                    "path": if path.is_empty() { "." } else { path }
                })
                .to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:grep") {
        if has_tool(tools, "file.search_text") {
            let mut parts = rest.trim().splitn(2, " in ");
            let query = parts.next().unwrap_or("").trim();
            let path = parts.next().unwrap_or(".").trim();
            return Some(ToolCall {
                tool_call_id: None,
                name: "file.search_text".to_string(),
                arguments_json: json!({
                    "query": if query.is_empty() { "TODO" } else { query },
                    "path": if path.is_empty() { "." } else { path },
                    "limit": 25
                })
                .to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:write") {
        if has_tool(tools, "file.write_text") {
            let (path, content) = rest.split_once("::").map(|(a, b)| (a.trim(), b.trim())).unwrap_or(("notes/generated.txt", rest.trim()));
            return Some(ToolCall {
                tool_call_id: None,
                name: "file.write_text".to_string(),
                arguments_json: json!({
                    "path": if path.is_empty() { "notes/generated.txt" } else { path },
                    "content": if content.is_empty() { "stub content" } else { content }
                })
                .to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:append") {
        if has_tool(tools, "file.append_text") {
            let (path, content) = rest
                .split_once("::")
                .map(|(a, b)| (a.trim(), b.trim()))
                .unwrap_or(("notes/generated.txt", rest.trim()));
            return Some(ToolCall {
                tool_call_id: None,
                name: "file.append_text".to_string(),
                arguments_json: json!({
                    "path": if path.is_empty() { "notes/generated.txt" } else { path },
                    "content": if content.is_empty() { "stub append" } else { content }
                })
                .to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:mkdir") {
        if has_tool(tools, "file.mkdir") {
            let path = rest.trim();
            return Some(ToolCall {
                tool_call_id: None,
                name: "file.mkdir".to_string(),
                arguments_json: json!({
                    "path": if path.is_empty() { "notes" } else { path }
                })
                .to_string(),
            });
        }
    }

    if prompt.to_ascii_lowercase().contains("tool:apps") && has_tool(tools, "desktop.app.list") {
        return Some(ToolCall {
            tool_call_id: None,
            name: "desktop.app.list".to_string(),
            arguments_json: json!({ "filter": "" }).to_string(),
        });
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:mcp") {
        if has_tool(tools, "mcp.tool_call") {
            let trimmed = rest.trim();
            let (server_id, tool_name, arguments_json) = trimmed
                .split_once("::")
                .and_then(|(sid, tail)| {
                    tail.split_once("::")
                        .map(|(tool, args)| (sid.trim(), tool.trim(), args.trim()))
                })
                .unwrap_or(("mcp-000001", "echo", "{}"));
            let args_value = serde_json::from_str::<Value>(arguments_json).unwrap_or_else(|_| json!({}));
            return Some(ToolCall {
                tool_call_id: None,
                name: "mcp.tool_call".to_string(),
                arguments_json: json!({
                    "server_id": if server_id.is_empty() { "mcp-000001" } else { server_id },
                    "tool_name": if tool_name.is_empty() { "echo" } else { tool_name },
                    "arguments": args_value
                })
                .to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:activate") {
        if has_tool(tools, "desktop.app.activate") {
            let app = rest.trim();
            return Some(ToolCall {
                tool_call_id: None,
                name: "desktop.app.activate".to_string(),
                arguments_json: json!({ "app": if app.is_empty() { "Browser" } else { app } }).to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:add") {
        if has_tool(tools, "math.add") {
            let mut nums = rest
                .split_whitespace()
                .filter_map(|s| s.parse::<f64>().ok());
            let a = nums.next().unwrap_or(2.0);
            let b = nums.next().unwrap_or(3.0);
            return Some(ToolCall {
                tool_call_id: None,
                name: "math.add".to_string(),
                arguments_json: json!({ "a": a, "b": b }).to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:upper") {
        if has_tool(tools, "text.uppercase") {
            let text = rest.trim();
            return Some(ToolCall {
                tool_call_id: None,
                name: "text.uppercase".to_string(),
                arguments_json: json!({ "text": if text.is_empty() { "stub" } else { text } }).to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:echo") {
        if has_tool(tools, "echo") {
            let input = rest.trim();
            return Some(ToolCall {
                tool_call_id: None,
                name: "echo".to_string(),
                arguments_json: json!({ "input": if input.is_empty() { "stub" } else { input } }).to_string(),
            });
        }
    }

    if prompt.to_ascii_lowercase().contains("tool:time") && has_tool(tools, "time.now") {
        return Some(ToolCall {
            tool_call_id: None,
            name: "time.now".to_string(),
            arguments_json: "{}".to_string(),
        });
    }

    None
}

fn has_tool(tools: &[Tool], name: &str) -> bool {
    tools.iter().any(|t| t.name == name)
}

fn slice_after_case_insensitive<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    let lower = haystack.to_ascii_lowercase();
    let idx = lower.find(&needle.to_ascii_lowercase())?;
    Some(&haystack[idx + needle.len()..])
}

fn extract_url(input: &str) -> Option<String> {
    input
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| ",.!?\"'()[]{}".contains(c)))
        .find(|t| t.starts_with("https://") || t.starts_with("http://"))
        .map(|s| s.to_string())
}

fn first_token(input: &str) -> String {
    input
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(|c: char| ",.!?\"'()[]{}".contains(c))
        .to_string()
}

fn token_after<'a>(input: &'a str, keyword: &str) -> Option<&'a str> {
    let lower = input.to_ascii_lowercase();
    let needle = format!("{keyword} ");
    let idx = lower.find(&needle)?;
    input[idx + needle.len()..].split_whitespace().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            description: "".to_string(),
            input_json_schema: "{}".to_string(),
        }
    }

    #[test]
    fn natural_language_open_url_maps_to_desktop_tool() {
        let tools = vec![tool("desktop.open_url")];
        let call = select_stub_tool_call("Please open https://example.com for me", &tools).expect("tool call");
        assert_eq!(call.name, "desktop.open_url");
        assert!(call.arguments_json.contains("https://example.com"));
    }

    #[test]
    fn legacy_tool_syntax_still_supported_for_non_ui_callers() {
        let tools = vec![tool("desktop.open_url")];
        let call = select_stub_tool_call("tool:open https://example.com", &tools).expect("tool call");
        assert_eq!(call.name, "desktop.open_url");
    }
}
