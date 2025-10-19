//! Error types for the Forthic interpreter
//!
//! This module provides comprehensive error handling for Forthic code execution,
//! including detailed location tracking and formatted error messages.

use thiserror::Error;

/// Code location information for error reporting
///
/// Tracks where in the source code an error occurred, including
/// line, column, and character positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLocation {
    /// Optional source identifier (e.g., module name, file path)
    pub source: Option<String>,
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
    /// Starting character position (0-indexed)
    pub start_pos: usize,
    /// Optional ending character position (0-indexed)
    pub end_pos: Option<usize>,
}

impl Default for CodeLocation {
    fn default() -> Self {
        Self {
            source: None,
            line: 1,
            column: 1,
            start_pos: 0,
            end_pos: None,
        }
    }
}

impl CodeLocation {
    /// Create a new code location
    pub fn new(line: usize, column: usize, start_pos: usize) -> Self {
        Self {
            source: None,
            line,
            column,
            start_pos,
            end_pos: None,
        }
    }

    /// Create a code location with source information
    pub fn with_source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }

    /// Set the end position
    pub fn with_end_pos(mut self, end_pos: usize) -> Self {
        self.end_pos = Some(end_pos);
        self
    }
}

/// Main error type for Forthic interpreter errors
#[derive(Error, Debug)]
pub enum ForthicError {
    /// Unknown word encountered during execution
    #[error("Unknown word: {word}")]
    UnknownWord {
        /// The Forthic code being executed
        forthic: String,
        /// The unknown word that was encountered
        word: String,
        /// Location where the error occurred
        location: Option<CodeLocation>,
        #[source]
        /// Optional underlying cause
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Error during word execution
    #[error("{message}")]
    WordExecution {
        /// Error message
        message: String,
        /// The inner error that occurred
        #[source]
        inner_error: Box<dyn std::error::Error + Send + Sync>,
        /// Location where the word was called
        call_location: Option<CodeLocation>,
        /// Location where the word was defined
        definition_location: Option<CodeLocation>,
    },

    /// Missing semicolon in word definition
    #[error("Missing semicolon")]
    MissingSemicolon {
        forthic: String,
        location: Option<CodeLocation>,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Extra semicolon encountered
    #[error("Extra semicolon")]
    ExtraSemicolon {
        forthic: String,
        location: Option<CodeLocation>,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Stack underflow (attempted to pop from empty stack)
    #[error("Stack underflow")]
    StackUnderflow {
        forthic: String,
        location: Option<CodeLocation>,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Invalid variable name
    #[error("Invalid variable name: {varname}")]
    InvalidVariableName {
        forthic: String,
        varname: String,
        location: Option<CodeLocation>,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Unknown module
    #[error("Unknown module: {module_name}")]
    UnknownModule {
        forthic: String,
        module_name: String,
        location: Option<CodeLocation>,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Invalid input position
    #[error("Invalid input position")]
    InvalidInputPosition {
        forthic: String,
        location: Option<CodeLocation>,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Invalid word name
    #[error("Invalid word name")]
    InvalidWordName {
        forthic: String,
        note: Option<String>,
        location: Option<CodeLocation>,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Unterminated string literal
    #[error("Unterminated string")]
    UnterminatedString {
        forthic: String,
        location: Option<CodeLocation>,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Unknown token type
    #[error("Unknown type of token: {token}")]
    UnknownToken {
        forthic: String,
        token: String,
        location: Option<CodeLocation>,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Error in a module
    #[error("Error in module {module_name}: {inner_message}")]
    Module {
        forthic: String,
        module_name: String,
        inner_message: String,
        #[source]
        inner_error: Box<dyn std::error::Error + Send + Sync>,
        location: Option<CodeLocation>,
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Too many recovery attempts
    #[error("Too many recovery attempts: {num_attempts} of {max_attempts}")]
    TooManyAttempts {
        forthic: String,
        num_attempts: usize,
        max_attempts: usize,
        location: Option<CodeLocation>,
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Intentional stop (not an error, used for control flow)
    #[error("Intentional stop: {message}")]
    IntentionalStop {
        message: String,
    },
}

impl ForthicError {
    /// Get the Forthic code associated with this error
    pub fn get_forthic(&self) -> Option<&str> {
        match self {
            Self::UnknownWord { forthic, .. }
            | Self::MissingSemicolon { forthic, .. }
            | Self::ExtraSemicolon { forthic, .. }
            | Self::StackUnderflow { forthic, .. }
            | Self::InvalidVariableName { forthic, .. }
            | Self::UnknownModule { forthic, .. }
            | Self::InvalidInputPosition { forthic, .. }
            | Self::InvalidWordName { forthic, .. }
            | Self::UnterminatedString { forthic, .. }
            | Self::UnknownToken { forthic, .. }
            | Self::Module { forthic, .. }
            | Self::TooManyAttempts { forthic, .. } => Some(forthic),
            Self::WordExecution { .. } | Self::IntentionalStop { .. } => None,
        }
    }

    /// Get the code location associated with this error
    pub fn get_location(&self) -> Option<&CodeLocation> {
        match self {
            Self::UnknownWord { location, .. }
            | Self::MissingSemicolon { location, .. }
            | Self::ExtraSemicolon { location, .. }
            | Self::StackUnderflow { location, .. }
            | Self::InvalidVariableName { location, .. }
            | Self::UnknownModule { location, .. }
            | Self::InvalidInputPosition { location, .. }
            | Self::InvalidWordName { location, .. }
            | Self::UnterminatedString { location, .. }
            | Self::UnknownToken { location, .. }
            | Self::Module { location, .. }
            | Self::TooManyAttempts { location, .. } => location.as_ref(),
            Self::WordExecution { call_location, .. } => call_location.as_ref(),
            Self::IntentionalStop { .. } => None,
        }
    }

    /// Get a formatted error description with code context
    pub fn format_with_context(&self) -> String {
        // Get the forthic code and location
        let forthic = match self.get_forthic() {
            Some(f) if !f.is_empty() => f,
            _ => return self.to_string(),
        };

        let location = match self.get_location() {
            Some(loc) => loc,
            None => return self.to_string(),
        };

        // Handle WordExecutionError specially (shows both definition and call locations)
        if let Self::WordExecution {
            message,
            call_location,
            definition_location: Some(def_loc),
            ..
        } = self
        {
            return format_word_execution_error(
                message,
                forthic,
                call_location.as_ref(),
                def_loc,
            );
        }

        // Standard error format
        format_standard_error(&self.to_string(), forthic, location)
    }
}

/// Format a standard error with code context
fn format_standard_error(message: &str, forthic: &str, location: &CodeLocation) -> String {
    let lines: Vec<&str> = forthic.split('\n').collect();
    let line_num = location.line;

    // Get the lines up to and including the error line
    let context_lines: Vec<String> = lines
        .iter()
        .take(line_num)
        .map(|line| (*line).to_string())
        .collect();

    // Create the error indicator line (spaces + carets)
    let end_pos = location.end_pos.unwrap_or(location.start_pos + 1);
    let error_indicator = " ".repeat(location.column.saturating_sub(1))
        + &"^".repeat((end_pos - location.start_pos).max(1));

    // Build location info
    let mut location_info = format!("at line {}", line_num);
    if let Some(ref source) = location.source {
        location_info.push_str(&format!(" in {}", source));
    }

    // Format the error message
    format!(
        "{} {}:\n```\n{}\n{}\n```",
        message,
        location_info,
        context_lines.join("\n"),
        error_indicator
    )
}

/// Format a word execution error (shows both definition and call locations)
fn format_word_execution_error(
    message: &str,
    forthic: &str,
    call_location: Option<&CodeLocation>,
    def_location: &CodeLocation,
) -> String {
    let lines: Vec<&str> = forthic.split('\n').collect();

    // Format definition location
    let def_line_num = def_location.line;
    let def_context_lines: Vec<String> = lines
        .iter()
        .take(def_line_num)
        .map(|line| (*line).to_string())
        .collect();

    let def_end_pos = def_location.end_pos.unwrap_or(def_location.start_pos + 1);
    let def_error_indicator = " ".repeat(def_location.column.saturating_sub(1))
        + &"^".repeat((def_end_pos - def_location.start_pos).max(1));

    let mut def_location_info = format!("at line {}", def_line_num);
    if let Some(ref source) = def_location.source {
        def_location_info.push_str(&format!(" in {}", source));
    }

    // Format call location if available
    let call_info = if let Some(call_loc) = call_location {
        let call_line_num = call_loc.line;
        let call_context_lines: Vec<String> = lines
            .iter()
            .take(call_line_num)
            .map(|line| (*line).to_string())
            .collect();

        let call_end_pos = call_loc.end_pos.unwrap_or(call_loc.start_pos + 1);
        let call_error_indicator = " ".repeat(call_loc.column.saturating_sub(1))
            + &"^".repeat((call_end_pos - call_loc.start_pos).max(1));

        let mut call_location_info = format!("line {}", call_line_num);
        if let Some(ref source) = call_loc.source {
            call_location_info.push_str(&format!(" in {}", source));
        }

        format!(
            "\nCalled from {}:\n```\n{}\n{}\n```",
            call_location_info,
            call_context_lines.join("\n"),
            call_error_indicator
        )
    } else {
        String::new()
    };

    // Combine everything
    format!(
        "{} {}:\n```\n{}\n{}\n```{}",
        message,
        def_location_info,
        def_context_lines.join("\n"),
        def_error_indicator,
        call_info
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_location_default() {
        let loc = CodeLocation::default();
        assert_eq!(loc.line, 1);
        assert_eq!(loc.column, 1);
        assert_eq!(loc.start_pos, 0);
        assert_eq!(loc.end_pos, None);
        assert_eq!(loc.source, None);
    }

    #[test]
    fn test_code_location_builder() {
        let loc = CodeLocation::new(10, 5, 42)
            .with_source("test.forthic".to_string())
            .with_end_pos(50);

        assert_eq!(loc.line, 10);
        assert_eq!(loc.column, 5);
        assert_eq!(loc.start_pos, 42);
        assert_eq!(loc.end_pos, Some(50));
        assert_eq!(loc.source, Some("test.forthic".to_string()));
    }

    #[test]
    fn test_unknown_word_error() {
        let error = ForthicError::UnknownWord {
            forthic: "DUP GARBAGE SWAP".to_string(),
            word: "GARBAGE".to_string(),
            location: Some(CodeLocation::new(1, 5, 4).with_end_pos(11)),
            cause: None,
        };

        assert_eq!(error.get_forthic(), Some("DUP GARBAGE SWAP"));
        assert!(error.to_string().contains("GARBAGE"));
    }

    #[test]
    fn test_stack_underflow_error() {
        let error = ForthicError::StackUnderflow {
            forthic: "POP".to_string(),
            location: Some(CodeLocation::default()),
            cause: None,
        };

        assert!(error.to_string().contains("Stack underflow"));
    }

    #[test]
    fn test_format_with_context() {
        let forthic = "DUP GARBAGE SWAP";
        let error = ForthicError::UnknownWord {
            forthic: forthic.to_string(),
            word: "GARBAGE".to_string(),
            location: Some(CodeLocation::new(1, 5, 4).with_end_pos(11)),
            cause: None,
        };

        let formatted = error.format_with_context();
        assert!(formatted.contains("Unknown word"));
        assert!(formatted.contains("GARBAGE"));
        assert!(formatted.contains("at line 1"));
        assert!(formatted.contains("^^^"));
    }

    #[test]
    fn test_format_multiline_error() {
        let forthic = "DUP\nGARBAGE\nSWAP";
        let error = ForthicError::UnknownWord {
            forthic: forthic.to_string(),
            word: "GARBAGE".to_string(),
            location: Some(CodeLocation::new(2, 1, 4).with_end_pos(11)),
            cause: None,
        };

        let formatted = error.format_with_context();
        assert!(formatted.contains("at line 2"));
        assert!(formatted.contains("DUP"));
        assert!(formatted.contains("GARBAGE"));
    }

    #[test]
    fn test_error_without_location() {
        let error = ForthicError::UnknownWord {
            forthic: "DUP".to_string(),
            word: "DUP".to_string(),
            location: None,
            cause: None,
        };

        let formatted = error.format_with_context();
        // Should just return the basic error message
        assert!(formatted.contains("Unknown word"));
    }

    #[test]
    fn test_intentional_stop_error() {
        let error = ForthicError::IntentionalStop {
            message: "User requested stop".to_string(),
        };

        assert!(error.to_string().contains("Intentional stop"));
        assert_eq!(error.get_forthic(), None);
        assert_eq!(error.get_location(), None);
    }

    #[test]
    fn test_invalid_variable_name_error() {
        let error = ForthicError::InvalidVariableName {
            forthic: "123 !".to_string(),
            varname: "123".to_string(),
            location: Some(CodeLocation::default()),
            cause: None,
        };

        assert!(error.to_string().contains("Invalid variable name"));
        assert!(error.to_string().contains("123"));
    }

    #[test]
    fn test_too_many_attempts_error() {
        let error = ForthicError::TooManyAttempts {
            forthic: "code".to_string(),
            num_attempts: 10,
            max_attempts: 5,
            location: None,
            cause: None,
        };

        let msg = error.to_string();
        assert!(msg.contains("Too many recovery attempts"));
        assert!(msg.contains("10"));
        assert!(msg.contains("5"));
    }
}
