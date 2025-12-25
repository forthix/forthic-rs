// DateTime module for Forthic
//
// Date and time operations using chrono for timezone-aware datetime manipulation.
//
// ## Categories
// - Current: TODAY, NOW
// - Conversion to: >TIME, >DATE, >DATETIME, AT
// - Conversion from: TIME>STR, DATE>STR, DATE>INT
// - Timestamps: >TIMESTAMP, TIMESTAMP>DATETIME
// - Date math: ADD-DAYS, SUBTRACT-DATES

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{InterpreterContext, Module, ModuleWord};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
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

    fn word_today(_context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let today = Local::now().naive_local().date();
        _context.stack_push(ForthicValue::Date(today));
        Ok(())
    }

    fn word_now(_context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let now = Utc::now().with_timezone(&chrono_tz::UTC);
        _context.stack_push(ForthicValue::DateTime(now));
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
        let word = Arc::new(ModuleWord::new(">DATETIME".to_string(), Self::word_to_datetime));
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

    fn word_to_date(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::Date(d) => ForthicValue::Date(d),
            ForthicValue::DateTime(dt) => ForthicValue::Date(dt.naive_local().date()),
            ForthicValue::String(s) => {
                // Try to parse date string (YYYY-MM-DD)
                match NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
                    Ok(date) => ForthicValue::Date(date),
                    Err(_) => ForthicValue::Null,
                }
            }
            _ => ForthicValue::Null,
        };

        context.stack_push(result);
        Ok(())
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
        let word = Arc::new(ModuleWord::new("TIME>STR".to_string(), Self::word_time_to_str));
        module.add_exportable_word(word);

        // DATE>STR
        let word = Arc::new(ModuleWord::new("DATE>STR".to_string(), Self::word_date_to_str));
        module.add_exportable_word(word);

        // DATE>INT
        let word = Arc::new(ModuleWord::new("DATE>INT".to_string(), Self::word_date_to_int));
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
        let word = Arc::new(ModuleWord::new(">TIMESTAMP".to_string(), Self::word_to_timestamp));
        module.add_exportable_word(word);

        // TIMESTAMP>DATETIME
        let word = Arc::new(ModuleWord::new("TIMESTAMP>DATETIME".to_string(), Self::word_timestamp_to_datetime));
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

    fn word_timestamp_to_datetime(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
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

        // SUBTRACT-DATES
        let word = Arc::new(ModuleWord::new("SUBTRACT-DATES".to_string(), Self::word_subtract_dates));
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

    fn word_subtract_dates(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let date2 = context.stack_pop()?;
        let date1 = context.stack_pop()?;

        let result = match (date1, date2) {
            (ForthicValue::Date(d1), ForthicValue::Date(d2)) => {
                let duration = d1.signed_duration_since(d2);
                ForthicValue::Int(duration.num_days())
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
