// String module for Forthic
//
// String manipulation and processing operations with URL encoding support.
//
// ## Categories
// - Conversion: >STR, URL-ENCODE, URL-DECODE
// - Transform: LOWERCASE, UPPERCASE, STRIP, ASCII
// - Split/Join: SPLIT, JOIN, CONCAT
// - Pattern: REPLACE
// - Constants: /N, /R, /T

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{register_words, InterpreterContext, Module};
use regex::Regex;

/// StringModule provides string manipulation operations
pub struct StringModule {
    module: Module,
}

impl StringModule {
    /// Create a new StringModule
    pub fn new() -> Self {
        let mut module = Module::new("string".to_string());

        // Register all words
        Self::register_conversion_words(&mut module);
        Self::register_transform_words(&mut module);
        Self::register_split_join_words(&mut module);
        Self::register_pattern_words(&mut module);
        Self::register_substring_words(&mut module);
        Self::register_regex_words(&mut module);
        Self::register_shell_words(&mut module);
        Self::register_constant_words(&mut module);

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

    // ===== Conversion Operations =====

    fn register_conversion_words(module: &mut Module) {
        register_words!(module, {
            ">STR" => Self::word_to_str,
                "( item:any -- string:string )",
                "Convert item to string. Records render as JSON; arrays comma-join their stringified elements.";
            "URL-ENCODE" => Self::word_url_encode,
                "( str:string -- encoded:string )",
                "Percent-encode a string for use in URLs";
            "URL-DECODE" => Self::word_url_decode,
                "( str:string -- decoded:string )",
                "Decode a percent-encoded URL string";
        });
    }

    fn word_to_str(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        context.stack_push(ForthicValue::String(Self::stringify(&val)));
        Ok(())
    }

    /// `>STR` stringification, mirrored byte-for-byte with ts (see ts
    /// string_module's stringifyValue):
    /// - null -> ""
    /// - records render as insertion-ordered JSON (not "[object Object]")
    /// - arrays comma-join their recursively stringified elements, with
    ///   null elements as empty strings (JS Array.prototype.toString) —
    ///   record elements render as JSON
    /// - temporal values use their ISO forms (Temporal toString)
    pub(crate) fn stringify(val: &ForthicValue) -> String {
        match val {
            ForthicValue::Null => String::new(),
            ForthicValue::String(s) => s.clone(),
            // Rust and JS agree here: 3.0 prints as "3", 3.25 as "3.25"
            ForthicValue::Int(i) => i.to_string(),
            ForthicValue::Float(f) => f.to_string(),
            ForthicValue::Bool(b) => b.to_string(),
            ForthicValue::Array(arr) => arr
                .iter()
                .map(Self::stringify)
                .collect::<Vec<_>>()
                .join(","),
            ForthicValue::Record(_) => {
                // Same rendering as >JSON, so >STR and >JSON agree
                crate::modules::standard::json::JSONModule::forthic_to_json(val).to_string()
            }
            ForthicValue::Date(d) => d.format("%Y-%m-%d").to_string(),
            ForthicValue::Time(t) => t.format("%H:%M:%S%.f").to_string(),
            ForthicValue::DateTime(dt) => {
                let tz_name = dt.timezone().name();
                format!(
                    "{}[{}]",
                    dt.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, false),
                    tz_name
                )
            }
            other => format!("{other:?}"),
        }
    }

    fn word_url_encode(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => {
                let encoded = urlencoding::encode(&s).to_string();
                ForthicValue::String(encoded)
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_url_decode(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => {
                let decoded = urlencoding::decode(&s).unwrap_or_default().to_string();
                ForthicValue::String(decoded)
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Transform Operations =====

    fn register_transform_words(module: &mut Module) {
        register_words!(module, {
            "LOWERCASE" => Self::word_lowercase,
                "( string:string -- result:string )",
                "Convert string to lowercase";
            "UPPERCASE" => Self::word_uppercase,
                "( string:string -- result:string )",
                "Convert string to uppercase";
            "STRIP" => Self::word_strip,
                "( string:string -- result:string )",
                "Trim whitespace from string";
            "ASCII" => Self::word_ascii,
                "( string:string -- result:string )",
                "Keep only characters with code points below 256";
        });
    }

    fn word_lowercase(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => ForthicValue::String(s.to_lowercase()),
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_uppercase(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => ForthicValue::String(s.to_uppercase()),
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_strip(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => ForthicValue::String(s.trim().to_string()),
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_ascii(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::String(s) => {
                let ascii: String = s.chars().filter(|c| (*c as u32) < 256).collect();
                ForthicValue::String(ascii)
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Split/Join Operations =====

    fn register_split_join_words(module: &mut Module) {
        register_words!(module, {
            "SPLIT" => Self::word_split,
                "( string:string sep:string -- items:any[] )",
                "Split string by separator";
            "JOIN" => Self::word_join,
                "( strings:string[] sep:string -- result:string )",
                "Join strings with separator (non-string elements are skipped)";
            "CONCAT" => Self::word_concat,
                "( strings:string[] -- result:string )",
                "Concatenate an array of strings into one string. For two strings: write [s1 s2] CONCAT. For arrays of arrays, use FLATTEN.";
        });
    }

    fn word_split(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let sep = context.stack_pop()?;
        let string = context.stack_pop()?;

        let result = match (string, sep) {
            (ForthicValue::String(s), ForthicValue::String(sep_str)) => {
                let parts: Vec<_> = s
                    .split(&sep_str as &str)
                    .map(|p| ForthicValue::String(p.to_string()))
                    .collect();
                ForthicValue::Array(parts)
            }
            _ => ForthicValue::Array(vec![]),
        };

        context.stack_push(result);
        Ok(())
    }

    fn word_join(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let sep = context.stack_pop()?;
        let strings = context.stack_pop()?;

        let result = match (strings, sep) {
            (ForthicValue::Array(arr), ForthicValue::String(sep_str)) => {
                let parts: Vec<String> = arr
                    .iter()
                    .filter_map(|v| match v {
                        ForthicValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect();
                ForthicValue::String(parts.join(&sep_str))
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    /// CONCAT: ( strings[] -- str ) — one argument, always (ts contract).
    /// The old two-string fallback popped a DIFFERENT number of stack items
    /// depending on argument type — arity instability ts deliberately
    /// removed. Two strings: `[ s1 s2 ] CONCAT`.
    fn word_concat(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        let result = match val {
            ForthicValue::Array(arr) => {
                let parts: Vec<String> = arr
                    .iter()
                    .map(|v| match v {
                        ForthicValue::Null => String::new(),
                        other => Self::stringify(other),
                    })
                    .collect();
                ForthicValue::String(parts.join(""))
            }
            other => {
                return Err(ForthicError::InvalidOperation {
                    forthic: String::new(),
                    message: format!(
                        "CONCAT requires an array of strings (got {other:?}). Wrap two strings as [s1 s2] CONCAT."
                    ),
                    location: None,
                    cause: None,
                })
            }
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Substrings & Affixes =====

    fn register_substring_words(module: &mut Module) {
        // Indices are CHAR (code point) indices — the sanctioned
        // divergence from ts's UTF-16 units (item 18)
        register_words!(module, {
            "STR-LENGTH" => Self::word_str_length,
                "( str:string -- length:number )",
                "Length of a string in characters (code points; 0 if null)";
            "SUBSTR" => Self::word_substr,
                "( str:string start:number end:number -- substring:string )",
                "Substring of str from start (inclusive) to end (exclusive), by character (code point) index. Indices clamp like String.slice (negatives count from the end).";
            "SPLICE" => Self::word_splice,
                "( str:string start:number end:number newval:string -- result:string )",
                "Replace the substring [start, end) of str with newval and return the result (a splice); character (code point) indices";
            "STARTS-WITH?" => Self::word_starts_with_q,
                "( str:string prefix:string -- bool:boolean )",
                "Returns true if str begins with prefix";
            "ENDS-WITH?" => Self::word_ends_with_q,
                "( str:string suffix:string -- bool:boolean )",
                "Returns true if str ends with suffix";
            "TRIM-PREFIX" => Self::word_trim_prefix,
                "( str:string prefix:string -- result:string )",
                "Strip prefix from start of str if present (otherwise return str unchanged)";
            "TRIM-SUFFIX" => Self::word_trim_suffix,
                "( str:string suffix:string -- result:string )",
                "Strip suffix from end of str if present (otherwise return str unchanged)";
        });
    }

    // ===== Regular Expressions =====

    fn register_regex_words(module: &mut Module) {
        register_words!(module, {
            "RE-MATCH?" => Self::word_re_match_q,
                "( str:string pattern:string -- bool:boolean )",
                "Returns true if str matches the regex pattern. Predicate-only — does not return the match.";
            "RE-MATCH" => Self::word_re_match,
                "( string:string pattern:string -- match:any )",
                "Match string against regex pattern: array [full, group1, ...] with null for non-participating groups; null on no match or null input";
            "RE-MATCH-ALL" => Self::word_re_match_all,
                "( string:string pattern:string -- matches:any[] )",
                "Find all regex matches in string (group 1 when it participated, else the full match)";
            "RE-REPLACE" => Self::word_re_replace,
                "( string:string pattern:string replace:string -- result:string )",
                "Replace all regex matches of pattern with replace. For literal replacement use REPLACE.";
        });
    }

    // ===== Shell-Flavored Text Processing =====

    fn register_shell_words(module: &mut Module) {
        register_words!(module, {
            "LINES" => Self::word_lines,
                "( str:string -- lines:string[] )",
                "Split string on newline. Equivalent to /N SPLIT.";
            "UNLINES" => Self::word_unlines,
                "( lines:string[] -- str:string )",
                "Join an array of lines with newlines. Equivalent to /N JOIN.";
            "GREP" => Self::word_grep,
                "( strings:string[] pattern:string -- matches:string[] )",
                "Keep only strings matching the regex pattern (bash grep); non-strings are dropped";
            "GREP-V" => Self::word_grep_v,
                "( strings:string[] pattern:string -- non_matches:string[] )",
                "Keep elements NOT matching the regex pattern, including non-strings (bash grep -v)";
            "SED" => Self::word_sed,
                "( strings:string[] pattern:string repl:string -- strings:string[] )",
                "Apply RE-REPLACE to each string in the array (bash sed s/pattern/repl/g); non-strings pass through";
            "CUT" => Self::word_cut,
                "( strings:string[] sep:string field:number -- field_values:any[] )",
                "Split each string on sep and pick the field-th column (bash cut). Out-of-range yields null.";
        });
    }

    fn type_error(word: &str, hint: &str) -> ForthicError {
        ForthicError::InvalidOperation {
            forthic: String::new(),
            message: format!("{word} requires a string. {hint}"),
            location: None,
            cause: None,
        }
    }

    /// Compile a pattern with a clean error (ts throws a raw SyntaxError).
    /// rs `regex` is linear-time, so ts's ReDoS caveat doesn't apply; its
    /// \d/\w classes are Unicode-aware (ts's are ASCII without the u
    /// flag) — accepted divergence.
    fn compile(pattern: &str) -> Result<Regex, ForthicError> {
        Regex::new(pattern).map_err(|e| ForthicError::InvalidOperation {
            forthic: String::new(),
            message: format!("Invalid regex '{pattern}': {e}"),
            location: None,
            cause: None,
        })
    }

    /// Normalize JS replacement syntax for the rs regex engine: `$&` means
    /// whole-match in JS (not special in rs) and JS reads `$1x` as group 1
    /// then literal x where rs would look for a group NAMED "1x". Rewriting
    /// to `${n}` form prevents silently wrong output.
    fn normalize_replacement(repl: &str) -> String {
        let mut out = String::with_capacity(repl.len());
        let chars: Vec<char> = repl.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '$' && i + 1 < chars.len() {
                match chars[i + 1] {
                    '$' => {
                        out.push_str("$$");
                        i += 2;
                    }
                    '&' => {
                        out.push_str("${0}");
                        i += 2;
                    }
                    c if c.is_ascii_digit() => {
                        let start = i + 1;
                        let mut j = start;
                        while j < chars.len() && chars[j].is_ascii_digit() {
                            j += 1;
                        }
                        let digits: String = chars[start..j].iter().collect();
                        out.push_str(&format!("${{{digits}}}"));
                        i = j;
                    }
                    _ => {
                        out.push('$');
                        i += 1;
                    }
                }
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        out
    }

    /// JS String.slice index resolution over CHAR indices (ts uses UTF-16
    /// units, which can split surrogate pairs — sanctioned quirk-fix)
    fn slice_index(i: i64, len: usize) -> usize {
        let len = len as i64;
        let resolved = if i < 0 { len + i } else { i };
        resolved.clamp(0, len) as usize
    }

    fn pop_int_default(context: &mut dyn InterpreterContext) -> Result<i64, ForthicError> {
        Ok(match context.stack_pop()? {
            ForthicValue::Int(i) => i,
            ForthicValue::Float(f) => f as i64,
            _ => 0, // JS ToInteger(null) == 0
        })
    }

    fn word_str_length(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let value = context.stack_pop()?;
        let length = match value {
            ForthicValue::Null => 0,
            ForthicValue::String(s) => s.chars().count() as i64,
            _ => {
                return Err(Self::type_error(
                    "STR-LENGTH",
                    "For arrays/records, use LENGTH.",
                ))
            }
        };
        context.stack_push(ForthicValue::Int(length));
        Ok(())
    }

    fn word_substr(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let end = Self::pop_int_default(context)?;
        let start = Self::pop_int_default(context)?;
        let value = context.stack_pop()?;
        let result = match value {
            ForthicValue::Null => String::new(),
            ForthicValue::String(s) => {
                let chars: Vec<char> = s.chars().collect();
                let a = Self::slice_index(start, chars.len());
                let b = Self::slice_index(end, chars.len());
                if a >= b {
                    String::new()
                } else {
                    chars[a..b].iter().collect()
                }
            }
            _ => return Err(Self::type_error("SUBSTR", "For arrays/records, use SLICE.")),
        };
        context.stack_push(ForthicValue::String(result));
        Ok(())
    }

    fn word_splice(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let newval = context.stack_pop()?;
        let end = Self::pop_int_default(context)?;
        let start = Self::pop_int_default(context)?;
        let value = context.stack_pop()?;
        let s = match value {
            ForthicValue::Null => String::new(),
            ForthicValue::String(s) => s,
            _ => return Err(Self::type_error("SPLICE", "")),
        };
        let ins = match newval {
            ForthicValue::Null => String::new(),
            other => Self::stringify(&other),
        };
        let chars: Vec<char> = s.chars().collect();
        let a = Self::slice_index(start, chars.len());
        let b = Self::slice_index(end, chars.len());
        let head: String = chars[..a].iter().collect();
        let tail: String = chars[b.min(chars.len())..].iter().collect();
        context.stack_push(ForthicValue::String(format!("{head}{ins}{tail}")));
        Ok(())
    }

    fn pop_two_strings(
        context: &mut dyn InterpreterContext,
    ) -> Result<Option<(String, String)>, ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;
        match (a, b) {
            (ForthicValue::String(a), ForthicValue::String(b)) => Ok(Some((a, b))),
            _ => Ok(None),
        }
    }

    fn word_starts_with_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let result = Self::pop_two_strings(context)?
            .map(|(s, prefix)| s.starts_with(&prefix))
            .unwrap_or(false);
        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    fn word_ends_with_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let result = Self::pop_two_strings(context)?
            .map(|(s, suffix)| s.ends_with(&suffix))
            .unwrap_or(false);
        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    fn word_trim_prefix(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let prefix = context.stack_pop()?;
        let value = context.stack_pop()?;
        let result = match (&value, &prefix) {
            (ForthicValue::String(s), ForthicValue::String(p)) if !p.is_empty() => {
                ForthicValue::String(s.strip_prefix(p.as_str()).unwrap_or(s).to_string())
            }
            _ => value, // non-string str/prefix: unchanged
        };
        context.stack_push(result);
        Ok(())
    }

    fn word_trim_suffix(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let suffix = context.stack_pop()?;
        let value = context.stack_pop()?;
        let result = match (&value, &suffix) {
            (ForthicValue::String(s), ForthicValue::String(p)) if !p.is_empty() => {
                ForthicValue::String(s.strip_suffix(p.as_str()).unwrap_or(s).to_string())
            }
            _ => value,
        };
        context.stack_push(result);
        Ok(())
    }

    fn word_re_match_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let result = match Self::pop_two_strings(context)? {
            Some((s, pattern)) => Self::compile(&pattern)?.is_match(&s),
            None => false,
        };
        context.stack_push(ForthicValue::Bool(result));
        Ok(())
    }

    /// RE-MATCH: ( string pattern -- match ) — array [full, g1, g2, ...]
    /// with NULL for non-participating groups; NULL on no match (and on a
    /// NULL input string — ts pushes false there, an implementation
    /// accident; both are falsy, one spelling is cleaner)
    fn word_re_match(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let result = match Self::pop_two_strings(context)? {
            Some((s, pattern)) => match Self::compile(&pattern)?.captures(&s) {
                Some(caps) => ForthicValue::Array(
                    (0..caps.len())
                        .map(|i| {
                            caps.get(i)
                                .map(|m| ForthicValue::String(m.as_str().to_string()))
                                .unwrap_or(ForthicValue::Null)
                        })
                        .collect(),
                ),
                None => ForthicValue::Null,
            },
            None => ForthicValue::Null,
        };
        context.stack_push(result);
        Ok(())
    }

    /// RE-MATCH-ALL: group 1 when it participated, else the full match
    /// (the post-fix ts contract)
    fn word_re_match_all(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let result = match Self::pop_two_strings(context)? {
            Some((s, pattern)) => {
                let re = Self::compile(&pattern)?;
                ForthicValue::Array(
                    re.captures_iter(&s)
                        .map(|caps| {
                            let m = caps.get(1).or_else(|| caps.get(0));
                            ForthicValue::String(
                                m.map(|m| m.as_str().to_string()).unwrap_or_default(),
                            )
                        })
                        .collect(),
                )
            }
            None => ForthicValue::Array(vec![]),
        };
        context.stack_push(result);
        Ok(())
    }

    fn word_re_replace(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let repl = context.stack_pop()?;
        let pattern = context.stack_pop()?;
        let value = context.stack_pop()?;
        let result = match (&value, &pattern) {
            (ForthicValue::Null, _) => ForthicValue::Null,
            (_, ForthicValue::Null) => value.clone(),
            (ForthicValue::String(s), ForthicValue::String(p)) => {
                let re = Self::compile(p)?;
                let repl_str = match &repl {
                    ForthicValue::Null => String::new(),
                    other => Self::stringify(other),
                };
                let normalized = Self::normalize_replacement(&repl_str);
                ForthicValue::String(re.replace_all(s, normalized.as_str()).to_string())
            }
            _ => value.clone(),
        };
        context.stack_push(result);
        Ok(())
    }

    fn word_lines(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let value = context.stack_pop()?;
        let result = match value {
            ForthicValue::String(s) => ForthicValue::Array(
                // Split on \n exactly; \r\n is NOT normalized (ts parity)
                s.split('\n')
                    .map(|l| ForthicValue::String(l.to_string()))
                    .collect(),
            ),
            _ => ForthicValue::Array(vec![]),
        };
        context.stack_push(result);
        Ok(())
    }

    fn word_unlines(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let value = context.stack_pop()?;
        let result = match value {
            ForthicValue::Array(lines) => lines
                .iter()
                .map(|l| match l {
                    ForthicValue::Null => String::new(),
                    other => Self::stringify(other),
                })
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        };
        context.stack_push(ForthicValue::String(result));
        Ok(())
    }

    /// GREP keeps string elements that match (non-strings dropped);
    /// GREP-V keeps NON-matching elements INCLUDING non-strings, and a
    /// non-string pattern filters nothing — deliberate asymmetry (ts)
    fn word_grep(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let pattern = context.stack_pop()?;
        let value = context.stack_pop()?;
        let result = match (&value, &pattern) {
            (ForthicValue::Array(items), ForthicValue::String(p)) => {
                let re = Self::compile(p)?;
                ForthicValue::Array(
                    items
                        .iter()
                        .filter(|v| matches!(v, ForthicValue::String(s) if re.is_match(s)))
                        .cloned()
                        .collect(),
                )
            }
            _ => ForthicValue::Array(vec![]),
        };
        context.stack_push(result);
        Ok(())
    }

    fn word_grep_v(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let pattern = context.stack_pop()?;
        let value = context.stack_pop()?;
        let result = match (&value, &pattern) {
            (ForthicValue::Array(items), ForthicValue::String(p)) => {
                let re = Self::compile(p)?;
                ForthicValue::Array(
                    items
                        .iter()
                        .filter(|v| !matches!(v, ForthicValue::String(s) if re.is_match(s)))
                        .cloned()
                        .collect(),
                )
            }
            (ForthicValue::Array(_), _) => value.clone(), // -v of nothing filters nothing
            _ => ForthicValue::Array(vec![]),
        };
        context.stack_push(result);
        Ok(())
    }

    fn word_sed(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let repl = context.stack_pop()?;
        let pattern = context.stack_pop()?;
        let value = context.stack_pop()?;
        let result = match (&value, &pattern) {
            (ForthicValue::Array(items), ForthicValue::String(p)) => {
                let re = Self::compile(p)?;
                let repl_str = match &repl {
                    ForthicValue::Null => String::new(),
                    other => Self::stringify(other),
                };
                let normalized = Self::normalize_replacement(&repl_str);
                ForthicValue::Array(
                    items
                        .iter()
                        .map(|v| match v {
                            ForthicValue::String(s) => ForthicValue::String(
                                re.replace_all(s, normalized.as_str()).to_string(),
                            ),
                            other => other.clone(), // non-strings pass through
                        })
                        .collect(),
                )
            }
            (ForthicValue::Array(_), _) => value.clone(),
            _ => ForthicValue::Array(vec![]),
        };
        context.stack_push(result);
        Ok(())
    }

    fn word_cut(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let field_val = context.stack_pop()?;
        let sep = context.stack_pop()?;
        let value = context.stack_pop()?;

        let field = match &field_val {
            ForthicValue::Int(i) => Some(*i),
            ForthicValue::Float(f) if f.fract() == 0.0 => Some(*f as i64),
            // ts Number("1") coercion parity
            ForthicValue::String(s) => s.parse::<i64>().ok(),
            _ => None,
        };
        let result = match (&value, &sep, field) {
            (ForthicValue::Array(items), ForthicValue::String(sep), Some(field)) if field >= 0 => {
                ForthicValue::Array(
                    items
                        .iter()
                        .map(|v| match v {
                            ForthicValue::String(s) => {
                                // Empty separator splits into chars (JS
                                // split('') parity; rust split("") yields
                                // empty bookends)
                                let parts: Vec<String> = if sep.is_empty() {
                                    s.chars().map(String::from).collect()
                                } else {
                                    s.split(sep.as_str()).map(String::from).collect()
                                };
                                parts
                                    .get(field as usize)
                                    .map(|p| ForthicValue::String(p.clone()))
                                    .unwrap_or(ForthicValue::Null)
                            }
                            _ => ForthicValue::Null,
                        })
                        .collect(),
                )
            }
            _ => ForthicValue::Array(vec![]),
        };
        context.stack_push(result);
        Ok(())
    }

    // ===== Pattern Operations =====

    fn register_pattern_words(module: &mut Module) {
        register_words!(module, {
            "REPLACE" => Self::word_replace,
                "( string:string text:string replace:string -- result:string )",
                "Replace all literal occurrences of text with replace. For regex matching use RE-REPLACE.";
        });
    }

    fn word_replace(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let replace = context.stack_pop()?;
        let text = context.stack_pop()?;
        let string = context.stack_pop()?;

        let result = match (string, text, replace) {
            (ForthicValue::String(s), ForthicValue::String(t), ForthicValue::String(r)) => {
                ForthicValue::String(s.replace(&t as &str, &r as &str))
            }
            _ => ForthicValue::String(String::new()),
        };

        context.stack_push(result);
        Ok(())
    }

    // ===== Constant Words =====

    fn register_constant_words(module: &mut Module) {
        register_words!(module, {
            "/N" => Self::word_newline,
                "( -- char:string )",
                "Newline character";
            "/R" => Self::word_carriage_return,
                "( -- char:string )",
                "Carriage return character";
            "/T" => Self::word_tab,
                "( -- char:string )",
                "Tab character";
        });
    }

    fn word_newline(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.stack_push(ForthicValue::String("\n".to_string()));
        Ok(())
    }

    fn word_carriage_return(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.stack_push(ForthicValue::String("\r".to_string()));
        Ok(())
    }

    fn word_tab(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.stack_push(ForthicValue::String("\t".to_string()));
        Ok(())
    }
}

impl Default for StringModule {
    fn default() -> Self {
        Self::new()
    }
}
