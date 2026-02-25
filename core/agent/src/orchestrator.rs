use actions::traits::ActionBackend;
use ipc::{ActionEvent, ChatMessage, ChatMode, ChatResponse, ProviderConfig, ToolResult};
use providers::provider_trait::{Provider, ProviderReply};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
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
        self.run_with_confirmation(messages, provider_config, mode, false)
    }

    pub fn run_with_confirmation(
        &mut self,
        messages: Vec<ChatMessage>,
        provider_config: ProviderConfig,
        mode: ChatMode,
        user_confirmed: bool,
    ) -> ChatResponse {
        let audit_id = self.next_audit_id();
        let request_fingerprint = request_fingerprint(&messages, &provider_config, &mode);
        let timestamp_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let tools = self.tool_registry.list();
        let mut executed_actions = Vec::new();
        let mut proposed_actions: Vec<ActionEvent> = Vec::new();
        let mut executed_action_events: Vec<ActionEvent> = Vec::new();
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
                        proposed_actions.push(ActionEvent {
                            tool_name: call.name.clone(),
                            capability_tier: capability_tier_label(&CapabilityTier::SystemActions),
                            status: "denied".to_string(),
                            reason: Some("unknown_tool".to_string()),
                            arguments_preview: Some(arguments_preview(&call.arguments_json)),
                            evidence_summary: None,
                        });
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
                            user_confirmed,
                        },
                    );
                    match auth {
                        Authorization::Allow => {
                            proposed_actions.push(ActionEvent {
                                tool_name: call.name.clone(),
                                capability_tier: capability_tier_label(&tier),
                                status: "approved".to_string(),
                                reason: None,
                                arguments_preview: Some(arguments_preview(&call.arguments_json)),
                                evidence_summary: None,
                            });
                            let result = self.action_backend.execute_tool(&call);
                            let evidence_summary = result.evidence.summary.clone();
                            executed_actions.push(call.name.clone());
                            executed_action_events.push(ActionEvent {
                                tool_name: call.name.clone(),
                                capability_tier: capability_tier_label(&tier),
                                status: "executed".to_string(),
                                reason: None,
                                arguments_preview: Some(arguments_preview(&call.arguments_json)),
                                evidence_summary: Some(evidence_summary),
                            });
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
                            proposed_actions.push(ActionEvent {
                                tool_name: call.name.clone(),
                                capability_tier: capability_tier_label(&tier),
                                status: "consent_required".to_string(),
                                reason: Some(reason.clone()),
                                arguments_preview: Some(arguments_preview(&call.arguments_json)),
                                evidence_summary: None,
                            });
                            policy_decisions.push(PolicyDecisionRecord {
                                tool_name: call.name.clone(),
                                capability_tier: tier,
                                decision: "require_confirmation".to_string(),
                                reason: Some(reason.clone()),
                            });
                            executed_actions.push(format!("confirm_required:{}:{}", call.name, reason));
                        }
                        Authorization::Deny { reason } => {
                            proposed_actions.push(ActionEvent {
                                tool_name: call.name.clone(),
                                capability_tier: capability_tier_label(&tier),
                                status: "denied".to_string(),
                                reason: Some(reason.clone()),
                                arguments_preview: Some(arguments_preview(&call.arguments_json)),
                                evidence_summary: None,
                            });
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

        let mut action_events = proposed_actions.clone();
        action_events.extend(executed_action_events.clone());

        ChatResponse {
            final_text,
            audit_id,
            request_fingerprint,
            consent_token: None,
            session_id: None,
            consent_request: None,
            actions_executed: executed_actions,
            proposed_actions,
            executed_action_events,
            action_events,
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

fn capability_tier_label(tier: &CapabilityTier) -> String {
    match tier {
        CapabilityTier::ReadOnly => "ReadOnly",
        CapabilityTier::LocalActions => "LocalActions",
        CapabilityTier::SystemActions => "SystemActions",
    }
    .to_string()
}

fn arguments_preview(arguments_json: &str) -> String {
    const MAX_CHARS: usize = 180;
    let compact = arguments_json.replace('\n', " ").replace('\r', " ");
    let mut chars = compact.chars();
    let preview: String = chars.by_ref().take(MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

fn request_fingerprint(messages: &[ChatMessage], provider_config: &ProviderConfig, mode: &ChatMode) -> String {
    let mut hasher = DefaultHasher::new();
    provider_config.provider_name.hash(&mut hasher);
    provider_config.model.hash(&mut hasher);
    match mode {
        ChatMode::RequireConfirmation => "RequireConfirmation".hash(&mut hasher),
        ChatMode::BestEffort => "BestEffort".hash(&mut hasher),
    }
    for message in messages {
        message.role.hash(&mut hasher);
        message.content.hash(&mut hasher);
    }
    format!("req-{:016x}", hasher.finish())
}
