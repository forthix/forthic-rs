// Record module for Forthic
//
// Record (object/dictionary) manipulation operations for working with key-value data structures.
//
// ## Categories
// - Core: REC, REC@, <REC!
// - Transform: RELABEL, INVERT-KEYS, REC-DEFAULTS, <DEL
// - Access: KEYS, VALUES

use super::jq_path::{jq_del, jq_get, jq_set, parse_jq_path};
use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{register_words, InterpreterContext, Module};
use indexmap::IndexMap;

/// RecordModule provides record/dictionary operations
pub struct RecordModule {
    module: Module,
}

impl RecordModule {
    /// Create a new RecordModule
    pub fn new() -> Self {
        let mut module = Module::new("record".to_string());

        // Register all words
        Self::register_core_words(&mut module);
        Self::register_jq_words(&mut module);
        Self::register_shaping_words(&mut module);
        Self::register_transform_words(&mut module);
        Self::register_access_words(&mut module);

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

    // ===== Core Operations =====

    // JQ-style path access (paths are DATA, never interpolated source)
    fn register_jq_words(module: &mut Module) {
        register_words!(module, {
            "JQ@" => Self::word_jq_at,
                "( container:any path:any -- value:any )",
                "Get value at jq-style path (e.g., .users[].name). Returns null on miss; [] iterates and flattens. Path arrays accepted for dynamic keys.";
            "JQ!" => Self::word_jq_set,
                "( container:any value:any path:any -- container:any )",
                "Set value at jq-style path. Auto-creates missing intermediates (record for field, array for index). [] iteration not supported.";
            "JQ-DEL" => Self::word_jq_del,
                "( container:any path:any -- container:any )",
                "Delete value at jq-style path. No-op if path doesn't exist. [] iteration not supported.";
        });
    }

    // Reshaping: merge/select/drop keys, entry conversions
    fn register_shaping_words(module: &mut Module) {
        register_words!(module, {
            "MERGE" => Self::word_merge,
                "( rec1:record rec2:record -- merged:record )",
                "Shallow merge two records. Keys present in rec2 override rec1.";
            "PICK" => Self::word_pick,
                "( rec:record keys:any[] -- rec:record )",
                "Return a new record containing only the listed keys (missing keys are skipped)";
            "OMIT" => Self::word_omit,
                "( rec:record keys:any[] -- rec:record )",
                "Return a new record without the listed keys";
            "HAS-KEY?" => Self::word_has_key_q,
                "( rec:record key:any -- bool:boolean )",
                "Returns true if rec has the given key (presence, even when the value is null)";
            "DELETE" => Self::word_delete,
                "( container:any key:any -- container:any )",
                "Delete key from record or index from array (copy-on-write; missing keys are no-ops)";
            "REC>ENTRIES" => Self::word_rec_to_entries,
                "( rec:record -- pairs:any[] )",
                "Convert a record to an array of [key, value] pairs in insertion order. Inverse of ENTRIES>REC.";
            "ENTRIES>REC" => Self::word_entries_to_rec,
                "( pairs:any[] -- rec:record )",
                "Build a record from an array of [key, value] pairs (strict pair validation). Inverse of REC>ENTRIES.";
        });
    }

    /// Coerce a value to a record key string (JS property-key semantics)
    fn key_string(v: &ForthicValue) -> String {
        match v {
            ForthicValue::String(s) => s.clone(),
            ForthicValue::Int(i) => i.to_string(),
            ForthicValue::Float(f) => f.to_string(),
            ForthicValue::Bool(b) => b.to_string(),
            ForthicValue::Null => "null".to_string(),
            other => format!("{other:?}"),
        }
    }

    /// JQ@: ( container path -- value ) — jq-style get; any miss is NULL.
    /// Paths: 'a.b[0]' strings, dynamic [ 'a' 0 ] arrays, and the []
    /// iterate segment (JQ@ only) with conditional flattening. Records
    /// iterate/index in insertion order (ts sorts — documented divergence).
    fn word_jq_at(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let path = context.stack_pop()?;
        let container = context.stack_pop()?;
        let segments = parse_jq_path(&path)?;
        context.stack_push(jq_get(&container, &segments));
        Ok(())
    }

    /// JQ!: ( container value path -- container ) — jq-style set with
    /// auto-created intermediates (kind decided by the NEXT segment). No []
    /// iteration. Empty path replaces the whole container; a NULL/scalar
    /// container is replaced by the kind the first segment needs.
    fn word_jq_set(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let path = context.stack_pop()?;
        let value = context.stack_pop()?;
        let mut container = context.stack_pop()?;
        let segments = parse_jq_path(&path)?;

        if segments.is_empty() {
            context.stack_push(value);
            return Ok(());
        }
        if !matches!(container, ForthicValue::Array(_) | ForthicValue::Record(_)) {
            container = super::jq_path::new_container_for(segments.first());
        }
        jq_set(&mut container, &segments, value)?;
        context.stack_push(container);
        Ok(())
    }

    /// JQ-DEL: ( container path -- container ) — jq-style delete; missing
    /// paths are silent no-ops; no [] iteration; array deletes shift left
    fn word_jq_del(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let path = context.stack_pop()?;
        let mut container = context.stack_pop()?;
        let segments = parse_jq_path(&path)?;
        if !segments.is_empty() && !matches!(container, ForthicValue::Null) {
            jq_del(&mut container, &segments)?;
        }
        context.stack_push(container);
        Ok(())
    }

    /// MERGE: ( rec1 rec2 -- merged ) — shallow; rec2 wins; non-records
    /// coerce to empty. New record; shared keys keep rec1's position.
    fn word_merge(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let rec2 = context.stack_pop()?;
        let rec1 = context.stack_pop()?;
        let mut merged = match rec1 {
            ForthicValue::Record(r) => r,
            _ => IndexMap::new(),
        };
        if let ForthicValue::Record(r2) = rec2 {
            for (k, v) in r2 {
                merged.insert(k, v);
            }
        }
        context.stack_push(ForthicValue::Record(merged));
        Ok(())
    }

    /// PICK: ( rec keys -- rec ) — keep only the named keys; missing keys
    /// silently skipped; output order follows the keys list
    fn word_pick(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let keys = context.stack_pop()?;
        let rec = context.stack_pop()?;
        let mut out = IndexMap::new();
        if let (ForthicValue::Record(rec), ForthicValue::Array(keys)) = (&rec, &keys) {
            for key_val in keys {
                let key = Self::key_string(key_val);
                if let Some(v) = rec.get(&key) {
                    out.insert(key, v.clone());
                }
            }
        }
        context.stack_push(ForthicValue::Record(out));
        Ok(())
    }

    /// OMIT: ( rec keys -- rec ) — drop the named keys, record order
    /// preserved. Drop keys stringify, so [ 1 ] OMIT matches key "1"
    /// (ts's === Set misses that — fixed by design here).
    fn word_omit(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let keys = context.stack_pop()?;
        let rec = context.stack_pop()?;
        let drop: Vec<String> = match &keys {
            ForthicValue::Array(keys) => keys.iter().map(Self::key_string).collect(),
            _ => Vec::new(),
        };
        let mut out = IndexMap::new();
        if let ForthicValue::Record(rec) = &rec {
            for (k, v) in rec {
                if !drop.contains(k) {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
        context.stack_push(ForthicValue::Record(out));
        Ok(())
    }

    /// HAS-KEY?: ( rec key -- bool ) — key PRESENCE, not value-non-null:
    /// a key explicitly set to NULL is present (distinct from REC@ NULL ==)
    fn word_has_key_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let key = context.stack_pop()?;
        let rec = context.stack_pop()?;
        let present =
            matches!(&rec, ForthicValue::Record(rec) if rec.contains_key(&Self::key_string(&key)));
        context.stack_push(ForthicValue::Bool(present));
        Ok(())
    }

    /// DELETE: ( container key -- container ) — copy-on-write delete
    /// (ts #32): the input is never mutated. Arrays require integer keys
    /// (negative wraps once; out-of-range is a no-op); record keys coerce
    /// to strings; missing keys are no-ops. Replaces the classic <DEL,
    /// which mutated in place.
    fn word_delete(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let key = context.stack_pop()?;
        let container = context.stack_pop()?;
        let result = match container {
            ForthicValue::Record(rec) => {
                let mut copy = rec.clone();
                // shift_remove preserves the order of remaining entries
                copy.shift_remove(&Self::key_string(&key));
                ForthicValue::Record(copy)
            }
            ForthicValue::Array(arr) => {
                let idx = match &key {
                    ForthicValue::Int(i) => Some(*i),
                    ForthicValue::Float(f) if f.fract() == 0.0 => Some(*f as i64),
                    _ => {
                        return Err(ForthicError::InvalidOperation {
                            forthic: String::new(),
                            message: format!(
                                "DELETE on an array requires an integer index, got {key:?}"
                            ),
                            location: None,
                            cause: None,
                        })
                    }
                };
                let mut copy = arr.clone();
                if let Some(n) = idx {
                    let norm = if n < 0 { n + copy.len() as i64 } else { n };
                    if norm >= 0 && (norm as usize) < copy.len() {
                        copy.remove(norm as usize);
                    }
                }
                ForthicValue::Array(copy)
            }
            other => other, // NULL/scalar: unchanged
        };
        context.stack_push(result);
        Ok(())
    }

    /// REC>ENTRIES: ( rec -- pairs ) — [k v] pairs in INSERTION order.
    /// ts sorts by key (a stability workaround for JS object-order quirks
    /// that IndexMap doesn't need) — documented divergence; this makes
    /// REC>ENTRIES ENTRIES>REC a true round-trip identity.
    fn word_rec_to_entries(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let rec = context.stack_pop()?;
        let pairs = match rec {
            ForthicValue::Record(rec) => rec
                .into_iter()
                .map(|(k, v)| ForthicValue::Array(vec![ForthicValue::String(k), v]))
                .collect(),
            _ => Vec::new(),
        };
        context.stack_push(ForthicValue::Array(pairs));
        Ok(())
    }

    /// ENTRIES>REC: ( pairs -- rec ) — the inverse of REC>ENTRIES; same
    /// contract as REC (strict [key value] pair validation; later
    /// duplicates win, keeping the first insertion position)
    fn word_entries_to_rec(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let pairs = context.stack_pop()?;
        let rec = Self::build_record(&pairs, "ENTRIES>REC")?;
        context.stack_push(ForthicValue::Record(rec));
        Ok(())
    }

    /// Build a record from [[k v] ...] pairs with strict validation
    /// (shared by REC-family words)
    fn build_record(
        pairs: &ForthicValue,
        word_name: &str,
    ) -> Result<IndexMap<String, ForthicValue>, ForthicError> {
        let mut rec = IndexMap::new();
        let ForthicValue::Array(pairs) = pairs else {
            return Ok(rec); // NULL -> empty record
        };
        for (i, pair) in pairs.iter().enumerate() {
            let ForthicValue::Array(kv) = pair else {
                return Err(ForthicError::InvalidOperation {
                    forthic: String::new(),
                    message: format!(
                        "{word_name} requires each pair to be a [key, value] array; pair at index {i} is {pair:?}"
                    ),
                    location: None,
                    cause: None,
                });
            };
            if kv.len() != 2 {
                return Err(ForthicError::InvalidOperation {
                    forthic: String::new(),
                    message: format!(
                        "{word_name} requires each pair to be a [key, value] array with exactly 2 elements; pair at index {i} has {}",
                        kv.len()
                    ),
                    location: None,
                    cause: None,
                });
            }
            rec.insert(Self::key_string(&kv[0]), kv[1].clone());
        }
        Ok(rec)
    }

    fn register_core_words(module: &mut Module) {
        register_words!(module, {
            "REC" => Self::word_rec,
                "( key_vals:any[] -- rec:record )",
                "Create record from [[key, val], ...] pairs (invalid pairs are skipped)";
            "REC@" => Self::word_rec_at,
                "( rec:record field:any -- value:any )",
                "Get value from record by field or array of fields (null on miss)";
            "<REC!" => Self::word_set_rec,
                "( rec:record value:any field:any -- rec:record )",
                "Set value in record at field, or at a nested path given an array of fields";
        });
    }

    fn word_rec(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let key_vals = context.stack_pop()?;

        let result = match key_vals {
            ForthicValue::Array(pairs) => {
                let mut record = IndexMap::new();

                for pair in pairs {
                    if let ForthicValue::Array(kv) = pair {
                        if kv.len() >= 2 {
                            if let ForthicValue::String(key) = &kv[0] {
                                record.insert(key.clone(), kv[1].clone());
                            }
                        }
                    }
                }

                ForthicValue::Record(record)
            }
            ForthicValue::Null => ForthicValue::Record(IndexMap::new()),
            _ => ForthicValue::Record(IndexMap::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_rec_at(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let field = context.stack_pop()?;
        let rec = context.stack_pop()?;

        let result = match rec {
            ForthicValue::Record(record) => {
                // Handle field as single key or array of keys (nested path)
                match field {
                    ForthicValue::String(key) => {
                        record.get(&key).cloned().unwrap_or(ForthicValue::Null)
                    }
                    ForthicValue::Array(fields) => {
                        Self::drill_for_value(&ForthicValue::Record(record), &fields)
                    }
                    _ => ForthicValue::Null,
                }
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_set_rec(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let field = context.stack_pop()?;
        let value = context.stack_pop()?;
        let rec = context.stack_pop()?;

        let mut record = match rec {
            ForthicValue::Record(r) => r,
            ForthicValue::Null => IndexMap::new(),
            _ => {
                context.stack_push(rec);
                return Ok(());
            }
        };

        // Handle field as single key or array of keys (nested path)
        match field {
            ForthicValue::String(key) => {
                record.insert(key, value);
            }
            ForthicValue::Array(fields) => {
                Self::set_nested_value(&mut record, &fields, value);
            }
            _ => {}
        }

        context.stack_push(ForthicValue::Record(record));
        Ok(())
    }

    /// Drill down into nested record structure
    fn drill_for_value(val: &ForthicValue, fields: &[ForthicValue]) -> ForthicValue {
        let mut current = val.clone();

        for field in fields {
            if let ForthicValue::String(key) = field {
                match current {
                    ForthicValue::Record(ref rec) => {
                        current = rec.get(key).cloned().unwrap_or(ForthicValue::Null);
                    }
                    _ => return ForthicValue::Null,
                }
            } else {
                return ForthicValue::Null;
            }
        }

        current
    }

    /// Set value in nested record structure
    fn set_nested_value(
        record: &mut IndexMap<String, ForthicValue>,
        fields: &[ForthicValue],
        value: ForthicValue,
    ) {
        if fields.is_empty() {
            return;
        }

        if fields.len() == 1 {
            if let ForthicValue::String(key) = &fields[0] {
                record.insert(key.clone(), value);
            }
            return;
        }

        // Navigate to the correct nested level
        if let ForthicValue::String(key) = &fields[0] {
            let current = record
                .entry(key.clone())
                .or_insert_with(|| ForthicValue::Record(IndexMap::new()));

            if let ForthicValue::Record(ref mut nested) = current {
                Self::set_nested_value(nested, &fields[1..], value);
            }
        }
    }

    // ===== Transform Operations =====

    fn register_transform_words(module: &mut Module) {
        register_words!(module, {
            "RELABEL" => Self::word_relabel,
                "( rec:record old_keys:string[] new_keys:string[] -- rec:record )",
                "Rename record keys: the value at old_keys[i] moves to new_keys[i]; unlisted keys are dropped";
            "INVERT-KEYS" => Self::word_invert_keys,
                "( rec:record -- rec:record )",
                "Invert a two-level record: result[inner_key][outer_key] = value";
        });
    }

    fn word_relabel(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let new_keys_val = context.stack_pop()?;
        let old_keys_val = context.stack_pop()?;
        let container = context.stack_pop()?;

        let (old_keys, new_keys) = match (old_keys_val, new_keys_val) {
            (ForthicValue::Array(old), ForthicValue::Array(new)) => (old, new),
            _ => {
                context.stack_push(container);
                return Ok(());
            }
        };

        if old_keys.len() != new_keys.len() {
            // Just push back the container unchanged if lengths don't match
            context.stack_push(container);
            return Ok(());
        }

        let result = match container {
            ForthicValue::Record(rec) => {
                let mut new_rec = IndexMap::new();

                for i in 0..old_keys.len() {
                    if let (ForthicValue::String(old_key), ForthicValue::String(new_key)) =
                        (&old_keys[i], &new_keys[i])
                    {
                        if let Some(value) = rec.get(old_key) {
                            new_rec.insert(new_key.clone(), value.clone());
                        }
                    }
                }

                ForthicValue::Record(new_rec)
            }
            _ => container,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_invert_keys(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let record = context.stack_pop()?;

        let result = match record {
            ForthicValue::Record(rec) => {
                let mut inverted: IndexMap<String, IndexMap<String, ForthicValue>> =
                    IndexMap::new();

                for (first_key, sub_val) in rec {
                    if let ForthicValue::Record(sub_rec) = sub_val {
                        for (second_key, value) in sub_rec {
                            inverted
                                .entry(second_key)
                                .or_default()
                                .insert(first_key.clone(), value);
                        }
                    }
                }

                let result_rec: IndexMap<String, ForthicValue> = inverted
                    .into_iter()
                    .map(|(k, v)| (k, ForthicValue::Record(v)))
                    .collect();

                ForthicValue::Record(result_rec)
            }
            _ => record,
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Access Operations =====

    fn register_access_words(module: &mut Module) {
        register_words!(module, {
            "KEYS" => Self::word_keys,
                "( container:any -- keys:any[] )",
                "Get keys from record or indices from array";
            "VALUES" => Self::word_values,
                "( container:any -- values:any[] )",
                "Get values from record or elements from array";
        });
    }

    fn word_keys(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;

        let result = match container {
            ForthicValue::Record(rec) => {
                let keys: Vec<_> = rec
                    .keys()
                    .map(|k| ForthicValue::String(k.clone()))
                    .collect();
                ForthicValue::Array(keys)
            }
            ForthicValue::Array(arr) => {
                let indices: Vec<_> = (0..arr.len() as i64).map(ForthicValue::Int).collect();
                ForthicValue::Array(indices)
            }
            ForthicValue::Null => ForthicValue::Array(vec![]),
            _ => ForthicValue::Array(vec![]),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_values(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;

        let result = match container {
            ForthicValue::Record(rec) => {
                let values: Vec<_> = rec.values().cloned().collect();
                ForthicValue::Array(values)
            }
            ForthicValue::Array(arr) => ForthicValue::Array(arr),
            ForthicValue::Null => ForthicValue::Array(vec![]),
            _ => ForthicValue::Array(vec![]),
        };

        context.stack_push(result);
        Ok(())
    }
}

impl Default for RecordModule {
    fn default() -> Self {
        Self::new()
    }
}
