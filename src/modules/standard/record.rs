// Record module for Forthic
//
// Record (object/dictionary) manipulation operations for working with key-value data structures.
//
// ## Categories
// - Core: REC, REC@, <REC!
// - Transform: RELABEL, INVERT-KEYS, REC-DEFAULTS, <DEL
// - Access: KEYS, VALUES

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{InterpreterContext, Module, ModuleWord};
use std::collections::HashMap;
use std::sync::Arc;

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

    fn register_core_words(module: &mut Module) {
        // REC
        let word = Arc::new(ModuleWord::new("REC".to_string(), Self::word_rec));
        module.add_exportable_word(word);

        // REC@
        let word = Arc::new(ModuleWord::new("REC@".to_string(), Self::word_rec_at));
        module.add_exportable_word(word);

        // <REC!
        let word = Arc::new(ModuleWord::new("<REC!".to_string(), Self::word_set_rec));
        module.add_exportable_word(word);
    }

    fn word_rec(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let key_vals = context.stack_pop()?;

        let result = match key_vals {
            ForthicValue::Array(pairs) => {
                let mut record = HashMap::new();

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
            ForthicValue::Null => ForthicValue::Record(HashMap::new()),
            _ => ForthicValue::Record(HashMap::new()),
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
            ForthicValue::Null => HashMap::new(),
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
        record: &mut HashMap<String, ForthicValue>,
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
            let mut current = record
                .entry(key.clone())
                .or_insert_with(|| ForthicValue::Record(HashMap::new()));

            if let ForthicValue::Record(ref mut nested) = current {
                Self::set_nested_value(nested, &fields[1..], value);
            }
        }
    }

    // ===== Transform Operations =====

    fn register_transform_words(module: &mut Module) {
        // RELABEL
        let word = Arc::new(ModuleWord::new("RELABEL".to_string(), Self::word_relabel));
        module.add_exportable_word(word);

        // INVERT-KEYS
        let word = Arc::new(ModuleWord::new("INVERT-KEYS".to_string(), Self::word_invert_keys));
        module.add_exportable_word(word);

        // REC-DEFAULTS
        let word = Arc::new(ModuleWord::new("REC-DEFAULTS".to_string(), Self::word_rec_defaults));
        module.add_exportable_word(word);

        // <DEL
        let word = Arc::new(ModuleWord::new("<DEL".to_string(), Self::word_del));
        module.add_exportable_word(word);
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
                let mut new_rec = HashMap::new();

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
                let mut inverted: HashMap<String, HashMap<String, ForthicValue>> = HashMap::new();

                for (first_key, sub_val) in rec {
                    if let ForthicValue::Record(sub_rec) = sub_val {
                        for (second_key, value) in sub_rec {
                            inverted
                                .entry(second_key)
                                .or_insert_with(HashMap::new)
                                .insert(first_key.clone(), value);
                        }
                    }
                }

                let result_rec: HashMap<String, ForthicValue> = inverted
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

    fn word_rec_defaults(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let key_vals = context.stack_pop()?;
        let record = context.stack_pop()?;

        let mut rec = match record {
            ForthicValue::Record(r) => r,
            _ => {
                context.stack_push(record);
                return Ok(());
            }
        };

        if let ForthicValue::Array(pairs) = key_vals {
            for pair in pairs {
                if let ForthicValue::Array(kv) = pair {
                    if kv.len() >= 2 {
                        if let ForthicValue::String(key) = &kv[0] {
                            let current = rec.get(key);
                            let should_set = match current {
                                None => true,
                                Some(ForthicValue::Null) => true,
                                Some(ForthicValue::String(s)) if s.is_empty() => true,
                                _ => false,
                            };

                            if should_set {
                                rec.insert(key.clone(), kv[1].clone());
                            }
                        }
                    }
                }
            }
        }

        context.stack_push(ForthicValue::Record(rec));
        Ok(())
    }

    fn word_del(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let key = context.stack_pop()?;
        let container = context.stack_pop()?;

        let result = match container {
            ForthicValue::Record(mut rec) => {
                if let ForthicValue::String(k) = key {
                    rec.remove(&k);
                }
                ForthicValue::Record(rec)
            }
            ForthicValue::Array(mut arr) => {
                if let ForthicValue::Int(idx) = key {
                    if idx >= 0 && (idx as usize) < arr.len() {
                        arr.remove(idx as usize);
                    }
                }
                ForthicValue::Array(arr)
            }
            _ => container,
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Access Operations =====

    fn register_access_words(module: &mut Module) {
        // KEYS
        let word = Arc::new(ModuleWord::new("KEYS".to_string(), Self::word_keys));
        module.add_exportable_word(word);

        // VALUES
        let word = Arc::new(ModuleWord::new("VALUES".to_string(), Self::word_values));
        module.add_exportable_word(word);
    }

    fn word_keys(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let container = context.stack_pop()?;

        let result = match container {
            ForthicValue::Record(rec) => {
                let keys: Vec<_> = rec.keys().map(|k| ForthicValue::String(k.clone())).collect();
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
