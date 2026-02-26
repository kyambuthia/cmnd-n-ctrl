pub mod anthropic_stub;
pub mod gemini_stub;
pub mod openai_http;
pub mod openai_stub;
pub mod provider_trait;

use crate::anthropic_stub::AnthropicStubProvider;
use crate::gemini_stub::GeminiStubProvider;
use crate::openai_http::OpenAiHttpProvider;
use crate::openai_stub::OpenAiStubProvider;
use crate::provider_trait::Provider;

pub enum ProviderChoice {
    OpenAi(OpenAiHttpProvider),
    OpenAiStub(OpenAiStubProvider),
    Anthropic(AnthropicStubProvider),
    Gemini(GeminiStubProvider),
}

impl ProviderChoice {
    pub fn by_name(name: &str) -> Self {
        match name {
            "openai" => Self::OpenAi(OpenAiHttpProvider),
            "anthropic" | "anthropic-stub" => Self::Anthropic(AnthropicStubProvider),
            "gemini" | "gemini-stub" => Self::Gemini(GeminiStubProvider),
            _ => Self::OpenAiStub(OpenAiStubProvider),
        }
    }
}

impl Provider for ProviderChoice {
    fn name(&self) -> &'static str {
        match self {
            Self::OpenAi(inner) => inner.name(),
            Self::OpenAiStub(inner) => inner.name(),
            Self::Anthropic(inner) => inner.name(),
            Self::Gemini(inner) => inner.name(),
        }
    }

    fn chat(
        &self,
        messages: &[ipc::ChatMessage],
        tools: &[ipc::Tool],
        tool_results: &[ipc::ToolResult],
        config: &ipc::ProviderConfig,
    ) -> crate::provider_trait::ProviderReply {
        match self {
            Self::OpenAi(inner) => inner.chat(messages, tools, tool_results, config),
            Self::OpenAiStub(inner) => inner.chat(messages, tools, tool_results, config),
            Self::Anthropic(inner) => inner.chat(messages, tools, tool_results, config),
            Self::Gemini(inner) => inner.chat(messages, tools, tool_results, config),
        }
    }
}
