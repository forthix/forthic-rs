//! JSON-RPC error codes and the structured ErrorInfo payload
//!
//! Codes and payload shape match forthic-ts `src/jsonrpc/errors.ts` so a
//! client can handle errors from either runtime identically.

use serde::Serialize;
use std::collections::HashMap;

/// JSON-RPC standard error codes plus Forthic's server-defined codes
pub struct JsonRpcErrorCode;

impl JsonRpcErrorCode {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
    /// Forthic runtime error — the error `data` carries a full [`ErrorInfo`]
    pub const RUNTIME_ERROR: i64 = -32000;
    /// Unknown module name passed to getModuleInfo
    pub const MODULE_NOT_FOUND: i64 = -32001;
}

/// Structured payload of RUNTIME_ERROR responses
///
/// Field names are the wire contract (ts `RemoteErrorInfo`). `stack_trace`
/// and `word_location` are omitted unless the server opts into exposing
/// error details — they describe server internals.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ErrorInfo {
    pub message: String,
    /// Always "rust" for this server
    pub runtime: String,
    pub error_type: String,
    pub context: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_name: Option<String>,
}

/// Error from a servicer method; maps 1:1 onto the JSON-RPC error object
/// `{ code, message, data? }`
#[derive(Debug, Clone)]
pub struct MethodError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl MethodError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(JsonRpcErrorCode::INVALID_PARAMS, message)
    }
}
