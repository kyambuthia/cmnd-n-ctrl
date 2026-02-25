use actions::traits::ActionBackend;
use ipc::{ChatMessage, ChatMode, ChatResponse, ProviderConfig, ToolResult};
use providers::provider_trait::{Provider, ProviderReply};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::policy::{Authorization, CapabilityTier, Policy, PolicyContext};
use crate::tool_registry::ToolRegistry;

#[derive(Clone, Debug)]
pub struct PolicyDecisionRecord {
    pub tool_name: String,
    pub capability_tier: CapabilityTier,
    pub decision: String,
    pub reason: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AuditEvent {
    pub audit_id: String,
    pub timestamp_unix_seconds: u64,
    pub provider: String,
    pub tool_calls_requested: Vec<String>,
    pub tool_calls_executed: Vec<String>,
    pub evidence_summaries: Vec<String>,
    pub policy_decisions: Vec<PolicyDecisionRecord>,
}

#[derive(Clone, Debug, Default)]
pub struct AuditLog {
    events: Vec<AuditEvent>,
}

impl AuditLog {
    pub fn push(&mut self, event: AuditEvent) {
        self.events.push(event);
    }

    pub fn events(&self) -> &[AuditEvent] {
        &self.events
    }
}

pub struct Orchestrator<P, A>
where
    P: Provider,
    A: ActionBackend,
{
    policy: Policy,
    tool_registry: ToolRegistry,
    provider: P,
    action_backend: A,
    audit_counter: u64,
    audit_log: AuditLog,
}

impl<P, A> Orchestrator<P, A>
where
    P: Provider,
    A: ActionBackend,
{
    pub fn new(policy: Policy, tool_registry: ToolRegistry, provider: P, action_backend: A) -> Self {
        Self {
            policy,
            tool_registry,
            provider,
            action_backend,
            audit_counter: 0,
            audit_log: AuditLog::default(),
        }
    }

    pub fn handle_user_message(
        &mut self,
        user_message: String,
        provider_config: ProviderConfig,
        mode: ChatMode,
    ) -> ChatResponse {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: user_message,
        }];
        self.run(messages, provider_config, mode)
    }

    pub fn run(
        &mut self,
        messages: Vec<ChatMessage>,
        provider_config: ProviderConfig,
        mode: ChatMode,
    ) -> ChatResponse {
        let audit_id = self.next_audit_id();
        let timestamp_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let tools = self.tool_registry.list();
        let mut executed_actions = Vec::new();
        let mut tool_results: Vec<ToolResult> = Vec::new();
        let mut requested_tool_calls = Vec::new();
        let mut policy_decisions = Vec::new();

        let first_reply = self.provider.chat(&messages, &tools, &tool_results, &provider_config);
        let final_text = match first_reply {
            ProviderReply::FinalText(text) => text,
            ProviderReply::ToolCalls(calls) => {
                let mut pending_confirmation = false;
                for call in calls {
                    requested_tool_calls.push(call.name.clone());
                    if !self.tool_registry.has_tool(&call.name) {
                        executed_actions.push(format!("denied:{}:unknown_tool", call.name));
                        policy_decisions.push(PolicyDecisionRecord {
                            tool_name: call.name,
                            capability_tier: CapabilityTier::SystemActions,
                            decision: "deny".to_string(),
                            reason: Some("unknown_tool".to_string()),
                        });
                        continue;
                    }
                    let tier = self.policy.capability_tier(&call);
                    let auth = self.policy.authorize(
                        &call,
                        &PolicyContext {
                            mode: mode.clone(),
                            user_confirmed: false,
                        },
                    );
                    match auth {
                        Authorization::Allow => {
                            let result = self.action_backend.execute_tool(&call);
                            executed_actions.push(call.name.clone());
                            policy_decisions.push(PolicyDecisionRecord {
                                tool_name: call.name.clone(),
                                capability_tier: tier,
                                decision: "allow".to_string(),
                                reason: None,
                            });
                            tool_results.push(result);
                        }
                        Authorization::RequireConfirmation { reason } => {
                            pending_confirmation = true;
                            policy_decisions.push(PolicyDecisionRecord {
                                tool_name: call.name.clone(),
                                capability_tier: tier,
                                decision: "require_confirmation".to_string(),
                                reason: Some(reason.clone()),
                            });
                            executed_actions.push(format!("confirm_required:{}:{}", call.name, reason));
                        }
                        Authorization::Deny { reason } => {
                            policy_decisions.push(PolicyDecisionRecord {
                                tool_name: call.name.clone(),
                                capability_tier: tier,
                                decision: "deny".to_string(),
                                reason: Some(reason.clone()),
                            });
                            executed_actions.push(format!("denied:{}:{}", call.name, reason));
                        }
                    }
                }

                if pending_confirmation && tool_results.is_empty() {
                    "Confirmation required before executing requested tools.".to_string()
                } else {
                    match self.provider.chat(&messages, &tools, &tool_results, &provider_config) {
                        ProviderReply::FinalText(text) => text,
                        ProviderReply::ToolCalls(_) => {
                            "Provider requested additional tool loop; scaffold executes a single tool round.".to_string()
                        }
                    }
                }
            }
        };

        self.audit_log.push(AuditEvent {
            audit_id: audit_id.clone(),
            timestamp_unix_seconds,
            provider: provider_config.provider_name.clone(),
            tool_calls_requested: requested_tool_calls,
            tool_calls_executed: executed_actions.clone(),
            evidence_summaries: tool_results.into_iter().map(|r| r.evidence.summary).collect(),
            policy_decisions,
        });

        ChatResponse {
            final_text,
            audit_id,
            actions_executed: executed_actions,
        }
    }

    fn next_audit_id(&mut self) -> String {
        self.audit_counter += 1;
        format!("audit-{:06}", self.audit_counter)
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        self.audit_log.events()
    }
}
