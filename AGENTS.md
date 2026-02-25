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
- Full core workspace check (run from `core/`): `cargo check --workspace`

## Desktop Tauri (Dev)
- `cd apps/desktop-tauri`
- Install frontend deps (example): `npm install`
- Run the UI + Tauri shell (after adding Tauri toolchain/deps): `npm run tauri dev`
- In dev mode, the UI can spawn/connect to a local IPC child process; production should use a local socket/pipe.
