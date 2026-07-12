//! Boolean module for Forthic
//!
//! Provides comparison, logic, and membership operations for boolean values and conditions.
//!
//! ## Categories
//! - Comparison: ==, !=, <, <=, >, >=
//! - Logic: OR, AND, NOT, XOR, NAND
//! - Membership: CONTAINS?, ANY, ALL, ANY?, ALL?
//! - Conversion: >BOOL

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{register_words, InterpreterContext, Module};

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
        register_words!(module, {
            "==" => Self::word_equals,
                "( a:any b:any -- equal:boolean )",
                "Test equality";
            "!=" => Self::word_not_equals,
                "( a:any b:any -- not_equal:boolean )",
                "Test inequality";
            "<" => Self::word_less_than,
                "( a:any b:any -- less_than:boolean )",
                "Less than";
            "<=" => Self::word_less_than_or_equal,
                "( a:any b:any -- less_equal:boolean )",
                "Less than or equal";
            ">" => Self::word_greater_than,
                "( a:any b:any -- greater_than:boolean )",
                "Greater than";
            ">=" => Self::word_greater_than_or_equal,
                "( a:any b:any -- greater_equal:boolean )",
                "Greater than or equal";
        });
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

    fn word_greater_than_or_equal(
        context: &mut dyn InterpreterContext,
    ) -> Result<(), ForthicError> {
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
        register_words!(module, {
            "OR" => Self::word_or,
                "( a:boolean b:boolean -- result:boolean )",
                "Logical OR of two values (truthiness-coerced); an array operand errors — use ANY?";
            "AND" => Self::word_and,
                "( a:boolean b:boolean -- result:boolean )",
                "Logical AND of two values (truthiness-coerced); an array operand errors — use ALL?";
            "NOT" => Self::word_not,
                "( bool:boolean -- result:boolean )",
                "Logical NOT";
            "XOR" => Self::word_xor,
                "( a:boolean b:boolean -- result:boolean )",
                "Logical XOR: true when exactly one operand is truthy";
            "NAND" => Self::word_nand,
                "( a:boolean b:boolean -- result:boolean )",
                "Logical NAND: true unless both operands are truthy";
        });
    }

    /// OR: ( a b -- bool ) — strictly two operands; an ARRAY operand is an
    /// error pointing at ANY? (ts contract — the old rs array-collapse form
    /// silently changed arity). Non-boolean operands coerce by truthiness
    /// and the result is always a Bool (ts returns a raw operand there — a
    /// JS || accident; sanctioned divergence).
    fn word_or(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;
        if matches!(a, ForthicValue::Array(_)) || matches!(b, ForthicValue::Array(_)) {
            return Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: "OR takes two values. For an array of booleans, use ANY?.".to_string(),
                location: None,
                cause: None,
            });
        }
        let result = Self::is_truthy(&a) || Self::is_truthy(&b);
        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    /// AND: ( a b -- bool ) — see OR; arrays error toward ALL?
    fn word_and(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;
        if matches!(a, ForthicValue::Array(_)) || matches!(b, ForthicValue::Array(_)) {
            return Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: "AND takes two values. For an array of booleans, use ALL?.".to_string(),
                location: None,
                cause: None,
            });
        }
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
        // CONTAINS? is haystack-first; it replaces the classic item-first
        // IN, dropped per the no-aliases decision
        register_words!(module, {
            "CONTAINS?" => Self::word_contains_q,
                "( haystack:any[] needle:any -- bool:boolean )",
                "Check if haystack array contains needle. Container-first arg order.";
            "ANY?" => Self::word_any_q,
                "( bools:boolean[] -- result:boolean )",
                "Returns true if any element of the array is truthy. False for empty array.";
            "ALL?" => Self::word_all_q,
                "( bools:boolean[] -- result:boolean )",
                "Returns true if all elements of the array are truthy. True for empty array.";
            "ANY" => Self::word_any,
                "( items1:any[] items2:any[] -- any:boolean )",
                "Check if any item from items1 is in items2";
            "ALL" => Self::word_all,
                "( items1:any[] items2:any[] -- all:boolean )",
                "Check if all items from items2 are in items1";
        });
    }

    /// CONTAINS?: ( haystack:any[] needle -- bool ) — container-first arg
    /// order (ts canonical; classic IN was item-first). Non-array haystack
    /// is false, not an error. Membership uses values_equal (structural);
    /// ts .includes is === — identical for scalars, a documented corner
    /// for records.
    fn word_contains_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let needle = context.stack_pop()?;
        let haystack = context.stack_pop()?;
        let result = match haystack {
            ForthicValue::Array(arr) => arr.iter().any(|v| Self::values_equal(v, &needle)),
            _ => false,
        };
        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    /// ANY?: ( bools:any[] -- bool ) — any element truthy; false on empty
    fn word_any_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let items = Self::require_array(context, "ANY?")?;
        let result = items.iter().any(Self::is_truthy);
        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    /// ALL?: ( bools:any[] -- bool ) — all elements truthy; true on empty
    fn word_all_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let items = Self::require_array(context, "ALL?")?;
        let result = items.iter().all(Self::is_truthy);
        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    fn require_array(
        context: &mut dyn InterpreterContext,
        word: &str,
    ) -> Result<Vec<ForthicValue>, ForthicError> {
        match context.stack_pop()? {
            ForthicValue::Array(arr) => Ok(arr),
            other => Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: format!("{word} requires an array of booleans (got {other:?})"),
                location: None,
                cause: None,
            }),
        }
    }

    fn word_any(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let items2 = context.stack_pop()?;
        let items1 = context.stack_pop()?;

        match (&items1, &items2) {
            (ForthicValue::Array(arr1), ForthicValue::Array(arr2)) => {
                // No special case for empty items2: nothing can match against
                // an empty set, so the loop correctly yields false (the old
                // return-true branch was a bug, fixed in ts #31)

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
        register_words!(module, {
            ">BOOL" => Self::word_to_bool,
                "( a:any -- bool:boolean )",
                "Convert to boolean (JavaScript truthiness: empty arrays are truthy, NaN is falsy)";
        });
    }

    fn word_to_bool(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(Self::is_truthy(&val)));
        Ok(())
    }

    // ===== Helper Functions =====

    /// Check if two values are equal
    pub(crate) fn values_equal(a: &ForthicValue, b: &ForthicValue) -> bool {
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
                av.iter()
                    .zip(bv.iter())
                    .all(|(a, b)| Self::values_equal(a, b))
            }
            (ForthicValue::Date(av), ForthicValue::Date(bv)) => av == bv,
            (ForthicValue::Time(av), ForthicValue::Time(bv)) => av == bv,
            // ts compares Temporal values by ISO string, which includes the
            // timezone annotation — the same instant in different timezones
            // is NOT equal. chrono's == compares instants only, so the
            // timezone check is required for parity.
            (ForthicValue::DateTime(av), ForthicValue::DateTime(bv)) => {
                av == bv && av.timezone().name() == bv.timezone().name()
            }
            (ForthicValue::Record(av), ForthicValue::Record(bv)) => {
                av.len() == bv.len()
                    && av
                        .iter()
                        .all(|(k, v)| bv.get(k).is_some_and(|bv2| Self::values_equal(v, bv2)))
            }
            _ => false,
        }
    }

    /// Check if a value is truthy (JavaScript-style truthiness)
    fn is_truthy(val: &ForthicValue) -> bool {
        // JS truthiness lives on ForthicValue (shared with IF/WHEN in core).
        // Note two fixes vs the old local copy: empty arrays are TRUTHY
        // (JS Boolean([]) === true) and NaN is falsy.
        val.is_truthy()
    }
}

impl Default for BooleanModule {
    fn default() -> Self {
        Self::new()
    }
}
