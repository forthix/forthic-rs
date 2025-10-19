# Forthic-TS to Forthic-RS Porting Sequence

Based on analysis of the TypeScript codebase and Ruby port experience, here's the recommended porting sequence organized by dependency layers for Rust.

## Phase 1: Foundation Layer (No Dependencies)

### 1. src/errors.rs
Error types and exception hierarchy using Rust's error handling
- Custom error types as enums with thiserror or similar
- ForthicError, UnknownWordError, StackUnderflowError, etc.
- CodeLocation struct
- Error description formatting with location highlighting
- Implement std::error::Error trait

**Source:** `src/forthic/errors.ts`, `forthic-rb/lib/forthic/errors.rb`

**Rust Adaptations:**
```rust
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct CodeLocation {
    pub start_pos: usize,
    pub end_pos: usize,
    pub string: String,
}

#[derive(Error, Debug)]
pub enum ForthicError {
    #[error("Unknown word: {word} at {location:?}")]
    UnknownWord {
        word: String,
        location: CodeLocation,
    },

    #[error("Stack underflow at {location:?}")]
    StackUnderflow {
        location: CodeLocation,
    },

    #[error("Invalid type: expected {expected}, got {actual}")]
    InvalidType {
        expected: String,
        actual: String,
        location: CodeLocation,
    },

    // ... other error variants
}

impl ForthicError {
    pub fn format_with_context(&self) -> String {
        // Format error with code location highlighting
        // Similar to TypeScript implementation
    }
}
```

### 2. src/utils.rs
Utility functions
- Date/time helpers using chrono
- String utilities
- Common type conversions

**Source:** `src/forthic/utils.ts`

**Rust Adaptations:**
```rust
use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};

pub fn to_zoned_datetime(s: &str) -> Result<DateTime<Utc>, ForthicError> {
    // Parse ISO8601 datetime strings
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| ForthicError::ParseError { /* ... */ })
}
```

### 3. src/literals.rs
Literal type handlers
- LiteralHandler trait or function type
- Boolean, integer, float parsers
- Date, time, DateTime parsers using chrono
- to_bool, to_float, to_int, to_time, to_literal_date, to_zoned_datetime

**Source:** `src/forthic/literals.ts`

**Rust Adaptations:**
```rust
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

pub type LiteralHandler = fn(&str) -> Option<ForthicValue>;

pub fn to_bool(s: &str) -> Option<ForthicValue> {
    match s.to_lowercase().as_str() {
        "true" => Some(ForthicValue::Bool(true)),
        "false" => Some(ForthicValue::Bool(false)),
        _ => None,
    }
}

pub fn to_int(s: &str) -> Option<ForthicValue> {
    s.parse::<i64>().ok().map(ForthicValue::Int)
}

pub fn to_float(s: &str) -> Option<ForthicValue> {
    s.parse::<f64>().ok().map(ForthicValue::Float)
}

pub fn to_literal_date(s: &str) -> Option<ForthicValue> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .map(ForthicValue::Date)
}

pub fn to_time(s: &str) -> Option<ForthicValue> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .ok()
        .map(ForthicValue::Time)
}

pub fn to_zoned_datetime_value(s: &str) -> Option<ForthicValue> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| ForthicValue::DateTime(dt.with_timezone(&Utc)))
}
```

## Phase 2: Core Infrastructure

### 4. src/tokenizer.rs
Lexical analysis
- Token enum, TokenType enum, CodeLocation struct
- PositionedString struct
- Tokenizer with all state transitions
- String delta tracking for streaming support

**Source:** `src/forthic/tokenizer.ts`

**Rust Adaptations:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    StartArray,
    EndArray,
    StartModule,
    EndModule,
    StartDefinition,
    EndDefinition,
    Comment,
    String,
    Word,
    Eos, // End of string
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub string: String,
    pub location: CodeLocation,
}

pub struct PositionedString {
    string: String,
    position: usize,
}

impl PositionedString {
    pub fn new(s: String) -> Self {
        Self { string: s, position: 0 }
    }

    pub fn peek(&self) -> Option<char> {
        self.string.chars().nth(self.position)
    }

    pub fn consume(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.position += ch.len_utf8();
        Some(ch)
    }

    pub fn position(&self) -> usize {
        self.position
    }
}

pub struct Tokenizer {
    positioned_string: PositionedString,
    state: TokenizerState,
    // ... other fields
}

#[derive(Debug, Clone, PartialEq)]
enum TokenizerState {
    FindNext,
    InWord,
    InString,
    InTripleQuotedString,
    InComment,
    // ... other states
}

impl Tokenizer {
    pub fn new(code: String) -> Self {
        Self {
            positioned_string: PositionedString::new(code),
            state: TokenizerState::FindNext,
        }
    }

    pub fn next_token(&mut self) -> Result<Option<Token>, ForthicError> {
        // State machine for tokenization
        // Returns Ok(None) when end of string reached
    }
}
```

### 5. src/word_options.rs
Options system for flexible word parameters
- WordOptions struct
- pop_options_if_present helper
- Support for [.key value] syntax

**Source:** `src/forthic/word_options.ts`

**Rust Adaptations:**
```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct WordOptions {
    options: HashMap<String, ForthicValue>,
}

impl WordOptions {
    pub fn new() -> Self {
        Self {
            options: HashMap::new(),
        }
    }

    pub fn from_array(arr: &[ForthicValue]) -> Result<Self, ForthicError> {
        let mut options = HashMap::new();
        let mut i = 0;

        while i < arr.len() {
            if let ForthicValue::String(key) = &arr[i] {
                if key.starts_with('.') {
                    let key_name = key[1..].to_string();
                    if i + 1 < arr.len() {
                        options.insert(key_name, arr[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(ForthicError::InvalidOptions { /* ... */ });
                    }
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        Ok(Self { options })
    }

    pub fn get(&self, key: &str) -> Option<&ForthicValue> {
        self.options.get(key)
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_string())
    }

    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_int())
    }
}

pub fn pop_options_if_present(stack: &mut Vec<ForthicValue>) -> Option<WordOptions> {
    if let Some(ForthicValue::Array(arr)) = stack.last() {
        if arr.iter().any(|v| matches!(v, ForthicValue::String(s) if s.starts_with('.'))) {
            if let Some(ForthicValue::Array(arr)) = stack.pop() {
                return WordOptions::from_array(&arr).ok();
            }
        }
    }
    None
}
```

## Phase 3: Module System

### 6. src/module.rs
Module and Word traits/structs
- Variable struct
- Word trait (polymorphic behavior)
- Word implementations:
  - PushValueWord
  - DefinitionWord
  - ModuleMemoWord
  - ModuleMemoBangWord
  - ModuleMemoBangAtWord
  - ExecuteWord
- Module struct with word/variable/module management
- Module import system with prefixes

**Source:** `src/forthic/module.ts`

**Rust Adaptations:**
```rust
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

// Core value type
#[derive(Debug, Clone)]
pub enum ForthicValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<ForthicValue>),
    Record(HashMap<String, ForthicValue>),
    Date(chrono::NaiveDate),
    Time(chrono::NaiveTime),
    DateTime(chrono::DateTime<chrono::Utc>),
}

impl ForthicValue {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ForthicValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            ForthicValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    // ... other type accessor methods
}

// Variable storage
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub value: ForthicValue,
}

// Word trait for polymorphic word execution
#[async_trait]
pub trait Word: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, interp: &mut Interpreter) -> Result<(), ForthicError>;
    fn is_memo(&self) -> bool { false }
}

// Push a literal value
pub struct PushValueWord {
    name: String,
    value: ForthicValue,
}

#[async_trait]
impl Word for PushValueWord {
    fn name(&self) -> &str { &self.name }

    async fn execute(&self, interp: &mut Interpreter) -> Result<(), ForthicError> {
        interp.stack_push(self.value.clone());
        Ok(())
    }
}

// Execute a definition
pub struct DefinitionWord {
    name: String,
    definition: String,
}

#[async_trait]
impl Word for DefinitionWord {
    fn name(&self) -> &str { &self.name }

    async fn execute(&self, interp: &mut Interpreter) -> Result<(), ForthicError> {
        interp.run(&self.definition).await
    }
}

// Module with memoization
pub struct ModuleMemoWord {
    name: String,
    module: Arc<dyn Module>,
    word_name: String,
    cache: std::sync::Mutex<Option<ForthicValue>>,
}

#[async_trait]
impl Word for ModuleMemoWord {
    fn name(&self) -> &str { &self.name }

    fn is_memo(&self) -> bool { true }

    async fn execute(&self, interp: &mut Interpreter) -> Result<(), ForthicError> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(cached) = &*cache {
            interp.stack_push(cached.clone());
            return Ok(());
        }

        // Execute word and cache result
        self.module.find_word(&self.word_name)?.execute(interp).await?;
        let result = interp.stack_pop()?;
        *cache = Some(result.clone());
        interp.stack_push(result);
        Ok(())
    }
}

// Module trait
pub trait Module: Send + Sync {
    fn name(&self) -> &str;
    fn find_word(&self, name: &str) -> Result<Arc<dyn Word>, ForthicError>;
    fn add_word(&mut self, word: Arc<dyn Word>);
    fn add_variable(&mut self, name: String, value: ForthicValue);
    fn get_variable(&self, name: &str) -> Option<&ForthicValue>;
    fn set_variable(&mut self, name: &str, value: ForthicValue) -> Result<(), ForthicError>;
    fn export_word(&mut self, name: &str);
    fn is_exported(&self, name: &str) -> bool;
}

// Basic module implementation
pub struct BasicModule {
    name: String,
    words: HashMap<String, Arc<dyn Word>>,
    variables: HashMap<String, Variable>,
    exported_words: Vec<String>,
    imported_modules: HashMap<String, (Arc<dyn Module>, Option<String>)>, // (module, prefix)
}

impl BasicModule {
    pub fn new(name: String) -> Self {
        Self {
            name,
            words: HashMap::new(),
            variables: HashMap::new(),
            exported_words: Vec::new(),
            imported_modules: HashMap::new(),
        }
    }

    pub fn import_module(&mut self, module: Arc<dyn Module>, prefix: Option<String>) {
        self.imported_modules.insert(module.name().to_string(), (module, prefix));
    }
}

impl Module for BasicModule {
    fn name(&self) -> &str { &self.name }

    fn find_word(&self, name: &str) -> Result<Arc<dyn Word>, ForthicError> {
        // Check local words first
        if let Some(word) = self.words.get(name) {
            return Ok(Arc::clone(word));
        }

        // Check imported modules
        for (module, prefix) in self.imported_modules.values() {
            let search_name = if let Some(p) = prefix {
                if let Some(stripped) = name.strip_prefix(&format!("{}.", p)) {
                    stripped
                } else {
                    continue;
                }
            } else {
                name
            };

            if let Ok(word) = module.find_word(search_name) {
                if module.is_exported(search_name) {
                    return Ok(word);
                }
            }
        }

        Err(ForthicError::UnknownWord {
            word: name.to_string(),
            location: CodeLocation::default(),
        })
    }

    fn add_word(&mut self, word: Arc<dyn Word>) {
        self.words.insert(word.name().to_string(), word);
    }

    fn add_variable(&mut self, name: String, value: ForthicValue) {
        self.variables.insert(name.clone(), Variable { name, value });
    }

    fn get_variable(&self, name: &str) -> Option<&ForthicValue> {
        self.variables.get(name).map(|v| &v.value)
    }

    fn set_variable(&mut self, name: &str, value: ForthicValue) -> Result<(), ForthicError> {
        if let Some(var) = self.variables.get_mut(name) {
            var.value = value;
            Ok(())
        } else {
            Err(ForthicError::UnknownVariable {
                name: name.to_string(),
            })
        }
    }

    fn export_word(&mut self, name: &str) {
        if !self.exported_words.contains(&name.to_string()) {
            self.exported_words.push(name.to_string());
        }
    }

    fn is_exported(&self, name: &str) -> bool {
        self.exported_words.contains(&name.to_string())
    }
}
```

## Phase 4: Interpreter Core

### 7. src/interpreter.rs
Main execution engine
- Stack struct (wrapper around Vec)
- Interpreter struct:
  - Tokenizer integration
  - Stack management
  - Module stack
  - Literal handler registration
  - Token handling (handle_token, handle_word_token, etc.)
  - Profiling support
  - Streaming execution
  - Error recovery
- StandardInterpreter (with stdlib)
- dup_interpreter function

**Source:** `src/forthic/interpreter.ts`

**Rust Adaptations:**
```rust
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Stack {
    items: Vec<ForthicValue>,
}

impl Stack {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, value: ForthicValue) {
        self.items.push(value);
    }

    pub fn pop(&mut self) -> Result<ForthicValue, ForthicError> {
        self.items.pop().ok_or(ForthicError::StackUnderflow {
            location: CodeLocation::default(),
        })
    }

    pub fn peek(&self) -> Option<&ForthicValue> {
        self.items.last()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    // Array-like access
    pub fn get(&self, index: usize) -> Option<&ForthicValue> {
        self.items.get(index)
    }
}

pub struct Interpreter {
    stack: Stack,
    module_stack: Vec<Arc<Mutex<dyn Module>>>,
    literal_handlers: Vec<LiteralHandler>,
    timezone: String,
    profiling_data: Option<Vec<ProfilingRecord>>,
}

impl Interpreter {
    pub fn new(modules: Vec<Arc<Mutex<dyn Module>>>, timezone: &str) -> Self {
        let mut interp = Self {
            stack: Stack::new(),
            module_stack: modules,
            literal_handlers: Vec::new(),
            timezone: timezone.to_string(),
            profiling_data: None,
        };

        // Register default literal handlers
        interp.register_literal_handler(to_bool);
        interp.register_literal_handler(to_int);
        interp.register_literal_handler(to_float);
        interp.register_literal_handler(to_literal_date);
        interp.register_literal_handler(to_time);
        interp.register_literal_handler(to_zoned_datetime_value);

        interp
    }

    pub fn stack_push(&mut self, value: ForthicValue) {
        self.stack.push(value);
    }

    pub fn stack_pop(&mut self) -> Result<ForthicValue, ForthicError> {
        self.stack.pop()
    }

    pub fn register_literal_handler(&mut self, handler: LiteralHandler) {
        self.literal_handlers.push(handler);
    }

    pub async fn run(&mut self, code: &str) -> Result<(), ForthicError> {
        let mut tokenizer = Tokenizer::new(code.to_string());

        while let Some(token) = tokenizer.next_token()? {
            self.handle_token(token).await?;
        }

        Ok(())
    }

    async fn handle_token(&mut self, token: Token) -> Result<(), ForthicError> {
        match token.token_type {
            TokenType::Word => self.handle_word_token(token).await?,
            TokenType::String => {
                self.stack_push(ForthicValue::String(token.string));
            },
            TokenType::StartArray => self.handle_start_array().await?,
            TokenType::EndArray => self.handle_end_array()?,
            TokenType::StartModule => self.handle_start_module(),
            TokenType::EndModule => self.handle_end_module()?,
            TokenType::StartDefinition => self.handle_start_definition(),
            TokenType::EndDefinition => self.handle_end_definition()?,
            TokenType::Comment => {}, // Ignore comments
            TokenType::Eos => {},
        }
        Ok(())
    }

    async fn handle_word_token(&mut self, token: Token) -> Result<(), ForthicError> {
        let word_str = &token.string;

        // Try literal handlers first
        for handler in &self.literal_handlers {
            if let Some(value) = handler(word_str) {
                self.stack_push(value);
                return Ok(());
            }
        }

        // Find word in module stack (from top to bottom)
        for module in self.module_stack.iter().rev() {
            let module = module.lock().await;
            if let Ok(word) = module.find_word(word_str) {
                drop(module); // Release lock before executing
                return word.execute(self).await;
            }
        }

        Err(ForthicError::UnknownWord {
            word: word_str.to_string(),
            location: token.location,
        })
    }

    async fn handle_start_array(&mut self) -> Result<(), ForthicError> {
        // Push marker for array start
        self.stack_push(ForthicValue::ArrayMarker);
        Ok(())
    }

    fn handle_end_array(&mut self) -> Result<(), ForthicError> {
        // Collect items until marker
        let mut items = Vec::new();
        loop {
            let value = self.stack_pop()?;
            if matches!(value, ForthicValue::ArrayMarker) {
                break;
            }
            items.push(value);
        }
        items.reverse();
        self.stack_push(ForthicValue::Array(items));
        Ok(())
    }

    // ... other token handlers
}

// StandardInterpreter with all stdlib modules
pub struct StandardInterpreter {
    interpreter: Interpreter,
}

impl StandardInterpreter {
    pub fn new(additional_modules: Vec<Arc<Mutex<dyn Module>>>, timezone: &str) -> Self {
        let mut modules = vec![
            Arc::new(Mutex::new(CoreModule::new())) as Arc<Mutex<dyn Module>>,
            Arc::new(Mutex::new(ArrayModule::new())),
            Arc::new(Mutex::new(RecordModule::new())),
            Arc::new(Mutex::new(StringModule::new())),
            Arc::new(Mutex::new(MathModule::new())),
            Arc::new(Mutex::new(BooleanModule::new())),
            Arc::new(Mutex::new(JsonModule::new())),
            Arc::new(Mutex::new(DateTimeModule::new())),
        ];
        modules.extend(additional_modules);

        Self {
            interpreter: Interpreter::new(modules, timezone),
        }
    }

    pub async fn run(&mut self, code: &str) -> Result<(), ForthicError> {
        self.interpreter.run(code).await
    }

    pub fn stack_pop(&mut self) -> Result<ForthicValue, ForthicError> {
        self.interpreter.stack_pop()
    }

    pub fn stack_push(&mut self, value: ForthicValue) {
        self.interpreter.stack_push(value);
    }
}
```

## Phase 5: Procedural Macro System (Rust-specific)

### 8. forthic-macros/src/lib.rs (separate crate)
Procedural macros for word registration
- #[forthic_word] macro (replaces @Word decorator)
- #[forthic_module] macro
- Metadata extraction and registration
- Stack effect parsing from doc comments

**Rust Implementation:**
```rust
// forthic-macros/Cargo.toml
[package]
name = "forthic-macros"
version = "0.1.0"
edition = "2021"

[lib]
proc-macro = true

[dependencies]
syn = "2.0"
quote = "1.0"
proc-macro2 = "1.0"

// forthic-macros/src/lib.rs
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ItemImpl};

#[proc_macro_attribute]
pub fn forthic_word(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let signature = parse_macro_input!(attr as syn::LitStr).value();

    let fn_name = &input.sig.ident;
    let fn_name_upper = fn_name.to_string().to_uppercase();

    // Extract doc comment for word documentation
    let doc = extract_doc_comment(&input.attrs);

    let expanded = quote! {
        #input

        // Auto-register this word
        inventory::submit! {
            WordRegistration {
                name: #fn_name_upper,
                signature: #signature,
                doc: #doc,
                handler: #fn_name,
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn forthic_module(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);

    // Extract all #[forthic_word] annotated methods
    // and register them with the module

    let expanded = quote! {
        #input

        // Module registration code
    };

    TokenStream::from(expanded)
}
```

**Usage Example:**
```rust
use forthic_macros::forthic_word;

pub struct CoreModule {
    // ...
}

impl CoreModule {
    #[forthic_word("( a:any b:any -- b:any a:any )")]
    /// Swaps the top two items on the stack
    async fn swap(&self, interp: &mut Interpreter) -> Result<(), ForthicError> {
        let b = interp.stack_pop()?;
        let a = interp.stack_pop()?;
        interp.stack_push(b);
        interp.stack_push(a);
        Ok(())
    }

    #[forthic_word("( item:any -- )")]
    /// Duplicates the top item on the stack
    async fn dup(&self, interp: &mut Interpreter) -> Result<(), ForthicError> {
        let item = interp.stack_pop()?;
        interp.stack_push(item.clone());
        interp.stack_push(item);
        Ok(())
    }
}
```

**Alternative: Builder Pattern (No Macros):**
```rust
impl CoreModule {
    pub fn new() -> Self {
        let mut module = BasicModule::new("core".to_string());

        module.add_word(Arc::new(SimpleWord {
            name: "SWAP".to_string(),
            signature: "( a:any b:any -- b:any a:any )".to_string(),
            doc: "Swaps the top two items on the stack".to_string(),
            handler: |interp| Box::pin(async move {
                let b = interp.stack_pop()?;
                let a = interp.stack_pop()?;
                interp.stack_push(b);
                interp.stack_push(a);
                Ok(())
            }),
        }));

        // ... more words

        Self { module }
    }
}
```

### 9. src/docs.rs
Documentation generation from metadata
- generate_module_docs
- generate_docs_json
- generate_stdlib_docs

**Source:** `src/forthic/decorators/docs.ts`

## Phase 6: Standard Library Modules

These can be ported in parallel once Phase 5 is complete:

### 10. src/modules/standard/core.rs
Essential operations
- Stack: POP, DUP, SWAP, PEEK!, STACK!
- Variables: VARIABLES, !, @, !@
- Module: EXPORT, USE_MODULES
- Execution: INTERPRET
- Control: IDENTITY, NOP, DEFAULT, *DEFAULT, NULL, ARRAY?
- Options: ~> (array to WordOptions)
- Profiling: PROFILE-START, PROFILE-END, PROFILE-TIMESTAMP, PROFILE-DATA
- Logging: START-LOG, END-LOG
- String: INTERPOLATE, PRINT

**Source:** `src/forthic/modules/core_module.ts`

### 11. src/modules/standard/array.rs
Array and collection operations using Vec

**Source:** `src/forthic/modules/array_module.ts`

**Rust Adaptations:**
- Leverage Rust iterators and functional programming
- Use Vec methods: map, filter, fold, etc.
- Type-safe operations with pattern matching

### 12. src/modules/standard/record.rs
Record/hash operations using HashMap

**Source:** `src/forthic/modules/record_module.ts`

**Rust Adaptations:**
- Use HashMap<String, ForthicValue>
- Entry API for efficient mutations
- Consider serde for structured records

### 13. src/modules/standard/string.rs
String manipulation operations

**Source:** `src/forthic/modules/string_module.ts`

**Rust Adaptations:**
- Use Rust String (UTF-8)
- regex crate for pattern matching
- String manipulation methods

### 14. src/modules/standard/math.rs
Mathematical operations

**Source:** `src/forthic/modules/math_module.ts`

**Rust Adaptations:**
- Use std math functions (f64 methods)
- Consider num crate for advanced math
- Type safety with pattern matching

### 15. src/modules/standard/boolean.rs
Boolean logic operations

**Source:** `src/forthic/modules/boolean_module.ts`

### 16. src/modules/standard/json.rs
JSON parsing and generation

**Source:** `src/forthic/modules/json_module.ts`

**Rust Adaptations:**
- Use serde_json for JSON operations
- JSONValue → ForthicValue conversion
- Type-safe serialization/deserialization

### 17. src/modules/standard/datetime.rs
DateTime operations

**Source:** `src/forthic/modules/datetime_module.ts`

**Rust Adaptations:**
- Use chrono for all datetime operations
- NaiveDate, NaiveTime, DateTime<Tz>
- Timezone handling with chrono-tz

## Phase 7: CLI Tool

### 18. src/bin/forthic.rs
Command-line interface
- REPL mode
- Script execution mode
- Interactive debugging

**Rust Adaptations:**
- Use clap for argument parsing
- rustyline for REPL line editing
- colored for terminal colors

**Example:**
```rust
use clap::Parser;
use rustyline::Editor;
use colored::*;

#[derive(Parser)]
#[command(name = "forthic")]
#[command(about = "Forthic interpreter", long_about = None)]
struct Cli {
    /// Forthic script file to execute
    file: Option<String>,

    /// Start REPL mode
    #[arg(short, long)]
    repl: bool,

    /// Timezone (default: UTC)
    #[arg(short, long, default_value = "UTC")]
    timezone: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if let Some(file) = cli.file {
        // Execute script file
        let code = std::fs::read_to_string(&file)?;
        let mut interp = StandardInterpreter::new(vec![], &cli.timezone);
        interp.run(&code).await?;
    } else {
        // REPL mode
        let mut rl = Editor::<()>::new()?;
        let mut interp = StandardInterpreter::new(vec![], &cli.timezone);

        println!("{}", "Forthic REPL".green().bold());
        println!("Type 'exit' to quit\n");

        loop {
            let readline = rl.readline("forthic> ");
            match readline {
                Ok(line) => {
                    if line.trim() == "exit" {
                        break;
                    }

                    rl.add_history_entry(&line);

                    match interp.run(&line).await {
                        Ok(_) => {
                            // Print stack
                            println!("{}", format!("Stack: {:?}", interp.stack()).dimmed());
                        }
                        Err(e) => {
                            eprintln!("{}", format!("Error: {}", e).red());
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }

    Ok(())
}
```

## Phase 8: Entry Point

### 19. src/lib.rs
Main library exports and public API
- Export Interpreter, StandardInterpreter
- Export Tokenizer
- Export Module trait
- Export error types
- Export utility functions
- Re-export all public types

**Source:** `src/index.ts`

**Example:**
```rust
// Core types
pub mod errors;
pub mod utils;
pub mod literals;
pub mod tokenizer;
pub mod word_options;
pub mod module;
pub mod interpreter;
pub mod docs;

// Standard library modules
pub mod modules {
    pub mod standard {
        pub mod core;
        pub mod array;
        pub mod record;
        pub mod string;
        pub mod math;
        pub mod boolean;
        pub mod json;
        pub mod datetime;
    }
}

// Public exports
pub use errors::{ForthicError, CodeLocation};
pub use interpreter::{Interpreter, StandardInterpreter};
pub use tokenizer::{Tokenizer, Token, TokenType};
pub use module::{Module, Word, ForthicValue};
pub use word_options::WordOptions;

// Prelude for common imports
pub mod prelude {
    pub use crate::errors::ForthicError;
    pub use crate::interpreter::{Interpreter, StandardInterpreter};
    pub use crate::module::{ForthicValue, Module, Word};
}
```

## Phase 9: Cargo Configuration

### 20. Cargo.toml
Project configuration and dependencies

**Example:**
```toml
[package]
name = "forthic"
version = "0.1.0"
edition = "2021"
authors = ["Your Name <you@example.com>"]
description = "A stack-based, concatenative language for composable transformations"
license = "MIT"
repository = "https://github.com/yourusername/forthic-rs"

[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Date/time
chrono = "0.4"
chrono-tz = "0.8"

# JSON
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Regex
regex = "1.0"

# CLI (optional, for bin)
clap = { version = "4", features = ["derive"], optional = true }
rustyline = { version = "12", optional = true }
colored = { version = "2", optional = true }

# Macros (optional)
forthic-macros = { path = "forthic-macros", optional = true }

[dev-dependencies]
# Testing
tokio-test = "0.4"
proptest = "1.0"

# Coverage
tarpaulin = "0.22"

[features]
default = ["cli", "macros"]
cli = ["clap", "rustyline", "colored"]
macros = ["forthic-macros"]

[[bin]]
name = "forthic"
path = "src/bin/forthic.rs"
required-features = ["cli"]

[workspace]
members = ["forthic-macros"]
```

## Key Rust Adaptation Considerations

### TypeScript → Rust Patterns

1. **Decorators → Procedural Macros**
   - `@Word(...)` → `#[forthic_word(...)]` attribute macro
   - Or use builder pattern for explicit registration
   - Metadata stored in static registries using inventory crate

2. **Async/Await → Tokio Async/Await**
   - Keep async/await model (Rust has excellent support)
   - Use `#[async_trait]` for async trait methods
   - Use tokio runtime for execution

3. **TypeScript Types → Rust Types**
   - Strong typing with Rust's type system
   - Use enums for sum types (ForthicValue)
   - Use traits for abstractions (Word, Module)
   - Pattern matching for type checking

4. **ES6 Modules → Rust Modules**
   - `import X from "y"` → `use crate::y::X;`
   - `export class X` → `pub struct X` in `mod.rs`
   - Cargo for dependency management

5. **Temporal API → chrono**
   - `Temporal.PlainTime` → `chrono::NaiveTime`
   - `Temporal.PlainDate` → `chrono::NaiveDate`
   - `Temporal.ZonedDateTime` → `chrono::DateTime<Tz>`

6. **WeakMap → Alternatives**
   - Use `Arc<Mutex<T>>` for shared ownership
   - Use `Weak<T>` for weak references if needed
   - Global registries with once_cell or lazy_static

7. **Proxy Pattern → Traits**
   - Stack: implement Index, Deref, etc.
   - Or just provide accessor methods

8. **Interface Types → Traits**
   - `LiteralHandler` → `fn(&str) -> Option<ForthicValue>`
   - `Word` → trait with `execute` method

### Memory Management

- **Ownership**: Values owned by stack, moved on pop
- **Borrowing**: References when reading without removing
- **Arc**: Shared ownership for modules and words
- **Mutex/RwLock**: Thread-safe interior mutability
- **Clone**: Explicit cloning for ForthicValue

### Error Handling

- Use `Result<T, ForthicError>` for all fallible operations
- Use `?` operator for error propagation
- Use `thiserror` for custom error types
- Provide context with anyhow for CLI

### Testing Strategy

For each phase:
1. Port the Rust code
2. Create unit tests with `#[test]` and `#[tokio::test]`
3. Compare behavior with TypeScript tests
4. Ensure test coverage matches or exceeds TS version

### Dependencies

Rust crates:
- **tokio** - Async runtime
- **async-trait** - Async trait methods
- **thiserror** - Error types
- **anyhow** - Error context
- **chrono** - Date/time handling
- **chrono-tz** - Timezone support
- **serde** + **serde_json** - JSON serialization
- **regex** - Regular expressions
- **clap** - CLI argument parsing
- **rustyline** - REPL line editing
- **colored** - Terminal colors
- **inventory** - Static registration (for macros)

### File Structure

```
forthic-rs/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Main exports
│   ├── errors.rs
│   ├── utils.rs
│   ├── literals.rs
│   ├── tokenizer.rs
│   ├── word_options.rs
│   ├── module.rs
│   ├── interpreter.rs
│   ├── docs.rs
│   ├── modules/
│   │   └── standard/
│   │       ├── mod.rs
│   │       ├── core.rs
│   │       ├── array.rs
│   │       ├── record.rs
│   │       ├── string.rs
│   │       ├── math.rs
│   │       ├── boolean.rs
│   │       ├── json.rs
│   │       └── datetime.rs
│   └── bin/
│       └── forthic.rs           # CLI tool
├── forthic-macros/              # Procedural macros (optional)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── tests/
│   ├── unit/
│   │   ├── core/
│   │   └── modules/
│   │       └── standard/
│   └── integration/
├── benches/                     # Benchmarks
│   └── interpreter.rs
└── README.md
```

## Rust-Specific Advantages

1. **Type Safety**
   - Compile-time type checking prevents many bugs
   - Pattern matching for exhaustive case handling
   - No null pointer exceptions (Option/Result)

2. **Performance**
   - Zero-cost abstractions
   - No garbage collection overhead
   - LLVM optimization

3. **Memory Safety**
   - No buffer overflows
   - No use-after-free
   - No data races (Send/Sync)

4. **Concurrency**
   - Fearless concurrency with ownership
   - Async/await built into language
   - Thread-safe by default

5. **Tooling**
   - Cargo: build, test, benchmark, doc
   - rustfmt: automatic code formatting
   - clippy: linting and best practices
   - rust-analyzer: IDE support

6. **Distribution**
   - Single binary with no runtime deps
   - Cross-compilation support
   - Small binary size with optimization

## Implementation Timeline Estimate

- **Phase 1-2** (Foundation + Infrastructure): 2-3 weeks
- **Phase 3** (Module System): 1-2 weeks
- **Phase 4** (Interpreter): 2-3 weeks
- **Phase 5** (Macros/Builder): 1-2 weeks
- **Phase 6** (Standard Library): 3-4 weeks
- **Phase 7** (CLI): 1 week
- **Phase 8-9** (Exports + Cargo): 1 week
- **Testing & Documentation**: 2-3 weeks

**Total: 13-21 weeks** for complete implementation

## Success Criteria

- [ ] All forthic-ts tests pass in Rust
- [ ] Performance matches or exceeds TypeScript
- [ ] Memory safety verified (no unsafe code, or minimal)
- [ ] CLI tool provides good REPL experience
- [ ] Documentation is comprehensive
- [ ] Can run same Forthic code across TS, Ruby, and Rust runtimes
- [ ] Ready for use as orchestrator (Phase 2 of Go vs Rust analysis)
