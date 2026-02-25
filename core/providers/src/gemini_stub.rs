use ipc::{ChatMessage, ProviderConfig, Tool, ToolResult};

use crate::provider_trait::{Provider, ProviderReply};

pub struct GeminiStubProvider;

impl Provider for GeminiStubProvider {
    fn name(&self) -> &'static str {
        "gemini-stub"
    }

    fn chat(
        &self,
        _messages: &[ChatMessage],
        _tools: &[Tool],
        _tool_results: &[ToolResult],
        _config: &ProviderConfig,
    ) -> ProviderReply {
        ProviderReply::FinalText(
            "Gemini stub provider TODO: add real implementation with policy-aware tool calling.".to_string(),
        )
    }
}
