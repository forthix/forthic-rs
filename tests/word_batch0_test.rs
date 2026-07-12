//! Batch 0 word-inventory fixes (plans/WORD-INVENTORY.md): the
//! same-name-different-meaning collisions and contract divergences that had
//! to be fixed before porting more of the ts vocabulary.

// ForthicError is large; accepted trade-off (see lib.rs / backlog item 11)
#![allow(clippy::result_large_err)]

use forthic::errors::ForthicError;
use forthic::interpreter::Interpreter;
use forthic::literals::ForthicValue;

fn run(code: &str) -> ForthicValue {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap();
    interp.get_stack_mut().pop().unwrap()
}

fn ints(values: &[i64]) -> ForthicValue {
    ForthicValue::Array(values.iter().map(|i| ForthicValue::Int(*i)).collect())
}

// ===== DROP / SKIP (the worst collision) =====

#[test]
fn test_drop_pops_the_stack_ts_semantics() {
    // ts core DROP: ( a -- ). The old rs DROP meant skip-first-n.
    let mut interp = Interpreter::standard("UTC");
    interp.run("1 2 DROP").unwrap();
    assert_eq!(interp.get_stack().items(), &[ForthicValue::Int(1)]);
}

#[test]
fn test_skip_skips_first_n() {
    assert_eq!(run("[ 1 2 3 ] 2 SKIP"), ints(&[3]));
}

#[test]
fn test_classic_pop_is_gone() {
    // Classic words with canonical replacements are dropped, not aliased
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("1 POP").unwrap_err();
    assert!(err.to_string().contains("POP"), "got: {err}");
}

#[test]
fn test_classic_identity_is_gone() {
    let mut interp = Interpreter::standard("UTC");
    assert!(interp.run("IDENTITY").is_err());
    interp.reset();
    interp.run("NOP").unwrap(); // the canonical no-op remains
}

// ===== CONCAT (single contract) =====

#[test]
fn test_concat_joins_string_arrays() {
    assert_eq!(
        run("[ 'a' 'b' 'c' ] CONCAT"),
        ForthicValue::String("abc".to_string())
    );
    // Null elements become empty strings (ts contract)
    assert_eq!(
        run("[ 'a' NULL 'b' ] CONCAT"),
        ForthicValue::String("ab".to_string())
    );
}

#[test]
fn test_concat_rejects_two_strings() {
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("'a' 'b' CONCAT").unwrap_err();
    assert!(
        err.to_string().contains("[s1 s2] CONCAT"),
        "helpful message: {err}"
    );
}

// ===== RANGE =====

#[test]
fn test_range_ascending() {
    assert_eq!(run("1 4 RANGE"), ints(&[1, 2, 3, 4]));
}

#[test]
fn test_range_reversed_is_empty() {
    // ts contract; the old rs behavior produced a descending range
    assert_eq!(run("5 1 RANGE"), ForthicValue::Array(vec![]));
}

#[test]
fn test_range_allocation_guard() {
    // ts #34: guard pathological sizes before allocating
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("1 2000000000 RANGE").unwrap_err();
    match err {
        ForthicError::InvalidOperation { message, .. } => {
            assert!(message.contains("too large"), "got: {message}")
        }
        other => panic!("expected InvalidOperation, got {other}"),
    }
}

// ===== FLATTEN =====

#[test]
fn test_flatten_is_full_depth_by_default() {
    // ts contract; the old rs behavior flattened exactly one level
    assert_eq!(run("[ [ [ 1 2 ] ] [ 3 ] 4 ] FLATTEN"), ints(&[1, 2, 3, 4]));
}

#[test]
fn test_flatten_depth_option_limits_descent() {
    assert_eq!(
        run("[ [ [ 1 2 ] ] [ 3 ] ] [ .depth 1 ] ~> FLATTEN"),
        ForthicValue::Array(vec![ints(&[1, 2]), ForthicValue::Int(3)])
    );
}

#[test]
fn test_flatten_records_to_tab_joined_key_paths() {
    let result = run("[ [ 'a' [ [ 'b' 1 ] ] REC ] [ 'c' 2 ] ] REC FLATTEN");
    match result {
        ForthicValue::Record(rec) => {
            assert_eq!(rec.get("a\tb"), Some(&ForthicValue::Int(1)));
            assert_eq!(rec.get("c"), Some(&ForthicValue::Int(2)));
        }
        other => panic!("expected record, got {other:?}"),
    }
}

#[test]
fn test_flatten_null_is_empty_array() {
    assert_eq!(run("NULL FLATTEN"), ForthicValue::Array(vec![]));
}
