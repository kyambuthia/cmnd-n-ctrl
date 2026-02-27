mod tui;

use std::env;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

use agent::AgentService;
use ipc::jsonrpc::{Id, Request};
use ipc::{mcp, ChatApproveRequest, ChatDenyRequest, ChatMode, ChatRequest, ChatResponse, JsonRpcClient, JsonRpcServer, ProviderConfig, Tool};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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

#[derive(Debug, Serialize, Deserialize)]
struct WireError {
    code: i64,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
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
    println!("  cli        # launches interactive terminal UI (ratatui)");
    println!("  cli --help");
    println!("  cli tools [--json] [--raw] [--addr <host:port>]");
    println!("  cli chat <message> [--provider <name>] [--session <id>] [--require-confirmation] [--json] [--addr <host:port>]");
    println!("  cli approve <consent-token> [--json] [--addr <host:port>]   # requires running serve-http");
    println!("  cli deny <consent-token> [--json] [--addr <host:port>]      # requires running serve-http");
    println!("  cli consent list|approve|deny ...");
    println!("  cli session new|list|open|rm|append ...");
    println!("  cli auth login|list|logout ...");
    println!("  cli providers list|set|config-get|config-set ...");
    println!("  cli mcp servers list|add|rm|start|stop|probe|tools|call|tool-call ...");
    println!("  cli project open|status ...");
    println!("  cli audit list|show ...");
    println!("  cli doctor [--json] [--strict] [--addr <host:port>]");
    println!("  cli tui   # interactive terminal UI (ratatui)");
    println!("  cli rpc <method> <params-json> [--addr <host:port>]");
    println!("  cli serve-stdio");
    println!("  cli serve-http [--addr <host:port>]");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    let service = AgentService::new_for_platform("cli");
    let mut server = JsonRpcServer::new(service);
    let mut client = JsonRpcClient::new(&mut server);

    if args.is_empty() {
        if let Err(err) = tui::run(&mut client) {
            eprintln!("tui error: {err}");
            std::process::exit(1);
        }
        return;
    }

    match args[0].as_str() {
        "tools" => {
            let json_output = args.iter().any(|a| a == "--json");
            let raw_output = args.iter().any(|a| a == "--raw");
            let remote_addr = parse_addr_flag(&args[1..]);
            let tools = if let Some(addr) = &remote_addr {
                let wire = match call_http_jsonrpc(addr, "tools.list", json!({})) {
                    Ok(w) => w,
                    Err(err) => {
                        eprintln!("tools error: {err}");
                        std::process::exit(1);
                    }
                };
                if raw_output {
                    eprintln!(
                        "raw json-rpc result: {}",
                        serde_json::to_string(&wire).unwrap_or_else(|_| "{}".to_string())
                    );
                }
                match wire_result::<Vec<Tool>>(wire) {
                    Ok(v) => v,
                    Err(err) => {
                        eprintln!("tools error: {err}");
                        std::process::exit(1);
                    }
                }
            } else {
                let tools = client.tools_list();
                if raw_output {
                    let raw = client.call_raw(Request::new(Id::Number(1), "tools.list", "{}"));
                    if let Some(result) = raw.result_json {
                        eprintln!("raw json-rpc result: {result}");
                    }
                }
                tools
            };
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&tools).unwrap_or_else(|_| "[]".to_string())
                );
            } else {
                for tool in tools {
                    println!("{} - {}", tool.name, tool.description);
                }
            }
        }
        "chat" => {
            if args.len() < 2 {
                eprintln!("error: missing chat message");
                print_help();
                std::process::exit(2);
            }
            if contains_tool_syntax(&args[1]) {
                eprintln!("error: explicit tool syntax is disabled; use natural language prompts");
                std::process::exit(2);
            }
            let mut provider_name = "openai-stub".to_string();
            let mut require_confirmation = false;
            let mut json_output = false;
            let mut remote_addr = None;
            let mut session_id = None;
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
                    "--json" => {
                        json_output = true;
                        i += 1;
                        continue;
                    }
                    "--addr" => {
                        if let Some(next) = args.get(i + 1) {
                            remote_addr = Some(next.clone());
                            i += 2;
                            continue;
                        }
                    }
                    "--session" => {
                        if let Some(next) = args.get(i + 1) {
                            session_id = Some(next.clone());
                            i += 2;
                            continue;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }

            let chat_request = ChatRequest {
                session_id,
                messages: ipc::sample_messages(&args[1]),
                provider_config: ProviderConfig {
                    provider_name,
                    model: None,
                    config_json: None,
                },
                mode: if require_confirmation {
                    ChatMode::RequireConfirmation
                } else {
                    ChatMode::BestEffort
                },
            };

            let response = if let Some(addr) = remote_addr.as_deref() {
                match call_http_jsonrpc(addr, "chat.request", serde_json::to_value(&chat_request).unwrap_or(json!({})))
                    .and_then(|wire| wire_result::<ChatResponse>(wire).map_err(io::Error::other))
                {
                    Ok(resp) => resp,
                    Err(err) => {
                        eprintln!("chat error: {err}");
                        std::process::exit(1);
                    }
                }
            } else {
                client.chat_request(chat_request)
            };

            print_chat_response(&response, json_output);
        }
        "consent" => {
            handle_consent_command(&mut client, &args[1..]);
        }
        "session" | "sessions" => {
            handle_session_command(&mut client, &args[1..]);
        }
        "auth" => {
            handle_auth_command(&mut client, &args[1..]);
        }
        "providers" => {
            handle_providers_command(&mut client, &args[1..]);
        }
        "mcp" => {
            handle_mcp_command(&mut client, &args[1..]);
        }
        "project" => {
            handle_project_command(&mut client, &args[1..]);
        }
        "audit" => {
            handle_audit_command(&mut client, &args[1..]);
        }
        "doctor" => {
            handle_doctor_command(&mut client, &args[1..]);
        }
        "tui" => {
            if let Err(err) = tui::run(&mut client) {
                eprintln!("tui error: {err}");
                std::process::exit(1);
            }
        }
        "approve" => {
            if args.len() < 2 {
                eprintln!("error: missing consent token");
                print_help();
                std::process::exit(2);
            }
            let json_output = args.iter().any(|a| a == "--json");
            let addr = parse_addr_flag(&args[1..]).unwrap_or_else(|| "127.0.0.1:7777".to_string());
            let params = serde_json::to_value(ChatApproveRequest {
                consent_token: args[1].clone(),
            })
            .unwrap_or(json!({}));
            let response = match call_http_jsonrpc(&addr, "chat.approve", params)
                .and_then(|wire| wire_result::<ChatResponse>(wire).map_err(io::Error::other))
            {
                Ok(resp) => resp,
                Err(err) => {
                    eprintln!("approve error: {err}");
                    std::process::exit(1);
                }
            };
            print_chat_response(&response, json_output);
        }
        "deny" => {
            if args.len() < 2 {
                eprintln!("error: missing consent token");
                print_help();
                std::process::exit(2);
            }
            let json_output = args.iter().any(|a| a == "--json");
            let addr = parse_addr_flag(&args[1..]).unwrap_or_else(|| "127.0.0.1:7777".to_string());
            let params = serde_json::to_value(ChatDenyRequest {
                consent_token: args[1].clone(),
            })
            .unwrap_or(json!({}));
            let response = match call_http_jsonrpc(&addr, "chat.deny", params)
                .and_then(|wire| wire_result::<ChatResponse>(wire).map_err(io::Error::other))
            {
                Ok(resp) => resp,
                Err(err) => {
                    eprintln!("deny error: {err}");
                    std::process::exit(1);
                }
            };
            print_chat_response(&response, json_output);
        }
        "rpc" => {
            if args.len() < 3 {
                eprintln!("error: usage: cli rpc <method> <params-json>");
                print_help();
                std::process::exit(2);
            }
            let remote_addr = parse_addr_flag(&args[3..]);
            let wire = if let Some(addr) = remote_addr.as_deref() {
                let params = serde_json::from_str::<Value>(&args[2]).unwrap_or_else(|_| Value::String(args[2].clone()));
                match call_http_jsonrpc(addr, &args[1], params) {
                    Ok(w) => w,
                    Err(err) => {
                        eprintln!("rpc error: {err}");
                        std::process::exit(1);
                    }
                }
            } else {
                let response = client.call_raw(Request::new(
                    Id::Number(1),
                    args[1].clone(),
                    args[2].clone(),
                ));
                to_wire_response(response)
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&wire)
                    .unwrap_or_else(|_| "{\"error\":\"serialize\"}".to_string())
            );
        }
        "serve-stdio" => {
            if let Err(err) = serve_stdio_jsonrpc() {
                eprintln!("stdio server error: {err}");
                std::process::exit(1);
            }
        }
        "serve-http" => {
            let mut addr = "127.0.0.1:7777".to_string();
            let mut i = 1;
            while i < args.len() {
                if args[i] == "--addr" {
                    if let Some(next) = args.get(i + 1) {
                        addr = next.clone();
                        i += 2;
                        continue;
                    }
                }
                i += 1;
            }

            if let Err(err) = serve_http_jsonrpc(&addr) {
                eprintln!("http server error: {err}");
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

fn contains_tool_syntax(input: &str) -> bool {
    input.to_ascii_lowercase().contains("tool:")
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

fn parse_addr_flag(args: &[String]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--addr" {
            return args.get(i + 1).cloned();
        }
        i += 1;
    }
    None
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn string_flag(args: &[String], flag: &str) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == flag {
            return args.get(i + 1).cloned();
        }
        i += 1;
    }
    None
}

fn positional_without_flags(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" | "--raw" | "--strict" => i += 1,
            "--addr" | "--key" | "--env" | "--provider" | "--session" | "--title" | "--path" | "--command" | "--name" | "--status" | "--limit" => {
                i += 2
            }
            "--args" => i += 2,
            _ => {
                out.push(args[i].clone());
                i += 1;
            }
        }
    }
    out
}

fn backend_call_value(
    client: &mut JsonRpcClient<AgentService>,
    addr: Option<&str>,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    if let Some(addr) = addr {
        return wire_result(call_http_jsonrpc(addr, method, params).map_err(|e| e.to_string())?);
    }
    let raw = client.call_raw(Request::new(
        Id::Number(1),
        method.to_string(),
        params.to_string(),
    ));
    wire_result(to_wire_response(raw))
}

fn print_value(value: &Value, json_output: bool) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_string())
        );
    } else if let Some(s) = value.as_str() {
        println!("{s}");
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_string())
        );
    }
}

fn handle_consent_command(client: &mut JsonRpcClient<AgentService>, args: &[String]) {
    let json_output = has_flag(args, "--json");
    let addr = parse_addr_flag(args);
    let pos = positional_without_flags(args);
    if pos.is_empty() {
        eprintln!("usage: cli consent list|approve|deny ...");
        std::process::exit(2);
    }
    let (method, params) = match pos[0].as_str() {
        "list" => (
            "consent.list",
            json!({
                "status": string_flag(args, "--status"),
                "session_id": string_flag(args, "--session")
            }),
        ),
        "approve" if pos.len() >= 2 => (
            "consent.approve",
            json!({ "consent_id": pos[1] }),
        ),
        "deny" if pos.len() >= 2 => (
            "consent.deny",
            json!({ "consent_id": pos[1] }),
        ),
        _ => {
            eprintln!("usage: cli consent list|approve <id>|deny <id>");
            std::process::exit(2);
        }
    };
    let result = backend_call_value(client, addr.as_deref(), method, params).unwrap_or_else(|err| {
        eprintln!("consent error: {err}");
        std::process::exit(1);
    });
    if method == "consent.approve" || method == "consent.deny" {
        let response: ChatResponse = serde_json::from_value(result).unwrap_or_else(|err| {
            eprintln!("consent parse error: {err}");
            std::process::exit(1);
        });
        print_chat_response(&response, json_output);
    } else {
        print_value(&result, json_output);
    }
}

fn handle_session_command(client: &mut JsonRpcClient<AgentService>, args: &[String]) {
    let json_output = has_flag(args, "--json");
    let addr = parse_addr_flag(args);
    let pos = positional_without_flags(args);
    if pos.is_empty() {
        eprintln!("usage: cli session new|list|open|rm|append ...");
        std::process::exit(2);
    }
    let (method, params) = match pos[0].as_str() {
        "new" => ("sessions.create", json!({ "title": string_flag(args, "--title") })),
        "list" => ("sessions.list", json!({})),
        "open" | "get" if pos.len() >= 2 => ("sessions.get", json!({ "session_id": pos[1] })),
        "rm" | "delete" if pos.len() >= 2 => ("sessions.delete", json!({ "session_id": pos[1] })),
        "append" if pos.len() >= 3 => (
            "sessions.messages.append",
            json!({
                "session_id": pos[1],
                "messages": [{"role": "user", "content": pos[2..].join(" ")}]
            }),
        ),
        _ => {
            eprintln!("usage: cli session new|list|open <id>|rm <id>|append <id> <message>");
            std::process::exit(2);
        }
    };
    let result = backend_call_value(client, addr.as_deref(), method, params).unwrap_or_else(|err| {
        eprintln!("session error: {err}");
        std::process::exit(1);
    });
    print_value(&result, json_output);
}

fn handle_auth_command(client: &mut JsonRpcClient<AgentService>, args: &[String]) {
    let json_output = has_flag(args, "--json");
    let addr = parse_addr_flag(args);
    let pos = positional_without_flags(args);
    if pos.is_empty() {
        eprintln!("usage: cli auth login|list|logout ...");
        std::process::exit(2);
    }
    match pos[0].as_str() {
        "list" => {
            let result = backend_call_value(client, addr.as_deref(), "providers.list", json!({}))
                .unwrap_or_else(|err| {
                    eprintln!("auth list error: {err}");
                    std::process::exit(1);
                });
            print_value(&result, json_output);
        }
        "login" => {
            if pos.len() < 2 {
                eprintln!("usage: cli auth login <provider> (--key <token> | --env <ENV_VAR>)");
                std::process::exit(2);
            }
            let provider = pos[1].clone();
            let cfg = if let Some(env_var) = string_flag(args, "--env") {
                json!({ "api_key_env": env_var }).to_string()
            } else {
                let key = string_flag(args, "--key").unwrap_or_default();
                json!({ "api_key": key }).to_string()
            };
            let _ = backend_call_value(
                client,
                addr.as_deref(),
                "providers.config.set",
                json!({ "provider_name": provider, "config_json": cfg }),
            )
            .unwrap_or_else(|err| {
                eprintln!("auth login error: {err}");
                std::process::exit(1);
            });
            let result = backend_call_value(
                client,
                addr.as_deref(),
                "providers.set",
                json!({ "provider_name": pos[1] }),
            )
            .unwrap_or_else(|err| {
                eprintln!("auth login error: {err}");
                std::process::exit(1);
            });
            print_value(&result, json_output);
        }
        "logout" => {
            if pos.len() < 2 {
                eprintln!("usage: cli auth logout <provider>");
                std::process::exit(2);
            }
            let result = backend_call_value(
                client,
                addr.as_deref(),
                "providers.config.set",
                json!({ "provider_name": pos[1], "config_json": "{}" }),
            )
            .unwrap_or_else(|err| {
                eprintln!("auth logout error: {err}");
                std::process::exit(1);
            });
            print_value(&result, json_output);
        }
        _ => {
            eprintln!("usage: cli auth login|list|logout");
            std::process::exit(2);
        }
    }
}

fn handle_providers_command(client: &mut JsonRpcClient<AgentService>, args: &[String]) {
    let json_output = has_flag(args, "--json");
    let addr = parse_addr_flag(args);
    let pos = positional_without_flags(args);
    if pos.is_empty() {
        eprintln!("usage: cli providers list|set|config-get|config-set ...");
        std::process::exit(2);
    }
    let (method, params) = match pos[0].as_str() {
        "list" => ("providers.list", json!({})),
        "set" if pos.len() >= 2 => ("providers.set", json!({ "provider_name": pos[1] })),
        "config-get" => (
            "providers.config.get",
            json!({ "provider_name": pos.get(1).cloned() }),
        ),
        "config-set" if pos.len() >= 3 => (
            "providers.config.set",
            json!({ "provider_name": pos[1], "config_json": pos[2] }),
        ),
        _ => {
            eprintln!("usage: cli providers list|set <name>|config-get [name]|config-set <name> <json>");
            std::process::exit(2);
        }
    };
    let result = backend_call_value(client, addr.as_deref(), method, params).unwrap_or_else(|err| {
        eprintln!("providers error: {err}");
        std::process::exit(1);
    });
    print_value(&result, json_output);
}

fn handle_mcp_command(client: &mut JsonRpcClient<AgentService>, args: &[String]) {
    let json_output = has_flag(args, "--json");
    let addr = parse_addr_flag(args);
    let pos = positional_without_flags(args);
    if pos.len() < 2 || pos[0] != "servers" {
        eprintln!("usage: cli mcp servers list|add|rm|start|stop|probe|tools|call|tool-call ...");
        std::process::exit(2);
    }
    let (method, params) = match pos[1].as_str() {
        "list" => ("mcp.servers.list", json!({})),
        "add" => {
            let name = string_flag(args, "--name").unwrap_or_else(|| "server".to_string());
            let command = string_flag(args, "--command").unwrap_or_else(|| "echo".to_string());
            let argv = string_flag(args, "--args")
                .map(|s| s.split_whitespace().map(|v| v.to_string()).collect::<Vec<_>>())
                .unwrap_or_default();
            (
                "mcp.servers.add",
                json!({ "name": name, "command": command, "args": argv }),
            )
        }
        "rm" | "remove" if pos.len() >= 3 => ("mcp.servers.remove", json!({ "server_id": pos[2] })),
        "start" if pos.len() >= 3 => ("mcp.servers.start", json!({ "server_id": pos[2] })),
        "stop" if pos.len() >= 3 => ("mcp.servers.stop", json!({ "server_id": pos[2] })),
        "probe" if pos.len() >= 3 => ("mcp.servers.probe", json!({ "server_id": pos[2] })),
        "tools" if pos.len() >= 3 => ("mcp.servers.tools", json!({ "server_id": pos[2] })),
        "call" if pos.len() >= 4 => (
            "mcp.servers.call",
            json!({
                "server_id": pos[2],
                "method": pos[3],
                "params_json": string_flag(args, "--params").unwrap_or_else(|| "{}".to_string())
            }),
        ),
        "tool-call" if pos.len() >= 4 => (
            "mcp.servers.tool_call",
            json!({
                "server_id": pos[2],
                "tool_name": pos[3],
                "arguments_json": string_flag(args, "--args-json").unwrap_or_else(|| "{}".to_string())
            }),
        ),
        _ => {
            eprintln!("usage: cli mcp servers list|add --name N --command CMD [--args \"...\"]|rm <id>|start <id>|stop <id>|probe <id>|tools <id>|call <id> <method> [--params JSON]|tool-call <id> <tool> [--args-json JSON]");
            std::process::exit(2);
        }
    };
    let result = backend_call_value(client, addr.as_deref(), method, params).unwrap_or_else(|err| {
        eprintln!("mcp error: {err}");
        std::process::exit(1);
    });
    print_value(&result, json_output);
}

fn handle_project_command(client: &mut JsonRpcClient<AgentService>, args: &[String]) {
    let json_output = has_flag(args, "--json");
    let addr = parse_addr_flag(args);
    let pos = positional_without_flags(args);
    if pos.is_empty() {
        eprintln!("usage: cli project open <path>|status [--path <path>]");
        std::process::exit(2);
    }
    let (method, params) = match pos[0].as_str() {
        "open" if pos.len() >= 2 => ("project.open", json!({ "path": pos[1] })),
        "status" => ("project.status", json!({ "path": string_flag(args, "--path") })),
        _ => {
            eprintln!("usage: cli project open <path>|status [--path <path>]");
            std::process::exit(2);
        }
    };
    let result = backend_call_value(client, addr.as_deref(), method, params).unwrap_or_else(|err| {
        eprintln!("project error: {err}");
        std::process::exit(1);
    });
    print_value(&result, json_output);
}

fn handle_audit_command(client: &mut JsonRpcClient<AgentService>, args: &[String]) {
    let json_output = has_flag(args, "--json");
    let addr = parse_addr_flag(args);
    let pos = positional_without_flags(args);
    if pos.is_empty() {
        eprintln!("usage: cli audit list|show ...");
        std::process::exit(2);
    }
    let (method, params) = match pos[0].as_str() {
        "list" => (
            "audit.list",
            json!({
                "session_id": string_flag(args, "--session"),
                "limit": string_flag(args, "--limit").and_then(|s| s.parse::<usize>().ok())
            }),
        ),
        "show" if pos.len() >= 2 => ("audit.get", json!({ "audit_id": pos[1] })),
        _ => {
            eprintln!("usage: cli audit list [--session <id>] [--limit N]|show <audit_id>");
            std::process::exit(2);
        }
    };
    let result = backend_call_value(client, addr.as_deref(), method, params).unwrap_or_else(|err| {
        eprintln!("audit error: {err}");
        std::process::exit(1);
    });
    print_value(&result, json_output);
}

fn handle_doctor_command(client: &mut JsonRpcClient<AgentService>, args: &[String]) {
    let json_output = has_flag(args, "--json");
    let strict = has_flag(args, "--strict");
    let addr = parse_addr_flag(args);
    let result = backend_call_value(client, addr.as_deref(), "system.health", json!({}))
        .unwrap_or_else(|err| {
            eprintln!("doctor error: {err}");
            std::process::exit(1);
        });

    if json_output {
        print_value(&result, true);
        if strict
            && result
                .get("ok")
                .and_then(|v| v.as_bool())
                .map(|ok| !ok)
                .unwrap_or(false)
        {
            std::process::exit(3);
        }
        return;
    }

    println!("backend: ok");
    let mut has_warnings = false;
    if let Some(obj) = result.as_object() {
        if let Some(ok) = obj.get("ok").and_then(|v| v.as_bool()) {
            if !ok {
                println!("status: warnings present");
                has_warnings = true;
            }
        }
        println!(
            "active_provider: {}",
            obj.get("active_provider")
                .and_then(|v| v.as_str())
                .unwrap_or("(none)")
        );
        println!(
            "provider_configs: {}",
            obj.get("provider_count").and_then(|v| v.as_u64()).unwrap_or(0)
        );
        println!(
            "pending_consents: {}",
            obj.get("pending_consents").and_then(|v| v.as_u64()).unwrap_or(0)
        );
        println!(
            "mcp_servers: {} total / {} running",
            obj.get("mcp_servers_total").and_then(|v| v.as_u64()).unwrap_or(0),
            obj.get("mcp_servers_running").and_then(|v| v.as_u64()).unwrap_or(0)
        );
        println!(
            "project: {}",
            obj.get("project_path")
                .and_then(|v| v.as_str())
                .unwrap_or("(not set)")
        );
        if let Some(warnings) = obj.get("warnings").and_then(|v| v.as_array()) {
            if !warnings.is_empty() {
                has_warnings = true;
                println!("warnings:");
                for warning in warnings {
                    if let Some(text) = warning.as_str() {
                        println!("  - {text}");
                    }
                }
            }
        }
    } else {
        print_value(&result, false);
    }

    if strict && has_warnings {
        std::process::exit(3);
    }
}

fn wire_result<T>(wire: WireResponse) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    if let Some(err) = wire.error {
        return Err(format!("json-rpc {}: {}", err.code, err.message));
    }
    let result = wire.result.ok_or_else(|| "missing json-rpc result".to_string())?;
    serde_json::from_value(result).map_err(|err| format!("invalid result payload: {err}"))
}

fn print_chat_response(response: &ChatResponse, json_output: bool) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(response).unwrap_or_else(|_| "{}".to_string())
        );
        return;
    }

    println!("audit_id: {}", response.audit_id);
    println!("request_fingerprint: {}", response.request_fingerprint);
    println!("execution_state: {}", response.execution_state);
    if let Some(consent_token) = &response.consent_token {
        println!("consent_token: {}", consent_token);
    }
    if let Some(consent) = &response.consent_request {
        println!("consent.scope: {}", consent.scope);
        println!("consent.summary: {}", consent.human_summary);
        if let Some(ttl) = consent.ttl_seconds {
            println!("consent.ttl_seconds: {}", ttl);
        }
        if let Some(expires_at) = consent.expires_at_unix_seconds {
            println!("consent.expires_at_unix_seconds: {}", expires_at);
        }
        println!(
            "consent.requires_extra_confirmation_click: {}",
            consent.requires_extra_confirmation_click
        );
        if !consent.risk_factors.is_empty() {
            println!("consent.risk_factors: {}", consent.risk_factors.join(", "));
        }
    }
    println!("actions: {}", response.actions_executed.join(", "));
    if !response.proposed_actions.is_empty() {
        println!("proposed_actions:");
        for evt in &response.proposed_actions {
            let mut line = format!("  - {} [{}] {}", evt.tool_name, evt.capability_tier, evt.status);
            if let Some(reason) = &evt.reason {
                line.push_str(&format!(" reason={}", reason));
            }
            if let Some(args) = &evt.arguments_preview {
                line.push_str(&format!(" args={}", args));
            }
            println!("{line}");
        }
    }
    if !response.executed_action_events.is_empty() {
        println!("executed_action_events:");
        for evt in &response.executed_action_events {
            let mut line = format!("  - {} [{}] {}", evt.tool_name, evt.capability_tier, evt.status);
            if let Some(args) = &evt.arguments_preview {
                line.push_str(&format!(" args={}", args));
            }
            if let Some(evidence) = &evt.evidence_summary {
                line.push_str(&format!(" evidence={}", evidence));
            }
            println!("{line}");
        }
    }
    if !response.action_events.is_empty() {
        println!("action_events:");
        for evt in &response.action_events {
            let mut line = format!("  - {} [{}] {}", evt.tool_name, evt.capability_tier, evt.status);
            if let Some(reason) = &evt.reason {
                line.push_str(&format!(" reason={}", reason));
            }
            if let Some(args) = &evt.arguments_preview {
                line.push_str(&format!(" args={}", args));
            }
            if let Some(evidence) = &evt.evidence_summary {
                line.push_str(&format!(" evidence={}", evidence));
            }
            println!("{line}");
        }
    }
    println!("response: {}", response.final_text);
}

fn call_http_jsonrpc(addr: &str, method: &str, params: Value) -> io::Result<WireResponse> {
    let body = serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    }))
    .map_err(|err| io::Error::other(format!("serialize request: {err}")))?;

    let mut stream = TcpStream::connect(addr)?;
    let request = format!(
        "POST /jsonrpc HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    let raw = String::from_utf8(bytes)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("utf8 response: {err}")))?;

    let (headers, body) = raw
        .split_once("\r\n\r\n")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed http response"))?;

    let status_line = headers.lines().next().unwrap_or_default();
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    if status_code != 200 {
        return Err(io::Error::other(format!("http {status_code}: {body}")));
    }

    serde_json::from_str::<WireResponse>(body)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("invalid json-rpc response: {err}")))
}

fn serve_http_jsonrpc(addr: &str) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    eprintln!("listening on http://{addr}/jsonrpc");

    let service = AgentService::new_for_platform("ipc-http");
    let mut server = JsonRpcServer::new(service);

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(err) => {
                eprintln!("accept error: {err}");
                continue;
            }
        };

        if let Err(err) = handle_http_connection(&mut stream, &mut server) {
            let _ = write_http_error(&mut stream, 500, "Internal Server Error", &format!("{err}"));
        }
    }

    Ok(())
}

fn handle_http_connection(stream: &mut TcpStream, server: &mut JsonRpcServer<AgentService>) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    let request_line = request_line.trim_end_matches(&['\r', '\n'][..]).to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        let line = line.trim_end_matches(&['\r', '\n'][..]);
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }

    if path != "/jsonrpc" {
        return write_http_error(stream, 404, "Not Found", "Use POST /jsonrpc");
    }

    if method == "OPTIONS" {
        return write_http_options(stream);
    }

    if method != "POST" {
        return write_http_error(stream, 405, "Method Not Allowed", "Use POST /jsonrpc");
    }

    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;
    let body_str = String::from_utf8(body)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("utf8 body: {err}")))?;

    let wire_req = match parse_wire_request(&body_str) {
        Ok(req) => req,
        Err(err) => {
            let payload = serde_json::to_string(&WireResponse {
                jsonrpc: "2.0".to_string(),
                id: Value::Null,
                result: None,
                error: Some(WireError {
                    code: -32700,
                    message: format!("parse error: {err}"),
                }),
            })
            .map_err(|e| io::Error::other(format!("serialize parse error response: {e}")))?;
            return write_http_json(stream, 200, &payload);
        }
    };

    let raw_req = Request::new(
        json_value_to_id(wire_req.id.unwrap_or(Value::Null)),
        wire_req.method,
        wire_req.params.to_string(),
    );
    let wire_resp = to_wire_response(server.handle(raw_req));
    let payload = serde_json::to_string(&wire_resp)
        .map_err(|err| io::Error::other(format!("serialize response: {err}")))?;
    write_http_json(stream, 200, &payload)
}

fn write_http_json(stream: &mut TcpStream, status: u16, body: &str) -> io::Result<()> {
    let status_text = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\n\r\n{}",
        status,
        status_text,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn write_http_error(stream: &mut TcpStream, status: u16, status_text: &str, message: &str) -> io::Result<()> {
    let body = serde_json::json!({ "error": message }).to_string();
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\n\r\n{}",
        status,
        status_text,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn write_http_options(stream: &mut TcpStream) -> io::Result<()> {
    let response = "HTTP/1.1 204 No Content\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nContent-Length: 0\r\n\r\n";
    stream.write_all(response.as_bytes())?;
    stream.flush()
}
