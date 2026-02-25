# Desktop Tauri App (Scaffold)

Minimal Tauri v2 shell with a TypeScript UI that talks to the local JSON-RPC endpoint.

## Dev Notes
- Fastest local prototype loop today:
  - Terminal 1: `cargo run -p cli -- serve-http`
  - Terminal 2: serve/open `apps/desktop-tauri/src/index.html` (any static file server is fine)
  - The frontend posts JSON-RPC to `http://127.0.0.1:7777/jsonrpc`
- In development, Tauri backend can later spawn the Rust core/CLI child process and communicate over stdio or local socket.
- In production, connect to a local IPC socket/pipe managed by the app backend.
- The frontend currently demonstrates the UI and a fetch-based JSON-RPC call to the local HTTP JSON-RPC dev server.
