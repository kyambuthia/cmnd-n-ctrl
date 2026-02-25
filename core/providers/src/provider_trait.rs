use ipc::{ChatMessage, ProviderConfig, Tool, ToolCall, ToolResult};

#[derive(Clone, Debug)]
pub enum ProviderReply {
    FinalText(String),
    ToolCalls(Vec<ToolCall>),
}

pub trait Provider {
    fn name(&self) -> &'static str;

    fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[Tool],
        tool_results: &[ToolResult],
        config: &ProviderConfig,
    ) -> ProviderReply;
}
