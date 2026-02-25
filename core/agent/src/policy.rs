use ipc::{ChatMode, ToolCall};

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
            default_require_confirmation: true,
        }
    }
}

impl Policy {
    pub fn authorize(&self, tool_call: &ToolCall, context: &PolicyContext) -> Authorization {
        if tool_call.name.starts_with("internal.") {
            return Authorization::Deny {
                reason: "internal.* tools are reserved".to_string(),
            };
        }

        let sensitive = tool_call.name.contains("desktop") || tool_call.name.contains("android") || tool_call.name.contains("ios");
        let require_confirmation = self.default_require_confirmation
            || matches!(context.mode, ChatMode::RequireConfirmation)
            || sensitive;

        if require_confirmation && !context.user_confirmed {
            Authorization::RequireConfirmation {
                reason: format!("Tool '{}' requires explicit user consent", tool_call.name),
            }
        } else {
            Authorization::Allow
        }
    }
}
