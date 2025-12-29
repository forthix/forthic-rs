use forthic::literals::{to_zoned_datetime, ForthicValue};
use chrono::Timelike;
use chrono_tz::Tz;

// Literal Parsing Tests

#[test]
fn test_parse_utc_datetime_with_z_suffix() {
    let parser = to_zoned_datetime("America/New_York");
    let result = parser("2025-05-24T10:15:00Z");

    assert!(result.is_some());
    if let ForthicValue::DateTime(dt) = result.unwrap() {
        assert_eq!(dt.timezone().name(), "UTC");
    } else {
        panic!("Expected DateTime");
    }
}

#[test]
fn test_parse_iana_timezone_with_bracket_notation() {
    let parser = to_zoned_datetime("America/New_York");
    let result = parser("2025-05-20T08:00:00[America/Los_Angeles]");

    assert!(result.is_some());
    if let ForthicValue::DateTime(dt) = result.unwrap() {
        assert_eq!(dt.timezone().name(), "America/Los_Angeles");
        assert_eq!(dt.hour(), 8);
    } else {
        panic!("Expected DateTime");
    }
}

#[test]
fn test_parse_datetime_with_offset_and_iana_timezone() {
    let parser = to_zoned_datetime("America/New_York");
    let result = parser("2025-05-20T08:00:00-07:00[America/Los_Angeles]");

    assert!(result.is_some());
    if let ForthicValue::DateTime(dt) = result.unwrap() {
        assert_eq!(dt.timezone().name(), "America/Los_Angeles");
        assert_eq!(dt.hour(), 8);
    } else {
        panic!("Expected DateTime");
    }
}

#[test]
fn test_parse_datetime_with_offset_only() {
    let parser = to_zoned_datetime("America/New_York");
    let result = parser("2025-05-24T10:15:00-05:00");

    assert!(result.is_some());
    // The datetime will be converted to the default timezone (America/New_York)
    // May 24, 2025 is during EDT (UTC-4), so 10:15-05:00 becomes 11:15-04:00
    if let ForthicValue::DateTime(dt) = result.unwrap() {
        // Just verify we got a valid datetime; hour will depend on timezone conversion
        assert_eq!(dt.minute(), 15);
    } else {
        panic!("Expected DateTime");
    }
}

#[test]
fn test_parse_datetime_without_timezone_uses_default() {
    let parser = to_zoned_datetime("America/Los_Angeles");
    let result = parser("2025-05-24T10:15:00");

    assert!(result.is_some());
    if let ForthicValue::DateTime(dt) = result.unwrap() {
        assert_eq!(dt.timezone().name(), "America/Los_Angeles");
        assert_eq!(dt.hour(), 10);
    } else {
        panic!("Expected DateTime");
    }
}

#[test]
fn test_parse_various_iana_timezones() {
    let parser = to_zoned_datetime("UTC");

    // Europe/London
    let result1 = parser("2025-05-20T14:30:00[Europe/London]");
    assert!(result1.is_some());
    if let ForthicValue::DateTime(dt) = result1.unwrap() {
        assert_eq!(dt.timezone().name(), "Europe/London");
    } else {
        panic!("Expected DateTime");
    }

    // Asia/Tokyo
    let result2 = parser("2025-05-20T09:00:00[Asia/Tokyo]");
    assert!(result2.is_some());
    if let ForthicValue::DateTime(dt) = result2.unwrap() {
        assert_eq!(dt.timezone().name(), "Asia/Tokyo");
    } else {
        panic!("Expected DateTime");
    }

    // Australia/Sydney
    let result3 = parser("2025-05-20T18:00:00[Australia/Sydney]");
    assert!(result3.is_some());
    if let ForthicValue::DateTime(dt) = result3.unwrap() {
        assert_eq!(dt.timezone().name(), "Australia/Sydney");
    } else {
        panic!("Expected DateTime");
    }
}

#[test]
fn test_returns_none_for_invalid_iana_timezone() {
    let parser = to_zoned_datetime("America/New_York");
    let result = parser("2025-05-20T08:00:00[Invalid/Timezone]");

    assert!(result.is_none());
}

#[test]
fn test_returns_none_for_strings_without_t() {
    let parser = to_zoned_datetime("America/New_York");

    assert!(parser("2025-05-20").is_none());
    assert!(parser("regular-word").is_none());
    assert!(parser("08:00:00").is_none());
}

#[test]
fn test_returns_none_for_malformed_datetime_strings() {
    let parser = to_zoned_datetime("America/New_York");

    assert!(parser("2025-13-45T10:15:00").is_none()); // Invalid month/day
    assert!(parser("not-a-datetime[America/Los_Angeles]").is_none());
    assert!(parser("2025-05-20T25:00:00").is_none()); // Invalid hour
}

#[test]
fn test_returns_none_for_brackets_without_datetime() {
    let parser = to_zoned_datetime("America/New_York");

    assert!(parser("[America/Los_Angeles]").is_none());
    assert!(parser("word[bracket]").is_none());
}

#[test]
fn test_parse_datetime_with_seconds() {
    let parser = to_zoned_datetime("America/New_York");
    let result = parser("2025-05-20T08:30:45[America/Los_Angeles]");

    assert!(result.is_some());
    if let ForthicValue::DateTime(dt) = result.unwrap() {
        assert_eq!(dt.timezone().name(), "America/Los_Angeles");
        assert_eq!(dt.hour(), 8);
        assert_eq!(dt.minute(), 30);
        assert_eq!(dt.second(), 45);
    } else {
        panic!("Expected DateTime");
    }
}

#[test]
fn test_parse_utc_datetime_with_brackets() {
    let parser = to_zoned_datetime("America/New_York");
    let result = parser("2025-05-20T08:00:00Z[UTC]");

    assert!(result.is_some());
    if let ForthicValue::DateTime(dt) = result.unwrap() {
        assert_eq!(dt.timezone().name(), "UTC");
    } else {
        panic!("Expected DateTime");
    }
}

#[test]
fn test_preserves_instant_in_time() {
    let parser = to_zoned_datetime("America/New_York");
    let result = parser("2025-05-20T08:00:00[America/Los_Angeles]");

    assert!(result.is_some());
    if let ForthicValue::DateTime(dt) = result.unwrap() {
        // Convert to UTC to verify it's the same instant
        let utc_tz: Tz = "UTC".parse().unwrap();
        let utc_time = dt.with_timezone(&utc_tz);

        // 8 AM PDT (UTC-7) = 3 PM UTC (May is during PDT)
        assert_eq!(utc_time.hour(), 15); // 8 + 7 = 15 (3 PM)
    } else {
        panic!("Expected DateTime");
    }
}
