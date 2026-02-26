use ipc::{ChatMode, ToolCall};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapabilityTier {
    ReadOnly,
    LocalActions,
    SystemActions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Authorization {
    Allow,
    RequireConfirmation { reason: String },
    Deny { reason: String },
}

#[derive(Clone, Debug)]
pub struct PolicyContext {
    pub mode: ChatMode,
    pub user_confirmed: bool,
}

#[derive(Clone, Debug)]
pub struct Policy {
    pub default_require_confirmation: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            default_require_confirmation: false,
        }
    }
}

impl Policy {
    pub fn capability_tier(&self, tool_call: &ToolCall) -> CapabilityTier {
        if tool_call.name == "desktop.app.activate" {
            return CapabilityTier::SystemActions;
        }
        if tool_call.name == "desktop.app.list" {
            return CapabilityTier::LocalActions;
        }
        if matches!(
            tool_call.name.as_str(),
            "file.write_text" | "file.append_text" | "file.mkdir"
        ) {
            return CapabilityTier::LocalActions;
        }
        if tool_call.name.starts_with("time.")
            || tool_call.name.starts_with("math.")
            || tool_call.name.starts_with("text.")
            || tool_call.name.starts_with("file.")
            || tool_call.name == "echo"
        {
            CapabilityTier::ReadOnly
        } else if tool_call.name.starts_with("desktop.") || tool_call.name.starts_with("android.") || tool_call.name.starts_with("ios.") {
            CapabilityTier::LocalActions
        } else {
            CapabilityTier::SystemActions
        }
    }

    pub fn authorize(&self, tool_call: &ToolCall, context: &PolicyContext) -> Authorization {
        if tool_call.name.starts_with("internal.") {
            return Authorization::Deny {
                reason: "internal.* tools are reserved".to_string(),
            };
        }

        let require_confirmation = match self.capability_tier(tool_call) {
            CapabilityTier::ReadOnly => {
                self.default_require_confirmation || matches!(context.mode, ChatMode::RequireConfirmation)
            }
            CapabilityTier::LocalActions | CapabilityTier::SystemActions => true,
        };

        if require_confirmation && !context.user_confirmed {
            Authorization::RequireConfirmation {
                reason: format!("Tool '{}' requires explicit user consent", tool_call.name),
            }
        } else {
            Authorization::Allow
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipc::ToolCall;

    fn call(name: &str) -> ToolCall {
        ToolCall {
            name: name.to_string(),
            arguments_json: "{}".to_string(),
        }
    }

    #[test]
    fn authorize_allows_readonly_in_best_effort() {
        let policy = Policy::default();
        let result = policy.authorize(
            &call("time.now"),
            &PolicyContext {
                mode: ChatMode::BestEffort,
                user_confirmed: false,
            },
        );
        assert!(matches!(result, Authorization::Allow));
    }

    #[test]
    fn authorize_requires_confirmation_for_local_action() {
        let policy = Policy::default();
        let result = policy.authorize(
            &call("desktop.app.list"),
            &PolicyContext {
                mode: ChatMode::BestEffort,
                user_confirmed: false,
            },
        );
        assert!(matches!(result, Authorization::RequireConfirmation { .. }));
    }

    #[test]
    fn file_write_text_is_local_action_and_requires_confirmation() {
        let policy = Policy::default();
        assert!(matches!(
            policy.capability_tier(&call("file.write_text")),
            CapabilityTier::LocalActions
        ));
        let result = policy.authorize(
            &call("file.write_text"),
            &PolicyContext {
                mode: ChatMode::BestEffort,
                user_confirmed: false,
            },
        );
        assert!(matches!(result, Authorization::RequireConfirmation { .. }));
    }

    #[test]
    fn file_append_and_mkdir_require_confirmation() {
        let policy = Policy::default();
        for name in ["file.append_text", "file.mkdir"] {
            assert!(matches!(
                policy.capability_tier(&call(name)),
                CapabilityTier::LocalActions
            ));
            let result = policy.authorize(
                &call(name),
                &PolicyContext {
                    mode: ChatMode::BestEffort,
                    user_confirmed: false,
                },
            );
            assert!(matches!(result, Authorization::RequireConfirmation { .. }));
        }
    }

    #[test]
    fn authorize_denies_internal_tools() {
        let policy = Policy::default();
        let result = policy.authorize(
            &call("internal.secret"),
            &PolicyContext {
                mode: ChatMode::BestEffort,
                user_confirmed: false,
            },
        );
        assert!(matches!(result, Authorization::Deny { .. }));
    }
}
