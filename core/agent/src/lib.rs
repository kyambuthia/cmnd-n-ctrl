pub mod orchestrator;
pub mod policy;
pub mod tool_registry;

use actions::traits::StubActionBackend;
use ipc::{ChatApproveRequest, ChatRequest, ChatResponse, ChatService, Tool};
use providers::ProviderChoice;
use std::collections::HashMap;

use crate::orchestrator::Orchestrator;
use crate::policy::Policy;
use crate::tool_registry::ToolRegistry;

pub struct AgentService {
    orchestrator: Orchestrator<ProviderChoice, StubActionBackend>,
    tool_registry: ToolRegistry,
    pending_approvals: HashMap<String, ChatRequest>,
    consent_counter: u64,
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
            pending_approvals: HashMap::new(),
            consent_counter: 0,
        }
    }

    fn rebuild_orchestrator(&mut self, provider_name: &str) {
        let provider = ProviderChoice::by_name(provider_name);
        self.orchestrator = Orchestrator::new(
            Policy::default(),
            self.tool_registry.clone(),
            provider,
            StubActionBackend::new("cli"),
        );
    }

    fn issue_consent_token(&mut self, request: ChatRequest) -> String {
        self.consent_counter += 1;
        let token = format!("consent-{:06}", self.consent_counter);
        self.pending_approvals.insert(token.clone(), request);
        token
    }
}

impl ChatService for AgentService {
    fn chat_request(&mut self, params: ChatRequest) -> ChatResponse {
        self.rebuild_orchestrator(&params.provider_config.provider_name);
        let mut response = self
            .orchestrator
            .run(params.messages.clone(), params.provider_config.clone(), params.mode.clone());
        if response
            .proposed_actions
            .iter()
            .any(|evt| evt.status == "consent_required")
        {
            response.consent_token = Some(self.issue_consent_token(params));
        }
        response
    }

    fn chat_approve(&mut self, params: ChatApproveRequest) -> Result<ChatResponse, String> {
        let request = self
            .pending_approvals
            .remove(&params.consent_token)
            .ok_or_else(|| "unknown or expired consent_token".to_string())?;
        self.rebuild_orchestrator(&request.provider_config.provider_name);
        Ok(self.orchestrator.run_with_confirmation(
            request.messages,
            request.provider_config,
            request.mode,
            true,
        ))
    }

    fn tools_list(&self) -> Vec<Tool> {
        self.tool_registry.list()
    }
}
