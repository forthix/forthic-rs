#!/usr/bin/env bash
# Cross-runtime smoke: start the rs JSON-RPC server, drive it with the real
# forthic-ts JsonRpcClient (see smoke_ts_client.cjs).
#
# Requires: node, and a built forthic-ts checkout (npm run build) at
# FORTHIC_TS_DIR (default: ../forthic-ts relative to this repo).
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FORTHIC_TS_DIR="${FORTHIC_TS_DIR:-$REPO_DIR/../forthic-ts}"
PORT="${PORT:-18999}"

if [ ! -f "$FORTHIC_TS_DIR/dist/cjs/jsonrpc/client.js" ]; then
  echo "forthic-ts dist not found at $FORTHIC_TS_DIR (set FORTHIC_TS_DIR; run npm run build there)" >&2
  exit 2
fi

cargo build --features jsonrpc --quiet
"$REPO_DIR/target/debug/forthic-jsonrpc" --port "$PORT" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT

# Wait for the server to accept connections
for _ in $(seq 1 50); do
  if curl -s -o /dev/null -X POST "127.0.0.1:$PORT/rpc" \
      -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":0,"method":"listModules","params":{}}'; then
    break
  fi
  sleep 0.1
done

node "$REPO_DIR/scripts/smoke_ts_client.cjs" "$PORT" "$FORTHIC_TS_DIR"
