use ipc::jsonrpc::{Id, Request};
use ipc::mcp::encode_stdio_frame;

#[derive(Clone, Debug)]
pub struct ProcessPluginConfig {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ProcessPluginClient {
    pub config: ProcessPluginConfig,
}

impl ProcessPluginClient {
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            config: ProcessPluginConfig {
                command: command.into(),
                args,
            },
        }
    }

    pub fn build_initialize_frame(&self) -> String {
        let req = Request::new(
            Id::Number(1),
            "initialize",
            "{\"protocol\":\"jsonrpc-stdio\",\"mcp_envelope\":true}",
        );
        let payload = format!(
            "{{\"jsonrpc\":\"{}\",\"id\":1,\"method\":\"{}\",\"params\":{}}}",
            req.jsonrpc, req.method, req.params_json
        );
        encode_stdio_frame(&payload)
    }
}
