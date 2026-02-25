pub mod jsonrpc;
pub mod mcp;

use crate::jsonrpc::{Request, Response};
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
    pub messages: Vec<ChatMessage>,
    pub provider_config: ProviderConfig,
    pub mode: ChatMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub final_text: String,
    pub audit_id: String,
    pub actions_executed: Vec<String>,
}

pub trait ChatService {
    fn chat_request(&mut self, params: ChatRequest) -> ChatResponse;
    fn tools_list(&self) -> Vec<Tool>;
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
            _ => Response::error(request.id, -32601, "method not found"),
        }
    }

    pub fn service_mut(&mut self) -> &mut S {
        &mut self.service
    }

    pub fn service(&self) -> &S {
        &self.service
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
