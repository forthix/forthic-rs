//! Tier 1 correctness regression tests
//!
//! Pins the five fixes from plans/TS-PARITY-BACKLOG.md Tier 1, each ported
//! from (or verified against) the forthic-ts correctness scrub (#26, #29,
//! #31). Runs without features — these are core-interpreter behaviors.

use chrono::TimeZone;
use forthic::errors::{CodeLocation, ForthicError};
use forthic::interpreter::Interpreter;
use forthic::literals::ForthicValue;
use forthic::module::{InterpreterContext, Module, ModuleWord};
use std::sync::Arc;

fn run(code: &str) -> ForthicValue {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap();
    interp.get_stack_mut().pop().unwrap()
}

// ===== 1. Error formatter must never panic (ts #26 crash-proof formatter) =====

#[test]
fn test_formatter_survives_degenerate_location() {
    // end_pos < start_pos is constructible (CodeLocation fields are pub);
    // formatting used to panic via unchecked usize subtraction
    let degenerate = CodeLocation {
        source: None,
        line: 1,
        column: 5,
        start_pos: 10,
        end_pos: Some(3),
    };
    let err = ForthicError::UnknownWord {
        forthic: "SOME CODE".to_string(),
        word: "CODE".to_string(),
        location: Some(degenerate),
        cause: None,
    };
    let formatted = err.format_with_context();
    assert!(
        formatted.contains("^"),
        "still renders a caret: {formatted}"
    );
}

#[test]
fn test_formatter_survives_location_past_end_of_source() {
    let past_end = CodeLocation {
        source: None,
        line: 99,
        column: 50,
        start_pos: 1000,
        end_pos: Some(1001),
    };
    let err = ForthicError::UnknownWord {
        forthic: "short".to_string(),
        word: "X".to_string(),
        location: Some(past_end),
        cause: None,
    };
    let _ = err.format_with_context(); // must not panic
}

// ===== 2. Temporal + record equality (ts #29) =====

#[test]
fn test_today_equals_today() {
    assert_eq!(run("TODAY TODAY =="), ForthicValue::Bool(true));
}

#[test]
fn test_equal_dates_are_equal() {
    assert_eq!(run("2020-06-05 2020-06-05 =="), ForthicValue::Bool(true));
    assert_eq!(run("2020-06-05 2020-06-06 =="), ForthicValue::Bool(false));
    assert_eq!(run("2020-06-05 2020-06-06 !="), ForthicValue::Bool(true));
}

#[test]
fn test_equal_times_are_equal() {
    assert_eq!(run("9:30 9:30 =="), ForthicValue::Bool(true));
    assert_eq!(run("9:30 9:31 =="), ForthicValue::Bool(false));
}

#[test]
fn test_datetime_equality_requires_same_timezone() {
    // ts compares Temporal values by ISO string (includes the tz
    // annotation): the same instant in different timezones is NOT equal
    let la: chrono_tz::Tz = "America/Los_Angeles".parse().unwrap();
    let instant_utc = chrono_tz::UTC
        .with_ymd_and_hms(2020, 6, 5, 17, 15, 0)
        .unwrap();
    let same_instant_la = instant_utc.with_timezone(&la);

    let mut interp = Interpreter::standard("UTC");
    interp.stack_push(ForthicValue::DateTime(instant_utc.clone()));
    interp.stack_push(ForthicValue::DateTime(same_instant_la));
    interp.run("==").unwrap();
    assert_eq!(
        interp.get_stack_mut().pop().unwrap(),
        ForthicValue::Bool(false)
    );

    interp.stack_push(ForthicValue::DateTime(instant_utc.clone()));
    interp.stack_push(ForthicValue::DateTime(instant_utc));
    interp.run("==").unwrap();
    assert_eq!(
        interp.get_stack_mut().pop().unwrap(),
        ForthicValue::Bool(true)
    );
}

#[test]
fn test_temporal_membership_works() {
    // values_equal also feeds IN/ANY/ALL
    assert_eq!(run("TODAY [ TODAY ] IN"), ForthicValue::Bool(true));
}

#[test]
fn test_record_equality_is_structural() {
    // rs has no reference identity, so records compare structurally
    // (arrays already did)
    assert_eq!(
        run("[ [ 'a' 1 ] ] REC [ [ 'a' 1 ] ] REC =="),
        ForthicValue::Bool(true)
    );
    assert_eq!(
        run("[ [ 'a' 1 ] ] REC [ [ 'a' 2 ] ] REC =="),
        ForthicValue::Bool(false)
    );
}

// ===== 3. ANY with empty second array (ts #31) =====

#[test]
fn test_any_with_empty_second_array_is_false() {
    // Nothing can match against an empty set (the old code returned true)
    assert_eq!(run("[ 1 2 ] [ ] ANY"), ForthicValue::Bool(false));
    assert_eq!(run("[ ] [ ] ANY"), ForthicValue::Bool(false));
    assert_eq!(run("[ 1 2 ] [ 2 ] ANY"), ForthicValue::Bool(true));
    // ALL over an empty items2 stays vacuously true (matches ts)
    assert_eq!(run("[ 1 2 ] [ ] ALL"), ForthicValue::Bool(true));
}

// ===== 4. IntentionalStop keeps its identity and message (ts #26) =====

#[test]
fn test_intentional_stop_passes_through_definitions_unwrapped() {
    let stopping_word = Arc::new(ModuleWord::new("STOPPER".to_string(), |_ctx| {
        Err(ForthicError::IntentionalStop {
            message: "debugger break at step 3".to_string(),
        })
    }));
    let mut module = Module::new("test".to_string());
    module.add_exportable_word(stopping_word);

    let mut interp = Interpreter::standard("UTC");
    interp.import_module(module, "");
    interp.run(": WRAPPED STOPPER ;").unwrap();
    let err = interp.run("WRAPPED").unwrap_err();

    match err {
        ForthicError::IntentionalStop { message } => {
            assert_eq!(message, "debugger break at step 3", "message preserved");
        }
        other => panic!("expected IntentionalStop to survive the definition wrapper, got: {other}"),
    }
}

// ===== 5. NOW and TODAY use the interpreter timezone (ts #29) =====

#[test]
fn test_now_uses_interpreter_timezone() {
    let mut interp = Interpreter::standard("America/New_York");
    interp.run("NOW").unwrap();
    match interp.get_stack_mut().pop().unwrap() {
        ForthicValue::DateTime(dt) => {
            assert_eq!(dt.timezone().name(), "America/New_York");
        }
        other => panic!("NOW pushed {other:?}"),
    }
}

#[test]
fn test_now_and_today_agree_on_the_date() {
    // The old code mixed hardcoded UTC (NOW) with host-local (TODAY), so
    // they could disagree on what day it is. In any single timezone they
    // must agree (allowing for a midnight tick between the two calls).
    for tz in ["Pacific/Kiritimati", "Pacific/Midway", "UTC"] {
        let mut interp = Interpreter::standard(tz);
        interp.run("TODAY NOW").unwrap();
        let now = interp.get_stack_mut().pop().unwrap();
        let today = interp.get_stack_mut().pop().unwrap();
        let (ForthicValue::DateTime(now), ForthicValue::Date(today)) = (now, today) else {
            panic!("unexpected types from TODAY NOW");
        };
        let day_delta = (now.date_naive() - today).num_days();
        assert!(
            (0..=1).contains(&day_delta),
            "NOW ({}) and TODAY ({today}) disagree in {tz}",
            now.date_naive()
        );
    }
}

#[test]
fn test_default_context_timezone_is_utc() {
    // The trait default keeps custom InterpreterContext impls compiling
    struct Minimal;
    impl InterpreterContext for Minimal {
        fn stack_push(&mut self, _value: ForthicValue) {}
        fn stack_pop(&mut self) -> Result<ForthicValue, ForthicError> {
            unimplemented!()
        }
        fn stack_peek(&self) -> Option<&ForthicValue> {
            None
        }
        fn cur_module(&self) -> &Module {
            unimplemented!()
        }
        fn cur_module_mut(&mut self) -> &mut Module {
            unimplemented!()
        }
        fn get_app_module(&self) -> &Module {
            unimplemented!()
        }
        fn module_stack_push(&mut self, _module: Module) {}
        fn module_stack_pop(&mut self) -> Result<Module, ForthicError> {
            unimplemented!()
        }
    }
    assert_eq!(Minimal.get_timezone(), "UTC");
}
