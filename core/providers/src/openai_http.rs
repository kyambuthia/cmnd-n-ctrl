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
        let provider_cfg = ProviderRuntimeConfig::from_provider_config(config);
        let api_key = match resolve_api_key(config) {
            Some(v) => v,
            None => {
                return ProviderReply::FinalText(
                    "OpenAI-compatible provider is selected but no API key was found in provider config or environment."
                        .to_string(),
                )
            }
        };

        let base_url = provider_cfg
            .base_url
            .or_else(|| env::var("OPENAI_BASE_URL").ok())
            .unwrap_or_else(|| "https://api.openai.com".to_string())
            .trim_end_matches('/')
            .to_string();
        let model = provider_cfg
            .model
            .or_else(|| config.model.clone())
            .or_else(|| env::var("OPENAI_MODEL").ok())
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| "gpt-4.1-mini".to_string());

        let body = json!({
            "model": model,
            "messages": build_openai_messages(messages, tool_results),
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
            let tool_call_id = call
                .get("id")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            let Some(name) = function.get("name").and_then(Value::as_str) else {
                continue;
            };
            let arguments_json = function
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}")
                .to_string();
            calls.push(ToolCall {
                tool_call_id,
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

fn build_openai_messages(messages: &[ChatMessage], tool_results: &[ToolResult]) -> Vec<Value> {
    let mut out = messages
        .iter()
        .map(|m| json!({"role": m.role, "content": m.content}))
        .collect::<Vec<_>>();

    if tool_results.is_empty() {
        return out;
    }

    // Reconstruct a minimal assistant tool-call message so OpenAI-compatible APIs can accept the
    // subsequent tool role messages on the next round.
    let tool_calls = tool_results
        .iter()
        .enumerate()
        .map(|(idx, r)| {
            let call_id = r
                .tool_call_id
                .clone()
                .unwrap_or_else(|| format!("call_stub_{idx}"));
            json!({
                "id": call_id,
                "type": "function",
                "function": {
                    "name": r.name,
                    "arguments": "{}"
                }
            })
        })
        .collect::<Vec<_>>();
    out.push(json!({
        "role": "assistant",
        "content": null,
        "tool_calls": tool_calls
    }));

    for (idx, r) in tool_results.iter().enumerate() {
        let call_id = r
            .tool_call_id
            .clone()
            .unwrap_or_else(|| format!("call_stub_{idx}"));
        out.push(json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": r.result_json
        }));
    }

    out
}

#[derive(Debug, Default)]
struct ProviderRuntimeConfig {
    base_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    api_key_env: Option<String>,
}

impl ProviderRuntimeConfig {
    fn from_provider_config(config: &ProviderConfig) -> Self {
        let mut out = Self::default();
        let Some(raw) = config.config_json.as_deref() else {
            return out;
        };
        let Ok(v) = serde_json::from_str::<Value>(raw) else {
            return out;
        };
        out.base_url = v.get("base_url").and_then(Value::as_str).map(|s| s.to_string());
        out.model = v.get("model").and_then(Value::as_str).map(|s| s.to_string());
        out.api_key = v.get("api_key").and_then(Value::as_str).map(|s| s.to_string());
        out.api_key_env = v
            .get("api_key_env")
            .or_else(|| v.get("token_env"))
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        out
    }
}

fn resolve_api_key(config: &ProviderConfig) -> Option<String> {
    let runtime = ProviderRuntimeConfig::from_provider_config(config);
    if let Some(key) = runtime.api_key.filter(|s| !s.trim().is_empty()) {
        return Some(key);
    }
    if let Some(env_name) = runtime.api_key_env {
        if let Ok(key) = env::var(env_name) {
            if !key.trim().is_empty() {
                return Some(key);
            }
        }
    }
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
                assert_eq!(calls[0].tool_call_id.as_deref(), Some("call_1"));
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

    #[test]
    fn provider_runtime_config_reads_alias_settings() {
        let cfg = ProviderConfig {
            provider_name: "ollama-local".to_string(),
            model: None,
            config_json: Some(
                r#"{"base_url":"http://127.0.0.1:11434/v1","model":"qwen","api_key_env":"OLLAMA_TOKEN"}"#
                    .to_string(),
            ),
        };
        let parsed = ProviderRuntimeConfig::from_provider_config(&cfg);
        assert_eq!(parsed.base_url.as_deref(), Some("http://127.0.0.1:11434/v1"));
        assert_eq!(parsed.model.as_deref(), Some("qwen"));
        assert_eq!(parsed.api_key_env.as_deref(), Some("OLLAMA_TOKEN"));
    }
}
