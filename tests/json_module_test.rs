use forthic::literals::ForthicValue;
use forthic::modules::standard::JSONModule;
use forthic::module::{InterpreterContext, Module};
use std::collections::HashMap;

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

// Conversion Tests

#[test]
fn test_to_json_null() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">JSON").unwrap();
    ctx.stack.push(ForthicValue::Null);
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("null".to_string())));
}

#[test]
fn test_to_json_bool() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">JSON").unwrap();
    ctx.stack.push(ForthicValue::Bool(true));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("true".to_string())));
}

#[test]
fn test_to_json_int() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">JSON").unwrap();
    ctx.stack.push(ForthicValue::Int(42));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("42".to_string())));
}

#[test]
fn test_to_json_float() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">JSON").unwrap();
    ctx.stack.push(ForthicValue::Float(3.14));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("3.14".to_string())));
}

#[test]
fn test_to_json_string() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">JSON").unwrap();
    ctx.stack.push(ForthicValue::String("hello".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("\"hello\"".to_string())));
}

#[test]
fn test_to_json_array() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">JSON").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Int(1),
        ForthicValue::Int(2),
        ForthicValue::Int(3),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("[1,2,3]".to_string())));
}

#[test]
fn test_to_json_record() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let mut rec = HashMap::new();
    rec.insert("name".to_string(), ForthicValue::String("Alice".to_string()));
    rec.insert("age".to_string(), ForthicValue::Int(30));

    let word = module.module().find_word(">JSON").unwrap();
    ctx.stack.push(ForthicValue::Record(rec));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::String(json) = result {
        // JSON object keys can be in any order
        assert!(json.contains("\"name\":\"Alice\""));
        assert!(json.contains("\"age\":30"));
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_from_json_null() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JSON>").unwrap();
    ctx.stack.push(ForthicValue::String("null".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Null));
}

#[test]
fn test_from_json_bool() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JSON>").unwrap();
    ctx.stack.push(ForthicValue::String("true".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Bool(true)));
}

#[test]
fn test_from_json_int() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JSON>").unwrap();
    ctx.stack.push(ForthicValue::String("42".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(42)));
}

#[test]
fn test_from_json_float() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JSON>").unwrap();
    ctx.stack.push(ForthicValue::String("3.14".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Float(3.14)));
}

#[test]
fn test_from_json_string() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JSON>").unwrap();
    ctx.stack.push(ForthicValue::String("\"hello\"".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("hello".to_string())));
}

#[test]
fn test_from_json_array() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JSON>").unwrap();
    ctx.stack.push(ForthicValue::String("[1,2,3]".to_string()));
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
fn test_from_json_object() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JSON>").unwrap();
    ctx.stack.push(ForthicValue::String("{\"name\":\"Alice\",\"age\":30}".to_string()));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Record(rec) = result {
        assert_eq!(rec.get("name"), Some(&ForthicValue::String("Alice".to_string())));
        assert_eq!(rec.get("age"), Some(&ForthicValue::Int(30)));
    } else {
        panic!("Expected record");
    }
}

#[test]
fn test_from_json_empty_string() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JSON>").unwrap();
    ctx.stack.push(ForthicValue::String("".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Null));
}

#[test]
fn test_from_json_invalid() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JSON>").unwrap();
    ctx.stack.push(ForthicValue::String("{invalid}".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Null));
}

// Formatting Tests

#[test]
fn test_json_prettify() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JSON-PRETTIFY").unwrap();
    ctx.stack.push(ForthicValue::String("{\"a\":1,\"b\":2}".to_string()));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::String(pretty) = result {
        assert!(pretty.contains("  \"a\": 1"));
        assert!(pretty.contains("  \"b\": 2"));
        assert!(pretty.len() > "{\"a\":1,\"b\":2}".len()); // Should be longer due to formatting
    } else {
        panic!("Expected string");
    }
}

#[test]
fn test_json_prettify_empty() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JSON-PRETTIFY").unwrap();
    ctx.stack.push(ForthicValue::String("".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("".to_string())));
}

// Round-trip Tests

#[test]
fn test_roundtrip_complex_structure() {
    let module = JSONModule::new();
    let mut ctx = MockContext::new();

    // Create complex structure
    let mut rec = HashMap::new();
    rec.insert("name".to_string(), ForthicValue::String("Bob".to_string()));
    rec.insert("scores".to_string(), ForthicValue::Array(vec![
        ForthicValue::Int(85),
        ForthicValue::Int(92),
        ForthicValue::Int(78),
    ]));

    // Convert to JSON
    let to_json = module.module().find_word(">JSON").unwrap();
    ctx.stack.push(ForthicValue::Record(rec.clone()));
    to_json.execute(&mut ctx).unwrap();

    // Convert back from JSON
    let from_json = module.module().find_word("JSON>").unwrap();
    from_json.execute(&mut ctx).unwrap();

    // Verify
    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Record(result_rec) = result {
        assert_eq!(result_rec.get("name"), Some(&ForthicValue::String("Bob".to_string())));
        if let Some(ForthicValue::Array(scores)) = result_rec.get("scores") {
            assert_eq!(scores.len(), 3);
            assert_eq!(scores[0], ForthicValue::Int(85));
        } else {
            panic!("Expected scores array");
        }
    } else {
        panic!("Expected record");
    }
}
