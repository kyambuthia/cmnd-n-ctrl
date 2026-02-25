# Desktop Tauri App (Scaffold)

Minimal Tauri v2 shell with a TypeScript UI that talks to the local JSON-RPC endpoint.

## Dev Notes
- Fastest local prototype loop today:
  - One command from repo root: `bash scripts/dev-prototype.sh`
  - Or manually:
    - Terminal 1: `cargo run -p cli -- serve-http`
    - Terminal 2: serve/open `apps/desktop-tauri/src/index.html` (any static file server is fine)
  - The frontend posts JSON-RPC to `http://127.0.0.1:7777/jsonrpc`
- `src-tauri` now includes a compileable desktop bridge backend that can:
  - auto-spawn `cargo run -p cli -- serve-http`
  - forward JSON-RPC payloads to the local backend
  - expose a `jsonrpc_request(payload_json)` function ready for Tauri command wiring
- Current limitation: Tauri v2 dependencies/command registration are still not added, so the native `invoke('jsonrpc_request')` path remains a scaffold contract until the next step.
- In production, connect to a local IPC socket/pipe managed by the app backend.
- The frontend currently demonstrates the UI and a fetch-based JSON-RPC call to the local HTTP JSON-RPC dev server.

## Native Tauri Dev (WIP, Now Scaffolded)
1. Install dependencies:
   - `cd apps/desktop-tauri`
   - `npm install`
   - Linux host packages (Debian/Ubuntu, required for Tauri/WebKitGTK build):
     - `sudo apt-get update`
     - `sudo apt-get install -y build-essential pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev libayatana-appindicator3-dev librsvg2-dev`
2. Start frontend static server (Terminal 1):
   - `npm run dev`
3. Start Tauri shell (Terminal 2):
   - `cargo run --manifest-path src-tauri/Cargo.toml --features tauri-app` (compile/runtime scaffold)
   - or `npm run tauri:dev` once Tauri toolchain prerequisites are installed

Notes:
- The feature-gated Tauri backend registers a real `jsonrpc_request` command and manages a local CLI backend bridge.
- The frontend already prefers `invoke('jsonrpc_request')` when running under Tauri and falls back to HTTP in browser/dev mode.
- If Linux build fails with `gdk-3.0` / `pkg-config` errors, the GTK3/WebKitGTK development packages above are missing.

## Backend Bridge Env (src-tauri)
- `CMND_N_CTRL_BACKEND_ADDR` (default `127.0.0.1:7777`)
- `CMND_N_CTRL_AUTOSPAWN_BACKEND=0` to disable child auto-spawn and require an already-running backend
