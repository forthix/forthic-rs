//! Batch 3 words: records & JQ paths (plans/WORD-INVENTORY.md)
//! Documented divergences from ts: records iterate/index/enumerate in
//! INSERTION order (ts sorts keys as a JS-object-order workaround);
//! strict integer parsing in [n]; OMIT stringifies drop keys.

// ForthicError is large; accepted trade-off (see lib.rs / backlog item 11)
#![allow(clippy::result_large_err)]

use forthic::interpreter::Interpreter;
use forthic::literals::ForthicValue;

fn run(code: &str) -> ForthicValue {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap();
    interp.get_stack_mut().pop().unwrap()
}

fn run_err(code: &str) -> forthic::errors::ForthicError {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap_err()
}

fn s(v: &str) -> ForthicValue {
    ForthicValue::String(v.to_string())
}

fn ints(values: &[i64]) -> ForthicValue {
    ForthicValue::Array(values.iter().map(|i| ForthicValue::Int(*i)).collect())
}

fn record_keys(value: &ForthicValue) -> Vec<String> {
    match value {
        ForthicValue::Record(rec) => rec.keys().cloned().collect(),
        other => panic!("expected record, got {other:?}"),
    }
}

/// users: [{name: alice, tags: [a b]}, {name: bob, tags: [c]}]
const USERS: &str = "[ [ [ 'name' 'alice' ] [ 'tags' [ 'a' 'b' ] ] ] REC \
                       [ [ 'name' 'bob' ] [ 'tags' [ 'c' ] ] ] REC ]";

// ===== JQ@ =====

#[test]
fn test_jq_at_string_paths() {
    assert_eq!(
        run("[ [ 'a' [ [ 'b' 7 ] ] REC ] ] REC 'a.b' JQ@"),
        ForthicValue::Int(7)
    );
    assert_eq!(run(&format!("{USERS} '[0].name' JQ@")), s("alice"));
    assert_eq!(run("[ 10 20 30 ] '[-1]' JQ@"), ForthicValue::Int(30));
    // Quoted keys for names with dots/brackets
    assert_eq!(
        run("[ [ 'a.b' 5 ] ] REC '[\"a.b\"]' JQ@"),
        ForthicValue::Int(5)
    );
}

#[test]
fn test_jq_at_misses_are_null() {
    assert_eq!(run("[ [ 'a' 1 ] ] REC 'zzz' JQ@"), ForthicValue::Null);
    assert_eq!(run("[ [ 'a' 1 ] ] REC 'a.b.c' JQ@"), ForthicValue::Null);
    assert_eq!(run("[ 1 ] '[9]' JQ@"), ForthicValue::Null);
    assert_eq!(run("NULL 'a' JQ@"), ForthicValue::Null);
}

#[test]
fn test_jq_at_iterate_flattens_conditionally() {
    // Single []: one level of mapping, flat result
    assert_eq!(
        run(&format!("{USERS} '[].name' JQ@")),
        ForthicValue::Array(vec![s("alice"), s("bob")])
    );
    // .[].tags -> array of arrays (no later iterate)
    assert_eq!(
        run(&format!("{USERS} '[].tags' JQ@")),
        ForthicValue::Array(vec![
            ForthicValue::Array(vec![s("a"), s("b")]),
            ForthicValue::Array(vec![s("c")]),
        ])
    );
    // .[].tags[] -> flattened
    assert_eq!(
        run(&format!("{USERS} '[].tags[]' JQ@")),
        ForthicValue::Array(vec![s("a"), s("b"), s("c")])
    );
}

#[test]
fn test_jq_at_array_paths_are_dynamic_keys() {
    assert_eq!(run(&format!("{USERS} [ 0 'tags' 1 ] JQ@")), s("b"));
}

#[test]
fn test_jq_at_record_index_uses_insertion_order() {
    // Documented divergence: ts indexes records by sorted keys; rs by
    // insertion order (z first here)
    assert_eq!(
        run("[ [ 'z' 1 ] [ 'a' 2 ] ] REC '[0]' JQ@"),
        ForthicValue::Int(1)
    );
}

#[test]
fn test_jq_path_strict_integer_parse() {
    // ts parseInt('1x') == 1 silently; rs errors (fixed by design)
    let err = run_err("[ 1 2 ] '[1x]' JQ@");
    assert!(err.to_string().contains("invalid index"), "got: {err}");
}

// ===== JQ! =====

#[test]
fn test_jq_set_deep_with_autocreate() {
    // Missing intermediates auto-create by NEXT segment kind
    let result = run("NULL 42 'a.b[0]' JQ!");
    assert_eq!(run_code_on(&result, "'a.b' JQ@"), ints(&[42]));
    // Existing values untouched elsewhere
    let result = run("[ [ 'keep' 1 ] ] REC 2 'new' JQ!");
    assert_eq!(record_keys(&result), vec!["keep", "new"]);
}

fn run_code_on(value: &ForthicValue, code: &str) -> ForthicValue {
    let mut interp = Interpreter::standard("UTC");
    interp.stack_push(value.clone());
    interp.run(code).unwrap();
    interp.get_stack_mut().pop().unwrap()
}

#[test]
fn test_jq_set_pads_arrays_with_null() {
    // ts leaves JS holes; rs pads explicitly
    let result = run("[ 1 ] 9 '[3]' JQ!");
    assert_eq!(
        result,
        ForthicValue::Array(vec![
            ForthicValue::Int(1),
            ForthicValue::Null,
            ForthicValue::Null,
            ForthicValue::Int(9)
        ])
    );
}

#[test]
fn test_jq_set_rejects_iterate_and_bad_shapes() {
    let err = run_err("[ [ 'a' 1 ] ] REC 5 'a[]' JQ!");
    assert!(
        err.to_string().contains("[] iteration not supported"),
        "got: {err}"
    );
    let err = run_err("[ 1 2 ] 5 'field' JQ!");
    assert!(err.to_string().contains("cannot set field"), "got: {err}");
    let err = run_err("[ 1 2 ] 5 '[-1]' JQ!");
    assert!(err.to_string().contains("negative set index"), "got: {err}");
}

#[test]
fn test_jq_set_empty_path_replaces_container() {
    assert_eq!(run("[ 1 2 ] 42 '' JQ!"), ForthicValue::Int(42));
}

// ===== JQ-DEL =====

#[test]
fn test_jq_del() {
    let result = run("[ [ 'a' 1 ] [ 'b' 2 ] ] REC 'a' JQ-DEL");
    assert_eq!(record_keys(&result), vec!["b"]);
    // Array delete shifts left
    assert_eq!(run("[ 1 2 3 ] '[1]' JQ-DEL"), ints(&[1, 3]));
    // Missing paths: silent no-op
    let result = run("[ [ 'a' 1 ] ] REC 'x.y.z' JQ-DEL");
    assert_eq!(record_keys(&result), vec!["a"]);
    // Iterate rejected
    let err = run_err("[ [ 'a' 1 ] ] REC '[]' JQ-DEL");
    assert!(
        err.to_string().contains("not supported in delete"),
        "got: {err}"
    );
}

// ===== MERGE / PICK / OMIT =====

#[test]
fn test_merge_shallow_rec2_wins() {
    let result = run("[ [ 'a' 1 ] [ 'b' 2 ] ] REC [ [ 'b' 20 ] [ 'c' 3 ] ] REC MERGE");
    assert_eq!(
        record_keys(&result),
        vec!["a", "b", "c"],
        "shared keys keep rec1's position"
    );
    assert_eq!(run_code_on(&result, "'b' REC@"), ForthicValue::Int(20));
    // Non-records coerce to empty
    let result = run("NULL [ [ 'x' 1 ] ] REC MERGE");
    assert_eq!(record_keys(&result), vec!["x"]);
}

#[test]
fn test_pick_and_omit() {
    let rec = "[ [ 'a' 1 ] [ 'b' 2 ] [ 'c' 3 ] ] REC";
    let picked = run(&format!("{rec} [ 'c' 'a' 'zzz' ] PICK"));
    assert_eq!(
        record_keys(&picked),
        vec!["c", "a"],
        "keys-list order; missing skipped"
    );
    let omitted = run(&format!("{rec} [ 'b' ] OMIT"));
    assert_eq!(record_keys(&omitted), vec!["a", "c"]);
    // OMIT stringifies drop keys ([ 1 ] matches key "1" — ts's === Set
    // misses this; fixed by design)
    let omitted = run("[ [ '1' 'x' ] [ 'b' 2 ] ] REC [ 1 ] OMIT");
    assert_eq!(record_keys(&omitted), vec!["b"]);
}

// ===== HAS-KEY? / DELETE =====

#[test]
fn test_has_key_is_presence_not_nonnull() {
    assert_eq!(
        run("[ [ 'a' NULL ] ] REC 'a' HAS-KEY?"),
        ForthicValue::Bool(true)
    );
    assert_eq!(
        run("[ [ 'a' 1 ] ] REC 'z' HAS-KEY?"),
        ForthicValue::Bool(false)
    );
    assert_eq!(run("NULL 'a' HAS-KEY?"), ForthicValue::Bool(false));
}

#[test]
fn test_delete_is_copy_on_write_flavor() {
    let result = run("[ [ 'z' 1 ] [ 'a' 2 ] [ 'm' 3 ] ] REC 'z' DELETE");
    assert_eq!(
        record_keys(&result),
        vec!["a", "m"],
        "order preserved (shift_remove)"
    );
    // Arrays: negative wraps once; out-of-range is a no-op
    assert_eq!(run("[ 1 2 3 ] -1 DELETE"), ints(&[1, 2]));
    assert_eq!(run("[ 1 2 ] 9 DELETE"), ints(&[1, 2]));
    // Non-integer array key errors (no ts NaN->0 splice surprise)
    let err = run_err("[ 1 2 ] 'x' DELETE");
    assert!(err.to_string().contains("integer index"), "got: {err}");
}

// ===== REC>ENTRIES / ENTRIES>REC =====

#[test]
fn test_entries_round_trip_in_insertion_order() {
    // Documented divergence: ts sorts REC>ENTRIES by key; rs preserves
    // insertion order, making the round trip an identity
    let entries = run("[ [ 'z' 1 ] [ 'a' 2 ] ] REC REC>ENTRIES");
    assert_eq!(
        entries,
        ForthicValue::Array(vec![
            ForthicValue::Array(vec![s("z"), ForthicValue::Int(1)]),
            ForthicValue::Array(vec![s("a"), ForthicValue::Int(2)]),
        ])
    );
    let back = run("[ [ 'z' 1 ] [ 'a' 2 ] ] REC REC>ENTRIES ENTRIES>REC");
    assert_eq!(record_keys(&back), vec!["z", "a"]);
}

#[test]
fn test_entries_to_rec_validates_pairs() {
    let err = run_err("[ [ 'a' 1 2 ] ] ENTRIES>REC");
    assert!(err.to_string().contains("exactly 2 elements"), "got: {err}");
    let err = run_err("[ 5 ] ENTRIES>REC");
    assert!(err.to_string().contains("[key, value] array"), "got: {err}");
    // Duplicate keys: later wins, first position kept
    let rec = run("[ [ 'a' 1 ] [ 'b' 2 ] [ 'a' 9 ] ] ENTRIES>REC");
    assert_eq!(record_keys(&rec), vec!["a", "b"]);
    assert_eq!(run_code_on(&rec, "'a' REC@"), ForthicValue::Int(9));
}
