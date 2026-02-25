# Cross-Platform AI Assistant Monorepo (Scaffold)

This repository scaffolds a security-first, cross-platform AI assistant architecture with:
- Rust core for orchestration, policy, tools, and IPC
- Rust CLI client
- Tauri v2 desktop app shell
- Android/iOS shell stubs with UniFFI bridge placeholders
- Provider and plugin abstractions (JSON-RPC 2.0 / MCP-compatible framing)

## Layout
- `core/` Rust workspace (core crates + `../cli` as workspace member)
- `cli/` CLI client
- `apps/` desktop/mobile shells
- `schemas/` JSON schemas
- `docs/` security docs

## Quickstart
1. `cd core && cargo check --workspace`
2. `cd core && cargo test -p agent -p ipc -p providers -p actions -p plugins`
3. `cargo run -p cli -- --help`

See `docs/threat-model.md` and `docs/permissions-matrix.md` before enabling real platform actions or model providers.
