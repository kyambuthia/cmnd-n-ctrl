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
        if tool_call.name.starts_with("time.")
            || tool_call.name.starts_with("math.")
            || tool_call.name.starts_with("text.")
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
