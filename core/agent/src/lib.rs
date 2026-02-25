pub mod orchestrator;
pub mod policy;
pub mod tool_registry;

use actions::traits::StubActionBackend;
use ipc::{ChatRequest, ChatResponse, ChatService, Tool};
use providers::ProviderChoice;

use crate::orchestrator::Orchestrator;
use crate::policy::Policy;
use crate::tool_registry::ToolRegistry;

pub struct AgentService {
    orchestrator: Orchestrator<ProviderChoice, StubActionBackend>,
    tool_registry: ToolRegistry,
}

impl AgentService {
    pub fn new_for_platform(platform: &'static str) -> Self {
        let tool_registry = ToolRegistry::new_default();
        let orchestrator = Orchestrator::new(
            Policy::default(),
            tool_registry.clone(),
            ProviderChoice::by_name("openai-stub"),
            StubActionBackend::new(platform),
        );
        Self {
            orchestrator,
            tool_registry,
        }
    }
}

impl ChatService for AgentService {
    fn chat_request(&mut self, params: ChatRequest) -> ChatResponse {
        let provider = ProviderChoice::by_name(&params.provider_config.provider_name);
        self.orchestrator = Orchestrator::new(
            Policy::default(),
            self.tool_registry.clone(),
            provider,
            StubActionBackend::new("cli"),
        );
        self.orchestrator
            .run(params.messages, params.provider_config, params.mode)
    }

    fn tools_list(&self) -> Vec<Tool> {
        self.tool_registry.list()
    }
}
