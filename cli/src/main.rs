use std::env;
use std::io::{self, BufReader};

use agent::AgentService;
use ipc::jsonrpc::{Id, Request};
use ipc::{mcp, ChatMode, ChatRequest, JsonRpcClient, JsonRpcServer, ProviderConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct WireRequest {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct WireError {
    code: i64,
    message: String,
}

#[derive(Debug, Serialize)]
struct WireResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<WireError>,
}

fn print_help() {
    println!("cli - Rust CLI client for local assistant IPC");
    println!();
    println!("USAGE:");
    println!("  cli --help");
    println!("  cli tools");
    println!("  cli chat <message> [--provider <name>] [--require-confirmation]");
    println!("  cli serve-stdio");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    let service = AgentService::new_for_platform("cli");
    let mut server = JsonRpcServer::new(service);
    let mut client = JsonRpcClient::new(&mut server);

    match args[0].as_str() {
        "tools" => {
            let tools = client.tools_list();
            for tool in tools {
                println!("{} - {}", tool.name, tool.description);
            }

            let raw = client.call_raw(Request::new(Id::Number(1), "tools.list", "{}"));
            if let Some(result) = raw.result_json {
                eprintln!("raw json-rpc result: {result}");
            }
        }
        "chat" => {
            if args.len() < 2 {
                eprintln!("error: missing chat message");
                print_help();
                std::process::exit(2);
            }
            let mut provider_name = "openai-stub".to_string();
            let mut require_confirmation = false;
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--provider" => {
                        if let Some(next) = args.get(i + 1) {
                            provider_name = next.clone();
                            i += 2;
                            continue;
                        }
                    }
                    "--require-confirmation" => {
                        require_confirmation = true;
                        i += 1;
                        continue;
                    }
                    _ => {}
                }
                i += 1;
            }

            let response = client.chat_request(ChatRequest {
                messages: ipc::sample_messages(&args[1]),
                provider_config: ProviderConfig {
                    provider_name,
                    model: None,
                },
                mode: if require_confirmation {
                    ChatMode::RequireConfirmation
                } else {
                    ChatMode::BestEffort
                },
            });

            println!("audit_id: {}", response.audit_id);
            println!("actions: {}", response.actions_executed.join(", "));
            println!("response: {}", response.final_text);
        }
        "serve-stdio" => {
            if let Err(err) = serve_stdio_jsonrpc() {
                eprintln!("stdio server error: {err}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("error: unknown command '{}'.", args[0]);
            print_help();
            std::process::exit(2);
        }
    }
}

fn serve_stdio_jsonrpc() -> io::Result<()> {
    let service = AgentService::new_for_platform("ipc-stdio");
    let mut server = JsonRpcServer::new(service);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    while let Some(frame) = mcp::read_stdio_frame_from(&mut reader)? {
        let response = match parse_wire_request(&frame) {
            Ok(request) => {
                let raw_request = Request::new(
                    json_value_to_id(request.id.unwrap_or(Value::Null)),
                    request.method,
                    request.params.to_string(),
                );
                to_wire_response(server.handle(raw_request))
            }
            Err(err) => WireResponse {
                jsonrpc: "2.0".to_string(),
                id: Value::Null,
                result: None,
                error: Some(WireError {
                    code: -32700,
                    message: format!("parse error: {err}"),
                }),
            },
        };

        let payload = serde_json::to_string(&response)
            .map_err(|err| io::Error::other(format!("serialize response: {err}")))?;
        mcp::write_stdio_frame_to(&mut writer, &payload)?;
    }

    Ok(())
}

fn parse_wire_request(payload: &str) -> Result<WireRequest, serde_json::Error> {
    let req: WireRequest = serde_json::from_str(payload)?;
    Ok(req)
}

fn json_value_to_id(value: Value) -> Id {
    match value {
        Value::Number(n) => n.as_u64().map(Id::Number).unwrap_or(Id::Null),
        Value::String(s) => Id::String(s),
        _ => Id::Null,
    }
}

fn id_to_json_value(id: Id) -> Value {
    match id {
        Id::Number(n) => Value::from(n),
        Id::String(s) => Value::String(s),
        Id::Null => Value::Null,
    }
}

fn to_wire_response(response: ipc::jsonrpc::Response) -> WireResponse {
    let result = response
        .result_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .or_else(|| response.result_json.map(Value::String));

    let error = response.error.map(|e| WireError {
        code: e.code,
        message: e.message,
    });

    WireResponse {
        jsonrpc: response.jsonrpc,
        id: id_to_json_value(response.id),
        result,
        error,
    }
}
