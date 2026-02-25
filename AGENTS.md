# AGENTS.md

## Repo Navigation
- `core/`: Rust workspace for agent orchestration, IPC, providers, actions, plugins, and the CLI (via workspace member `../cli`).
- `cli/`: Rust CLI client that calls the local IPC server APIs.
- `apps/desktop-tauri/`: Tauri v2 desktop shell (Rust backend + minimal TypeScript UI).
- `apps/android/`: Android shell stubs (Kotlin).
- `apps/ios/`: iOS shell stubs (Swift).
- `schemas/`: JSON schemas for tools and policy config.
- `docs/`: Threat model and permissions matrix.
- `ci/github-actions/`: Example CI workflows.

## Build and Test Commands
- Rust tests (run from `core/`): `cargo test -p agent -p ipc -p providers -p actions -p plugins`
- CLI help (run from repo root): `cargo run -p cli -- --help`
- CLI TUI (run from repo root): `cargo run -p cli -- tui`
- Full core workspace check (run from `core/`): `cargo check --workspace`
- Local backend server (HTTP): `cargo run -p cli -- serve-http`
- Local backend server (stdio/MCP framing): `cargo run -p cli -- serve-stdio`

## CLI Command Overview (OpenCode-like Surface)
- `auth login|list|logout`
- `providers list|set|config-get|config-set`
- `session new|list|open|rm|append`
- `chat`, `tools`, `rpc`
- `consent list|approve|deny`
- `mcp servers list|add|rm|start|stop`
- `project open|status`
- `audit list|show`
- `tui` (minimal terminal UI shell)
  - TUI keys: `Tab` switch pane, `Enter` activate/send/approve, `n` new session, `x` delete session, `a` approve consent, `d` deny consent, `c` toggle confirmation mode, `r` refresh, `q` quit

## Consent Flow Test (End-to-End)
- Start backend: `cargo run -p cli -- serve-http`
- Create a session (optional): `cargo run -p cli -- session new --json --addr 127.0.0.1:7777`
- Trigger a consent-required action: `cargo run -p cli -- chat "tool:activate Browser" --require-confirmation --json --addr 127.0.0.1:7777`
- List pending approvals: `cargo run -p cli -- consent list --json --addr 127.0.0.1:7777`
- Approve: `cargo run -p cli -- consent approve <consent-id> --json --addr 127.0.0.1:7777`
- Or deny: `cargo run -p cli -- consent deny <consent-id> --json --addr 127.0.0.1:7777`

## Desktop Tauri (Dev)
- `cd apps/desktop-tauri`
- Install frontend deps (example): `npm install`
- Run the UI + Tauri shell (after adding Tauri toolchain/deps): `npm run tauri dev`
- In dev mode, the UI can spawn/connect to a local IPC child process; production should use a local socket/pipe.
- Prototype webview harness (current): `bash scripts/dev-prototype.sh` (starts local HTTP JSON-RPC + static UI server)
