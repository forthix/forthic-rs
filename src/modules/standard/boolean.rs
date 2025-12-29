//! Boolean module for Forthic
//!
//! Provides comparison, logic, and membership operations for boolean values and conditions.
//!
//! ## Categories
//! - Comparison: ==, !=, <, <=, >, >=
//! - Logic: OR, AND, NOT, XOR, NAND
//! - Membership: IN, ANY, ALL
//! - Conversion: >BOOL

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{InterpreterContext, Module, ModuleWord};
use std::sync::Arc;

/// BooleanModule provides comparison and logic operations
pub struct BooleanModule {
    module: Module,
}

impl BooleanModule {
    /// Create a new BooleanModule
    pub fn new() -> Self {
        let mut module = Module::new("boolean".to_string());

        // Register all words
        Self::register_comparison_words(&mut module);
        Self::register_logic_words(&mut module);
        Self::register_membership_words(&mut module);
        Self::register_conversion_words(&mut module);

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

    // ===== Comparison Operations =====

    fn register_comparison_words(module: &mut Module) {
        // ==
        let word = Arc::new(ModuleWord::new("==".to_string(), Self::word_equals));
        module.add_exportable_word(word);

        // !=
        let word = Arc::new(ModuleWord::new("!=".to_string(), Self::word_not_equals));
        module.add_exportable_word(word);

        // <
        let word = Arc::new(ModuleWord::new("<".to_string(), Self::word_less_than));
        module.add_exportable_word(word);

        // <=
        let word = Arc::new(ModuleWord::new("<=".to_string(), Self::word_less_than_or_equal));
        module.add_exportable_word(word);

        // >
        let word = Arc::new(ModuleWord::new(">".to_string(), Self::word_greater_than));
        module.add_exportable_word(word);

        // >=
        let word = Arc::new(ModuleWord::new(">=".to_string(), Self::word_greater_than_or_equal));
        module.add_exportable_word(word);
    }

    fn word_equals(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(Self::values_equal(&a, &b)));
        Ok(())
    }

    fn word_not_equals(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(!Self::values_equal(&a, &b)));
        Ok(())
    }

    fn word_less_than(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;

        let result = match (&a, &b) {
            (ForthicValue::Int(av), ForthicValue::Int(bv)) => *av < *bv,
            (ForthicValue::Float(av), ForthicValue::Float(bv)) => *av < *bv,
            (ForthicValue::Int(av), ForthicValue::Float(bv)) => (*av as f64) < *bv,
            (ForthicValue::Float(av), ForthicValue::Int(bv)) => *av < (*bv as f64),
            (ForthicValue::String(av), ForthicValue::String(bv)) => av < bv,
            _ => false,
        };

        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    fn word_less_than_or_equal(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;

        let result = match (&a, &b) {
            (ForthicValue::Int(av), ForthicValue::Int(bv)) => *av <= *bv,
            (ForthicValue::Float(av), ForthicValue::Float(bv)) => *av <= *bv,
            (ForthicValue::Int(av), ForthicValue::Float(bv)) => (*av as f64) <= *bv,
            (ForthicValue::Float(av), ForthicValue::Int(bv)) => *av <= (*bv as f64),
            (ForthicValue::String(av), ForthicValue::String(bv)) => av <= bv,
            _ => Self::values_equal(&a, &b),
        };

        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    fn word_greater_than(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;

        let result = match (&a, &b) {
            (ForthicValue::Int(av), ForthicValue::Int(bv)) => *av > *bv,
            (ForthicValue::Float(av), ForthicValue::Float(bv)) => *av > *bv,
            (ForthicValue::Int(av), ForthicValue::Float(bv)) => (*av as f64) > *bv,
            (ForthicValue::Float(av), ForthicValue::Int(bv)) => *av > (*bv as f64),
            (ForthicValue::String(av), ForthicValue::String(bv)) => av > bv,
            _ => false,
        };

        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    fn word_greater_than_or_equal(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;

        let result = match (&a, &b) {
            (ForthicValue::Int(av), ForthicValue::Int(bv)) => *av >= *bv,
            (ForthicValue::Float(av), ForthicValue::Float(bv)) => *av >= *bv,
            (ForthicValue::Int(av), ForthicValue::Float(bv)) => (*av as f64) >= *bv,
            (ForthicValue::Float(av), ForthicValue::Int(bv)) => *av >= (*bv as f64),
            (ForthicValue::String(av), ForthicValue::String(bv)) => av >= bv,
            _ => Self::values_equal(&a, &b),
        };

        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    // ===== Logic Operations =====

    fn register_logic_words(module: &mut Module) {
        // OR
        let word = Arc::new(ModuleWord::new("OR".to_string(), Self::word_or));
        module.add_exportable_word(word);

        // AND
        let word = Arc::new(ModuleWord::new("AND".to_string(), Self::word_and));
        module.add_exportable_word(word);

        // NOT
        let word = Arc::new(ModuleWord::new("NOT".to_string(), Self::word_not));
        module.add_exportable_word(word);

        // XOR
        let word = Arc::new(ModuleWord::new("XOR".to_string(), Self::word_xor));
        module.add_exportable_word(word);

        // NAND
        let word = Arc::new(ModuleWord::new("NAND".to_string(), Self::word_nand));
        module.add_exportable_word(word);
    }

    fn word_or(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;

        // Case 1: Array on top of stack
        if let ForthicValue::Array(arr) = b {
            for val in arr {
                if Self::is_truthy(&val) {
                    context.stack_push(ForthicValue::Bool(true));
                    return Ok(());
                }
            }
            context.stack_push(ForthicValue::Bool(false));
            return Ok(());
        }

        // Case 2: Two values
        let a = context.stack_pop()?;
        let result = Self::is_truthy(&a) || Self::is_truthy(&b);
        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    fn word_and(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;

        // Case 1: Array on top of stack
        if let ForthicValue::Array(arr) = b {
            for val in arr {
                if !Self::is_truthy(&val) {
                    context.stack_push(ForthicValue::Bool(false));
                    return Ok(());
                }
            }
            context.stack_push(ForthicValue::Bool(true));
            return Ok(());
        }

        // Case 2: Two values
        let a = context.stack_pop()?;
        let result = Self::is_truthy(&a) && Self::is_truthy(&b);
        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    fn word_not(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(!Self::is_truthy(&val)));
        Ok(())
    }

    fn word_xor(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;
        let a_bool = Self::is_truthy(&a);
        let b_bool = Self::is_truthy(&b);
        let result = (a_bool || b_bool) && !(a_bool && b_bool);
        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    fn word_nand(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;
        let result = !(Self::is_truthy(&a) && Self::is_truthy(&b));
        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    // ===== Membership Operations =====

    fn register_membership_words(module: &mut Module) {
        // IN
        let word = Arc::new(ModuleWord::new("IN".to_string(), Self::word_in));
        module.add_exportable_word(word);

        // ANY
        let word = Arc::new(ModuleWord::new("ANY".to_string(), Self::word_any));
        module.add_exportable_word(word);

        // ALL
        let word = Arc::new(ModuleWord::new("ALL".to_string(), Self::word_all));
        module.add_exportable_word(word);
    }

    fn word_in(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let array = context.stack_pop()?;
        let item = context.stack_pop()?;

        if let ForthicValue::Array(arr) = array {
            let result = arr.iter().any(|val| Self::values_equal(val, &item));
            context.stack_push(ForthicValue::Bool(result));
        } else {
            context.stack_push(ForthicValue::Bool(false));
        }
        Ok(())
    }

    fn word_any(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let items2 = context.stack_pop()?;
        let items1 = context.stack_pop()?;

        match (&items1, &items2) {
            (ForthicValue::Array(arr1), ForthicValue::Array(arr2)) => {
                // If items2 is empty, return true
                if arr2.is_empty() {
                    context.stack_push(ForthicValue::Bool(true));
                    return Ok(());
                }

                // Check if any item from items1 is in items2
                for item in arr1 {
                    if arr2.iter().any(|val| Self::values_equal(val, item)) {
                        context.stack_push(ForthicValue::Bool(true));
                        return Ok(());
                    }
                }
                context.stack_push(ForthicValue::Bool(false));
            }
            _ => context.stack_push(ForthicValue::Bool(false)),
        }
        Ok(())
    }

    fn word_all(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let items2 = context.stack_pop()?;
        let items1 = context.stack_pop()?;

        match (&items1, &items2) {
            (ForthicValue::Array(arr1), ForthicValue::Array(arr2)) => {
                // If items2 is empty, return true
                if arr2.is_empty() {
                    context.stack_push(ForthicValue::Bool(true));
                    return Ok(());
                }

                // Check if all items from items2 are in items1
                for item in arr2 {
                    if !arr1.iter().any(|val| Self::values_equal(val, item)) {
                        context.stack_push(ForthicValue::Bool(false));
                        return Ok(());
                    }
                }
                context.stack_push(ForthicValue::Bool(true));
            }
            _ => context.stack_push(ForthicValue::Bool(false)),
        }
        Ok(())
    }

    // ===== Conversion Operations =====

    fn register_conversion_words(module: &mut Module) {
        // >BOOL
        let word = Arc::new(ModuleWord::new(">BOOL".to_string(), Self::word_to_bool));
        module.add_exportable_word(word);
    }

    fn word_to_bool(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(Self::is_truthy(&val)));
        Ok(())
    }

    // ===== Helper Functions =====

    /// Check if two values are equal
    fn values_equal(a: &ForthicValue, b: &ForthicValue) -> bool {
        match (a, b) {
            (ForthicValue::Null, ForthicValue::Null) => true,
            (ForthicValue::Bool(av), ForthicValue::Bool(bv)) => av == bv,
            (ForthicValue::Int(av), ForthicValue::Int(bv)) => av == bv,
            (ForthicValue::Float(av), ForthicValue::Float(bv)) => av == bv,
            (ForthicValue::Int(av), ForthicValue::Float(bv)) => (*av as f64) == *bv,
            (ForthicValue::Float(av), ForthicValue::Int(bv)) => *av == (*bv as f64),
            (ForthicValue::String(av), ForthicValue::String(bv)) => av == bv,
            (ForthicValue::Array(av), ForthicValue::Array(bv)) => {
                if av.len() != bv.len() {
                    return false;
                }
                av.iter().zip(bv.iter()).all(|(a, b)| Self::values_equal(a, b))
            }
            _ => false,
        }
    }

    /// Check if a value is truthy (JavaScript-style truthiness)
    fn is_truthy(val: &ForthicValue) -> bool {
        match val {
            ForthicValue::Null => false,
            ForthicValue::Bool(b) => *b,
            ForthicValue::Int(i) => *i != 0,
            ForthicValue::Float(f) => *f != 0.0,
            ForthicValue::String(s) => !s.is_empty(),
            ForthicValue::Array(a) => !a.is_empty(),
            _ => true,
        }
    }
}

impl Default for BooleanModule {
    fn default() -> Self {
        Self::new()
    }
}
