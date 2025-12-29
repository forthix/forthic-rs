// JSON module for Forthic
//
// JSON serialization, parsing, and formatting operations.
//
// ## Categories
// - Conversion: >JSON, JSON>
// - Formatting: JSON-PRETTIFY

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{InterpreterContext, Module, ModuleWord};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::sync::Arc;

/// JSONModule provides JSON serialization operations
pub struct JSONModule {
    module: Module,
}

impl JSONModule {
    /// Create a new JSONModule
    pub fn new() -> Self {
        let mut module = Module::new("json".to_string());

        // Register all words
        Self::register_conversion_words(&mut module);
        Self::register_formatting_words(&mut module);

        Self { module }
    }

    /// Get the underlying module
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Get a mutable reference to the underlying module
    pub fn module_mut(&mut self) -> &mut Module {
        &mut self.module
    }

    // ===== Conversion Operations =====

    fn register_conversion_words(module: &mut Module) {
        // >JSON
        let word = Arc::new(ModuleWord::new(">JSON".to_string(), Self::word_to_json));
        module.add_exportable_word(word);

        // JSON>
        let word = Arc::new(ModuleWord::new("JSON>".to_string(), Self::word_from_json));
        module.add_exportable_word(word);
    }

    fn word_to_json(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let json_val = Self::forthic_to_json(&val);
        let json_str = serde_json::to_string(&json_val).unwrap_or_else(|_| "null".to_string());

        context.stack_push(ForthicValue::String(json_str));
        Ok(())
    }

    fn word_from_json(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => {
                if s.trim().is_empty() {
                    ForthicValue::Null
                } else {
                    match serde_json::from_str::<JsonValue>(&s) {
                        Ok(json_val) => Self::json_to_forthic(&json_val),
                        Err(_) => ForthicValue::Null,
                    }
                }
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Formatting Operations =====

    fn register_formatting_words(module: &mut Module) {
        // JSON-PRETTIFY
        let word = Arc::new(ModuleWord::new(
            "JSON-PRETTIFY".to_string(),
            Self::word_json_prettify,
        ));
        module.add_exportable_word(word);
    }

    fn word_json_prettify(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => {
                if s.trim().is_empty() {
                    ForthicValue::String(String::new())
                } else {
                    match serde_json::from_str::<JsonValue>(&s) {
                        Ok(json_val) => {
                            let pretty = serde_json::to_string_pretty(&json_val)
                                .unwrap_or_else(|_| String::new());
                            ForthicValue::String(pretty)
                        }
                        Err(_) => ForthicValue::String(String::new()),
                    }
                }
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Helper Functions =====

    /// Convert ForthicValue to serde_json::Value
    fn forthic_to_json(val: &ForthicValue) -> JsonValue {
        match val {
            ForthicValue::Null => JsonValue::Null,
            ForthicValue::Bool(b) => JsonValue::Bool(*b),
            ForthicValue::Int(i) => json!(i),
            ForthicValue::Float(f) => json!(f),
            ForthicValue::String(s) => JsonValue::String(s.clone()),
            ForthicValue::Array(arr) => {
                let json_arr: Vec<JsonValue> = arr.iter().map(Self::forthic_to_json).collect();
                JsonValue::Array(json_arr)
            }
            ForthicValue::Record(rec) => {
                let json_obj: serde_json::Map<String, JsonValue> = rec
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::forthic_to_json(v)))
                    .collect();
                JsonValue::Object(json_obj)
            }
            ForthicValue::Date(d) => JsonValue::String(d.format("%Y-%m-%d").to_string()),
            ForthicValue::Time(t) => JsonValue::String(t.format("%H:%M:%S").to_string()),
            ForthicValue::DateTime(dt) => JsonValue::String(dt.to_rfc3339()),
            _ => JsonValue::Null,
        }
    }

    /// Convert serde_json::Value to ForthicValue
    fn json_to_forthic(val: &JsonValue) -> ForthicValue {
        match val {
            JsonValue::Null => ForthicValue::Null,
            JsonValue::Bool(b) => ForthicValue::Bool(*b),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ForthicValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    ForthicValue::Float(f)
                } else {
                    ForthicValue::Null
                }
            }
            JsonValue::String(s) => ForthicValue::String(s.clone()),
            JsonValue::Array(arr) => {
                let forthic_arr: Vec<ForthicValue> = arr.iter().map(Self::json_to_forthic).collect();
                ForthicValue::Array(forthic_arr)
            }
            JsonValue::Object(obj) => {
                let forthic_rec: HashMap<String, ForthicValue> = obj
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::json_to_forthic(v)))
                    .collect();
                ForthicValue::Record(forthic_rec)
            }
        }
    }
}

impl Default for JSONModule {
    fn default() -> Self {
        Self::new()
    }
}
