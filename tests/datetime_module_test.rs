use forthic::literals::ForthicValue;
use forthic::modules::standard::DateTimeModule;
use forthic::module::{InterpreterContext, Module};
use chrono::{Datelike, NaiveDate, NaiveTime, Timelike};

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

// Current Tests

#[test]
fn test_today() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("TODAY").unwrap();
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    assert!(matches!(result, ForthicValue::Date(_)));
}

#[test]
fn test_now() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("NOW").unwrap();
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    assert!(matches!(result, ForthicValue::DateTime(_)));
}

// Conversion To Tests

#[test]
fn test_to_time_from_string() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">TIME").unwrap();
    ctx.stack.push(ForthicValue::String("14:30".to_string()));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Time(t) = result {
        assert_eq!(t.hour(), 14);
        assert_eq!(t.minute(), 30);
    } else {
        panic!("Expected time");
    }
}

#[test]
fn test_to_time_am_pm() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">TIME").unwrap();
    ctx.stack.push(ForthicValue::String("2:30 PM".to_string()));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Time(t) = result {
        assert_eq!(t.hour(), 14);
        assert_eq!(t.minute(), 30);
    } else {
        panic!("Expected time");
    }
}

#[test]
fn test_to_date_from_string() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">DATE").unwrap();
    ctx.stack.push(ForthicValue::String("2024-01-15".to_string()));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Date(d) = result {
        assert_eq!(d.year(), 2024);
        assert_eq!(d.month(), 1);
        assert_eq!(d.day(), 15);
    } else {
        panic!("Expected date");
    }
}

#[test]
fn test_to_datetime_from_timestamp() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word(">DATETIME").unwrap();
    ctx.stack.push(ForthicValue::Int(1700000000)); // Unix timestamp
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    assert!(matches!(result, ForthicValue::DateTime(_)));
}

#[test]
fn test_at_combine_date_and_time() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    let time = NaiveTime::from_hms_opt(14, 30, 0).unwrap();

    let word = module.module().find_word("AT").unwrap();
    ctx.stack.push(ForthicValue::Date(date));
    ctx.stack.push(ForthicValue::Time(time));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    assert!(matches!(result, ForthicValue::DateTime(_)));
}

// Conversion From Tests

#[test]
fn test_time_to_str() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let time = NaiveTime::from_hms_opt(14, 30, 0).unwrap();

    let word = module.module().find_word("TIME>STR").unwrap();
    ctx.stack.push(ForthicValue::Time(time));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("14:30".to_string())));
}

#[test]
fn test_date_to_str() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

    let word = module.module().find_word("DATE>STR").unwrap();
    ctx.stack.push(ForthicValue::Date(date));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::String("2024-01-15".to_string())));
}

#[test]
fn test_date_to_int() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

    let word = module.module().find_word("DATE>INT").unwrap();
    ctx.stack.push(ForthicValue::Date(date));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(20240115)));
}

// Timestamp Tests

#[test]
fn test_to_timestamp() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    // Create a specific datetime
    let word_to_dt = module.module().find_word(">DATETIME").unwrap();
    ctx.stack.push(ForthicValue::Int(1700000000));
    word_to_dt.execute(&mut ctx).unwrap();

    // Convert to timestamp
    let word = module.module().find_word(">TIMESTAMP").unwrap();
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(1700000000)));
}

#[test]
fn test_timestamp_to_datetime() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let word = module.module().find_word("TIMESTAMP>DATETIME").unwrap();
    ctx.stack.push(ForthicValue::Int(1700000000));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    assert!(matches!(result, ForthicValue::DateTime(_)));
}

// Date Math Tests

#[test]
fn test_add_days() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

    let word = module.module().find_word("ADD-DAYS").unwrap();
    ctx.stack.push(ForthicValue::Date(date));
    ctx.stack.push(ForthicValue::Int(7));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Date(d) = result {
        assert_eq!(d.year(), 2024);
        assert_eq!(d.month(), 1);
        assert_eq!(d.day(), 22);
    } else {
        panic!("Expected date");
    }
}

#[test]
fn test_add_negative_days() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

    let word = module.module().find_word("ADD-DAYS").unwrap();
    ctx.stack.push(ForthicValue::Date(date));
    ctx.stack.push(ForthicValue::Int(-5));
    word.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Date(d) = result {
        assert_eq!(d.year(), 2024);
        assert_eq!(d.month(), 1);
        assert_eq!(d.day(), 10);
    } else {
        panic!("Expected date");
    }
}

#[test]
fn test_subtract_dates() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let date1 = NaiveDate::from_ymd_opt(2024, 1, 22).unwrap();
    let date2 = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

    let word = module.module().find_word("SUBTRACT-DATES").unwrap();
    ctx.stack.push(ForthicValue::Date(date1));
    ctx.stack.push(ForthicValue::Date(date2));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(7)));
}

#[test]
fn test_subtract_dates_negative() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let date1 = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    let date2 = NaiveDate::from_ymd_opt(2024, 1, 22).unwrap();

    let word = module.module().find_word("SUBTRACT-DATES").unwrap();
    ctx.stack.push(ForthicValue::Date(date1));
    ctx.stack.push(ForthicValue::Date(date2));
    word.execute(&mut ctx).unwrap();

    assert_eq!(ctx.stack.pop(), Some(ForthicValue::Int(-7)));
}

// Round-trip Tests

#[test]
fn test_roundtrip_date_to_string_and_back() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let original_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

    // Convert to string
    let to_str = module.module().find_word("DATE>STR").unwrap();
    ctx.stack.push(ForthicValue::Date(original_date));
    to_str.execute(&mut ctx).unwrap();

    // Convert back to date
    let to_date = module.module().find_word(">DATE").unwrap();
    to_date.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Date(d) = result {
        assert_eq!(d, original_date);
    } else {
        panic!("Expected date");
    }
}

#[test]
fn test_roundtrip_time_to_string_and_back() {
    let module = DateTimeModule::new();
    let mut ctx = MockContext::new();

    let original_time = NaiveTime::from_hms_opt(14, 30, 0).unwrap();

    // Convert to string
    let to_str = module.module().find_word("TIME>STR").unwrap();
    ctx.stack.push(ForthicValue::Time(original_time));
    to_str.execute(&mut ctx).unwrap();

    // Convert back to time
    let to_time = module.module().find_word(">TIME").unwrap();
    to_time.execute(&mut ctx).unwrap();

    let result = ctx.stack.pop().unwrap();
    if let ForthicValue::Time(t) = result {
        assert_eq!(t.hour(), original_time.hour());
        assert_eq!(t.minute(), original_time.minute());
    } else {
        panic!("Expected time");
    }
}
