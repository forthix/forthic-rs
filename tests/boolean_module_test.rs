use forthic::literals::ForthicValue;
use forthic::modules::standard::BooleanModule;
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

// Comparison Tests

#[test]
fn test_equals() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    // Find and execute ==
    let word = module.module().find_word("==").unwrap();
    ctx.stack.push(ForthicValue::Int(5));
    ctx.stack.push(ForthicValue::Int(5));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

#[test]
fn test_not_equals() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("!=").unwrap();
    ctx.stack.push(ForthicValue::Int(5));
    ctx.stack.push(ForthicValue::Int(3));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

#[test]
fn test_less_than() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("<").unwrap();
    ctx.stack.push(ForthicValue::Int(3));
    ctx.stack.push(ForthicValue::Int(5));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

#[test]
fn test_greater_than() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">").unwrap();
    ctx.stack.push(ForthicValue::Int(5));
    ctx.stack.push(ForthicValue::Int(3));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

// Logic Tests

#[test]
fn test_or_two_values() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("OR").unwrap();
    ctx.stack.push(ForthicValue::Bool(true));
    ctx.stack.push(ForthicValue::Bool(false));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

#[test]
fn test_or_array() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("OR").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Bool(false),
        ForthicValue::Bool(true),
        ForthicValue::Bool(false),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

#[test]
fn test_and_two_values() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("AND").unwrap();
    ctx.stack.push(ForthicValue::Bool(true));
    ctx.stack.push(ForthicValue::Bool(true));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

#[test]
fn test_and_array() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("AND").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Bool(true),
        ForthicValue::Bool(true),
        ForthicValue::Bool(true),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

#[test]
fn test_not() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("NOT").unwrap();
    ctx.stack.push(ForthicValue::Bool(true));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(false)));
}

#[test]
fn test_xor() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("XOR").unwrap();
    ctx.stack.push(ForthicValue::Bool(true));
    ctx.stack.push(ForthicValue::Bool(false));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

// Membership Tests

#[test]
fn test_in() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("IN").unwrap();
    ctx.stack.push(ForthicValue::Int(2));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

#[test]
fn test_any() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("ANY").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
    ]));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(2),
        ForthicValue::Int(3),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

#[test]
fn test_all() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("ALL").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
    ]));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

// Conversion Tests

#[test]
fn test_to_bool() {
    let module = BooleanModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">BOOL").unwrap();
    ctx.stack.push(ForthicValue::Int(42));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));

    // Test with 0
    ctx.stack.push(ForthicValue::Int(0));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(false)));
}
