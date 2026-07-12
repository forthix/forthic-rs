//! Present-but-verify pass (plans/WORD-INVENTORY.md): words that existed
//! in both runtimes but were never verified against the ts contract.
//! Verified-matching (no code change): TAKE, `>BOOL`, SLICE. Fixed here:
//! `>DATETIME` / AT / `TIMESTAMP>DATETIME` (interpreter timezone, was
//! UTC), OR/AND (strictly two operands), MEAN (full polymorphic
//! dispatch), `@` (read-only; unknown variable is an ERROR, never a
//! get-or-create).

// ForthicError is large; accepted trade-off (see lib.rs / backlog item 11)
#![allow(clippy::result_large_err)]

use chrono::{Datelike, Timelike};
use forthic::errors::ForthicError;
use forthic::interpreter::Interpreter;
use forthic::literals::ForthicValue;
use forthic::module::InterpreterContext;
use indexmap::IndexMap;

fn run_tz(tz: &str, code: &str) -> ForthicValue {
    let mut interp = Interpreter::standard(tz);
    interp.run(code).unwrap();
    interp.get_stack_mut().pop().unwrap()
}

fn run(code: &str) -> ForthicValue {
    run_tz("UTC", code)
}

fn run_err(code: &str) -> ForthicError {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap_err()
}

fn rec(entries: &[(&str, ForthicValue)]) -> ForthicValue {
    let map: IndexMap<String, ForthicValue> = entries
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    ForthicValue::Record(map)
}

fn datetime_parts(value: &ForthicValue) -> (i32, u32, u32, u32, u32, String) {
    match value {
        ForthicValue::DateTime(dt) => (
            dt.year(),
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.timezone().name().to_string(),
        ),
        other => panic!("expected datetime, got {other:?}"),
    }
}

// ===== >DATETIME / TIMESTAMP>DATETIME: interpreter timezone =====

#[test]
fn test_timestamps_resolve_in_interpreter_timezone() {
    // The ts test-pinned instant: 1593895532 is 2020-07-04 13:45 in LA
    // (20:45 UTC — a UTC-hardcoded runtime fails this)
    let result = run_tz("America/Los_Angeles", "1593895532 >DATETIME");
    let (y, m, d, hh, mm, tz) = datetime_parts(&result);
    assert_eq!((y, m, d, hh, mm), (2020, 7, 4, 13, 45));
    assert_eq!(tz, "America/Los_Angeles");

    let result = run_tz("America/Los_Angeles", "1593895532 TIMESTAMP>DATETIME");
    assert_eq!(datetime_parts(&result).3, 13);
}

#[test]
fn test_to_datetime_input_breadth() {
    // Epoch 0 is a value, not a miss (ts #29)
    let (y, ..) = datetime_parts(&run("0 >DATETIME"));
    assert_eq!(y, 1970);
    // Floats are epoch seconds too
    let (y, ..) = datetime_parts(&run("0.5 >DATETIME"));
    assert_eq!(y, 1970);
    // A Date is midnight in the interpreter tz
    let result = run_tz("Asia/Tokyo", "'2024-01-15' >DATE >DATETIME");
    let (y, m, d, hh, _, tz) = datetime_parts(&result);
    assert_eq!((y, m, d, hh), (2024, 1, 15, 0));
    assert_eq!(tz, "Asia/Tokyo");
    // An existing DateTime passes through KEEPING its own timezone
    let result = run_tz("Asia/Tokyo", "0 >DATETIME");
    let via_utc_interp = {
        let mut interp = Interpreter::standard("UTC");
        interp.stack_push(result);
        interp.run(">DATETIME").unwrap();
        interp.get_stack_mut().pop().unwrap()
    };
    assert_eq!(datetime_parts(&via_utc_interp).5, "Asia/Tokyo");
    assert_eq!(run("NULL >DATETIME"), ForthicValue::Null);
    assert_eq!(run("'' >DATETIME"), ForthicValue::Null);
}

#[test]
fn test_to_datetime_strings_are_wall_clocks_in_interpreter_tz() {
    let result = run_tz("America/Los_Angeles", "'2024-01-15T14:30:00' >DATETIME");
    let (y, m, d, hh, mm, tz) = datetime_parts(&result);
    assert_eq!((y, m, d, hh, mm), (2024, 1, 15, 14, 30));
    assert_eq!(tz, "America/Los_Angeles");
    // Short and date-only forms
    assert_eq!(datetime_parts(&run("'2024-01-15T14:30' >DATETIME")).4, 30);
    assert_eq!(datetime_parts(&run("'2024-01-15' >DATETIME")).3, 0);
}

#[test]
fn test_to_datetime_zoned_strings_are_instants() {
    // Sanctioned divergence: ts nulls Z-strings and reinterprets offset
    // wall-clocks; rs reads both as the instants they denote, resolved
    // into the interpreter tz (consistent with >DATE's #35 rule)
    let result = run_tz("Asia/Tokyo", "'2024-01-15T23:30:00Z' >DATETIME");
    let (y, m, d, hh, _, tz) = datetime_parts(&result);
    assert_eq!(
        (y, m, d, hh),
        (2024, 1, 16, 8),
        "23:30Z is 08:30 next day in Tokyo"
    );
    assert_eq!(tz, "Asia/Tokyo");
}

#[test]
fn test_at_builds_wall_clock_in_interpreter_tz() {
    let result = run_tz("America/Los_Angeles", "'2024-01-15' >DATE '14:30' >TIME AT");
    let (y, m, d, hh, mm, tz) = datetime_parts(&result);
    assert_eq!((y, m, d, hh, mm), (2024, 1, 15, 14, 30));
    assert_eq!(tz, "America/Los_Angeles");
    // Round trip through the timestamp words agrees with itself
    let ts = run_tz(
        "America/Los_Angeles",
        "'2024-01-15' >DATE '14:30' >TIME AT >TIMESTAMP",
    );
    let back = {
        let mut interp = Interpreter::standard("America/Los_Angeles");
        interp.stack_push(ts);
        interp.run("TIMESTAMP>DATETIME").unwrap();
        interp.get_stack_mut().pop().unwrap()
    };
    assert_eq!(datetime_parts(&back).3, 14);
    // Falsy operands
    assert_eq!(run("NULL '14:30' >TIME AT"), ForthicValue::Null);
    assert_eq!(run("'2024-01-15' >DATE NULL AT"), ForthicValue::Null);
}

// ===== OR / AND: strictly two operands =====

#[test]
fn test_or_and_reject_arrays_toward_any_all() {
    let err = run_err("FALSE [ TRUE FALSE ] OR");
    assert!(err.to_string().contains("use ANY?"), "got: {err}");
    let err = run_err("[ TRUE ] TRUE AND");
    assert!(err.to_string().contains("use ALL?"), "got: {err}");
    // Two-operand form: truthiness-coerced Bool result
    assert_eq!(run("FALSE 'x' OR"), ForthicValue::Bool(true));
    assert_eq!(run("1 0 AND"), ForthicValue::Bool(false));
}

// ===== MEAN: polymorphic dispatch =====

#[test]
fn test_mean_numbers() {
    assert_eq!(run("[ 2 4 6 ] MEAN"), ForthicValue::Int(4));
    assert_eq!(run("[ 1 2 ] MEAN"), ForthicValue::Float(1.5));
    // NULL elements are SKIPPED, not zero
    assert_eq!(run("[ 2 NULL 4 ] MEAN"), ForthicValue::Int(3));
    assert_eq!(run("[ NULL NULL ] MEAN"), ForthicValue::Int(0));
}

#[test]
fn test_mean_edges() {
    assert_eq!(run("NULL MEAN"), ForthicValue::Int(0), "falsy input is 0");
    assert_eq!(run("[ ] MEAN"), ForthicValue::Int(0));
    // Truthy non-array passes through as-is
    assert_eq!(
        run("'hello' MEAN"),
        ForthicValue::String("hello".to_string())
    );
    // Single-element array: that element AS-IS (before null filtering)
    assert_eq!(run("[ 'a' ] MEAN"), ForthicValue::String("a".to_string()));
    assert_eq!(run("[ NULL ] MEAN"), ForthicValue::Null);
}

#[test]
fn test_mean_strings_give_frequency_distribution() {
    assert_eq!(
        run("[ 'a' 'a' 'b' 'c' ] MEAN"),
        rec(&[
            ("a", ForthicValue::Float(0.5)),
            ("b", ForthicValue::Float(0.25)),
            ("c", ForthicValue::Float(0.25)),
        ])
    );
}

#[test]
fn test_mean_records_give_field_wise_mean() {
    let result = run("[ [ [ 'score' 10 ] [ 'grade' 'A' ] ] REC \
           [ [ 'score' 20 ] [ 'grade' 'A' ] ] REC ] MEAN");
    assert_eq!(
        result,
        rec(&[
            ("score", ForthicValue::Int(15)),
            ("grade", rec(&[("A", ForthicValue::Int(1))])),
        ])
    );
}

// ===== @ : read-only fetch, unknown variable errors =====

#[test]
fn test_fetch_unknown_variable_is_an_error() {
    let err = run_err(".ghost @");
    assert!(
        matches!(err, ForthicError::UnknownVariable { .. }),
        "got: {err}"
    );
    assert!(err.to_string().contains("ghost"), "got: {err}");
}

#[test]
fn test_fetch_does_not_create_as_a_side_effect() {
    // ts pins this explicitly: the failed @ must not mint the variable
    let mut interp = Interpreter::standard("UTC");
    assert!(interp.run(".ghost @").is_err());
    assert!(interp.find_variable_value("ghost").is_none());
}

#[test]
fn test_declared_and_stored_variables_still_read() {
    // Declared-but-unset reads as NULL (no error)
    assert_eq!(run("[ 'x' ] VARIABLES .x @"), ForthicValue::Null);
    // Stored reads back; ! still get-or-creates
    assert_eq!(run("7 .y ! .y @"), ForthicValue::Int(7));
}
