pub mod orchestrator;
pub mod policy;
pub mod tool_registry;

use actions::traits::StubActionBackend;
use ipc::{ActionEvent, ChatApproveRequest, ChatDenyRequest, ChatRequest, ChatResponse, ChatService, ConsentRequest, Tool};
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
    synthetic_audit_counter: u64,
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
            synthetic_audit_counter: 0,
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

    fn next_synthetic_audit_id(&mut self) -> String {
        self.synthetic_audit_counter += 1;
        format!("audit-local-{:06}", self.synthetic_audit_counter)
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
            response.consent_request = Some(build_consent_request(&response.proposed_actions));
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

    fn chat_deny(&mut self, params: ChatDenyRequest) -> Result<ChatResponse, String> {
        let request = self
            .pending_approvals
            .remove(&params.consent_token)
            .ok_or_else(|| "unknown or expired consent_token".to_string())?;

        self.rebuild_orchestrator(&request.provider_config.provider_name);
        let mut response = self
            .orchestrator
            .run(request.messages.clone(), request.provider_config.clone(), request.mode.clone());

        let mut denied_any = false;
        for evt in &mut response.proposed_actions {
            if evt.status == "consent_required" {
                evt.status = "denied".to_string();
                if evt.reason.is_none() {
                    evt.reason = Some("Denied by user".to_string());
                }
                denied_any = true;
            }
        }
        if denied_any {
            response.final_text = "User denied consent for requested actions.".to_string();
            response.consent_token = None;
            response.consent_request = None;
            response.actions_executed = response
                .proposed_actions
                .iter()
                .filter(|evt| evt.status == "denied")
                .map(|evt| {
                    format!(
                        "denied:{}:{}",
                        evt.tool_name,
                        evt.reason.clone().unwrap_or_else(|| "Denied by user".to_string())
                    )
                })
                .collect();
            response.executed_action_events.clear();
            response.action_events = response.proposed_actions.clone();
            response.audit_id = self.next_synthetic_audit_id();
        }

        Ok(response)
    }

    fn tools_list(&self) -> Vec<Tool> {
        self.tool_registry.list()
    }
}

fn build_consent_request(proposed_actions: &[ActionEvent]) -> ConsentRequest {
    let pending: Vec<&ActionEvent> = proposed_actions
        .iter()
        .filter(|evt| evt.status == "consent_required")
        .collect();
    let requires_extra_confirmation_click = pending.iter().any(|evt| {
        matches!(
            evt.capability_tier.as_str(),
            "LocalActions" | "SystemActions"
        )
    });

    let mut risk_factors = Vec::new();
    if pending.iter().any(|evt| evt.capability_tier == "LocalActions") {
        risk_factors.push("local_device_action".to_string());
    }
    if pending.iter().any(|evt| evt.capability_tier == "SystemActions") {
        risk_factors.push("system_level_action".to_string());
    }
    if pending.len() > 1 {
        risk_factors.push("multiple_actions_requested".to_string());
    }
    if pending.iter().any(|evt| evt.arguments_preview.as_deref().map(|s| s.len()).unwrap_or(0) > 0) {
        risk_factors.push("external_arguments_present".to_string());
    }

    let human_summary = if pending.is_empty() {
        "No consent-required actions pending.".to_string()
    } else if pending.len() == 1 {
        format!(
            "Approve execution of '{}' once for this exact request.",
            pending[0].tool_name
        )
    } else {
        format!(
            "Approve execution of {} actions once for this exact request.",
            pending.len()
        )
    };

    ConsentRequest {
        scope: "once_exact_request".to_string(),
        human_summary,
        risk_factors,
        requires_extra_confirmation_click,
    }
}
