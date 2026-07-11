//! StackValue serializer tests
//!
//! The GOLDEN_FIXTURES block is verbatim output of forthic-ts's
//! `serializeValue` (dist/cjs/grpc/serializer.js, temporal-polyfill 0.3),
//! so these tests assert wire-format parity against the real ts runtime,
//! not against a hand-written reading of it.

use chrono::{NaiveDate, NaiveTime, TimeZone};
use chrono_tz::Tz;
use forthic::jsonrpc::{deserialize_value, serialize_value, SerializerError};
use forthic::literals::ForthicValue;
use forthic::word_options::WordOptions;
use serde_json::{json, Value};
use std::collections::HashMap;

const GOLDEN_FIXTURES: &str = r#"{
  "null_v": { "null_value": {} },
  "bool_v": { "bool_value": true },
  "int_v": { "int_value": 42 },
  "float_v": { "float_value": 3.25 },
  "string_v": { "string_value": "howdy" },
  "array_v": {
    "array_value": {
      "items": [
        { "int_value": 1 },
        { "string_value": "two" },
        { "array_value": { "items": [ { "bool_value": true }, { "null_value": {} } ] } }
      ]
    }
  },
  "record_v": {
    "record_value": {
      "fields": {
        "alpha": { "int_value": 1 },
        "needs quoting!": {
          "record_value": { "fields": { "nested": { "float_value": 2.5 } } }
        }
      }
    }
  },
  "plain_date": { "plain_date_value": { "iso8601_date": "2020-06-05" } },
  "plain_time": { "plain_time_value": { "iso8601_time": "09:30:00" } },
  "plain_time_ms": { "plain_time_value": { "iso8601_time": "23:59:59.123" } },
  "instant": { "instant_value": { "iso8601": "2020-06-05T17:15:00Z" } },
  "zoned": {
    "zoned_datetime_value": {
      "iso8601": "2020-06-05T10:15:00-07:00[America/Los_Angeles]",
      "timezone": "America/Los_Angeles"
    }
  },
  "zoned_utc": {
    "zoned_datetime_value": {
      "iso8601": "2021-01-02T03:04:05+00:00[UTC]",
      "timezone": "UTC"
    }
  },
  "zoned_ms": {
    "zoned_datetime_value": {
      "iso8601": "2020-06-05T10:15:00.123-07:00[America/Los_Angeles]",
      "timezone": "America/Los_Angeles"
    }
  },
  "int_like_float": { "int_value": 5 }
}"#;

fn fixture(name: &str) -> Value {
    let all: Value = serde_json::from_str(GOLDEN_FIXTURES).expect("fixtures parse");
    all.get(name)
        .unwrap_or_else(|| panic!("no fixture '{name}'"))
        .clone()
}

fn la_datetime(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> ForthicValue {
    let tz: Tz = "America/Los_Angeles".parse().unwrap();
    ForthicValue::DateTime(tz.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap())
}

/// Assert serialize(value) == fixture AND deserialize(fixture) == value
fn assert_bidirectional(fixture_name: &str, value: ForthicValue) {
    let wire = fixture(fixture_name);
    assert_eq!(
        serialize_value(&value).unwrap(),
        wire,
        "serialize mismatch for '{fixture_name}'"
    );
    assert_eq!(
        deserialize_value(&wire).unwrap(),
        value,
        "deserialize mismatch for '{fixture_name}'"
    );
}

// ===== Cross-runtime parity against ts golden fixtures =====

#[test]
fn test_scalars_match_ts_wire_format() {
    assert_bidirectional("null_v", ForthicValue::Null);
    assert_bidirectional("bool_v", ForthicValue::Bool(true));
    assert_bidirectional("int_v", ForthicValue::Int(42));
    assert_bidirectional("float_v", ForthicValue::Float(3.25));
    assert_bidirectional("string_v", ForthicValue::String("howdy".to_string()));
}

#[test]
fn test_nested_array_matches_ts_wire_format() {
    assert_bidirectional(
        "array_v",
        ForthicValue::Array(vec![
            ForthicValue::Int(1),
            ForthicValue::String("two".to_string()),
            ForthicValue::Array(vec![ForthicValue::Bool(true), ForthicValue::Null]),
        ]),
    );
}

#[test]
fn test_nested_record_matches_ts_wire_format() {
    let mut inner = HashMap::new();
    inner.insert("nested".to_string(), ForthicValue::Float(2.5));
    let mut outer = HashMap::new();
    outer.insert("alpha".to_string(), ForthicValue::Int(1));
    outer.insert("needs quoting!".to_string(), ForthicValue::Record(inner));
    assert_bidirectional("record_v", ForthicValue::Record(outer));
}

#[test]
fn test_dates_match_ts_wire_format() {
    assert_bidirectional(
        "plain_date",
        ForthicValue::Date(NaiveDate::from_ymd_opt(2020, 6, 5).unwrap()),
    );
    assert_bidirectional(
        "plain_time",
        ForthicValue::Time(NaiveTime::from_hms_opt(9, 30, 0).unwrap()),
    );
    assert_bidirectional(
        "plain_time_ms",
        ForthicValue::Time(NaiveTime::from_hms_milli_opt(23, 59, 59, 123).unwrap()),
    );
    assert_bidirectional("zoned", la_datetime(2020, 6, 5, 10, 15, 0));
    assert_bidirectional(
        "zoned_utc",
        ForthicValue::DateTime(Tz::UTC.with_ymd_and_hms(2021, 1, 2, 3, 4, 5).unwrap()),
    );
}

#[test]
fn test_zoned_datetime_with_milliseconds() {
    let tz: Tz = "America/Los_Angeles".parse().unwrap();
    let dt = tz
        .with_ymd_and_hms(2020, 6, 5, 10, 15, 0)
        .unwrap()
        .checked_add_signed(chrono::Duration::milliseconds(123))
        .unwrap();
    assert_bidirectional("zoned_ms", ForthicValue::DateTime(dt));
}

#[test]
fn test_instant_deserializes_to_utc_datetime() {
    // One-directional: rs has no separate Instant type, so instants land as
    // UTC DateTimes (and would re-serialize as zoned_datetime_value).
    let got = deserialize_value(&fixture("instant")).unwrap();
    let expected = ForthicValue::DateTime(Tz::UTC.with_ymd_and_hms(2020, 6, 5, 17, 15, 0).unwrap());
    assert_eq!(got, expected);
}

#[test]
fn test_ts_collapses_integral_floats_to_int() {
    // ts serializes 5.0 as int_value (JS has one number type); rs must accept it
    assert_eq!(
        deserialize_value(&fixture("int_like_float")).unwrap(),
        ForthicValue::Int(5)
    );
}

// ===== Round-trip beyond the fixtures =====

#[test]
fn test_round_trip_deeply_nested() {
    let mut rec = HashMap::new();
    rec.insert(
        "items".to_string(),
        ForthicValue::Array(vec![
            ForthicValue::Record({
                let mut m = HashMap::new();
                m.insert(
                    "date".to_string(),
                    ForthicValue::Date(NaiveDate::from_ymd_opt(1999, 12, 31).unwrap()),
                );
                m.insert("when".to_string(), la_datetime(2024, 2, 29, 23, 59, 59));
                m
            }),
            ForthicValue::Float(-0.5),
            ForthicValue::Int(i64::MAX),
            ForthicValue::String(String::new()),
        ]),
    );
    let value = ForthicValue::Record(rec);
    let wire = serialize_value(&value).unwrap();
    assert_eq!(deserialize_value(&wire).unwrap(), value);
}

#[test]
fn test_round_trip_empty_containers() {
    for value in [
        ForthicValue::Array(vec![]),
        ForthicValue::Record(HashMap::new()),
    ] {
        let wire = serialize_value(&value).unwrap();
        assert_eq!(deserialize_value(&wire).unwrap(), value);
    }
}

// ===== Deserialize robustness =====

#[test]
fn test_zoned_datetime_without_bracket_annotation() {
    // The timezone field alone is enough; ts always sends brackets but
    // other callers may not
    let wire = json!({
        "zoned_datetime_value": {
            "iso8601": "2020-06-05T10:15:00-07:00",
            "timezone": "America/Los_Angeles"
        }
    });
    assert_eq!(
        deserialize_value(&wire).unwrap(),
        la_datetime(2020, 6, 5, 10, 15, 0)
    );
}

#[test]
fn test_zoned_datetime_bracket_only() {
    let wire = json!({
        "zoned_datetime_value": {
            "iso8601": "2020-06-05T10:15:00-07:00[America/Los_Angeles]"
        }
    });
    assert_eq!(
        deserialize_value(&wire).unwrap(),
        la_datetime(2020, 6, 5, 10, 15, 0)
    );
}

#[test]
fn test_unknown_timezone_is_an_error() {
    let wire = json!({
        "zoned_datetime_value": {
            "iso8601": "2020-06-05T10:15:00-07:00[Mars/Olympus_Mons]",
            "timezone": "Mars/Olympus_Mons"
        }
    });
    let err = deserialize_value(&wire).unwrap_err();
    assert!(err.to_string().contains("Unknown timezone"), "got: {err}");
}

#[test]
fn test_unknown_stack_value_tag() {
    let err = deserialize_value(&json!({ "bogus_value": 1 })).unwrap_err();
    assert!(matches!(err, SerializerError::UnknownStackValue { .. }));
    assert_eq!(err.to_string(), "Unknown stack value type");
}

#[test]
fn test_unknown_tag_reports_nested_path() {
    let wire = json!({
        "array_value": { "items": [ { "int_value": 1 }, { "bogus_value": 2 } ] }
    });
    let err = deserialize_value(&wire).unwrap_err();
    assert_eq!(err.to_string(), "Unknown stack value type at path: [1]");
}

#[test]
fn test_malformed_payloads_are_errors() {
    for wire in [
        json!({ "int_value": "not a number" }),
        json!({ "int_value": 1.5 }),
        json!({ "string_value": 7 }),
        json!({ "bool_value": "yes" }),
        json!({ "float_value": "3.14" }),
        json!({ "array_value": {} }),
        json!({ "record_value": {} }),
        json!({ "plain_date_value": { "iso8601_date": "June 5th" } }),
        json!({ "instant_value": {} }),
        json!(42),
        json!(null),
    ] {
        assert!(
            deserialize_value(&wire).is_err(),
            "expected error for {wire}"
        );
    }
}

// ===== Serialize failures =====

#[test]
fn test_unsupported_types_fail_to_serialize() {
    let cases: Vec<(ForthicValue, &str)> = vec![
        (ForthicValue::WordOptions(WordOptions::new()), "WordOptions"),
        (ForthicValue::StartArrayMarker, "StartArrayMarker"),
    ];
    for (value, type_name) in cases {
        let err = serialize_value(&value).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("Unsupported Forthic type: {type_name}")
        );
    }
}

#[test]
fn test_unsupported_type_reports_nested_path() {
    let mut rec = HashMap::new();
    rec.insert(
        "opts".to_string(),
        ForthicValue::Array(vec![ForthicValue::WordOptions(WordOptions::new())]),
    );
    let err = serialize_value(&ForthicValue::Record(rec)).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Unsupported Forthic type: WordOptions at path: .opts[0]"
    );
}

#[test]
fn test_malformed_plain_time_is_an_error() {
    let wire = json!({ "plain_time_value": { "iso8601_time": "half past nine" } });
    let err = deserialize_value(&wire).unwrap_err();
    assert!(err.to_string().contains("Invalid plain time"), "got: {err}");
}

#[test]
fn test_non_finite_floats_fail_to_serialize() {
    for f in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(serialize_value(&ForthicValue::Float(f)).is_err());
    }
}
