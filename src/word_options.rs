//! WordOptions - Type-safe options container for module words
//!
//! # Overview
//!
//! WordOptions provides a structured way for Forthic words to accept optional
//! configuration parameters without requiring fixed parameter positions. This
//! enables flexible, extensible APIs similar to keyword arguments in other languages.
//!
//! # Usage in Forthic
//!
//! ```forthic
//! [.option_name value ...] ~> WORD
//! ```
//!
//! The `~>` operator takes an options array and a word, passing the options as
//! an additional parameter to words that support them.
//!
//! # Example in Forthic code
//!
//! ```forthic
//! [1 2 3] '2 *' [.with_key TRUE] ~> MAP
//! [10 20 30] [.comparator "-1 *"] ~> SORT
//! [[[1 2]]] [.depth 1] ~> FLATTEN
//! ```
//!
//! # Implementation Pattern
//!
//! Words can check for WordOptions on the stack and extract options:
//!
//! ```rust,ignore
//! use forthic::word_options::WordOptions;
//!
//! // Pop options if present (returns empty HashMap if not)
//! let options = WordOptions::pop_from_stack(&mut stack);
//! let with_key = options.get_bool("with_key").unwrap_or(false);
//! let depth = options.get_int("depth").unwrap_or(0);
//! ```
//!
//! # Internal Representation
//!
//! - Created from flat array: `[.key1 val1 .key2 val2]`
//! - Stored as HashMap internally for efficient lookup
//! - Keys are strings (dot-symbol with `.` already stripped)
//! - Values are ForthicValue enums

use crate::literals::ForthicValue;
use std::collections::HashMap;

/// WordOptions - Container for optional word parameters
///
/// Constructed from a flat array of key-value pairs where keys are
/// strings (dot-symbols with the leading '.' stripped) and values are any ForthicValue.
///
/// # Examples
///
/// ```
/// use forthic::word_options::WordOptions;
/// use forthic::literals::ForthicValue;
///
/// // Create from flat array: [.key1, val1, .key2, val2]
/// let flat = vec![
///     ForthicValue::String("key1".to_string()),
///     ForthicValue::Int(42),
///     ForthicValue::String("key2".to_string()),
///     ForthicValue::Bool(true),
/// ];
///
/// let opts = WordOptions::from_flat_array(&flat).unwrap();
/// assert_eq!(opts.get_int("key1"), Some(42));
/// assert_eq!(opts.get_bool("key2"), Some(true));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct WordOptions {
    options: HashMap<String, ForthicValue>,
}

impl WordOptions {
    /// Create a new empty WordOptions
    pub fn new() -> Self {
        Self {
            options: HashMap::new(),
        }
    }

    /// Create WordOptions from a flat array of key-value pairs
    ///
    /// # Arguments
    ///
    /// * `flat_array` - Slice of ForthicValues in format: [key1, val1, key2, val2, ...]
    ///   where keys must be strings
    ///
    /// # Errors
    ///
    /// Returns error string if:
    /// - Array length is odd (not key-value pairs)
    /// - Any key is not a String variant
    ///
    /// # Examples
    ///
    /// ```
    /// use forthic::word_options::WordOptions;
    /// use forthic::literals::ForthicValue;
    ///
    /// let flat = vec![
    ///     ForthicValue::String("with_key".to_string()),
    ///     ForthicValue::Bool(true),
    /// ];
    ///
    /// let opts = WordOptions::from_flat_array(&flat).unwrap();
    /// assert_eq!(opts.get_bool("with_key"), Some(true));
    /// ```
    pub fn from_flat_array(flat_array: &[ForthicValue]) -> Result<Self, String> {
        if flat_array.len() % 2 != 0 {
            return Err(format!(
                "Options must be key-value pairs (even length). Got {} elements",
                flat_array.len()
            ));
        }

        let mut options = HashMap::new();

        for i in (0..flat_array.len()).step_by(2) {
            let key = match &flat_array[i] {
                ForthicValue::String(s) => s.clone(),
                other => {
                    return Err(format!(
                        "Option key must be a string (dot-symbol). Got: {:?}",
                        other
                    ))
                }
            };

            let value = flat_array[i + 1].clone();
            options.insert(key, value);
        }

        Ok(Self { options })
    }

    /// Get an option value by key
    ///
    /// Returns None if the key doesn't exist.
    pub fn get(&self, key: &str) -> Option<&ForthicValue> {
        self.options.get(key)
    }

    /// Get an option value by key with a default
    ///
    /// Returns the value if present, otherwise returns the default.
    pub fn get_or<'a>(&'a self, key: &str, default: &'a ForthicValue) -> &'a ForthicValue {
        self.options.get(key).unwrap_or(default)
    }

    /// Check if an option exists
    pub fn has(&self, key: &str) -> bool {
        self.options.contains_key(key)
    }

    /// Get an integer option value
    ///
    /// Returns None if key doesn't exist or value is not an Int.
    ///
    /// # Examples
    ///
    /// ```
    /// use forthic::word_options::WordOptions;
    /// use forthic::literals::ForthicValue;
    ///
    /// let flat = vec![
    ///     ForthicValue::String("depth".to_string()),
    ///     ForthicValue::Int(3),
    /// ];
    ///
    /// let opts = WordOptions::from_flat_array(&flat).unwrap();
    /// assert_eq!(opts.get_int("depth"), Some(3));
    /// assert_eq!(opts.get_int("missing"), None);
    /// ```
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_int())
    }

    /// Get a float option value
    ///
    /// Returns None if key doesn't exist or value is not a Float.
    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.as_float())
    }

    /// Get a boolean option value
    ///
    /// Returns None if key doesn't exist or value is not a Bool.
    ///
    /// # Examples
    ///
    /// ```
    /// use forthic::word_options::WordOptions;
    /// use forthic::literals::ForthicValue;
    ///
    /// let flat = vec![
    ///     ForthicValue::String("with_key".to_string()),
    ///     ForthicValue::Bool(true),
    /// ];
    ///
    /// let opts = WordOptions::from_flat_array(&flat).unwrap();
    /// assert_eq!(opts.get_bool("with_key"), Some(true));
    /// ```
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    /// Get a string option value
    ///
    /// Returns None if key doesn't exist or value is not a String.
    ///
    /// # Examples
    ///
    /// ```
    /// use forthic::word_options::WordOptions;
    /// use forthic::literals::ForthicValue;
    ///
    /// let flat = vec![
    ///     ForthicValue::String("comparator".to_string()),
    ///     ForthicValue::String("-1 *".to_string()),
    /// ];
    ///
    /// let opts = WordOptions::from_flat_array(&flat).unwrap();
    /// assert_eq!(opts.get_string("comparator"), Some("-1 *"));
    /// ```
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_string())
    }

    /// Get all option keys
    pub fn keys(&self) -> Vec<&str> {
        self.options.keys().map(|s| s.as_str()).collect()
    }

    /// Get the number of options
    pub fn len(&self) -> usize {
        self.options.len()
    }

    /// Check if there are no options
    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    /// Convert to a HashMap (consuming self)
    pub fn into_map(self) -> HashMap<String, ForthicValue> {
        self.options
    }

    /// Get a reference to the internal HashMap
    pub fn as_map(&self) -> &HashMap<String, ForthicValue> {
        &self.options
    }
}

impl Default for WordOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WordOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut pairs: Vec<String> = self
            .options
            .iter()
            .map(|(k, v)| format!(".{} {:?}", k, v))
            .collect();
        pairs.sort(); // For consistent output
        write!(f, "<WordOptions: {}>", pairs.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let opts = WordOptions::new();
        assert!(opts.is_empty());
        assert_eq!(opts.len(), 0);
    }

    #[test]
    fn test_from_flat_array() {
        let flat = vec![
            ForthicValue::String("key1".to_string()),
            ForthicValue::Int(42),
            ForthicValue::String("key2".to_string()),
            ForthicValue::Bool(true),
        ];

        let opts = WordOptions::from_flat_array(&flat).unwrap();
        assert_eq!(opts.len(), 2);
        assert_eq!(opts.get_int("key1"), Some(42));
        assert_eq!(opts.get_bool("key2"), Some(true));
    }

    #[test]
    fn test_from_flat_array_odd_length() {
        let flat = vec![
            ForthicValue::String("key1".to_string()),
            ForthicValue::Int(42),
            ForthicValue::String("key2".to_string()),
        ];

        let result = WordOptions::from_flat_array(&flat);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("even length"));
    }

    #[test]
    fn test_from_flat_array_non_string_key() {
        let flat = vec![ForthicValue::Int(42), ForthicValue::Bool(true)];

        let result = WordOptions::from_flat_array(&flat);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be a string"));
    }

    #[test]
    fn test_get_methods() {
        let flat = vec![
            ForthicValue::String("int_val".to_string()),
            ForthicValue::Int(42),
            ForthicValue::String("float_val".to_string()),
            ForthicValue::Float(3.14),
            ForthicValue::String("bool_val".to_string()),
            ForthicValue::Bool(true),
            ForthicValue::String("string_val".to_string()),
            ForthicValue::String("hello".to_string()),
        ];

        let opts = WordOptions::from_flat_array(&flat).unwrap();

        assert_eq!(opts.get_int("int_val"), Some(42));
        assert_eq!(opts.get_float("float_val"), Some(3.14));
        assert_eq!(opts.get_bool("bool_val"), Some(true));
        assert_eq!(opts.get_string("string_val"), Some("hello"));

        // Wrong type returns None
        assert_eq!(opts.get_int("float_val"), None);
        assert_eq!(opts.get_string("int_val"), None);

        // Missing key returns None
        assert_eq!(opts.get_int("missing"), None);
    }

    #[test]
    fn test_has() {
        let flat = vec![
            ForthicValue::String("key1".to_string()),
            ForthicValue::Int(42),
        ];

        let opts = WordOptions::from_flat_array(&flat).unwrap();
        assert!(opts.has("key1"));
        assert!(!opts.has("missing"));
    }

    #[test]
    fn test_get_or() {
        let flat = vec![
            ForthicValue::String("key1".to_string()),
            ForthicValue::Int(42),
        ];

        let opts = WordOptions::from_flat_array(&flat).unwrap();
        let default = ForthicValue::Int(99);

        assert_eq!(opts.get_or("key1", &default), &ForthicValue::Int(42));
        assert_eq!(opts.get_or("missing", &default), &ForthicValue::Int(99));
    }

    #[test]
    fn test_keys() {
        let flat = vec![
            ForthicValue::String("z_key".to_string()),
            ForthicValue::Int(1),
            ForthicValue::String("a_key".to_string()),
            ForthicValue::Int(2),
        ];

        let opts = WordOptions::from_flat_array(&flat).unwrap();
        let mut keys = opts.keys();
        keys.sort();

        assert_eq!(keys, vec!["a_key", "z_key"]);
    }

    #[test]
    fn test_display() {
        let flat = vec![
            ForthicValue::String("key1".to_string()),
            ForthicValue::Int(42),
            ForthicValue::String("key2".to_string()),
            ForthicValue::Bool(true),
        ];

        let opts = WordOptions::from_flat_array(&flat).unwrap();
        let display = format!("{}", opts);

        assert!(display.starts_with("<WordOptions:"));
        assert!(display.contains(".key1"));
        assert!(display.contains(".key2"));
    }

    #[test]
    fn test_into_map() {
        let flat = vec![
            ForthicValue::String("key1".to_string()),
            ForthicValue::Int(42),
        ];

        let opts = WordOptions::from_flat_array(&flat).unwrap();
        let map = opts.into_map();

        assert_eq!(map.get("key1"), Some(&ForthicValue::Int(42)));
    }

    #[test]
    fn test_empty_array() {
        let flat: Vec<ForthicValue> = vec![];
        let opts = WordOptions::from_flat_array(&flat).unwrap();

        assert!(opts.is_empty());
        assert_eq!(opts.len(), 0);
    }
}
