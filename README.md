# Cross-Platform AI Assistant Monorepo

This repository contains a security-first, cross-platform AI assistant architecture with:
- Rust core for orchestration, policy, tools, and IPC
- Rust CLI client
- Tauri v2 desktop app shell
- Android/iOS shell stubs with UniFFI bridge placeholders
- Provider and plugin abstractions (JSON-RPC 2.0 / MCP-compatible framing)

Build targets prepared in this scaffold are Windows and Linux only. Mobile folders are stubs and no mobile CI/build pipelines are configured.

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
4. Desktop shell scaffold: `cd apps/desktop-tauri && npm install && npm run tauri dev` (after installing Tauri v2 prerequisites)

See `docs/threat-model.md` and `docs/permissions-matrix.md` before enabling real platform actions or model providers.

## Product Roadmap
- See `docs/ROADMAP.md` for the phased plan to ship a useful Windows/Linux desktop operator agent (browser + file workflows first, desktop app automation next).
