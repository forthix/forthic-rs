//! JSON-RPC servicer and dispatch (transport-independent)
//!
//! The four methods of forthic-ts `src/jsonrpc/server.ts` — `executeWord`,
//! `executeSequence`, `listModules`, `getModuleInfo` — operating on parsed
//! JSON-RPC 2.0 envelopes. Same param/result keys, same error codes, same
//! validation messages, so a forthic-ts `JsonRpcClient` works unchanged.
//!
//! This layer knows nothing about HTTP; Phase 3 wraps [`dispatch`] in the
//! axum transport (envelope validation, auth, body limits happen there).

use super::errors::{ErrorInfo, JsonRpcErrorCode, MethodError};
use super::serializer::{deserialize_value, serialize_value};
use crate::errors::ForthicError;
use crate::interpreter::Interpreter;
use crate::literals::ForthicValue;
use crate::module::Module;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// A parsed JSON-RPC 2.0 request envelope
///
/// `id` defaults to `Value::Null` and `params` to `Value::Null` when absent;
/// the transport layer is responsible for rejecting envelopes that are
/// structurally invalid (wrong `jsonrpc`, missing `id`, batch arrays).
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Implements the four Forthic RPC methods
///
/// Each execute call runs on a fresh standard interpreter (full stdlib plus
/// any registered runtime modules), so requests are isolated from each other.
pub struct ForthicJsonRpcServicer {
    /// Runtime-specific modules exposed over RPC (ts registers `fs` here;
    /// rs has none yet — the registry is ready for them)
    runtime_modules: Vec<Module>,
    timezone: String,
}

impl ForthicJsonRpcServicer {
    pub fn new() -> Self {
        Self {
            runtime_modules: Vec::new(),
            timezone: "UTC".to_string(),
        }
    }

    /// Names of the registered runtime-specific modules
    pub fn registered_module_names(&self) -> Vec<String> {
        self.runtime_modules
            .iter()
            .map(|m| m.get_name().to_string())
            .collect()
    }

    fn make_interpreter(&self) -> Interpreter {
        let mut interp = Interpreter::standard(&self.timezone);
        for module in &self.runtime_modules {
            interp.import_module(module.clone(), "");
        }
        interp
    }

    /// `executeWord`: push the supplied stack, run one word, return the stack
    pub fn execute_word(
        &self,
        params: &Value,
        expose_error_details: bool,
    ) -> Result<Value, MethodError> {
        let word_name = params
            .get("word_name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                MethodError::invalid_params("executeWord requires string \"word_name\"")
            })?;
        let stack_json = params
            .get("stack")
            .and_then(Value::as_array)
            .ok_or_else(|| MethodError::invalid_params("executeWord requires array \"stack\""))?;

        let context = HashMap::from([("word_name".to_string(), word_name.to_string())]);
        let stack = self.deserialize_stack(stack_json, &context)?;

        let mut interp = self.make_interpreter();
        for item in stack {
            interp.stack_push(item);
        }
        interp.run(word_name).map_err(|e| {
            self.runtime_error_from_forthic(&e, context.clone(), expose_error_details)
        })?;

        self.serialize_result_stack(&interp, &context)
    }

    /// `executeSequence`: like `executeWord` but runs several words in order
    /// on the same stack. One JSON-RPC call — not a JSON-RPC batch.
    pub fn execute_sequence(
        &self,
        params: &Value,
        expose_error_details: bool,
    ) -> Result<Value, MethodError> {
        let word_names: Vec<&str> = params
            .get("word_names")
            .and_then(Value::as_array)
            .and_then(|arr| arr.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
            .ok_or_else(|| {
                MethodError::invalid_params("executeSequence requires string[] \"word_names\"")
            })?;
        let stack_json = params
            .get("stack")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                MethodError::invalid_params("executeSequence requires array \"stack\"")
            })?;

        let context = HashMap::from([("word_sequence".to_string(), word_names.join(", "))]);
        let stack = self.deserialize_stack(stack_json, &context)?;

        let mut interp = self.make_interpreter();
        for item in stack {
            interp.stack_push(item);
        }
        for word_name in &word_names {
            interp.run(word_name).map_err(|e| {
                self.runtime_error_from_forthic(&e, context.clone(), expose_error_details)
            })?;
        }

        self.serialize_result_stack(&interp, &context)
    }

    /// `listModules`: summaries of the runtime-specific modules
    pub fn list_modules(&self) -> Result<Value, MethodError> {
        let modules: Vec<Value> = self
            .runtime_modules
            .iter()
            .map(|m| {
                json!({
                    "name": m.get_name(),
                    "description": format!("Rust-specific {} module", m.get_name()),
                    "word_count": m.exportable_words().len(),
                    "runtime_specific": true,
                })
            })
            .collect();
        Ok(json!({ "modules": modules }))
    }

    /// `getModuleInfo`: word listing for one runtime-specific module
    pub fn get_module_info(&self, params: &Value) -> Result<Value, MethodError> {
        let module_name = params
            .get("module_name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                MethodError::invalid_params("getModuleInfo requires string \"module_name\"")
            })?;
        let module = self
            .runtime_modules
            .iter()
            .find(|m| m.get_name() == module_name)
            .ok_or_else(|| {
                MethodError::new(
                    JsonRpcErrorCode::MODULE_NOT_FOUND,
                    format!("Module '{module_name}' not found"),
                )
            })?;
        let words: Vec<Value> = module
            .exportable_words()
            .iter()
            .map(|w| {
                json!({
                    "name": w.name(),
                    "stack_effect": "( -- )",
                    "description": format!("{} word from {} module", w.name(), module_name),
                })
            })
            .collect();
        Ok(json!({
            "name": module_name,
            "description": format!("Rust-specific {module_name} module"),
            "words": words,
        }))
    }

    fn deserialize_stack(
        &self,
        stack_json: &[Value],
        context: &HashMap<String, String>,
    ) -> Result<Vec<ForthicValue>, MethodError> {
        stack_json
            .iter()
            .map(deserialize_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                self.runtime_error(
                    e.to_string(),
                    "SerializerError",
                    context.clone(),
                    None,
                    None,
                )
            })
    }

    fn serialize_result_stack(
        &self,
        interp: &Interpreter,
        context: &HashMap<String, String>,
    ) -> Result<Value, MethodError> {
        let result_stack = interp
            .get_stack()
            .items()
            .iter()
            .map(serialize_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                self.runtime_error(
                    e.to_string(),
                    "SerializerError",
                    context.clone(),
                    None,
                    None,
                )
            })?;
        Ok(json!({ "result_stack": result_stack }))
    }

    /// Build a RUNTIME_ERROR (-32000) whose `data` is a full ErrorInfo
    fn runtime_error(
        &self,
        message: String,
        error_type: &str,
        context: HashMap<String, String>,
        module_name: Option<String>,
        word_location: Option<String>,
    ) -> MethodError {
        let info = ErrorInfo {
            message: message.clone(),
            runtime: "rust".to_string(),
            error_type: error_type.to_string(),
            context,
            stack_trace: None,
            word_location,
            module_name,
        };
        MethodError {
            code: JsonRpcErrorCode::RUNTIME_ERROR,
            message,
            data: Some(serde_json::to_value(info).expect("ErrorInfo serializes")),
        }
    }

    fn runtime_error_from_forthic(
        &self,
        e: &ForthicError,
        context: HashMap<String, String>,
        expose_error_details: bool,
    ) -> MethodError {
        let module_name = match e {
            ForthicError::Module { module_name, .. } => Some(module_name.clone()),
            _ => None,
        };
        // Code locations point into caller-supplied Forthic, but their
        // formatting reveals server internals; expose only on request
        // (rs analog of ts exposeStackTraces).
        let word_location = if expose_error_details {
            e.get_location().map(|loc| {
                format!(
                    "{}:{}:{}",
                    loc.source.as_deref().unwrap_or("<request>"),
                    loc.line,
                    loc.column
                )
            })
        } else {
            None
        };
        self.runtime_error(
            e.to_string(),
            forthic_error_type(e),
            context,
            module_name,
            word_location,
        )
    }
}

impl Default for ForthicJsonRpcServicer {
    fn default() -> Self {
        Self::new()
    }
}

/// The `error_type` wire string for each ForthicError variant
fn forthic_error_type(e: &ForthicError) -> &'static str {
    e.type_name()
}

/// Dispatch one parsed JSON-RPC request to the servicer, producing the full
/// response envelope (success or error) as JSON
pub fn dispatch(
    servicer: &ForthicJsonRpcServicer,
    request: &JsonRpcRequest,
    expose_error_details: bool,
) -> Value {
    let outcome = match request.method.as_str() {
        "executeWord" => servicer.execute_word(&request.params, expose_error_details),
        "executeSequence" => servicer.execute_sequence(&request.params, expose_error_details),
        "listModules" => servicer.list_modules(),
        "getModuleInfo" => servicer.get_module_info(&request.params),
        method => Err(MethodError::new(
            JsonRpcErrorCode::METHOD_NOT_FOUND,
            format!("Method not found: {method}"),
        )),
    };
    match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": request.id, "result": result }),
        Err(e) => {
            let mut error = json!({ "code": e.code, "message": e.message });
            if let Some(data) = e.data {
                error["data"] = data;
            }
            json!({ "jsonrpc": "2.0", "id": request.id, "error": error })
        }
    }
}
