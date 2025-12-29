use forthic::literals::ForthicValue;
use forthic::modules::standard::MathModule;
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

// Arithmetic Tests

#[test]
fn test_plus_two_numbers() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("+").unwrap();
    ctx.stack.push(ForthicValue::Int(3));
    ctx.stack.push(ForthicValue::Int(5));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(8)));
}

#[test]
fn test_plus_array() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("+").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(6)));
}

#[test]
fn test_minus() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("-").unwrap();
    ctx.stack.push(ForthicValue::Int(10));
    ctx.stack.push(ForthicValue::Int(3));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(7)));
}

#[test]
fn test_times_two_numbers() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("*").unwrap();
    ctx.stack.push(ForthicValue::Int(3));
    ctx.stack.push(ForthicValue::Int(4));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(12)));
}

#[test]
fn test_times_array() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("*").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(2),
        ForthicValue::Int(3),
        ForthicValue::Int(4),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(24)));
}

#[test]
fn test_divide() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("/").unwrap();
    ctx.stack.push(ForthicValue::Int(10));
    ctx.stack.push(ForthicValue::Int(2));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(5)));
}

#[test]
fn test_divide_by_zero() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("/").unwrap();
    ctx.stack.push(ForthicValue::Int(10));
    ctx.stack.push(ForthicValue::Int(0));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Null));
}

#[test]
fn test_mod() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("MOD").unwrap();
    ctx.stack.push(ForthicValue::Int(10));
    ctx.stack.push(ForthicValue::Int(3));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(1)));
}

// Aggregate Tests

#[test]
fn test_sum() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("SUM").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
        ForthicValue::Int(4),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(10)));
}

#[test]
fn test_max_array() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("MAX").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(5),
        ForthicValue::Int(3),
        ForthicValue::Int(2),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(5)));
}

#[test]
fn test_max_two_values() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("MAX").unwrap();
    ctx.stack.push(ForthicValue::Int(3));
    ctx.stack.push(ForthicValue::Int(7));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(7)));
}

#[test]
fn test_min_array() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("MIN").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(5),
        ForthicValue::Int(1),
        ForthicValue::Int(3),
        ForthicValue::Int(2),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(1)));
}

#[test]
fn test_min_two_values() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("MIN").unwrap();
    ctx.stack.push(ForthicValue::Int(3));
    ctx.stack.push(ForthicValue::Int(7));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(3)));
}

#[test]
fn test_mean() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("MEAN").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(2),
        ForthicValue::Int(4),
        ForthicValue::Int(6),
        ForthicValue::Int(8),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(5)));
}

// Conversion Tests

#[test]
fn test_to_int_from_float() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">INT").unwrap();
    ctx.stack.push(ForthicValue::Float(3.7));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(3)));
}

#[test]
fn test_to_int_from_string() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">INT").unwrap();
    ctx.stack.push(ForthicValue::String("42".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(42)));
}

#[test]
fn test_to_float_from_int() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">FLOAT").unwrap();
    ctx.stack.push(ForthicValue::Int(5));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Float(5.0)));
}

#[test]
fn test_round() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("ROUND").unwrap();
    ctx.stack.push(ForthicValue::Float(3.7));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(4)));
}

// Math Function Tests

#[test]
fn test_abs_positive() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("ABS").unwrap();
    ctx.stack.push(ForthicValue::Int(5));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(5)));
}

#[test]
fn test_abs_negative() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("ABS").unwrap();
    ctx.stack.push(ForthicValue::Int(-5));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(5)));
}

#[test]
fn test_floor() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("FLOOR").unwrap();
    ctx.stack.push(ForthicValue::Float(3.7));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(3)));
}

#[test]
fn test_ceil() {
    let module = MathModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("CEIL").unwrap();
    ctx.stack.push(ForthicValue::Float(3.2));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(4)));
}
