use forthic::literals::ForthicValue;
use forthic::modules::standard::CoreModule;
use forthic::module::{InterpreterContext, Module};

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

    fn stack_pop(&mut self) -> Result<ForthicValue, forthic::ForthicError> {
        self.stack.pop().ok_or(forthic::ForthicError::StackUnderflow {
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

    fn module_stack_push(&mut self, _module: Module) {}

    fn module_stack_pop(&mut self) -> Result<Module, forthic::ForthicError> {
        Err(forthic::ForthicError::StackUnderflow {
            forthic: "test".to_string(),
            location: None,
            cause: None,
        })
    }
}

// Stack Operation Tests

#[test]
fn test_pop() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("POP").unwrap();
    ctx.stack.push(ForthicValue::Int(5));
    ctx.stack.push(ForthicValue::Int(10));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.len(), 1);
    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(5)));
}

#[test]
fn test_dup() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("DUP").unwrap();
    ctx.stack.push(ForthicValue::Int(42));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.len(), 2);
    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(42)));
    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(42)));
}

#[test]
fn test_swap() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("SWAP").unwrap();
    ctx.stack.push(ForthicValue::Int(1));
    ctx.stack.push(ForthicValue::Int(2));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(1)));
    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(2)));
}

// Variable Tests

#[test]
fn test_variables() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("VARIABLES").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::String("count".to_string()),
        ForthicValue::String("total".to_string()),
    ]));
    word.execute(&mut ctx).unwrap();

    assert!(ctx.module.get_variable("count").is_some());
    assert!(ctx.module.get_variable("total").is_some());
}

#[test]
fn test_store_and_fetch() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    // Store value
    let store_word = module.module().find_word("!").unwrap();
    ctx.stack.push(ForthicValue::Int(42));
    ctx.stack.push(ForthicValue::String("count".to_string()));
    store_word.execute(&mut ctx).unwrap();

    // Fetch value
    let fetch_word = module.module().find_word("@").unwrap();
    ctx.stack.push(ForthicValue::String("count".to_string()));
    fetch_word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(42)));
}

#[test]
fn test_store_fetch() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("!@").unwrap();
    ctx.stack.push(ForthicValue::Int(99));
    ctx.stack.push(ForthicValue::String("value".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(99)));
    // Verify variable was set
    let var = ctx.module.get_variable("value").unwrap();
    assert_eq!(var.get_value(), &ForthicValue::Int(99));
}

#[test]
fn test_invalid_variable_name() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("VARIABLES").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::String("__invalid".to_string()),
    ]));

    let result = word.execute(&mut ctx);
    assert!(result.is_err());
}

// Control Flow Tests

#[test]
fn test_identity() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("IDENTITY").unwrap();
    ctx.stack.push(ForthicValue::Int(5));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.len(), 1);
    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(5)));
}

#[test]
fn test_nop() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("NOP").unwrap();
    ctx.stack.push(ForthicValue::Int(5));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.len(), 1);
    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(5)));
}

#[test]
fn test_null() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("NULL").unwrap();
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Null));
}

#[test]
fn test_is_array_true() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("ARRAY?").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

#[test]
fn test_is_array_false() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("ARRAY?").unwrap();
    ctx.stack.push(ForthicValue::Int(42));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(false)));
}

#[test]
fn test_default_with_null() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("DEFAULT").unwrap();
    ctx.stack.push(ForthicValue::Null);
    ctx.stack.push(ForthicValue::Int(99));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(99)));
}

#[test]
fn test_default_with_value() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("DEFAULT").unwrap();
    ctx.stack.push(ForthicValue::Int(42));
    ctx.stack.push(ForthicValue::Int(99));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(42)));
}

#[test]
fn test_default_with_empty_string() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("DEFAULT").unwrap();
    ctx.stack.push(ForthicValue::String("".to_string()));
    ctx.stack.push(ForthicValue::String("default".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("default".to_string())));
}

// Options Tests

#[test]
fn test_to_options() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("~>").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::String("key1".to_string()),
        ForthicValue::Int(42),
        ForthicValue::String("key2".to_string()),
        ForthicValue::Bool(true),
    ]));
    word.execute(&mut ctx).unwrap();

    // Verify we got a WordOptions value
    let result = ctx.stack.pop();
    assert!(matches!(result, Some(ForthicValue::WordOptions(_))));
}

#[test]
fn test_to_options_invalid() {
    let module = CoreModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("~>").unwrap();
    ctx.stack.push(ForthicValue::Int(42));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Null));
}
