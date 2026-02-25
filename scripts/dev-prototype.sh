#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
API_ADDR="${API_ADDR:-127.0.0.1:7777}"
UI_PORT="${UI_PORT:-8080}"
UI_HOST="${UI_HOST:-127.0.0.1}"
UI_DIR="$ROOT_DIR/apps/desktop-tauri/src"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found in PATH" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 not found in PATH (needed for static UI server)" >&2
  exit 1
fi

backend_pid=""
ui_pid=""

cleanup() {
  if [[ -n "$ui_pid" ]] && kill -0 "$ui_pid" 2>/dev/null; then
    kill "$ui_pid" 2>/dev/null || true
    wait "$ui_pid" 2>/dev/null || true
  fi
  if [[ -n "$backend_pid" ]] && kill -0 "$backend_pid" 2>/dev/null; then
    kill "$backend_pid" 2>/dev/null || true
    wait "$backend_pid" 2>/dev/null || true
  fi
}

trap cleanup EXIT INT TERM

echo "Starting local JSON-RPC server on http://$API_ADDR/jsonrpc"
(
  cd "$ROOT_DIR"
  cargo run -p cli -- serve-http --addr "$API_ADDR"
) &
backend_pid=$!

echo "Starting static UI server on http://$UI_HOST:$UI_PORT"
python3 -m http.server "$UI_PORT" --bind "$UI_HOST" -d "$UI_DIR" &
ui_pid=$!

cat <<EOF

Prototype dev loop is running.

- Backend: http://$API_ADDR/jsonrpc
- UI:      http://$UI_HOST:$UI_PORT

Try:
- hello
- please use tool:
- toggle "require confirmation"

Press Ctrl+C to stop both processes.
EOF

wait "$backend_pid"
