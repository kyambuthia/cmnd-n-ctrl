use ipc::{ChatMessage, ProviderConfig, Tool, ToolCall, ToolResult};

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
                if last.content.contains("tool:") {
                    let first_tool = tools.first().map(|t| t.name.clone()).unwrap_or_else(|| "echo".to_string());
                    return ProviderReply::ToolCalls(vec![ToolCall {
                        name: first_tool,
                        arguments_json: "{\"input\":\"stub\"}".to_string(),
                    }]);
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
