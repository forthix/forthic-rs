//! HTTP transport for the JSON-RPC server (axum)
//!
//! Wraps [`dispatch`](super::server::dispatch) in an HTTP POST endpoint at
//! `/rpc` (and `/`), with the hardening measures from forthic-ts #25 — the
//! server executes caller-supplied Forthic code, so the defaults are
//! deliberately conservative:
//!
//! - Binds `127.0.0.1` unless overridden (`ServeOptions.host` /
//!   `FORTHIC_JSONRPC_HOST`). Binding non-loopback without a token logs a
//!   loud warning.
//! - Optional bearer-token auth (`token` / `FORTHIC_JSONRPC_TOKEN`),
//!   checked in constant time BEFORE the body is read.
//! - Request body cap (default 1 MiB), rejected up front via Content-Length
//!   and enforced while streaming.
//! - JSON-RPC envelope validation; batch arrays rejected with -32600.
//! - Error details (`word_location`) stripped unless
//!   `expose_error_details` is set.
//!
//! The sync interpreter runs on the blocking thread pool via
//! `spawn_blocking`; each request gets a fresh interpreter (see servicer).

use super::errors::JsonRpcErrorCode;
use super::server::{dispatch, ForthicJsonRpcServicer, JsonRpcRequest};
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Default cap on request body size (1 MiB)
const DEFAULT_MAX_BODY_BYTES: usize = 1024 * 1024;

/// Options controlling how the JSON-RPC server is exposed
///
/// Each `None` field falls back to its `FORTHIC_JSONRPC_*` environment
/// variable, then to the conservative default.
#[derive(Debug, Clone, Default)]
pub struct ServeOptions {
    /// Interface to bind. Default `127.0.0.1` (loopback only). Set
    /// `0.0.0.0` to accept remote connections — only together with `token`.
    /// Env: `FORTHIC_JSONRPC_HOST`.
    pub host: Option<String>,
    /// Shared secret. When set, every request must send
    /// `Authorization: Bearer <token>`; others get 401.
    /// Env: `FORTHIC_JSONRPC_TOKEN`.
    pub token: Option<String>,
    /// Maximum request body in bytes. Env: `FORTHIC_JSONRPC_MAX_BODY_BYTES`.
    pub max_body_bytes: Option<usize>,
    /// Include code locations in error responses. Off by default; for
    /// local debugging only (rs analog of ts `exposeStackTraces`).
    pub expose_error_details: bool,
}

struct ServerContext {
    servicer: ForthicJsonRpcServicer,
    token: Option<String>,
    max_body_bytes: usize,
    expose_error_details: bool,
}

/// Handle to a running server: bound address + graceful shutdown
pub struct ServerHandle {
    addr: SocketAddr,
    shutdown_tx: oneshot::Sender<()>,
    join: tokio::task::JoinHandle<()>,
}

impl ServerHandle {
    /// The actual bound address (useful with port 0)
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Signal graceful shutdown and wait for the server to stop
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
        let _ = self.join.await;
    }
}

/// Start the JSON-RPC server on `port` (0 picks a free port)
pub async fn serve(port: u16, options: ServeOptions) -> std::io::Result<ServerHandle> {
    let host = options
        .host
        .or_else(|| std::env::var("FORTHIC_JSONRPC_HOST").ok())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let token = options
        .token
        .or_else(|| std::env::var("FORTHIC_JSONRPC_TOKEN").ok());
    let max_body_bytes = options
        .max_body_bytes
        .or_else(|| {
            std::env::var("FORTHIC_JSONRPC_MAX_BODY_BYTES")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(DEFAULT_MAX_BODY_BYTES);

    if !is_loopback_host(&host) && token.is_none() {
        eprintln!(
            "⚠ SECURITY: JSON-RPC server bound to non-loopback host '{host}' without an \
             auth token. It executes Forthic code from any client that can reach it. \
             Set ServeOptions.token / FORTHIC_JSONRPC_TOKEN, or bind to 127.0.0.1."
        );
    }

    let servicer = ForthicJsonRpcServicer::new();
    let module_names = servicer.registered_module_names();
    if module_names.is_empty() {
        println!("  - No runtime-specific modules loaded");
    } else {
        println!("  - Available runtime modules: {}", module_names.join(", "));
    }

    let ctx = Arc::new(ServerContext {
        servicer,
        token,
        max_body_bytes,
        expose_error_details: options.expose_error_details,
    });

    // Undefined paths 404 via the default fallback; non-POST on defined
    // paths gets 405 + Allow from axum's method router.
    let app = Router::new()
        .route("/rpc", post(rpc_handler))
        .route("/", post(rpc_handler))
        .with_state(ctx);

    let listener = TcpListener::bind((host.as_str(), port)).await?;
    let addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let join = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    Ok(ServerHandle {
        addr,
        shutdown_tx,
        join,
    })
}

async fn rpc_handler(
    State(ctx): State<Arc<ServerContext>>,
    req: Request<Body>,
) -> Response {
    // Authenticate before reading the body, so unauthorized callers can't
    // execute code or push a large payload.
    if !is_authorized(req.headers(), ctx.token.as_deref()) {
        return json_response(
            StatusCode::UNAUTHORIZED,
            Some(("WWW-Authenticate", "Bearer")),
            error_envelope(
                Value::Null,
                JsonRpcErrorCode::INVALID_REQUEST,
                "Unauthorized",
            ),
        );
    }

    let ctype = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !ctype.contains("application/json") {
        return plain_response(StatusCode::UNSUPPORTED_MEDIA_TYPE, "Unsupported Media Type");
    }

    // Reject an oversized body up front via Content-Length, before reading
    let declared_len = req
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok());
    if matches!(declared_len, Some(len) if len > ctx.max_body_bytes) {
        return payload_too_large();
    }

    let raw = match read_body_capped(req.into_body(), ctx.max_body_bytes).await {
        Ok(bytes) => bytes,
        Err(BodyReadError::TooLarge) => return payload_too_large(),
        Err(BodyReadError::Io) => {
            return json_response(
                StatusCode::OK,
                None,
                error_envelope(
                    Value::Null,
                    JsonRpcErrorCode::PARSE_ERROR,
                    "Failed to read request body",
                ),
            )
        }
    };

    let parsed: Value = match serde_json::from_slice(&raw) {
        Ok(v) => v,
        Err(e) => {
            return json_response(
                StatusCode::OK,
                None,
                error_envelope(
                    Value::Null,
                    JsonRpcErrorCode::PARSE_ERROR,
                    &format!("Parse error: {e}"),
                ),
            )
        }
    };

    if parsed.is_array() {
        return json_response(
            StatusCode::OK,
            None,
            error_envelope(
                Value::Null,
                JsonRpcErrorCode::INVALID_REQUEST,
                "Batch requests are not supported",
            ),
        );
    }

    let valid_envelope = parsed
        .as_object()
        .map(|obj| {
            obj.get("jsonrpc").and_then(Value::as_str) == Some("2.0")
                && obj.get("method").is_some_and(Value::is_string)
                && obj.contains_key("id")
        })
        .unwrap_or(false);
    if !valid_envelope {
        let id = parsed.get("id").cloned().unwrap_or(Value::Null);
        return json_response(
            StatusCode::OK,
            None,
            error_envelope(
                id,
                JsonRpcErrorCode::INVALID_REQUEST,
                "Invalid JSON-RPC 2.0 request",
            ),
        );
    }

    let request: JsonRpcRequest = match serde_json::from_value(parsed) {
        Ok(r) => r,
        Err(_) => {
            return json_response(
                StatusCode::OK,
                None,
                error_envelope(
                    Value::Null,
                    JsonRpcErrorCode::INVALID_REQUEST,
                    "Invalid JSON-RPC 2.0 request",
                ),
            )
        }
    };

    // The interpreter is synchronous by design; run it off the async
    // workers. Everything the closure needs moves in with it.
    let ctx2 = Arc::clone(&ctx);
    let response = tokio::task::spawn_blocking(move || {
        dispatch(&ctx2.servicer, &request, ctx2.expose_error_details)
    })
    .await
    .unwrap_or_else(|_| {
        error_envelope(
            Value::Null,
            JsonRpcErrorCode::INTERNAL_ERROR,
            "Internal error",
        )
    });

    json_response(StatusCode::OK, None, response)
}

enum BodyReadError {
    TooLarge,
    Io,
}

/// Read the request body, enforcing the cap while streaming (covers the
/// chunked / no-Content-Length case the up-front check can't)
async fn read_body_capped(body: Body, max: usize) -> Result<Vec<u8>, BodyReadError> {
    let mut body = body;
    let mut out: Vec<u8> = Vec::new();
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| BodyReadError::Io)?;
        if let Some(data) = frame.data_ref() {
            if out.len() + data.len() > max {
                return Err(BodyReadError::TooLarge);
            }
            out.extend_from_slice(data);
        }
    }
    Ok(out)
}

fn is_loopback_host(host: &str) -> bool {
    host == "127.0.0.1" || host == "::1" || host == "localhost"
}

/// True when no token is configured, or the request carries the matching
/// Bearer token (compared in constant time to avoid a timing oracle)
fn is_authorized(headers: &HeaderMap, token: Option<&str>) -> bool {
    let Some(token) = token else { return true };
    let Some(header_val) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let Some(presented) = header_val.strip_prefix("Bearer ") else {
        return false;
    };
    // subtle's slice ct_eq returns false for length mismatch without
    // revealing where the strings differ
    presented.as_bytes().ct_eq(token.as_bytes()).into()
}

fn error_envelope(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn payload_too_large() -> Response {
    json_response(
        StatusCode::PAYLOAD_TOO_LARGE,
        None,
        error_envelope(
            Value::Null,
            JsonRpcErrorCode::INVALID_REQUEST,
            "Payload too large",
        ),
    )
}

fn json_response(
    status: StatusCode,
    extra_header: Option<(&'static str, &'static str)>,
    payload: Value,
) -> Response {
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some((name, value)) = extra_header {
        builder = builder.header(name, value);
    }
    builder
        .body(Body::from(payload.to_string()))
        .expect("static response parts are valid")
}

fn plain_response(status: StatusCode, text: &'static str) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from(text))
        .expect("static response parts are valid")
}
