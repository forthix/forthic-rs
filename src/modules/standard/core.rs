// Core module for Forthic
//
// Essential interpreter operations for stack manipulation, variables, and control flow.
//
// ## Categories
// - Stack: POP, DUP, SWAP
// - Variables: VARIABLES, !, @, !@
// - Control: IDENTITY, NOP, NULL, ARRAY?, DEFAULT
// - Errors: TRY, OK?, ERROR?, UNWRAP, UNWRAP-OR (Rust Result semantics:
//   'CODE' TRY UNWRAP is CODE — mirrored with forthic-ts)
// - Options: ~> (converts array to WordOptions)

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{InterpreterContext, Module, ModuleWord};
use crate::word_options::WordOptions;
use indexmap::IndexMap;
use std::sync::Arc;

/// Build an `{"ok": payload}` outcome record (TRY / MAP outcomes)
pub(crate) fn ok_outcome(payload: ForthicValue) -> ForthicValue {
    let mut outcome = IndexMap::new();
    outcome.insert("ok".to_string(), payload);
    ForthicValue::Record(outcome)
}

/// Build an `{"error": {message, error_type}}` outcome record. The error
/// info uses the same field names as the JSON-RPC wire ErrorInfo — one
/// error representation everywhere.
pub(crate) fn error_outcome(e: &ForthicError) -> ForthicValue {
    let mut info = IndexMap::new();
    info.insert("message".to_string(), ForthicValue::String(e.to_string()));
    info.insert(
        "error_type".to_string(),
        ForthicValue::String(e.type_name().to_string()),
    );
    let mut outcome = IndexMap::new();
    outcome.insert("error".to_string(), ForthicValue::Record(info));
    ForthicValue::Record(outcome)
}

/// CoreModule provides core interpreter operations
pub struct CoreModule {
    module: Module,
}

impl CoreModule {
    /// Create a new CoreModule
    pub fn new() -> Self {
        let mut module = Module::new("core".to_string());

        // Register all words
        Self::register_stack_words(&mut module);
        Self::register_variable_words(&mut module);
        Self::register_control_words(&mut module);
        Self::register_error_words(&mut module);
        Self::register_options_words(&mut module);

        Self { module }
    }

    // ===== Error Handling (Rust Result semantics; see backlog item 20) =====

    fn register_error_words(module: &mut Module) {
        let word = Arc::new(ModuleWord::new("TRY".to_string(), Self::word_try));
        module.add_exportable_word(word);

        let word = Arc::new(ModuleWord::new("OK?".to_string(), Self::word_ok_q));
        module.add_exportable_word(word);

        let word = Arc::new(ModuleWord::new("ERROR?".to_string(), Self::word_error_q));
        module.add_exportable_word(word);

        let word = Arc::new(ModuleWord::new("UNWRAP".to_string(), Self::word_unwrap));
        module.add_exportable_word(word);

        let word = Arc::new(ModuleWord::new(
            "UNWRAP-OR".to_string(),
            Self::word_unwrap_or,
        ));
        module.add_exportable_word(word);
    }

    /// TRY: ( forthic -- outcome )
    ///
    /// Runs the code, capturing the outcome as data: `{"ok": value}` on
    /// success (value = top of stack if the run changed the stack; NULL for
    /// no-net-effect code), `{"error": {message, error_type}}` on failure.
    /// Transactional for the stack: on failure it is restored to its
    /// pre-TRY state and modules left open by the failed code are unwound.
    /// Side effects (variable writes) persist — catch_unwind semantics.
    /// Law: `'CODE' TRY UNWRAP` ≡ `CODE`.
    ///
    /// For error-tolerant mapping use MAP's outcomes option
    /// (`[.outcomes TRUE] ~> MAP`): TRY inside MAP would transactionally
    /// restore the item MAP pushed, stranding it beneath the outcome.
    fn word_try(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = match context.stack_pop()? {
            ForthicValue::String(s) => s,
            other => {
                return Err(ForthicError::InvalidOperation {
                    forthic: String::new(),
                    message: format!("TRY requires a Forthic string, got {other:?}"),
                    location: None,
                    cause: None,
                })
            }
        };

        let snapshot = context.stack_snapshot();
        let module_depth = context.module_stack_depth();

        match context.run(&forthic) {
            Ok(()) => {
                // Payload = top of stack IF the run changed the stack at all
                // (covers transforms like '2 *' that consume and push,
                // leaving the length unchanged). Note: rs compares
                // structurally (PartialEq) where ts compares identity —
                // code that replaces a value with an equal one reads as
                // "unchanged" here; behaviorally indistinguishable.
                let after = context.stack_snapshot();
                let unchanged = after == snapshot;
                let payload = if !unchanged && !after.is_empty() {
                    context.stack_pop()?
                } else {
                    ForthicValue::Null
                };
                context.stack_push(ok_outcome(payload));
            }
            Err(e) => {
                context.stack_restore(snapshot);
                while context.module_stack_depth() > module_depth {
                    let _ = context.module_stack_pop();
                }
                context.stack_push(error_outcome(&e));
            }
        }
        Ok(())
    }

    fn is_outcome_with(value: &ForthicValue, key: &str) -> bool {
        matches!(value, ForthicValue::Record(rec) if rec.contains_key(key))
    }

    fn word_ok_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let outcome = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(Self::is_outcome_with(&outcome, "ok")));
        Ok(())
    }

    fn word_error_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let outcome = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(Self::is_outcome_with(&outcome, "error")));
        Ok(())
    }

    /// UNWRAP: ( outcome -- value ) — ok: push the value; error: re-raise
    /// preserving message and error_type (like Rust, the concrete variant
    /// becomes a generic wrapper — Err(e).unwrap() panics with Debug of e)
    fn word_unwrap(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let outcome = context.stack_pop()?;
        if let ForthicValue::Record(rec) = &outcome {
            if let Some(value) = rec.get("ok") {
                context.stack_push(value.clone());
                return Ok(());
            }
            if let Some(info) = rec.get("error") {
                let (message, error_type) = match info {
                    ForthicValue::Record(info) => (
                        info.get("message")
                            .and_then(|m| m.as_string())
                            .unwrap_or("UNWRAP of error outcome")
                            .to_string(),
                        info.get("error_type")
                            .and_then(|t| t.as_string())
                            .map(str::to_string),
                    ),
                    _ => ("UNWRAP of error outcome".to_string(), None),
                };
                let suffix = error_type.map(|t| format!(" ({t})")).unwrap_or_default();
                return Err(ForthicError::InvalidOperation {
                    forthic: String::new(),
                    message: format!("{message}{suffix}"),
                    location: None,
                    cause: None,
                });
            }
        }
        Err(ForthicError::InvalidOperation {
            forthic: String::new(),
            message: "UNWRAP requires a TRY outcome record with an 'ok' or 'error' key".to_string(),
            location: None,
            cause: None,
        })
    }

    /// UNWRAP-OR: ( outcome default -- value ) — ok wins even when the ok
    /// value is NULL (failure is not nullness)
    fn word_unwrap_or(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let default_value = context.stack_pop()?;
        let outcome = context.stack_pop()?;
        if let ForthicValue::Record(rec) = &outcome {
            if let Some(value) = rec.get("ok") {
                context.stack_push(value.clone());
                return Ok(());
            }
            if rec.contains_key("error") {
                context.stack_push(default_value);
                return Ok(());
            }
        }
        Err(ForthicError::InvalidOperation {
            forthic: String::new(),
            message: "UNWRAP-OR requires a TRY outcome record with an 'ok' or 'error' key"
                .to_string(),
            location: None,
            cause: None,
        })
    }

    /// Get the underlying module
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Get a mutable reference to the underlying module
    pub fn module_mut(&mut self) -> &mut Module {
        &mut self.module
    }

    // ===== Stack Operations =====

    fn register_stack_words(module: &mut Module) {
        // POP
        let word = Arc::new(ModuleWord::new("POP".to_string(), Self::word_pop));
        module.add_exportable_word(word);

        // DUP
        let word = Arc::new(ModuleWord::new("DUP".to_string(), Self::word_dup));
        module.add_exportable_word(word);

        // SWAP
        let word = Arc::new(ModuleWord::new("SWAP".to_string(), Self::word_swap));
        module.add_exportable_word(word);
    }

    fn word_pop(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.stack_pop()?;
        Ok(())
    }

    fn word_dup(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        context.stack_push(val.clone());
        context.stack_push(val);
        Ok(())
    }

    fn word_swap(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;
        context.stack_push(b);
        context.stack_push(a);
        Ok(())
    }

    // ===== Variable Operations =====

    fn register_variable_words(module: &mut Module) {
        // VARIABLES
        let word = Arc::new(ModuleWord::new(
            "VARIABLES".to_string(),
            Self::word_variables,
        ));
        module.add_exportable_word(word);

        // !
        let word = Arc::new(ModuleWord::new("!".to_string(), Self::word_store));
        module.add_exportable_word(word);

        // @
        let word = Arc::new(ModuleWord::new("@".to_string(), Self::word_fetch));
        module.add_exportable_word(word);

        // !@
        let word = Arc::new(ModuleWord::new("!@".to_string(), Self::word_store_fetch));
        module.add_exportable_word(word);
    }

    fn word_variables(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        if let ForthicValue::Array(varnames) = val {
            let cur_module = context.cur_module_mut();

            for varname_val in varnames {
                if let ForthicValue::String(varname) = varname_val {
                    // Validate variable name - no __ prefix allowed
                    if varname.starts_with("__") {
                        return Err(ForthicError::InvalidVariableName {
                            forthic: "".to_string(),
                            varname,
                            location: None,
                            cause: None,
                        });
                    }
                    cur_module.add_variable(varname, ForthicValue::Null);
                }
            }
        }

        Ok(())
    }

    fn word_store(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let varname_val = context.stack_pop()?;
        let value = context.stack_pop()?;

        if let ForthicValue::String(varname) = varname_val {
            // Validate variable name - no __ prefix allowed
            if varname.starts_with("__") {
                return Err(ForthicError::InvalidVariableName {
                    forthic: "".to_string(),
                    varname,
                    location: None,
                    cause: None,
                });
            }

            let cur_module = context.cur_module_mut();

            // Get or create variable
            if cur_module.get_variable(&varname).is_none() {
                cur_module.add_variable(varname.clone(), ForthicValue::Null);
            }

            // Set value
            if let Some(var) = cur_module.get_variable_mut(&varname) {
                var.set_value(value);
            }
        }

        Ok(())
    }

    fn word_fetch(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let varname_val = context.stack_pop()?;

        if let ForthicValue::String(varname) = varname_val {
            // Validate variable name - no __ prefix allowed
            if varname.starts_with("__") {
                return Err(ForthicError::InvalidVariableName {
                    forthic: "".to_string(),
                    varname,
                    location: None,
                    cause: None,
                });
            }

            // Get or create variable
            let value = {
                let cur_module = context.cur_module_mut();

                if cur_module.get_variable(&varname).is_none() {
                    cur_module.add_variable(varname.clone(), ForthicValue::Null);
                }

                // Get value
                cur_module
                    .get_variable(&varname)
                    .map(|var| var.get_value().clone())
                    .unwrap_or(ForthicValue::Null)
            };

            context.stack_push(value);
        } else {
            context.stack_push(ForthicValue::Null);
        }

        Ok(())
    }

    fn word_store_fetch(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let varname_val = context.stack_pop()?;
        let value = context.stack_pop()?;

        if let ForthicValue::String(varname) = varname_val {
            // Validate variable name - no __ prefix allowed
            if varname.starts_with("__") {
                return Err(ForthicError::InvalidVariableName {
                    forthic: "".to_string(),
                    varname,
                    location: None,
                    cause: None,
                });
            }

            let cur_module = context.cur_module_mut();

            // Get or create variable
            if cur_module.get_variable(&varname).is_none() {
                cur_module.add_variable(varname.clone(), ForthicValue::Null);
            }

            // Set value
            if let Some(var) = cur_module.get_variable_mut(&varname) {
                var.set_value(value.clone());
            }

            // Push value back
            context.stack_push(value);
        } else {
            context.stack_push(value);
        }

        Ok(())
    }

    // ===== Control Flow Operations =====

    fn register_control_words(module: &mut Module) {
        // IDENTITY
        let word = Arc::new(ModuleWord::new("IDENTITY".to_string(), Self::word_identity));
        module.add_exportable_word(word);

        // NOP
        let word = Arc::new(ModuleWord::new("NOP".to_string(), Self::word_nop));
        module.add_exportable_word(word);

        // NULL
        let word = Arc::new(ModuleWord::new("NULL".to_string(), Self::word_null));
        module.add_exportable_word(word);

        // ARRAY?
        let word = Arc::new(ModuleWord::new("ARRAY?".to_string(), Self::word_is_array));
        module.add_exportable_word(word);

        // DEFAULT
        let word = Arc::new(ModuleWord::new("DEFAULT".to_string(), Self::word_default));
        module.add_exportable_word(word);
    }

    fn word_identity(_context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        // No-op
        Ok(())
    }

    fn word_nop(_context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        // No-op
        Ok(())
    }

    fn word_null(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.stack_push(ForthicValue::Null);
        Ok(())
    }

    fn word_is_array(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        let is_array = matches!(val, ForthicValue::Array(_));
        context.stack_push(ForthicValue::Bool(is_array));
        Ok(())
    }

    fn word_default(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let default_value = context.stack_pop()?;
        let value = context.stack_pop()?;

        match value {
            ForthicValue::Null => context.stack_push(default_value),
            ForthicValue::String(s) if s.is_empty() => context.stack_push(default_value),
            _ => context.stack_push(value),
        }

        Ok(())
    }

    // ===== Options Operations =====

    fn register_options_words(module: &mut Module) {
        // ~>
        let word = Arc::new(ModuleWord::new("~>".to_string(), Self::word_to_options));
        module.add_exportable_word(word);
    }

    fn word_to_options(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        if let ForthicValue::Array(arr) = val {
            // Convert to WordOptions
            match WordOptions::from_flat_array(&arr) {
                Ok(options) => context.stack_push(ForthicValue::WordOptions(options)),
                Err(_) => context.stack_push(ForthicValue::Null),
            }
        } else {
            // Not an array, push back null
            context.stack_push(ForthicValue::Null);
        }

        Ok(())
    }
}

impl Default for CoreModule {
    fn default() -> Self {
        Self::new()
    }
}
