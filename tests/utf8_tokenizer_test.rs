//! UTF-8 tokenizer robustness tests (plans/TS-PARITY-BACKLOG.md item 12)
//!
//! The tokenizer previously mixed byte offsets (input_string.len()) with
//! char indexes (chars().nth, chars[i]) — on multibyte input that either
//! panicked (is_triple_quote indexed a chars vec with a byte-bounded index)
//! or silently mis-tokenized (loops ran past the last char). All positions
//! are now char indexes; these tests pin that for every token kind.

// ForthicError is large; accepted trade-off (see lib.rs / backlog item 11)
#![allow(clippy::result_large_err)]

use forthic::interpreter::Interpreter;
use forthic::literals::ForthicValue;
use forthic::tokenizer::{Token, TokenType, Tokenizer};

fn tokenize_all(code: &str) -> Vec<Token> {
    let mut tokenizer = Tokenizer::new(code.to_string(), None, false);
    let mut tokens = Vec::new();
    loop {
        let token = tokenizer.next_token().expect("tokenizes");
        if token.token_type == TokenType::Eos {
            break;
        }
        tokens.push(token);
    }
    tokens
}

fn run(code: &str) -> ForthicValue {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap();
    interp.get_stack_mut().pop().unwrap()
}

// ===== String literals =====

#[test]
fn test_multibyte_in_single_quoted_string() {
    // 2-byte (é), 3-byte (日), and 4-byte (emoji) chars
    assert_eq!(
        run("'héllo 日本 🦀'"),
        ForthicValue::String("héllo 日本 🦀".to_string())
    );
}

#[test]
fn test_multibyte_string_followed_by_more_tokens() {
    // The old byte-bounded loops lost sync after multibyte content;
    // everything after the string is the regression surface
    let mut interp = Interpreter::standard("UTC");
    interp.run("'héllo' 1 2 +").unwrap();
    assert_eq!(interp.get_stack().len(), 2);
    assert_eq!(interp.get_stack_mut().pop().unwrap(), ForthicValue::Int(3));
}

#[test]
fn test_multibyte_in_triple_quoted_string() {
    // is_triple_quote used a byte-length bound to index a chars vec —
    // this input panicked outright before the fix
    assert_eq!(
        run("'''héllo 🦀 wörld'''"),
        ForthicValue::String("héllo 🦀 wörld".to_string())
    );
}

#[test]
fn test_triple_quote_detection_near_end_of_multibyte_input() {
    // Byte length exceeds char count: a quote near the char-end used to
    // pass the byte-bound check in is_triple_quote and panic on chars[index]
    let mut tokenizer = Tokenizer::new("日本日本日本 'x".to_string(), None, false);
    let word = tokenizer.next_token().unwrap();
    assert_eq!(word.string, "日本日本日本");
    // The 'x is unterminated: must be a clean error, not a panic
    assert!(tokenizer.next_token().is_err());
}

#[test]
fn test_unterminated_multibyte_string_errors_cleanly() {
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("'日本語のテキスト").unwrap_err();
    assert!(err.to_string().contains("Unterminated"), "got: {err}");
}

// ===== Words, definitions, comments =====

#[test]
fn test_multibyte_word_names() {
    let mut interp = Interpreter::standard("UTC");
    interp.run(": GRÜSSE '👋 servus' ; GRÜSSE").unwrap();
    assert_eq!(
        interp.get_stack_mut().pop().unwrap(),
        ForthicValue::String("👋 servus".to_string())
    );
}

#[test]
fn test_multibyte_comment_then_code() {
    let mut interp = Interpreter::standard("UTC");
    interp.run("# コメント 🦀 comment\n40 2 +").unwrap();
    assert_eq!(interp.get_stack_mut().pop().unwrap(), ForthicValue::Int(42));
}

#[test]
fn test_multibyte_dot_symbol() {
    let tokens = tokenize_all(".变量");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::DotSymbol);
    assert_eq!(tokens[0].string, "变量");
}

#[test]
fn test_multibyte_module_name() {
    let tokens = tokenize_all("{módulo }");
    assert_eq!(tokens[0].token_type, TokenType::StartModule);
    assert_eq!(tokens[0].string, "módulo");
}

#[test]
fn test_multibyte_record_keys_end_to_end() {
    let value = run("[ [ 'ключ' 1 ] [ '鍵' 2 ] ] REC '鍵' REC@");
    assert_eq!(value, ForthicValue::Int(2));
}

// ===== Positions are char indexes =====

#[test]
fn test_token_positions_are_char_indexes() {
    // "日本 WORD" — the word starts at char 3 (not byte 7)
    let tokens = tokenize_all("'日本' WORD");
    assert_eq!(tokens.len(), 2);
    let word = &tokens[1];
    assert_eq!(word.string, "WORD");
    assert_eq!(word.location.start_pos, 5, "char index, not byte offset");
    assert_eq!(word.location.end_pos, Some(9));
    assert_eq!(word.location.column, 6);
}

#[test]
fn test_multibyte_token_end_pos_is_char_count() {
    let tokens = tokenize_all("日本語");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].location.start_pos, 0);
    // 3 chars, 9 bytes — end_pos must be 3
    assert_eq!(tokens[0].location.end_pos, Some(3));
}

#[test]
fn test_error_caret_lines_up_for_multibyte_source() {
    // Tokenizer errors carry locations; with char-based positions the
    // caret underline stays finite and aligned after multibyte content.
    // (Interpreter errors like UnknownWord have no location yet — Tier 4.)
    let mut interp = Interpreter::standard("UTC");
    let err = interp.run("'日本' 'unterminated").unwrap_err();
    let formatted = err.format_with_context();
    let caret_line = formatted
        .lines()
        .find(|l| l.contains('^'))
        .unwrap_or_else(|| panic!("no caret line in: {formatted}"));
    // Sanity: the caret indent + span must stay within the source's char
    // length; the old byte-based end_pos overshot on multibyte content
    assert!(caret_line.len() < 40, "runaway caret line: {caret_line:?}");
}

// ===== Line/column tracking across multibyte lines =====

#[test]
fn test_line_column_after_multibyte_line() {
    let tokens = tokenize_all("'héllo wörld'\nWORD");
    let word = &tokens[1];
    assert_eq!(word.location.line, 2);
    assert_eq!(word.location.column, 1);
}

// ===== String LENGTH counts chars, not bytes =====

#[test]
fn test_string_length_counts_chars() {
    // Same bug class as the tokenizer: s.len() is bytes. '🦀' is 1 char,
    // 4 bytes; 'héllo' is 5 chars, 6 bytes.
    assert_eq!(run("'🦀' LENGTH"), ForthicValue::Int(1));
    assert_eq!(run("'héllo' LENGTH"), ForthicValue::Int(5));
    assert_eq!(run("'日本語' LENGTH"), ForthicValue::Int(3));
}

// ===== Mixed stress =====

#[test]
fn test_mixed_multibyte_program() {
    let mut interp = Interpreter::standard("UTC");
    interp
        .run(concat!(
            "# säge 🪚\n",
            ": 挨拶 'こんにちは' ;\n",
            "[ [ 'clé' 挨拶 ] ] REC 'clé' REC@"
        ))
        .unwrap();
    assert_eq!(
        interp.get_stack_mut().pop().unwrap(),
        ForthicValue::String("こんにちは".to_string())
    );
}
