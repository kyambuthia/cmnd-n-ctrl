use std::env;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

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
    println!("  cli tools [--json] [--raw]");
    println!("  cli chat <message> [--provider <name>] [--require-confirmation] [--json]");
    println!("  cli rpc <method> <params-json>");
    println!("  cli serve-stdio");
    println!("  cli serve-http [--addr <host:port>]");
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
            let json_output = args.iter().any(|a| a == "--json");
            let raw_output = args.iter().any(|a| a == "--raw");
            let tools = client.tools_list();
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

            if raw_output {
                let raw = client.call_raw(Request::new(Id::Number(1), "tools.list", "{}"));
                if let Some(result) = raw.result_json {
                    eprintln!("raw json-rpc result: {result}");
                }
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
            let mut json_output = false;
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

            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
                );
            } else {
                println!("audit_id: {}", response.audit_id);
                println!("request_fingerprint: {}", response.request_fingerprint);
                println!("actions: {}", response.actions_executed.join(", "));
                if !response.proposed_actions.is_empty() {
                    println!("proposed_actions:");
                    for evt in &response.proposed_actions {
                        let mut line = format!(
                            "  - {} [{}] {}",
                            evt.tool_name, evt.capability_tier, evt.status
                        );
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
                        let mut line = format!(
                            "  - {} [{}] {}",
                            evt.tool_name, evt.capability_tier, evt.status
                        );
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
                        let mut line = format!(
                            "  - {} [{}] {}",
                            evt.tool_name, evt.capability_tier, evt.status
                        );
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
        }
        "rpc" => {
            if args.len() < 3 {
                eprintln!("error: usage: cli rpc <method> <params-json>");
                print_help();
                std::process::exit(2);
            }
            let response = client.call_raw(Request::new(
                Id::Number(1),
                args[1].clone(),
                args[2].clone(),
            ));
            let wire = to_wire_response(response);
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
