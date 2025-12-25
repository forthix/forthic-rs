use forthic::errors::ForthicError;
use forthic::literals::ForthicValue;
use forthic::module::{
    InterpreterContext, Module, ModuleWord, Word, WordErrorHandler,
};
use std::sync::{Arc, Mutex};

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
        // Not needed for tests
    }

    fn module_stack_pop(&mut self) -> Result<Module, ForthicError> {
        Err(ForthicError::StackUnderflow {
            forthic: "test".to_string(),
            location: None,
            cause: None,
        })
    }
}

// Helper: Create a word that throws an error
fn error_word(_context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
    Err(ForthicError::UnknownWord {
        forthic: "test".to_string(),
        word: "TEST".to_string(),
        location: None,
        cause: None,
    })
}

// Helper: Create a word that succeeds
fn success_word(_context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
    Ok(())
}

// Helper: Error handler that succeeds
struct SuccessHandler;
impl WordErrorHandler for SuccessHandler {
    fn handle(
        &self,
        _error: &ForthicError,
        _word_name: &str,
        _context: &mut dyn InterpreterContext,
    ) -> Result<(), ForthicError> {
        Ok(())
    }
}

// Helper: Error handler that fails
struct FailHandler;
impl WordErrorHandler for FailHandler {
    fn handle(
        &self,
        error: &ForthicError,
        _word_name: &str,
        _context: &mut dyn InterpreterContext,
    ) -> Result<(), ForthicError> {
        Err(ForthicError::UnknownWord {
            forthic: "test".to_string(),
            word: format!("Failed: {}", error),
            location: None,
            cause: None,
        })
    }
}

// Helper: Error handler that pushes a value to stack
struct StackPushHandler {
    value: i64,
}
impl WordErrorHandler for StackPushHandler {
    fn handle(
        &self,
        _error: &ForthicError,
        _word_name: &str,
        context: &mut dyn InterpreterContext,
    ) -> Result<(), ForthicError> {
        context.stack_push(ForthicValue::Int(self.value));
        Ok(())
    }
}

// Error Handler Registration Tests

#[test]
fn test_add_error_handler() {
    let word = ModuleWord::new("TEST".to_string(), error_word);
    let handler = Arc::new(SuccessHandler);

    word.add_error_handler(handler);
    assert_eq!(word.get_error_handlers().len(), 1);
}

#[test]
fn test_remove_error_handler() {
    let word = ModuleWord::new("TEST".to_string(), success_word);
    let handler: Arc<dyn WordErrorHandler> = Arc::new(SuccessHandler);

    word.add_error_handler(handler.clone());
    assert_eq!(word.get_error_handlers().len(), 1);

    word.remove_error_handler(&handler);
    assert_eq!(word.get_error_handlers().len(), 0);
}

#[test]
fn test_clear_error_handlers() {
    let word = ModuleWord::new("TEST".to_string(), success_word);

    word.add_error_handler(Arc::new(SuccessHandler));
    word.add_error_handler(Arc::new(SuccessHandler));
    word.add_error_handler(Arc::new(SuccessHandler));
    assert_eq!(word.get_error_handlers().len(), 3);

    word.clear_error_handlers();
    assert_eq!(word.get_error_handlers().len(), 0);
}

#[test]
fn test_remove_nonexistent_handler() {
    let word = ModuleWord::new("TEST".to_string(), success_word);
    let handler: Arc<dyn WordErrorHandler> = Arc::new(SuccessHandler);

    // Should not error when removing handler that doesn't exist
    word.remove_error_handler(&handler);
    assert_eq!(word.get_error_handlers().len(), 0);
}

// Error Handler Execution Tests

#[test]
fn test_handler_suppresses_error() {
    let word = ModuleWord::new("TEST".to_string(), error_word);
    word.add_error_handler(Arc::new(SuccessHandler));

    let mut ctx = MockContext::new();

    // Should not throw - error is suppressed
    let result = word.execute(&mut ctx);
    assert!(result.is_ok());
}

#[test]
fn test_handler_does_not_suppress_error() {
    let word = ModuleWord::new("TEST".to_string(), error_word);
    word.add_error_handler(Arc::new(FailHandler));

    let mut ctx = MockContext::new();

    // Should throw - handler re-raised error
    let result = word.execute(&mut ctx);
    assert!(result.is_err());
}

#[test]
fn test_multiple_handlers_first_succeeds() {
    let handler1_called = Arc::new(Mutex::new(false));
    let handler2_called = Arc::new(Mutex::new(false));

    let handler1_called_clone = handler1_called.clone();
    let handler2_called_clone = handler2_called.clone();

    struct TrackingHandler {
        called: Arc<Mutex<bool>>,
        succeed: bool,
    }
    impl WordErrorHandler for TrackingHandler {
        fn handle(
            &self,
            _error: &ForthicError,
            _word_name: &str,
            _context: &mut dyn InterpreterContext,
        ) -> Result<(), ForthicError> {
            *self.called.lock().unwrap() = true;
            if self.succeed {
                Ok(())
            } else {
                Err(ForthicError::UnknownWord {
                    forthic: "test".to_string(),
                    word: "TEST".to_string(),
                    location: None,
                    cause: None,
                })
            }
        }
    }

    let word = ModuleWord::new("TEST".to_string(), error_word);
    word.add_error_handler(Arc::new(TrackingHandler {
        called: handler1_called_clone,
        succeed: true,
    }));
    word.add_error_handler(Arc::new(TrackingHandler {
        called: handler2_called_clone,
        succeed: true,
    }));

    let mut ctx = MockContext::new();
    let result = word.execute(&mut ctx);

    assert!(result.is_ok());
    assert!(*handler1_called.lock().unwrap(), "First handler should be called");
    assert!(!*handler2_called.lock().unwrap(), "Second handler should not be called when first succeeds");
}

#[test]
fn test_multiple_handlers_first_fails() {
    let handler1_called = Arc::new(Mutex::new(false));
    let handler2_called = Arc::new(Mutex::new(false));

    let handler1_called_clone = handler1_called.clone();
    let handler2_called_clone = handler2_called.clone();

    struct TrackingHandler {
        called: Arc<Mutex<bool>>,
        succeed: bool,
    }
    impl WordErrorHandler for TrackingHandler {
        fn handle(
            &self,
            _error: &ForthicError,
            _word_name: &str,
            _context: &mut dyn InterpreterContext,
        ) -> Result<(), ForthicError> {
            *self.called.lock().unwrap() = true;
            if self.succeed {
                Ok(())
            } else {
                Err(ForthicError::UnknownWord {
                    forthic: "test".to_string(),
                    word: "TEST".to_string(),
                    location: None,
                    cause: None,
                })
            }
        }
    }

    let word = ModuleWord::new("TEST".to_string(), error_word);
    word.add_error_handler(Arc::new(TrackingHandler {
        called: handler1_called_clone,
        succeed: false,
    }));
    word.add_error_handler(Arc::new(TrackingHandler {
        called: handler2_called_clone,
        succeed: true,
    }));

    let mut ctx = MockContext::new();
    let result = word.execute(&mut ctx);

    assert!(result.is_ok());
    assert!(*handler1_called.lock().unwrap(), "First handler should be called");
    assert!(*handler2_called.lock().unwrap(), "Second handler should be called when first fails");
}

#[test]
fn test_all_handlers_fail() {
    let handler1_called = Arc::new(Mutex::new(false));
    let handler2_called = Arc::new(Mutex::new(false));

    let handler1_called_clone = handler1_called.clone();
    let handler2_called_clone = handler2_called.clone();

    struct TrackingHandler {
        called: Arc<Mutex<bool>>,
    }
    impl WordErrorHandler for TrackingHandler {
        fn handle(
            &self,
            _error: &ForthicError,
            _word_name: &str,
            _context: &mut dyn InterpreterContext,
        ) -> Result<(), ForthicError> {
            *self.called.lock().unwrap() = true;
            Err(ForthicError::UnknownWord {
                forthic: "test".to_string(),
                word: "TEST".to_string(),
                location: None,
                cause: None,
            })
        }
    }

    let word = ModuleWord::new("TEST".to_string(), error_word);
    word.add_error_handler(Arc::new(TrackingHandler {
        called: handler1_called_clone,
    }));
    word.add_error_handler(Arc::new(TrackingHandler {
        called: handler2_called_clone,
    }));

    let mut ctx = MockContext::new();
    let result = word.execute(&mut ctx);

    assert!(result.is_err());
    assert!(*handler1_called.lock().unwrap(), "First handler should be called");
    assert!(*handler2_called.lock().unwrap(), "Second handler should be called");
}

// IntentionalStopError Tests

#[test]
fn test_intentional_stop_error_bypasses_handlers() {
    let handler_called = Arc::new(Mutex::new(false));
    let handler_called_clone = handler_called.clone();

    struct TrackingHandler {
        called: Arc<Mutex<bool>>,
    }
    impl WordErrorHandler for TrackingHandler {
        fn handle(
            &self,
            _error: &ForthicError,
            _word_name: &str,
            _context: &mut dyn InterpreterContext,
        ) -> Result<(), ForthicError> {
            *self.called.lock().unwrap() = true;
            Ok(())
        }
    }

    fn intentional_stop_word(_context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        Err(ForthicError::IntentionalStop {
            message: "Intentional stop".to_string(),
        })
    }

    let word = ModuleWord::new("TEST".to_string(), intentional_stop_word);
    word.add_error_handler(Arc::new(TrackingHandler {
        called: handler_called_clone,
    }));

    let mut ctx = MockContext::new();
    let result = word.execute(&mut ctx);

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ForthicError::IntentionalStop { .. }));
    assert!(!*handler_called.lock().unwrap(), "Handler should not be called for IntentionalStopError");
}

// Integration Tests

#[test]
fn test_error_handler_accesses_stack() {
    let word = ModuleWord::new("TEST".to_string(), error_word);
    word.add_error_handler(Arc::new(StackPushHandler { value: 42 }));

    let mut ctx = MockContext::new();
    let result = word.execute(&mut ctx);

    assert!(result.is_ok());
    assert_eq!(ctx.stack.len(), 1);
    assert_eq!(ctx.stack[0], ForthicValue::Int(42));
}

#[test]
fn test_error_handler_can_modify_error() {
    let word = ModuleWord::new("TEST".to_string(), error_word);
    word.add_error_handler(Arc::new(StackPushHandler { value: 999 }));

    let mut ctx = MockContext::new();
    let result = word.execute(&mut ctx);

    assert!(result.is_ok());
    assert_eq!(ctx.stack.len(), 1);
    assert_eq!(ctx.stack[0], ForthicValue::Int(999));
}
