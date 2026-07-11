test:
	cargo test --all-features

# Cross-runtime smoke: drives this repo's JSON-RPC server with the real
# forthic-ts JsonRpcClient. Needs a built forthic-ts checkout
# (FORTHIC_TS_DIR, default ../forthic-ts).
smoke-ts:
	./scripts/smoke_ts_client.sh
