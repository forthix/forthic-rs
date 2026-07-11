//! Word-location tests (plans/TS-PARITY-BACKLOG.md item 13, ts #30 design)
//!
//! Interpreter errors now carry code locations: top-level errors get the
//! failing token's position, errors inside `:` definitions report the
//! failing word's capture site (recorded per-definition, parallel to its
//! words — never on the shared Word object), and WordExecution's
//! call_location records where the definition was invoked. run() attaches
//! the source snippet so format_with_context renders real code + caret.

// ForthicError is large; accepted trade-off (see lib.rs / backlog item 11)
#![allow(clippy::result_large_err)]

use forthic::errors::ForthicError;
use forthic::interpreter::Interpreter;

fn run_err(code: &str) -> ForthicError {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap_err()
}

/// 1-based column of `needle` in `code` (single-line codes only)
fn col_of(code: &str, needle: &str) -> usize {
    code.find(needle).expect("needle present") + 1
}

// ===== Top-level errors carry the failing token's location =====

#[test]
fn test_unknown_word_carries_location() {
    let code = "1 2 NO-SUCH-WORD";
    let err = run_err(code);
    let loc = err.get_location().expect("UnknownWord has a location now");
    assert_eq!(loc.line, 1);
    assert_eq!(loc.column, col_of(code, "NO-SUCH-WORD"));
    assert_eq!(loc.start_pos, 4);
    assert_eq!(loc.end_pos, Some(16));
}

#[test]
fn test_stack_underflow_carries_call_site() {
    let code = "1 POP POP";
    let err = run_err(code);
    let loc = err.get_location().expect("StackUnderflow has a location");
    assert_eq!(loc.column, code.rfind("POP").unwrap() + 1, "second POP");
}

#[test]
fn test_definition_syntax_errors_carry_locations() {
    // Nested ':' while compiling — the StartDef token's location points at
    // the definition NAME (the tokenizer skips whitespace to it), so the
    // error lands on G, not on ':'
    let code = ": F 1 : G";
    let err = run_err(code);
    assert!(matches!(err, ForthicError::MissingSemicolon { .. }));
    assert_eq!(err.get_location().unwrap().column, col_of(code, "G"));

    // Stray ';'
    let err = run_err("1 ;");
    assert!(matches!(err, ForthicError::ExtraSemicolon { .. }));
    assert_eq!(err.get_location().unwrap().column, 3);

    // EOS mid-definition
    let err = run_err(": F 1");
    assert!(matches!(err, ForthicError::MissingSemicolon { .. }));
    assert!(err.get_location().is_some());
}

// ===== Errors inside definitions: per-definition capture sites (ts #30) =====

fn word_execution_locations(err: &ForthicError) -> (Option<usize>, Option<usize>) {
    match err {
        ForthicError::WordExecution {
            call_location,
            definition_location,
            ..
        } => (
            call_location.as_ref().map(|l| l.column),
            definition_location.as_ref().map(|l| l.column),
        ),
        other => panic!("expected WordExecution, got {other}"),
    }
}

#[test]
fn test_error_reports_failing_words_own_capture_site() {
    // The ts #30 race scenario: two definitions share the SAME dictionary
    // word (POP). Each error must point at that definition's own use of it.
    let mut interp = Interpreter::standard("UTC");
    let alpha = ": ALPHA      POP ;";
    let beta = ": BETA POP ;";
    interp.run(alpha).unwrap();
    interp.run(beta).unwrap();

    let err_alpha = interp.run("ALPHA").unwrap_err();
    let (_, def_col) = word_execution_locations(&err_alpha);
    assert_eq!(def_col, Some(col_of(alpha, "POP")), "ALPHA's own POP");

    let err_beta = interp.run("BETA").unwrap_err();
    let (_, def_col) = word_execution_locations(&err_beta);
    assert_eq!(def_col, Some(col_of(beta, "POP")), "BETA's own POP");
}

#[test]
fn test_same_word_twice_in_one_definition() {
    // The failing OCCURRENCE is identified, not just the word
    let mut interp = Interpreter::standard("UTC");
    let def = ": F POP POP ;";
    interp.run(def).unwrap();
    interp.stack_push(forthic::literals::ForthicValue::Int(1));
    let err = interp.run("F").unwrap_err();
    let (_, def_col) = word_execution_locations(&err);
    assert_eq!(
        def_col,
        Some(def.rfind("POP").unwrap() + 1),
        "second POP is the one that underflows"
    );
}

#[test]
fn test_call_location_records_the_invocation_site() {
    let mut interp = Interpreter::standard("UTC");
    let def = ": F POP ;";
    interp.run(def).unwrap();
    let call_code = "42 POP F"; // stack empty by the time F runs
    let err = interp.run(call_code).unwrap_err();
    let (call_col, def_col) = word_execution_locations(&err);
    assert_eq!(call_col, Some(col_of(call_code, "F")), "where F was called");
    assert_eq!(
        def_col,
        Some(col_of(def, "POP")),
        "which word inside F failed"
    );
}

// ===== format_with_context renders real source =====

#[test]
fn test_formatted_error_shows_source_and_caret() {
    let code = "1 2 NO-SUCH-WORD";
    let err = run_err(code);
    let formatted = err.format_with_context();
    assert!(formatted.contains(code), "source rendered: {formatted}");
    let caret_line = formatted
        .lines()
        .find(|l| l.trim_start().starts_with('^'))
        .unwrap_or_else(|| panic!("no caret line in: {formatted}"));
    // Caret starts under NO-SUCH-WORD (column 5 -> 4 spaces of indent)
    assert_eq!(
        caret_line.chars().take_while(|c| *c == ' ').count(),
        col_of(code, "NO-SUCH-WORD") - 1,
        "caret aligned: {formatted}"
    );
    assert_eq!(
        caret_line.trim().len(),
        "NO-SUCH-WORD".len(),
        "caret spans the word: {formatted}"
    );
}

// ===== Nested runs keep their own source =====

#[test]
fn test_inner_run_source_wins() {
    // INTERPRET-style nesting: the inner error's forthic/location should
    // describe the inner source, not be clobbered by the outer run()
    let mut interp = Interpreter::standard("UTC");
    interp.run(": INNER 'NO-SUCH-WORD' ;").unwrap();
    // (No INTERPRET word in the stdlib yet; simulate nesting directly)
    let inner_err = interp.run("NO-SUCH-WORD").unwrap_err();
    let outer_wrapped = inner_err.with_forthic("OUTER SOURCE");
    assert_eq!(
        outer_wrapped.get_forthic(),
        Some("NO-SUCH-WORD"),
        "existing snippet not clobbered"
    );
}
