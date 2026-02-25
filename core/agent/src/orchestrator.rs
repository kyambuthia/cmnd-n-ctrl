use actions::traits::ActionBackend;
use ipc::{ChatMessage, ChatMode, ChatResponse, ProviderConfig, ToolResult};
use providers::provider_trait::{Provider, ProviderReply};

use crate::policy::{Authorization, Policy, PolicyContext};
use crate::tool_registry::ToolRegistry;

#[derive(Clone, Debug)]
pub struct AuditEvent {
    pub audit_id: String,
    pub message: String,
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
        let tools = self.tool_registry.list();
        let mut executed_actions = Vec::new();
        let mut tool_results: Vec<ToolResult> = Vec::new();

        let first_reply = self.provider.chat(&messages, &tools, &tool_results, &provider_config);
        let final_text = match first_reply {
            ProviderReply::FinalText(text) => text,
            ProviderReply::ToolCalls(calls) => {
                let mut pending_confirmation = false;
                for call in calls {
                    if !self.tool_registry.has_tool(&call.name) {
                        executed_actions.push(format!("denied:{}:unknown_tool", call.name));
                        continue;
                    }
                    let auth = self.policy.authorize(
                        &call,
                        &PolicyContext {
                            mode: mode.clone(),
                            user_confirmed: matches!(mode, ChatMode::BestEffort),
                        },
                    );
                    match auth {
                        Authorization::Allow => {
                            let result = self.action_backend.execute_tool(&call);
                            executed_actions.push(call.name.clone());
                            tool_results.push(result);
                        }
                        Authorization::RequireConfirmation { reason } => {
                            pending_confirmation = true;
                            executed_actions.push(format!("confirm_required:{}:{}", call.name, reason));
                        }
                        Authorization::Deny { reason } => {
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
}
