# JSON-RPC Server Plan for forthic-rs

> **Status (2026-07-10): Phases 1–4 complete.** Phase 5 (rust client)
> deliberately deferred. See Follow-ups for the error-location work and
> plans/TS-PARITY-BACKLOG.md for the broader parity items.

Port of the forthic-ts JSON-RPC 2.0 server (`src/jsonrpc/` in forthic-ts) to Rust,
including the security hardening from ts PR #25. Goal: a forthic-ts `JsonRpcClient`
(or any runtime) can call into the Rust runtime with zero client changes — same
methods, same wire format, same error codes.

## Reference (forthic-ts)

- `src/jsonrpc/server.ts` — HTTP POST `/rpc` (and `/`), JSON-RPC 2.0 envelopes,
  four methods: `executeWord`, `executeSequence`, `listModules`, `getModuleInfo`.
  Batch envelopes rejected with -32600. Fresh interpreter per request.
- `src/grpc/serializer.ts` — tagged `StackValue` JSON format shared by both transports.
- `src/jsonrpc/errors.ts` — standard codes plus `RuntimeError` (-32000, data = ErrorInfo)
  and `ModuleNotFound` (-32001).
- Hardening (ts #25): loopback bind by default, optional bearer token (constant-time
  compare, checked before body read), 1 MiB body cap (Content-Length precheck + streaming
  enforcement), error sanitization (no stack traces / server paths by default).

## Wire format (must match exactly)

Request/response keys are snake_case: `word_name`, `word_names`, `stack`,
`result_stack`, `module_name`, `modules`, `word_count`, `runtime_specific`,
`stack_effect`. `ErrorInfo.runtime` is `"rust"`.

StackValue tags: `int_value`, `float_value`, `string_value`, `bool_value`,
`null_value` (empty object), `array_value.items`, `record_value.fields`,
`instant_value.iso8601`, `plain_date_value.iso8601_date`,
`zoned_datetime_value.{iso8601, timezone}`.

ForthicValue mapping:

| ForthicValue | serialize → | deserialize ← |
|---|---|---|
| `Null` | `null_value: {}` | `null_value` |
| `Bool` | `bool_value` | `bool_value` |
| `Int` | `int_value` | `int_value` |
| `Float` | `float_value` | `float_value` |
| `String` | `string_value` | `string_value` |
| `Array` | `array_value` | `array_value` |
| `Record` | `record_value` | `record_value` |
| `Date` | `plain_date_value` | `plain_date_value` |
| `Time` | `plain_time_value` (coordinated ts/rs extension, proto field 11) | `plain_time_value` |
| `DateTime` | `zoned_datetime_value` (RFC3339 + tz name) | `zoned_datetime_value`; `instant_value` → `DateTime` in UTC |
| `WordOptions`, `StartArrayMarker` | serialization error (interpreter-internal; never on the wire) | — |

## Design decisions

0. **The interpreter stays synchronous — settled.** forthic-ts is async because JS
   must be for I/O; Rust doesn't share that constraint. Asyncifying would infect the
   recursive `run` → `Word::execute` chain (async-trait boxing, `Box::pin` recursion,
   Send bounds across await points) to benefit words that don't exist yet. I/O words,
   when they arrive, use blocking clients or `Handle::block_on`. Async lives only at
   transport edges (this server). Consequence: tokio and async-trait were removed
   from base `[dependencies]`; tokio returns as an optional dep of the `jsonrpc`
   feature with trimmed features (`rt-multi-thread`, `net`, `macros`, `signal`).
   Revisit only if the roadmap becomes thousands of concurrent long-lived I/O-bound
   interpreter sessions per process.
1. **HTTP stack: axum 0.7+, feature-gated.** ts used Node's built-in `http` for zero
   deps; Rust has no std HTTP server, so some dep is unavoidable. axum is the tokio
   project's own, gives us method routing (405), and `DefaultBodyLimit`/manual cap
   for 413. Everything (axum, tokio, subtle) lives behind a new `jsonrpc` cargo
   feature so library consumers pay nothing.
2. **Fresh `Interpreter` per request** (matches ts semantics and sidesteps shared-state
   questions). The sync interpreter runs inside `tokio::task::spawn_blocking`:
   deserialize params → spawn_blocking closure constructs interpreter, pushes stack,
   runs, returns serialized result. `Word: Send + Sync` already, so this is
   unconstrained.
3. **Standard interpreter builder.** Unlike ts's `StandardInterpreter`, `Interpreter::new`
   doesn't preload the standard library. Add `Interpreter::standard(timezone)` (not
   jsonrpc-gated — generally useful) that imports all 8 standard modules
   (core, array, boolean, datetime, json, math, record, string) with no prefix.
4. **Runtime modules registry starts empty.** ts registers `FsModule`; rs has no
   runtime-specific modules yet. `listModules` returns `[]`, `getModuleInfo` returns
   -32001 for anything. Keep the registry structure so future modules slot in.
5. **Manual CLI arg parsing** (`--port`, `--host`, `--token`) like ts `main()` — avoids
   pulling clap into the `jsonrpc` feature.
6. **Constant-time token compare** via the `subtle` crate (tiny, no transitive deps).

## Phases

### Phase 1: Serializer (`src/jsonrpc/serializer.rs`)

- `serialize_value(&ForthicValue) -> Result<serde_json::Value, SerializeError>` and
  `deserialize_value(&serde_json::Value) -> Result<ForthicValue, SerializeError>`,
  with path tracking for error messages (mirrors ts `path` param, e.g. `at path: [2].key`).
- Recursive for arrays/records. Dates via chrono: `Date` ↔ `%Y-%m-%d`;
  `DateTime` → RFC3339 + `Tz::name()`; parse `zoned_datetime_value` by applying the
  named tz, `instant_value` as UTC.
- Unit tests: round-trip every variant, nested structures, the unsupported types,
  and exact-JSON fixtures copied from ts serializer output for cross-runtime parity.

### Phase 2: Servicer + dispatch (`src/jsonrpc/server.rs`, `src/jsonrpc/errors.rs`)

- `errors.rs`: `JsonRpcErrorCode` consts (-32700, -32600, -32601, -32602, -32603,
  -32000 RuntimeError, -32001 ModuleNotFound); `ErrorInfo` struct
  `{message, runtime: "rust", error_type, context, stack_trace?, word_location?}`.
- Servicer with the four methods; param validation errors → -32602 with the same
  message texts as ts (`executeWord requires string "word_name"` etc.).
- Forthic execution errors → -32000, `data` = ErrorInfo. `error_type` from the
  `ForthicError` variant name; `context.word_name` / `context.word_sequence` as in ts.
  Location/`forthic`-snippet detail stripped unless `expose_error_details` is set
  (rs analog of ts `exposeStackTraces`).
- `Interpreter::standard(timezone)` added in `src/interpreter.rs` + used here.

### Phase 3: HTTP layer + hardening

- `serve(port, ServeOptions) -> ServerHandle` (handle exposes bound addr for
  port-0 tests, and graceful shutdown).
- `ServeOptions { host, token, max_body_bytes, expose_error_details }`, each falling
  back to `FORTHIC_JSONRPC_HOST` / `FORTHIC_JSONRPC_TOKEN` /
  `FORTHIC_JSONRPC_MAX_BODY_BYTES` env vars; defaults `127.0.0.1`, no token, 1 MiB, false.
- Request pipeline, in order (parity with ts): non-POST → 405 + `Allow: POST`;
  path not `/rpc` or `/` → 404; auth check **before** body read → 401 +
  `WWW-Authenticate: Bearer`; content-type must contain `application/json` → 415;
  Content-Length > cap → 413 JSON-RPC error; body cap enforced while reading;
  JSON parse failure → 200 + -32700; batch array → 200 + -32600; envelope must have
  `jsonrpc: "2.0"`, string `method`, and an `id` member → else -32600.
- Loud startup warning when bound non-loopback without a token.
- `src/bin/forthic-jsonrpc.rs` (`required-features = ["jsonrpc"]`): manual flag
  parsing, default port 8765, prints listening host:port and loaded runtime modules.
- Cargo: `jsonrpc = ["dep:axum", "dep:tokio", "dep:subtle"]` feature (tokio optional,
  features trimmed to `rt-multi-thread`, `net`, `macros`, `signal`); `reqwest` as
  dev-dependency for tests.

### Phase 4: Tests + cross-runtime verification

- `tests/jsonrpc_server_test.rs` — port of ts `server.test.ts`: happy paths for all
  four methods, stack round-tripping through `executeWord`/`executeSequence`,
  unknown word → -32000 with ErrorInfo, unknown method → -32601, bad params → -32602,
  batch rejection, malformed JSON → -32700.
- `tests/jsonrpc_hardening_test.rs` — port of `server_hardening.test.ts`: default
  loopback bind, 401 without/with-wrong token, success with token, 413 via
  Content-Length and via oversized body, error responses contain no location/snippet
  detail by default and do when `expose_error_details` is on.
- Cross-runtime smoke (Makefile target): start `forthic-jsonrpc`, run forthic-ts's
  `JsonRpcClient` against it (`executeWord` with a mixed-type stack incl. dates),
  assert round-trip equality. This is the real compatibility proof.

### Phase 5 (optional, defer): Rust client

`src/jsonrpc/client.rs` mirroring ts `JsonRpcClient` (reqwest-based) — only needed
when the Rust runtime wants to call *other* runtimes. Not required for the primary
goal (rs as a callable runtime), so skip until there's a use case.

## Follow-ups (after Phase 4)

- **Thread code locations into interpreter errors.** Every ForthicError has
  `location: Option<CodeLocation>` and every Token carries a CodeLocation,
  but all ~10 interpreter error sites hardcode `location: None` (only the
  tokenizer fills it — e.g. UnterminatedString). Consequence: the server's
  `expose_error_details` option only ever yields `word_location` for
  tokenizer errors; UnknownWord/StackUnderflow report no position. Fix:
  `ForthicError::with_location(loc)` helper that fills the field if None,
  applied at the token handlers (e.g.
  `find_word(&token.string).map_err(|e| e.with_location(token.location.clone()))`).
  Second, larger step: per-definition location tracking as in forthic-ts #30.
- **plain_time for other runtimes** if/when python/ruby servers exist
  (proto field 11 is the contract; ts #36 / rs commit are the references).

## Out of scope

- gRPC transport (ts unhooked its gRPC surface in #22; JSON-RPC is the path forward).
- Batch JSON-RPC support (explicitly rejected, matching ts).
- Runtime-specific modules (fs etc.) — separate effort; the registry is ready for them.
