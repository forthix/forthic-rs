// Array module for Forthic
//
// Array and collection operations for manipulating arrays and records.
//
// ## Categories
// - Access: NTH, LAST, SLICE, TAKE, DROP, LENGTH
// - Transform: REVERSE
// - Combine: APPEND, ZIP, CONCAT
// - Filter: UNIQUE, DIFFERENCE, INTERSECTION, UNION
// - Utility: FLATTEN, RANGE, UNPACK

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{InterpreterContext, Module, ModuleWord};
use std::collections::HashSet;
use std::sync::Arc;

/// ArrayModule provides array and collection operations
pub struct ArrayModule {
    module: Module,
}

impl ArrayModule {
    /// Create a new ArrayModule
    pub fn new() -> Self {
        let mut module = Module::new("array".to_string());

        // Register all words
        Self::register_access_words(&mut module);
        Self::register_transform_words(&mut module);
        Self::register_combine_words(&mut module);
        Self::register_filter_words(&mut module);
        Self::register_utility_words(&mut module);

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

    // ===== Access Operations =====

    fn register_access_words(module: &mut Module) {
        // LENGTH
        let word = Arc::new(ModuleWord::new("LENGTH".to_string(), Self::word_length));
        module.add_exportable_word(word);

        // NTH
        let word = Arc::new(ModuleWord::new("NTH".to_string(), Self::word_nth));
        module.add_exportable_word(word);

        // LAST
        let word = Arc::new(ModuleWord::new("LAST".to_string(), Self::word_last));
        module.add_exportable_word(word);

        // SLICE
        let word = Arc::new(ModuleWord::new("SLICE".to_string(), Self::word_slice));
        module.add_exportable_word(word);

        // TAKE
        let word = Arc::new(ModuleWord::new("TAKE".to_string(), Self::word_take));
        module.add_exportable_word(word);

        // DROP
        let word = Arc::new(ModuleWord::new("DROP".to_string(), Self::word_drop));
        module.add_exportable_word(word);
    }

    fn word_length(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;

        let length = match container {
            ForthicValue::Array(ref arr) => arr.len() as i64,
            ForthicValue::Record(ref rec) => rec.len() as i64,
            ForthicValue::String(ref s) => s.len() as i64,
            ForthicValue::Null => 0,
            _ => 0,
        };

        context.stack_push(ForthicValue::Int(length));
        Ok(())
    }

    fn word_nth(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let n_val = context.stack_pop()?;
        let container = context.stack_pop()?;

        let n = match n_val {
            ForthicValue::Int(i) => i,
            _ => {
                context.stack_push(ForthicValue::Null);
                return Ok(());
            }
        };

        let result = match container {
            ForthicValue::Array(arr) => {
                if n < 0 || n >= arr.len() as i64 {
                    ForthicValue::Null
                } else {
                    arr[n as usize].clone()
                }
            }
            ForthicValue::Record(rec) => {
                let mut keys: Vec<_> = rec.keys().cloned().collect();
                keys.sort();
                if n < 0 || n >= keys.len() as i64 {
                    ForthicValue::Null
                } else {
                    rec.get(&keys[n as usize]).cloned().unwrap_or(ForthicValue::Null)
                }
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_last(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;

        let result = match container {
            ForthicValue::Array(arr) => {
                if arr.is_empty() {
                    ForthicValue::Null
                } else {
                    arr[arr.len() - 1].clone()
                }
            }
            ForthicValue::Record(rec) => {
                let mut keys: Vec<_> = rec.keys().cloned().collect();
                keys.sort();
                if keys.is_empty() {
                    ForthicValue::Null
                } else {
                    rec.get(&keys[keys.len() - 1]).cloned().unwrap_or(ForthicValue::Null)
                }
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_slice(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let end_val = context.stack_pop()?;
        let start_val = context.stack_pop()?;
        let container = context.stack_pop()?;

        let start = match start_val {
            ForthicValue::Int(i) => i,
            ForthicValue::Float(f) => f as i64,
            _ => 0,
        };

        let end = match end_val {
            ForthicValue::Int(i) => i,
            ForthicValue::Float(f) => f as i64,
            _ => 0,
        };

        let result = match container {
            ForthicValue::Array(arr) => {
                let len = arr.len() as i64;
                let norm_start = if start < 0 { start + len } else { start };
                let norm_end = if end < 0 { end + len } else { end };

                if norm_start < 0 || norm_start >= len {
                    ForthicValue::Array(vec![])
                } else {
                    let step = if norm_start > norm_end { -1 } else { 1 };
                    let mut result = Vec::new();
                    let mut i = norm_start;

                    loop {
                        if i < 0 || i >= len {
                            result.push(ForthicValue::Null);
                        } else {
                            result.push(arr[i as usize].clone());
                        }
                        if i == norm_end {
                            break;
                        }
                        i += step;
                    }

                    ForthicValue::Array(result)
                }
            }
            ForthicValue::Null => ForthicValue::Array(vec![]),
            _ => container,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_take(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let n_val = context.stack_pop()?;
        let container = context.stack_pop()?;

        let n = match n_val {
            ForthicValue::Int(i) => i as usize,
            ForthicValue::Float(f) => f as usize,
            _ => 0,
        };

        let result = match container {
            ForthicValue::Array(arr) => {
                let taken: Vec<_> = arr.iter().take(n).cloned().collect();
                ForthicValue::Array(taken)
            }
            ForthicValue::Null => ForthicValue::Array(vec![]),
            _ => container,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_drop(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let n_val = context.stack_pop()?;
        let container = context.stack_pop()?;

        let n = match n_val {
            ForthicValue::Int(i) => i as usize,
            ForthicValue::Float(f) => f as usize,
            _ => 0,
        };

        let result = match container {
            ForthicValue::Array(arr) => {
                let dropped: Vec<_> = arr.iter().skip(n).cloned().collect();
                ForthicValue::Array(dropped)
            }
            ForthicValue::Null => ForthicValue::Array(vec![]),
            _ => container,
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Transform Operations =====

    fn register_transform_words(module: &mut Module) {
        // REVERSE
        let word = Arc::new(ModuleWord::new("REVERSE".to_string(), Self::word_reverse));
        module.add_exportable_word(word);
    }

    fn word_reverse(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;

        let result = match container {
            ForthicValue::Array(mut arr) => {
                arr.reverse();
                ForthicValue::Array(arr)
            }
            _ => container,
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Combine Operations =====

    fn register_combine_words(module: &mut Module) {
        // APPEND
        let word = Arc::new(ModuleWord::new("APPEND".to_string(), Self::word_append));
        module.add_exportable_word(word);

        // CONCAT
        let word = Arc::new(ModuleWord::new("CONCAT".to_string(), Self::word_concat));
        module.add_exportable_word(word);

        // ZIP
        let word = Arc::new(ModuleWord::new("ZIP".to_string(), Self::word_zip));
        module.add_exportable_word(word);
    }

    fn word_append(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let item = context.stack_pop()?;
        let container = context.stack_pop()?;

        let result = match container {
            ForthicValue::Array(mut arr) => {
                arr.push(item);
                ForthicValue::Array(arr)
            }
            ForthicValue::Record(mut rec) => {
                // Item should be [key, value] array
                if let ForthicValue::Array(ref kv) = item {
                    if kv.len() >= 2 {
                        if let ForthicValue::String(ref key) = kv[0] {
                            rec.insert(key.clone(), kv[1].clone());
                        }
                    }
                }
                ForthicValue::Record(rec)
            }
            ForthicValue::Null => {
                let mut arr = Vec::new();
                arr.push(item);
                ForthicValue::Array(arr)
            }
            _ => container,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_concat(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let right = context.stack_pop()?;
        let left = context.stack_pop()?;

        let result = match (left, right) {
            (ForthicValue::Array(mut l), ForthicValue::Array(r)) => {
                l.extend(r);
                ForthicValue::Array(l)
            }
            (ForthicValue::Null, ForthicValue::Array(r)) => ForthicValue::Array(r),
            (ForthicValue::Array(l), ForthicValue::Null) => ForthicValue::Array(l),
            (l, _) => l,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_zip(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let right = context.stack_pop()?;
        let left = context.stack_pop()?;

        let result = match (left, right) {
            (ForthicValue::Array(l), ForthicValue::Array(r)) => {
                let mut result = Vec::new();
                let len = l.len().min(r.len());
                for i in 0..len {
                    result.push(ForthicValue::Array(vec![l[i].clone(), r[i].clone()]));
                }
                ForthicValue::Array(result)
            }
            _ => ForthicValue::Array(vec![]),
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Filter Operations =====

    fn register_filter_words(module: &mut Module) {
        // UNIQUE
        let word = Arc::new(ModuleWord::new("UNIQUE".to_string(), Self::word_unique));
        module.add_exportable_word(word);

        // DIFFERENCE
        let word = Arc::new(ModuleWord::new("DIFFERENCE".to_string(), Self::word_difference));
        module.add_exportable_word(word);

        // INTERSECTION
        let word = Arc::new(ModuleWord::new("INTERSECTION".to_string(), Self::word_intersection));
        module.add_exportable_word(word);

        // UNION
        let word = Arc::new(ModuleWord::new("UNION".to_string(), Self::word_union));
        module.add_exportable_word(word);
    }

    fn word_unique(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;

        let result = match container {
            ForthicValue::Array(arr) => {
                let mut seen = HashSet::new();
                let mut unique = Vec::new();

                for item in arr {
                    let key = Self::value_to_key(&item);
                    if seen.insert(key) {
                        unique.push(item);
                    }
                }

                ForthicValue::Array(unique)
            }
            _ => container,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_difference(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let right = context.stack_pop()?;
        let left = context.stack_pop()?;

        let result = match (left, right) {
            (ForthicValue::Array(l), ForthicValue::Array(r)) => {
                let r_set: HashSet<_> = r.iter().map(|v| Self::value_to_key(v)).collect();
                let diff: Vec<_> = l
                    .into_iter()
                    .filter(|v| !r_set.contains(&Self::value_to_key(v)))
                    .collect();
                ForthicValue::Array(diff)
            }
            _ => ForthicValue::Array(vec![]),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_intersection(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let right = context.stack_pop()?;
        let left = context.stack_pop()?;

        let result = match (left, right) {
            (ForthicValue::Array(l), ForthicValue::Array(r)) => {
                let r_set: HashSet<_> = r.iter().map(|v| Self::value_to_key(v)).collect();
                let inter: Vec<_> = l
                    .into_iter()
                    .filter(|v| r_set.contains(&Self::value_to_key(v)))
                    .collect();
                ForthicValue::Array(inter)
            }
            _ => ForthicValue::Array(vec![]),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_union(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let right = context.stack_pop()?;
        let left = context.stack_pop()?;

        let result = match (left, right) {
            (ForthicValue::Array(l), ForthicValue::Array(r)) => {
                let mut seen = HashSet::new();
                let mut union = Vec::new();

                for item in l.into_iter().chain(r.into_iter()) {
                    let key = Self::value_to_key(&item);
                    if seen.insert(key) {
                        union.push(item);
                    }
                }

                ForthicValue::Array(union)
            }
            _ => ForthicValue::Array(vec![]),
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Utility Operations =====

    fn register_utility_words(module: &mut Module) {
        // FLATTEN
        let word = Arc::new(ModuleWord::new("FLATTEN".to_string(), Self::word_flatten));
        module.add_exportable_word(word);

        // RANGE
        let word = Arc::new(ModuleWord::new("RANGE".to_string(), Self::word_range));
        module.add_exportable_word(word);

        // UNPACK
        let word = Arc::new(ModuleWord::new("UNPACK".to_string(), Self::word_unpack));
        module.add_exportable_word(word);
    }

    fn word_flatten(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;

        let result = match container {
            ForthicValue::Array(arr) => {
                let flattened = Self::flatten_recursive(&arr, 1);
                ForthicValue::Array(flattened)
            }
            _ => container,
        };

        context.stack_push(result);
        Ok(())
    }

    fn flatten_recursive(arr: &[ForthicValue], depth: i32) -> Vec<ForthicValue> {
        if depth <= 0 {
            return arr.to_vec();
        }

        let mut result = Vec::new();
        for item in arr {
            match item {
                ForthicValue::Array(inner) => {
                    result.extend(Self::flatten_recursive(inner, depth - 1));
                }
                _ => result.push(item.clone()),
            }
        }
        result
    }

    fn word_range(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let end_val = context.stack_pop()?;
        let start_val = context.stack_pop()?;

        let start = match start_val {
            ForthicValue::Int(i) => i,
            ForthicValue::Float(f) => f as i64,
            _ => 0,
        };

        let end = match end_val {
            ForthicValue::Int(i) => i,
            ForthicValue::Float(f) => f as i64,
            _ => 0,
        };

        let range: Vec<_> = if start <= end {
            (start..=end).map(ForthicValue::Int).collect()
        } else {
            (end..=start).rev().map(ForthicValue::Int).collect()
        };

        context.stack_push(ForthicValue::Array(range));
        Ok(())
    }

    fn word_unpack(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;

        match container {
            ForthicValue::Array(arr) => {
                for item in arr {
                    context.stack_push(item);
                }
            }
            _ => context.stack_push(container),
        }

        Ok(())
    }

    // ===== Helper Functions =====

    /// Convert ForthicValue to a string key for hashing
    fn value_to_key(val: &ForthicValue) -> String {
        match val {
            ForthicValue::Null => "null".to_string(),
            ForthicValue::Bool(b) => format!("bool:{}", b),
            ForthicValue::Int(i) => format!("int:{}", i),
            ForthicValue::Float(f) => format!("float:{}", f),
            ForthicValue::String(s) => format!("string:{}", s),
            _ => format!("{:?}", val),
        }
    }
}

impl Default for ArrayModule {
    fn default() -> Self {
        Self::new()
    }
}
