//! Module system for Forthic
//!
//! This module provides the core abstractions for organizing Forthic code:
//! - **Variable**: Named mutable value containers
//! - **Word**: Executable units (trait and implementations)
//! - **Module**: Containers for words, variables, and imported modules
//! - **WordErrorHandler**: Per-word error handling
//!
//! # Word Types
//!
//! - **PushValueWord**: Pushes a literal value onto the stack
//! - **DefinitionWord**: User-defined word composed of other words
//! - **ModuleMemoWord**: Memoized word that caches its result
//! - **ModuleMemoBangWord**: Forces refresh of a memoized word
//! - **ModuleMemoBangAtWord**: Refreshes and returns memoized value
//! - **ExecuteWord**: Wrapper that executes another word (for prefixed imports)
//! - **ModuleWord**: Word with integrated per-word error handling support
//!
//! # Module Features
//!
//! - Word and variable management
//! - Module importing with optional prefixes
//! - Exportable word lists for controlled visibility
//! - Module duplication for isolated execution contexts
//! - Per-word error handlers with automatic retry logic

use crate::errors::{CodeLocation, ForthicError};
use crate::literals::ForthicValue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Forward declaration - Interpreter will be defined in interpreter.rs
// We use a trait to avoid circular dependencies
pub trait InterpreterContext {
    fn stack_push(&mut self, value: ForthicValue);
    fn stack_pop(&mut self) -> Result<ForthicValue, ForthicError>;
    fn stack_peek(&self) -> Option<&ForthicValue>;
    fn cur_module(&self) -> &Module;
    fn cur_module_mut(&mut self) -> &mut Module;
    fn get_app_module(&self) -> &Module;
    fn module_stack_push(&mut self, module: Module);
    fn module_stack_pop(&mut self) -> Result<Module, ForthicError>;
}

/// Word error handler trait - handles errors during word execution
///
/// Error handlers can suppress errors by returning Ok, or propagate them by returning Err.
/// Multiple handlers can be attached to a single word and are tried in order.
pub trait WordErrorHandler: Send + Sync {
    /// Handle an error that occurred during word execution
    ///
    /// # Arguments
    /// * `error` - The error that occurred
    /// * `word_name` - Name of the word that generated the error
    /// * `context` - Interpreter context for stack manipulation
    ///
    /// # Returns
    /// * `Ok(())` - Handler successfully handled the error (error is suppressed)
    /// * `Err(error)` - Handler did not handle the error (try next handler or propagate)
    fn handle(
        &self,
        error: &ForthicError,
        word_name: &str,
        context: &mut dyn InterpreterContext,
    ) -> Result<(), ForthicError>;
}

// Type alias for word executor functions
pub type WordExecutor = fn(&mut dyn InterpreterContext) -> Result<(), ForthicError>;

/// Variable - Named mutable value container
///
/// Represents a variable that can store and retrieve values within a module scope.
///
/// # Examples
///
/// ```
/// use forthic::module::Variable;
/// use forthic::literals::ForthicValue;
///
/// let mut var = Variable::new("counter".to_string(), ForthicValue::Int(0));
/// assert_eq!(var.get_value(), &ForthicValue::Int(0));
///
/// var.set_value(ForthicValue::Int(42));
/// assert_eq!(var.get_value(), &ForthicValue::Int(42));
/// ```
#[derive(Debug, Clone)]
pub struct Variable {
    name: String,
    value: ForthicValue,
}

impl Variable {
    /// Create a new variable with a name and initial value
    pub fn new(name: String, value: ForthicValue) -> Self {
        Self { name, value }
    }

    /// Get the variable name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set the variable value
    pub fn set_value(&mut self, value: ForthicValue) {
        self.value = value;
    }

    /// Get a reference to the variable value
    pub fn get_value(&self) -> &ForthicValue {
        &self.value
    }

    /// Duplicate the variable
    pub fn dup(&self) -> Self {
        Self {
            name: self.name.clone(),
            value: self.value.clone(),
        }
    }
}

/// Word trait - Base abstraction for all executable words in Forthic
///
/// A word is the fundamental unit of execution. When interpreted,
/// it performs an action (typically manipulating the stack or control flow).
pub trait Word: Send + Sync {
    /// Get the word name
    fn name(&self) -> &str;

    /// Get the word's source string representation
    fn string(&self) -> &str {
        self.name()
    }

    /// Get the word's code location (where it was defined)
    fn location(&self) -> Option<&CodeLocation> {
        None
    }

    /// Set the word's code location
    fn set_location(&mut self, _location: CodeLocation) {
        // Default implementation does nothing - override in concrete types if needed
    }

    /// Execute the word (will be async in full implementation)
    ///
    /// Note: For Phase 3, we'll use a simplified synchronous version.
    /// The full interpreter in Phase 4 will make this async.
    fn execute(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError>;

    /// Check if this word is a memo word
    fn is_memo(&self) -> bool {
        false
    }
}

/// PushValueWord - Word that pushes a value onto the stack
///
/// Executes by pushing its stored value onto the interpreter's stack.
/// Used for literals, variables, and constants.
#[derive(Debug, Clone)]
pub struct PushValueWord {
    name: String,
    value: ForthicValue,
    location: Option<CodeLocation>,
}

impl PushValueWord {
    pub fn new(name: String, value: ForthicValue) -> Self {
        Self {
            name,
            value,
            location: None,
        }
    }
}

impl Word for PushValueWord {
    fn name(&self) -> &str {
        &self.name
    }

    fn location(&self) -> Option<&CodeLocation> {
        self.location.as_ref()
    }

    fn set_location(&mut self, location: CodeLocation) {
        self.location = Some(location);
    }

    fn execute(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.stack_push(self.value.clone());
        Ok(())
    }
}

/// DefinitionWord - User-defined word composed of other words
///
/// Represents a word defined in Forthic code using `:`
/// Contains a sequence of words that are executed in order.
#[derive(Clone)]
pub struct DefinitionWord {
    name: String,
    words: Vec<Arc<dyn Word>>,
    location: Option<CodeLocation>,
}

impl DefinitionWord {
    pub fn new(name: String) -> Self {
        Self {
            name,
            words: Vec::new(),
            location: None,
        }
    }

    pub fn add_word(&mut self, word: Arc<dyn Word>) {
        self.words.push(word);
    }

    pub fn get_words(&self) -> &[Arc<dyn Word>] {
        &self.words
    }
}

impl Word for DefinitionWord {
    fn name(&self) -> &str {
        &self.name
    }

    fn location(&self) -> Option<&CodeLocation> {
        self.location.as_ref()
    }

    fn set_location(&mut self, location: CodeLocation) {
        self.location = Some(location);
    }

    fn execute(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        for word in &self.words {
            word.execute(context).map_err(|e| {
                ForthicError::WordExecution {
                    message: format!("Error executing {}", self.name),
                    inner_error: Box::new(e),
                    call_location: None,
                    definition_location: self.location.clone(),
                }
            })?;
        }
        Ok(())
    }
}

/// ModuleMemoWord - Memoized word that caches its result
///
/// Executes the wrapped word once and caches the result on the stack.
/// Subsequent calls return the cached value without re-executing.
/// Defined in Forthic using `@:`.
pub struct ModuleMemoWord {
    name: String,
    word: Arc<dyn Word>,
    has_value: std::sync::Mutex<bool>,
    value: std::sync::Mutex<Option<ForthicValue>>,
    location: Option<CodeLocation>,
}

impl ModuleMemoWord {
    pub fn new(word: Arc<dyn Word>) -> Self {
        let name = word.name().to_string();
        Self {
            name,
            word,
            has_value: std::sync::Mutex::new(false),
            value: std::sync::Mutex::new(None),
            location: None,
        }
    }

    pub fn refresh(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        self.word.execute(context)?;
        let val = context.stack_pop()?;

        let mut has_value = self.has_value.lock().unwrap();
        let mut value = self.value.lock().unwrap();

        *has_value = true;
        *value = Some(val);

        Ok(())
    }

    pub fn get_value(&self) -> Option<ForthicValue> {
        self.value.lock().unwrap().clone()
    }
}

impl Word for ModuleMemoWord {
    fn name(&self) -> &str {
        &self.name
    }

    fn location(&self) -> Option<&CodeLocation> {
        self.location.as_ref()
    }

    fn is_memo(&self) -> bool {
        true
    }

    fn execute(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let has_value = *self.has_value.lock().unwrap();

        if !has_value {
            self.refresh(context)?;
        }

        if let Some(val) = self.value.lock().unwrap().as_ref() {
            context.stack_push(val.clone());
        }

        Ok(())
    }
}

/// ModuleMemoBangWord - Forces refresh of a memoized word
///
/// Re-executes the memoized word and updates its cached value.
/// Named with a `!` suffix (e.g., `WORD!` for a memo word named `WORD`).
/// Does not push the new value onto the stack.
pub struct ModuleMemoBangWord {
    name: String,
    memo_word: Arc<ModuleMemoWord>,
}

impl ModuleMemoBangWord {
    pub fn new(memo_word: Arc<ModuleMemoWord>) -> Self {
        let name = format!("{}!", memo_word.name());
        Self { name, memo_word }
    }
}

impl Word for ModuleMemoBangWord {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        self.memo_word.refresh(context)
    }
}

/// ModuleMemoBangAtWord - Refreshes a memoized word and returns its value
///
/// Re-executes the memoized word, updates its cached value, and pushes the new value onto the stack.
/// Named with a `!@` suffix (e.g., `WORD!@` for a memo word named `WORD`).
pub struct ModuleMemoBangAtWord {
    name: String,
    memo_word: Arc<ModuleMemoWord>,
}

impl ModuleMemoBangAtWord {
    pub fn new(memo_word: Arc<ModuleMemoWord>) -> Self {
        let name = format!("{}!@", memo_word.name());
        Self { name, memo_word }
    }
}

impl Word for ModuleMemoBangAtWord {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        self.memo_word.refresh(context)?;
        if let Some(val) = self.memo_word.get_value() {
            context.stack_push(val);
        }
        Ok(())
    }
}

/// ExecuteWord - Wrapper word that executes another word
///
/// Delegates execution to a target word. Used for prefixed module imports
/// to create words like `prefix.word` that execute the original word from the imported module.
#[derive(Clone)]
pub struct ExecuteWord {
    name: String,
    target_word: Arc<dyn Word>,
}

impl ExecuteWord {
    pub fn new(name: String, target_word: Arc<dyn Word>) -> Self {
        Self { name, target_word }
    }
}

impl Word for ExecuteWord {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        self.target_word.execute(context)
    }
}

/// ModuleWord - Word that executes a handler with error handling support
///
/// Used to create module words with integrated per-word error handling.
/// Wraps execution in error handling logic, trying handlers in order if an error occurs.
/// IntentionalStop errors bypass error handlers (used for flow control).
pub struct ModuleWord {
    name: String,
    handler: WordExecutor,
    error_handlers: Mutex<Vec<Arc<dyn WordErrorHandler>>>,
    location: Option<CodeLocation>,
}

impl ModuleWord {
    /// Create a new ModuleWord with a given name and handler function
    pub fn new(name: String, handler: WordExecutor) -> Self {
        Self {
            name,
            handler,
            error_handlers: Mutex::new(Vec::new()),
            location: None,
        }
    }

    /// Add an error handler to this word
    pub fn add_error_handler(&self, handler: Arc<dyn WordErrorHandler>) {
        self.error_handlers.lock().unwrap().push(handler);
    }

    /// Remove an error handler (requires PartialEq, so we compare Arc pointers)
    pub fn remove_error_handler(&self, handler: &Arc<dyn WordErrorHandler>) {
        let mut handlers = self.error_handlers.lock().unwrap();
        if let Some(pos) = handlers.iter().position(|h| Arc::ptr_eq(h, handler)) {
            handlers.remove(pos);
        }
    }

    /// Clear all error handlers
    pub fn clear_error_handlers(&self) {
        self.error_handlers.lock().unwrap().clear();
    }

    /// Get a copy of all error handlers (for testing)
    pub fn get_error_handlers(&self) -> Vec<Arc<dyn WordErrorHandler>> {
        self.error_handlers.lock().unwrap().clone()
    }

    /// Try error handlers in order until one succeeds
    ///
    /// Returns true if any handler successfully handled the error
    fn try_error_handlers(
        &self,
        error: &ForthicError,
        context: &mut dyn InterpreterContext,
    ) -> bool {
        let handlers = self.error_handlers.lock().unwrap().clone();
        for handler in handlers {
            if handler.handle(error, &self.name, context).is_ok() {
                return true; // Handler succeeded
            }
            // Handler failed, try next one
        }
        false // No handler succeeded
    }
}

impl Word for ModuleWord {
    fn name(&self) -> &str {
        &self.name
    }

    fn location(&self) -> Option<&CodeLocation> {
        self.location.as_ref()
    }

    fn set_location(&mut self, location: CodeLocation) {
        self.location = Some(location);
    }

    fn execute(&self, context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        match (self.handler)(context) {
            Ok(()) => Ok(()),
            Err(ForthicError::IntentionalStop { .. }) => {
                // Never handle intentional flow control errors
                Err(ForthicError::IntentionalStop {
                    message: "Intentional stop".to_string(),
                })
            }
            Err(e) => {
                // Try error handlers
                let handled = self.try_error_handlers(&e, context);
                if handled {
                    Ok(()) // Error was handled, execution continues
                } else {
                    Err(e) // Re-raise if not handled
                }
            }
        }
    }
}

/// Module - Container for words, variables, and imported modules
///
/// Modules provide namespacing and code organization in Forthic.
/// Each module maintains its own dictionary of words, variables, and imported modules.
///
/// # Examples
///
/// ```
/// use forthic::module::Module;
///
/// let mut module = Module::new("my_module".to_string());
/// assert_eq!(module.get_name(), "my_module");
/// ```
#[derive(Clone)]
pub struct Module {
    name: String,
    words: Vec<Arc<dyn Word>>,
    exportable: Vec<String>,
    variables: HashMap<String, Variable>,
    modules: HashMap<String, Module>,
    module_prefixes: HashMap<String, Vec<String>>,
    forthic_code: String,
}

impl Module {
    /// Create a new module with the given name
    pub fn new(name: String) -> Self {
        Self {
            name,
            words: Vec::new(),
            exportable: Vec::new(),
            variables: HashMap::new(),
            modules: HashMap::new(),
            module_prefixes: HashMap::new(),
            forthic_code: String::new(),
        }
    }

    /// Create a new module with name and forthic code
    pub fn new_with_code(name: String, forthic_code: String) -> Self {
        Self {
            name,
            words: Vec::new(),
            exportable: Vec::new(),
            variables: HashMap::new(),
            modules: HashMap::new(),
            module_prefixes: HashMap::new(),
            forthic_code,
        }
    }

    /// Get the module name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set the Forthic code for this module
    pub fn set_forthic_code(&mut self, code: String) {
        self.forthic_code = code;
    }

    /// Get the Forthic code for this module
    pub fn get_forthic_code(&self) -> &str {
        &self.forthic_code
    }

    // ---- Word management ----

    /// Add a word to the module
    pub fn add_word(&mut self, word: Arc<dyn Word>) {
        self.words.push(word);
    }

    /// Add a memoized word and its refresh variants (!word and !@word)
    ///
    /// Returns the Arc<ModuleMemoWord> for potential further use
    pub fn add_memo_words(&mut self, word: Arc<dyn Word>) -> Arc<ModuleMemoWord> {
        let memo_word = Arc::new(ModuleMemoWord::new(word));
        let bang_word = Arc::new(ModuleMemoBangWord::new(Arc::clone(&memo_word)));
        let bang_at_word = Arc::new(ModuleMemoBangAtWord::new(Arc::clone(&memo_word)));

        self.words.push(memo_word.clone());
        self.words.push(bang_word);
        self.words.push(bang_at_word);

        memo_word
    }

    /// Add a word to the exportable list
    pub fn add_exportable(&mut self, names: Vec<String>) {
        self.exportable.extend(names);
    }

    /// Add a word and mark it as exportable
    pub fn add_exportable_word(&mut self, word: Arc<dyn Word>) {
        let name = word.name().to_string();
        self.words.push(word);
        self.exportable.push(name);
    }

    /// Get all exportable words
    pub fn exportable_words(&self) -> Vec<Arc<dyn Word>> {
        self.words
            .iter()
            .filter(|w| self.exportable.contains(&w.name().to_string()))
            .cloned()
            .collect()
    }

    /// Find a word by name (searches dictionary then variables)
    pub fn find_word(&self, name: &str) -> Option<Arc<dyn Word>> {
        // First check dictionary words
        if let Some(word) = self.find_dictionary_word(name) {
            return Some(word);
        }

        // Then check variables
        self.find_variable(name)
    }

    /// Find a word in the word dictionary (not variables)
    pub fn find_dictionary_word(&self, word_name: &str) -> Option<Arc<dyn Word>> {
        // Search backwards to find most recently defined word
        self.words
            .iter()
            .rev()
            .find(|w| w.name() == word_name)
            .cloned()
    }

    /// Find a variable and return it as a PushValueWord
    pub fn find_variable(&self, varname: &str) -> Option<Arc<dyn Word>> {
        self.variables.get(varname).map(|var| {
            Arc::new(PushValueWord::new(
                varname.to_string(),
                var.get_value().clone(),
            )) as Arc<dyn Word>
        })
    }

    // ---- Variable management ----

    /// Add a variable to the module
    pub fn add_variable(&mut self, name: String, value: ForthicValue) {
        if !self.variables.contains_key(&name) {
            self.variables.insert(name.clone(), Variable::new(name, value));
        }
    }

    /// Get a variable by name
    pub fn get_variable(&self, name: &str) -> Option<&Variable> {
        self.variables.get(name)
    }

    /// Get a mutable reference to a variable
    pub fn get_variable_mut(&mut self, name: &str) -> Option<&mut Variable> {
        self.variables.get_mut(name)
    }

    // ---- Module management ----

    /// Find a module by name
    pub fn find_module(&self, name: &str) -> Option<&Module> {
        self.modules.get(name)
    }

    /// Register a module with a prefix
    pub fn register_module(&mut self, module_name: String, prefix: String, module: Module) {
        self.modules.insert(module_name.clone(), module);

        self.module_prefixes
            .entry(module_name)
            .or_insert_with(Vec::new)
            .push(prefix);
    }

    /// Import a module with optional prefix
    ///
    /// If prefix is empty, words are imported directly.
    /// If prefix is provided, words are imported as `prefix.word_name`.
    pub fn import_module(&mut self, prefix: &str, module: &Module) {
        let new_module = module.dup();
        let words = new_module.exportable_words();

        for word in words {
            if prefix.is_empty() {
                // Unprefixed import - add word directly
                self.add_word(word);
            } else {
                // Prefixed import - create ExecuteWord with prefix
                let prefixed_name = format!("{}.{}", prefix, word.name());
                let prefixed_word = Arc::new(ExecuteWord::new(prefixed_name, word));
                self.add_word(prefixed_word);
            }
        }

        self.register_module(new_module.get_name().to_string(), prefix.to_string(), new_module);
    }

    /// Duplicate the module (shallow copy of words, deep copy of variables)
    pub fn dup(&self) -> Self {
        let mut result = Module::new(self.name.clone());

        result.words = self.words.clone();
        result.exportable = self.exportable.clone();

        // Deep copy variables
        for (key, var) in &self.variables {
            result.variables.insert(key.clone(), var.dup());
        }

        // Shallow copy modules
        result.modules = self.modules.clone();
        result.forthic_code = self.forthic_code.clone();

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock interpreter context for testing
    struct MockContext {
        stack: Vec<ForthicValue>,
        module: Module,
    }

    impl MockContext {
        fn new() -> Self {
            Self {
                stack: Vec::new(),
                module: Module::new("test".to_string()),
            }
        }
    }

    impl InterpreterContext for MockContext {
        fn stack_push(&mut self, value: ForthicValue) {
            self.stack.push(value);
        }

        fn stack_pop(&mut self) -> Result<ForthicValue, ForthicError> {
            self.stack.pop().ok_or(ForthicError::StackUnderflow {
                forthic: "test".to_string(),
                location: None,
                cause: None,
            })
        }

        fn stack_peek(&self) -> Option<&ForthicValue> {
            self.stack.last()
        }

        fn cur_module(&self) -> &Module {
            &self.module
        }

        fn cur_module_mut(&mut self) -> &mut Module {
            &mut self.module
        }

        fn get_app_module(&self) -> &Module {
            &self.module
        }

        fn module_stack_push(&mut self, _module: Module) {
            // Not needed for basic tests
        }

        fn module_stack_pop(&mut self) -> Result<Module, ForthicError> {
            Err(ForthicError::StackUnderflow {
                forthic: "test".to_string(),
                location: None,
                cause: None,
            })
        }
    }

    #[test]
    fn test_variable() {
        let mut var = Variable::new("test".to_string(), ForthicValue::Int(42));
        assert_eq!(var.get_name(), "test");
        assert_eq!(var.get_value(), &ForthicValue::Int(42));

        var.set_value(ForthicValue::Int(99));
        assert_eq!(var.get_value(), &ForthicValue::Int(99));
    }

    #[test]
    fn test_variable_dup() {
        let var = Variable::new("test".to_string(), ForthicValue::Int(42));
        let var2 = var.dup();

        assert_eq!(var.get_name(), var2.get_name());
        assert_eq!(var.get_value(), var2.get_value());
    }

    #[test]
    fn test_push_value_word() {
        let word = PushValueWord::new("FORTY_TWO".to_string(), ForthicValue::Int(42));
        let mut ctx = MockContext::new();

        word.execute(&mut ctx).unwrap();
        assert_eq!(ctx.stack.len(), 1);
        assert_eq!(ctx.stack[0], ForthicValue::Int(42));
    }

    #[test]
    fn test_definition_word() {
        let mut def = DefinitionWord::new("TEST".to_string());
        def.add_word(Arc::new(PushValueWord::new(
            "ONE".to_string(),
            ForthicValue::Int(1),
        )));
        def.add_word(Arc::new(PushValueWord::new(
            "TWO".to_string(),
            ForthicValue::Int(2),
        )));

        let mut ctx = MockContext::new();
        def.execute(&mut ctx).unwrap();

        assert_eq!(ctx.stack.len(), 2);
        assert_eq!(ctx.stack[0], ForthicValue::Int(1));
        assert_eq!(ctx.stack[1], ForthicValue::Int(2));
    }

    #[test]
    fn test_module_new() {
        let module = Module::new("test".to_string());
        assert_eq!(module.get_name(), "test");
    }

    #[test]
    fn test_module_add_word() {
        let mut module = Module::new("test".to_string());
        let word = Arc::new(PushValueWord::new("WORD".to_string(), ForthicValue::Int(42)));

        module.add_word(word);
        assert!(module.find_word("WORD").is_some());
    }

    #[test]
    fn test_module_find_word() {
        let mut module = Module::new("test".to_string());
        let word = Arc::new(PushValueWord::new("WORD".to_string(), ForthicValue::Int(42)));

        module.add_word(word);

        let found = module.find_word("WORD");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "WORD");

        assert!(module.find_word("MISSING").is_none());
    }

    #[test]
    fn test_module_exportable_words() {
        let mut module = Module::new("test".to_string());

        let word1 = Arc::new(PushValueWord::new("PUBLIC".to_string(), ForthicValue::Int(1)));
        let word2 = Arc::new(PushValueWord::new("PRIVATE".to_string(), ForthicValue::Int(2)));

        module.add_exportable_word(word1);
        module.add_word(word2);

        let exportable = module.exportable_words();
        assert_eq!(exportable.len(), 1);
        assert_eq!(exportable[0].name(), "PUBLIC");
    }

    #[test]
    fn test_module_variables() {
        let mut module = Module::new("test".to_string());

        module.add_variable("var1".to_string(), ForthicValue::Int(42));
        assert!(module.get_variable("var1").is_some());
        assert_eq!(
            module.get_variable("var1").unwrap().get_value(),
            &ForthicValue::Int(42)
        );

        // Variables can be found as words
        let word = module.find_word("var1");
        assert!(word.is_some());
    }

    #[test]
    fn test_module_import_unprefixed() {
        let mut module1 = Module::new("module1".to_string());
        let word = Arc::new(PushValueWord::new("WORD".to_string(), ForthicValue::Int(42)));
        module1.add_exportable_word(word);

        let mut module2 = Module::new("module2".to_string());
        module2.import_module("", &module1);

        // Word should be accessible without prefix
        assert!(module2.find_word("WORD").is_some());
    }

    #[test]
    fn test_module_import_prefixed() {
        let mut module1 = Module::new("module1".to_string());
        let word = Arc::new(PushValueWord::new("WORD".to_string(), ForthicValue::Int(42)));
        module1.add_exportable_word(word);

        let mut module2 = Module::new("module2".to_string());
        module2.import_module("m1", &module1);

        // Word should be accessible with prefix
        assert!(module2.find_word("m1.WORD").is_some());
        assert!(module2.find_word("WORD").is_none());
    }

    #[test]
    fn test_execute_word() {
        let target = Arc::new(PushValueWord::new(
            "TARGET".to_string(),
            ForthicValue::Int(42),
        ));
        let exec = ExecuteWord::new("WRAPPER".to_string(), target);

        let mut ctx = MockContext::new();
        exec.execute(&mut ctx).unwrap();

        assert_eq!(ctx.stack.len(), 1);
        assert_eq!(ctx.stack[0], ForthicValue::Int(42));
    }

    #[test]
    fn test_memo_word() {
        let push_word = Arc::new(PushValueWord::new(
            "VALUE".to_string(),
            ForthicValue::Int(42),
        ));
        let memo = ModuleMemoWord::new(push_word);

        let mut ctx = MockContext::new();

        // First execution
        memo.execute(&mut ctx).unwrap();
        assert_eq!(ctx.stack.len(), 1);
        assert_eq!(ctx.stack[0], ForthicValue::Int(42));

        // Second execution should return cached value
        memo.execute(&mut ctx).unwrap();
        assert_eq!(ctx.stack.len(), 2);
        assert_eq!(ctx.stack[1], ForthicValue::Int(42));
    }

    #[test]
    fn test_module_dup() {
        let mut module = Module::new("test".to_string());
        module.add_variable("var".to_string(), ForthicValue::Int(42));

        let word = Arc::new(PushValueWord::new("WORD".to_string(), ForthicValue::Int(99)));
        module.add_word(word);

        let dup = module.dup();
        assert_eq!(dup.get_name(), "test");
        assert!(dup.find_word("WORD").is_some());
        assert!(dup.get_variable("var").is_some());
    }
}
