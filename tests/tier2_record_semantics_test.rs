//! Tier 2 regression tests: insertion-ordered records and the record
//! contracts for container words (plans/TS-PARITY-BACKLOG.md items 6-8,
//! per the post-scrub ts #31/#33 behavior).

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

fn run_all(code: &str) -> Vec<ForthicValue> {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap();
    interp.get_stack().items().to_vec()
}

fn int(i: i64) -> ForthicValue {
    ForthicValue::Int(i)
}

fn s(v: &str) -> ForthicValue {
    ForthicValue::String(v.to_string())
}

/// A record with deliberately non-alphabetical insertion order: z, a, m
const ZAM: &str = "[ [ 'z' 1 ] [ 'a' 2 ] [ 'm' 3 ] ] REC";

fn record_keys(value: &ForthicValue) -> Vec<String> {
    match value {
        ForthicValue::Record(rec) => rec.keys().cloned().collect(),
        other => panic!("expected record, got {other:?}"),
    }
}

// ===== Insertion order (IndexMap switch) =====

#[test]
fn test_record_preserves_insertion_order() {
    assert_eq!(record_keys(&run(ZAM)), vec!["z", "a", "m"]);
}

#[test]
fn test_keys_and_values_follow_insertion_order() {
    let keys = run(&format!("{ZAM} KEYS"));
    assert_eq!(keys, ForthicValue::Array(vec![s("z"), s("a"), s("m")]));
    let values = run(&format!("{ZAM} VALUES"));
    assert_eq!(values, ForthicValue::Array(vec![int(1), int(2), int(3)]));
}

#[test]
fn test_nth_first_last_use_insertion_order() {
    // Sorted-key order would give a=2 first and z=1 last; insertion order
    // gives z=1 first and m=3 last
    assert_eq!(run(&format!("{ZAM} 0 NTH")), int(1));
    assert_eq!(run(&format!("{ZAM} FIRST")), int(1));
    assert_eq!(run(&format!("{ZAM} LAST")), int(3));
    assert_eq!(run(&format!("{ZAM} 1 NTH")), int(2));
}

#[test]
fn test_to_json_is_insertion_ordered() {
    assert_eq!(run(&format!("{ZAM} >JSON")), s(r#"{"z":1,"a":2,"m":3}"#));
}

#[test]
fn test_json_round_trip_preserves_order() {
    let result = run(r#"'{"z":1,"a":2,"m":3}' JSON> >JSON"#);
    assert_eq!(result, s(r#"{"z":1,"a":2,"m":3}"#));
}

// ===== TAKE / DROP / TAKE-LAST on records =====

#[test]
fn test_take_on_record_preserves_shape_and_order() {
    let taken = run(&format!("{ZAM} 2 TAKE"));
    assert_eq!(record_keys(&taken), vec!["z", "a"]);
}

#[test]
fn test_take_push_rest_option() {
    let stack = run_all(&format!("{ZAM} 2 [ .push_rest TRUE ] ~> TAKE"));
    assert_eq!(stack.len(), 2);
    assert_eq!(record_keys(&stack[0]), vec!["z", "a"]);
    assert_eq!(record_keys(&stack[1]), vec!["m"]);
}

#[test]
fn test_take_push_rest_on_arrays() {
    let stack = run_all("[ 1 2 3 ] 2 [ .push_rest TRUE ] ~> TAKE");
    assert_eq!(stack[0], ForthicValue::Array(vec![int(1), int(2)]));
    assert_eq!(stack[1], ForthicValue::Array(vec![int(3)]));
}

#[test]
fn test_drop_on_record() {
    let rest = run(&format!("{ZAM} 1 DROP"));
    assert_eq!(record_keys(&rest), vec!["a", "m"]);
    // n <= 0 drops nothing
    let unchanged = run(&format!("{ZAM} 0 DROP"));
    assert_eq!(record_keys(&unchanged), vec!["z", "a", "m"]);
}

#[test]
fn test_take_last() {
    assert_eq!(
        run("[ 1 2 3 4 ] 2 TAKE-LAST"),
        ForthicValue::Array(vec![int(3), int(4)])
    );
    let tail = run(&format!("{ZAM} 2 TAKE-LAST"));
    assert_eq!(record_keys(&tail), vec!["a", "m"]);
    assert_eq!(run("[ 1 2 ] 0 TAKE-LAST"), ForthicValue::Array(vec![]));
}

// ===== SLICE on records + span guard =====

#[test]
fn test_slice_on_record() {
    let sliced = run(&format!("{ZAM} 0 1 SLICE"));
    assert_eq!(record_keys(&sliced), vec!["z", "a"]);
    // Negative indexes count from the end
    let tail = run(&format!("{ZAM} -2 -1 SLICE"));
    assert_eq!(record_keys(&tail), vec!["a", "m"]);
}

#[test]
fn test_slice_record_skips_out_of_range() {
    // Arrays null-pad out-of-range; records skip (ts #33)
    let sliced = run(&format!("{ZAM} 1 5 SLICE"));
    assert_eq!(record_keys(&sliced), vec!["a", "m"]);
    let padded = run("[ 1 2 3 ] 1 4 SLICE");
    assert_eq!(
        padded,
        ForthicValue::Array(vec![int(2), int(3), ForthicValue::Null, ForthicValue::Null])
    );
}

#[test]
fn test_slice_span_guard() {
    // Materializing a ~billion-element span must error, not OOM
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("[ 1 ] 0 999999999 SLICE").unwrap_err();
    match err {
        ForthicError::InvalidOperation { message, .. } => {
            assert!(message.contains("too large"), "got: {message}");
        }
        other => panic!("expected InvalidOperation, got {other}"),
    }
}

#[test]
fn test_del_preserves_order_of_remaining_entries() {
    // IndexMap's plain remove() is a swap_remove — deleting 'z' would move
    // 'm' to the front. shift_remove keeps a, m in order.
    let rest = run(&format!("{ZAM} 'z' <DEL"));
    assert_eq!(record_keys(&rest), vec!["a", "m"]);
}

// ===== UNPACK on records =====

#[test]
fn test_unpack_record_pushes_values_in_insertion_order() {
    let stack = run_all(&format!("{ZAM} UNPACK"));
    assert_eq!(stack, vec![int(1), int(2), int(3)]);
}

// ===== DIFFERENCE / INTERSECTION =====

#[test]
fn test_set_ops_on_arrays() {
    assert_eq!(
        run("[ 1 2 3 ] [ 2 ] DIFFERENCE"),
        ForthicValue::Array(vec![int(1), int(3)])
    );
    assert_eq!(
        run("[ 1 2 3 ] [ 2 4 ] INTERSECTION"),
        ForthicValue::Array(vec![int(2)])
    );
}

#[test]
fn test_record_left_set_ops_behave_like_pick_and_omit() {
    // INTERSECTION with an array of keys = PICK
    let picked = run(&format!("{ZAM} [ 'z' 'm' ] INTERSECTION"));
    assert_eq!(record_keys(&picked), vec!["z", "m"]);
    // DIFFERENCE with an array of keys = OMIT
    let omitted = run(&format!("{ZAM} [ 'z' ] DIFFERENCE"));
    assert_eq!(record_keys(&omitted), vec!["a", "m"]);
    // Record right operand: membership by its keys
    let picked2 = run(&format!("{ZAM} [ [ 'a' 99 ] ] REC INTERSECTION"));
    assert_eq!(record_keys(&picked2), vec!["a"]);
}

#[test]
fn test_array_left_record_right_uses_values() {
    // ts: array-left membership tests against Object.values(right)
    let result = run("[ 1 2 3 ] [ [ 'x' 2 ] ] REC INTERSECTION");
    assert_eq!(result, ForthicValue::Array(vec![int(2)]));
}

#[test]
fn test_set_ops_unify_int_and_float() {
    // JS has one number type: 1 and 1.0 are the same set element
    assert_eq!(
        run("[ 1 2 ] [ 1.0 ] INTERSECTION"),
        ForthicValue::Array(vec![int(1)])
    );
}

// ===== >STR (coordinated ts/rs contract — Tier 5 item 17) =====

#[test]
fn test_to_str_matches_js_semantics() {
    assert_eq!(run("NULL >STR"), s(""));
    assert_eq!(run("TRUE >STR"), s("true"));
    assert_eq!(run("42 >STR"), s("42"));
    assert_eq!(run("3.25 >STR"), s("3.25"));
    // JS (3.0).toString() === "3"; Rust Display agrees
    assert_eq!(run("3.0 >STR"), s("3"));
    // JS Array.toString: comma-join, null elements empty, nested flattened
    assert_eq!(run("[ 1 NULL [ 2 3 ] ] >STR"), s("1,,2,3"));
    // Temporal-style ISO forms
    assert_eq!(run("2020-06-05 >STR"), s("2020-06-05"));
    assert_eq!(run("9:30 >STR"), s("09:30:00"));
}

#[test]
fn test_to_str_renders_records_as_json() {
    // Coordinated contract change (both repos): insertion-ordered JSON
    // instead of "[object Object]"
    assert_eq!(run(&format!("{ZAM} >STR")), s(r#"{"z":1,"a":2,"m":3}"#));
    // Record elements inside arrays render as JSON within the comma-join
    assert_eq!(
        run("[ [ [ 'a' 1 ] ] REC [ [ 'b' 2 ] ] REC ] >STR"),
        s(r#"{"a":1},{"b":2}"#)
    );
    // Temporal values inside records use their ISO forms (ts Temporal.toJSON)
    assert_eq!(
        run("[ [ 'd' 2020-06-05 ] ] REC >STR"),
        s(r#"{"d":"2020-06-05"}"#)
    );
}

// ===== Wire round-trip order (jsonrpc feature) =====

#[cfg(feature = "jsonrpc")]
#[test]
fn test_record_order_survives_the_wire() {
    use forthic::jsonrpc::{deserialize_value, serialize_value};
    let record = run(ZAM);
    let wire = serialize_value(&record).unwrap();
    let back = deserialize_value(&wire).unwrap();
    assert_eq!(record_keys(&back), vec!["z", "a", "m"]);
    // And the JSON text itself is insertion-ordered (preserve_order)
    assert_eq!(
        wire["record_value"]["fields"].to_string(),
        r#"{"z":{"int_value":1},"a":{"int_value":2},"m":{"int_value":3}}"#
    );
}
