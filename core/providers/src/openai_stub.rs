use ipc::{ChatMessage, ProviderConfig, Tool, ToolCall, ToolResult};
use serde_json::json;

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
    if !prompt.contains("tool:") {
        return None;
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:open") {
        if has_tool(tools, "desktop.open_url") {
            let url = rest.trim();
            return Some(ToolCall {
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
                name: "file.write_text".to_string(),
                arguments_json: json!({
                    "path": if path.is_empty() { "notes/generated.txt" } else { path },
                    "content": if content.is_empty() { "stub content" } else { content }
                })
                .to_string(),
            });
        }
    }

    if prompt.to_ascii_lowercase().contains("tool:apps") && has_tool(tools, "desktop.app.list") {
        return Some(ToolCall {
            name: "desktop.app.list".to_string(),
            arguments_json: json!({ "filter": "" }).to_string(),
        });
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:activate") {
        if has_tool(tools, "desktop.app.activate") {
            let app = rest.trim();
            return Some(ToolCall {
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
                name: "math.add".to_string(),
                arguments_json: json!({ "a": a, "b": b }).to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:upper") {
        if has_tool(tools, "text.uppercase") {
            let text = rest.trim();
            return Some(ToolCall {
                name: "text.uppercase".to_string(),
                arguments_json: json!({ "text": if text.is_empty() { "stub" } else { text } }).to_string(),
            });
        }
    }

    if let Some(rest) = slice_after_case_insensitive(prompt, "tool:echo") {
        if has_tool(tools, "echo") {
            let input = rest.trim();
            return Some(ToolCall {
                name: "echo".to_string(),
                arguments_json: json!({ "input": if input.is_empty() { "stub" } else { input } }).to_string(),
            });
        }
    }

    if prompt.to_ascii_lowercase().contains("tool:time") && has_tool(tools, "time.now") {
        return Some(ToolCall {
            name: "time.now".to_string(),
            arguments_json: "{}".to_string(),
        });
    }

    let first_tool = tools
        .first()
        .map(|t| t.name.clone())
        .unwrap_or_else(|| "echo".to_string());
    Some(ToolCall {
        name: first_tool,
        arguments_json: json!({ "input": "stub" }).to_string(),
    })
}

fn has_tool(tools: &[Tool], name: &str) -> bool {
    tools.iter().any(|t| t.name == name)
}

fn slice_after_case_insensitive<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    let lower = haystack.to_ascii_lowercase();
    let idx = lower.find(&needle.to_ascii_lowercase())?;
    Some(&haystack[idx + needle.len()..])
}
