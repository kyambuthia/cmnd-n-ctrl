pub mod jsonrpc;
pub mod mcp;

use crate::jsonrpc::{Id, Request, Response};
use serde::{Deserialize, Serialize};

pub type JsonBlob = String;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_json_schema: JsonBlob,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments_json: JsonBlob,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub summary: String,
    pub artifacts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub name: String,
    pub result_json: JsonBlob,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_name: String,
    pub model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatMode {
    RequireConfirmation,
    BestEffort,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRequest {
    #[serde(default)]
    pub session_id: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub provider_config: ProviderConfig,
    pub mode: ChatMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatApproveRequest {
    pub consent_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatDenyRequest {
    pub consent_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
    pub message_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
    pub title: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCreateRequest {
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionGetRequest {
    pub session_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDeleteRequest {
    pub session_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDeleteResponse {
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMessagesAppendRequest {
    pub session_id: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMessagesAppendResponse {
    pub session: Session,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub enabled: bool,
    pub is_active: bool,
    pub has_auth: bool,
    pub config_summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvidersSetRequest {
    pub provider_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfigGetRequest {
    pub provider_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfigRecord {
    pub provider_name: String,
    pub is_active: bool,
    pub config_json: JsonBlob,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfigSetRequest {
    pub provider_name: String,
    pub config_json: JsonBlob,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfigSetResponse {
    pub provider_name: String,
    pub has_auth: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerRecord {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerAddRequest {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerRemoveRequest {
    pub server_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerStateRequest {
    pub server_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerMutationResponse {
    pub ok: bool,
    pub server: Option<McpServerRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectOpenRequest {
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectOpenResponse {
    pub path: String,
    pub exists: bool,
    pub is_dir: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectStatusRequest {
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectStatusResponse {
    pub path: String,
    pub exists: bool,
    pub is_dir: bool,
    pub entry_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub audit_id: String,
    pub timestamp_unix_seconds: u64,
    pub session_id: Option<String>,
    pub provider: String,
    pub policy_decisions: Vec<String>,
    pub proposed_tool_calls: Vec<String>,
    pub executed_actions: Vec<String>,
    pub evidence_summaries: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditListRequest {
    pub session_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditGetRequest {
    pub audit_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemHealthResponse {
    pub ok: bool,
    pub active_provider: Option<String>,
    pub provider_count: usize,
    pub pending_consents: usize,
    pub mcp_servers_total: usize,
    pub mcp_servers_running: usize,
    pub project_path: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingConsentRecord {
    pub consent_id: String,
    pub session_id: Option<String>,
    pub requested_at_unix_seconds: u64,
    #[serde(default)]
    pub expires_at_unix_seconds: u64,
    pub tool_name: String,
    pub capability_tier: String,
    pub status: String,
    pub rationale: String,
    pub arguments_preview: Option<String>,
    pub request_fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentListRequest {
    pub status: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentActionRequest {
    pub consent_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRpcRequest {
    pub id: Option<u64>,
    pub method: String,
    pub params_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionEvent {
    pub tool_name: String,
    pub capability_tier: String,
    pub status: String,
    pub reason: Option<String>,
    pub arguments_preview: Option<String>,
    pub evidence_summary: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentRequest {
    pub scope: String,
    pub human_summary: String,
    pub risk_factors: Vec<String>,
    pub requires_extra_confirmation_click: bool,
    #[serde(default)]
    pub expires_at_unix_seconds: Option<u64>,
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub final_text: String,
    pub audit_id: String,
    pub request_fingerprint: String,
    pub execution_state: String,
    pub consent_token: Option<String>,
    pub session_id: Option<String>,
    pub consent_request: Option<ConsentRequest>,
    pub actions_executed: Vec<String>,
    pub proposed_actions: Vec<ActionEvent>,
    pub executed_action_events: Vec<ActionEvent>,
    pub action_events: Vec<ActionEvent>,
}

pub trait ChatService {
    fn chat_request(&mut self, params: ChatRequest) -> ChatResponse;
    fn chat_approve(&mut self, params: ChatApproveRequest) -> Result<ChatResponse, String>;
    fn chat_deny(&mut self, params: ChatDenyRequest) -> Result<ChatResponse, String>;
    fn sessions_create(&mut self, params: SessionCreateRequest) -> Result<Session, String>;
    fn sessions_list(&self) -> Result<Vec<SessionSummary>, String>;
    fn sessions_get(&self, params: SessionGetRequest) -> Result<Session, String>;
    fn sessions_delete(&mut self, params: SessionDeleteRequest) -> Result<SessionDeleteResponse, String>;
    fn sessions_messages_append(
        &mut self,
        params: SessionMessagesAppendRequest,
    ) -> Result<SessionMessagesAppendResponse, String>;
    fn providers_list(&self) -> Result<Vec<ProviderInfo>, String>;
    fn providers_set(&mut self, params: ProvidersSetRequest) -> Result<ProviderInfo, String>;
    fn providers_config_get(&self, params: ProviderConfigGetRequest) -> Result<ProviderConfigRecord, String>;
    fn providers_config_set(
        &mut self,
        params: ProviderConfigSetRequest,
    ) -> Result<ProviderConfigSetResponse, String>;
    fn mcp_servers_list(&self) -> Result<Vec<McpServerRecord>, String>;
    fn mcp_servers_add(&mut self, params: McpServerAddRequest) -> Result<McpServerMutationResponse, String>;
    fn mcp_servers_remove(
        &mut self,
        params: McpServerRemoveRequest,
    ) -> Result<McpServerMutationResponse, String>;
    fn mcp_servers_start(&mut self, params: McpServerStateRequest) -> Result<McpServerMutationResponse, String>;
    fn mcp_servers_stop(&mut self, params: McpServerStateRequest) -> Result<McpServerMutationResponse, String>;
    fn project_open(&mut self, params: ProjectOpenRequest) -> Result<ProjectOpenResponse, String>;
    fn project_status(&self, params: ProjectStatusRequest) -> Result<ProjectStatusResponse, String>;
    fn audit_list(&self, params: AuditListRequest) -> Result<Vec<AuditEntry>, String>;
    fn audit_get(&self, params: AuditGetRequest) -> Result<AuditEntry, String>;
    fn consent_list(&self, params: ConsentListRequest) -> Result<Vec<PendingConsentRecord>, String>;
    fn consent_approve(&mut self, params: ConsentActionRequest) -> Result<ChatResponse, String>;
    fn consent_deny(&mut self, params: ConsentActionRequest) -> Result<ChatResponse, String>;
    fn tools_list(&self) -> Vec<Tool>;
    fn system_health(&self) -> Result<SystemHealthResponse, String>;
}

pub struct JsonRpcServer<S> {
    service: S,
}

impl<S> JsonRpcServer<S>
where
    S: ChatService,
{
    pub fn new(service: S) -> Self {
        Self { service }
    }

    pub fn handle(&mut self, request: Request) -> Response {
        match request.method.as_str() {
            "tools.list" => {
                match serde_json::to_string(&self.service.tools_list()) {
                    Ok(payload) => Response::success(request.id, payload),
                    Err(err) => Response::error(request.id, -32603, format!("serialization error: {err}")),
                }
            }
            "chat.request" => {
                match serde_json::from_str::<ChatRequest>(&request.params_json) {
                    Ok(params) => match serde_json::to_string(&self.service.chat_request(params)) {
                        Ok(payload) => Response::success(request.id, payload),
                        Err(err) => Response::error(request.id, -32603, format!("serialization error: {err}")),
                    },
                    Err(err) => Response::error(request.id, -32602, format!("invalid params: {err}")),
                }
            }
            "chat.approve" => {
                match serde_json::from_str::<ChatApproveRequest>(&request.params_json) {
                    Ok(params) => match self.service.chat_approve(params) {
                        Ok(response) => serialize_ok(request.id, response),
                        Err(err) => Response::error(request.id, -32000, err),
                    },
                    Err(err) => Response::error(request.id, -32602, format!("invalid params: {err}")),
                }
            }
            "chat.deny" => {
                match serde_json::from_str::<ChatDenyRequest>(&request.params_json) {
                    Ok(params) => match self.service.chat_deny(params) {
                        Ok(response) => serialize_ok(request.id, response),
                        Err(err) => Response::error(request.id, -32000, err),
                    },
                    Err(err) => Response::error(request.id, -32602, format!("invalid params: {err}")),
                }
            }
            "sessions.create" => self.parse_and_call(&request, |s, p: SessionCreateRequest| s.sessions_create(p)),
            "sessions.list" => self.parse_and_call(&request, |s, _p: EmptyParams| s.sessions_list()),
            "sessions.get" => self.parse_and_call(&request, |s, p: SessionGetRequest| s.sessions_get(p)),
            "sessions.delete" => self.parse_and_call(&request, |s, p: SessionDeleteRequest| s.sessions_delete(p)),
            "sessions.messages.append" => self.parse_and_call(&request, |s, p: SessionMessagesAppendRequest| {
                s.sessions_messages_append(p)
            }),
            "providers.list" => self.parse_and_call(&request, |s, _p: EmptyParams| s.providers_list()),
            "providers.set" => self.parse_and_call(&request, |s, p: ProvidersSetRequest| s.providers_set(p)),
            "providers.config.get" => {
                self.parse_and_call(&request, |s, p: ProviderConfigGetRequest| s.providers_config_get(p))
            }
            "providers.config.set" => {
                self.parse_and_call(&request, |s, p: ProviderConfigSetRequest| s.providers_config_set(p))
            }
            "mcp.servers.list" => self.parse_and_call(&request, |s, _p: EmptyParams| s.mcp_servers_list()),
            "mcp.servers.add" => self.parse_and_call(&request, |s, p: McpServerAddRequest| s.mcp_servers_add(p)),
            "mcp.servers.remove" => {
                self.parse_and_call(&request, |s, p: McpServerRemoveRequest| s.mcp_servers_remove(p))
            }
            "mcp.servers.start" => {
                self.parse_and_call(&request, |s, p: McpServerStateRequest| s.mcp_servers_start(p))
            }
            "mcp.servers.stop" => self.parse_and_call(&request, |s, p: McpServerStateRequest| s.mcp_servers_stop(p)),
            "project.open" => self.parse_and_call(&request, |s, p: ProjectOpenRequest| s.project_open(p)),
            "project.status" => self.parse_and_call(&request, |s, p: ProjectStatusRequest| s.project_status(p)),
            "audit.list" => self.parse_and_call(&request, |s, p: AuditListRequest| s.audit_list(p)),
            "audit.get" => self.parse_and_call(&request, |s, p: AuditGetRequest| s.audit_get(p)),
            "consent.list" => self.parse_and_call(&request, |s, p: ConsentListRequest| s.consent_list(p)),
            "consent.approve" => {
                self.parse_and_call(&request, |s, p: ConsentActionRequest| s.consent_approve(p))
            }
            "consent.deny" => self.parse_and_call(&request, |s, p: ConsentActionRequest| s.consent_deny(p)),
            "system.health" => self.parse_and_call(&request, |s, _p: EmptyParams| s.system_health()),
            "rpc.raw" => {
                match serde_json::from_str::<RawRpcRequest>(&request.params_json) {
                    Ok(inner) => self.handle(Request::new(
                        Id::Number(inner.id.unwrap_or(1)),
                        inner.method,
                        inner.params_json,
                    )),
                    Err(err) => Response::error(request.id, -32602, format!("invalid params: {err}")),
                }
            }
            _ => Response::error(request.id, -32601, "method not found"),
        }
    }

    pub fn service_mut(&mut self) -> &mut S {
        &mut self.service
    }

    pub fn service(&self) -> &S {
        &self.service
    }

    fn parse_and_call<P, R, F>(&mut self, request: &Request, mut f: F) -> Response
    where
        P: for<'de> Deserialize<'de>,
        R: Serialize,
        F: FnMut(&mut S, P) -> Result<R, String>,
    {
        match serde_json::from_str::<P>(&request.params_json) {
            Ok(params) => match f(&mut self.service, params) {
                Ok(value) => serialize_ok(request.id.clone(), value),
                Err(err) => Response::error(request.id.clone(), -32000, err),
            },
            Err(err) => Response::error(
                request.id.clone(),
                -32602,
                format!("invalid params: {err}"),
            ),
        }
    }
}

pub struct JsonRpcClient<'a, S> {
    server: &'a mut JsonRpcServer<S>,
}

impl<'a, S> JsonRpcClient<'a, S>
where
    S: ChatService,
{
    pub fn new(server: &'a mut JsonRpcServer<S>) -> Self {
        Self { server }
    }

    pub fn chat_request(&mut self, params: ChatRequest) -> ChatResponse {
        self.server.service_mut().chat_request(params)
    }

    pub fn chat_approve(&mut self, params: ChatApproveRequest) -> Result<ChatResponse, String> {
        self.server.service_mut().chat_approve(params)
    }

    pub fn chat_deny(&mut self, params: ChatDenyRequest) -> Result<ChatResponse, String> {
        self.server.service_mut().chat_deny(params)
    }

    pub fn tools_list(&mut self) -> Vec<Tool> {
        self.server.service().tools_list()
    }

    pub fn call_raw(&mut self, request: Request) -> Response {
        self.server.handle(request)
    }
}

pub fn sample_messages(user_message: &str) -> Vec<ChatMessage> {
    vec![ChatMessage {
        role: "user".to_string(),
        content: user_message.to_string(),
    }]
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmptyParams {}

fn serialize_ok<T: Serialize>(id: Id, value: T) -> Response {
    match serde_json::to_string(&value) {
        Ok(payload) => Response::success(id, payload),
        Err(err) => Response::error(id, -32603, format!("serialization error: {err}")),
    }
}
