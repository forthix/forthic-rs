//! TRY tests — error handling as data (backlog item 20, mirrored with ts)
//!
//! Rust Result is the model: Forthic's default propagation is `?`; TRY holds
//! the error as a value. Law: `'CODE' TRY UNWRAP` ≡ `CODE`. TRY is
//! transactional for the stack; MAP's outcomes option owns per-element error
//! tolerance (option A from the design discussion — TRY inside MAP would
//! restore the pushed item and strand it).

// ForthicError is large; accepted trade-off (see lib.rs / backlog item 11)
#![allow(clippy::result_large_err)]

use forthic::interpreter::Interpreter;
use forthic::literals::ForthicValue;

fn run(code: &str) -> ForthicValue {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap();
    interp.get_stack_mut().pop().unwrap()
}

fn run_all(code: &str) -> Vec<ForthicValue> {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap();
    interp.get_stack().items().to_vec()
}

fn ok_of(value: &ForthicValue) -> Option<&ForthicValue> {
    match value {
        ForthicValue::Record(rec) => rec.get("ok"),
        _ => None,
    }
}

fn error_info_of(value: &ForthicValue) -> Option<&ForthicValue> {
    match value {
        ForthicValue::Record(rec) => rec.get("error"),
        _ => None,
    }
}

fn error_field(outcome: &ForthicValue, field: &str) -> String {
    match error_info_of(outcome) {
        Some(ForthicValue::Record(info)) => info
            .get(field)
            .and_then(|v| v.as_string())
            .unwrap_or_default()
            .to_string(),
        _ => panic!("expected error outcome, got {outcome:?}"),
    }
}

// ===== TRY basics + the law =====

#[test]
fn test_try_wraps_success() {
    let outcome = run("5 '2 *' TRY");
    assert_eq!(ok_of(&outcome), Some(&ForthicValue::Int(10)));
}

#[test]
fn test_law_try_unwrap_equals_code_on_success() {
    assert_eq!(run("5 '2 *' TRY UNWRAP"), ForthicValue::Int(10));
}

#[test]
fn test_law_unwrap_reraises_with_message_and_type() {
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("'NO-SUCH-WORD' TRY UNWRAP").unwrap_err();
    let message = err.to_string();
    assert!(message.contains("NO-SUCH-WORD"), "got: {message}");
    assert!(message.contains("UnknownWord"), "type preserved: {message}");
}

#[test]
fn test_try_wraps_failure_with_message_and_error_type() {
    let outcome = run("'NO-SUCH-WORD' TRY");
    assert!(error_field(&outcome, "message").contains("NO-SUCH-WORD"));
    assert_eq!(error_field(&outcome, "error_type"), "UnknownWord");
}

// ===== Transactionality =====

#[test]
fn test_try_is_transactional_for_the_stack_on_failure() {
    // The failing code consumes 2 and would have kept going; afterwards the
    // stack must be exactly [1, 2, outcome]
    let stack = run_all("1 2 'DROP DROP NO-SUCH-WORD' TRY");
    assert_eq!(stack.len(), 3);
    assert_eq!(stack[0], ForthicValue::Int(1));
    assert_eq!(stack[1], ForthicValue::Int(2));
    assert!(error_info_of(&stack[2]).is_some());
}

#[test]
fn test_try_does_not_roll_back_side_effects() {
    // catch_unwind semantics: the variable write before the failure persists
    assert_eq!(
        run("'42 .written ! NO-SUCH-WORD' TRY DROP .written @"),
        ForthicValue::Int(42)
    );
}

#[test]
fn test_try_unwinds_modules_left_open_by_failed_code() {
    let mut interp = Interpreter::standard("UTC");
    interp.run("'{my-mod NO-SUCH-WORD' TRY").unwrap();
    let outcome = interp.get_stack_mut().pop().unwrap();
    assert!(error_info_of(&outcome).is_some());
    // If the module were still open, this unknown word would resolve
    // against my-mod's (empty) dictionary the same way — instead prove the
    // stack is usable and a stray } errors (we're back at the app module)
    assert!(interp.run("}").is_err(), "module stack fully unwound");
}

#[test]
fn test_try_success_consumes_inputs_legitimately() {
    let stack = run_all("1 2 '+' TRY");
    assert_eq!(stack.len(), 1);
    assert_eq!(ok_of(&stack[0]), Some(&ForthicValue::Int(3)));
}

#[test]
fn test_try_net_zero_code_succeeds_with_ok_null() {
    let outcome = run("'1 DROP' TRY");
    assert_eq!(ok_of(&outcome), Some(&ForthicValue::Null));
}

// ===== OK? / ERROR? / UNWRAP-OR =====

#[test]
fn test_ok_and_error_discriminate() {
    assert_eq!(run("'1' TRY OK?"), ForthicValue::Bool(true));
    assert_eq!(run("'1' TRY ERROR?"), ForthicValue::Bool(false));
    assert_eq!(run("'NO-SUCH-WORD' TRY OK?"), ForthicValue::Bool(false));
    assert_eq!(run("'NO-SUCH-WORD' TRY ERROR?"), ForthicValue::Bool(true));
}

#[test]
fn test_unwrap_or_fallbacks() {
    assert_eq!(run("'5' TRY 0 UNWRAP-OR"), ForthicValue::Int(5));
    assert_eq!(run("'NO-SUCH-WORD' TRY 0 UNWRAP-OR"), ForthicValue::Int(0));
}

#[test]
fn test_unwrap_or_ok_null_beats_default() {
    // Failure is not nullness
    assert_eq!(run("'NULL' TRY 99 UNWRAP-OR"), ForthicValue::Null);
}

#[test]
fn test_unwrap_is_structural() {
    // Hand-built ok records participate (records are records)
    assert_eq!(run("[ [ 'ok' 7 ] ] REC UNWRAP"), ForthicValue::Int(7));
}

#[test]
fn test_unwrap_rejects_non_outcomes() {
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("[ [ 'other' 1 ] ] REC UNWRAP").unwrap_err();
    assert!(err.to_string().contains("outcome record"), "got: {err}");
}

// ===== MAP outcomes (option A) =====

#[test]
fn test_map_outcomes_wraps_successes() {
    let result = run("[ 1 2 ] '2 *' [ .outcomes TRUE ] ~> MAP");
    match result {
        ForthicValue::Array(items) => {
            assert_eq!(ok_of(&items[0]), Some(&ForthicValue::Int(2)));
            assert_eq!(ok_of(&items[1]), Some(&ForthicValue::Int(4)));
        }
        other => panic!("expected array, got {other:?}"),
    }
}

#[test]
fn test_map_outcomes_failures_strand_nothing() {
    let stack = run_all("[ 1 2 ] 'NO-SUCH-WORD' [ .outcomes TRUE ] ~> MAP");
    // MAP owns its pushes: exactly one result container, nothing stranded
    assert_eq!(stack.len(), 1);
    match &stack[0] {
        ForthicValue::Array(items) => {
            assert_eq!(items.len(), 2);
            for item in items {
                assert_eq!(error_field(item, "error_type"), "UnknownWord");
            }
        }
        other => panic!("expected array, got {other:?}"),
    }
}

#[test]
fn test_map_outcomes_mixes_success_and_failure() {
    // Items that are themselves Forthic strings: mapping with bare 'TRY'
    // works (the item IS TRY's code argument, so TRY consumes it), and the
    // garbage item yields an error outcome without aborting the map
    let stack = run_all("[ '5' 'NO-SUCH-WORD' ] 'TRY' MAP");
    assert_eq!(stack.len(), 1);
    match &stack[0] {
        ForthicValue::Array(items) => {
            assert_eq!(ok_of(&items[0]), Some(&ForthicValue::Int(5)));
            assert!(error_info_of(&items[1]).is_some());
        }
        other => panic!("expected array, got {other:?}"),
    }
}

#[test]
fn test_map_outcomes_with_depth() {
    let result = run("[ [ 1 2 ] 5 ] '2 *' [ .depth 1 .outcomes TRUE ] ~> MAP");
    match result {
        ForthicValue::Array(items) => {
            match &items[0] {
                ForthicValue::Array(inner) => {
                    assert_eq!(ok_of(&inner[0]), Some(&ForthicValue::Int(2)));
                    assert_eq!(ok_of(&inner[1]), Some(&ForthicValue::Int(4)));
                }
                other => panic!("expected inner array, got {other:?}"),
            }
            // Scalar leaf at depth is also wrapped
            assert_eq!(ok_of(&items[1]), Some(&ForthicValue::Int(10)));
        }
        other => panic!("expected array, got {other:?}"),
    }
}

#[test]
fn test_try_inside_map_restores_items_the_documented_reason_for_outcomes() {
    // TRY is transactional: its snapshot includes the item MAP pushed, so a
    // failing element is faithfully restored... beneath the outcome record.
    // Correct TRY behavior, wrong tool for mapping — use outcomes mode.
    let stack = run_all("[ 1 2 ] \"'NO-SUCH-WORD' TRY\" MAP");
    assert_eq!(stack.len(), 3, "restored items + result array");
    assert_eq!(stack[0], ForthicValue::Int(1));
    assert_eq!(stack[1], ForthicValue::Int(2));
    match &stack[2] {
        ForthicValue::Array(items) => assert!(error_info_of(&items[0]).is_some()),
        other => panic!("expected outcome array, got {other:?}"),
    }
}
