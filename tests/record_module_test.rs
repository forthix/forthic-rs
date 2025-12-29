use forthic::literals::ForthicValue;
use forthic::modules::standard::RecordModule;
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

// Core Tests

#[test]
fn test_rec() {
    let module = RecordModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("REC").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Array(vec![
            ForthicValue::String("name".to_string()),
            ForthicValue::String("Alice".to_string()),
        ]),
        ForthicValue::Array(vec![
            ForthicValue::String("age".to_string()),
            ForthicValue::Int(30),
        ]),
    ]));
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
fn test_rec_at() {
    let module = RecordModule::new();
    let mut ctx = MockContext::new();

    let mut rec = HashMap::new();
    rec.insert("name".to_string(), ForthicValue::String("Bob".to_string()));
    rec.insert("age".to_string(), ForthicValue::Int(25));

    let word = module.module().find_word("REC@").unwrap();
    ctx.stack.push(ForthicValue::Record(rec));
    ctx.stack.push(ForthicValue::String("name".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("Bob".to_string())));
}

#[test]
fn test_rec_at_nested() {
    let module = RecordModule::new();
    let mut ctx = MockContext::new();

    let mut inner_rec = HashMap::new();
    inner_rec.insert("city".to_string(), ForthicValue::String("NYC".to_string()));

    let mut rec = HashMap::new();
    rec.insert("address".to_string(), ForthicValue::Record(inner_rec));

    let word = module.module().find_word("REC@").unwrap();
    ctx.stack.push(ForthicValue::Record(rec));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::String("address".to_string()),
        ForthicValue::String("city".to_string()),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("NYC".to_string())));
}

#[test]
fn test_set_rec() {
    let module = RecordModule::new();
    let mut ctx = MockContext::new();

    let mut rec = HashMap::new();
    rec.insert("name".to_string(), ForthicValue::String("Alice".to_string()));

    let word = module.module().find_word("<REC!").unwrap();
    ctx.stack.push(ForthicValue::Record(rec));
    ctx.stack.push(ForthicValue::Int(30));
    ctx.stack.push(ForthicValue::String("age".to_string()));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Record(rec) = result {
        assert_eq!(rec.get("age"), Some(&ForthicValue::Int(30)));
        assert_eq!(rec.get("name"), Some(&ForthicValue::String("Alice".to_string())));
    } else {
        panic!("Expected record");
    }
}

// Transform Tests

#[test]
fn test_relabel() {
    let module = RecordModule::new();
    let mut ctx = MockContext::new();

    let mut rec = HashMap::new();
    rec.insert("old1".to_string(), ForthicValue::Int(1));
    rec.insert("old2".to_string(), ForthicValue::Int(2));

    let word = module.module().find_word("RELABEL").unwrap();
    ctx.stack.push(ForthicValue::Record(rec));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::String("old1".to_string()),
        ForthicValue::String("old2".to_string()),
    ]));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::String("new1".to_string()),
        ForthicValue::String("new2".to_string()),
    ]));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Record(rec) = result {
        assert_eq!(rec.get("new1"), Some(&ForthicValue::Int(1)));
        assert_eq!(rec.get("new2"), Some(&ForthicValue::Int(2)));
        assert_eq!(rec.get("old1"), None);
    } else {
        panic!("Expected record");
    }
}

#[test]
fn test_invert_keys() {
    let module = RecordModule::new();
    let mut ctx = MockContext::new();

    let mut inner1 = HashMap::new();
    inner1.insert("a".to_string(), ForthicValue::Int(1));
    inner1.insert("b".to_string(), ForthicValue::Int(2));

    let mut inner2 = HashMap::new();
    inner2.insert("a".to_string(), ForthicValue::Int(3));
    inner2.insert("b".to_string(), ForthicValue::Int(4));

    let mut rec = HashMap::new();
    rec.insert("x".to_string(), ForthicValue::Record(inner1));
    rec.insert("y".to_string(), ForthicValue::Record(inner2));

    let word = module.module().find_word("INVERT-KEYS").unwrap();
    ctx.stack.push(ForthicValue::Record(rec));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Record(inverted) = result {
        // Should have keys "a" and "b" now at top level
        assert!(inverted.contains_key("a"));
        assert!(inverted.contains_key("b"));

        if let Some(ForthicValue::Record(a_rec)) = inverted.get("a") {
            assert_eq!(a_rec.get("x"), Some(&ForthicValue::Int(1)));
            assert_eq!(a_rec.get("y"), Some(&ForthicValue::Int(3)));
        } else {
            panic!("Expected nested record for 'a'");
        }
    } else {
        panic!("Expected record");
    }
}

#[test]
fn test_rec_defaults() {
    let module = RecordModule::new();
    let mut ctx = MockContext::new();

    let mut rec = HashMap::new();
    rec.insert("name".to_string(), ForthicValue::String("Alice".to_string()));
    rec.insert("age".to_string(), ForthicValue::Null);

    let word = module.module().find_word("REC-DEFAULTS").unwrap();
    ctx.stack.push(ForthicValue::Record(rec));
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::Array(vec![
            ForthicValue::String("age".to_string()),
            ForthicValue::Int(25),
        ]),
        ForthicValue::Array(vec![
            ForthicValue::String("city".to_string()),
            ForthicValue::String("NYC".to_string()),
        ]),
    ]));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Record(rec) = result {
        assert_eq!(rec.get("age"), Some(&ForthicValue::Int(25))); // Was null, so replaced
        assert_eq!(rec.get("city"), Some(&ForthicValue::String("NYC".to_string()))); // Was missing, so added
        assert_eq!(rec.get("name"), Some(&ForthicValue::String("Alice".to_string()))); // Unchanged
    } else {
        panic!("Expected record");
    }
}

#[test]
fn test_del() {
    let module = RecordModule::new();
    let mut ctx = MockContext::new();

    let mut rec = HashMap::new();
    rec.insert("name".to_string(), ForthicValue::String("Alice".to_string()));
    rec.insert("age".to_string(), ForthicValue::Int(30));

    let word = module.module().find_word("<DEL").unwrap();
    ctx.stack.push(ForthicValue::Record(rec));
    ctx.stack.push(ForthicValue::String("age".to_string()));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Record(rec) = result {
        assert_eq!(rec.len(), 1);
        assert!(rec.contains_key("name"));
        assert!(!rec.contains_key("age"));
    } else {
        panic!("Expected record");
    }
}

// Access Tests

#[test]
fn test_keys() {
    let module = RecordModule::new();
    let mut ctx = MockContext::new();

    let mut rec = HashMap::new();
    rec.insert("name".to_string(), ForthicValue::String("Alice".to_string()));
    rec.insert("age".to_string(), ForthicValue::Int(30));

    let word = module.module().find_word("KEYS").unwrap();
    ctx.stack.push(ForthicValue::Record(rec));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(keys) = result {
        assert_eq!(keys.len(), 2);
        // Keys might be in any order
        assert!(keys.contains(&ForthicValue::String("name".to_string())));
        assert!(keys.contains(&ForthicValue::String("age".to_string())));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_values() {
    let module = RecordModule::new();
    let mut ctx = MockContext::new();

    let mut rec = HashMap::new();
    rec.insert("a".to_string(), ForthicValue::Int(1));
    rec.insert("b".to_string(), ForthicValue::Int(2));

    let word = module.module().find_word("VALUES").unwrap();
    ctx.stack.push(ForthicValue::Record(rec));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(values) = result {
        assert_eq!(values.len(), 2);
        // Values might be in any order
        assert!(values.contains(&ForthicValue::Int(1)));
        assert!(values.contains(&ForthicValue::Int(2)));
    } else {
        panic!("Expected array");
    }
}
