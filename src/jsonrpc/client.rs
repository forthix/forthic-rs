//! Blocking JSON-RPC 2.0 client for remote Forthic runtimes
//!
//! The client-side counterpart of [`super::server`]: speaks the same wire
//! format as the forthic-ts and forthic-py JSON-RPC servers (executeWord /
//! executeSequence / listModules / getModuleInfo), so any of the three
//! runtimes can drive any other.
//!
//! Deliberately dependency-free: a hand-rolled HTTP/1.1 POST over
//! `std::net::TcpStream` with `Connection: close` (Content-Length and
//! chunked responses both handled). Plain HTTP only — like the server,
//! this is loopback/deployment-network oriented; there is no TLS.
//! Blocking by design: the rs interpreter is synchronous, so a word that
//! calls a remote runtime blocks its calling thread, matching the rest of
//! the runtime's execution model.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use serde_json::{json, Value};

use super::errors::JsonRpcErrorCode;
use super::serializer::{deserialize_value, serialize_value, SerializerError};
use crate::literals::ForthicValue;

/// Error information parsed from a RUNTIME_ERROR response's `data` payload.
/// Field names are the wire contract (ts `RemoteErrorInfo`); every field is
/// parsed leniently so a sparse payload still surfaces as a usable error.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteErrorInfo {
    pub message: String,
    pub runtime: String,
    pub error_type: String,
    pub context: HashMap<String, String>,
    pub stack_trace: Vec<String>,
    pub word_location: Option<String>,
    pub module_name: Option<String>,
}

impl RemoteErrorInfo {
    fn from_value(data: &Value) -> Self {
        let str_field = |key: &str| data.get(key).and_then(Value::as_str).map(str::to_string);
        Self {
            message: str_field("message").unwrap_or_else(|| "Unknown error".to_string()),
            runtime: str_field("runtime").unwrap_or_else(|| "unknown".to_string()),
            error_type: str_field("error_type").unwrap_or_else(|| "Error".to_string()),
            context: data
                .get("context")
                .and_then(Value::as_object)
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
            stack_trace: data
                .get("stack_trace")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            word_location: str_field("word_location"),
            module_name: str_field("module_name"),
        }
    }
}

/// Client-side failures, from transport problems up to structured errors
/// raised inside the remote runtime
#[derive(Debug)]
pub enum ClientError {
    /// Socket-level failure (connect, read, write, timeout)
    Io(std::io::Error),
    /// Non-200 HTTP response outside the JSON-RPC envelope (401, 404, ...)
    Http { status: u16, body: String },
    /// Malformed HTTP or JSON-RPC framing
    Protocol(String),
    /// StackValue (de)serialization failure
    Serializer(SerializerError),
    /// JSON-RPC error other than RUNTIME_ERROR (invalid params, unknown
    /// method, module not found, ...)
    Rpc {
        code: i64,
        message: String,
        data: Option<Value>,
    },
    /// A Forthic error inside the remote runtime, with its metadata
    Remote(RemoteErrorInfo),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Io(e) => write!(f, "JSON-RPC transport error: {e}"),
            ClientError::Http { status, body } => {
                write!(f, "JSON-RPC HTTP error {status}: {body}")
            }
            ClientError::Protocol(msg) => write!(f, "JSON-RPC protocol error: {msg}"),
            ClientError::Serializer(e) => write!(f, "JSON-RPC serializer error: {e:?}"),
            ClientError::Rpc { code, message, .. } => {
                write!(f, "JSON-RPC error {code}: {message}")
            }
            ClientError::Remote(info) => write!(
                f,
                "Error in {} runtime: {} ({})",
                info.runtime, info.message, info.error_type
            ),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<std::io::Error> for ClientError {
    fn from(e: std::io::Error) -> Self {
        ClientError::Io(e)
    }
}

impl From<SerializerError> for ClientError {
    fn from(e: SerializerError) -> Self {
        ClientError::Serializer(e)
    }
}

/// Blocking JSON-RPC 2.0 client for a remote Forthic runtime
#[derive(Debug)]
pub struct JsonRpcClient {
    host: String,
    port: u16,
    path: String,
    token: Option<String>,
    timeout: Duration,
    next_id: AtomicI64,
}

impl JsonRpcClient {
    /// `address` is `host:port` (e.g. `"127.0.0.1:8765"`). The RPC path
    /// defaults to `/rpc`.
    pub fn new(address: &str) -> Result<Self, ClientError> {
        let (host, port) = address
            .rsplit_once(':')
            .and_then(|(h, p)| p.parse::<u16>().ok().map(|p| (h.to_string(), p)))
            .ok_or_else(|| {
                ClientError::Protocol(format!("address must be host:port, got {address:?}"))
            })?;
        Ok(Self {
            host,
            port,
            path: "/rpc".to_string(),
            token: None,
            timeout: Duration::from_secs(30),
            next_id: AtomicI64::new(1),
        })
    }

    /// Bearer token for servers started with `--token`
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Execute a single word against the remote runtime's stack
    pub fn execute_word(
        &self,
        word_name: &str,
        stack: &[ForthicValue],
    ) -> Result<Vec<ForthicValue>, ClientError> {
        let params = json!({
            "word_name": word_name,
            "stack": self.serialize_stack(stack)?,
        });
        let result = self.call("executeWord", params)?;
        self.parse_result_stack(&result)
    }

    /// Execute a sequence of words as ONE call (stack continuity between
    /// words — this is not a JSON-RPC batch)
    pub fn execute_sequence(
        &self,
        word_names: &[&str],
        stack: &[ForthicValue],
    ) -> Result<Vec<ForthicValue>, ClientError> {
        let params = json!({
            "word_names": word_names,
            "stack": self.serialize_stack(stack)?,
        });
        let result = self.call("executeSequence", params)?;
        self.parse_result_stack(&result)
    }

    /// List the remote runtime's runtime-specific modules
    pub fn list_modules(&self) -> Result<Vec<Value>, ClientError> {
        let result = self.call("listModules", json!({}))?;
        Ok(result
            .get("modules")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    /// Word documentation for one remote module
    pub fn get_module_info(&self, module_name: &str) -> Result<Value, ClientError> {
        self.call("getModuleInfo", json!({ "module_name": module_name }))
    }

    // ---- internals ----

    fn serialize_stack(&self, stack: &[ForthicValue]) -> Result<Vec<Value>, ClientError> {
        stack
            .iter()
            .map(|v| serialize_value(v).map_err(ClientError::from))
            .collect()
    }

    fn parse_result_stack(&self, result: &Value) -> Result<Vec<ForthicValue>, ClientError> {
        result
            .get("result_stack")
            .and_then(Value::as_array)
            .ok_or_else(|| ClientError::Protocol("response missing result_stack".to_string()))?
            .iter()
            .map(|v| deserialize_value(v).map_err(ClientError::from))
            .collect()
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, ClientError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let envelope = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let (status, body) = self.http_post(&envelope.to_string())?;

        // The server answers JSON-RPC errors with HTTP 200; anything else
        // is outside the protocol (401 unauthorized, 404, ...)
        if status != 200 {
            return Err(ClientError::Http { status, body });
        }

        let response: Value = serde_json::from_str(&body)
            .map_err(|e| ClientError::Protocol(format!("invalid JSON-RPC response: {e}")))?;

        if let Some(error) = response.get("error") {
            let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
                .to_string();
            if code == JsonRpcErrorCode::RUNTIME_ERROR {
                if let Some(data) = error.get("data") {
                    return Err(ClientError::Remote(RemoteErrorInfo::from_value(data)));
                }
            }
            return Err(ClientError::Rpc {
                code,
                message,
                data: error.get("data").cloned(),
            });
        }

        response.get("result").cloned().ok_or_else(|| {
            ClientError::Protocol("response has neither result nor error".to_string())
        })
    }

    fn http_post(&self, body: &str) -> Result<(u16, String), ClientError> {
        let addr = format!("{}:{}", self.host, self.port);
        let mut stream = TcpStream::connect(&addr)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;

        let auth_header = match &self.token {
            Some(token) => format!("Authorization: Bearer {token}\r\n"),
            None => String::new(),
        };
        let request = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",
            self.path,
            addr,
            body.len(),
            auth_header,
            body,
        );
        stream.write_all(request.as_bytes())?;

        // Connection: close — read the full response, then frame it
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw)?;
        Self::parse_http_response(&raw)
    }

    fn parse_http_response(raw: &[u8]) -> Result<(u16, String), ClientError> {
        let header_end = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(|| ClientError::Protocol("no HTTP header terminator".to_string()))?;
        let head = String::from_utf8_lossy(&raw[..header_end]);
        let payload = &raw[header_end + 4..];

        let mut lines = head.split("\r\n");
        let status_line = lines
            .next()
            .ok_or_else(|| ClientError::Protocol("empty HTTP response".to_string()))?;
        let status: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| ClientError::Protocol(format!("bad status line: {status_line}")))?;

        let mut content_length: Option<usize> = None;
        let mut chunked = false;
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            if name == "content-length" {
                content_length = value.parse().ok();
            } else if name == "transfer-encoding" && value.eq_ignore_ascii_case("chunked") {
                chunked = true;
            }
        }

        let body_bytes = if chunked {
            Self::decode_chunked(payload)?
        } else if let Some(len) = content_length {
            payload.get(..len).unwrap_or(payload).to_vec()
        } else {
            payload.to_vec()
        };
        Ok((status, String::from_utf8_lossy(&body_bytes).into_owned()))
    }

    fn decode_chunked(mut payload: &[u8]) -> Result<Vec<u8>, ClientError> {
        let mut body = Vec::new();
        loop {
            let line_end = payload
                .windows(2)
                .position(|w| w == b"\r\n")
                .ok_or_else(|| ClientError::Protocol("bad chunked framing".to_string()))?;
            let size_str = String::from_utf8_lossy(&payload[..line_end]);
            let size = usize::from_str_radix(size_str.trim().split(';').next().unwrap_or(""), 16)
                .map_err(|_| ClientError::Protocol(format!("bad chunk size: {size_str}")))?;
            payload = &payload[line_end + 2..];
            if size == 0 {
                return Ok(body);
            }
            let chunk = payload
                .get(..size)
                .ok_or_else(|| ClientError::Protocol("truncated chunk".to_string()))?;
            body.extend_from_slice(chunk);
            payload = payload.get(size + 2..).unwrap_or(&[]);
        }
    }
}
