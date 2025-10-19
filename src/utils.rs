//! Utility functions for the Forthic interpreter
//!
//! This module provides helper functions for date/time handling,
//! string manipulation, and common type conversions.

use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

/// Parse a datetime string and create a timezone-aware DateTime
///
/// Parses datetime strings in the format "YYYY-MM-DD HH:MM:SS" and
/// converts them to a timezone-aware DateTime in the specified timezone.
///
/// # Arguments
///
/// * `date_string` - Date string in format "YYYY-MM-DD HH:MM:SS"
/// * `timezone` - Timezone name (e.g., "America/Los_Angeles", "UTC")
///
/// # Returns
///
/// * `Some(DateTime<Tz>)` - Parsed datetime in the specified timezone
/// * `None` - If parsing fails or timezone is invalid
///
/// # Examples
///
/// ```
/// use forthic::utils::to_zoned_datetime;
///
/// let dt = to_zoned_datetime("2023-12-25 14:30:00", "America/Los_Angeles");
/// assert!(dt.is_some());
///
/// let dt = to_zoned_datetime("invalid", "UTC");
/// assert!(dt.is_none());
/// ```
pub fn to_zoned_datetime(date_string: &str, timezone: &str) -> Option<DateTime<Tz>> {
    // Parse timezone
    let tz: Tz = timezone.parse().ok()?;

    // Ensure we have enough characters for the full datetime format
    if date_string.len() < 19 {
        return None;
    }

    // Extract date and time components
    // Format: "YYYY-MM-DD HH:MM:SS"
    let year = date_string.get(0..4)?.parse::<i32>().ok()?;
    let month = date_string.get(5..7)?.parse::<u32>().ok()?;
    let day = date_string.get(8..10)?.parse::<u32>().ok()?;
    let hour = date_string.get(11..13)?.parse::<u32>().ok()?;
    let minute = date_string.get(14..16)?.parse::<u32>().ok()?;
    let second = date_string.get(17..19)?.parse::<u32>().ok()?;

    // Create NaiveDate and NaiveTime
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(hour, minute, second)?;

    // Combine into NaiveDateTime
    let naive_dt = date.and_time(time);

    // Convert to timezone-aware DateTime
    // Use earliest option in case of DST ambiguity
    tz.from_local_datetime(&naive_dt).earliest()
}

/// Convert a UTC DateTime to a specific timezone
///
/// # Arguments
///
/// * `dt` - DateTime in UTC
/// * `timezone` - Target timezone name
///
/// # Returns
///
/// * `Some(DateTime<Tz>)` - DateTime in the target timezone
/// * `None` - If timezone is invalid
///
/// # Examples
///
/// ```
/// use chrono::Utc;
/// use forthic::utils::convert_timezone;
///
/// let utc_now = Utc::now();
/// let la_time = convert_timezone(&utc_now, "America/Los_Angeles");
/// assert!(la_time.is_some());
/// ```
pub fn convert_timezone(dt: &DateTime<Utc>, timezone: &str) -> Option<DateTime<Tz>> {
    let tz: Tz = timezone.parse().ok()?;
    Some(dt.with_timezone(&tz))
}

/// Format a DateTime as a string in the format "YYYY-MM-DD HH:MM:SS"
///
/// # Arguments
///
/// * `dt` - DateTime to format
///
/// # Returns
///
/// Formatted datetime string
///
/// # Examples
///
/// ```
/// use chrono::Utc;
/// use forthic::utils::format_datetime;
///
/// let dt = Utc::now();
/// let formatted = format_datetime(&dt);
/// assert!(formatted.contains('-'));
/// assert!(formatted.contains(':'));
/// ```
pub fn format_datetime<T: TimeZone>(dt: &DateTime<T>) -> String
where
    T::Offset: std::fmt::Display,
{
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Parse a date string in format "YYYY-MM-DD"
///
/// # Arguments
///
/// * `date_string` - Date string in format "YYYY-MM-DD"
///
/// # Returns
///
/// * `Some(NaiveDate)` - Parsed date
/// * `None` - If parsing fails
///
/// # Examples
///
/// ```
/// use forthic::utils::parse_date;
///
/// let date = parse_date("2023-12-25");
/// assert!(date.is_some());
///
/// let invalid = parse_date("invalid");
/// assert!(invalid.is_none());
/// ```
pub fn parse_date(date_string: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date_string, "%Y-%m-%d").ok()
}

/// Parse a time string in format "HH:MM:SS"
///
/// # Arguments
///
/// * `time_string` - Time string in format "HH:MM:SS"
///
/// # Returns
///
/// * `Some(NaiveTime)` - Parsed time
/// * `None` - If parsing fails
///
/// # Examples
///
/// ```
/// use forthic::utils::parse_time;
///
/// let time = parse_time("14:30:00");
/// assert!(time.is_some());
///
/// let invalid = parse_time("invalid");
/// assert!(invalid.is_none());
/// ```
pub fn parse_time(time_string: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(time_string, "%H:%M:%S").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike, Utc};

    #[test]
    fn test_to_zoned_datetime_valid() {
        let dt = to_zoned_datetime("2023-12-25 14:30:00", "America/Los_Angeles");
        assert!(dt.is_some());

        let dt = dt.unwrap();
        assert_eq!(dt.year(), 2023);
        assert_eq!(dt.month(), 12);
        assert_eq!(dt.day(), 25);
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 30);
        assert_eq!(dt.second(), 0);
    }

    #[test]
    fn test_to_zoned_datetime_utc() {
        let dt = to_zoned_datetime("2023-06-15 10:00:00", "UTC");
        assert!(dt.is_some());

        let dt = dt.unwrap();
        assert_eq!(dt.year(), 2023);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_to_zoned_datetime_invalid_format() {
        let dt = to_zoned_datetime("invalid", "UTC");
        assert!(dt.is_none());

        let dt = to_zoned_datetime("2023-12-25", "UTC");
        assert!(dt.is_none());

        let dt = to_zoned_datetime("2023-99-99 25:99:99", "UTC");
        assert!(dt.is_none());
    }

    #[test]
    fn test_to_zoned_datetime_invalid_timezone() {
        let dt = to_zoned_datetime("2023-12-25 14:30:00", "Invalid/Timezone");
        assert!(dt.is_none());
    }

    #[test]
    fn test_convert_timezone() {
        let utc_now = Utc::now();
        let la_time = convert_timezone(&utc_now, "America/Los_Angeles");
        assert!(la_time.is_some());

        // UTC offset should be different for LA
        let la = la_time.unwrap();
        assert_eq!(utc_now.timestamp(), la.timestamp()); // Same moment in time
    }

    #[test]
    fn test_convert_timezone_invalid() {
        let utc_now = Utc::now();
        let invalid = convert_timezone(&utc_now, "Invalid/Timezone");
        assert!(invalid.is_none());
    }

    #[test]
    fn test_format_datetime() {
        let tz: Tz = "UTC".parse().unwrap();
        let dt = tz
            .with_ymd_and_hms(2023, 12, 25, 14, 30, 0)
            .unwrap();

        let formatted = format_datetime(&dt);
        assert_eq!(formatted, "2023-12-25 14:30:00");
    }

    #[test]
    fn test_parse_date_valid() {
        let date = parse_date("2023-12-25");
        assert!(date.is_some());

        let date = date.unwrap();
        assert_eq!(date.year(), 2023);
        assert_eq!(date.month(), 12);
        assert_eq!(date.day(), 25);
    }

    #[test]
    fn test_parse_date_invalid() {
        assert!(parse_date("invalid").is_none());
        assert!(parse_date("2023-99-99").is_none());
        assert!(parse_date("12/25/2023").is_none()); // Wrong format (US style)
    }

    #[test]
    fn test_parse_time_valid() {
        let time = parse_time("14:30:00");
        assert!(time.is_some());

        let time = time.unwrap();
        assert_eq!(time.hour(), 14);
        assert_eq!(time.minute(), 30);
        assert_eq!(time.second(), 0);
    }

    #[test]
    fn test_parse_time_invalid() {
        assert!(parse_time("invalid").is_none());
        assert!(parse_time("25:99:99").is_none());
        assert!(parse_time("14:30").is_none()); // Missing seconds
    }

    #[test]
    fn test_roundtrip_datetime() {
        // Create a datetime, format it, parse it back
        let tz: Tz = "America/New_York".parse().unwrap();
        let original = tz
            .with_ymd_and_hms(2023, 6, 15, 10, 30, 0)
            .unwrap();

        let formatted = format_datetime(&original);
        let parsed = to_zoned_datetime(&formatted, "America/New_York");

        assert!(parsed.is_some());
        let parsed = parsed.unwrap();
        assert_eq!(original.timestamp(), parsed.timestamp());
    }
}
