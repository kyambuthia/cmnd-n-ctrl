# Setup

## Build Artifacts (Important)

This repo is configured to use a repo-local Cargo target directory.

- Default Cargo target dir is set in `.cargo/config.toml`
- Artifacts are written to `./target`
- `.gitignore` excludes `target/`

## Quickstart (Terminal)

1. Verify core crates:
   - `cd core && cargo test -p agent -p ipc -p providers -p actions -p plugins`
2. Verify CLI:
   - `cargo run -p cli -- --help`
3. Start interactive CLI (TUI when available, REPL fallback otherwise):
   - `cargo run -p cli --`
   - TUI shortcuts:
     - `Tab` switch pane
     - `j` / `k` move selection
     - `v` toggle selected execution details
     - `a` / `d` approve or deny selected consent
     - `n` new session, `x` delete session, `q` quit
4. List tools:
   - `cargo run -p cli -- tools`
5. Try natural-language chat:
   - `cargo run -p cli -- chat "Open https://example.com" --require-confirmation`
   - `cargo run -p cli -- chat "List applications running" --require-confirmation`

### Natural Language Rule
- User-facing CLI/TUI/REPL chat enforces natural-language-only prompts.
- Explicit `tool:` syntax is blocked in those surfaces.
- Backend compatibility still supports legacy `tool:` parsing for non-UI callers/tests.

## One-Command Local Prototype Runner

From the repo root:

```bash
bash scripts/dev-prototype.sh
```

This starts:
- local JSON-RPC backend on `http://127.0.0.1:7777/jsonrpc`
- static UI server for `apps/desktop-tauri/src` on `http://127.0.0.1:8080`
- Desktop feed UX includes:
  - execution history filter (`Filter executions...`)
  - inline replay buttons on history entries with prompts
  - natural-language input enforcement (`tool:` syntax is blocked)

Optional overrides:
- `API_ADDR=127.0.0.1:9000 bash scripts/dev-prototype.sh`
- `UI_PORT=8090 bash scripts/dev-prototype.sh`

## Overriding the Target Directory

For a single command:

```bash
CARGO_TARGET_DIR=/tmp/cmnd-n-ctrl-target cargo run -p cli -- --help
```

## Desktop Tauri Backend Scaffold Check

The Tauri backend is a scaffold and can be checked independently:

- `cargo check --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml`

The frontend UI and local JSON-RPC bridge are present; production-hardening and full feature parity are still in progress.
