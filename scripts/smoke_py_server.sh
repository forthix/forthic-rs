#!/usr/bin/env bash
# Cross-runtime smoke: start the forthic-py JSON-RPC server, drive it with
# the rs JsonRpcClient (examples/smoke_client.rs).
#
# Requires: uv, and a forthic-py checkout at FORTHIC_PY_DIR (default:
# ../forthic-py relative to this repo).
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FORTHIC_PY_DIR="${FORTHIC_PY_DIR:-$REPO_DIR/../forthic-py}"
PORT="${PORT:-18995}"

if [ ! -f "$FORTHIC_PY_DIR/pyproject.toml" ]; then
  echo "forthic-py checkout not found at $FORTHIC_PY_DIR (set FORTHIC_PY_DIR)" >&2
  exit 2
fi

cargo build --features jsonrpc --example smoke_client --quiet

# No subshell: $! must be the process we can actually kill. Server output goes
# to /dev/null so a surviving child can never hold this script's stdout open.
(cd "$FORTHIC_PY_DIR" && exec uv run --no-sync python -m forthic.jsonrpc.server --port "$PORT") \
  > /dev/null 2>&1 &
SERVER_PID=$!
# `uv run` spawns python as a child; kill both.
trap 'pkill -P "$SERVER_PID" 2>/dev/null || true; kill "$SERVER_PID" 2>/dev/null || true' EXIT

for _ in $(seq 1 50); do
  if curl -s -o /dev/null -X POST "127.0.0.1:$PORT/rpc" \
      -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":0,"method":"listModules","params":{}}'; then
    break
  fi
  sleep 0.1
done

"$REPO_DIR/target/debug/examples/smoke_client" "$PORT" python UnknownWordError
