use forthic::literals::ForthicValue;
use forthic::modules::standard::StringModule;
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

// Conversion Tests

#[test]
fn test_to_str_int() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">STR").unwrap();
    ctx.stack.push(ForthicValue::Int(42));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("42".to_string())));
}

#[test]
fn test_to_str_bool() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">STR").unwrap();
    ctx.stack.push(ForthicValue::Bool(true));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("true".to_string())));
}

#[test]
fn test_url_encode() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("URL-ENCODE").unwrap();
    ctx.stack.push(ForthicValue::String("hello world".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("hello%20world".to_string())));
}

#[test]
fn test_url_decode() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("URL-DECODE").unwrap();
    ctx.stack.push(ForthicValue::String("hello%20world".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("hello world".to_string())));
}

// Transform Tests

#[test]
fn test_lowercase() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("LOWERCASE").unwrap();
    ctx.stack.push(ForthicValue::String("Hello World".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("hello world".to_string())));
}

#[test]
fn test_uppercase() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("UPPERCASE").unwrap();
    ctx.stack.push(ForthicValue::String("Hello World".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("HELLO WORLD".to_string())));
}

#[test]
fn test_strip() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("STRIP").unwrap();
    ctx.stack.push(ForthicValue::String("  hello world  ".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("hello world".to_string())));
}

#[test]
fn test_ascii() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("ASCII").unwrap();
    ctx.stack.push(ForthicValue::String("hello\u{1F600}world".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("helloworld".to_string())));
}

// Split/Join Tests

#[test]
fn test_split() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("SPLIT").unwrap();
    ctx.stack.push(ForthicValue::String("hello,world,test".to_string()));
    ctx.stack.push(ForthicValue::String(",".to_string()));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Array(arr) = result {
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], ForthicValue::String("hello".to_string()));
        assert_eq!(arr[1], ForthicValue::String("world".to_string()));
        assert_eq!(arr[2], ForthicValue::String("test".to_string()));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_join() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("JOIN").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::String("hello".to_string()),
        ForthicValue::String("world".to_string()),
    ]));
    ctx.stack.push(ForthicValue::String(" ".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("hello world".to_string())));
}

#[test]
fn test_concat_two_strings() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("CONCAT").unwrap();
    ctx.stack.push(ForthicValue::String("hello".to_string()));
    ctx.stack.push(ForthicValue::String(" world".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("hello world".to_string())));
}

#[test]
fn test_concat_array() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("CONCAT").unwrap();
    ctx.stack.push(ForthicValue::Array(vec![
        ForthicValue::String("hello".to_string()),
        ForthicValue::String(" ".to_string()),
        ForthicValue::String("world".to_string()),
    ]));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("hello world".to_string())));
}

// Pattern Tests

#[test]
fn test_replace() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("REPLACE").unwrap();
    ctx.stack.push(ForthicValue::String("hello world".to_string()));
    ctx.stack.push(ForthicValue::String("world".to_string()));
    ctx.stack.push(ForthicValue::String("Rust".to_string()));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("hello Rust".to_string())));
}

// Constant Tests

#[test]
fn test_newline() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("/N").unwrap();
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("\n".to_string())));
}

#[test]
fn test_carriage_return() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("/R").unwrap();
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("\r".to_string())));
}

#[test]
fn test_tab() {
    let module = StringModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("/T").unwrap();
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("\t".to_string())));
}
