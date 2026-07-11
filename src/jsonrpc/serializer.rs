//! StackValue serialization for Forthic values
//!
//! Converts `ForthicValue` to/from the tagged JSON "StackValue" format shared
//! by all Forthic runtimes (see forthic-ts `src/grpc/serializer.ts`):
//!
//! ```json
//! { "int_value": 42 }
//! { "array_value": { "items": [ ... ] } }
//! { "zoned_datetime_value": { "iso8601": "2020-06-05T10:15:00-07:00[America/Los_Angeles]",
//!                             "timezone": "America/Los_Angeles" } }
//! ```
//!
//! Cross-runtime type mapping:
//! - `Date` ↔ `plain_date_value` (Temporal.PlainDate in ts)
//! - `Time` ↔ `plain_time_value` (Temporal.PlainTime in ts)
//! - `DateTime` → `zoned_datetime_value`; both `zoned_datetime_value` and
//!   `instant_value` deserialize to `DateTime` (instants land in UTC).
//!   The `iso8601` field carries Temporal's bracketed timezone annotation
//!   (`...-07:00[America/Los_Angeles]`) because ts parses it with
//!   `Temporal.ZonedDateTime.from`, which requires the annotation.
//! - `WordOptions` and interpreter-internal markers have no wire
//!   representation and fail to serialize, matching ts behavior for types
//!   its serializer doesn't recognize.

use crate::literals::ForthicValue;
use chrono::SecondsFormat;
use chrono_tz::Tz;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use thiserror::Error;

/// Errors from StackValue serialization/deserialization
///
/// Messages mirror forthic-ts where a counterpart exists, including the
/// ` at path: ...` suffix that pinpoints the failure inside nested values.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum SerializerError {
    /// The ForthicValue has no wire representation
    #[error("Unsupported Forthic type: {type_name}{}", path_suffix(.path))]
    UnsupportedType { type_name: String, path: String },

    /// The JSON value carries none of the known StackValue tags
    #[error("Unknown stack value type{}", path_suffix(.path))]
    UnknownStackValue { path: String },

    /// A known tag holds a malformed payload
    #[error("{message}{}", path_suffix(.path))]
    Invalid { message: String, path: String },
}

fn path_suffix(path: &str) -> String {
    if path.is_empty() {
        String::new()
    } else {
        format!(" at path: {path}")
    }
}

fn invalid(message: impl Into<String>, path: &str) -> SerializerError {
    SerializerError::Invalid {
        message: message.into(),
        path: path.to_string(),
    }
}

/// Serialize a ForthicValue to a StackValue JSON value
pub fn serialize_value(value: &ForthicValue) -> Result<Value, SerializerError> {
    serialize_at(value, "")
}

/// Deserialize a StackValue JSON value to a ForthicValue
pub fn deserialize_value(stack_value: &Value) -> Result<ForthicValue, SerializerError> {
    deserialize_at(stack_value, "")
}

fn serialize_at(value: &ForthicValue, path: &str) -> Result<Value, SerializerError> {
    match value {
        ForthicValue::Null => Ok(json!({ "null_value": {} })),
        ForthicValue::Bool(b) => Ok(json!({ "bool_value": b })),
        ForthicValue::Int(i) => Ok(json!({ "int_value": i })),
        ForthicValue::Float(f) => {
            if !f.is_finite() {
                return Err(invalid(
                    format!("Non-finite float has no JSON representation: {f}"),
                    path,
                ));
            }
            Ok(json!({ "float_value": f }))
        }
        ForthicValue::String(s) => Ok(json!({ "string_value": s })),
        ForthicValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                out.push(serialize_at(item, &format!("{path}[{i}]"))?);
            }
            Ok(json!({ "array_value": { "items": out } }))
        }
        ForthicValue::Record(fields) => {
            let mut out = Map::new();
            for (key, val) in fields {
                let child_path = format!("{path}{}", path_segment_for_key(key));
                out.insert(key.clone(), serialize_at(val, &child_path)?);
            }
            Ok(json!({ "record_value": { "fields": out } }))
        }
        ForthicValue::Date(d) => Ok(json!({
            "plain_date_value": { "iso8601_date": d.format("%Y-%m-%d").to_string() }
        })),
        ForthicValue::DateTime(dt) => {
            let tz_name = dt.timezone().name();
            // AutoSi matches Temporal.toString(): no fractional digits for
            // whole seconds, millisecond precision (".123") when present.
            let rfc3339 = dt.to_rfc3339_opts(SecondsFormat::AutoSi, false);
            Ok(json!({
                "zoned_datetime_value": {
                    "iso8601": format!("{rfc3339}[{tz_name}]"),
                    "timezone": tz_name,
                }
            }))
        }
        ForthicValue::Time(t) => Ok(json!({
            // %.f matches Temporal.toString() trimming: nothing for whole
            // seconds, minimal 3/6/9 digits otherwise
            "plain_time_value": { "iso8601_time": t.format("%H:%M:%S%.f").to_string() }
        })),
        ForthicValue::WordOptions(_) => Err(unsupported("WordOptions", path)),
        ForthicValue::StartArrayMarker => Err(unsupported("StartArrayMarker", path)),
    }
}

fn unsupported(type_name: &str, path: &str) -> SerializerError {
    SerializerError::UnsupportedType {
        type_name: type_name.to_string(),
        path: path.to_string(),
    }
}

/// Path segment for a record key in error messages: `.key` for identifier-like
/// keys, `["key"]` otherwise (mirrors ts `pathSegmentForKey`)
fn path_segment_for_key(key: &str) -> String {
    let mut chars = key.chars();
    let ident_start = |c: char| c.is_ascii_alphabetic() || c == '_' || c == '$';
    let is_ident = match chars.next() {
        Some(first) => {
            ident_start(first) && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
        }
        None => false,
    };
    if is_ident {
        format!(".{key}")
    } else {
        format!("[{}]", Value::String(key.to_string()))
    }
}

fn deserialize_at(stack_value: &Value, path: &str) -> Result<ForthicValue, SerializerError> {
    let obj = match stack_value.as_object() {
        Some(obj) => obj,
        None => {
            return Err(SerializerError::UnknownStackValue {
                path: path.to_string(),
            })
        }
    };

    // Tag checks in the same order as ts deserializeValue
    if let Some(v) = obj.get("int_value") {
        let i = v
            .as_i64()
            .ok_or_else(|| invalid("int_value must be an integer", path))?;
        return Ok(ForthicValue::Int(i));
    }
    if let Some(v) = obj.get("string_value") {
        let s = v
            .as_str()
            .ok_or_else(|| invalid("string_value must be a string", path))?;
        return Ok(ForthicValue::String(s.to_string()));
    }
    if let Some(v) = obj.get("bool_value") {
        let b = v
            .as_bool()
            .ok_or_else(|| invalid("bool_value must be a boolean", path))?;
        return Ok(ForthicValue::Bool(b));
    }
    if let Some(v) = obj.get("float_value") {
        let f = v
            .as_f64()
            .ok_or_else(|| invalid("float_value must be a number", path))?;
        return Ok(ForthicValue::Float(f));
    }
    if obj.contains_key("null_value") {
        return Ok(ForthicValue::Null);
    }
    if let Some(v) = obj.get("instant_value") {
        let iso = require_str_field(v, "iso8601", path)?;
        let dt = chrono::DateTime::parse_from_rfc3339(iso)
            .map_err(|e| invalid(format!("Invalid instant '{iso}': {e}"), path))?;
        return Ok(ForthicValue::DateTime(dt.with_timezone(&Tz::UTC)));
    }
    if let Some(v) = obj.get("plain_date_value") {
        let iso = require_str_field(v, "iso8601_date", path)?;
        let date = chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d")
            .map_err(|e| invalid(format!("Invalid plain date '{iso}': {e}"), path))?;
        return Ok(ForthicValue::Date(date));
    }
    if let Some(v) = obj.get("plain_time_value") {
        let iso = require_str_field(v, "iso8601_time", path)?;
        let time = chrono::NaiveTime::parse_from_str(iso, "%H:%M:%S%.f")
            .or_else(|_| chrono::NaiveTime::parse_from_str(iso, "%H:%M"))
            .map_err(|e| invalid(format!("Invalid plain time '{iso}': {e}"), path))?;
        return Ok(ForthicValue::Time(time));
    }
    if let Some(v) = obj.get("zoned_datetime_value") {
        let iso = require_str_field(v, "iso8601", path)?;
        let tz_field = v.get("timezone").and_then(Value::as_str).unwrap_or("");
        return parse_zoned_datetime(iso, tz_field, path);
    }
    if let Some(v) = obj.get("array_value") {
        let items = v
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("array_value requires array \"items\"", path))?;
        let mut out = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            out.push(deserialize_at(item, &format!("{path}[{i}]"))?);
        }
        return Ok(ForthicValue::Array(out));
    }
    if let Some(v) = obj.get("record_value") {
        let fields = v
            .get("fields")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("record_value requires object \"fields\"", path))?;
        let mut out = HashMap::with_capacity(fields.len());
        for (key, val) in fields {
            let child_path = format!("{path}{}", path_segment_for_key(key));
            out.insert(key.clone(), deserialize_at(val, &child_path)?);
        }
        return Ok(ForthicValue::Record(out));
    }

    Err(SerializerError::UnknownStackValue {
        path: path.to_string(),
    })
}

fn require_str_field<'a>(
    container: &'a Value,
    field: &str,
    path: &str,
) -> Result<&'a str, SerializerError> {
    container
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("Missing string field \"{field}\""), path))
}

/// Parse a zoned datetime from Temporal's string form
/// (`2020-06-05T10:15:00-07:00[America/Los_Angeles]`). The bracket annotation
/// is optional here; the explicit `timezone` field wins when both are present.
fn parse_zoned_datetime(
    iso: &str,
    tz_field: &str,
    path: &str,
) -> Result<ForthicValue, SerializerError> {
    let (datetime_part, bracket_tz) = match (iso.find('['), iso.rfind(']')) {
        (Some(open), Some(close)) if close > open => (&iso[..open], &iso[open + 1..close]),
        _ => (iso, ""),
    };
    let tz_name = if !tz_field.is_empty() { tz_field } else { bracket_tz };
    let tz: Tz = tz_name
        .parse()
        .map_err(|_| invalid(format!("Unknown timezone '{tz_name}'"), path))?;
    let dt = chrono::DateTime::parse_from_rfc3339(datetime_part)
        .map_err(|e| invalid(format!("Invalid zoned datetime '{datetime_part}': {e}"), path))?;
    Ok(ForthicValue::DateTime(dt.with_timezone(&tz)))
}
