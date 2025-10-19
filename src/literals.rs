//! Literal parsing for the Forthic interpreter
//!
//! This module provides literal parsing functions that convert string tokens into typed values.
//! These handlers are used by the Forthic interpreter to recognize and parse different literal types.
//!
//! Built-in literal types:
//! - Boolean: TRUE, FALSE
//! - Integer: 42, -10, 0
//! - Float: 3.14, -2.5, 0.0
//! - Time: 9:00, 11:30 PM, 22:15
//! - Date: 2020-06-05, YYYY-MM-DD (with wildcards)
//! - ZonedDateTime: ISO 8601 timestamps with timezone support

use chrono::{Datelike, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use regex::Regex;
use std::collections::HashMap;

/// Core value type for Forthic
#[derive(Debug, Clone, PartialEq)]
pub enum ForthicValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<ForthicValue>),
    Record(HashMap<String, ForthicValue>),
    Date(NaiveDate),
    Time(NaiveTime),
    DateTime(chrono::DateTime<Tz>),
}

impl ForthicValue {
    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, ForthicValue::Null)
    }

    /// Convert to string if possible
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ForthicValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to integer if possible
    pub fn as_int(&self) -> Option<i64> {
        match self {
            ForthicValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Convert to float if possible
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ForthicValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Convert to bool if possible
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ForthicValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

/// Literal handler function type
///
/// Takes a string and returns a parsed ForthicValue or None if can't parse
pub type LiteralHandler = fn(&str) -> Option<ForthicValue>;

/// Parse boolean literals: TRUE, FALSE
///
/// # Examples
///
/// ```
/// use forthic::literals::to_bool;
///
/// assert!(to_bool("TRUE").is_some());
/// assert!(to_bool("FALSE").is_some());
/// assert!(to_bool("true").is_none());
/// ```
pub fn to_bool(s: &str) -> Option<ForthicValue> {
    match s {
        "TRUE" => Some(ForthicValue::Bool(true)),
        "FALSE" => Some(ForthicValue::Bool(false)),
        _ => None,
    }
}

/// Parse float literals: 3.14, -2.5, 0.0
///
/// Must contain a decimal point to be recognized as a float.
///
/// # Examples
///
/// ```
/// use forthic::literals::to_float;
///
/// assert!(to_float("3.14").is_some());
/// assert!(to_float("-2.5").is_some());
/// assert!(to_float("42").is_none()); // No decimal point
/// ```
pub fn to_float(s: &str) -> Option<ForthicValue> {
    // Must contain a decimal point
    if !s.contains('.') {
        return None;
    }

    s.parse::<f64>().ok().map(ForthicValue::Float)
}

/// Parse integer literals: 42, -10, 0
///
/// Must not contain a decimal point.
///
/// # Examples
///
/// ```
/// use forthic::literals::to_int;
///
/// assert!(to_int("42").is_some());
/// assert!(to_int("-10").is_some());
/// assert!(to_int("3.14").is_none()); // Has decimal point
/// ```
pub fn to_int(s: &str) -> Option<ForthicValue> {
    // Must not contain a decimal point
    if s.contains('.') {
        return None;
    }

    // Parse the integer
    let result = s.parse::<i64>().ok()?;

    // Verify it's actually an integer string (not "42abc")
    if result.to_string() != s {
        return None;
    }

    Some(ForthicValue::Int(result))
}

/// Parse time literals: 9:00, 11:30 PM, 22:15
///
/// Supports both 24-hour format and 12-hour format with AM/PM.
///
/// # Examples
///
/// ```
/// use forthic::literals::to_time;
///
/// assert!(to_time("14:30").is_some());
/// assert!(to_time("2:30 PM").is_some());
/// assert!(to_time("11:30 AM").is_some());
/// ```
pub fn to_time(s: &str) -> Option<ForthicValue> {
    // Regex: HH:MM or H:MM with optional AM/PM
    let re = Regex::new(r"^(\d{1,2}):(\d{2})(?:\s*(AM|PM))?$").ok()?;
    let caps = re.captures(s)?;

    let mut hours = caps.get(1)?.as_str().parse::<u32>().ok()?;
    let minutes = caps.get(2)?.as_str().parse::<u32>().ok()?;
    let meridiem = caps.get(3).map(|m| m.as_str());

    // Adjust for AM/PM
    if let Some(m) = meridiem {
        match m {
            "PM" => {
                if hours < 12 {
                    hours += 12;
                }
            }
            "AM" => {
                if hours == 12 {
                    hours = 0;
                } else if hours > 12 {
                    // Handle invalid cases like "22:15 AM"
                    hours -= 12;
                }
            }
            _ => {}
        }
    }

    // Validate hours and minutes
    if hours > 23 || minutes >= 60 {
        return None;
    }

    NaiveTime::from_hms_opt(hours, minutes, 0).map(ForthicValue::Time)
}

/// Create a date literal parser with timezone support
///
/// Parses dates in format: YYYY-MM-DD
/// Supports wildcards: YYYY, MM, DD which use current values from the timezone
///
/// # Arguments
///
/// * `timezone` - Timezone to use for wildcard resolution
///
/// # Examples
///
/// ```
/// use forthic::literals::to_literal_date;
///
/// let parser = to_literal_date("UTC");
/// assert!(parser("2023-12-25").is_some());
/// assert!(parser("YYYY-12-25").is_some()); // Uses current year
/// ```
pub fn to_literal_date(timezone: &str) -> impl Fn(&str) -> Option<ForthicValue> + '_ {
    move |s: &str| {
        // Regex: YYYY-MM-DD or wildcards
        let re = Regex::new(r"^(\d{4}|YYYY)-(\d{2}|MM)-(\d{2}|DD)$").ok()?;
        let caps = re.captures(s)?;

        // Get current date in the timezone for wildcard substitution
        let tz: Tz = timezone.parse().ok()?;
        let now = Utc::now().with_timezone(&tz);

        // Parse components with wildcard support
        let year = match caps.get(1)?.as_str() {
            "YYYY" => now.year(),
            y => y.parse::<i32>().ok()?,
        };

        let month = match caps.get(2)?.as_str() {
            "MM" => now.month(),
            m => m.parse::<u32>().ok()?,
        };

        let day = match caps.get(3)?.as_str() {
            "DD" => now.day(),
            d => d.parse::<u32>().ok()?,
        };

        NaiveDate::from_ymd_opt(year, month, day).map(ForthicValue::Date)
    }
}

/// Create a zoned datetime literal parser with timezone support
///
/// Parses ISO 8601 datetime strings:
/// - With UTC: 2025-05-24T10:15:00Z
/// - With offset: 2025-05-24T10:15:00-05:00
/// - Without timezone: 2025-05-24T10:15:00 (uses provided timezone)
///
/// # Arguments
///
/// * `timezone` - Default timezone to use if not specified in string
///
/// # Examples
///
/// ```
/// use forthic::literals::to_zoned_datetime;
///
/// let parser = to_zoned_datetime("America/Los_Angeles");
/// assert!(parser("2023-12-25T14:30:00Z").is_some());
/// assert!(parser("2023-12-25T14:30:00-08:00").is_some());
/// ```
pub fn to_zoned_datetime(timezone: &str) -> impl Fn(&str) -> Option<ForthicValue> + '_ {
    move |s: &str| {
        // Must have 'T' separator for datetime
        if !s.contains('T') {
            return None;
        }

        let tz: Tz = timezone.parse().ok()?;

        // Handle explicit UTC (Z suffix)
        if s.ends_with('Z') {
            let dt = chrono::DateTime::parse_from_rfc3339(s).ok()?;
            return Some(ForthicValue::DateTime(dt.with_timezone(&tz)));
        }

        // Handle explicit timezone offset (+05:00, -05:00)
        let offset_re = Regex::new(r"[+-]\d{2}:\d{2}$").ok()?;
        if offset_re.is_match(s) {
            let dt = chrono::DateTime::parse_from_rfc3339(s).ok()?;
            return Some(ForthicValue::DateTime(dt.with_timezone(&tz)));
        }

        // No timezone specified, use interpreter's timezone
        // Parse as NaiveDateTime first
        let naive_dt = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").ok()?;

        // Convert to timezone-aware DateTime
        // Use earliest option in case of DST ambiguity
        tz.from_local_datetime(&naive_dt)
            .earliest()
            .map(ForthicValue::DateTime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn test_to_bool() {
        assert_eq!(to_bool("TRUE"), Some(ForthicValue::Bool(true)));
        assert_eq!(to_bool("FALSE"), Some(ForthicValue::Bool(false)));
        assert_eq!(to_bool("true"), None);
        assert_eq!(to_bool("false"), None);
        assert_eq!(to_bool("True"), None);
    }

    #[test]
    fn test_to_int() {
        assert_eq!(to_int("42"), Some(ForthicValue::Int(42)));
        assert_eq!(to_int("-10"), Some(ForthicValue::Int(-10)));
        assert_eq!(to_int("0"), Some(ForthicValue::Int(0)));
        assert_eq!(to_int("3.14"), None); // Has decimal
        assert_eq!(to_int("42abc"), None); // Invalid
        assert_eq!(to_int("abc"), None);
    }

    #[test]
    fn test_to_float() {
        assert_eq!(to_float("3.14"), Some(ForthicValue::Float(3.14)));
        assert_eq!(to_float("-2.5"), Some(ForthicValue::Float(-2.5)));
        assert_eq!(to_float("0.0"), Some(ForthicValue::Float(0.0)));
        assert_eq!(to_float("42"), None); // No decimal
        assert_eq!(to_float("abc.def"), None); // Invalid
    }

    #[test]
    fn test_to_time_24hour() {
        let time = to_time("14:30").unwrap();
        if let ForthicValue::Time(t) = time {
            assert_eq!(t.hour(), 14);
            assert_eq!(t.minute(), 30);
        } else {
            panic!("Expected Time");
        }
    }

    #[test]
    fn test_to_time_12hour_pm() {
        let time = to_time("2:30 PM").unwrap();
        if let ForthicValue::Time(t) = time {
            assert_eq!(t.hour(), 14); // 2 PM = 14:00
            assert_eq!(t.minute(), 30);
        } else {
            panic!("Expected Time");
        }
    }

    #[test]
    fn test_to_time_12hour_am() {
        let time = to_time("11:30 AM").unwrap();
        if let ForthicValue::Time(t) = time {
            assert_eq!(t.hour(), 11);
            assert_eq!(t.minute(), 30);
        } else {
            panic!("Expected Time");
        }
    }

    #[test]
    fn test_to_time_midnight() {
        let time = to_time("12:00 AM").unwrap();
        if let ForthicValue::Time(t) = time {
            assert_eq!(t.hour(), 0); // 12 AM = 00:00
            assert_eq!(t.minute(), 0);
        } else {
            panic!("Expected Time");
        }
    }

    #[test]
    fn test_to_time_noon() {
        let time = to_time("12:00 PM").unwrap();
        if let ForthicValue::Time(t) = time {
            assert_eq!(t.hour(), 12); // 12 PM = 12:00
            assert_eq!(t.minute(), 0);
        } else {
            panic!("Expected Time");
        }
    }

    #[test]
    fn test_to_time_invalid() {
        assert!(to_time("25:00").is_none()); // Invalid hour
        assert!(to_time("12:60").is_none()); // Invalid minute
        assert!(to_time("abc").is_none()); // Not a time
        assert!(to_time("12:30:45").is_none()); // Has seconds (not supported)
    }

    #[test]
    fn test_to_literal_date() {
        let parser = to_literal_date("UTC");

        let date = parser("2023-12-25").unwrap();
        if let ForthicValue::Date(d) = date {
            assert_eq!(d.year(), 2023);
            assert_eq!(d.month(), 12);
            assert_eq!(d.day(), 25);
        } else {
            panic!("Expected Date");
        }
    }

    #[test]
    fn test_to_literal_date_with_wildcards() {
        let parser = to_literal_date("UTC");

        // YYYY-12-25 should use current year
        let date = parser("YYYY-12-25");
        assert!(date.is_some());
        if let Some(ForthicValue::Date(d)) = date {
            assert_eq!(d.month(), 12);
            assert_eq!(d.day(), 25);
            // Year should be current year
        }

        // 2023-MM-25 should use current month
        let date = parser("2023-MM-25");
        assert!(date.is_some());

        // 2023-12-DD should use current day
        let date = parser("2023-12-DD");
        assert!(date.is_some());
    }

    #[test]
    fn test_to_literal_date_invalid() {
        let parser = to_literal_date("UTC");

        assert!(parser("invalid").is_none());
        assert!(parser("2023-13-01").is_none()); // Invalid month
        assert!(parser("2023-12-32").is_none()); // Invalid day
        assert!(parser("23-12-25").is_none()); // Wrong format
    }

    #[test]
    fn test_to_zoned_datetime_utc() {
        let parser = to_zoned_datetime("UTC");

        let dt = parser("2023-12-25T14:30:00Z").unwrap();
        if let ForthicValue::DateTime(d) = dt {
            assert_eq!(d.year(), 2023);
            assert_eq!(d.month(), 12);
            assert_eq!(d.day(), 25);
            assert_eq!(d.hour(), 14);
            assert_eq!(d.minute(), 30);
        } else {
            panic!("Expected DateTime");
        }
    }

    #[test]
    fn test_to_zoned_datetime_with_offset() {
        let parser = to_zoned_datetime("UTC");

        let dt = parser("2023-12-25T14:30:00-08:00");
        assert!(dt.is_some());
    }

    #[test]
    fn test_to_zoned_datetime_no_timezone() {
        let parser = to_zoned_datetime("America/Los_Angeles");

        let dt = parser("2023-12-25T14:30:00").unwrap();
        if let ForthicValue::DateTime(d) = dt {
            assert_eq!(d.year(), 2023);
            assert_eq!(d.month(), 12);
            assert_eq!(d.day(), 25);
            assert_eq!(d.hour(), 14);
            assert_eq!(d.minute(), 30);
        } else {
            panic!("Expected DateTime");
        }
    }

    #[test]
    fn test_to_zoned_datetime_invalid() {
        let parser = to_zoned_datetime("UTC");

        assert!(parser("invalid").is_none());
        assert!(parser("2023-12-25").is_none()); // No time component
        assert!(parser("not-a-datetime").is_none());
    }

    #[test]
    fn test_forthic_value_type_checks() {
        assert!(ForthicValue::Null.is_null());
        assert!(!ForthicValue::Bool(true).is_null());

        let val = ForthicValue::Int(42);
        assert_eq!(val.as_int(), Some(42));
        assert_eq!(val.as_float(), None);

        let val = ForthicValue::String("hello".to_string());
        assert_eq!(val.as_string(), Some("hello"));
        assert_eq!(val.as_int(), None);
    }
}
