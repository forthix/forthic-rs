//! Math module for Forthic
//!
//! Mathematical operations and utilities including arithmetic, aggregation, and conversions.
//!
//! ## Categories
//! - Arithmetic: +, -, *, /, MOD
//! - Aggregates: MEAN, MAX, MIN, SUM, PRODUCT
//! - Type conversion: >INT, >FLOAT, ROUND, FLOOR, CEIL, FORMAT-FIXED
//! - Math functions: ABS, SQRT, CLAMP

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{register_words, InterpreterContext, Module};
use indexmap::IndexMap;

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
        register_words!(module, {
            "+" => Self::word_plus,
                "( a:number b:number -- sum:number )",
                "Add two numbers. For arrays use SUM.";
            "-" => Self::word_minus,
                "( a:number b:number -- difference:number )",
                "Subtract b from a";
            "*" => Self::word_times,
                "( a:number b:number -- product:number )",
                "Multiply two numbers. For arrays use PRODUCT.";
            "/" => Self::word_divide,
                "( a:number b:number -- quotient:number )",
                "Divide a by b (null on division by zero)";
            "MOD" => Self::word_mod,
                "( m:number n:number -- remainder:number )",
                "Modulo operation (m % n)";
        });
    }

    fn word_plus(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;

        // Strictly binary — an array operand errors, pointing at SUM
        // (matches forthic-ts and forthic-py).
        if matches!(a, ForthicValue::Array(_)) || matches!(b, ForthicValue::Array(_)) {
            return Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: "+ takes two numbers. For an array of numbers, use SUM.".to_string(),
                location: None,
                cause: None,
            });
        }

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
        let a = context.stack_pop()?;

        // Strictly binary — an array operand errors, pointing at PRODUCT
        // (matches forthic-ts and forthic-py).
        if matches!(a, ForthicValue::Array(_)) || matches!(b, ForthicValue::Array(_)) {
            return Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: "* takes two numbers. For an array of numbers, use PRODUCT.".to_string(),
                location: None,
                cause: None,
            });
        }

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
        register_words!(module, {
            "PRODUCT" => Self::word_product,
                "( numbers:number[] -- product:number )",
                "Product of array of numbers (1 if empty). Null/non-numeric elements yield null.";
            "SUM" => Self::word_sum,
                "( numbers:number[] -- sum:number )",
                "Sum of an array of numbers (non-numeric elements are skipped; non-array input passes through)";
            "MAX" => Self::word_max,
                "( numbers:number[] -- max:number )",
                "Maximum of an array of numbers (null if empty/all non-numeric); two scalars compare directly";
            "MIN" => Self::word_min,
                "( numbers:number[] -- min:number )",
                "Minimum of an array of numbers (null if empty/all non-numeric); two scalars compare directly";
            "MEAN" => Self::word_mean,
                "( items:any[] -- mean:any )",
                "Polymorphic mean: numbers average; strings give a frequency record; records give field-wise means; nulls skipped";
        });
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

            let numbers: Vec<f64> = arr.iter().filter_map(Self::to_number).collect();
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

            let numbers: Vec<f64> = arr.iter().filter_map(Self::to_number).collect();
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

    /// MEAN: ( items -- mean ) — polymorphic (ts contract):
    /// falsy input / empty array -> 0; truthy non-array -> as-is;
    /// single-element array -> that element as-is (even NULL — this check
    /// precedes null-filtering); NULL elements are SKIPPED (all-null -> 0);
    /// then dispatch on the first survivor: numbers -> arithmetic mean,
    /// strings -> frequency-distribution record, records -> field-wise
    /// mean over the union of keys (numeric fields mean, string fields
    /// frequency, other fields dropped); anything else -> 0.
    fn word_mean(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        context.stack_push(Self::mean_of(&val));
        Ok(())
    }

    fn mean_of(val: &ForthicValue) -> ForthicValue {
        let ForthicValue::Array(arr) = val else {
            return if val.is_truthy() {
                val.clone()
            } else {
                ForthicValue::Int(0)
            };
        };
        if arr.is_empty() {
            return ForthicValue::Int(0);
        }
        if arr.len() == 1 {
            return arr[0].clone();
        }
        let filtered: Vec<&ForthicValue> = arr
            .iter()
            .filter(|v| !matches!(v, ForthicValue::Null))
            .collect();
        if filtered.is_empty() {
            return ForthicValue::Int(0);
        }
        match filtered[0] {
            ForthicValue::Int(_) | ForthicValue::Float(_) => Self::numeric_mean(&filtered),
            ForthicValue::String(_) => Self::frequency_record(&filtered),
            ForthicValue::Record(_) => Self::field_wise_mean(&filtered),
            _ => ForthicValue::Int(0),
        }
    }

    /// Sum via to_number (non-numeric stragglers count as 0 — ts's mixed
    /// arrays are unpinned JS-coercion territory), divide by the FILTERED
    /// length (ts divides by filtered.length, not the numeric count)
    fn numeric_mean(values: &[&ForthicValue]) -> ForthicValue {
        let sum: f64 = values.iter().filter_map(|v| Self::to_number(v)).sum();
        Self::number_to_value(sum / values.len() as f64)
    }

    /// ["a" "a" "b"] -> {a: 2/3, b: 1/3}, insertion order of first sighting
    fn frequency_record(values: &[&ForthicValue]) -> ForthicValue {
        let mut counts: IndexMap<String, usize> = IndexMap::new();
        for v in values {
            let key = match v {
                ForthicValue::String(s) => s.clone(),
                other => crate::modules::standard::string::StringModule::stringify(other),
            };
            *counts.entry(key).or_insert(0) += 1;
        }
        let len = values.len() as f64;
        ForthicValue::Record(
            counts
                .into_iter()
                .map(|(k, count)| (k, Self::number_to_value(count as f64 / len)))
                .collect(),
        )
    }

    fn field_wise_mean(values: &[&ForthicValue]) -> ForthicValue {
        // Union of keys in first-sighting order (non-record elements
        // contribute nothing, mirroring ts's undefined-field filtering)
        let mut keys: Vec<&String> = Vec::new();
        for v in values {
            if let ForthicValue::Record(rec) = v {
                for k in rec.keys() {
                    if !keys.contains(&k) {
                        keys.push(k);
                    }
                }
            }
        }
        let mut result = IndexMap::new();
        for key in keys {
            let field_values: Vec<&ForthicValue> = values
                .iter()
                .filter_map(|v| match v {
                    ForthicValue::Record(rec) => rec.get(key),
                    _ => None,
                })
                .filter(|v| !matches!(v, ForthicValue::Null))
                .collect();
            let Some(first) = field_values.first() else {
                continue; // all-null/missing field: dropped
            };
            match first {
                ForthicValue::Int(_) | ForthicValue::Float(_) => {
                    result.insert(key.clone(), Self::numeric_mean(&field_values));
                }
                ForthicValue::String(_) => {
                    result.insert(key.clone(), Self::frequency_record(&field_values));
                }
                _ => {} // other field types dropped (ts contract)
            }
        }
        ForthicValue::Record(result)
    }

    // ===== Conversion Operations =====

    fn register_conversion_words(module: &mut Module) {
        register_words!(module, {
            "FORMAT-FIXED" => Self::word_format_fixed,
                "( num:number digits:number -- result:string )",
                "Format number with fixed decimal places";
            ">INT" => Self::word_to_int,
                "( a:any -- int:number )",
                "Convert to integer (returns length for arrays, 0 for null/unparseable input)";
            ">FLOAT" => Self::word_to_float,
                "( a:any -- float:number )",
                "Convert to float (0.0 for null/unparseable input)";
            "ROUND" => Self::word_round,
                "( num:number -- int:number )",
                "Round to nearest integer";
        });
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
        register_words!(module, {
            "SQRT" => Self::word_sqrt,
                "( n:number -- sqrt:number )",
                "Square root (NaN for negative input, null for non-numeric)";
            "CLAMP" => Self::word_clamp,
                "( value:number min:number max:number -- clamped:number )",
                "Constrain value to range [min, max] (min wins when min > max)";
            "ABS" => Self::word_abs,
                "( n:number -- abs:number )",
                "Absolute value";
            "FLOOR" => Self::word_floor,
                "( n:number -- floor:number )",
                "Round down to integer";
            "CEIL" => Self::word_ceil,
                "( n:number -- ceil:number )",
                "Round up to integer";
        });
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

    /// PRODUCT: ( array -- product ) — empty array is 1. Deliberate ts
    /// asymmetry with SUM: non-array input is NULL (SUM says 0), and a
    /// NULL/non-numeric element nulls the WHOLE result (SUM skips them).
    fn word_product(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        let result = match val {
            ForthicValue::Array(arr) => {
                let mut product = 1.0f64;
                let mut all_numeric = true;
                for v in &arr {
                    match Self::to_number(v) {
                        Some(n) => product *= n,
                        None => {
                            all_numeric = false;
                            break;
                        }
                    }
                }
                if all_numeric {
                    Self::number_to_value(product)
                } else {
                    ForthicValue::Null
                }
            }
            _ => ForthicValue::Null,
        };
        context.stack_push(result);
        Ok(())
    }

    /// SQRT: ( n -- sqrt ) — negative input is NaN (JS Math.sqrt), not an
    /// error; NULL/non-numeric is NULL
    fn word_sqrt(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        let result = match Self::to_number(&val) {
            Some(n) => Self::number_to_value(n.sqrt()),
            None => ForthicValue::Null,
        };
        context.stack_push(result);
        Ok(())
    }

    /// CLAMP: ( value min max -- clamped ) — exactly JS
    /// Math.max(min, Math.min(max, value)), so when min > max, MIN WINS
    /// (ts contract; don't "fix"). Any NULL operand is NULL.
    fn word_clamp(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let max = context.stack_pop()?;
        let min = context.stack_pop()?;
        let value = context.stack_pop()?;
        let result = match (
            Self::to_number(&value),
            Self::to_number(&min),
            Self::to_number(&max),
        ) {
            (Some(v), Some(lo), Some(hi)) => {
                // JS Math.min/max PROPAGATE NaN; rust f64::min/max swallow
                // it — check explicitly
                if v.is_nan() || lo.is_nan() || hi.is_nan() {
                    ForthicValue::Float(f64::NAN)
                } else {
                    Self::number_to_value(lo.max(hi.min(v)))
                }
            }
            _ => ForthicValue::Null,
        };
        context.stack_push(result);
        Ok(())
    }

    /// FORMAT-FIXED: ( num digits -- string ) — JS Number.toFixed. NULL num
    /// is NULL; a NON-NUMERIC num is an ERROR (ts throws TypeError here,
    /// unlike SQRT/CLAMP); digits outside 0..=100 is an error (ts
    /// RangeError); NULL digits means 0; NaN/Infinity format as "NaN" /
    /// "Infinity". ts's >=1e21 exponential-notation quirk is NOT
    /// reproduced — large values format in plain decimal (documented).
    fn word_format_fixed(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let digits_val = context.stack_pop()?;
        let num_val = context.stack_pop()?;

        if matches!(num_val, ForthicValue::Null) {
            context.stack_push(ForthicValue::Null);
            return Ok(());
        }
        let Some(num) = Self::to_number(&num_val) else {
            return Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: "FORMAT-FIXED requires a number".to_string(),
                location: None,
                cause: None,
            });
        };
        let digits = match &digits_val {
            ForthicValue::Null => 0,
            other => match Self::to_number(other) {
                Some(d) => d.trunc() as i64,
                None => {
                    return Err(ForthicError::InvalidOperation {
                        forthic: String::new(),
                        message: "FORMAT-FIXED digits must be a number".to_string(),
                        location: None,
                        cause: None,
                    })
                }
            },
        };
        if !(0..=100).contains(&digits) {
            return Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: format!("FORMAT-FIXED digits must be between 0 and 100, got {digits}"),
                location: None,
                cause: None,
            });
        }

        context.stack_push(ForthicValue::String(Self::to_fixed(num, digits as usize)));
        Ok(())
    }

    /// JS toFixed rounds ties half-AWAY-from-zero ((0.5).toFixed(0) is
    /// "1"); Rust's format! rounds ties-to-even ("0"). Scale + f64::round
    /// (which IS half-away-from-zero) first, then format. Binary-inexact
    /// "ties" like 1.005 come out identically either way.
    fn to_fixed(num: f64, digits: usize) -> String {
        if num.is_nan() {
            return "NaN".to_string();
        }
        if num.is_infinite() {
            return if num > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
        }
        let factor = 10f64.powi(digits as i32);
        let rounded = (num * factor).round() / factor;
        format!("{rounded:.digits$}")
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
        // Collapse to Int only within the f64-exact integer range (2^53) —
        // beyond it `as i64` silently saturates (e.g. [1e18 100] PRODUCT)
        const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_992.0;
        if num.fract() == 0.0 && num.is_finite() && num.abs() <= MAX_SAFE_INTEGER {
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
