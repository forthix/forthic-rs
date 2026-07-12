// Array module for Forthic
//
// Array and collection operations for manipulating arrays and records.
//
// ## Categories
// - Access: NTH, FIRST, LAST, SLICE, TAKE, TAKE-LAST, SKIP, LENGTH
// - Transform: REVERSE, MAP, MAP-AT, ZIP-WITH, NUMBERED
// - Combine: APPEND, ZIP
// - Filter: UNIQUE, UNIQUE-BY, DIFFERENCE, INTERSECTION, UNION, FILTER
// - Higher-order: FOREACH, REDUCE, FIND, COUNT, TIMES-RUN
// - Sort: SORT, SORT-BY, SORT-U, MIN-BY, MAX-BY
// - Group: GROUP-BY, GROUP-BY-FIELD, BY-FIELD, GROUPS-OF, INDEX, KEY-OF
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
        Self::register_higher_order_words(&mut module);
        Self::register_query_words(&mut module);

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

    // ===== Higher-Order Words (Batch 2 — run Forthic per element) =====

    fn register_higher_order_words(module: &mut Module) {
        for (name, handler) in [
            (
                "FILTER",
                Self::word_filter as fn(&mut dyn InterpreterContext) -> Result<(), ForthicError>,
            ),
            ("FOREACH", Self::word_foreach),
            ("REDUCE", Self::word_reduce),
            ("FIND", Self::word_find),
            ("COUNT", Self::word_count),
            ("SORT", Self::word_sort),
            ("SORT-BY", Self::word_sort_by),
            ("MIN-BY", Self::word_min_by),
            ("MAX-BY", Self::word_max_by),
            ("UNIQUE-BY", Self::word_unique_by),
            ("TIMES-RUN", Self::word_times_run),
            ("ZIP-WITH", Self::word_zip_with),
            ("MAP-AT", Self::word_map_at),
        ] {
            let word = Arc::new(ModuleWord::new(name.to_string(), handler));
            module.add_exportable_word(word);
        }
    }

    fn register_query_words(module: &mut Module) {
        for (name, handler) in [
            (
                "SORT-U",
                Self::word_sort_u as fn(&mut dyn InterpreterContext) -> Result<(), ForthicError>,
            ),
            ("GROUP-BY", Self::word_group_by),
            ("GROUP-BY-FIELD", Self::word_group_by_field),
            ("BY-FIELD", Self::word_by_field),
            ("GROUPS-OF", Self::word_groups_of),
            ("INDEX", Self::word_index),
            ("KEY-OF", Self::word_key_of),
            ("NUMBERED", Self::word_numbered),
        ] {
            let word = Arc::new(ModuleWord::new(name.to_string(), handler));
            module.add_exportable_word(word);
        }
    }

    /// Push item (and optionally its key/index beneath it), run the code,
    /// pop the result — the per-element protocol shared by the batch-2 words
    fn run_on_item(
        context: &mut dyn InterpreterContext,
        key: Option<ForthicValue>,
        item: ForthicValue,
        forthic: &str,
    ) -> Result<ForthicValue, ForthicError> {
        if let Some(key) = key {
            context.stack_push(key);
        }
        context.stack_push(item);
        context.run(forthic)?;
        context.stack_pop()
    }

    /// Total order over ForthicValues for SORT/SORT-BY/MIN-BY/MAX-BY: numbers
    /// numeric (Int and Float share the number line), strings lexicographic,
    /// NULL sorts LAST (ts natural_cmp), cross-type by fixed rank. Ties are
    /// Equal, and the sorts are stable, so ties keep input order.
    fn natural_cmp(a: &ForthicValue, b: &ForthicValue) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        fn rank(v: &ForthicValue) -> u8 {
            match v {
                ForthicValue::Bool(_) => 0,
                ForthicValue::Int(_) | ForthicValue::Float(_) => 1,
                ForthicValue::String(_) => 2,
                ForthicValue::Date(_) => 3,
                ForthicValue::Time(_) => 4,
                ForthicValue::DateTime(_) => 5,
                ForthicValue::Array(_) => 6,
                ForthicValue::Record(_) => 7,
                ForthicValue::Null => 9, // null sorts last
                _ => 8,
            }
        }
        fn as_f64(v: &ForthicValue) -> Option<f64> {
            match v {
                ForthicValue::Int(i) => Some(*i as f64),
                ForthicValue::Float(f) => Some(*f),
                _ => None,
            }
        }
        match (a, b) {
            (ForthicValue::Null, ForthicValue::Null) => Ordering::Equal,
            _ => match (as_f64(a), as_f64(b)) {
                (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
                _ => match (a, b) {
                    (ForthicValue::String(x), ForthicValue::String(y)) => x.cmp(y),
                    (ForthicValue::Bool(x), ForthicValue::Bool(y)) => x.cmp(y),
                    (ForthicValue::Date(x), ForthicValue::Date(y)) => x.cmp(y),
                    (ForthicValue::Time(x), ForthicValue::Time(y)) => x.cmp(y),
                    (ForthicValue::DateTime(x), ForthicValue::DateTime(y)) => x.cmp(y),
                    _ => rank(a).cmp(&rank(b)),
                },
            },
        }
    }

    /// Scalar value -> record key string, matching JS property-key coercion
    /// (5 -> "5", true -> "true", null -> "null"); container keys error
    fn value_to_key_string(v: &ForthicValue) -> Result<String, ForthicError> {
        match v {
            ForthicValue::String(s) => Ok(s.clone()),
            ForthicValue::Int(i) => Ok(i.to_string()),
            ForthicValue::Float(f) => Ok(f.to_string()),
            ForthicValue::Bool(b) => Ok(b.to_string()),
            ForthicValue::Null => Ok("null".to_string()),
            other => Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: format!("Cannot use {other:?} as a record key"),
                location: None,
                cause: None,
            }),
        }
    }

    /// Container -> (key, value) pairs: arrays yield Int indexes, records
    /// yield String keys in insertion order. NULL/other -> empty.
    fn keyed_items(container: &ForthicValue) -> Vec<(ForthicValue, ForthicValue)> {
        match container {
            ForthicValue::Array(arr) => arr
                .iter()
                .enumerate()
                .map(|(i, v)| (ForthicValue::Int(i as i64), v.clone()))
                .collect(),
            ForthicValue::Record(rec) => rec
                .iter()
                .map(|(k, v)| (ForthicValue::String(k.clone()), v.clone()))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// FILTER: ( container forthic [options] -- filtered ) — keep elements
    /// whose predicate result is truthy. Record in -> record out (keys and
    /// insertion order preserved); falsy container passes through unchanged.
    fn word_filter(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let options = Self::pop_options(context);
        let forthic = Self::pop_code(context, "FILTER")?;
        let container = context.stack_pop()?;
        let with_key = options
            .as_ref()
            .and_then(|o| o.get_bool("with_key"))
            .unwrap_or(false);

        let result = match &container {
            ForthicValue::Array(arr) => {
                let mut kept = Vec::new();
                for (i, item) in arr.iter().enumerate() {
                    let key = with_key.then(|| ForthicValue::Int(i as i64));
                    if Self::run_on_item(context, key, item.clone(), &forthic)?.is_truthy() {
                        kept.push(item.clone());
                    }
                }
                ForthicValue::Array(kept)
            }
            ForthicValue::Record(rec) => {
                let mut kept = IndexMap::new();
                for (k, item) in rec {
                    let key = with_key.then(|| ForthicValue::String(k.clone()));
                    if Self::run_on_item(context, key, item.clone(), &forthic)?.is_truthy() {
                        kept.insert(k.clone(), item.clone());
                    }
                }
                ForthicValue::Record(kept)
            }
            _ => container.clone(),
        };
        context.stack_push(result);
        Ok(())
    }

    /// FOREACH: ( items forthic [options] -- ? ) — run the code per element;
    /// whatever it leaves stays on the stack. Error tolerance is
    /// composition: items "'W' TRY" FOREACH-style via MAP outcomes.
    fn word_foreach(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let options = Self::pop_options(context);
        let forthic = Self::pop_code(context, "FOREACH")?;
        let container = context.stack_pop()?;
        let with_key = options
            .as_ref()
            .and_then(|o| o.get_bool("with_key"))
            .unwrap_or(false);

        for (key, item) in Self::keyed_items(&container) {
            if with_key {
                context.stack_push(key);
            }
            context.stack_push(item);
            context.run(&forthic)?;
        }
        Ok(())
    }

    /// REDUCE: ( container initial forthic -- result ) — push initial once,
    /// run the code per element (which must net `( acc item -- acc )`),
    /// pop once at the end. Records reduce over their values.
    fn word_reduce(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = Self::pop_code(context, "REDUCE")?;
        let initial = context.stack_pop()?;
        let container = context.stack_pop()?;

        context.stack_push(initial);
        for (_, item) in Self::keyed_items(&container) {
            context.stack_push(item);
            context.run(&forthic)?;
        }
        let result = context.stack_pop()?;
        context.stack_push(result);
        Ok(())
    }

    /// FIND: ( items forthic -- item|NULL ) — first element whose predicate
    /// is truthy; SHORT-CIRCUITS (remaining elements never run)
    fn word_find(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = Self::pop_code(context, "FIND")?;
        let container = context.stack_pop()?;

        for (_, item) in Self::keyed_items(&container) {
            if Self::run_on_item(context, None, item.clone(), &forthic)?.is_truthy() {
                context.stack_push(item);
                return Ok(());
            }
        }
        context.stack_push(ForthicValue::Null);
        Ok(())
    }

    /// COUNT: ( items forthic -- n ) — number of elements whose predicate is
    /// truthy (runs the code on every element)
    fn word_count(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = Self::pop_code(context, "COUNT")?;
        let container = context.stack_pop()?;

        let mut count = 0i64;
        for (_, item) in Self::keyed_items(&container) {
            if Self::run_on_item(context, None, item, &forthic)?.is_truthy() {
                count += 1;
            }
        }
        context.stack_push(ForthicValue::Int(count));
        Ok(())
    }

    /// SORT: ( container [options] -- sorted ) — stable natural_cmp sort
    /// (NULL sorts last). The `comparator` option is a KEY FUNCTION, not a
    /// two-argument comparator (the ts docstring's "SWAP -" example is
    /// stale): each element is pushed, the code runs, and the popped value
    /// is that element's sort key. Keys are computed for all elements in
    /// input order BEFORE sorting. Non-arrays (including records) pass
    /// through unchanged.
    fn word_sort(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let options = Self::pop_options(context);
        let container = context.stack_pop()?;
        let comparator = options.as_ref().and_then(|o| o.get_string("comparator"));

        let ForthicValue::Array(arr) = container else {
            context.stack_push(container);
            return Ok(());
        };

        let sorted = match comparator {
            None => {
                let mut items = arr;
                items.sort_by(Self::natural_cmp);
                items
            }
            Some(forthic) => Self::sort_by_key(context, arr, forthic)?,
        };
        context.stack_push(ForthicValue::Array(sorted));
        Ok(())
    }

    /// SORT-BY: ( items forthic -- sorted ) — ascending by the code-produced
    /// key; stable (ties keep input order). Non-arrays pass through.
    fn word_sort_by(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = Self::pop_code(context, "SORT-BY")?;
        let container = context.stack_pop()?;
        let ForthicValue::Array(arr) = container else {
            context.stack_push(container);
            return Ok(());
        };
        let sorted = Self::sort_by_key(context, arr, &forthic)?;
        context.stack_push(ForthicValue::Array(sorted));
        Ok(())
    }

    /// Decorate (compute all keys in input order) — stable sort — undecorate
    fn sort_by_key(
        context: &mut dyn InterpreterContext,
        items: Vec<ForthicValue>,
        forthic: &str,
    ) -> Result<Vec<ForthicValue>, ForthicError> {
        let mut decorated = Vec::with_capacity(items.len());
        for item in items {
            let key = Self::run_on_item(context, None, item.clone(), forthic)?;
            decorated.push((key, item));
        }
        decorated.sort_by(|(ka, _), (kb, _)| Self::natural_cmp(ka, kb));
        Ok(decorated.into_iter().map(|(_, item)| item).collect())
    }

    /// MIN-BY / MAX-BY: ( items forthic -- item|NULL ) — smallest/largest by
    /// code-produced key; ties keep the EARLIEST element; NULL for
    /// non-array or empty input. Runs the code on every element.
    fn word_min_by(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        Self::extreme_by(context, "MIN-BY", std::cmp::Ordering::Less)
    }

    fn word_max_by(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        Self::extreme_by(context, "MAX-BY", std::cmp::Ordering::Greater)
    }

    fn extreme_by(
        context: &mut dyn InterpreterContext,
        word: &str,
        wanted: std::cmp::Ordering,
    ) -> Result<(), ForthicError> {
        let forthic = Self::pop_code(context, word)?;
        let container = context.stack_pop()?;
        let ForthicValue::Array(arr) = container else {
            context.stack_push(ForthicValue::Null);
            return Ok(());
        };

        let mut best: Option<(ForthicValue, ForthicValue)> = None;
        for item in arr {
            let key = Self::run_on_item(context, None, item.clone(), &forthic)?;
            let better = match &best {
                None => true,
                // Strict comparison: ties keep the earliest element
                Some((best_key, _)) => Self::natural_cmp(&key, best_key) == wanted,
            };
            if better {
                best = Some((key, item));
            }
        }
        context.stack_push(best.map(|(_, item)| item).unwrap_or(ForthicValue::Null));
        Ok(())
    }

    /// UNIQUE-BY: ( items forthic -- items ) — dedupe by code-produced key
    /// (structural equality), keeping the FIRST occurrence, input order
    /// preserved. Non-arrays pass through.
    fn word_unique_by(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = Self::pop_code(context, "UNIQUE-BY")?;
        let container = context.stack_pop()?;
        let ForthicValue::Array(arr) = container else {
            context.stack_push(container);
            return Ok(());
        };

        let mut seen: Vec<ForthicValue> = Vec::new();
        let mut kept = Vec::new();
        for item in arr {
            let key = Self::run_on_item(context, None, item.clone(), &forthic)?;
            if !seen
                .iter()
                .any(|s| crate::modules::standard::boolean::BooleanModule::values_equal(s, &key))
            {
                seen.push(key);
                kept.push(item);
            }
        }
        context.stack_push(ForthicValue::Array(kept));
        Ok(())
    }

    /// TIMES-RUN: ( num forthic -- ) — run the code n times against the
    /// current stack (no per-iteration values pushed). Fractional counts
    /// truncate; NULL count or empty code is a no-op.
    fn word_times_run(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = context.stack_pop()?;
        let num = context.stack_pop()?;
        let n = match num {
            ForthicValue::Int(i) => i,
            ForthicValue::Float(f) => f as i64,
            _ => return Ok(()),
        };
        let ForthicValue::String(forthic) = forthic else {
            return Ok(());
        };
        if forthic.is_empty() {
            return Ok(());
        }
        for _ in 0..n.max(0) {
            context.run(&forthic)?;
        }
        Ok(())
    }

    /// ZIP-WITH: ( c1 c2 forthic -- result ) — combine element-wise; the
    /// code receives ( v1 v2 -- combined ). Array mode iterates c1 (c2
    /// shorter pads NULL, longer truncates); record mode (c2 is a record)
    /// iterates c1's keys, missing c2 entries are NULL.
    fn word_zip_with(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = Self::pop_code(context, "ZIP-WITH")?;
        let c2 = context.stack_pop()?;
        let c1 = context.stack_pop()?;

        let result = match (&c1, &c2) {
            (ForthicValue::Array(a1), ForthicValue::Array(a2)) => {
                let mut out = Vec::with_capacity(a1.len());
                for (i, v1) in a1.iter().enumerate() {
                    context.stack_push(v1.clone());
                    context.stack_push(a2.get(i).cloned().unwrap_or(ForthicValue::Null));
                    context.run(&forthic)?;
                    out.push(context.stack_pop()?);
                }
                ForthicValue::Array(out)
            }
            (ForthicValue::Record(r1), ForthicValue::Record(r2)) => {
                let mut out = IndexMap::new();
                for (k, v1) in r1 {
                    context.stack_push(v1.clone());
                    context.stack_push(r2.get(k).cloned().unwrap_or(ForthicValue::Null));
                    context.run(&forthic)?;
                    out.insert(k.clone(), context.stack_pop()?);
                }
                ForthicValue::Record(out)
            }
            _ => ForthicValue::Array(vec![]),
        };
        context.stack_push(result);
        Ok(())
    }

    /// MAP-AT: ( container key|path forthic -- container ) — transform the
    /// value at a key (or at a path, given an array) — jq's `|=`. A missing
    /// key, out-of-range index, or scalar mid-path returns the container
    /// UNCHANGED, silently. Persistent update: clones along the touched
    /// path only. Empty path transforms the whole container.
    fn word_map_at(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = Self::pop_code(context, "MAP-AT")?;
        let key = context.stack_pop()?;
        let container = context.stack_pop()?;

        if matches!(container, ForthicValue::Null) {
            context.stack_push(container);
            return Ok(());
        }

        let path: Vec<ForthicValue> = match key {
            ForthicValue::Array(path) => path,
            single => vec![single],
        };

        if path.is_empty() {
            let result = Self::run_on_item(context, None, container, &forthic)?;
            context.stack_push(result);
            return Ok(());
        }

        let result = Self::map_at_path(context, &container, &path, &forthic)?;
        context.stack_push(result);
        Ok(())
    }

    fn map_at_path(
        context: &mut dyn InterpreterContext,
        container: &ForthicValue,
        path: &[ForthicValue],
        forthic: &str,
    ) -> Result<ForthicValue, ForthicError> {
        let (head, rest) = path.split_first().expect("non-empty path");
        match container {
            ForthicValue::Array(arr) => {
                // Numeric strings coerce to indices (ts Number(head))
                let idx = match head {
                    ForthicValue::Int(i) => Some(*i),
                    ForthicValue::Float(f) if f.fract() == 0.0 => Some(*f as i64),
                    ForthicValue::String(s) => s.parse::<i64>().ok(),
                    _ => None,
                };
                let Some(idx) = idx else {
                    return Ok(container.clone());
                };
                if idx < 0 || idx as usize >= arr.len() {
                    return Ok(container.clone());
                }
                let idx = idx as usize;
                let new_value = if rest.is_empty() {
                    Self::run_on_item(context, None, arr[idx].clone(), forthic)?
                } else {
                    Self::map_at_path(context, &arr[idx], rest, forthic)?
                };
                let mut out = arr.clone();
                out[idx] = new_value;
                Ok(ForthicValue::Array(out))
            }
            ForthicValue::Record(rec) => {
                let key = Self::value_to_key_string(head)?;
                let Some(current) = rec.get(&key) else {
                    return Ok(container.clone());
                };
                let new_value = if rest.is_empty() {
                    Self::run_on_item(context, None, current.clone(), forthic)?
                } else {
                    Self::map_at_path(context, current, rest, forthic)?
                };
                let mut out = rec.clone();
                out.insert(key, new_value);
                Ok(ForthicValue::Record(out))
            }
            // Scalar mid-path: unchanged, silently
            other => Ok(other.clone()),
        }
    }

    /// SORT-U: ( items -- items ) — bash `sort -u`: natural_cmp sort, then
    /// structural dedupe keeping the first occurrence in sorted order
    fn word_sort_u(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;
        let ForthicValue::Array(arr) = container else {
            context.stack_push(container);
            return Ok(());
        };
        let mut items = arr;
        items.sort_by(Self::natural_cmp);
        let mut seen: Vec<ForthicValue> = Vec::new();
        let mut out = Vec::new();
        for item in items {
            if !seen
                .iter()
                .any(|s| crate::modules::standard::boolean::BooleanModule::values_equal(s, &item))
            {
                seen.push(item.clone());
                out.push(item);
            }
        }
        context.stack_push(ForthicValue::Array(out));
        Ok(())
    }

    /// GROUP-BY: ( items forthic [options] -- grouped ) — record of
    /// key -> array of items; group keys are the code's popped results,
    /// coerced to key strings; group order is first-encounter order
    fn word_group_by(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let options = Self::pop_options(context);
        let forthic = Self::pop_code(context, "GROUP-BY")?;
        let container = context.stack_pop()?;
        let with_key = options
            .as_ref()
            .and_then(|o| o.get_bool("with_key"))
            .unwrap_or(false);

        let mut groups: IndexMap<String, Vec<ForthicValue>> = IndexMap::new();
        for (key, item) in Self::keyed_items(&container) {
            let pushed_key = with_key.then_some(key);
            let group_key_val = Self::run_on_item(context, pushed_key, item.clone(), &forthic)?;
            let group_key = Self::value_to_key_string(&group_key_val)?;
            groups.entry(group_key).or_default().push(item);
        }
        let result: IndexMap<String, ForthicValue> = groups
            .into_iter()
            .map(|(k, items)| (k, ForthicValue::Array(items)))
            .collect();
        context.stack_push(ForthicValue::Record(result));
        Ok(())
    }

    /// Field access for the -FIELD group words: a NULL element errors
    /// (faithful to ts, where null[field] throws); a missing field is NULL
    fn field_of(
        item: &ForthicValue,
        field: &str,
        word: &str,
    ) -> Result<ForthicValue, ForthicError> {
        match item {
            ForthicValue::Record(rec) => Ok(rec.get(field).cloned().unwrap_or(ForthicValue::Null)),
            ForthicValue::Null => Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: format!("{word}: cannot read field '{field}' of NULL"),
                location: None,
                cause: None,
            }),
            _ => Ok(ForthicValue::Null),
        }
    }

    /// GROUP-BY-FIELD: ( container field -- grouped ) — group elements by a
    /// field's value; an ARRAY field value puts the element in EVERY named
    /// group (multi-membership). Missing fields group under "null".
    fn word_group_by_field(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let field = match context.stack_pop()? {
            ForthicValue::String(s) => s,
            other => {
                return Err(ForthicError::InvalidOperation {
                    forthic: String::new(),
                    message: format!("GROUP-BY-FIELD requires a string field name, got {other:?}"),
                    location: None,
                    cause: None,
                })
            }
        };
        let container = context.stack_pop()?;

        let mut groups: IndexMap<String, Vec<ForthicValue>> = IndexMap::new();
        for (_, item) in Self::keyed_items(&container) {
            let fv = Self::field_of(&item, &field, "GROUP-BY-FIELD")?;
            match fv {
                ForthicValue::Array(keys) => {
                    for key_val in keys {
                        let key = Self::value_to_key_string(&key_val)?;
                        groups.entry(key).or_default().push(item.clone());
                    }
                }
                other => {
                    let key = Self::value_to_key_string(&other)?;
                    groups.entry(key).or_default().push(item);
                }
            }
        }
        let result: IndexMap<String, ForthicValue> = groups
            .into_iter()
            .map(|(k, items)| (k, ForthicValue::Array(items)))
            .collect();
        context.stack_push(ForthicValue::Record(result));
        Ok(())
    }

    /// BY-FIELD: ( container field -- indexed ) — record of field value ->
    /// element (LAST wins on duplicates); falsy elements are skipped
    fn word_by_field(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let field = match context.stack_pop()? {
            ForthicValue::String(s) => s,
            other => {
                return Err(ForthicError::InvalidOperation {
                    forthic: String::new(),
                    message: format!("BY-FIELD requires a string field name, got {other:?}"),
                    location: None,
                    cause: None,
                })
            }
        };
        let container = context.stack_pop()?;

        let mut result: IndexMap<String, ForthicValue> = IndexMap::new();
        for (_, item) in Self::keyed_items(&container) {
            if !item.is_truthy() {
                continue;
            }
            let fv = Self::field_of(&item, &field, "BY-FIELD")?;
            let key = Self::value_to_key_string(&fv)?;
            result.insert(key, item);
        }
        context.stack_push(ForthicValue::Record(result));
        Ok(())
    }

    /// GROUPS-OF: ( container n -- groups ) — chunk into groups of n (last
    /// may be short). Records chunk their entries into sub-records. n <= 0
    /// errors (checked before the NULL default); fractional n truncates.
    fn word_groups_of(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let n_val = context.stack_pop()?;
        let n = match n_val {
            ForthicValue::Int(i) => i,
            ForthicValue::Float(f) => f as i64,
            _ => 0,
        };
        if n <= 0 {
            return Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: "GROUPS-OF requires group size > 0".to_string(),
                location: None,
                cause: None,
            });
        }
        let container = context.stack_pop()?;
        let n = n as usize;

        let result = match container {
            ForthicValue::Array(arr) => ForthicValue::Array(
                arr.chunks(n)
                    .map(|chunk| ForthicValue::Array(chunk.to_vec()))
                    .collect(),
            ),
            ForthicValue::Record(rec) => {
                let entries: Vec<_> = rec.into_iter().collect();
                ForthicValue::Array(
                    entries
                        .chunks(n)
                        .map(|chunk| ForthicValue::Record(chunk.iter().cloned().collect()))
                        .collect(),
                )
            }
            _ => ForthicValue::Array(vec![]),
        };
        context.stack_push(result);
        Ok(())
    }

    /// INDEX: ( items forthic -- indexed ) — the code returns an array of
    /// string keys per item; the item lands in every named bucket, with
    /// keys LOWERCASED. Arrays only (records yield an empty record).
    fn word_index(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = Self::pop_code(context, "INDEX")?;
        let container = context.stack_pop()?;

        let ForthicValue::Array(arr) = &container else {
            context.stack_push(match container {
                ForthicValue::Record(_) => ForthicValue::Record(IndexMap::new()),
                other => other,
            });
            return Ok(());
        };

        let mut buckets: IndexMap<String, Vec<ForthicValue>> = IndexMap::new();
        for item in arr {
            let keys = Self::run_on_item(context, None, item.clone(), &forthic)?;
            let ForthicValue::Array(keys) = keys else {
                continue;
            };
            for key_val in keys {
                let ForthicValue::String(key) = key_val else {
                    return Err(ForthicError::InvalidOperation {
                        forthic: String::new(),
                        message: format!("INDEX keys must be strings, got {key_val:?}"),
                        location: None,
                        cause: None,
                    });
                };
                buckets
                    .entry(key.to_lowercase())
                    .or_default()
                    .push(item.clone());
            }
        }
        let result: IndexMap<String, ForthicValue> = buckets
            .into_iter()
            .map(|(k, items)| (k, ForthicValue::Array(items)))
            .collect();
        context.stack_push(ForthicValue::Record(result));
        Ok(())
    }

    /// KEY-OF: ( container value -- key|NULL ) — first index (arrays) or
    /// key (records, insertion order) whose element equals the value.
    /// Structural equality (values_equal) — a sanctioned deviation from
    /// ts's ===, which can never match distinct-but-equal records anyway.
    fn word_key_of(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let value = context.stack_pop()?;
        let container = context.stack_pop()?;

        let result = match &container {
            ForthicValue::Array(arr) => arr
                .iter()
                .position(|v| {
                    crate::modules::standard::boolean::BooleanModule::values_equal(v, &value)
                })
                .map(|i| ForthicValue::Int(i as i64))
                .unwrap_or(ForthicValue::Null),
            ForthicValue::Record(rec) => rec
                .iter()
                .find(|(_, v)| {
                    crate::modules::standard::boolean::BooleanModule::values_equal(v, &value)
                })
                .map(|(k, _)| ForthicValue::String(k.clone()))
                .unwrap_or(ForthicValue::Null),
            _ => ForthicValue::Null,
        };
        context.stack_push(result);
        Ok(())
    }

    /// NUMBERED: ( items -- pairs ) — enumerate: [[0 v0] [1 v1] ...].
    /// Non-arrays (including records) yield an EMPTY array.
    fn word_numbered(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;
        let result = match container {
            ForthicValue::Array(arr) => ForthicValue::Array(
                arr.into_iter()
                    .enumerate()
                    .map(|(i, v)| ForthicValue::Array(vec![ForthicValue::Int(i as i64), v]))
                    .collect(),
            ),
            _ => ForthicValue::Array(vec![]),
        };
        context.stack_push(result);
        Ok(())
    }

    /// Pop a required Forthic-string code argument
    fn pop_code(context: &mut dyn InterpreterContext, word: &str) -> Result<String, ForthicError> {
        match context.stack_pop()? {
            ForthicValue::String(s) => Ok(s),
            other => Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: format!("{word} requires a Forthic string, got {other:?}"),
                location: None,
                cause: None,
            }),
        }
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
