// Core module for Forthic
//
// Essential interpreter operations for stack manipulation, variables, and control flow.
//
// ## Categories
// - Stack: POP, DUP, SWAP
// - Variables: VARIABLES, !, @, !@
// - Control: IDENTITY, NOP, NULL, ARRAY?, DEFAULT
// - Options: ~> (converts array to WordOptions)

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{InterpreterContext, Module, ModuleWord};
use crate::word_options::WordOptions;
use std::sync::Arc;

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
        Self::register_options_words(&mut module);

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
                            varname: varname,
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
                    varname: varname,
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
                    varname: varname,
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
                cur_module.get_variable(&varname)
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
                    varname: varname,
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
