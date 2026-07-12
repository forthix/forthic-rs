.PHONY: test smoke-ts smoke-ts-server smoke-py-server smoke-all

test:
	cargo test --all-features

# --- Cross-runtime smoke ---
#
# Three directions from this repo, proving the JSON-RPC wire format in both
# roles:
#   smoke-ts         ts client <- rs server   (this repo serves)
#   smoke-ts-server  rs client -> ts server   (this repo calls)
#   smoke-py-server  rs client -> py server   (this repo calls)
# forthic-py carries the mirrored pair (ts client -> py server, py client ->
# rs server).

# Drives this repo's JSON-RPC server with the real forthic-ts JsonRpcClient.
# Needs a built forthic-ts checkout (FORTHIC_TS_DIR, default ../forthic-ts).
smoke-ts:
	./scripts/smoke_ts_client.sh

# Drives the forthic-ts JSON-RPC server with this repo's JsonRpcClient.
# Needs a built forthic-ts checkout (FORTHIC_TS_DIR, default ../forthic-ts).
smoke-ts-server:
	./scripts/smoke_ts_server.sh

# Drives the forthic-py JSON-RPC server with this repo's JsonRpcClient.
# Needs a forthic-py checkout (FORTHIC_PY_DIR, default ../forthic-py).
smoke-py-server:
	./scripts/smoke_py_server.sh

smoke-all: smoke-ts smoke-ts-server smoke-py-server
