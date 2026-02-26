use std::env;

use ipc::{ChatMessage, ProviderConfig, Tool, ToolResult};
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
        _tools: &[Tool],
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

        let text = payload
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                "OpenAI provider returned no text content. Tool-calling integration for the real provider is not implemented yet."
                    .to_string()
            });

        ProviderReply::FinalText(text)
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

