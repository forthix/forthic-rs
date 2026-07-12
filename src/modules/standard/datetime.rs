// DateTime module for Forthic
//
// Date and time operations using chrono for timezone-aware datetime manipulation.
//
// ## Categories
// - Current: TODAY, NOW
// - Conversion to: >TIME, >DATE, >DATETIME, AT
// - Conversion from: TIME>STR, DATE>STR, DATE>INT
// - Timestamps: >TIMESTAMP, TIMESTAMP>DATETIME
// - Date math: ADD-DAYS, DAYS-BETWEEN
// - Components: YEAR, MONTH (1-based), DAY-OF-WEEK (ISO 1=Mon)
// - Meridiem: AM, PM

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{register_words, InterpreterContext, Module, ModuleWord};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Timelike, Utc};
use std::sync::Arc;

/// DateTimeModule provides date and time operations
pub struct DateTimeModule {
    module: Module,
}

impl DateTimeModule {
    /// Create a new DateTimeModule
    pub fn new() -> Self {
        let mut module = Module::new("datetime".to_string());

        // Register all words
        Self::register_current_words(&mut module);
        Self::register_conversion_to_words(&mut module);
        Self::register_conversion_from_words(&mut module);
        Self::register_timestamp_words(&mut module);
        Self::register_date_math_words(&mut module);
        Self::register_meridiem_words(&mut module);
        Self::register_component_words(&mut module);

        Self { module }
    }

    /// Get the underlying module
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Get a mutable reference to the underlying module
    pub fn module_mut(&mut self) -> &mut Module {
        &mut self.module
    }

    // ===== Current Date/Time Operations =====

    fn register_current_words(module: &mut Module) {
        // TODAY
        let word = Arc::new(ModuleWord::new("TODAY".to_string(), Self::word_today));
        module.add_exportable_word(word);

        // NOW
        let word = Arc::new(ModuleWord::new("NOW".to_string(), Self::word_now));
        module.add_exportable_word(word);
    }

    /// The interpreter's configured timezone (falls back to UTC on an
    /// unparseable name). NOW and TODAY must use the same source, or they
    /// can disagree on what day it is.
    fn context_tz(context: &dyn InterpreterContext) -> chrono_tz::Tz {
        context.get_timezone().parse().unwrap_or(chrono_tz::UTC)
    }

    fn word_today(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let today = Utc::now()
            .with_timezone(&Self::context_tz(context))
            .date_naive();
        context.stack_push(ForthicValue::Date(today));
        Ok(())
    }

    fn word_now(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let now = Utc::now().with_timezone(&Self::context_tz(context));
        context.stack_push(ForthicValue::DateTime(now));
        Ok(())
    }

    // ===== Conversion To Date/Time =====

    fn register_conversion_to_words(module: &mut Module) {
        // >TIME
        let word = Arc::new(ModuleWord::new(">TIME".to_string(), Self::word_to_time));
        module.add_exportable_word(word);

        // >DATE
        let word = Arc::new(ModuleWord::new(">DATE".to_string(), Self::word_to_date));
        module.add_exportable_word(word);

        // >DATETIME
        let word = Arc::new(ModuleWord::new(
            ">DATETIME".to_string(),
            Self::word_to_datetime,
        ));
        module.add_exportable_word(word);

        // AT
        let word = Arc::new(ModuleWord::new("AT".to_string(), Self::word_at));
        module.add_exportable_word(word);
    }

    fn word_to_time(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::Time(t) => ForthicValue::Time(t),
            ForthicValue::DateTime(dt) => ForthicValue::Time(dt.time()),
            ForthicValue::String(s) => {
                // Try to parse time string (HH:MM or HH:MM:SS or with AM/PM)
                Self::parse_time_string(&s).unwrap_or(ForthicValue::Null)
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    /// >DATE: ( value -- date ) — ts #35 contract:
    /// - Date passes through; DateTime takes its OWN timezone's calendar
    ///   date
    /// - ISO date strings, and ISO datetime strings with NO zone or with
    ///   an explicit numeric OFFSET, take the date AS WRITTEN
    /// - a trailing-Z instant resolves in the INTERPRETER's timezone
    ///   (never the host's)
    /// - "Oct 21, 2020"-style month names parse; ts's arbitrary
    ///   new Date() leniency beyond that is NOT reproduced (sanctioned
    ///   strict-parsing divergence)
    /// - anything unparseable (and any non-date/-string value) is NULL
    fn word_to_date(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::Date(d) => ForthicValue::Date(d),
            ForthicValue::DateTime(dt) => ForthicValue::Date(dt.date_naive()),
            ForthicValue::String(s) => Self::parse_date_string(s.trim(), Self::context_tz(context))
                .map(ForthicValue::Date)
                .unwrap_or(ForthicValue::Null),
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    fn parse_date_string(s: &str, tz: chrono_tz::Tz) -> Option<NaiveDate> {
        // ISO date as written
        if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            return Some(d);
        }
        // ISO datetime without zone: date as written
        for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M"] {
            if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
                return Some(ndt.date());
            }
        }
        // ISO datetime with a zone: a trailing-Z INSTANT resolves in the
        // interpreter timezone (the #35 rule); an explicit numeric offset
        // takes the date as written (offset ignored — ts PlainDate.from)
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return if s.ends_with('Z') || s.ends_with('z') {
                Some(dt.with_timezone(&tz).date_naive())
            } else {
                Some(dt.naive_local().date())
            };
        }
        // Month-name forms ("Oct 21, 2020" is test-pinned in ts)
        for fmt in ["%b %d, %Y", "%B %d, %Y"] {
            if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
                return Some(d);
            }
        }
        None
    }

    fn word_to_datetime(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::DateTime(dt) => ForthicValue::DateTime(dt),
            ForthicValue::Int(timestamp) => {
                // Treat as Unix timestamp (seconds)
                if let Some(dt) = DateTime::from_timestamp(timestamp, 0) {
                    let utc_dt = dt.with_timezone(&chrono_tz::UTC);
                    ForthicValue::DateTime(utc_dt)
                } else {
                    ForthicValue::Null
                }
            }
            ForthicValue::String(s) => {
                // Try to parse datetime string
                Self::parse_datetime_string(&s).unwrap_or(ForthicValue::Null)
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_at(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let time = context.stack_pop()?;
        let date = context.stack_pop()?;

        let result = match (date, time) {
            (ForthicValue::Date(d), ForthicValue::Time(t)) => {
                // Combine date and time into datetime
                let dt = d.and_time(t);
                let zoned = Utc.from_local_datetime(&dt).single();
                if let Some(zdt) = zoned {
                    ForthicValue::DateTime(zdt.with_timezone(&chrono_tz::UTC))
                } else {
                    ForthicValue::Null
                }
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Conversion From Date/Time =====

    fn register_conversion_from_words(module: &mut Module) {
        // TIME>STR
        let word = Arc::new(ModuleWord::new(
            "TIME>STR".to_string(),
            Self::word_time_to_str,
        ));
        module.add_exportable_word(word);

        // DATE>STR
        let word = Arc::new(ModuleWord::new(
            "DATE>STR".to_string(),
            Self::word_date_to_str,
        ));
        module.add_exportable_word(word);

        // DATE>INT
        let word = Arc::new(ModuleWord::new(
            "DATE>INT".to_string(),
            Self::word_date_to_int,
        ));
        module.add_exportable_word(word);
    }

    fn word_time_to_str(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::Time(t) => ForthicValue::String(t.format("%H:%M").to_string()),
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_date_to_str(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::Date(d) => ForthicValue::String(d.format("%Y-%m-%d").to_string()),
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_date_to_int(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::Date(d) => {
                let year = d.year();
                let month = d.month();
                let day = d.day();
                let int_val = year * 10000 + (month as i32) * 100 + day as i32;
                ForthicValue::Int(int_val as i64)
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Timestamp Operations =====

    fn register_timestamp_words(module: &mut Module) {
        // >TIMESTAMP
        let word = Arc::new(ModuleWord::new(
            ">TIMESTAMP".to_string(),
            Self::word_to_timestamp,
        ));
        module.add_exportable_word(word);

        // TIMESTAMP>DATETIME
        let word = Arc::new(ModuleWord::new(
            "TIMESTAMP>DATETIME".to_string(),
            Self::word_timestamp_to_datetime,
        ));
        module.add_exportable_word(word);
    }

    fn word_to_timestamp(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::DateTime(dt) => ForthicValue::Int(dt.timestamp()),
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_timestamp_to_datetime(
        context: &mut dyn InterpreterContext,
    ) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::Int(timestamp) => {
                if let Some(dt) = DateTime::from_timestamp(timestamp, 0) {
                    let utc_dt = dt.with_timezone(&chrono_tz::UTC);
                    ForthicValue::DateTime(utc_dt)
                } else {
                    ForthicValue::Null
                }
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Date Math Operations =====

    fn register_date_math_words(module: &mut Module) {
        // ADD-DAYS
        let word = Arc::new(ModuleWord::new("ADD-DAYS".to_string(), Self::word_add_days));
        module.add_exportable_word(word);

        // DAYS-BETWEEN
        let word = Arc::new(ModuleWord::new(
            "DAYS-BETWEEN".to_string(),
            Self::word_days_between,
        ));
        module.add_exportable_word(word);
    }

    fn word_add_days(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let num_days = context.stack_pop()?;
        let date = context.stack_pop()?;

        let result = match (date, num_days) {
            (ForthicValue::Date(d), ForthicValue::Int(days)) => {
                if let Some(new_date) = d.checked_add_signed(Duration::days(days)) {
                    ForthicValue::Date(new_date)
                } else {
                    ForthicValue::Null
                }
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    /// DAYS-BETWEEN: ( date1 date2 -- date1 - date2 in days ) — replaces
    /// classic SUBTRACT-DATES with IDENTICAL semantics (same sign
    /// convention: the top of stack is subtracted from the value beneath).
    /// DateTime operands use their own-timezone calendar date; anything
    /// else is NULL.
    fn word_days_between(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let date2 = context.stack_pop()?;
        let date1 = context.stack_pop()?;

        let result = match (
            Self::as_calendar_date(&date1),
            Self::as_calendar_date(&date2),
        ) {
            (Some(d1), Some(d2)) => ForthicValue::Int(d1.signed_duration_since(d2).num_days()),
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
    }

    /// The calendar date of a Date or DateTime (in the DateTime's own
    /// timezone — mirrors ts duck typing on .year)
    fn as_calendar_date(val: &ForthicValue) -> Option<NaiveDate> {
        match val {
            ForthicValue::Date(d) => Some(*d),
            ForthicValue::DateTime(dt) => Some(dt.date_naive()),
            _ => None,
        }
    }

    // ===== Meridiem Operations =====

    fn register_meridiem_words(module: &mut Module) {
        register_words!(module, {
            "AM" => Self::word_am,
            "PM" => Self::word_pm,
        });
    }

    /// AM: ( time -- time ) — force into the morning: hour >= 12 loses 12
    /// (14:30 -> 02:30, 12:00 -> 00:00). Works on Time and DateTime (ts
    /// duck-types on .hour); anything else passes through UNCHANGED (not
    /// NULL — ts returns the input itself).
    fn word_am(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        context.stack_push(Self::with_meridiem(val, false));
        Ok(())
    }

    /// PM: ( time -- time ) — force into the afternoon: hour < 12 gains 12
    /// (09:15 -> 21:15, 00:00 -> 12:00). Same pass-through rule as AM.
    fn word_pm(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        context.stack_push(Self::with_meridiem(val, true));
        Ok(())
    }

    fn with_meridiem(val: ForthicValue, pm: bool) -> ForthicValue {
        // .then (lazy), not .then_some — hour - 12 must not be evaluated
        // when hour < 12 (u32 underflow)
        let adjust = |hour: u32| -> Option<u32> {
            if pm {
                (hour < 12).then(|| hour + 12)
            } else {
                (hour >= 12).then(|| hour - 12)
            }
        };
        match val {
            ForthicValue::Time(t) => match adjust(t.hour()) {
                Some(h) => ForthicValue::Time(t.with_hour(h).unwrap_or(t)),
                None => ForthicValue::Time(t),
            },
            ForthicValue::DateTime(dt) => match adjust(dt.hour()) {
                Some(h) => ForthicValue::DateTime(dt.with_hour(h).unwrap_or(dt)),
                None => ForthicValue::DateTime(dt),
            },
            other => other, // pass through unchanged
        }
    }

    // ===== Date Components =====

    fn register_component_words(module: &mut Module) {
        register_words!(module, {
            "YEAR" => Self::word_year,
            "MONTH" => Self::word_month,
            "DAY-OF-WEEK" => Self::word_day_of_week,
        });
    }

    /// YEAR: ( date -- year ) — Date or DateTime; anything else NULL
    fn word_year(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        let result = match &val {
            ForthicValue::Date(d) => ForthicValue::Int(d.year() as i64),
            ForthicValue::DateTime(dt) => ForthicValue::Int(dt.year() as i64),
            _ => ForthicValue::Null,
        };
        context.stack_push(result);
        Ok(())
    }

    /// MONTH: ( date -- month ) — 1-based (1=January), matching both
    /// Temporal .month and chrono .month()
    fn word_month(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        let result = match &val {
            ForthicValue::Date(d) => ForthicValue::Int(d.month() as i64),
            ForthicValue::DateTime(dt) => ForthicValue::Int(dt.month() as i64),
            _ => ForthicValue::Null,
        };
        context.stack_push(result);
        Ok(())
    }

    /// DAY-OF-WEEK: ( date -- day ) — ISO 8601: 1=Monday .. 7=Sunday
    /// (number_from_monday, NOT the 0-based num_days_from_monday)
    fn word_day_of_week(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        let result = match &val {
            ForthicValue::Date(d) => ForthicValue::Int(d.weekday().number_from_monday() as i64),
            ForthicValue::DateTime(dt) => {
                ForthicValue::Int(dt.weekday().number_from_monday() as i64)
            }
            _ => ForthicValue::Null,
        };
        context.stack_push(result);
        Ok(())
    }

    // ===== Helper Functions =====

    /// Parse time string (HH:MM, HH:MM:SS, or with AM/PM)
    fn parse_time_string(s: &str) -> Option<ForthicValue> {
        let s = s.trim();

        // Try HH:MM AM/PM format
        if let Some(captures) = regex::Regex::new(r"^(\d{1,2}):(\d{2})\s*(AM|PM)$")
            .ok()?
            .captures(s)
        {
            let hour: u32 = captures.get(1)?.as_str().parse().ok()?;
            let minute: u32 = captures.get(2)?.as_str().parse().ok()?;
            let meridiem = captures.get(3)?.as_str();

            let hour = if meridiem == "PM" && hour < 12 {
                hour + 12
            } else if meridiem == "AM" && hour == 12 {
                0
            } else {
                hour
            };

            return NaiveTime::from_hms_opt(hour, minute, 0).map(ForthicValue::Time);
        }

        // Try standard formats
        NaiveTime::parse_from_str(s, "%H:%M:%S")
            .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
            .ok()
            .map(ForthicValue::Time)
    }

    /// Parse datetime string
    fn parse_datetime_string(s: &str) -> Option<ForthicValue> {
        let s = s.trim();

        // Try parsing with chrono
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return Some(ForthicValue::DateTime(dt.with_timezone(&chrono_tz::UTC)));
        }

        // Try parsing as naive datetime and assume UTC
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
            if let Some(dt) = Utc.from_local_datetime(&naive).single() {
                return Some(ForthicValue::DateTime(dt.with_timezone(&chrono_tz::UTC)));
            }
        }

        None
    }
}

impl Default for DateTimeModule {
    fn default() -> Self {
        Self::new()
    }
}
