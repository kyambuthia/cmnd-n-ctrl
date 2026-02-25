use std::env;

use agent::AgentService;
use ipc::jsonrpc::{Id, Request};
use ipc::{ChatMode, ChatRequest, JsonRpcClient, JsonRpcServer, ProviderConfig};

fn print_help() {
    println!("cli - Rust CLI client for local assistant IPC");
    println!();
    println!("USAGE:");
    println!("  cli --help");
    println!("  cli tools");
    println!("  cli chat <message> [--provider <name>] [--require-confirmation]");
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
        _ => {
            eprintln!("error: unknown command '{}'.", args[0]);
            print_help();
            std::process::exit(2);
        }
    }
}
