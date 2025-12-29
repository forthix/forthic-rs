//! Forthic interpreter - Core execution engine
//!
//! This module provides the main interpreter that executes Forthic code.
//!
//! # Components
//!
//! - **Stack**: Data stack for values
//! - **Interpreter**: Main execution engine with tokenizer integration
//!
//! # Example
//!
//! ```no_run
//! use forthic::interpreter::Interpreter;
//! use forthic::module::Module;
//!
//! let mut interp = Interpreter::new("UTC");
//! // interp.run("42 3.14 'hello'").unwrap();
//! ```

use crate::errors::ForthicError;
use crate::literals::{to_bool, to_float, to_int, to_literal_date, to_time, to_zoned_datetime};
use crate::literals::{ForthicValue, LiteralHandler};
use crate::module::{DefinitionWord, InterpreterContext, Module, PushValueWord, Word};
use crate::tokenizer::{Token, TokenType, Tokenizer};
use std::sync::Arc;

// ========================================
// Special Word Classes
// ========================================

/// StartModuleWord - Handles module creation and switching
///
/// Pushes a module onto the module stack, creating it if necessary.
/// An empty name refers to the app module.
#[derive(Debug, Clone)]
pub struct StartModuleWord {
    name: String,
}

impl StartModuleWord {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl Word for StartModuleWord {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        // The app module is the only module with a blank name
        if self.name.is_empty() {
            let app_module = context.get_app_module().clone();
            context.module_stack_push(app_module);
            return Ok(());
        }

        // If the module is used by the current module, push it onto the stack, otherwise
        // create a new module.
        let module = match context.cur_module().find_module(&self.name) {
            Some(m) => m.clone(),
            None => {
                let new_module = Module::new(self.name.clone());
                context
                    .cur_module_mut()
                    .register_module(self.name.clone(), self.name.clone(), new_module.clone());
                new_module
            }
        };
        context.module_stack_push(module);
        Ok(())
    }
}

/// EndModuleWord - Pops the current module from the module stack
///
/// Completes module context and returns to the previous module.
#[derive(Debug, Clone)]
pub struct EndModuleWord;

impl EndModuleWord {
    pub fn new() -> Self {
        Self
    }
}

impl Word for EndModuleWord {
    fn name(&self) -> &str {
        "}"
    }

    fn execute(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.module_stack_pop()?;
        Ok(())
    }
}

/// EndArrayWord - Collects items from stack into an array
///
/// Pops items from the stack until a START_ARRAY marker is found,
/// then pushes them as a single array in the correct order.
#[derive(Debug, Clone)]
pub struct EndArrayWord;

impl EndArrayWord {
    pub fn new() -> Self {
        Self
    }
}

impl Word for EndArrayWord {
    fn name(&self) -> &str {
        "]"
    }

    fn execute(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let mut items = Vec::new();

        // Pop items until we find START_ARRAY marker
        loop {
            let item = context.stack_pop()?;
            if matches!(item, ForthicValue::StartArrayMarker) {
                break;
            }
            items.push(item);
        }

        // Reverse to get correct order
        items.reverse();

        // Push as array
        context.stack_push(ForthicValue::Array(items));
        Ok(())
    }
}

// ========================================
// Stack
// ========================================

/// Stack - Data stack for the Forthic interpreter
///
/// Wraps a Vec<ForthicValue> and provides stack operations.
///
/// # Examples
///
/// ```
/// use forthic::interpreter::Stack;
/// use forthic::literals::ForthicValue;
///
/// let mut stack = Stack::new();
/// stack.push(ForthicValue::Int(42));
/// stack.push(ForthicValue::String("hello".to_string()));
///
/// assert_eq!(stack.len(), 2);
/// assert_eq!(stack.pop().unwrap(), ForthicValue::String("hello".to_string()));
/// assert_eq!(stack.pop().unwrap(), ForthicValue::Int(42));
/// assert!(stack.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct Stack {
    items: Vec<ForthicValue>,
}

impl Stack {
    /// Create a new empty stack
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Push a value onto the stack
    pub fn push(&mut self, value: ForthicValue) {
        self.items.push(value);
    }

    /// Pop a value from the stack
    ///
    /// Returns an error if the stack is empty.
    pub fn pop(&mut self) -> Result<ForthicValue, ForthicError> {
        self.items.pop().ok_or_else(|| ForthicError::StackUnderflow {
            forthic: String::new(),
            location: None,
            cause: None,
        })
    }

    /// Peek at the top value without removing it
    pub fn peek(&self) -> Option<&ForthicValue> {
        self.items.last()
    }

    /// Get the number of items on the stack
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the stack is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clear all items from the stack
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Get an item at a specific index (0 is bottom of stack)
    pub fn get(&self, index: usize) -> Option<&ForthicValue> {
        self.items.get(index)
    }

    /// Get a mutable reference to an item at a specific index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut ForthicValue> {
        self.items.get_mut(index)
    }

    /// Get a reference to all items
    pub fn items(&self) -> &[ForthicValue] {
        &self.items
    }

    /// Duplicate the stack (shallow copy)
    pub fn dup(&self) -> Self {
        Self {
            items: self.items.clone(),
        }
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

/// Interpreter - Main Forthic execution engine
///
/// Manages the data stack, module stack, and execution state.
///
/// # Examples
///
/// ```no_run
/// use forthic::interpreter::Interpreter;
///
/// let mut interp = Interpreter::new("UTC");
/// // Future: interp.run("42 3 +").unwrap();
/// ```
pub struct Interpreter {
    /// Data stack for values
    stack: Stack,

    /// Application module (root module with empty name)
    app_module: Module,

    /// Module stack for nested module contexts
    module_stack: Vec<Module>,

    /// Tokenizer stack for nested code execution
    tokenizer_stack: Vec<Tokenizer>,

    /// Timezone for date/time operations
    timezone: String,

    /// Whether we're currently compiling a definition
    is_compiling: bool,

    /// Whether the current definition is a memo
    is_memo_definition: bool,

    /// Current definition being compiled
    cur_definition: Option<DefinitionWord>,

    /// Literal handlers for parsing values (checked in registration order)
    literal_handlers: Vec<LiteralHandler>,
}

impl Interpreter {
    /// Create a new interpreter with the specified timezone
    ///
    /// # Arguments
    ///
    /// * `timezone` - Timezone string (e.g., "UTC", "America/Los_Angeles")
    ///
    /// # Examples
    ///
    /// ```
    /// use forthic::interpreter::Interpreter;
    ///
    /// let interp = Interpreter::new("UTC");
    /// ```
    pub fn new(timezone: &str) -> Self {
        let app_module = Module::new(String::new()); // Empty name for app module

        let mut interp = Self {
            stack: Stack::new(),
            app_module: app_module.clone(),
            module_stack: vec![app_module],
            tokenizer_stack: Vec::new(),
            timezone: timezone.to_string(),
            is_compiling: false,
            is_memo_definition: false,
            cur_definition: None,
            literal_handlers: Vec::new(),
        };

        // Register default literal handlers
        // Order matters: more specific handlers first
        interp.register_literal_handler(Box::new(to_bool)); // TRUE, FALSE
        interp.register_literal_handler(Box::new(to_float)); // 3.14
        interp.register_literal_handler(Box::new(to_zoned_datetime(timezone))); // 2020-06-05T10:15:00Z
        interp.register_literal_handler(Box::new(to_literal_date(timezone))); // 2020-06-05
        interp.register_literal_handler(Box::new(to_time)); // 9:00, 11:30 PM
        interp.register_literal_handler(Box::new(to_int)); // 42

        interp
    }

    /// Get the timezone
    pub fn get_timezone(&self) -> &str {
        &self.timezone
    }

    /// Set the timezone
    pub fn set_timezone(&mut self, timezone: String) {
        self.timezone = timezone;
    }

    /// Get a reference to the stack
    pub fn get_stack(&self) -> &Stack {
        &self.stack
    }

    /// Get a mutable reference to the stack
    pub fn get_stack_mut(&mut self) -> &mut Stack {
        &mut self.stack
    }

    /// Set the entire stack (replacing the current stack)
    pub fn set_stack(&mut self, stack: Stack) {
        self.stack = stack;
    }

    /// Push a value onto the stack
    pub fn stack_push(&mut self, value: ForthicValue) {
        self.stack.push(value);
    }

    /// Pop a value from the stack
    pub fn stack_pop(&mut self) -> Result<ForthicValue, ForthicError> {
        self.stack.pop()
    }

    /// Peek at the top of the stack
    pub fn stack_peek(&self) -> Option<&ForthicValue> {
        self.stack.peek()
    }

    /// Get the current module (top of module stack)
    pub fn cur_module(&self) -> &Module {
        self.module_stack
            .last()
            .expect("Module stack should never be empty")
    }

    /// Get a mutable reference to the current module
    pub fn cur_module_mut(&mut self) -> &mut Module {
        self.module_stack
            .last_mut()
            .expect("Module stack should never be empty")
    }

    /// Push a module onto the module stack
    pub fn module_stack_push(&mut self, module: Module) {
        self.module_stack.push(module);
    }

    /// Pop a module from the module stack
    ///
    /// Returns an error if trying to pop the app module.
    pub fn module_stack_pop(&mut self) -> Result<Module, ForthicError> {
        if self.module_stack.len() <= 1 {
            return Err(ForthicError::StackUnderflow {
                forthic: "Cannot pop app module".to_string(),
                location: None,
                cause: None,
            });
        }
        Ok(self.module_stack.pop().unwrap())
    }

    /// Get the application module
    pub fn get_app_module(&self) -> &Module {
        &self.app_module
    }

    /// Get a mutable reference to the application module
    pub fn get_app_module_mut(&mut self) -> &mut Module {
        &mut self.app_module
    }

    /// Check if currently compiling a definition
    pub fn is_compiling(&self) -> bool {
        self.is_compiling
    }

    /// Set compilation state
    pub fn set_compiling(&mut self, compiling: bool) {
        self.is_compiling = compiling;
    }

    /// Check if current definition is a memo
    pub fn is_memo_definition(&self) -> bool {
        self.is_memo_definition
    }

    /// Set memo definition state
    pub fn set_memo_definition(&mut self, is_memo: bool) {
        self.is_memo_definition = is_memo;
    }

    /// Get the current definition being compiled
    pub fn get_cur_definition(&self) -> Option<&DefinitionWord> {
        self.cur_definition.as_ref()
    }

    /// Get a mutable reference to the current definition
    pub fn get_cur_definition_mut(&mut self) -> Option<&mut DefinitionWord> {
        self.cur_definition.as_mut()
    }

    /// Set the current definition
    pub fn set_cur_definition(&mut self, definition: Option<DefinitionWord>) {
        self.cur_definition = definition;
    }

    /// Reset the interpreter state
    pub fn reset(&mut self) {
        self.stack.clear();
        self.module_stack = vec![self.app_module.clone()];
        self.is_compiling = false;
        self.is_memo_definition = false;
        self.cur_definition = None;
    }

    // ========================================
    // Literal Handlers
    // ========================================

    /// Register a custom literal handler
    ///
    /// Handlers are checked in registration order when parsing words.
    /// More specific handlers should be registered first.
    ///
    /// # Examples
    ///
    /// ```
    /// use forthic::interpreter::Interpreter;
    /// use forthic::literals::ForthicValue;
    ///
    /// let mut interp = Interpreter::new("UTC");
    ///
    /// // Custom handler for hex numbers like 0xFF
    /// fn to_hex(s: &str) -> Option<ForthicValue> {
    ///     if s.starts_with("0x") || s.starts_with("0X") {
    ///         i64::from_str_radix(&s[2..], 16)
    ///             .ok()
    ///             .map(ForthicValue::Int)
    ///     } else {
    ///         None
    ///     }
    /// }
    ///
    /// interp.register_literal_handler(Box::new(to_hex));
    /// ```
    pub fn register_literal_handler(&mut self, handler: LiteralHandler) {
        self.literal_handlers.push(handler);
    }

    /// Unregister a literal handler
    ///
    /// Removes the first matching handler from the list.
    /// Note: Due to boxed closures, comparison is done by raw pointer to the trait object.
    pub fn unregister_literal_handler(&mut self, handler: LiteralHandler) {
        let handler_ptr = &*handler as *const dyn Fn(&str) -> Option<ForthicValue>;
        if let Some(index) = self.literal_handlers.iter().position(|h| {
            let h_ptr = &**h as *const dyn Fn(&str) -> Option<ForthicValue>;
            std::ptr::eq(h_ptr, handler_ptr)
        }) {
            let _ = self.literal_handlers.remove(index);
        }
    }

    /// Try to parse a string as a literal value
    ///
    /// Checks all registered literal handlers in order.
    /// Returns a PushValueWord if successful, None otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use forthic::interpreter::Interpreter;
    ///
    /// let interp = Interpreter::new("UTC");
    ///
    /// // Should recognize integer
    /// assert!(interp.find_literal_word("42").is_some());
    ///
    /// // Should recognize float
    /// assert!(interp.find_literal_word("3.14").is_some());
    ///
    /// // Should recognize boolean
    /// assert!(interp.find_literal_word("TRUE").is_some());
    ///
    /// // Should not recognize unknown word
    /// assert!(interp.find_literal_word("GARBAGE").is_none());
    /// ```
    pub fn find_literal_word(&self, name: &str) -> Option<Arc<dyn Word>> {
        for handler in &self.literal_handlers {
            if let Some(value) = handler(name) {
                return Some(Arc::new(PushValueWord::new(name.to_string(), value)));
            }
        }
        None
    }

    // ========================================
    // Find Word
    // ========================================

    /// Find a word by name
    ///
    /// Searches in this order:
    /// 1. Module stack (from top to bottom) - dictionary words and variables
    /// 2. Literal handlers
    ///
    /// Returns an error if word is not found.
    pub fn find_word(&self, name: &str) -> Result<Arc<dyn Word>, ForthicError> {
        // 1. Check module stack (dictionary words + variables)
        for module in self.module_stack.iter().rev() {
            if let Some(word) = module.find_word(name) {
                return Ok(word);
            }
        }

        // 2. Check literal handlers as fallback
        if let Some(word) = self.find_literal_word(name) {
            return Ok(word);
        }

        // 3. Throw error if still not found
        Err(ForthicError::UnknownWord {
            forthic: String::new(),
            word: name.to_string(),
            location: None,
            cause: None,
        })
    }

    // ========================================
    // Token Handlers
    // ========================================

    /// Main token dispatcher
    ///
    /// Routes tokens to appropriate handlers based on token type.
    pub fn handle_token(&mut self, token: Token) -> Result<(), ForthicError> {
        match token.token_type {
            TokenType::String => self.handle_string_token(token),
            TokenType::Comment => self.handle_comment_token(token),
            TokenType::StartArray => self.handle_start_array_token(token),
            TokenType::EndArray => self.handle_end_array_token(token),
            TokenType::StartModule => self.handle_start_module_token(token),
            TokenType::EndModule => self.handle_end_module_token(token),
            TokenType::StartDef => self.handle_start_definition_token(token),
            TokenType::StartMemo => self.handle_start_memo_token(token),
            TokenType::EndDef => self.handle_end_definition_token(token),
            TokenType::DotSymbol => self.handle_dot_symbol_token(token),
            TokenType::Word => self.handle_word_token(token),
            TokenType::Eos => self.handle_eos_token(token),
        }
    }

    /// Handle string literal tokens
    fn handle_string_token(&mut self, token: Token) -> Result<(), ForthicError> {
        let word = PushValueWord::new(
            "<string>".to_string(),
            ForthicValue::String(token.string.clone()),
        );
        self.handle_word(Arc::new(word))
    }

    /// Handle dot-symbol tokens (.foo)
    fn handle_dot_symbol_token(&mut self, token: Token) -> Result<(), ForthicError> {
        let word = PushValueWord::new(
            "<dot-symbol>".to_string(),
            ForthicValue::String(token.string.clone()),
        );
        self.handle_word(Arc::new(word))
    }

    /// Handle start array tokens [
    fn handle_start_array_token(&mut self, _token: Token) -> Result<(), ForthicError> {
        let word = PushValueWord::new(
            "<start_array_token>".to_string(),
            ForthicValue::StartArrayMarker,
        );
        self.handle_word(Arc::new(word))
    }

    /// Handle end array tokens ]
    fn handle_end_array_token(&mut self, _token: Token) -> Result<(), ForthicError> {
        let word = Arc::new(EndArrayWord::new());
        self.handle_word(word)
    }

    /// Handle start module tokens
    ///
    /// Start/end module tokens are IMMEDIATE words - they execute even during compilation
    /// and are also added to the current definition.
    fn handle_start_module_token(&mut self, token: Token) -> Result<(), ForthicError> {
        let word = Arc::new(StartModuleWord::new(token.string.clone()));

        // If compiling, add to definition
        if self.is_compiling {
            if let Some(def) = &mut self.cur_definition {
                def.add_word(word.clone());
            }
        }

        // Always execute (IMMEDIATE word)
        word.execute(self)
    }

    /// Handle end module tokens }
    fn handle_end_module_token(&mut self, _token: Token) -> Result<(), ForthicError> {
        let word = Arc::new(EndModuleWord::new());

        // If compiling, add to definition
        if self.is_compiling {
            if let Some(def) = &mut self.cur_definition {
                def.add_word(word.clone());
            }
        }

        // Always execute (IMMEDIATE word)
        word.execute(self)
    }

    /// Handle start definition tokens :
    fn handle_start_definition_token(&mut self, token: Token) -> Result<(), ForthicError> {
        if self.is_compiling {
            return Err(ForthicError::MissingSemicolon {
                forthic: String::new(),
                location: None,
                cause: None,
            });
        }

        self.cur_definition = Some(DefinitionWord::new(token.string.clone()));
        self.is_compiling = true;
        self.is_memo_definition = false;
        Ok(())
    }

    /// Handle start memo tokens @:
    fn handle_start_memo_token(&mut self, token: Token) -> Result<(), ForthicError> {
        if self.is_compiling {
            return Err(ForthicError::MissingSemicolon {
                forthic: String::new(),
                location: None,
                cause: None,
            });
        }

        self.cur_definition = Some(DefinitionWord::new(token.string.clone()));
        self.is_compiling = true;
        self.is_memo_definition = true;
        Ok(())
    }

    /// Handle end definition tokens ;
    fn handle_end_definition_token(&mut self, _token: Token) -> Result<(), ForthicError> {
        if !self.is_compiling || self.cur_definition.is_none() {
            return Err(ForthicError::ExtraSemicolon {
                forthic: String::new(),
                location: None,
                cause: None,
            });
        }

        let definition = self.cur_definition.take().unwrap();

        // Add to current module
        if self.is_memo_definition {
            self.cur_module_mut().add_memo_words(Arc::new(definition));
        } else {
            self.cur_module_mut().add_word(Arc::new(definition));
        }

        self.is_compiling = false;
        Ok(())
    }

    /// Handle word tokens (identifiers)
    fn handle_word_token(&mut self, token: Token) -> Result<(), ForthicError> {
        let word = self.find_word(&token.string)?;
        self.handle_word(word)
    }

    /// Handle end-of-stream tokens
    fn handle_eos_token(&mut self, _token: Token) -> Result<(), ForthicError> {
        if self.is_compiling {
            return Err(ForthicError::MissingSemicolon {
                forthic: String::new(),
                location: None,
                cause: None,
            });
        }
        Ok(())
    }

    /// Handle comment tokens (no-op)
    fn handle_comment_token(&mut self, _token: Token) -> Result<(), ForthicError> {
        // Comments are ignored
        Ok(())
    }

    /// Execute or compile a word
    ///
    /// If compiling, adds word to current definition.
    /// Otherwise, executes the word immediately.
    fn handle_word(&mut self, word: Arc<dyn Word>) -> Result<(), ForthicError> {
        if self.is_compiling {
            if let Some(def) = &mut self.cur_definition {
                def.add_word(word);
            }
            Ok(())
        } else {
            word.execute(self)
        }
    }

    // ========================================
    // Execution
    // ========================================

    /// Run Forthic code
    ///
    /// Tokenizes the input string and executes all tokens.
    ///
    /// # Arguments
    ///
    /// * `code` - Forthic code to execute
    ///
    /// # Examples
    ///
    /// ```
    /// use forthic::interpreter::Interpreter;
    ///
    /// let mut interp = Interpreter::new("UTC");
    /// // interp.run("42 3 +").unwrap();
    /// ```
    pub fn run(&mut self, code: &str) -> Result<(), ForthicError> {
        let tokenizer = Tokenizer::new(code.to_string(), None, false);
        self.tokenizer_stack.push(tokenizer);

        let result = self.run_with_tokenizer();

        self.tokenizer_stack.pop();
        result
    }

    /// Execute tokens from the current tokenizer
    ///
    /// Loops through tokens, handling each one until EOS is reached.
    fn run_with_tokenizer(&mut self) -> Result<(), ForthicError> {
        loop {
            // Get the current tokenizer (must exist since run() pushes one)
            let tokenizer = self
                .tokenizer_stack
                .last_mut()
                .expect("Tokenizer stack should not be empty");

            let token = tokenizer.next_token()?;

            // Check for EOS before handling
            if token.token_type == TokenType::Eos {
                self.handle_token(token)?;
                break;
            }

            // Handle the token
            self.handle_token(token)?;
        }

        Ok(())
    }

    // ========================================
    // Tokenizer Access
    // ========================================

    /// Get the current tokenizer (top of tokenizer stack)
    ///
    /// Returns None if the tokenizer stack is empty.
    pub fn get_tokenizer(&self) -> Option<&Tokenizer> {
        self.tokenizer_stack.last()
    }

    /// Get a mutable reference to the current tokenizer
    ///
    /// Returns None if the tokenizer stack is empty.
    pub fn get_tokenizer_mut(&mut self) -> Option<&mut Tokenizer> {
        self.tokenizer_stack.last_mut()
    }

    /// Get the input string from the first (bottom) tokenizer
    ///
    /// This is useful for error messages to show the original code being executed.
    /// Returns an empty string if the tokenizer stack is empty.
    pub fn get_top_input_string(&self) -> String {
        if self.tokenizer_stack.is_empty() {
            return String::new();
        }
        self.tokenizer_stack[0].get_input_string().to_string()
    }

    // ========================================
    // Module Management
    // ========================================

    /// Register a module with the interpreter
    ///
    /// Makes the module available for use by name.
    /// Modules registered this way can be imported into other modules.
    pub fn register_module(&mut self, module: Module) {
        let name = module.get_name().to_string();
        self.get_app_module_mut()
            .register_module(name.clone(), name, module);
    }

    /// Find a registered module by name
    ///
    /// Returns an error if the module is not found.
    pub fn find_module(&self, name: &str) -> Result<&Module, ForthicError> {
        self.get_app_module()
            .find_module(name)
            .ok_or_else(|| ForthicError::UnknownModule {
                forthic: String::new(),
                module_name: name.to_string(),
                location: None,
                cause: None,
            })
    }

    /// Import a module into the app module with optional prefix
    ///
    /// Convenience method that registers and imports a module in one step.
    ///
    /// # Arguments
    ///
    /// * `module` - The module to import
    /// * `prefix` - Optional prefix for the imported words (empty string for no prefix)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use forthic::interpreter::Interpreter;
    /// use forthic::module::Module;
    ///
    /// let mut interp = Interpreter::new("UTC");
    /// let module = Module::new("math".to_string());
    ///
    /// // Import without prefix
    /// interp.import_module(module.clone(), "");
    ///
    /// // Import with prefix
    /// interp.import_module(module, "m");
    /// ```
    pub fn import_module(&mut self, module: Module, prefix: &str) {
        // Register the module first
        self.register_module(module.clone());

        // Import into app module
        self.get_app_module_mut().import_module(prefix, &module);
    }

    /// Import multiple modules without prefixes
    ///
    /// Convenience method for importing several modules at once.
    pub fn import_modules(&mut self, modules: Vec<Module>) {
        for module in modules {
            self.import_module(module, "");
        }
    }

    // ========================================
    // Module Execution
    // ========================================

    /// Execute a module's Forthic code
    ///
    /// Pushes the module onto the module stack, runs its code, then pops it.
    /// Wraps any errors in a ModuleError for better diagnostics.
    ///
    /// # Arguments
    ///
    /// * `module` - The module to execute
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use forthic::interpreter::Interpreter;
    /// use forthic::module::Module;
    ///
    /// let mut interp = Interpreter::new("UTC");
    /// let module = Module::new_with_code("test".to_string(), ": HELLO 'world' ;".to_string());
    /// interp.run_module_code(&module).unwrap();
    /// ```
    pub fn run_module_code(&mut self, module: &Module) -> Result<(), ForthicError> {
        self.module_stack_push(module.clone());

        // Try to run the module's code
        let result = self.run(&module.get_forthic_code());

        // Always pop the module, even if there was an error
        self.module_stack_pop()?;

        // If there was an error, wrap it in a Module error
        if let Err(e) = result {
            let inner_message = e.to_string();
            return Err(ForthicError::Module {
                forthic: String::new(),
                module_name: module.get_name().to_string(),
                inner_message,
                inner_error: Box::new(e),
                location: None,
                cause: None,
            });
        }

        Ok(())
    }
}

// ========================================
// InterpreterContext Implementation
// ========================================

impl InterpreterContext for Interpreter {
    fn stack_push(&mut self, value: ForthicValue) {
        self.stack.push(value);
    }

    fn stack_pop(&mut self) -> Result<ForthicValue, ForthicError> {
        self.stack.pop()
    }

    fn stack_peek(&self) -> Option<&ForthicValue> {
        self.stack.peek()
    }

    fn cur_module(&self) -> &Module {
        self.module_stack
            .last()
            .expect("Module stack should never be empty")
    }

    fn cur_module_mut(&mut self) -> &mut Module {
        self.module_stack
            .last_mut()
            .expect("Module stack should never be empty")
    }

    fn get_app_module(&self) -> &Module {
        &self.app_module
    }

    fn module_stack_push(&mut self, module: Module) {
        self.module_stack.push(module);
    }

    fn module_stack_pop(&mut self) -> Result<Module, ForthicError> {
        if self.module_stack.len() <= 1 {
            return Err(ForthicError::StackUnderflow {
                forthic: "Cannot pop app module".to_string(),
                location: None,
                cause: None,
            });
        }
        Ok(self.module_stack.pop().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_new() {
        let stack = Stack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_stack_push_pop() {
        let mut stack = Stack::new();

        stack.push(ForthicValue::Int(42));
        stack.push(ForthicValue::String("hello".to_string()));

        assert_eq!(stack.len(), 2);
        assert_eq!(
            stack.pop().unwrap(),
            ForthicValue::String("hello".to_string())
        );
        assert_eq!(stack.pop().unwrap(), ForthicValue::Int(42));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_pop_empty() {
        let mut stack = Stack::new();
        assert!(stack.pop().is_err());
    }

    #[test]
    fn test_stack_peek() {
        let mut stack = Stack::new();
        assert!(stack.peek().is_none());

        stack.push(ForthicValue::Int(42));
        assert_eq!(stack.peek(), Some(&ForthicValue::Int(42)));
        assert_eq!(stack.len(), 1); // Peek doesn't remove
    }

    #[test]
    fn test_stack_clear() {
        let mut stack = Stack::new();
        stack.push(ForthicValue::Int(1));
        stack.push(ForthicValue::Int(2));
        stack.push(ForthicValue::Int(3));

        stack.clear();
        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_get() {
        let mut stack = Stack::new();
        stack.push(ForthicValue::Int(1));
        stack.push(ForthicValue::Int(2));
        stack.push(ForthicValue::Int(3));

        assert_eq!(stack.get(0), Some(&ForthicValue::Int(1)));
        assert_eq!(stack.get(1), Some(&ForthicValue::Int(2)));
        assert_eq!(stack.get(2), Some(&ForthicValue::Int(3)));
        assert_eq!(stack.get(3), None);
    }

    #[test]
    fn test_stack_dup() {
        let mut stack = Stack::new();
        stack.push(ForthicValue::Int(42));

        let dup = stack.dup();
        assert_eq!(stack.len(), dup.len());
        assert_eq!(stack.pop().unwrap(), dup.peek().unwrap().clone());
    }

    #[test]
    fn test_interpreter_new() {
        let interp = Interpreter::new("UTC");
        assert_eq!(interp.get_timezone(), "UTC");
        assert!(interp.get_stack().is_empty());
        assert!(!interp.is_compiling());
    }

    #[test]
    fn test_interpreter_stack_operations() {
        let mut interp = Interpreter::new("UTC");

        interp.stack_push(ForthicValue::Int(42));
        interp.stack_push(ForthicValue::String("test".to_string()));

        assert_eq!(interp.get_stack().len(), 2);
        assert_eq!(
            interp.stack_peek(),
            Some(&ForthicValue::String("test".to_string()))
        );

        assert_eq!(
            interp.stack_pop().unwrap(),
            ForthicValue::String("test".to_string())
        );
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(42));
    }

    #[test]
    fn test_interpreter_module_stack() {
        let mut interp = Interpreter::new("UTC");

        let module = Module::new("test_module".to_string());
        interp.module_stack_push(module);

        assert_eq!(interp.cur_module().get_name(), "test_module");

        let popped = interp.module_stack_pop().unwrap();
        assert_eq!(popped.get_name(), "test_module");

        // Should be back to app module
        assert_eq!(interp.cur_module().get_name(), "");
    }

    #[test]
    fn test_interpreter_reset() {
        let mut interp = Interpreter::new("UTC");

        interp.stack_push(ForthicValue::Int(42));
        interp.set_compiling(true);
        interp.module_stack_push(Module::new("test".to_string()));

        interp.reset();

        assert!(interp.get_stack().is_empty());
        assert!(!interp.is_compiling());
        assert_eq!(interp.cur_module().get_name(), "");
    }

    #[test]
    fn test_interpreter_compilation_state() {
        let mut interp = Interpreter::new("UTC");

        assert!(!interp.is_compiling());
        assert!(!interp.is_memo_definition());

        interp.set_compiling(true);
        assert!(interp.is_compiling());

        interp.set_memo_definition(true);
        assert!(interp.is_memo_definition());
    }

    #[test]
    fn test_find_literal_word_int() {
        let interp = Interpreter::new("UTC");

        let word = interp.find_literal_word("42");
        assert!(word.is_some());
        assert_eq!(word.unwrap().name(), "42");
    }

    #[test]
    fn test_find_literal_word_float() {
        let interp = Interpreter::new("UTC");

        let word = interp.find_literal_word("3.14");
        assert!(word.is_some());
    }

    #[test]
    fn test_find_literal_word_bool() {
        let interp = Interpreter::new("UTC");

        assert!(interp.find_literal_word("TRUE").is_some());
        assert!(interp.find_literal_word("FALSE").is_some());
    }

    #[test]
    fn test_find_literal_word_time() {
        let interp = Interpreter::new("UTC");

        assert!(interp.find_literal_word("14:30").is_some());
        assert!(interp.find_literal_word("2:30 PM").is_some());
    }

    #[test]
    fn test_find_literal_word_date() {
        let interp = Interpreter::new("UTC");

        assert!(interp.find_literal_word("2023-12-25").is_some());
    }

    #[test]
    fn test_find_literal_word_unknown() {
        let interp = Interpreter::new("UTC");

        assert!(interp.find_literal_word("GARBAGE").is_none());
        assert!(interp.find_literal_word("not-a-literal").is_none());
    }

    #[test]
    fn test_register_custom_literal_handler() {
        let mut interp = Interpreter::new("UTC");

        // Custom handler for hex numbers
        fn to_hex(s: &str) -> Option<ForthicValue> {
            if s.starts_with("0x") || s.starts_with("0X") {
                i64::from_str_radix(&s[2..], 16)
                    .ok()
                    .map(ForthicValue::Int)
            } else {
                None
            }
        }

        interp.register_literal_handler(Box::new(to_hex));

        // Should now recognize hex numbers
        let word = interp.find_literal_word("0xFF");
        assert!(word.is_some());
        assert_eq!(word.unwrap().name(), "0xFF");

        // Should still recognize regular literals
        assert!(interp.find_literal_word("42").is_some());
    }

    #[test]
    fn test_literal_handler_order() {
        let interp = Interpreter::new("UTC");

        // Float handler should be checked before int handler
        // So "3.14" should be recognized as float, not fail as int
        let word = interp.find_literal_word("3.14");
        assert!(word.is_some());

        // Integer should still work
        let word = interp.find_literal_word("42");
        assert!(word.is_some());
    }

    // ========================================
    // Integration Tests
    // ========================================

    #[test]
    fn test_run_simple_literals() {
        let mut interp = Interpreter::new("UTC");

        // Push some literals onto the stack
        interp.run("42 3.14 TRUE 'hello'").unwrap();

        // Check stack contents
        assert_eq!(interp.get_stack().len(), 4);
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::String("hello".to_string()));
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Bool(true));
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Float(3.14));
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(42));
    }

    #[test]
    fn test_run_array_construction() {
        let mut interp = Interpreter::new("UTC");

        // Build an array
        interp.run("[1 2 3]").unwrap();

        // Check that we have one item on the stack (the array)
        assert_eq!(interp.get_stack().len(), 1);

        let arr = interp.stack_pop().unwrap();
        if let ForthicValue::Array(items) = arr {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], ForthicValue::Int(1));
            assert_eq!(items[1], ForthicValue::Int(2));
            assert_eq!(items[2], ForthicValue::Int(3));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_run_nested_arrays() {
        let mut interp = Interpreter::new("UTC");

        // Build nested arrays
        interp.run("[[1 2] [3 4]]").unwrap();

        assert_eq!(interp.get_stack().len(), 1);

        let outer = interp.stack_pop().unwrap();
        if let ForthicValue::Array(outer_items) = outer {
            assert_eq!(outer_items.len(), 2);

            if let ForthicValue::Array(inner1) = &outer_items[0] {
                assert_eq!(inner1.len(), 2);
                assert_eq!(inner1[0], ForthicValue::Int(1));
                assert_eq!(inner1[1], ForthicValue::Int(2));
            } else {
                panic!("Expected inner array");
            }

            if let ForthicValue::Array(inner2) = &outer_items[1] {
                assert_eq!(inner2.len(), 2);
                assert_eq!(inner2[0], ForthicValue::Int(3));
                assert_eq!(inner2[1], ForthicValue::Int(4));
            } else {
                panic!("Expected inner array");
            }
        } else {
            panic!("Expected outer array");
        }
    }

    #[test]
    fn test_run_simple_definition() {
        let mut interp = Interpreter::new("UTC");

        // Define a word that pushes 42
        interp.run(": FORTY-TWO 42 ;").unwrap();

        // Execute the word
        interp.run("FORTY-TWO").unwrap();

        assert_eq!(interp.get_stack().len(), 1);
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(42));
    }

    #[test]
    fn test_run_definition_with_multiple_values() {
        let mut interp = Interpreter::new("UTC");

        // Define a word that pushes multiple values
        interp.run(": NUMS 1 2 3 ;").unwrap();

        // Execute the word
        interp.run("NUMS").unwrap();

        assert_eq!(interp.get_stack().len(), 3);
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(3));
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(2));
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(1));
    }

    #[test]
    fn test_run_comments() {
        let mut interp = Interpreter::new("UTC");

        // Comments should be ignored
        interp.run("42 # This is a comment\n3.14").unwrap();

        assert_eq!(interp.get_stack().len(), 2);
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Float(3.14));
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(42));
    }

    #[test]
    fn test_run_unknown_word_error() {
        let mut interp = Interpreter::new("UTC");

        // Should get an error for unknown word
        let result = interp.run("UNKNOWN-WORD");
        assert!(result.is_err());

        if let Err(ForthicError::UnknownWord { word, .. }) = result {
            assert_eq!(word, "UNKNOWN-WORD");
        } else {
            panic!("Expected UnknownWord error");
        }
    }

    #[test]
    fn test_run_missing_semicolon_error() {
        let mut interp = Interpreter::new("UTC");

        // Should get an error for unclosed definition
        let result = interp.run(": UNCLOSED 42");
        assert!(result.is_err());

        if let Err(ForthicError::MissingSemicolon { .. }) = result {
            // Expected
        } else {
            panic!("Expected MissingSemicolon error");
        }
    }

    #[test]
    fn test_run_extra_semicolon_error() {
        let mut interp = Interpreter::new("UTC");

        // Should get an error for extra semicolon
        let result = interp.run("42 ;");
        assert!(result.is_err());

        if let Err(ForthicError::ExtraSemicolon { .. }) = result {
            // Expected
        } else {
            panic!("Expected ExtraSemicolon error");
        }
    }

    // ========================================
    // Phase 4.5 Tests: Module and String Execution
    // ========================================

    #[test]
    fn test_run_module_code() {
        let mut interp = Interpreter::new("UTC");

        // Create a module with code that defines a word
        let module = Module::new_with_code(
            "test_module".to_string(),
            ": MODULE-WORD 99 ;".to_string(),
        );

        // Run the module's code
        interp.run_module_code(&module).unwrap();

        // After running, the word should NOT be in the app module (not imported)
        assert!(interp.cur_module().find_word("MODULE-WORD").is_none());

        // To actually access the module's words, we'd need to import it or register it
        // The run_module_code just executes the code in the module's context
    }

    #[test]
    fn test_run_module_code_with_error() {
        let mut interp = Interpreter::new("UTC");

        // Create a module with invalid code
        let module = Module::new_with_code(
            "bad_module".to_string(),
            ": UNCLOSED 42".to_string(), // Missing semicolon
        );

        // Should get a Module error wrapping the MissingSemicolon error
        let result = interp.run_module_code(&module);
        assert!(result.is_err());

        if let Err(ForthicError::Module { module_name, .. }) = result {
            assert_eq!(module_name, "bad_module");
        } else {
            panic!("Expected Module error");
        }
    }

    #[test]
    fn test_register_module() {
        let mut interp = Interpreter::new("UTC");

        // Create and register a module
        let mut module = Module::new("my_module".to_string());
        let word = Arc::new(PushValueWord::new("TEST_WORD".to_string(), ForthicValue::Int(42)));
        module.add_word(word);

        interp.register_module(module.clone());

        // Module should now be registered in app module
        assert!(interp.get_app_module().find_module("my_module").is_some());
    }

    #[test]
    fn test_module_with_definitions() {
        let mut interp = Interpreter::new("UTC");

        // Create a module with multiple simple definitions
        let module = Module::new_with_code(
            "numbers".to_string(),
            ": FORTY-TWO 42 ; : NINETY-NINE 99 ;".to_string(),
        );

        // Run the module's code - should not error with simple literals
        interp.run_module_code(&module).unwrap();

        // The module executed successfully (no errors)
        // Note: Words are defined in a clone of the module that gets popped
    }

    // ========================================
    // Phase 4.6 Tests: Module Import/Export
    // ========================================

    #[test]
    fn test_find_module() {
        let mut interp = Interpreter::new("UTC");

        // Register a module
        let module = Module::new("test_module".to_string());
        interp.register_module(module);

        // Should be able to find it
        assert!(interp.find_module("test_module").is_ok());

        // Should get error for unknown module
        assert!(interp.find_module("unknown").is_err());
    }

    #[test]
    fn test_import_module_unprefixed() {
        let mut interp = Interpreter::new("UTC");

        // Create a module with an exportable word
        let mut module = Module::new("math".to_string());
        let word = Arc::new(PushValueWord::new("PI".to_string(), ForthicValue::Float(3.14)));
        module.add_exportable_word(word);

        // Import without prefix
        interp.import_module(module, "");

        // Word should be accessible directly
        assert!(interp.get_app_module().find_word("PI").is_some());
    }

    #[test]
    fn test_import_module_prefixed() {
        let mut interp = Interpreter::new("UTC");

        // Create a module with an exportable word
        let mut module = Module::new("math".to_string());
        let word = Arc::new(PushValueWord::new("PI".to_string(), ForthicValue::Float(3.14)));
        module.add_exportable_word(word);

        // Import with prefix
        interp.import_module(module, "m");

        // Word should be accessible with prefix
        assert!(interp.get_app_module().find_word("m.PI").is_some());

        // But not without prefix
        assert!(interp.get_app_module().find_word("PI").is_none());
    }

    #[test]
    fn test_import_modules() {
        let mut interp = Interpreter::new("UTC");

        // Create two modules
        let mut module1 = Module::new("mod1".to_string());
        module1.add_exportable_word(Arc::new(PushValueWord::new(
            "WORD1".to_string(),
            ForthicValue::Int(1),
        )));

        let mut module2 = Module::new("mod2".to_string());
        module2.add_exportable_word(Arc::new(PushValueWord::new(
            "WORD2".to_string(),
            ForthicValue::Int(2),
        )));

        // Import both modules
        interp.import_modules(vec![module1, module2]);

        // Both words should be accessible
        assert!(interp.get_app_module().find_word("WORD1").is_some());
        assert!(interp.get_app_module().find_word("WORD2").is_some());
    }

    #[test]
    fn test_module_registration_and_lookup() {
        let mut interp = Interpreter::new("UTC");

        // Create and register a module
        let mut module = Module::new("test".to_string());
        module.add_exportable_word(Arc::new(PushValueWord::new(
            "TEST_WORD".to_string(),
            ForthicValue::String("test".to_string()),
        )));

        interp.register_module(module);

        // Module should be findable
        let found = interp.find_module("test").unwrap();
        assert_eq!(found.get_name(), "test");

        // Should be registered in app module
        assert!(interp.get_app_module().find_module("test").is_some());
    }

    // ========================================
    // Phase 4.7 Tests: Utility Methods
    // ========================================

    #[test]
    fn test_set_stack() {
        let mut interp = Interpreter::new("UTC");

        // Push some values
        interp.stack_push(ForthicValue::Int(1));
        interp.stack_push(ForthicValue::Int(2));
        assert_eq!(interp.get_stack().len(), 2);

        // Create a new stack and set it
        let mut new_stack = Stack::new();
        new_stack.push(ForthicValue::String("hello".to_string()));
        new_stack.push(ForthicValue::Bool(true));

        interp.set_stack(new_stack);

        // Stack should be replaced
        assert_eq!(interp.get_stack().len(), 2);
        assert_eq!(
            interp.stack_pop().unwrap(),
            ForthicValue::Bool(true)
        );
        assert_eq!(
            interp.stack_pop().unwrap(),
            ForthicValue::String("hello".to_string())
        );
    }

    #[test]
    fn test_get_tokenizer() {
        let mut interp = Interpreter::new("UTC");

        // No tokenizer initially
        assert!(interp.get_tokenizer().is_none());

        // Run some code (which pushes a tokenizer)
        interp.run("42").unwrap();

        // Still none after run completes (tokenizer is popped)
        assert!(interp.get_tokenizer().is_none());
    }

    #[test]
    fn test_get_top_input_string() {
        let interp = Interpreter::new("UTC");

        // Empty initially
        assert_eq!(interp.get_top_input_string(), "");

        // We can't easily test this with the current API since run() pops the tokenizer
        // This method is mainly useful during execution for error messages
    }

    #[test]
    fn test_tokenizer_access_during_execution() {
        // This test verifies that tokenizer methods work conceptually
        // In practice, they're used internally during word execution
        let mut interp = Interpreter::new("UTC");

        // Create a simple definition
        interp.run(": TEST 42 ;").unwrap();

        // Execute the definition
        interp.run("TEST").unwrap();

        // Should have the value on the stack
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(42));
    }

    #[test]
    fn test_stack_manipulation() {
        let mut interp = Interpreter::new("UTC");

        // Test stack_push
        interp.stack_push(ForthicValue::Int(10));
        interp.stack_push(ForthicValue::Int(20));
        interp.stack_push(ForthicValue::Int(30));

        // Test get_stack
        assert_eq!(interp.get_stack().len(), 3);

        // Test stack_peek
        assert_eq!(interp.stack_peek(), Some(&ForthicValue::Int(30)));

        // Test stack_pop
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(30));
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(20));
        assert_eq!(interp.get_stack().len(), 1);

        // Test get_stack_mut
        interp.get_stack_mut().clear();
        assert_eq!(interp.get_stack().len(), 0);
    }
}
