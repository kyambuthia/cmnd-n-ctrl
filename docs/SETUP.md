# Setup

## Build Artifacts (Important)

This repo is configured to keep Cargo build artifacts out of the project directory.

- Default Cargo target dir is set in `.cargo/config.toml`
- Artifacts are written outside the repo by default (on this machine: `/home/mbuthi/.cmnd-n-ctrl-target`)
- This avoids creating `target/` folders in the repo during normal development

## Quickstart (Terminal)

1. Verify core crates:
   - `cd core && cargo test -p agent -p ipc -p providers -p actions -p plugins`
2. Verify CLI:
   - `cargo run -p cli -- --help`
3. List tools:
   - `cargo run -p cli -- tools`
4. Try chat:
   - `cargo run -p cli -- chat "hello"`
   - `cargo run -p cli -- chat "please use tool:"`
   - `cargo run -p cli -- chat "please use tool:" --require-confirmation`

## One-Command Local Prototype Runner

From the repo root:

```bash
bash scripts/dev-prototype.sh
```

This starts:
- local JSON-RPC backend on `http://127.0.0.1:7777/jsonrpc`
- static UI server for `apps/desktop-tauri/src` on `http://127.0.0.1:8080`

Optional overrides:
- `API_ADDR=127.0.0.1:9000 bash scripts/dev-prototype.sh`
- `UI_PORT=8090 bash scripts/dev-prototype.sh`

## Overriding the Target Directory

For a single command:

```bash
CARGO_TARGET_DIR=/tmp/cmnd-n-ctrl-target cargo run -p cli -- --help
```

For your current shell session:

```bash
export CARGO_TARGET_DIR=/tmp/cmnd-n-ctrl-target
```

## Desktop Tauri Backend Scaffold Check

The Tauri backend is a scaffold and can be checked independently:

- `cargo check --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml`

The frontend UI is present, but end-to-end Tauri wiring to a running local JSON-RPC server is still a TODO in this scaffold.
