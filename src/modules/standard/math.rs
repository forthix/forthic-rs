//! Math module for Forthic
//!
//! Mathematical operations and utilities including arithmetic, aggregation, and conversions.
//!
//! ## Categories
//! - Arithmetic: +, -, *, /, MOD
//! - Aggregates: MEAN, MAX, MIN, SUM
//! - Type conversion: >INT, >FLOAT, ROUND, FLOOR, CEIL
//! - Math functions: ABS

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{InterpreterContext, Module, ModuleWord};
use std::sync::Arc;

/// MathModule provides mathematical operations
pub struct MathModule {
    module: Module,
}

impl MathModule {
    /// Create a new MathModule
    pub fn new() -> Self {
        let mut module = Module::new("math".to_string());

        // Register all words
        Self::register_arithmetic_words(&mut module);
        Self::register_aggregate_words(&mut module);
        Self::register_conversion_words(&mut module);
        Self::register_math_functions(&mut module);

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

    // ===== Arithmetic Operations =====

    fn register_arithmetic_words(module: &mut Module) {
        // +
        let word = Arc::new(ModuleWord::new("+".to_string(), Self::word_plus));
        module.add_exportable_word(word);

        // -
        let word = Arc::new(ModuleWord::new("-".to_string(), Self::word_minus));
        module.add_exportable_word(word);

        // *
        let word = Arc::new(ModuleWord::new("*".to_string(), Self::word_times));
        module.add_exportable_word(word);

        // /
        let word = Arc::new(ModuleWord::new("/".to_string(), Self::word_divide));
        module.add_exportable_word(word);

        // MOD
        let word = Arc::new(ModuleWord::new("MOD".to_string(), Self::word_mod));
        module.add_exportable_word(word);
    }

    fn word_plus(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;

        // Case 1: Array on top of stack - sum all elements
        if let ForthicValue::Array(arr) = b {
            let mut sum = 0.0;
            for val in arr {
                if let Some(num) = Self::to_number(&val) {
                    sum += num;
                }
            }
            context.stack_push(Self::number_to_value(sum));
            return Ok(());
        }

        // Case 2: Two numbers
        let a = context.stack_pop()?;
        let num_a = Self::to_number(&a).unwrap_or(0.0);
        let num_b = Self::to_number(&b).unwrap_or(0.0);
        context.stack_push(Self::number_to_value(num_a + num_b));
        Ok(())
    }

    fn word_minus(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;

        match (Self::to_number(&a), Self::to_number(&b)) {
            (Some(num_a), Some(num_b)) => {
                context.stack_push(Self::number_to_value(num_a - num_b));
                Ok(())
            }
            _ => {
                context.stack_push(ForthicValue::Null);
                Ok(())
            }
        }
    }

    fn word_times(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;

        // Case 1: Array on top of stack - product of all elements
        if let ForthicValue::Array(arr) = b {
            let mut product = 1.0;
            for val in arr {
                match Self::to_number(&val) {
                    Some(num) => product *= num,
                    None => {
                        context.stack_push(ForthicValue::Null);
                        return Ok(());
                    }
                }
            }
            context.stack_push(Self::number_to_value(product));
            return Ok(());
        }

        // Case 2: Two numbers
        let a = context.stack_pop()?;
        match (Self::to_number(&a), Self::to_number(&b)) {
            (Some(num_a), Some(num_b)) => {
                context.stack_push(Self::number_to_value(num_a * num_b));
                Ok(())
            }
            _ => {
                context.stack_push(ForthicValue::Null);
                Ok(())
            }
        }
    }

    fn word_divide(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;

        match (Self::to_number(&a), Self::to_number(&b)) {
            (Some(num_a), Some(num_b)) => {
                if num_b == 0.0 {
                    context.stack_push(ForthicValue::Null);
                } else {
                    context.stack_push(Self::number_to_value(num_a / num_b));
                }
                Ok(())
            }
            _ => {
                context.stack_push(ForthicValue::Null);
                Ok(())
            }
        }
    }

    fn word_mod(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let n = context.stack_pop()?;
        let m = context.stack_pop()?;

        match (Self::to_number(&m), Self::to_number(&n)) {
            (Some(num_m), Some(num_n)) => {
                context.stack_push(Self::number_to_value(num_m % num_n));
                Ok(())
            }
            _ => {
                context.stack_push(ForthicValue::Null);
                Ok(())
            }
        }
    }

    // ===== Aggregate Operations =====

    fn register_aggregate_words(module: &mut Module) {
        // SUM
        let word = Arc::new(ModuleWord::new("SUM".to_string(), Self::word_sum));
        module.add_exportable_word(word);

        // MAX
        let word = Arc::new(ModuleWord::new("MAX".to_string(), Self::word_max));
        module.add_exportable_word(word);

        // MIN
        let word = Arc::new(ModuleWord::new("MIN".to_string(), Self::word_min));
        module.add_exportable_word(word);

        // MEAN
        let word = Arc::new(ModuleWord::new("MEAN".to_string(), Self::word_mean));
        module.add_exportable_word(word);
    }

    fn word_sum(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        if let ForthicValue::Array(arr) = val {
            let mut sum = 0.0;
            for item in arr {
                if let Some(num) = Self::to_number(&item) {
                    sum += num;
                }
            }
            context.stack_push(Self::number_to_value(sum));
        } else {
            context.stack_push(val);
        }
        Ok(())
    }

    fn word_max(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;

        // Case 1: Array on top of stack
        if let ForthicValue::Array(arr) = b {
            if arr.is_empty() {
                context.stack_push(ForthicValue::Null);
                return Ok(());
            }

            let numbers: Vec<f64> = arr.iter().filter_map(|v| Self::to_number(v)).collect();
            if numbers.is_empty() {
                context.stack_push(ForthicValue::Null);
                return Ok(());
            }

            let max = numbers.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            context.stack_push(Self::number_to_value(max));
            return Ok(());
        }

        // Case 2: Two values
        let a = context.stack_pop()?;
        match (Self::to_number(&a), Self::to_number(&b)) {
            (Some(num_a), Some(num_b)) => {
                context.stack_push(Self::number_to_value(num_a.max(num_b)));
                Ok(())
            }
            _ => {
                context.stack_push(ForthicValue::Null);
                Ok(())
            }
        }
    }

    fn word_min(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;

        // Case 1: Array on top of stack
        if let ForthicValue::Array(arr) = b {
            if arr.is_empty() {
                context.stack_push(ForthicValue::Null);
                return Ok(());
            }

            let numbers: Vec<f64> = arr.iter().filter_map(|v| Self::to_number(v)).collect();
            if numbers.is_empty() {
                context.stack_push(ForthicValue::Null);
                return Ok(());
            }

            let min = numbers.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            context.stack_push(Self::number_to_value(min));
            return Ok(());
        }

        // Case 2: Two values
        let a = context.stack_pop()?;
        match (Self::to_number(&a), Self::to_number(&b)) {
            (Some(num_a), Some(num_b)) => {
                context.stack_push(Self::number_to_value(num_a.min(num_b)));
                Ok(())
            }
            _ => {
                context.stack_push(ForthicValue::Null);
                Ok(())
            }
        }
    }

    fn word_mean(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        if let ForthicValue::Array(arr) = val {
            if arr.is_empty() {
                context.stack_push(ForthicValue::Int(0));
                return Ok(());
            }

            let numbers: Vec<f64> = arr.iter().filter_map(|v| Self::to_number(v)).collect();
            if numbers.is_empty() {
                context.stack_push(ForthicValue::Int(0));
                return Ok(());
            }

            let sum: f64 = numbers.iter().sum();
            let mean = sum / numbers.len() as f64;
            context.stack_push(Self::number_to_value(mean));
        } else {
            context.stack_push(val);
        }
        Ok(())
    }

    // ===== Conversion Operations =====

    fn register_conversion_words(module: &mut Module) {
        // >INT
        let word = Arc::new(ModuleWord::new(">INT".to_string(), Self::word_to_int));
        module.add_exportable_word(word);

        // >FLOAT
        let word = Arc::new(ModuleWord::new(">FLOAT".to_string(), Self::word_to_float));
        module.add_exportable_word(word);

        // ROUND
        let word = Arc::new(ModuleWord::new("ROUND".to_string(), Self::word_round));
        module.add_exportable_word(word);
    }

    fn word_to_int(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::Int(i) => ForthicValue::Int(i),
            ForthicValue::Float(f) => ForthicValue::Int(f as i64),
            ForthicValue::String(s) => {
                if let Ok(i) = s.parse::<i64>() {
                    ForthicValue::Int(i)
                } else {
                    ForthicValue::Int(0)
                }
            }
            ForthicValue::Bool(b) => ForthicValue::Int(if b { 1 } else { 0 }),
            ForthicValue::Array(ref a) => ForthicValue::Int(a.len() as i64),
            ForthicValue::Null => ForthicValue::Int(0),
            _ => ForthicValue::Int(0),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_to_float(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::Float(f) => ForthicValue::Float(f),
            ForthicValue::Int(i) => ForthicValue::Float(i as f64),
            ForthicValue::String(s) => {
                if let Ok(f) = s.parse::<f64>() {
                    ForthicValue::Float(f)
                } else {
                    ForthicValue::Float(0.0)
                }
            }
            ForthicValue::Bool(b) => ForthicValue::Float(if b { 1.0 } else { 0.0 }),
            _ => ForthicValue::Float(0.0),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_round(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        match Self::to_number(&val) {
            Some(num) => {
                context.stack_push(ForthicValue::Int(num.round() as i64));
                Ok(())
            }
            None => {
                context.stack_push(ForthicValue::Null);
                Ok(())
            }
        }
    }

    // ===== Math Functions =====

    fn register_math_functions(module: &mut Module) {
        // ABS
        let word = Arc::new(ModuleWord::new("ABS".to_string(), Self::word_abs));
        module.add_exportable_word(word);

        // FLOOR
        let word = Arc::new(ModuleWord::new("FLOOR".to_string(), Self::word_floor));
        module.add_exportable_word(word);

        // CEIL
        let word = Arc::new(ModuleWord::new("CEIL".to_string(), Self::word_ceil));
        module.add_exportable_word(word);
    }

    fn word_abs(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        match Self::to_number(&val) {
            Some(num) => {
                context.stack_push(Self::number_to_value(num.abs()));
                Ok(())
            }
            None => {
                context.stack_push(ForthicValue::Null);
                Ok(())
            }
        }
    }

    fn word_floor(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        match Self::to_number(&val) {
            Some(num) => {
                context.stack_push(ForthicValue::Int(num.floor() as i64));
                Ok(())
            }
            None => {
                context.stack_push(ForthicValue::Null);
                Ok(())
            }
        }
    }

    fn word_ceil(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        match Self::to_number(&val) {
            Some(num) => {
                context.stack_push(ForthicValue::Int(num.ceil() as i64));
                Ok(())
            }
            None => {
                context.stack_push(ForthicValue::Null);
                Ok(())
            }
        }
    }

    // ===== Helper Functions =====

    /// Convert ForthicValue to number (f64)
    fn to_number(val: &ForthicValue) -> Option<f64> {
        match val {
            ForthicValue::Int(i) => Some(*i as f64),
            ForthicValue::Float(f) => Some(*f),
            ForthicValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// Convert number to appropriate ForthicValue (Int or Float)
    fn number_to_value(num: f64) -> ForthicValue {
        if num.fract() == 0.0 && num.is_finite() {
            ForthicValue::Int(num as i64)
        } else {
            ForthicValue::Float(num)
        }
    }
}

impl Default for MathModule {
    fn default() -> Self {
        Self::new()
    }
}
