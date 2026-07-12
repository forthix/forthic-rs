//! Batch 5 words: math & datetime round-out (plans/WORD-INVENTORY.md)
//!
//! Documented divergences from ts: strict parsing in >DATE (ts's arbitrary
//! new Date() leniency beyond month-name forms is not reproduced);
//! non-numeric PRODUCT elements are NULL (no JS string coercion);
//! FORMAT-FIXED formats >=1e21 in plain decimal (no exponential quirk).

// ForthicError is large; accepted trade-off (see lib.rs / backlog item 11)
#![allow(clippy::result_large_err)]

use chrono::{NaiveDate, NaiveTime};
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

fn run_tz(tz: &str, code: &str) -> ForthicValue {
    let mut interp = Interpreter::standard(tz);
    interp.run(code).unwrap();
    interp.get_stack_mut().pop().unwrap()
}

fn s(v: &str) -> ForthicValue {
    ForthicValue::String(v.to_string())
}

fn date(y: i32, m: u32, d: u32) -> ForthicValue {
    ForthicValue::Date(NaiveDate::from_ymd_opt(y, m, d).unwrap())
}

fn time(h: u32, m: u32) -> ForthicValue {
    ForthicValue::Time(NaiveTime::from_hms_opt(h, m, 0).unwrap())
}

// ===== PRODUCT =====

#[test]
fn test_product() {
    assert_eq!(run("[ 2 3 4 ] PRODUCT"), ForthicValue::Int(24));
    assert_eq!(
        run("[ ] PRODUCT"),
        ForthicValue::Int(1),
        "empty product is 1"
    );
    assert_eq!(run("[ 2 0.5 ] PRODUCT"), ForthicValue::Int(1));
    // Deliberate ts asymmetry with SUM: null elements NULL the whole
    // result (SUM skips), and non-array input is NULL (SUM says 0)
    assert_eq!(run("[ 2 NULL 3 ] PRODUCT"), ForthicValue::Null);
    assert_eq!(run("5 PRODUCT"), ForthicValue::Null);
    assert_eq!(run("NULL PRODUCT"), ForthicValue::Null);
    // Non-numeric elements: NULL (rs strictness; ts would JS-coerce)
    assert_eq!(run("[ 2 'x' ] PRODUCT"), ForthicValue::Null);
}

#[test]
fn test_product_does_not_saturate_to_int_max() {
    // Whole-but-huge results stay Float — `as i64` saturation guard
    let result = run("[ 1000000000000000000 100 ] PRODUCT");
    assert_eq!(result, ForthicValue::Float(1e20));
}

// ===== SQRT =====

#[test]
fn test_sqrt() {
    assert_eq!(run("16 SQRT"), ForthicValue::Int(4));
    assert_eq!(run("NULL SQRT"), ForthicValue::Null);
    match run("2 SQRT") {
        ForthicValue::Float(f) => assert!((f - std::f64::consts::SQRT_2).abs() < 1e-9),
        other => panic!("expected float, got {other:?}"),
    }
    // Negative input is NaN (JS Math.sqrt), not an error
    match run("-1 SQRT") {
        ForthicValue::Float(f) => assert!(f.is_nan()),
        other => panic!("expected NaN, got {other:?}"),
    }
}

// ===== CLAMP =====

#[test]
fn test_clamp() {
    assert_eq!(run("5 0 10 CLAMP"), ForthicValue::Int(5));
    assert_eq!(run("-5 0 10 CLAMP"), ForthicValue::Int(0));
    assert_eq!(run("15 0 10 CLAMP"), ForthicValue::Int(10));
    // Boundaries inclusive
    assert_eq!(run("0 0 10 CLAMP"), ForthicValue::Int(0));
    assert_eq!(run("10 0 10 CLAMP"), ForthicValue::Int(10));
    // ts formula is max(min, min(max, value)): when min > max, MIN WINS
    assert_eq!(run("5 10 0 CLAMP"), ForthicValue::Int(10));
    // Any NULL operand nulls the result
    assert_eq!(run("NULL 0 10 CLAMP"), ForthicValue::Null);
    assert_eq!(run("5 NULL 10 CLAMP"), ForthicValue::Null);
}

#[test]
fn test_clamp_propagates_nan_like_js() {
    // JS Math.min/max propagate NaN; rust f64::min/max swallow it — pinned
    let mut interp = Interpreter::standard("UTC");
    interp.stack_push(ForthicValue::Float(f64::NAN));
    interp.run("0 10 CLAMP").unwrap();
    match interp.get_stack_mut().pop().unwrap() {
        ForthicValue::Float(f) => assert!(f.is_nan()),
        other => panic!("expected NaN, got {other:?}"),
    }
}

// ===== FORMAT-FIXED =====

#[test]
fn test_format_fixed() {
    assert_eq!(run("3.14159 2 FORMAT-FIXED"), s("3.14"));
    assert_eq!(run("5 2 FORMAT-FIXED"), s("5.00"), "pads with zeros");
    assert_eq!(run("3.7 0 FORMAT-FIXED"), s("4"), "digits 0 has no point");
    assert_eq!(run("NULL 2 FORMAT-FIXED"), ForthicValue::Null);
    // NULL digits means 0 (JS ToInteger)
    assert_eq!(run("3.7 NULL FORMAT-FIXED"), s("4"));
}

#[test]
fn test_format_fixed_rounds_half_away_from_zero() {
    // JS toFixed semantics on exactly-representable ties; Rust's format!
    // would give "0" and "2" (ties-to-even)
    assert_eq!(run("0.5 0 FORMAT-FIXED"), s("1"));
    assert_eq!(run("2.5 0 FORMAT-FIXED"), s("3"));
    assert_eq!(run("-0.5 0 FORMAT-FIXED"), s("-1"));
    // 1.005 is 1.00499... in binary — "1.00" in BOTH runtimes
    assert_eq!(run("1.005 2 FORMAT-FIXED"), s("1.00"));
}

#[test]
fn test_format_fixed_errors() {
    // ts throws RangeError outside 0..=100 and TypeError on non-numbers —
    // the one math word where wrong inputs THROW
    let err = run_err("3.14 -1 FORMAT-FIXED");
    assert!(err.to_string().contains("between 0 and 100"), "got: {err}");
    let err = run_err("3.14 101 FORMAT-FIXED");
    assert!(err.to_string().contains("between 0 and 100"), "got: {err}");
    let err = run_err("'x' 2 FORMAT-FIXED");
    assert!(err.to_string().contains("requires a number"), "got: {err}");
}

// ===== AM / PM =====

#[test]
fn test_am_forces_morning() {
    assert_eq!(run("'14:30' >TIME AM"), time(2, 30));
    assert_eq!(run("'12:00' >TIME AM"), time(0, 0), "noon -> midnight");
    assert_eq!(run("'09:15' >TIME AM"), time(9, 15), "already morning");
}

#[test]
fn test_pm_forces_afternoon() {
    assert_eq!(run("'09:15' >TIME PM"), time(21, 15));
    assert_eq!(run("'00:00' >TIME PM"), time(12, 0), "midnight -> noon");
    assert_eq!(run("'14:30' >TIME PM"), time(14, 30), "already afternoon");
}

#[test]
fn test_am_pm_pass_non_times_through_unchanged() {
    // ts returns the input itself, NOT null
    assert_eq!(run("NULL AM"), ForthicValue::Null);
    assert_eq!(run("'not a time' PM"), s("not a time"));
    assert_eq!(run("42 AM"), ForthicValue::Int(42));
}

// ===== DAYS-BETWEEN (replaces classic SUBTRACT-DATES) =====

#[test]
fn test_days_between_is_date1_minus_date2() {
    assert_eq!(
        run("'2026-01-10' >DATE '2026-01-01' >DATE DAYS-BETWEEN"),
        ForthicValue::Int(9)
    );
    assert_eq!(
        run("'2024-01-15' >DATE '2024-01-25' >DATE DAYS-BETWEEN"),
        ForthicValue::Int(-10),
        "sign convention identical to the dropped SUBTRACT-DATES"
    );
    assert_eq!(
        run("'2024-01-15' >DATE '2024-01-15' >DATE DAYS-BETWEEN"),
        ForthicValue::Int(0)
    );
    assert_eq!(
        run("NULL '2024-01-15' >DATE DAYS-BETWEEN"),
        ForthicValue::Null
    );
}

#[test]
fn test_classic_subtract_dates_is_gone() {
    // Tombstone: the last scheduled classic drop — DAYS-BETWEEN is a pure
    // rename (same operand order, same sign)
    let mut interp = Interpreter::standard("UTC");
    assert!(interp
        .run("'2024-01-15' >DATE '2024-01-25' >DATE SUBTRACT-DATES")
        .is_err());
}

// ===== YEAR / MONTH / DAY-OF-WEEK =====

#[test]
fn test_date_components() {
    assert_eq!(run("'2024-01-15' >DATE YEAR"), ForthicValue::Int(2024));
    assert_eq!(
        run("'2024-01-15' >DATE MONTH"),
        ForthicValue::Int(1),
        "1-based (1=January)"
    );
    assert_eq!(
        run("'2024-01-15' >DATE DAY-OF-WEEK"),
        ForthicValue::Int(1),
        "2024-01-15 is a Monday; ISO 1=Mon"
    );
    assert_eq!(
        run("'2024-01-21' >DATE DAY-OF-WEEK"),
        ForthicValue::Int(7),
        "Sunday is 7, never 0"
    );
}

#[test]
fn test_date_components_need_a_date() {
    // Strings do NOT coerce (ts duck-types on .year, which strings lack)
    assert_eq!(run("'2024-01-15' YEAR"), ForthicValue::Null);
    assert_eq!(run("NULL MONTH"), ForthicValue::Null);
    assert_eq!(run("42 DAY-OF-WEEK"), ForthicValue::Null);
}

#[test]
fn test_date_components_work_on_datetimes() {
    assert_eq!(run("0 >DATETIME YEAR"), ForthicValue::Int(1970));
    assert_eq!(run("0 >DATETIME MONTH"), ForthicValue::Int(1));
    assert_eq!(
        run("0 >DATETIME DAY-OF-WEEK"),
        ForthicValue::Int(4),
        "epoch was a Thursday"
    );
}

// ===== >DATE after ts #35 =====

#[test]
fn test_to_date_takes_written_date_for_no_zone_and_offset_strings() {
    assert_eq!(run("'2024-01-15' >DATE"), date(2024, 1, 15));
    assert_eq!(run("'2024-01-15T23:30:00' >DATE"), date(2024, 1, 15));
    // Explicit numeric offset: date AS WRITTEN, offset ignored (ts
    // PlainDate.from)
    assert_eq!(run("'2024-01-15T23:30:00+09:00' >DATE"), date(2024, 1, 15));
    // Whitespace trims
    assert_eq!(run("'  2024-01-15  ' >DATE"), date(2024, 1, 15));
}

#[test]
fn test_to_date_resolves_z_instants_in_interpreter_timezone() {
    // The #35 rule: a trailing-Z instant is a moment in time; its calendar
    // date depends on the INTERPRETER timezone (never the host's)
    assert_eq!(
        run_tz("Asia/Tokyo", "'2024-01-15T23:30:00Z' >DATE"),
        date(2024, 1, 16)
    );
    assert_eq!(
        run_tz("America/Los_Angeles", "'2024-01-15T23:30:00Z' >DATE"),
        date(2024, 1, 15)
    );
}

#[test]
fn test_to_date_month_name_forms_and_strictness() {
    assert_eq!(run("'Oct 21, 2020' >DATE"), date(2020, 10, 21));
    assert_eq!(run("'October 21, 2020' >DATE"), date(2020, 10, 21));
    // Beyond that, no new Date() leniency — sanctioned strict divergence
    assert_eq!(run("'20240115' >DATE"), ForthicValue::Null);
    assert_eq!(run("'garbage' >DATE"), ForthicValue::Null);
    assert_eq!(
        run("0 >DATE"),
        ForthicValue::Null,
        "ts falsy asymmetry kept"
    );
}

// ===== USE-MODULES =====

fn test_module() -> forthic::module::Module {
    use forthic::module::{Module, ModuleWord};
    use std::sync::Arc;
    let mut module = Module::new("greet".to_string());
    let word = Arc::new(ModuleWord::new("GREETING".to_string(), |context| {
        context.stack_push(ForthicValue::String("hello".to_string()));
        Ok(())
    }));
    module.add_exportable_word(word);
    module
}

#[test]
fn test_use_modules_unprefixed() {
    let mut interp = Interpreter::standard("UTC");
    interp.register_module(test_module());
    interp.run("[ 'greet' ] USE-MODULES GREETING").unwrap();
    assert_eq!(interp.get_stack_mut().pop().unwrap(), s("hello"));
}

#[test]
fn test_use_modules_prefixed_option() {
    let mut interp = Interpreter::standard("UTC");
    interp.register_module(test_module());
    interp
        .run("[ 'greet' ] [ .prefixed TRUE ] ~> USE-MODULES greet.GREETING")
        .unwrap();
    assert_eq!(interp.get_stack_mut().pop().unwrap(), s("hello"));
    // ...and the bare name was NOT imported
    assert!(interp.run("GREETING").is_err());
}

#[test]
fn test_use_modules_pair_prefix_beats_option() {
    // ts contract: an explicit [name prefix] pair ignores .prefixed
    let mut interp = Interpreter::standard("UTC");
    interp.register_module(test_module());
    interp
        .run("[ [ 'greet' 'g' ] ] [ .prefixed TRUE ] ~> USE-MODULES g.GREETING")
        .unwrap();
    assert_eq!(interp.get_stack_mut().pop().unwrap(), s("hello"));
    assert!(interp.run("greet.GREETING").is_err());
}

#[test]
fn test_use_modules_errors() {
    let err = run_err("[ 'no-such-module' ] USE-MODULES");
    assert!(err.to_string().contains("no-such-module"), "got: {err}");
    let err = run_err("'greet' USE-MODULES");
    assert!(err.to_string().contains("requires an array"), "got: {err}");
    // NULL names is a silent no-op
    let mut interp = Interpreter::standard("UTC");
    interp.run("NULL USE-MODULES").unwrap();
    assert!(interp.get_stack().items().is_empty());
}
