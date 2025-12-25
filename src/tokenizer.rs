//! Tokenizer for the Forthic interpreter
//!
//! This module provides lexical analysis for Forthic code, converting source text
//! into a stream of tokens that can be processed by the interpreter.

use crate::errors::{CodeLocation, ForthicError};

/// Token types recognized by the Forthic tokenizer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    String,
    Comment,
    StartArray,
    EndArray,
    StartModule,
    EndModule,
    StartDef,
    EndDef,
    StartMemo,
    Word,
    DotSymbol,
    Eos, // End of string
}

/// A token with its type, string value, and location
#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub string: String,
    pub location: CodeLocation,
}

impl Token {
    pub fn new(token_type: TokenType, string: String, location: CodeLocation) -> Self {
        Self {
            token_type,
            string,
            location,
        }
    }
}

/// Tracks changes to the input string for streaming support
#[derive(Debug, Clone)]
struct StringDelta {
    #[allow(dead_code)]
    start: usize,
    end: usize,
}

/// Tokenizer state machine for Forthic code
///
/// The tokenizer processes Forthic source code character by character,
/// identifying tokens such as words, strings, arrays, modules, and definitions.
pub struct Tokenizer {
    reference_location: CodeLocation,
    line: usize,
    column: usize,
    input_string: String,
    input_pos: usize,
    whitespace: Vec<char>,
    quote_chars: Vec<char>,

    // Token tracking
    token_start_pos: usize,
    token_line: usize,
    token_column: usize,
    token_string: String,

    string_delta: Option<StringDelta>,
    streaming: bool,
}

impl Tokenizer {
    /// Create a new tokenizer
    ///
    /// # Arguments
    ///
    /// * `string` - The source code to tokenize
    /// * `reference_location` - Optional starting location for error reporting
    /// * `streaming` - Whether this is a streaming tokenizer (incomplete input allowed)
    pub fn new(
        string: String,
        reference_location: Option<CodeLocation>,
        streaming: bool,
    ) -> Self {
        let reference_location = reference_location.unwrap_or_default();
        let line = reference_location.line;
        let column = reference_location.column;

        Self {
            reference_location: reference_location.clone(),
            line,
            column,
            input_string: Self::unescape_string(&string),
            input_pos: 0,
            whitespace: vec![' ', '\t', '\n', '\r', '(', ')', ','],
            quote_chars: vec!['"', '\'', '^'],
            token_start_pos: 0,
            token_line: 0,
            token_column: 0,
            token_string: String::new(),
            string_delta: None,
            streaming,
        }
    }

    /// Get the next token from the input
    pub fn next_token(&mut self) -> Result<Token, ForthicError> {
        self.clear_token_string();
        self.transition_from_start()
    }

    /// Get the input string being tokenized
    pub fn get_input_string(&self) -> &str {
        &self.input_string
    }

    /// Unescape HTML entities in the input string
    fn unescape_string(s: &str) -> String {
        s.replace("&lt;", "<").replace("&gt;", ">")
    }

    fn clear_token_string(&mut self) {
        self.token_string.clear();
    }

    fn note_start_token(&mut self) {
        self.token_start_pos = self.input_pos + self.reference_location.start_pos;
        self.token_line = self.line;
        self.token_column = self.column;
    }

    fn is_whitespace(&self, ch: char) -> bool {
        self.whitespace.contains(&ch)
    }

    fn is_quote(&self, ch: char) -> bool {
        self.quote_chars.contains(&ch)
    }

    fn is_triple_quote(&self, index: usize, ch: char) -> bool {
        if !self.is_quote(ch) {
            return false;
        }
        if index + 2 >= self.input_string.len() {
            return false;
        }
        let chars: Vec<char> = self.input_string.chars().collect();
        chars[index + 1] == ch && chars[index + 2] == ch
    }

    fn is_start_memo(&self, index: usize) -> bool {
        if index + 1 >= self.input_string.len() {
            return false;
        }
        let chars: Vec<char> = self.input_string.chars().collect();
        chars[index] == '@' && chars[index + 1] == ':'
    }

    fn advance_position(&mut self, num_chars: isize) -> Result<usize, ForthicError> {
        let chars: Vec<char> = self.input_string.chars().collect();

        if num_chars >= 0 {
            for _ in 0..num_chars {
                if self.input_pos < chars.len() && chars[self.input_pos] == '\n' {
                    self.line += 1;
                    self.column = 1;
                } else {
                    self.column += 1;
                }
                self.input_pos += 1;
            }
            Ok(num_chars as usize)
        } else {
            for _ in 0..(-num_chars) {
                self.input_pos = self.input_pos.checked_sub(1).ok_or_else(|| {
                    ForthicError::InvalidInputPosition {
                        forthic: self.input_string.clone(),
                        location: Some(self.get_token_location()),
                        cause: None,
                    }
                })?;

                if self.input_pos < chars.len() && chars[self.input_pos] == '\n' {
                    self.line = self.line.saturating_sub(1);
                    self.column = 1;
                } else {
                    self.column = self.column.saturating_sub(1);
                }
            }
            Ok((-num_chars) as usize)
        }
    }

    fn get_token_location(&self) -> CodeLocation {
        CodeLocation {
            source: self.reference_location.source.clone(),
            line: self.token_line,
            column: self.token_column,
            start_pos: self.token_start_pos,
            end_pos: Some(self.token_start_pos + self.token_string.len()),
        }
    }

    fn get_char_at(&self, index: usize) -> Option<char> {
        self.input_string.chars().nth(index)
    }

    // State transitions

    fn transition_from_start(&mut self) -> Result<Token, ForthicError> {
        while self.input_pos < self.input_string.len() {
            let ch = self.get_char_at(self.input_pos).unwrap();
            self.note_start_token();
            self.advance_position(1)?;

            if self.is_whitespace(ch) {
                continue;
            } else if ch == '#' {
                return self.transition_from_comment();
            } else if ch == ':' {
                return self.transition_from_start_definition();
            } else if self.is_start_memo(self.input_pos - 1) {
                self.advance_position(1)?; // Skip over ":" in "@:"
                return self.transition_from_start_memo();
            } else if ch == ';' {
                self.token_string = ch.to_string();
                return Ok(Token::new(
                    TokenType::EndDef,
                    ch.to_string(),
                    self.get_token_location(),
                ));
            } else if ch == '[' {
                self.token_string = ch.to_string();
                return Ok(Token::new(
                    TokenType::StartArray,
                    ch.to_string(),
                    self.get_token_location(),
                ));
            } else if ch == ']' {
                self.token_string = ch.to_string();
                return Ok(Token::new(
                    TokenType::EndArray,
                    ch.to_string(),
                    self.get_token_location(),
                ));
            } else if ch == '{' {
                return self.transition_from_gather_module();
            } else if ch == '}' {
                self.token_string = ch.to_string();
                return Ok(Token::new(
                    TokenType::EndModule,
                    ch.to_string(),
                    self.get_token_location(),
                ));
            } else if self.is_triple_quote(self.input_pos - 1, ch) {
                self.advance_position(2)?; // Skip over 2nd and 3rd quote chars
                return self.transition_from_gather_triple_quote_string(ch);
            } else if self.is_quote(ch) {
                return self.transition_from_gather_string(ch);
            } else if ch == '.' {
                self.advance_position(-1)?; // Back up to beginning of dot symbol
                return self.transition_from_gather_dot_symbol();
            } else {
                self.advance_position(-1)?; // Back up to beginning of word
                return self.transition_from_gather_word();
            }
        }

        Ok(Token::new(
            TokenType::Eos,
            String::new(),
            self.get_token_location(),
        ))
    }

    fn transition_from_comment(&mut self) -> Result<Token, ForthicError> {
        self.note_start_token();
        while self.input_pos < self.input_string.len() {
            let ch = self.get_char_at(self.input_pos).unwrap();
            self.token_string.push(ch);
            self.advance_position(1)?;
            if ch == '\n' {
                self.advance_position(-1)?;
                break;
            }
        }
        Ok(Token::new(
            TokenType::Comment,
            self.token_string.clone(),
            self.get_token_location(),
        ))
    }

    fn transition_from_start_definition(&mut self) -> Result<Token, ForthicError> {
        while self.input_pos < self.input_string.len() {
            let ch = self.get_char_at(self.input_pos).unwrap();
            self.advance_position(1)?;

            if self.is_whitespace(ch) {
                continue;
            } else if self.is_quote(ch) {
                return Err(ForthicError::InvalidWordName {
                    forthic: self.input_string.clone(),
                    note: Some("Definition names can't have quotes in them".to_string()),
                    location: Some(self.get_token_location()),
                    cause: None,
                });
            } else {
                self.advance_position(-1)?;
                return self.transition_from_gather_definition_name();
            }
        }

        Err(ForthicError::InvalidWordName {
            forthic: self.input_string.clone(),
            note: Some("Got EOS in START_DEFINITION".to_string()),
            location: Some(self.get_token_location()),
            cause: None,
        })
    }

    fn transition_from_start_memo(&mut self) -> Result<Token, ForthicError> {
        while self.input_pos < self.input_string.len() {
            let ch = self.get_char_at(self.input_pos).unwrap();
            self.advance_position(1)?;

            if self.is_whitespace(ch) {
                continue;
            } else if self.is_quote(ch) {
                return Err(ForthicError::InvalidWordName {
                    forthic: self.input_string.clone(),
                    note: Some("Memo names can't have quotes in them".to_string()),
                    location: Some(self.get_token_location()),
                    cause: None,
                });
            } else {
                self.advance_position(-1)?;
                return self.transition_from_gather_memo_name();
            }
        }

        Err(ForthicError::InvalidWordName {
            forthic: self.input_string.clone(),
            note: Some("Got EOS in START_MEMO".to_string()),
            location: Some(self.get_token_location()),
            cause: None,
        })
    }

    fn gather_definition_name(&mut self) -> Result<(), ForthicError> {
        while self.input_pos < self.input_string.len() {
            let ch = self.get_char_at(self.input_pos).unwrap();
            self.advance_position(1)?;

            if self.is_whitespace(ch) {
                break;
            }
            if self.is_quote(ch) {
                return Err(ForthicError::InvalidWordName {
                    forthic: self.input_string.clone(),
                    note: Some("Definition names can't have quotes in them".to_string()),
                    location: Some(self.get_token_location()),
                    cause: None,
                });
            }
            if [';', '[', ']', '{', '}'].contains(&ch) {
                return Err(ForthicError::InvalidWordName {
                    forthic: self.input_string.clone(),
                    note: Some(format!("Definition names can't have '{}' in them", ch)),
                    location: Some(self.get_token_location()),
                    cause: None,
                });
            }
            self.token_string.push(ch);
        }
        Ok(())
    }

    fn transition_from_gather_definition_name(&mut self) -> Result<Token, ForthicError> {
        self.note_start_token();
        self.gather_definition_name()?;
        Ok(Token::new(
            TokenType::StartDef,
            self.token_string.clone(),
            self.get_token_location(),
        ))
    }

    fn transition_from_gather_memo_name(&mut self) -> Result<Token, ForthicError> {
        self.note_start_token();
        self.gather_definition_name()?;
        Ok(Token::new(
            TokenType::StartMemo,
            self.token_string.clone(),
            self.get_token_location(),
        ))
    }

    fn transition_from_gather_module(&mut self) -> Result<Token, ForthicError> {
        self.note_start_token();
        while self.input_pos < self.input_string.len() {
            let ch = self.get_char_at(self.input_pos).unwrap();
            self.advance_position(1)?;

            if self.is_whitespace(ch) {
                break;
            } else if ch == '}' {
                self.advance_position(-1)?;
                break;
            } else {
                self.token_string.push(ch);
            }
        }
        Ok(Token::new(
            TokenType::StartModule,
            self.token_string.clone(),
            self.get_token_location(),
        ))
    }

    fn transition_from_gather_triple_quote_string(
        &mut self,
        delim: char,
    ) -> Result<Token, ForthicError> {
        self.note_start_token();
        self.string_delta = Some(StringDelta {
            start: self.input_pos,
            end: self.input_pos,
        });

        while self.input_pos < self.input_string.len() {
            let ch = self.get_char_at(self.input_pos).unwrap();

            if ch == delim && self.is_triple_quote(self.input_pos, ch) {
                // Check if this triple quote is followed by at least one more quote (greedy mode)
                if self.input_pos + 3 < self.input_string.len()
                    && self.get_char_at(self.input_pos + 3) == Some(delim)
                {
                    // Greedy mode: include this quote as content and continue
                    self.advance_position(1)?;
                    self.token_string.push(delim);
                    if let Some(ref mut delta) = self.string_delta {
                        delta.end = self.input_pos;
                    }
                    continue;
                }

                // Normal behavior: close at first triple quote
                self.advance_position(3)?;
                self.string_delta = None;
                return Ok(Token::new(
                    TokenType::String,
                    self.token_string.clone(),
                    self.get_token_location(),
                ));
            } else {
                self.advance_position(1)?;
                self.token_string.push(ch);
                if let Some(ref mut delta) = self.string_delta {
                    delta.end = self.input_pos;
                }
            }
        }

        if self.streaming {
            // In streaming mode, return incomplete token (implementation specific)
            return Ok(Token::new(
                TokenType::String,
                self.token_string.clone(),
                self.get_token_location(),
            ));
        }

        Err(ForthicError::UnterminatedString {
            forthic: self.input_string.clone(),
            location: Some(self.get_token_location()),
            cause: None,
        })
    }

    fn transition_from_gather_string(&mut self, delim: char) -> Result<Token, ForthicError> {
        self.note_start_token();
        self.string_delta = Some(StringDelta {
            start: self.input_pos,
            end: self.input_pos,
        });

        while self.input_pos < self.input_string.len() {
            let ch = self.get_char_at(self.input_pos).unwrap();
            self.advance_position(1)?;

            if ch == delim {
                self.string_delta = None;
                return Ok(Token::new(
                    TokenType::String,
                    self.token_string.clone(),
                    self.get_token_location(),
                ));
            } else {
                self.token_string.push(ch);
                if let Some(ref mut delta) = self.string_delta {
                    delta.end = self.input_pos;
                }
            }
        }

        if self.streaming {
            return Ok(Token::new(
                TokenType::String,
                self.token_string.clone(),
                self.get_token_location(),
            ));
        }

        Err(ForthicError::UnterminatedString {
            forthic: self.input_string.clone(),
            location: Some(self.get_token_location()),
            cause: None,
        })
    }

    fn transition_from_gather_word(&mut self) -> Result<Token, ForthicError> {
        self.note_start_token();
        while self.input_pos < self.input_string.len() {
            let ch = self.get_char_at(self.input_pos).unwrap();
            self.advance_position(1)?;

            if self.is_whitespace(ch) {
                break;
            }

            // Special case: if token contains 'T' and we encounter '[',
            // this is likely a zoned datetime with IANA timezone bracket notation.
            // Include the bracketed timezone as part of the token.
            if ch == '[' && self.token_string.contains('T') {
                self.token_string.push(ch);
                // Continue gathering until closing bracket
                while self.input_pos < self.input_string.len() {
                    let ch2 = self.get_char_at(self.input_pos).unwrap();
                    self.advance_position(1)?;
                    self.token_string.push(ch2);
                    if ch2 == ']' {
                        break;
                    }
                }
            } else if [';', '[', ']', '{', '}', '#'].contains(&ch) {
                self.advance_position(-1)?;
                break;
            } else {
                self.token_string.push(ch);
            }
        }
        Ok(Token::new(
            TokenType::Word,
            self.token_string.clone(),
            self.get_token_location(),
        ))
    }

    fn transition_from_gather_dot_symbol(&mut self) -> Result<Token, ForthicError> {
        self.note_start_token();
        let mut full_token_string = String::new();

        while self.input_pos < self.input_string.len() {
            let ch = self.get_char_at(self.input_pos).unwrap();
            self.advance_position(1)?;

            if self.is_whitespace(ch) {
                break;
            }
            if [';', '[', ']', '{', '}', '#'].contains(&ch) {
                self.advance_position(-1)?;
                break;
            }
            full_token_string.push(ch);
            self.token_string.push(ch);
        }

        // If dot symbol has no characters after the dot, treat it as a word
        if full_token_string.len() < 2 {
            return Ok(Token::new(
                TokenType::Word,
                full_token_string,
                self.get_token_location(),
            ));
        }

        // For DOT_SYMBOL, return the string without the dot prefix
        let symbol_without_dot = full_token_string[1..].to_string();
        Ok(Token::new(
            TokenType::DotSymbol,
            symbol_without_dot,
            self.get_token_location(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize_all(code: &str) -> Result<Vec<Token>, ForthicError> {
        let mut tokenizer = Tokenizer::new(code.to_string(), None, false);
        let mut tokens = Vec::new();

        loop {
            let token = tokenizer.next_token()?;
            if token.token_type == TokenType::Eos {
                break;
            }
            tokens.push(token);
        }

        Ok(tokens)
    }

    #[test]
    fn test_simple_words() {
        let tokens = tokenize_all("DUP SWAP").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].token_type, TokenType::Word);
        assert_eq!(tokens[0].string, "DUP");
        assert_eq!(tokens[1].token_type, TokenType::Word);
        assert_eq!(tokens[1].string, "SWAP");
    }

    #[test]
    fn test_string_literal() {
        let tokens = tokenize_all(r#""hello world""#).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::String);
        assert_eq!(tokens[0].string, "hello world");
    }

    #[test]
    fn test_triple_quote_string() {
        let tokens = tokenize_all(r#""""multi
line
string""""#)
            .unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::String);
        assert!(tokens[0].string.contains("multi"));
        assert!(tokens[0].string.contains("line"));
    }

    #[test]
    fn test_array() {
        let tokens = tokenize_all("[ 1 2 3 ]").unwrap();
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].token_type, TokenType::StartArray);
        assert_eq!(tokens[1].token_type, TokenType::Word);
        assert_eq!(tokens[4].token_type, TokenType::EndArray);
    }

    #[test]
    fn test_definition() {
        let tokens = tokenize_all(": DOUBLE 2 * ;").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::StartDef);
        assert_eq!(tokens[0].string, "DOUBLE");
        assert_eq!(tokens[3].token_type, TokenType::EndDef);
    }

    #[test]
    fn test_memo() {
        let tokens = tokenize_all("@: CACHED 42 ;").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::StartMemo);
        assert_eq!(tokens[0].string, "CACHED");
    }

    #[test]
    fn test_module() {
        let tokens = tokenize_all("{ : WORD 42 ; }").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::StartModule);
        assert!(tokens.iter().any(|t| t.token_type == TokenType::EndModule));
    }

    #[test]
    fn test_comment() {
        let tokens = tokenize_all("DUP # This is a comment\nSWAP").unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].token_type, TokenType::Word);
        assert_eq!(tokens[1].token_type, TokenType::Comment);
        assert_eq!(tokens[2].token_type, TokenType::Word);
    }

    #[test]
    fn test_dot_symbol() {
        let tokens = tokenize_all(".field").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::DotSymbol);
        assert_eq!(tokens[0].string, "field");
    }

    #[test]
    fn test_unterminated_string() {
        let result = tokenize_all(r#""unterminated"#);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ForthicError::UnterminatedString { .. }
        ));
    }

    #[test]
    fn test_invalid_definition_name() {
        let result = tokenize_all(r#": "INVALID" ;"#);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ForthicError::InvalidWordName { .. }
        ));
    }

    #[test]
    fn test_whitespace_handling() {
        let tokens = tokenize_all("  DUP  \n\t  SWAP  ").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].string, "DUP");
        assert_eq!(tokens[1].string, "SWAP");
    }

    #[test]
    fn test_token_locations() {
        let tokens = tokenize_all("DUP SWAP").unwrap();
        assert_eq!(tokens[0].location.start_pos, 0);
        assert_eq!(tokens[0].location.end_pos, Some(3));
        assert_eq!(tokens[1].location.start_pos, 4);
        assert_eq!(tokens[1].location.end_pos, Some(8));
    }
}
