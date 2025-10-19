# Test Porting Plan: forthic-ts → forthic-rs

## Overview

This plan outlines the strategy for porting the test suite from TypeScript (Jest) to Rust (built-in test framework with cargo test).

## Test Framework Analysis

**TypeScript (forthic-ts):**
- Framework: Jest
- Test organization: `describe` blocks and `test` functions
- Assertions: `expect().toBe()`, `expect().toEqual()`, `expect().toBeInstanceOf()`
- Async: All tests use `async/await`
- Setup: `beforeEach` for test initialization

**Rust (forthic-rs):**
- Framework: Built-in `#[test]` with cargo test
- Test organization: `mod tests` nested modules with `#[test]` functions
- Assertions: `assert_eq!()`, `assert!()`, `assert_ne!()`, `matches!()`
- Async: `#[tokio::test]` for async tests
- Setup: Helper functions, fixtures, or `setup()` functions per test

## Test File Structure

```
forthic-ts/src/forthic/tests/
├── unit/
│   ├── core/
│   │   ├── decorators.test.ts
│   │   ├── literals.test.ts
│   │   ├── utils.test.ts
│   │   ├── error.test.ts
│   │   ├── options.test.ts
│   │   ├── interpreter.test.ts
│   │   └── tokenizer.test.ts
│   └── modules/
│       ├── array_module.test.ts
│       ├── boolean_module.test.ts
│       ├── core_module.test.ts
│       ├── datetime_module.test.ts
│       ├── json_module.test.ts
│       ├── math_module.test.ts
│       ├── record_module.test.ts
│       └── string_module.test.ts
└── integration/
    ├── interpreter_complete.test.ts
    ├── dot_symbol.test.ts
    ├── streamingRun.test.ts
    └── standard_interpreter.test.ts

Proposed:
forthic-rs/
├── tests/                        # Integration tests
│   ├── integration_tests.rs
│   ├── dot_symbol_tests.rs
│   ├── streaming_run_tests.rs
│   └── standard_interpreter_tests.rs
└── src/
    ├── tokenizer.rs
    │   └── #[cfg(test)] mod tests { ... }
    ├── literals.rs
    │   └── #[cfg(test)] mod tests { ... }
    ├── interpreter.rs
    │   └── #[cfg(test)] mod tests { ... }
    └── modules/
        └── standard/
            ├── array.rs
            │   └── #[cfg(test)] mod tests { ... }
            ├── core.rs
            │   └── #[cfg(test)] mod tests { ... }
            └── ...
```

## Porting Strategy

### Phase 1: Setup (Foundation)

1. **Configure Cargo.toml**
   ```toml
   [dev-dependencies]
   tokio-test = "0.4"
   proptest = "1.0"  # Property-based testing

   [features]
   # ... existing features ...
   ```

2. **Create test utilities**
   ```rust
   // tests/common/mod.rs
   use forthic::prelude::*;

   pub fn create_test_interpreter() -> StandardInterpreter {
       StandardInterpreter::new(vec![], "America/Los_Angeles")
   }

   pub async fn run_and_pop(code: &str) -> Result<ForthicValue, ForthicError> {
       let mut interp = create_test_interpreter();
       interp.run(code).await?;
       interp.stack_pop()
   }
   ```

### Phase 2: Core Unit Tests (Priority: High)

Port core infrastructure tests to ensure foundation is solid:

#### 1. src/tokenizer.rs (Unit Tests)

**TypeScript:**
```typescript
describe("Tokenizer", () => {
  test("tokenizes simple words", async () => {
    const tokenizer = new Tokenizer("DUP SWAP");
    const tokens = [];
    let token = tokenizer.nextToken();
    while (token.tokenType !== TokenType.EOS) {
      tokens.push(token);
      token = tokenizer.nextToken();
    }
    expect(tokens).toHaveLength(2);
    expect(tokens[0].string).toBe("DUP");
    expect(tokens[1].string).toBe("SWAP");
  });
});
```

**Rust:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_simple_words() {
        let mut tokenizer = Tokenizer::new("DUP SWAP".to_string());
        let mut tokens = Vec::new();

        while let Ok(Some(token)) = tokenizer.next_token() {
            if token.token_type == TokenType::Eos {
                break;
            }
            tokens.push(token);
        }

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].string, "DUP");
        assert_eq!(tokens[1].string, "SWAP");
    }

    #[test]
    fn tokenizes_strings() {
        let mut tokenizer = Tokenizer::new(r#""hello world""#.to_string());
        let token = tokenizer.next_token().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::String);
        assert_eq!(token.string, "hello world");
    }

    #[test]
    fn tokenizes_triple_quoted_strings() {
        let mut tokenizer = Tokenizer::new(r#""""multi
line
string""""#.to_string());
        let token = tokenizer.next_token().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::String);
        assert!(token.string.contains("multi"));
        assert!(token.string.contains("line"));
    }

    #[test]
    fn tokenizes_arrays() {
        let mut tokenizer = Tokenizer::new("[ 1 2 3 ]".to_string());
        let tokens: Vec<_> = std::iter::from_fn(|| {
            tokenizer.next_token().ok().flatten()
        })
        .take_while(|t| t.token_type != TokenType::Eos)
        .collect();

        assert_eq!(tokens[0].token_type, TokenType::StartArray);
        assert_eq!(tokens[4].token_type, TokenType::EndArray);
    }

    #[test]
    fn handles_invalid_words() {
        let mut tokenizer = Tokenizer::new("valid `invalid`".to_string());

        // Valid token
        let token1 = tokenizer.next_token();
        assert!(token1.is_ok());

        // Invalid token (backticks not allowed in words)
        let token2 = tokenizer.next_token();
        assert!(token2.is_err());
    }

    #[test]
    fn tracks_token_positions() {
        let mut tokenizer = Tokenizer::new("DUP SWAP".to_string());
        let token1 = tokenizer.next_token().unwrap().unwrap();
        let token2 = tokenizer.next_token().unwrap().unwrap();

        assert_eq!(token1.location.start_pos, 0);
        assert_eq!(token1.location.end_pos, 3);
        assert_eq!(token2.location.start_pos, 4);
        assert_eq!(token2.location.end_pos, 8);
    }
}
```

#### 2. src/literals.rs (Unit Tests)

**Rust:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_booleans() {
        assert!(matches!(to_bool("true"), Some(ForthicValue::Bool(true))));
        assert!(matches!(to_bool("false"), Some(ForthicValue::Bool(false))));
        assert!(matches!(to_bool("TRUE"), Some(ForthicValue::Bool(true))));
        assert_eq!(to_bool("not_a_bool"), None);
    }

    #[test]
    fn parses_integers() {
        assert_eq!(to_int("42"), Some(ForthicValue::Int(42)));
        assert_eq!(to_int("-100"), Some(ForthicValue::Int(-100)));
        assert_eq!(to_int("0"), Some(ForthicValue::Int(0)));
        assert_eq!(to_int("not_an_int"), None);
    }

    #[test]
    fn parses_floats() {
        if let Some(ForthicValue::Float(f)) = to_float("3.14") {
            assert!((f - 3.14).abs() < 0.001);
        } else {
            panic!("Expected float");
        }

        assert_eq!(to_float("not_a_float"), None);
    }

    #[test]
    fn parses_dates() {
        use chrono::NaiveDate;

        let expected = NaiveDate::from_ymd_opt(2023, 12, 25).unwrap();
        assert_eq!(
            to_literal_date("2023-12-25"),
            Some(ForthicValue::Date(expected))
        );
    }

    #[test]
    fn parses_times() {
        use chrono::NaiveTime;

        let expected = NaiveTime::from_hms_opt(14, 30, 0).unwrap();
        assert_eq!(
            to_time("14:30:00"),
            Some(ForthicValue::Time(expected))
        );
    }

    #[test]
    fn parses_datetimes() {
        let result = to_zoned_datetime_value("2023-12-25T14:30:00Z");
        assert!(matches!(result, Some(ForthicValue::DateTime(_))));
    }
}
```

#### 3. src/word_options.rs (Unit Tests)

**Rust:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_options_from_array() {
        let arr = vec![
            ForthicValue::String(".key1".to_string()),
            ForthicValue::Int(42),
            ForthicValue::String(".key2".to_string()),
            ForthicValue::String("value".to_string()),
        ];

        let opts = WordOptions::from_array(&arr).unwrap();
        assert_eq!(opts.get_int("key1"), Some(42));
        assert_eq!(opts.get_string("key2"), Some("value"));
    }

    #[test]
    fn pops_options_if_present() {
        let mut stack = vec![
            ForthicValue::Int(1),
            ForthicValue::Array(vec![
                ForthicValue::String(".opt".to_string()),
                ForthicValue::Int(99),
            ]),
        ];

        let opts = pop_options_if_present(&mut stack);
        assert!(opts.is_some());
        assert_eq!(opts.unwrap().get_int("opt"), Some(99));
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn does_not_pop_non_options_array() {
        let mut stack = vec![
            ForthicValue::Array(vec![
                ForthicValue::Int(1),
                ForthicValue::Int(2),
            ]),
        ];

        let opts = pop_options_if_present(&mut stack);
        assert!(opts.is_none());
        assert_eq!(stack.len(), 1);
    }
}
```

#### 4. src/errors.rs (Unit Tests)

**Rust:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_unknown_word_error() {
        let error = ForthicError::UnknownWord {
            word: "GARBAGE".to_string(),
            location: CodeLocation {
                start_pos: 5,
                end_pos: 12,
                string: "DUP GARBAGE SWAP".to_string(),
            },
        };

        let formatted = error.format_with_context();
        assert!(formatted.contains("GARBAGE"));
        assert!(formatted.contains("Unknown word"));
    }

    #[test]
    fn formats_stack_underflow_error() {
        let error = ForthicError::StackUnderflow {
            location: CodeLocation::default(),
        };

        let formatted = format!("{}", error);
        assert!(formatted.contains("Stack underflow"));
    }

    #[test]
    fn error_implements_std_error() {
        let error: Box<dyn std::error::Error> = Box::new(ForthicError::UnknownWord {
            word: "TEST".to_string(),
            location: CodeLocation::default(),
        });

        assert!(error.to_string().contains("TEST"));
    }
}
```

#### 5. src/interpreter.rs (Unit Tests)

**Rust:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_interp() -> StandardInterpreter {
        StandardInterpreter::new(vec![], "America/Los_Angeles")
    }

    #[tokio::test]
    async fn executes_simple_literals() {
        let mut interp = create_test_interp();
        interp.run("42").await.unwrap();

        let value = interp.stack_pop().unwrap();
        assert_eq!(value, ForthicValue::Int(42));
    }

    #[tokio::test]
    async fn executes_string_literals() {
        let mut interp = create_test_interp();
        interp.run(r#""hello""#).await.unwrap();

        let value = interp.stack_pop().unwrap();
        assert_eq!(value, ForthicValue::String("hello".to_string()));
    }

    #[tokio::test]
    async fn executes_array_literals() {
        let mut interp = create_test_interp();
        interp.run("[ 1 2 3 ]").await.unwrap();

        let value = interp.stack_pop().unwrap();
        if let ForthicValue::Array(arr) = value {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn handles_unknown_word() {
        let mut interp = create_test_interp();
        let result = interp.run("UNKNOWN_WORD").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ForthicError::UnknownWord { .. }));
    }

    #[tokio::test]
    async fn handles_stack_underflow() {
        let mut interp = create_test_interp();
        let result = interp.stack_pop();

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ForthicError::StackUnderflow { .. }));
    }

    #[tokio::test]
    async fn executes_definitions() {
        let mut interp = create_test_interp();
        interp.run(": DOUBLE 2 * ; 21 DOUBLE").await.unwrap();

        let value = interp.stack_pop().unwrap();
        assert_eq!(value, ForthicValue::Int(42));
    }

    #[tokio::test]
    async fn manages_variables() {
        let mut interp = create_test_interp();
        interp.run(r#"
            ["x"] VARIABLES
            42 x !
            x @
        "#).await.unwrap();

        let value = interp.stack_pop().unwrap();
        assert_eq!(value, ForthicValue::Int(42));
    }
}
```

### Phase 3: Module Unit Tests (Priority: High)

Port standard library module tests:

#### 1. src/modules/standard/core.rs (Unit Tests)

**Rust:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::StandardInterpreter;

    async fn run_code(code: &str) -> Result<ForthicValue, ForthicError> {
        let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");
        interp.run(code).await?;
        interp.stack_pop()
    }

    #[tokio::test]
    async fn test_dup() {
        let result = run_code("42 DUP").await.unwrap();
        assert_eq!(result, ForthicValue::Int(42));

        // Check there are two items
        let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");
        interp.run("42 DUP").await.unwrap();
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(42));
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(42));
    }

    #[tokio::test]
    async fn test_swap() {
        let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");
        interp.run("1 2 SWAP").await.unwrap();

        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(1));
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(2));
    }

    #[tokio::test]
    async fn test_pop() {
        let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");
        interp.run("1 2 3 POP").await.unwrap();

        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(2));
        assert_eq!(interp.stack_pop().unwrap(), ForthicValue::Int(1));
    }

    #[tokio::test]
    async fn test_variables() {
        let result = run_code(r#"
            ["x" "y"] VARIABLES
            10 x !
            20 y !
            x @ y @ +
        "#).await.unwrap();

        assert_eq!(result, ForthicValue::Int(30));
    }

    #[tokio::test]
    async fn test_default() {
        let result = run_code("NULL 42 DEFAULT").await.unwrap();
        assert_eq!(result, ForthicValue::Int(42));

        let result = run_code("10 42 DEFAULT").await.unwrap();
        assert_eq!(result, ForthicValue::Int(10));
    }

    #[tokio::test]
    async fn test_interpolate() {
        let result = run_code(r#"
            ["name"] VARIABLES
            "Alice" name !
            "Hello, {name}!" INTERPOLATE
        "#).await.unwrap();

        assert_eq!(result, ForthicValue::String("Hello, Alice!".to_string()));
    }
}
```

#### 2. src/modules/standard/array.rs (Unit Tests)

**Rust:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn run_code(code: &str) -> Result<ForthicValue, ForthicError> {
        let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");
        interp.run(code).await?;
        interp.stack_pop()
    }

    #[tokio::test]
    async fn test_append() {
        let result = run_code("[ 1 2 3 ] 4 APPEND").await.unwrap();
        if let ForthicValue::Array(arr) = result {
            assert_eq!(arr, vec![
                ForthicValue::Int(1),
                ForthicValue::Int(2),
                ForthicValue::Int(3),
                ForthicValue::Int(4),
            ]);
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_reverse() {
        let result = run_code("[ 1 2 3 ] REVERSE").await.unwrap();
        if let ForthicValue::Array(arr) = result {
            assert_eq!(arr[0], ForthicValue::Int(3));
            assert_eq!(arr[2], ForthicValue::Int(1));
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_nth() {
        let result = run_code("[ 10 20 30 ] 1 NTH").await.unwrap();
        assert_eq!(result, ForthicValue::Int(20));
    }

    #[tokio::test]
    async fn test_nth_out_of_bounds() {
        let result = run_code("[ 10 20 30 ] 99 NTH").await.unwrap();
        assert_eq!(result, ForthicValue::Null);
    }

    #[tokio::test]
    async fn test_length() {
        let result = run_code("[ 1 2 3 4 5 ] LENGTH").await.unwrap();
        assert_eq!(result, ForthicValue::Int(5));
    }

    #[tokio::test]
    async fn test_map() {
        let result = run_code(r#"
            [ 1 2 3 ] : DOUBLE 2 * ; MAP
        "#).await.unwrap();

        if let ForthicValue::Array(arr) = result {
            assert_eq!(arr[0], ForthicValue::Int(2));
            assert_eq!(arr[1], ForthicValue::Int(4));
            assert_eq!(arr[2], ForthicValue::Int(6));
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_filter() {
        let result = run_code(r#"
            [ 1 2 3 4 5 ] : 2 MOD 0 == ; FILTER
        "#).await.unwrap();

        if let ForthicValue::Array(arr) = result {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0], ForthicValue::Int(2));
            assert_eq!(arr[1], ForthicValue::Int(4));
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_reduce() {
        let result = run_code(r#"
            [ 1 2 3 4 5 ] 0 : + ; REDUCE
        "#).await.unwrap();

        assert_eq!(result, ForthicValue::Int(15));
    }

    #[tokio::test]
    async fn test_unique() {
        let result = run_code("[ 1 2 2 3 3 3 ] UNIQUE").await.unwrap();

        if let ForthicValue::Array(arr) = result {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_flatten() {
        let result = run_code("[ [ 1 2 ] [ 3 4 ] ] FLATTEN").await.unwrap();

        if let ForthicValue::Array(arr) = result {
            assert_eq!(arr.len(), 4);
            assert_eq!(arr[0], ForthicValue::Int(1));
            assert_eq!(arr[3], ForthicValue::Int(4));
        } else {
            panic!("Expected array");
        }
    }
}
```

#### 3. src/modules/standard/record.rs (Unit Tests)

**Rust:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn run_code(code: &str) -> Result<ForthicValue, ForthicError> {
        let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");
        interp.run(code).await?;
        interp.stack_pop()
    }

    #[tokio::test]
    async fn test_rec_creation() {
        let result = run_code(r#"
            [ "name" "Alice" "age" 30 ] REC
        "#).await.unwrap();

        if let ForthicValue::Record(rec) = result {
            assert_eq!(rec.get("name"), Some(&ForthicValue::String("Alice".to_string())));
            assert_eq!(rec.get("age"), Some(&ForthicValue::Int(30)));
        } else {
            panic!("Expected record");
        }
    }

    #[tokio::test]
    async fn test_rec_access() {
        let result = run_code(r#"
            [ "name" "Alice" "age" 30 ] REC "name" REC@
        "#).await.unwrap();

        assert_eq!(result, ForthicValue::String("Alice".to_string()));
    }

    #[tokio::test]
    async fn test_rec_update() {
        let result = run_code(r#"
            [ "name" "Alice" ] REC
            "age" 30 <REC!
        "#).await.unwrap();

        if let ForthicValue::Record(rec) = result {
            assert_eq!(rec.get("name"), Some(&ForthicValue::String("Alice".to_string())));
            assert_eq!(rec.get("age"), Some(&ForthicValue::Int(30)));
        } else {
            panic!("Expected record");
        }
    }

    #[tokio::test]
    async fn test_rec_delete() {
        let result = run_code(r#"
            [ "name" "Alice" "age" 30 ] REC
            "age" <DEL
        "#).await.unwrap();

        if let ForthicValue::Record(rec) = result {
            assert_eq!(rec.get("name"), Some(&ForthicValue::String("Alice".to_string())));
            assert_eq!(rec.get("age"), None);
        } else {
            panic!("Expected record");
        }
    }
}
```

#### 4-8. Other Modules (String, Math, Boolean, JSON, DateTime)

Similar pattern - port each module's tests to `#[cfg(test)] mod tests` within the module file.

### Phase 4: Integration Tests (Priority: Medium)

Port comprehensive integration tests to `tests/` directory:

#### 1. tests/standard_interpreter_tests.rs

**Rust:**
```rust
use forthic::prelude::*;

#[tokio::test]
async fn test_complex_workflow() {
    let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");

    interp.run(r#"
        # Define variables
        ["numbers" "result"] VARIABLES

        # Store array
        [ 1 2 3 4 5 ] numbers !

        # Process: filter evens, double them, sum
        numbers @
        : 2 MOD 0 == ; FILTER
        : 2 * ; MAP
        0 : + ; REDUCE
        result !

        result @
    "#).await.unwrap();

    let value = interp.stack_pop().unwrap();
    assert_eq!(value, ForthicValue::Int(12)); // (2 + 4) * 2 = 12
}

#[tokio::test]
async fn test_nested_definitions() {
    let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");

    interp.run(r#"
        : SQUARE DUP * ;
        : SUM-OF-SQUARES : SQUARE ; MAP 0 : + ; REDUCE ;

        [ 1 2 3 ] SUM-OF-SQUARES
    "#).await.unwrap();

    let value = interp.stack_pop().unwrap();
    assert_eq!(value, ForthicValue::Int(14)); // 1 + 4 + 9 = 14
}

#[tokio::test]
async fn test_module_imports() {
    // Test USE_MODULES functionality
    let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");

    interp.run(r#"
        # Create and use custom module
        { : TRIPLE 3 * ; "TRIPLE" EXPORT } "math-utils" USE_MODULES
        5 math-utils.TRIPLE
    "#).await.unwrap();

    let value = interp.stack_pop().unwrap();
    assert_eq!(value, ForthicValue::Int(15));
}
```

#### 2. tests/dot_symbol_tests.rs

**Rust:**
```rust
use forthic::prelude::*;

#[tokio::test]
async fn test_dot_symbol_in_records() {
    let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");

    interp.run(r#"
        [ "user" [ "name" "Alice" ] REC ] REC
        .user .name
    "#).await.unwrap();

    let value = interp.stack_pop().unwrap();
    assert_eq!(value, ForthicValue::String("Alice".to_string()));
}

#[tokio::test]
async fn test_dot_symbol_chaining() {
    let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");

    interp.run(r#"
        [
            "company" [
                "name" "TechCorp"
                "employees" [
                    [ "name" "Alice" ] REC
                    [ "name" "Bob" ] REC
                ]
            ] REC
        ] REC
        .company .employees 0 NTH .name
    "#).await.unwrap();

    let value = interp.stack_pop().unwrap();
    assert_eq!(value, ForthicValue::String("Alice".to_string()));
}
```

#### 3. tests/streaming_run_tests.rs

**Rust:**
```rust
use forthic::prelude::*;

#[tokio::test]
async fn test_streaming_execution() {
    // Test that interpreter can handle code sent in chunks
    let mut interp = StandardInterpreter::new(vec![], "America/Los_Angeles");

    // Send code in parts
    interp.run("[ 1 2").await.unwrap();
    interp.run(" 3 ]").await.unwrap();

    let value = interp.stack_pop().unwrap();
    if let ForthicValue::Array(arr) = value {
        assert_eq!(arr.len(), 3);
    } else {
        panic!("Expected array");
    }
}
```

## Key Adaptations for Rust

### 1. Async/Await → Tokio Test

**TypeScript:**
```typescript
test("APPEND", async () => {
  await interp.run(`[ 1 2 3 ] 4 APPEND`);
  expect(interp.stack_pop()).toEqual([1, 2, 3, 4]);
});
```

**Rust:**
```rust
#[tokio::test]
async fn test_append() {
    let mut interp = create_test_interpreter();
    interp.run("[ 1 2 3 ] 4 APPEND").await.unwrap();

    let value = interp.stack_pop().unwrap();
    if let ForthicValue::Array(arr) = value {
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[3], ForthicValue::Int(4));
    }
}
```

### 2. Replace Jest Matchers with Rust Assertions

**TypeScript:**
```typescript
expect(value).toBe(5);              // Reference equality
expect(value).toEqual([1, 2, 3]);   // Deep equality
expect(value).toBeNull();           // Null check
expect(value).toBeInstanceOf(Error); // Type check
expect(value).toBeCloseTo(3.14);    // Float comparison
```

**Rust:**
```rust
assert_eq!(value, 5);                           // Equality
assert_eq!(value, vec![1, 2, 3]);               // Deep equality
assert!(matches!(value, ForthicValue::Null));   // Null check
assert!(matches!(error, ForthicError::_));      // Type check

// Float comparison
if let ForthicValue::Float(f) = value {
    assert!((f - 3.14).abs() < 0.001);
}
```

### 3. Exception Testing

**TypeScript:**
```typescript
try {
  await interp.run("GARBAGE");
  fail("Expected error");
} catch (e) {
  expect(e).toBeInstanceOf(UnknownWordError);
  expect(e.getWord()).toEqual("GARBAGE");
}
```

**Rust:**
```rust
#[tokio::test]
async fn test_unknown_word_error() {
    let mut interp = create_test_interpreter();
    let result = interp.run("GARBAGE").await;

    assert!(result.is_err());
    if let Err(ForthicError::UnknownWord { word, .. }) = result {
        assert_eq!(word, "GARBAGE");
    } else {
        panic!("Expected UnknownWordError");
    }
}
```

### 4. Test Fixtures and Setup

**TypeScript:**
```typescript
let interp: StandardInterpreter;

beforeEach(async () => {
  interp = new StandardInterpreter([], "America/Los_Angeles");
});
```

**Rust:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_interpreter() -> StandardInterpreter {
        StandardInterpreter::new(vec![], "America/Los_Angeles")
    }

    // Or use a test fixture
    struct TestFixture {
        interp: StandardInterpreter,
    }

    impl TestFixture {
        fn new() -> Self {
            Self {
                interp: StandardInterpreter::new(vec![], "America/Los_Angeles"),
            }
        }
    }

    #[tokio::test]
    async fn test_with_fixture() {
        let mut fixture = TestFixture::new();
        fixture.interp.run("42").await.unwrap();
        // ...
    }
}
```

### 5. Temporal API → chrono

**TypeScript:**
```typescript
expect(date).toEqual(Temporal.PlainDate.from({
  year: 2020,
  month: 6,
  day: 5,
}));
```

**Rust:**
```rust
use chrono::NaiveDate;

let expected = NaiveDate::from_ymd_opt(2020, 6, 5).unwrap();
assert_eq!(date, expected);
```

### 6. Property-Based Testing (Rust-specific)

**Rust (using proptest):**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_reversing_twice_is_identity(vec in prop::collection::vec(any::<i64>(), 0..100)) {
        let mut interp = create_test_interpreter();

        // Convert vec to Forthic array
        let forthic_arr = vec.iter()
            .map(|&i| ForthicValue::Int(i))
            .collect::<Vec<_>>();

        interp.stack_push(ForthicValue::Array(forthic_arr.clone()));

        // REVERSE REVERSE should equal original
        let result = tokio_test::block_on(async {
            interp.run("REVERSE REVERSE").await.unwrap();
            interp.stack_pop().unwrap()
        });

        assert_eq!(result, ForthicValue::Array(forthic_arr));
    }
}
```

## Test Execution Order

1. **Tokenizer tests** - Foundation for everything
2. **Literals and utils tests** - Core parsing
3. **Interpreter tests** - Basic execution
4. **Core module tests** - Essential operations
5. **Other module tests** - Standard library
6. **Integration tests** - End-to-end validation

## Test Coverage Goals

- **Unit tests**: Cover all words in each module
- **Edge cases**: Null values, empty arrays, type mismatches
- **Error handling**: Invalid inputs, stack underflow, unknown words
- **Integration tests**: Complex real-world scenarios
- **Target coverage**: 90%+ for core functionality

Use `cargo-tarpaulin` for coverage:
```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

## Migration Checklist

For each test file:
- [ ] Create corresponding test module or file
- [ ] Convert `test()` to `#[test]` or `#[tokio::test]`
- [ ] Convert `beforeEach()` to fixture functions
- [ ] Keep `async/await` but use `#[tokio::test]`
- [ ] Replace Jest matchers with Rust assertions
- [ ] Update Temporal API calls to chrono
- [ ] Update exception testing to Result checking
- [ ] Run tests with `cargo test`
- [ ] Verify test coverage with `cargo tarpaulin`

## Example Test Module Template

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    fn create_test_interpreter() -> StandardInterpreter {
        StandardInterpreter::new(vec![], "America/Los_Angeles")
    }

    async fn run_and_pop(code: &str) -> Result<ForthicValue, ForthicError> {
        let mut interp = create_test_interpreter();
        interp.run(code).await?;
        interp.stack_pop()
    }

    #[tokio::test]
    async fn test_simple_operation() {
        let result = run_and_pop("[ 1 2 3 ] 4 APPEND").await.unwrap();

        if let ForthicValue::Array(arr) = result {
            assert_eq!(arr.len(), 4);
            assert_eq!(arr[3], ForthicValue::Int(4));
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_error_handling() {
        let result = run_and_pop("INVALID_WORD").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ForthicError::UnknownWord { .. }));
    }

    #[test]
    fn test_synchronous_operation() {
        // For non-async tests
        let value = ForthicValue::Int(42);
        assert_eq!(value, ForthicValue::Int(42));
    }
}
```

## Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_append

# Run tests with output
cargo test -- --nocapture

# Run tests in parallel
cargo test -- --test-threads=4

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test '*'

# Run with coverage
cargo tarpaulin --out Html

# Run benchmarks
cargo bench
```

## Next Steps

1. Set up test infrastructure in forthic-rs
2. Create common test utilities
3. Start with Phase 2: Core unit tests
4. Port tests incrementally, running them as you go
5. Fix any implementation issues discovered by tests
6. Add Rust-specific tests (property-based, etc.)
7. Achieve comprehensive test coverage
8. Set up CI/CD for automated testing

## Notes

- Tests should be ported incrementally to validate the Rust implementation
- Some tests may reveal bugs in the ported code - fix implementation before moving on
- Rust-specific edge cases may require additional tests not in TypeScript version
- Use `cargo-tarpaulin` for test coverage analysis
- Consider property-based testing with `proptest` for additional validation
- Benchmark critical paths with `cargo bench`
