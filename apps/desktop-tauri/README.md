# Desktop Tauri App (Scaffold)

Minimal Tauri v2 shell with a TypeScript UI that talks to the local JSON-RPC endpoint.

## Dev Notes
- In development, spawn the Rust core/CLI child process and communicate over stdio or local socket.
- In production, connect to a local IPC socket/pipe managed by the app backend.
- The frontend currently demonstrates the UI and a fetch-based JSON-RPC call stub.
