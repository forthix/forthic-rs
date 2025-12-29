// String module for Forthic
//
// String manipulation and processing operations with URL encoding support.
//
// ## Categories
// - Conversion: >STR, URL-ENCODE, URL-DECODE
// - Transform: LOWERCASE, UPPERCASE, STRIP, ASCII
// - Split/Join: SPLIT, JOIN, CONCAT
// - Pattern: REPLACE
// - Constants: /N, /R, /T

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{InterpreterContext, Module, ModuleWord};
use std::sync::Arc;

/// StringModule provides string manipulation operations
pub struct StringModule {
    module: Module,
}

impl StringModule {
    /// Create a new StringModule
    pub fn new() -> Self {
        let mut module = Module::new("string".to_string());

        // Register all words
        Self::register_conversion_words(&mut module);
        Self::register_transform_words(&mut module);
        Self::register_split_join_words(&mut module);
        Self::register_pattern_words(&mut module);
        Self::register_constant_words(&mut module);

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

    // ===== Conversion Operations =====

    fn register_conversion_words(module: &mut Module) {
        // >STR
        let word = Arc::new(ModuleWord::new(">STR".to_string(), Self::word_to_str));
        module.add_exportable_word(word);

        // URL-ENCODE
        let word = Arc::new(ModuleWord::new("URL-ENCODE".to_string(), Self::word_url_encode));
        module.add_exportable_word(word);

        // URL-DECODE
        let word = Arc::new(ModuleWord::new("URL-DECODE".to_string(), Self::word_url_decode));
        module.add_exportable_word(word);
    }

    fn word_to_str(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => s,
            ForthicValue::Int(i) => i.to_string(),
            ForthicValue::Float(f) => f.to_string(),
            ForthicValue::Bool(b) => b.to_string(),
            ForthicValue::Null => "null".to_string(),
            _ => format!("{:?}", val),
        };

        context.stack_push(ForthicValue::String(result));
        Ok(())
    }

    fn word_url_encode(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => {
                let encoded = urlencoding::encode(&s).to_string();
                ForthicValue::String(encoded)
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_url_decode(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => {
                let decoded = urlencoding::decode(&s).unwrap_or_default().to_string();
                ForthicValue::String(decoded)
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Transform Operations =====

    fn register_transform_words(module: &mut Module) {
        // LOWERCASE
        let word = Arc::new(ModuleWord::new("LOWERCASE".to_string(), Self::word_lowercase));
        module.add_exportable_word(word);

        // UPPERCASE
        let word = Arc::new(ModuleWord::new("UPPERCASE".to_string(), Self::word_uppercase));
        module.add_exportable_word(word);

        // STRIP
        let word = Arc::new(ModuleWord::new("STRIP".to_string(), Self::word_strip));
        module.add_exportable_word(word);

        // ASCII
        let word = Arc::new(ModuleWord::new("ASCII".to_string(), Self::word_ascii));
        module.add_exportable_word(word);
    }

    fn word_lowercase(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => ForthicValue::String(s.to_lowercase()),
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_uppercase(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => ForthicValue::String(s.to_uppercase()),
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_strip(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => ForthicValue::String(s.trim().to_string()),
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_ascii(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => {
                let ascii: String = s.chars().filter(|c| (*c as u32) < 256).collect();
                ForthicValue::String(ascii)
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Split/Join Operations =====

    fn register_split_join_words(module: &mut Module) {
        // SPLIT
        let word = Arc::new(ModuleWord::new("SPLIT".to_string(), Self::word_split));
        module.add_exportable_word(word);

        // JOIN
        let word = Arc::new(ModuleWord::new("JOIN".to_string(), Self::word_join));
        module.add_exportable_word(word);

        // CONCAT
        let word = Arc::new(ModuleWord::new("CONCAT".to_string(), Self::word_concat));
        module.add_exportable_word(word);
    }

    fn word_split(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let sep = context.stack_pop()?;
        let string = context.stack_pop()?;

        let result = match (string, sep) {
            (ForthicValue::String(s), ForthicValue::String(sep_str)) => {
                let parts: Vec<_> = s
                    .split(&sep_str as &str)
                    .map(|p| ForthicValue::String(p.to_string()))
                    .collect();
                ForthicValue::Array(parts)
            }
            _ => ForthicValue::Array(vec![]),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_join(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let sep = context.stack_pop()?;
        let strings = context.stack_pop()?;

        let result = match (strings, sep) {
            (ForthicValue::Array(arr), ForthicValue::String(sep_str)) => {
                let parts: Vec<String> = arr
                    .iter()
                    .filter_map(|v| match v {
                        ForthicValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect();
                ForthicValue::String(parts.join(&sep_str))
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_concat(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val2 = context.stack_pop()?;

        let result = match val2 {
            ForthicValue::Array(arr) => {
                // Concatenate array of strings
                let parts: Vec<String> = arr
                    .iter()
                    .filter_map(|v| match v {
                        ForthicValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect();
                ForthicValue::String(parts.join(""))
            }
            ForthicValue::String(s2) => {
                // Concatenate two strings
                let val1 = context.stack_pop()?;
                match val1 {
                    ForthicValue::String(s1) => ForthicValue::String(format!("{}{}", s1, s2)),
                    _ => ForthicValue::String(s2),
                }
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Pattern Operations =====

    fn register_pattern_words(module: &mut Module) {
        // REPLACE
        let word = Arc::new(ModuleWord::new("REPLACE".to_string(), Self::word_replace));
        module.add_exportable_word(word);
    }

    fn word_replace(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let replace = context.stack_pop()?;
        let text = context.stack_pop()?;
        let string = context.stack_pop()?;

        let result = match (string, text, replace) {
            (ForthicValue::String(s), ForthicValue::String(t), ForthicValue::String(r)) => {
                ForthicValue::String(s.replace(&t as &str, &r as &str))
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Constant Words =====

    fn register_constant_words(module: &mut Module) {
        // /N - newline
        let word = Arc::new(ModuleWord::new("/N".to_string(), Self::word_newline));
        module.add_exportable_word(word);

        // /R - carriage return
        let word = Arc::new(ModuleWord::new("/R".to_string(), Self::word_carriage_return));
        module.add_exportable_word(word);

        // /T - tab
        let word = Arc::new(ModuleWord::new("/T".to_string(), Self::word_tab));
        module.add_exportable_word(word);
    }

    fn word_newline(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.stack_push(ForthicValue::String("\n".to_string()));
        Ok(())
    }

    fn word_carriage_return(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.stack_push(ForthicValue::String("\r".to_string()));
        Ok(())
    }

    fn word_tab(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.stack_push(ForthicValue::String("\t".to_string()));
        Ok(())
    }
}

impl Default for StringModule {
    fn default() -> Self {
        Self::new()
    }
}
