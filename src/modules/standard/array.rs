// Array module for Forthic
//
// Array and collection operations for manipulating arrays and records.
//
// ## Categories
// - Access: NTH, FIRST, LAST, SLICE, TAKE, TAKE-LAST, SKIP, LENGTH
// - Transform: REVERSE
// - Combine: APPEND, ZIP
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

/// MAP's option flags (see word_map)
struct MapFlags {
    with_key: bool,
    depth: i64,
    outcomes: bool,
}

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

        // SKIP (ts-canonical name; this word was previously misnamed DROP,
        // which in ts core means pop-top-of-stack — a cross-runtime landmine)
        let word = Arc::new(ModuleWord::new("SKIP".to_string(), Self::word_skip));
        module.add_exportable_word(word);

        // TAKE-LAST
        let word = Arc::new(ModuleWord::new(
            "TAKE-LAST".to_string(),
            Self::word_take_last,
        ));
        module.add_exportable_word(word);

        // MAP
        let word = Arc::new(ModuleWord::new("MAP".to_string(), Self::word_map));
        module.add_exportable_word(word);
    }

    /// MAP: ( items forthic [options] -- result )
    ///
    /// Runs `forthic` once per element — the element is pushed, the code
    /// runs via the context, and the popped result replaces the element.
    /// Arrays map to arrays; records map their values (keys and insertion
    /// order preserved). Options (ts parity):
    /// - `with_key`: push the key (records) or index (arrays) beneath the value
    /// - `depth` (int): descend nested containers this many levels, mapping
    ///   scalar leaves (post ts #31: leaves are mapped, never coerced to {})
    ///
    /// - `outcomes`: each element maps to `{"ok": value}` /
    ///   `{"error": {message, error_type}}` — per-element failures don't
    ///   abort and can't disturb the stack (MAP snapshots BEFORE pushing the
    ///   item, so a failed element consumes it; TRY composed inside MAP
    ///   would transactionally restore the pushed item and strand it).
    ///
    /// Without outcomes, errors propagate (Forthic's default `?`-like
    /// behavior). The ts push_error option was removed in both runtimes
    /// (flag-dependent arity, NULL/failure conflation, operand stranding),
    /// and the ts `interps` option (parallel interpreters) is not
    /// supported — the rs interpreter is deliberately synchronous.
    fn word_map(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let options = Self::pop_options(context);
        let forthic_val = context.stack_pop()?;
        let items = context.stack_pop()?;

        let forthic = match forthic_val {
            ForthicValue::String(s) => s,
            other => {
                return Err(ForthicError::InvalidOperation {
                    forthic: String::new(),
                    message: format!("MAP requires a Forthic string, got {other:?}"),
                    location: None,
                    cause: None,
                })
            }
        };

        let flags = MapFlags {
            with_key: options
                .as_ref()
                .and_then(|o| o.get_bool("with_key"))
                .unwrap_or(false),
            depth: options
                .as_ref()
                .and_then(|o| o.get_int("depth"))
                .unwrap_or(0),
            outcomes: options
                .as_ref()
                .and_then(|o| o.get_bool("outcomes"))
                .unwrap_or(false),
        };

        let result = match items {
            // Null/empty in, same out
            ForthicValue::Null => {
                context.stack_push(ForthicValue::Null);
                return Ok(());
            }
            ForthicValue::Array(arr) if arr.is_empty() => {
                context.stack_push(ForthicValue::Array(vec![]));
                return Ok(());
            }
            ForthicValue::Array(arr) => ForthicValue::Array(Self::map_descend_array(
                context,
                &arr,
                &forthic,
                flags.depth,
                &flags,
            )?),
            ForthicValue::Record(rec) => ForthicValue::Record(Self::map_descend_record(
                context,
                &rec,
                &forthic,
                flags.depth,
                &flags,
            )?),
            // Non-container: pass through unchanged (ts's behavior here is a
            // JS truthiness accident — {} for most scalars — not worth parity)
            other => other,
        };

        context.stack_push(result);
        Ok(())
    }

    fn map_descend_array(
        context: &mut dyn InterpreterContext,
        items: &[ForthicValue],
        forthic: &str,
        depth: i64,
        flags: &MapFlags,
    ) -> Result<Vec<ForthicValue>, ForthicError> {
        let mut accum = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            match item {
                ForthicValue::Array(inner) if depth > 0 => {
                    accum.push(ForthicValue::Array(Self::map_descend_array(
                        context,
                        inner,
                        forthic,
                        depth - 1,
                        flags,
                    )?));
                }
                ForthicValue::Record(inner) if depth > 0 => {
                    accum.push(ForthicValue::Record(Self::map_descend_record(
                        context,
                        inner,
                        forthic,
                        depth - 1,
                        flags,
                    )?));
                }
                // Scalar leaf (or depth exhausted): map it (ts #31 — never
                // coerce leaves to empty containers)
                leaf => accum.push(Self::map_one(
                    context,
                    ForthicValue::Int(i as i64),
                    leaf.clone(),
                    forthic,
                    flags,
                )?),
            }
        }
        Ok(accum)
    }

    fn map_descend_record(
        context: &mut dyn InterpreterContext,
        items: &IndexMap<String, ForthicValue>,
        forthic: &str,
        depth: i64,
        flags: &MapFlags,
    ) -> Result<IndexMap<String, ForthicValue>, ForthicError> {
        let mut accum = IndexMap::with_capacity(items.len());
        for (k, item) in items {
            let mapped = match item {
                ForthicValue::Array(inner) if depth > 0 => ForthicValue::Array(
                    Self::map_descend_array(context, inner, forthic, depth - 1, flags)?,
                ),
                ForthicValue::Record(inner) if depth > 0 => ForthicValue::Record(
                    Self::map_descend_record(context, inner, forthic, depth - 1, flags)?,
                ),
                leaf => Self::map_one(
                    context,
                    ForthicValue::String(k.clone()),
                    leaf.clone(),
                    forthic,
                    flags,
                )?,
            };
            accum.insert(k.clone(), mapped);
        }
        Ok(accum)
    }

    /// Map one leaf: push [key] value, run the forthic, pop the result.
    /// In outcomes mode each leaf yields {"ok": ...}/{"error": ...} instead,
    /// with the snapshot taken BEFORE the pushes — MAP owns them, so a
    /// failed element consumes the item and cannot strand it (this is why
    /// outcomes lives on MAP rather than being composed from TRY, whose
    /// snapshot would include the pushed item and faithfully restore it).
    fn map_one(
        context: &mut dyn InterpreterContext,
        key: ForthicValue,
        value: ForthicValue,
        forthic: &str,
        flags: &MapFlags,
    ) -> Result<ForthicValue, ForthicError> {
        if !flags.outcomes {
            if flags.with_key {
                context.stack_push(key);
            }
            context.stack_push(value);
            context.run(forthic)?;
            return context.stack_pop();
        }

        let snapshot = context.stack_snapshot();
        let module_depth = context.module_stack_depth();
        if flags.with_key {
            context.stack_push(key);
        }
        context.stack_push(value);
        match context.run(forthic) {
            Ok(()) => {
                // Same payload rule as TRY, relative to the pre-push
                // snapshot: a no-op code yields the pushed item itself
                let after = context.stack_snapshot();
                let unchanged = after == snapshot;
                let payload = if !unchanged && !after.is_empty() {
                    context.stack_pop()?
                } else {
                    ForthicValue::Null
                };
                Ok(super::core::ok_outcome(payload))
            }
            Err(e) => {
                context.stack_restore(snapshot);
                while context.module_stack_depth() > module_depth {
                    let _ = context.module_stack_pop();
                }
                Ok(super::core::error_outcome(&e))
            }
        }
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

    fn word_skip(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let n_val = context.stack_pop()?;
        let container = context.stack_pop()?;

        let n = match n_val {
            ForthicValue::Int(i) => i,
            ForthicValue::Float(f) => f as i64,
            _ => 0,
        };
        // n <= 0 skips nothing (returns the container unchanged)
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

    /// FLATTEN: ( container [options] -- flat )
    ///
    /// Fully flattens by default (ts contract — the old rs behavior
    /// flattened exactly one level, a silent divergence); the `depth`
    /// option limits descent. Records flatten to tab-joined key paths
    /// ("k1\tk2"), matching ts; empty records are leaves.
    fn word_flatten(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let options = Self::pop_options(context);
        let container = context.stack_pop()?;
        let depth = options.as_ref().and_then(|o| o.get_int("depth"));

        let result = match container {
            ForthicValue::Null => ForthicValue::Array(vec![]),
            ForthicValue::Array(arr) => ForthicValue::Array(Self::flatten_array(&arr, depth)),
            ForthicValue::Record(rec) => {
                let mut out = IndexMap::new();
                Self::flatten_record(&rec, depth, &mut out, &mut Vec::new());
                ForthicValue::Record(out)
            }
            other => other,
        };

        context.stack_push(result);
        Ok(())
    }

    fn flatten_array(items: &[ForthicValue], depth: Option<i64>) -> Vec<ForthicValue> {
        let mut accum = Vec::new();
        for item in items {
            match item {
                ForthicValue::Array(inner) if depth.is_none_or(|d| d > 0) => {
                    accum.extend(Self::flatten_array(inner, depth.map(|d| d - 1)));
                }
                other => accum.push(other.clone()),
            }
        }
        accum
    }

    fn flatten_record(
        rec: &IndexMap<String, ForthicValue>,
        depth: Option<i64>,
        out: &mut IndexMap<String, ForthicValue>,
        keys: &mut Vec<String>,
    ) {
        for (k, item) in rec {
            let descend = matches!(item, ForthicValue::Record(inner) if !inner.is_empty())
                && depth.is_none_or(|d| d > 0);
            if descend {
                if let ForthicValue::Record(inner) = item {
                    keys.push(k.clone());
                    Self::flatten_record(inner, depth.map(|d| d - 1), out, keys);
                    keys.pop();
                }
            } else {
                let key = if keys.is_empty() {
                    k.clone()
                } else {
                    format!("{}\t{}", keys.join("\t"), k)
                };
                out.insert(key, item.clone());
            }
        }
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

        // end < start yields an empty range (ts contract — the old rs
        // behavior produced a reversed descending range, a silent divergence)
        // and needs no bound; guard pathological sizes before allocating
        // (ts #34)
        if end >= start && end - start + 1 > MAX_MATERIALIZED_ELEMENTS {
            return Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: format!(
                    "RANGE size {} is too large (limit {MAX_MATERIALIZED_ELEMENTS})",
                    end - start + 1
                ),
                location: None,
                cause: None,
            });
        }
        let range: Vec<_> = if start <= end {
            (start..=end).map(ForthicValue::Int).collect()
        } else {
            Vec::new()
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
