use std::env;

use ipc::{ChatMessage, ProviderConfig, Tool, ToolCall, ToolResult};
use serde_json::{json, Value};

use crate::provider_trait::{Provider, ProviderReply};

pub struct OpenAiHttpProvider;

impl Provider for OpenAiHttpProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[Tool],
        tool_results: &[ToolResult],
        config: &ProviderConfig,
    ) -> ProviderReply {
        let api_key = match resolve_api_key() {
            Some(v) => v,
            None => {
                return ProviderReply::FinalText(
                    "OpenAI provider is selected but OPENAI_API_KEY (or OPENAI_API_KEY_FILE) is not configured."
                        .to_string(),
                )
            }
        };

        let base_url = env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com".to_string())
            .trim_end_matches('/')
            .to_string();
        let model = config
            .model
            .clone()
            .or_else(|| env::var("OPENAI_MODEL").ok())
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| "gpt-4.1-mini".to_string());

        let mut combined = messages.to_vec();
        if !tool_results.is_empty() {
            combined.push(ChatMessage {
                role: "system".to_string(),
                content: format!(
                    "Tool results (JSON): {}",
                    serde_json::to_string(tool_results).unwrap_or_else(|_| "[]".to_string())
                ),
            });
        }

        let body = json!({
            "model": model,
            "messages": combined
                .iter()
                .map(|m| json!({"role": m.role, "content": m.content}))
                .collect::<Vec<_>>(),
            "tools": build_openai_tools(tools),
            "tool_choice": "auto",
        });

        let url = format!("{base_url}/v1/chat/completions");
        let response = match ureq::post(&url)
            .set("Authorization", &format!("Bearer {}", api_key))
            .set("Content-Type", "application/json")
            .send_json(body)
        {
            Ok(resp) => resp,
            Err(err) => {
                return ProviderReply::FinalText(format!(
                    "OpenAI provider request failed: {err}. Use 'openai-stub' for offline testing."
                ))
            }
        };

        let payload: Value = match response.into_json() {
            Ok(v) => v,
            Err(err) => {
                return ProviderReply::FinalText(format!(
                    "OpenAI provider returned invalid JSON: {err}"
                ))
            }
        };

        interpret_chat_completion_payload(&payload)
    }
}

fn build_openai_tools(tools: &[Tool]) -> Vec<Value> {
    tools.iter()
        .map(|tool| {
            let parameters = serde_json::from_str::<Value>(&tool.input_json_schema)
                .unwrap_or_else(|_| json!({"type":"object"}));
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": parameters
                }
            })
        })
        .collect()
}

fn interpret_chat_completion_payload(payload: &Value) -> ProviderReply {
    let Some(message) = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|c| c.get("message"))
    else {
        return ProviderReply::FinalText("OpenAI provider returned no choices.".to_string());
    };

    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        let mut calls = Vec::new();
        for call in tool_calls {
            let Some(function) = call.get("function") else {
                continue;
            };
            let Some(name) = function.get("name").and_then(Value::as_str) else {
                continue;
            };
            let arguments_json = function
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}")
                .to_string();
            calls.push(ToolCall {
                name: name.to_string(),
                arguments_json,
            });
        }
        if !calls.is_empty() {
            return ProviderReply::ToolCalls(calls);
        }
    }

    let text = extract_message_text(message).unwrap_or_else(|| {
        "OpenAI provider returned no text content and no tool calls.".to_string()
    });
    ProviderReply::FinalText(text)
}

fn extract_message_text(message: &Value) -> Option<String> {
    if let Some(s) = message.get("content").and_then(Value::as_str) {
        if !s.trim().is_empty() {
            return Some(s.to_string());
        }
    }

    // Some APIs may return structured content arrays. Concatenate text fragments conservatively.
    let parts = message.get("content").and_then(Value::as_array)?;
    let mut text = String::new();
    for part in parts {
        if let Some(s) = part.get("text").and_then(Value::as_str) {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(s);
        }
    }
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn resolve_api_key() -> Option<String> {
    if let Ok(key) = env::var("OPENAI_API_KEY") {
        if !key.trim().is_empty() {
            return Some(key);
        }
    }
    let path = env::var("OPENAI_API_KEY_FILE").ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let key = raw.trim().to_string();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_trait::ProviderReply;

    #[test]
    fn interprets_tool_calls_from_chat_completions_payload() {
        let payload = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "file.list",
                            "arguments": "{\"path\":\".\"}"
                        }
                    }]
                }
            }]
        });

        match interpret_chat_completion_payload(&payload) {
            ProviderReply::ToolCalls(calls) => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].name, "file.list");
                assert_eq!(calls[0].arguments_json, "{\"path\":\".\"}");
            }
            ProviderReply::FinalText(text) => panic!("expected tool calls, got text: {text}"),
        }
    }

    #[test]
    fn interprets_text_from_chat_completions_payload() {
        let payload = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "hello world"
                }
            }]
        });

        match interpret_chat_completion_payload(&payload) {
            ProviderReply::FinalText(text) => assert_eq!(text, "hello world"),
            ProviderReply::ToolCalls(_) => panic!("expected final text"),
        }
    }
}
