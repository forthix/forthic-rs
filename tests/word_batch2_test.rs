//! Batch 2 words: higher-order, sorting, grouping (plans/WORD-INVENTORY.md)
//! Contracts per the ts implementation spec; sanctioned deviations noted
//! inline (structural equality for KEY-OF/UNIQUE-BY, natural_cmp for key
//! comparison, insertion order everywhere).

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

fn s(v: &str) -> ForthicValue {
    ForthicValue::String(v.to_string())
}

fn record_keys(value: &ForthicValue) -> Vec<String> {
    match value {
        ForthicValue::Record(rec) => rec.keys().cloned().collect(),
        other => panic!("expected record, got {other:?}"),
    }
}

fn rec_get<'a>(value: &'a ForthicValue, key: &str) -> &'a ForthicValue {
    match value {
        ForthicValue::Record(rec) => rec.get(key).unwrap_or(&ForthicValue::Null),
        other => panic!("expected record, got {other:?}"),
    }
}

// ===== FILTER / FOREACH / REDUCE / FIND / COUNT =====

#[test]
fn test_filter_arrays_and_records() {
    assert_eq!(run("[ 1 2 3 4 ] '2 >' FILTER"), ints(&[3, 4]));
    // Record in -> record out, insertion order kept
    let result = run("[ [ 'z' 1 ] [ 'a' 5 ] [ 'm' 2 ] ] REC '2 >' FILTER");
    assert_eq!(record_keys(&result), vec!["a"]);
}

#[test]
fn test_filter_with_key() {
    // Keep elements whose INDEX is > 0 (key pushed beneath value; drop value)
    assert_eq!(
        run("[ 10 20 30 ] 'DROP 0 >' [ .with_key TRUE ] ~> FILTER"),
        ints(&[20, 30])
    );
}

#[test]
fn test_foreach_leaves_results_on_stack() {
    let stack = run_all("[ 1 2 3 ] '2 *' FOREACH");
    assert_eq!(
        stack,
        vec![
            ForthicValue::Int(2),
            ForthicValue::Int(4),
            ForthicValue::Int(6)
        ]
    );
}

#[test]
fn test_reduce() {
    assert_eq!(run("[ 1 2 3 4 ] 0 '+' REDUCE"), ForthicValue::Int(10));
    // Record reduces over values
    assert_eq!(
        run("[ [ 'a' 2 ] [ 'b' 3 ] ] REC 1 '*' REDUCE"),
        ForthicValue::Int(6)
    );
    // Null container -> initial
    assert_eq!(run("NULL 42 '+' REDUCE"), ForthicValue::Int(42));
}

#[test]
fn test_find_short_circuits() {
    assert_eq!(run("[ 1 5 2 ] '3 >' FIND"), ForthicValue::Int(5));
    assert_eq!(run("[ 1 2 ] '10 >' FIND"), ForthicValue::Null);
    // Short-circuit proof: the poison element after the match never runs
    // ('NO-SUCH-WORD' would error) — DUP the item first so the predicate
    // consumes a copy
    assert_eq!(
        run("[ 1 'NO-SUCH-WORD' ] 'STRING? NOT' FIND"),
        ForthicValue::Int(1)
    );
}

#[test]
fn test_count() {
    assert_eq!(run("[ 1 5 2 6 ] '3 >' COUNT"), ForthicValue::Int(2));
    assert_eq!(run("NULL '3 >' COUNT"), ForthicValue::Int(0));
}

// ===== SORT family =====

#[test]
fn test_sort_natural() {
    assert_eq!(run("[ 3 1 2 ] SORT"), ints(&[1, 2, 3]));
    // NULL sorts last
    assert_eq!(
        run("[ 3 NULL 1 ] SORT"),
        ForthicValue::Array(vec![
            ForthicValue::Int(1),
            ForthicValue::Int(3),
            ForthicValue::Null
        ])
    );
    // Strings lexicographic
    assert_eq!(
        run("[ 'b' 'a' ] SORT"),
        ForthicValue::Array(vec![s("a"), s("b")])
    );
    // Int and Float share the number line
    assert_eq!(
        run("[ 2.5 1 3 ] SORT"),
        ForthicValue::Array(vec![
            ForthicValue::Int(1),
            ForthicValue::Float(2.5),
            ForthicValue::Int(3)
        ])
    );
}

#[test]
fn test_sort_comparator_is_a_key_function() {
    // The comparator option is a KEY function (the ts docstring's "SWAP -"
    // two-arg example is stale): '-1 *' sorts descending by negated key
    assert_eq!(
        run("[ 1 3 2 ] [ .comparator '-1 *' ] ~> SORT"),
        ints(&[3, 2, 1])
    );
}

#[test]
fn test_sort_non_array_passes_through() {
    let result = run("[ [ 'z' 1 ] ] REC SORT");
    assert_eq!(record_keys(&result), vec!["z"]);
    assert_eq!(run("NULL SORT"), ForthicValue::Null);
}

#[test]
fn test_sort_by_stable_ties() {
    // Equal keys keep input order (decorate-stable-sort-undecorate)
    assert_eq!(
        run("[ 21 11 22 12 ] '10 MOD' SORT-BY"),
        ints(&[21, 11, 22, 12])
    );
    assert_eq!(run("[ 3 1 2 ] 'DUP *' SORT-BY"), ints(&[1, 2, 3]));
}

#[test]
fn test_min_by_max_by() {
    assert_eq!(run("[ 3 1 2 ] 'DUP *' MIN-BY"), ForthicValue::Int(1));
    assert_eq!(run("[ 3 1 2 ] 'DUP *' MAX-BY"), ForthicValue::Int(3));
    // Empty and non-array -> NULL
    assert_eq!(run("[ ] 'DUP' MIN-BY"), ForthicValue::Null);
    assert_eq!(run("NULL 'DUP' MAX-BY"), ForthicValue::Null);
    // Ties keep the EARLIEST element: both have key 1
    assert_eq!(run("[ -1 1 ] 'DUP *' MIN-BY"), ForthicValue::Int(-1));
}

#[test]
fn test_unique_by_keeps_first() {
    assert_eq!(run("[ 21 11 31 12 ] '10 MOD' UNIQUE-BY"), ints(&[21, 12]));
}

#[test]
fn test_sort_u() {
    assert_eq!(run("[ 3 1 3 2 1 ] SORT-U"), ints(&[1, 2, 3]));
    assert_eq!(
        run("[ 'b' 'a' 'b' ] SORT-U"),
        ForthicValue::Array(vec![s("a"), s("b")])
    );
}

// ===== Grouping =====

#[test]
fn test_group_by() {
    let result = run("[ 1 2 3 4 5 ] '2 MOD' GROUP-BY");
    assert_eq!(
        record_keys(&result),
        vec!["1", "0"],
        "first-encounter order"
    );
    assert_eq!(rec_get(&result, "1"), &ints(&[1, 3, 5]));
    assert_eq!(rec_get(&result, "0"), &ints(&[2, 4]));
}

#[test]
fn test_group_by_field_with_multi_membership() {
    let code = "[ [ [ 'name' 'a' ] [ 'tags' [ 'x' 'y' ] ] ] REC \
                  [ [ 'name' 'b' ] [ 'tags' [ 'x' ] ] ] REC ] 'tags' GROUP-BY-FIELD";
    let result = run(code);
    assert_eq!(record_keys(&result), vec!["x", "y"]);
    match rec_get(&result, "x") {
        ForthicValue::Array(items) => assert_eq!(items.len(), 2, "a and b both tagged x"),
        other => panic!("expected array, got {other:?}"),
    }
}

#[test]
fn test_by_field_last_wins_and_skips_falsy() {
    let code =
        "[ [ [ 'id' 'k' ] [ 'v' 1 ] ] REC NULL [ [ 'id' 'k' ] [ 'v' 2 ] ] REC ] 'id' BY-FIELD";
    let result = run(code);
    assert_eq!(record_keys(&result), vec!["k"]);
    assert_eq!(rec_get(rec_get(&result, "k"), "v"), &ForthicValue::Int(2));
}

#[test]
fn test_groups_of() {
    assert_eq!(
        run("[ 1 2 3 4 5 ] 2 GROUPS-OF"),
        ForthicValue::Array(vec![ints(&[1, 2]), ints(&[3, 4]), ints(&[5])])
    );
    // Records chunk into sub-records
    let result = run("[ [ 'a' 1 ] [ 'b' 2 ] [ 'c' 3 ] ] REC 2 GROUPS-OF");
    match result {
        ForthicValue::Array(groups) => {
            assert_eq!(record_keys(&groups[0]), vec!["a", "b"]);
            assert_eq!(record_keys(&groups[1]), vec!["c"]);
        }
        other => panic!("expected array of records, got {other:?}"),
    }
}

#[test]
fn test_groups_of_rejects_nonpositive() {
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("[ 1 ] 0 GROUPS-OF").unwrap_err();
    assert!(err.to_string().contains("group size"), "got: {err}");
}

#[test]
fn test_index_lowercases_and_multi_buckets() {
    let result = run("[ 'Apple' 'Avocado' ] \"DROP [ 'A' 'FRUIT' ]\" INDEX");
    assert_eq!(record_keys(&result), vec!["a", "fruit"]);
    match rec_get(&result, "fruit") {
        ForthicValue::Array(items) => assert_eq!(items.len(), 2),
        other => panic!("expected array, got {other:?}"),
    }
}

#[test]
fn test_key_of_structural() {
    assert_eq!(run("[ 'a' 'b' ] 'b' KEY-OF"), ForthicValue::Int(1));
    assert_eq!(run("[ [ 'k' 'b' ] ] REC 'b' KEY-OF"), s("k"));
    assert_eq!(run("[ 'a' ] 'z' KEY-OF"), ForthicValue::Null);
    // Structural equality: a distinct-but-equal record matches (sanctioned
    // deviation from ts ===, which could never match this)
    assert_eq!(
        run("[ [ [ 'a' 1 ] ] REC ] [ [ 'a' 1 ] ] REC KEY-OF"),
        ForthicValue::Int(0)
    );
}

#[test]
fn test_numbered() {
    assert_eq!(
        run("[ 'a' 'b' ] NUMBERED"),
        ForthicValue::Array(vec![
            ForthicValue::Array(vec![ForthicValue::Int(0), s("a")]),
            ForthicValue::Array(vec![ForthicValue::Int(1), s("b")]),
        ])
    );
    // Non-arrays (including records) yield an EMPTY array
    assert_eq!(run("NULL NUMBERED"), ForthicValue::Array(vec![]));
}

// ===== ZIP-WITH / TIMES-RUN / MAP-AT =====

#[test]
fn test_zip_with_arrays_pads_null() {
    assert_eq!(run("[ 1 2 ] [ 10 20 ] '+' ZIP-WITH"), ints(&[11, 22]));
    // c1 longer: missing c2 entries are NULL; use DEFAULT to absorb
    assert_eq!(
        run("[ 1 2 3 ] [ 10 ] '0 DEFAULT +' ZIP-WITH"),
        ints(&[11, 2, 3])
    );
}

#[test]
fn test_zip_with_records() {
    let result = run("[ [ 'a' 1 ] ] REC [ [ 'a' 10 ] ] REC '+' ZIP-WITH");
    assert_eq!(rec_get(&result, "a"), &ForthicValue::Int(11));
}

#[test]
fn test_times_run() {
    assert_eq!(run("1 3 '2 *' TIMES-RUN"), ForthicValue::Int(8));
    // Zero/negative and empty code are no-ops
    assert_eq!(run("7 0 '2 *' TIMES-RUN"), ForthicValue::Int(7));
    assert_eq!(run("7 3 '' TIMES-RUN"), ForthicValue::Int(7));
}

#[test]
fn test_map_at_single_key_and_path() {
    let result = run("[ [ 'a' 1 ] [ 'b' 2 ] ] REC 'a' '10 *' MAP-AT");
    assert_eq!(rec_get(&result, "a"), &ForthicValue::Int(10));
    assert_eq!(
        rec_get(&result, "b"),
        &ForthicValue::Int(2),
        "sibling untouched"
    );

    // Deep path through record + array
    let result = run("[ [ 'xs' [ 1 2 3 ] ] ] REC [ 'xs' 1 ] '10 *' MAP-AT");
    assert_eq!(rec_get(&result, "xs"), &ints(&[1, 20, 3]));
}

#[test]
fn test_map_at_misses_are_silent() {
    // Missing key, out-of-range index, scalar mid-path: unchanged, no error
    let result = run("[ [ 'a' 1 ] ] REC 'zzz' '10 *' MAP-AT");
    assert_eq!(rec_get(&result, "a"), &ForthicValue::Int(1));
    assert_eq!(run("[ 1 2 ] 9 '10 *' MAP-AT"), ints(&[1, 2]));
    assert_eq!(run("NULL 'a' '10 *' MAP-AT"), ForthicValue::Null);
}

#[test]
fn test_map_at_empty_path_transforms_whole_container() {
    assert_eq!(run("[ 1 2 ] [ ] 'LENGTH' MAP-AT"), ForthicValue::Int(2));
}

#[test]
fn test_map_at_numeric_string_index() {
    // ts Number(head) coercion: '1' works as an array index
    assert_eq!(run("[ 1 2 3 ] '1' '10 *' MAP-AT"), ints(&[1, 20, 3]));
}
