use ipc::{ChatMessage, ProviderConfig, Tool, ToolResult};

use crate::provider_trait::{Provider, ProviderReply};

pub struct AnthropicStubProvider;

impl Provider for AnthropicStubProvider {
    fn name(&self) -> &'static str {
        "anthropic-stub"
    }

    fn chat(
        &self,
        _messages: &[ChatMessage],
        _tools: &[Tool],
        _tool_results: &[ToolResult],
        _config: &ProviderConfig,
    ) -> ProviderReply {
        ProviderReply::FinalText(
            "Anthropic stub provider TODO: add SDK/plugin integration via JSON-RPC process plugin.".to_string(),
        )
    }
}
