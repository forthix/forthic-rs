//! Batch 1 words: control flow, predicates, membership, debug
//! (plans/WORD-INVENTORY.md — post-scrub ts contracts)

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

fn b(v: bool) -> ForthicValue {
    ForthicValue::Bool(v)
}

// ===== RUN =====

#[test]
fn test_run_executes_in_current_context() {
    assert_eq!(run("'40 2 +' RUN"), ForthicValue::Int(42));
    // Shares interpreter scope
    assert_eq!(run("7 .x ! '.x @' RUN"), ForthicValue::Int(7));
}

#[test]
fn test_run_null_and_empty_are_noops() {
    let mut interp = Interpreter::standard("UTC");
    interp.run("1 NULL RUN '' RUN").unwrap();
    assert_eq!(interp.get_stack().items(), &[ForthicValue::Int(1)]);
}

// ===== IF / IF-RUN / WHEN =====

#[test]
fn test_if_is_pure_value_selection() {
    assert_eq!(run("TRUE 1 2 IF"), ForthicValue::Int(1));
    assert_eq!(run("FALSE 1 2 IF"), ForthicValue::Int(2));
    // The post-scrub contract: IF does NOT execute — strings stay strings
    assert_eq!(
        run("TRUE '1 +' 'x' IF"),
        ForthicValue::String("1 +".to_string())
    );
}

#[test]
fn test_if_uses_js_truthiness() {
    assert_eq!(run("0 1 2 IF"), ForthicValue::Int(2));
    assert_eq!(run("'' 1 2 IF"), ForthicValue::Int(2));
    assert_eq!(run("NULL 1 2 IF"), ForthicValue::Int(2));
    // Empty arrays are TRUTHY (JS Boolean([]) === true)
    assert_eq!(run("[ ] 1 2 IF"), ForthicValue::Int(1));
}

#[test]
fn test_if_run_executes_the_chosen_branch() {
    assert_eq!(run("TRUE '40 2 +' '0' IF-RUN"), ForthicValue::Int(42));
    assert_eq!(run("FALSE '40 2 +' '0' IF-RUN"), ForthicValue::Int(0));
}

#[test]
fn test_if_run_null_branch_is_noop() {
    let mut interp = Interpreter::standard("UTC");
    interp.run("9 FALSE '1' NULL IF-RUN").unwrap();
    assert_eq!(interp.get_stack().items(), &[ForthicValue::Int(9)]);
}

#[test]
fn test_when_runs_only_on_truthy() {
    assert_eq!(run("1 TRUE '10 *' WHEN"), ForthicValue::Int(10));
    assert_eq!(run("1 FALSE '10 *' WHEN"), ForthicValue::Int(1));
}

// ===== DEFAULT-RUN =====

#[test]
fn test_default_run_is_lazy() {
    // Non-empty value: forthic never runs
    assert_eq!(run("5 'NO-SUCH-WORD' DEFAULT-RUN"), ForthicValue::Int(5));
    // NULL and "" trigger the default computation
    assert_eq!(run("NULL '40 2 +' DEFAULT-RUN"), ForthicValue::Int(42));
    assert_eq!(run("'' '40 2 +' DEFAULT-RUN"), ForthicValue::Int(42));
}

// ===== Predicates =====

#[test]
fn test_null_q() {
    assert_eq!(run("NULL NULL?"), b(true));
    assert_eq!(run("0 NULL?"), b(false));
    assert_eq!(run("'' NULL?"), b(false));
}

#[test]
fn test_empty_q() {
    assert_eq!(run("NULL EMPTY?"), b(true));
    assert_eq!(run("'' EMPTY?"), b(true));
    assert_eq!(run("[ ] EMPTY?"), b(true));
    assert_eq!(run("[ [ 'a' 1 ] ] REC 'a' <DEL EMPTY?"), b(true));
    assert_eq!(run("'x' EMPTY?"), b(false));
    assert_eq!(run("[ 1 ] EMPTY?"), b(false));
    assert_eq!(run("0 EMPTY?"), b(false));
}

#[test]
fn test_string_q_and_record_q() {
    assert_eq!(run("'x' STRING?"), b(true));
    assert_eq!(run("1 STRING?"), b(false));
    assert_eq!(run("[ [ 'a' 1 ] ] REC RECORD?"), b(true));
    assert_eq!(run("[ 1 ] RECORD?"), b(false));
    assert_eq!(run("NULL RECORD?"), b(false));
}

#[test]
fn test_number_q_infinity_yes_nan_no() {
    // ts #31 contract
    assert_eq!(run("42 NUMBER?"), b(true));
    assert_eq!(run("3.25 NUMBER?"), b(true));
    assert_eq!(run("'42' NUMBER?"), b(false));

    let mut interp = Interpreter::standard("UTC");
    interp.stack_push(ForthicValue::Float(f64::INFINITY));
    interp.run("NUMBER?").unwrap();
    assert_eq!(interp.get_stack_mut().pop().unwrap(), b(true));
    interp.stack_push(ForthicValue::Float(f64::NAN));
    interp.run("NUMBER?").unwrap();
    assert_eq!(interp.get_stack_mut().pop().unwrap(), b(false));
}

// ===== CONTAINS? / ANY? / ALL? =====

#[test]
fn test_contains_q_is_haystack_first() {
    assert_eq!(run("[ 1 2 3 ] 2 CONTAINS?"), b(true));
    assert_eq!(run("[ 1 2 3 ] 9 CONTAINS?"), b(false));
    // Non-array haystack is false, not an error
    assert_eq!(run("NULL 2 CONTAINS?"), b(false));
}

#[test]
fn test_classic_in_is_gone() {
    // Tombstone: classic item-first IN dropped when CONTAINS? landed
    let mut interp = Interpreter::standard("UTC");
    assert!(interp.run("2 [ 1 2 ] IN").is_err());
}

#[test]
fn test_any_q_and_all_q() {
    assert_eq!(run("[ FALSE TRUE ] ANY?"), b(true));
    assert_eq!(run("[ FALSE FALSE ] ANY?"), b(false));
    assert_eq!(run("[ ] ANY?"), b(false), "any of nothing is false");
    assert_eq!(run("[ TRUE TRUE ] ALL?"), b(true));
    assert_eq!(run("[ TRUE FALSE ] ALL?"), b(false));
    assert_eq!(run("[ ] ALL?"), b(true), "all of nothing is vacuously true");
    // JS truthiness on elements
    assert_eq!(run("[ 1 'x' ] ALL?"), b(true));
    assert_eq!(run("[ 1 0 ] ALL?"), b(false));
}

#[test]
fn test_any_q_requires_an_array() {
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("5 ANY?").unwrap_err();
    assert!(err.to_string().contains("requires an array"), "got: {err}");
}

// ===== >BOOL truthiness fixes =====

#[test]
fn test_bool_empty_array_is_truthy_and_nan_is_falsy() {
    // JS semantics: Boolean([]) === true — the old rs is_truthy had empty
    // arrays falsy and NaN truthy, both wrong
    assert_eq!(run("[ ] >BOOL"), b(true));
    let mut interp = Interpreter::standard("UTC");
    interp.stack_push(ForthicValue::Float(f64::NAN));
    interp.run(">BOOL").unwrap();
    assert_eq!(interp.get_stack_mut().pop().unwrap(), b(false));
}

// ===== PEEK! / STACK! =====

#[test]
fn test_peek_and_stack_stop_intentionally() {
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("42 PEEK!").unwrap_err();
    assert!(matches!(err, ForthicError::IntentionalStop { .. }));
    // The stack survives — PEEK! only peeks
    assert_eq!(interp.get_stack().items(), &[ForthicValue::Int(42)]);

    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("1 2 STACK!").unwrap_err();
    assert!(matches!(err, ForthicError::IntentionalStop { .. }));
}
