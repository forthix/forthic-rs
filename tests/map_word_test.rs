//! MAP tests (plans/TS-PARITY-BACKLOG.md item 14, first higher-order word)
//!
//! MAP is the first word to execute Forthic from within a word, via the new
//! InterpreterContext::run. Contracts ported from post-#31 ts (depth maps
//! scalar leaves; record in -> record out in insertion order).

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

fn ints(values: &[i64]) -> ForthicValue {
    ForthicValue::Array(values.iter().map(|i| ForthicValue::Int(*i)).collect())
}

// ===== Basics =====

#[test]
fn test_map_over_array() {
    assert_eq!(run("[ 1 2 3 ] '2 *' MAP"), ints(&[2, 4, 6]));
}

#[test]
fn test_map_with_definition_word() {
    assert_eq!(
        run(": DOUBLE 2 * ; [ 1 2 3 ] 'DOUBLE' MAP"),
        ints(&[2, 4, 6])
    );
}

#[test]
fn test_map_over_record_preserves_keys_and_order() {
    let result = run("[ [ 'z' 1 ] [ 'a' 2 ] ] REC '10 *' MAP");
    match result {
        ForthicValue::Record(rec) => {
            let entries: Vec<_> = rec.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            assert_eq!(
                entries,
                vec![
                    ("z".to_string(), ForthicValue::Int(10)),
                    ("a".to_string(), ForthicValue::Int(20)),
                ]
            );
        }
        other => panic!("expected record, got {other:?}"),
    }
}

#[test]
fn test_map_empty_and_null() {
    assert_eq!(run("[ ] '2 *' MAP"), ForthicValue::Array(vec![]));
    assert_eq!(run("NULL '2 *' MAP"), ForthicValue::Null);
}

#[test]
fn test_map_forthic_can_be_multi_word() {
    assert_eq!(run("[ 1 2 ] '1 + 10 *' MAP"), ints(&[20, 30]));
}

// ===== with_key =====

#[test]
fn test_map_with_key_on_arrays_pushes_index() {
    // index + value: [10+0, 20+1]
    assert_eq!(
        run("[ 10 20 ] '+' [ .with_key TRUE ] ~> MAP"),
        ints(&[10, 21])
    );
}

#[test]
fn test_map_with_key_on_records_pushes_key() {
    let result = run("[ [ 'a' 1 ] ] REC 'DROP' [ .with_key TRUE ] ~> MAP");
    // DROP drops the value, leaving the key as the mapped result
    match result {
        ForthicValue::Record(rec) => {
            assert_eq!(rec.get("a"), Some(&ForthicValue::String("a".to_string())));
        }
        other => panic!("expected record, got {other:?}"),
    }
}

// ===== depth (post ts #31 semantics) =====

#[test]
fn test_map_depth_descends_nested_arrays() {
    assert_eq!(
        run("[ [ 1 2 ] [ 3 ] ] '2 *' [ .depth 1 ] ~> MAP"),
        ForthicValue::Array(vec![ints(&[2, 4]), ints(&[6])])
    );
}

#[test]
fn test_map_depth_maps_scalar_leaves() {
    // The ts #31 fix: a scalar sitting at depth must be mapped, not
    // coerced into an empty container
    assert_eq!(
        run("[ [ 1 2 ] 5 ] '2 *' [ .depth 1 ] ~> MAP"),
        ForthicValue::Array(vec![ints(&[2, 4]), ForthicValue::Int(10)])
    );
}

#[test]
fn test_map_depth_descends_records_inside_arrays() {
    let result = run("[ [ [ 'a' 1 ] ] REC ] '2 *' [ .depth 1 ] ~> MAP");
    match result {
        ForthicValue::Array(items) => match &items[0] {
            ForthicValue::Record(rec) => {
                assert_eq!(rec.get("a"), Some(&ForthicValue::Int(2)));
            }
            other => panic!("expected record, got {other:?}"),
        },
        other => panic!("expected array, got {other:?}"),
    }
}

// ===== Error propagation =====
// MAP has no push_error option (deliberately not ported from ts — see
// backlog item 20). Errors propagate; error-tolerant mapping composes with
// TRY once it lands: [xs] "'F' TRY" MAP.

#[test]
fn test_map_aborts_on_failure() {
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("[ 1 2 ] 'NO-SUCH-WORD' MAP").unwrap_err();
    assert!(err.to_string().contains("NO-SUCH-WORD"), "got: {err}");
}

#[test]
fn test_map_ignores_unknown_options() {
    // push_error is not an option; passing it has no effect on arity —
    // exactly one result comes back
    let stack = run_all("[ 1 2 ] '2 *' [ .push_error TRUE ] ~> MAP");
    assert_eq!(stack.len(), 1, "one result, no errors array");
    assert_eq!(stack[0], ints(&[2, 4]));
}

// ===== Reentrancy =====

#[test]
fn test_nested_map() {
    // MAP inside MAP: the inner run() nests a tokenizer on the same
    // interpreter
    assert_eq!(
        run("[ [ 1 2 ] [ 3 ] ] \"'2 *' MAP\" MAP"),
        ForthicValue::Array(vec![ints(&[2, 4]), ints(&[6])])
    );
}

#[test]
fn test_map_code_can_use_variables() {
    // The mapped code runs in the same interpreter scope
    assert_eq!(
        run("100 .base ! [ 1 2 ] '.base @ +' MAP"),
        ints(&[101, 102])
    );
}
