use forthic::literals::ForthicValue;
use forthic::modules::standard::ArrayModule;
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

// Access Tests

#[test]
fn test_length_array() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("LENGTH").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(3)));
}

#[test]
fn test_nth() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("NTH").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(10),
        ForthicValue::Int(20),
        ForthicValue::Int(30),
    ]));
    ctx.stack.push(ForthicValue::Int(1));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(20)));
}

#[test]
fn test_nth_out_of_bounds() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("NTH").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(10),
        ForthicValue::Int(20),
    ]));
    ctx.stack.push(ForthicValue::Int(5));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Null));
}

#[test]
fn test_last() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("LAST").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(10),
        ForthicValue::Int(20),
        ForthicValue::Int(30),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(30)));
}

#[test]
fn test_slice() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("SLICE").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(0),
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
        ForthicValue::Int(4),
    ]));
    ctx.stack.push(ForthicValue::Int(1));
    ctx.stack.push(ForthicValue::Int(3));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], ForthicValue::Int(1));
        assert_eq!(arr[1], ForthicValue::Int(2));
        assert_eq!(arr[2], ForthicValue::Int(3));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_take() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("TAKE").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
        ForthicValue::Int(4),
    ]));
    ctx.stack.push(ForthicValue::Int(2));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], ForthicValue::Int(1));
        assert_eq!(arr[1], ForthicValue::Int(2));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_drop() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("DROP").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
        ForthicValue::Int(4),
    ]));
    ctx.stack.push(ForthicValue::Int(2));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], ForthicValue::Int(3));
        assert_eq!(arr[1], ForthicValue::Int(4));
    } else {
        panic!("Expected array");
    }
}

// Transform Tests

#[test]
fn test_reverse() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("REVERSE").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
    ]));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr[0], ForthicValue::Int(3));
        assert_eq!(arr[1], ForthicValue::Int(2));
        assert_eq!(arr[2], ForthicValue::Int(1));
    } else {
        panic!("Expected array");
    }
}

// Combine Tests

#[test]
fn test_append() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("APPEND").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
    ]));
    ctx.stack.push(ForthicValue::Int(3));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[2], ForthicValue::Int(3));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_concat() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("CONCAT").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
    ]));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(3),
        ForthicValue::Int(4),
    ]));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0], ForthicValue::Int(1));
        assert_eq!(arr[3], ForthicValue::Int(4));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_zip() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("ZIP").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
    ]));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::String("a".to_string()),
        ForthicValue::String("b".to_string()),
    ]));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 2);
        if let ForthicValue::Array(ref pair) = arr[0] {
            assert_eq!(pair[0], ForthicValue::Int(1));
            assert_eq!(pair[1], ForthicValue::String("a".to_string()));
        } else {
            panic!("Expected array pair");
        }
    } else {
        panic!("Expected array");
    }
}

// Filter Tests

#[test]
fn test_unique() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("UNIQUE").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(1),
        ForthicValue::Int(3),
        ForthicValue::Int(2),
    ]));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], ForthicValue::Int(1));
        assert_eq!(arr[1], ForthicValue::Int(2));
        assert_eq!(arr[2], ForthicValue::Int(3));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_difference() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("DIFFERENCE").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
    ]));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(2),
        ForthicValue::Int(4),
    ]));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], ForthicValue::Int(1));
        assert_eq!(arr[1], ForthicValue::Int(3));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_intersection() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("INTERSECTION").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
    ]));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(2),
        ForthicValue::Int(3),
        ForthicValue::Int(4),
    ]));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], ForthicValue::Int(2));
        assert_eq!(arr[1], ForthicValue::Int(3));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_union() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("UNION").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
    ]));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(2),
        ForthicValue::Int(3),
    ]));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 3);
        // Order: 1, 2, 3 (first occurrence wins for duplicates)
    } else {
        panic!("Expected array");
    }
}

// Utility Tests

#[test]
fn test_flatten() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("FLATTEN").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Array(vec![ForthicValue::Int(1), ForthicValue::Int(2)]),
        ForthicValue::Array(vec![ForthicValue::Int(3), ForthicValue::Int(4)]),
    ]));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0], ForthicValue::Int(1));
        assert_eq!(arr[3], ForthicValue::Int(4));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_range_ascending() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("RANGE").unwrap();
    ctx.stack.push(ForthicValue::Int(1));
    ctx.stack.push(ForthicValue::Int(5));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 5);
        assert_eq!(arr[0], ForthicValue::Int(1));
        assert_eq!(arr[4], ForthicValue::Int(5));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_range_descending() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("RANGE").unwrap();
    ctx.stack.push(ForthicValue::Int(5));
    ctx.stack.push(ForthicValue::Int(1));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 5);
        assert_eq!(arr[0], ForthicValue::Int(5));
        assert_eq!(arr[4], ForthicValue::Int(1));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_unpack() {
    let module = ArrayModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("UNPACK").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.len(), 3);
    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(3)));
    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(2)));
    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(1)));
}
