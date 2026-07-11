// Array module for Forthic
//
// Array and collection operations for manipulating arrays and records.
//
// ## Categories
// - Access: NTH, FIRST, LAST, SLICE, TAKE, TAKE-LAST, DROP, LENGTH
// - Transform: REVERSE
// - Combine: APPEND, ZIP, CONCAT
// - Filter: UNIQUE, DIFFERENCE, INTERSECTION, UNION
// - Utility: FLATTEN, RANGE, UNPACK
//
// Record-aware words follow the ts #33 contract: record in -> record out,
// entries in insertion order.

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{InterpreterContext, Module, ModuleWord};
use indexmap::IndexMap;
use std::collections::HashSet;
use std::sync::Arc;

/// SLICE pads out-of-range indexes, so a huge span would materialize a huge
/// array; guard it (same limit as forthic-ts)
const MAX_MATERIALIZED_ELEMENTS: i64 = 10_000_000;

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

        // FIRST
        let word = Arc::new(ModuleWord::new("FIRST".to_string(), Self::word_first));
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

        // TAKE-LAST
        let word = Arc::new(ModuleWord::new(
            "TAKE-LAST".to_string(),
            Self::word_take_last,
        ));
        module.add_exportable_word(word);
    }

    fn word_first(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;

        let result = match container {
            ForthicValue::Array(arr) => arr.first().cloned().unwrap_or(ForthicValue::Null),
            ForthicValue::Record(rec) => {
                // Insertion order (ts #33)
                rec.first()
                    .map(|(_, v)| v.clone())
                    .unwrap_or(ForthicValue::Null)
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_length(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;

        let length = match container {
            ForthicValue::Array(ref arr) => arr.len() as i64,
            ForthicValue::Record(ref rec) => rec.len() as i64,
            // Chars, not bytes ('🦀' has length 1, not 4). ts currently
            // reports UTF-16 units (2 for '🦀') — unifying on code points
            // is backlog item 18.
            ForthicValue::String(ref s) => s.chars().count() as i64,
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
                // Insertion order (ts #33) — IndexMap makes this direct
                if n < 0 {
                    ForthicValue::Null
                } else {
                    rec.get_index(n as usize)
                        .map(|(_, v)| v.clone())
                        .unwrap_or(ForthicValue::Null)
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
                // Insertion order (ts #33)
                rec.last()
                    .map(|(_, v)| v.clone())
                    .unwrap_or(ForthicValue::Null)
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
                match Self::slice_indexes(start, end, arr.len())? {
                    None => ForthicValue::Array(vec![]),
                    Some(indexes) => {
                        // Out-of-range indexes pad with nulls (arrays only)
                        let sliced = indexes
                            .into_iter()
                            .map(|i| match i {
                                Some(i) => arr[i].clone(),
                                None => ForthicValue::Null,
                            })
                            .collect();
                        ForthicValue::Array(sliced)
                    }
                }
            }
            ForthicValue::Record(rec) => {
                // Record in -> record out, entries by insertion order;
                // out-of-range indexes are skipped rather than null-padded
                match Self::slice_indexes(start, end, rec.len())? {
                    None => ForthicValue::Record(IndexMap::new()),
                    Some(indexes) => {
                        let mut result = IndexMap::new();
                        for i in indexes.into_iter().flatten() {
                            if let Some((k, v)) = rec.get_index(i) {
                                result.insert(k.clone(), v.clone());
                            }
                        }
                        ForthicValue::Record(result)
                    }
                }
            }
            ForthicValue::Null => ForthicValue::Array(vec![]),
            _ => container,
        };

        context.stack_push(result);
        Ok(())
    }

    /// Shared SLICE index walk: normalize negative indexes, then step from
    /// start toward end (either direction). Returns None when start is out
    /// of range (empty result); out-of-range steps yield None entries.
    /// Guards the span size — SLICE pads out-of-range indexes, so a huge
    /// end index would otherwise materialize a huge array.
    fn slice_indexes(
        start: i64,
        end: i64,
        len: usize,
    ) -> Result<Option<Vec<Option<usize>>>, ForthicError> {
        let len = len as i64;
        let normalize = |i: i64| if i < 0 { i + len } else { i };
        let start = normalize(start);
        let end = normalize(end);

        let span = (end - start).abs() + 1;
        if span > MAX_MATERIALIZED_ELEMENTS {
            return Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: format!(
                    "SLICE span {span} is too large (limit {MAX_MATERIALIZED_ELEMENTS})"
                ),
                location: None,
                cause: None,
            });
        }

        if start < 0 || start >= len {
            return Ok(None);
        }

        let step = if start > end { -1 } else { 1 };
        let mut indexes = Vec::new();
        let mut i = start;
        loop {
            if i < 0 || i >= len {
                indexes.push(None);
            } else {
                indexes.push(Some(i as usize));
            }
            if i == end {
                break;
            }
            i += step;
        }
        Ok(Some(indexes))
    }

    fn word_take(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        // Optional trailing WordOptions: ( container n [options] -- result )
        let options = Self::pop_options(context);
        let n_val = context.stack_pop()?;
        let container = context.stack_pop()?;

        let n = match n_val {
            ForthicValue::Int(i) => i.max(0) as usize,
            ForthicValue::Float(f) => f.max(0.0) as usize,
            _ => 0,
        };
        let push_rest = options
            .as_ref()
            .and_then(|o| o.get_bool("push_rest"))
            .unwrap_or(false);

        let (taken, rest) = match container {
            ForthicValue::Array(arr) => {
                let taken: Vec<_> = arr.iter().take(n).cloned().collect();
                let rest: Vec<_> = arr.iter().skip(n).cloned().collect();
                (ForthicValue::Array(taken), ForthicValue::Array(rest))
            }
            ForthicValue::Record(rec) => {
                // Record in -> record out, insertion order (ts #33)
                let taken: IndexMap<_, _> = rec
                    .iter()
                    .take(n)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                let rest: IndexMap<_, _> = rec
                    .iter()
                    .skip(n)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                (ForthicValue::Record(taken), ForthicValue::Record(rest))
            }
            ForthicValue::Null => (ForthicValue::Array(vec![]), ForthicValue::Array(vec![])),
            other => (other, ForthicValue::Array(vec![])),
        };

        if push_rest {
            context.stack_push(taken);
            context.stack_push(rest);
        } else {
            context.stack_push(taken);
        }
        Ok(())
    }

    fn word_drop(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let n_val = context.stack_pop()?;
        let container = context.stack_pop()?;

        let n = match n_val {
            ForthicValue::Int(i) => i,
            ForthicValue::Float(f) => f as i64,
            _ => 0,
        };
        // n <= 0 skips nothing (ts SKIP returns the container unchanged)
        if n <= 0 {
            context.stack_push(container);
            return Ok(());
        }
        let n = n as usize;

        let result = match container {
            ForthicValue::Array(arr) => {
                let dropped: Vec<_> = arr.iter().skip(n).cloned().collect();
                ForthicValue::Array(dropped)
            }
            ForthicValue::Record(rec) => {
                // Record in -> record out, insertion order (ts #33)
                let rest: IndexMap<_, _> = rec
                    .iter()
                    .skip(n)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                ForthicValue::Record(rest)
            }
            ForthicValue::Null => ForthicValue::Array(vec![]),
            _ => container,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_take_last(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let n_val = context.stack_pop()?;
        let container = context.stack_pop()?;

        let n = match n_val {
            ForthicValue::Int(i) => i,
            ForthicValue::Float(f) => f as i64,
            _ => 0,
        };

        let result = match container {
            _ if n <= 0 => match container {
                ForthicValue::Record(_) => ForthicValue::Record(IndexMap::new()),
                _ => ForthicValue::Array(vec![]),
            },
            ForthicValue::Array(arr) => {
                let skip = arr.len().saturating_sub(n as usize);
                ForthicValue::Array(arr.iter().skip(skip).cloned().collect())
            }
            ForthicValue::Record(rec) => {
                let skip = rec.len().saturating_sub(n as usize);
                let tail: IndexMap<_, _> = rec
                    .iter()
                    .skip(skip)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                ForthicValue::Record(tail)
            }
            ForthicValue::Null => ForthicValue::Array(vec![]),
            other => other,
        };

        context.stack_push(result);
        Ok(())
    }

    /// Pop a WordOptions value if one sits on top of the stack (Forthic's
    /// optional-trailing-options convention: `... [.push_rest TRUE] ~> TAKE`)
    fn pop_options(
        context: &mut dyn InterpreterContext,
    ) -> Option<crate::word_options::WordOptions> {
        if matches!(context.stack_peek(), Some(ForthicValue::WordOptions(_))) {
            if let Ok(ForthicValue::WordOptions(options)) = context.stack_pop() {
                return Some(options);
            }
        }
        None
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
            ForthicValue::Null => ForthicValue::Array(vec![item]),
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
        let word = Arc::new(ModuleWord::new(
            "DIFFERENCE".to_string(),
            Self::word_difference,
        ));
        module.add_exportable_word(word);

        // INTERSECTION
        let word = Arc::new(ModuleWord::new(
            "INTERSECTION".to_string(),
            Self::word_intersection,
        ));
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
        context.stack_push(Self::set_op(left, right, false));
        Ok(())
    }

    fn word_intersection(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let right = context.stack_pop()?;
        let left = context.stack_pop()?;
        context.stack_push(Self::set_op(left, right, true));
        Ok(())
    }

    /// Shared set operation for DIFFERENCE (keep=false) and INTERSECTION
    /// (keep=true). The result follows the LEFT operand's shape (ts #31):
    /// - array left: element membership against the right's elements
    ///   (or its values when the right is a record);
    /// - record left: keep/drop entries whose KEY is in the right's key set
    ///   (its string elements if the right is an array, its keys if it's a
    ///   record) — INTERSECTION behaves like PICK, DIFFERENCE like OMIT.
    fn set_op(left: ForthicValue, right: ForthicValue, keep: bool) -> ForthicValue {
        match left {
            ForthicValue::Array(l) => {
                let r_set: HashSet<String> = match &right {
                    ForthicValue::Array(r) => r.iter().map(Self::value_to_key).collect(),
                    ForthicValue::Record(r) => r.values().map(Self::value_to_key).collect(),
                    _ => HashSet::new(),
                };
                let filtered: Vec<_> = l
                    .into_iter()
                    .filter(|v| r_set.contains(&Self::value_to_key(v)) == keep)
                    .collect();
                ForthicValue::Array(filtered)
            }
            ForthicValue::Record(l) => {
                // Key membership: only string elements of an array right can
                // match record keys (same in ts, where Set.has compares ===)
                let r_keys: HashSet<String> = match &right {
                    ForthicValue::Array(r) => r
                        .iter()
                        .filter_map(|v| v.as_string().map(str::to_string))
                        .collect(),
                    ForthicValue::Record(r) => r.keys().cloned().collect(),
                    _ => HashSet::new(),
                };
                let filtered: IndexMap<_, _> = l
                    .into_iter()
                    .filter(|(k, _)| r_keys.contains(k) == keep)
                    .collect();
                ForthicValue::Record(filtered)
            }
            ForthicValue::Null => ForthicValue::Array(vec![]),
            other => other,
        }
    }

    fn word_union(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let right = context.stack_pop()?;
        let left = context.stack_pop()?;

        let result = match (left, right) {
            (ForthicValue::Array(l), ForthicValue::Array(r)) => {
                let mut seen = HashSet::new();
                let mut union = Vec::new();

                for item in l.into_iter().chain(r) {
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
            ForthicValue::Record(rec) => {
                // Values in insertion order (ts #33)
                for (_, value) in rec {
                    context.stack_push(value);
                }
            }
            _ => context.stack_push(container),
        }

        Ok(())
    }

    // ===== Helper Functions =====

    /// Convert ForthicValue to a string key for hashing. Int and Float share
    /// the numeric keyspace (JS has one number type, so 1 and 1.0 are the
    /// same set element there — and values_equal treats them as equal too).
    fn value_to_key(val: &ForthicValue) -> String {
        match val {
            ForthicValue::Null => "null".to_string(),
            ForthicValue::Bool(b) => format!("bool:{}", b),
            ForthicValue::Int(i) => format!("num:{}", *i as f64),
            ForthicValue::Float(f) => format!("num:{}", f),
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
