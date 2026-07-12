//! Batch 4 words: strings & interpolation (plans/WORD-INVENTORY.md)
//!
//! Documented divergences from ts: char (code point) indices for
//! STR-LENGTH/SUBSTR/SPLICE per the host-native units decision (backlog
//! item 18); RE-MATCH pushes NULL (not false) for no-match/null input;
//! regex compile failures are clean InvalidOperation errors (ts throws a
//! raw SyntaxError); the rs regex engine is linear-time (no ReDoS) with
//! Unicode-aware \d/\w classes.

// ForthicError is large; accepted trade-off (see lib.rs / backlog item 11)
#![allow(clippy::result_large_err)]

use forthic::interpreter::Interpreter;
use forthic::literals::ForthicValue;

fn run(code: &str) -> ForthicValue {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap();
    interp.get_stack_mut().pop().unwrap()
}

fn run_err(code: &str) -> forthic::errors::ForthicError {
    let mut interp = Interpreter::standard("UTC");
    interp.run(code).unwrap_err()
}

fn s(v: &str) -> ForthicValue {
    ForthicValue::String(v.to_string())
}

fn strs(values: &[&str]) -> ForthicValue {
    ForthicValue::Array(values.iter().map(|v| s(v)).collect())
}

fn b(v: bool) -> ForthicValue {
    ForthicValue::Bool(v)
}

// ===== STR-LENGTH =====

#[test]
fn test_str_length_counts_chars() {
    assert_eq!(run("'hello' STR-LENGTH"), ForthicValue::Int(5));
    assert_eq!(run("'' STR-LENGTH"), ForthicValue::Int(0));
    assert_eq!(run("NULL STR-LENGTH"), ForthicValue::Int(0));
    // Host-native units: rs counts code points (ts .length would say 2)
    assert_eq!(run("'🦀' STR-LENGTH"), ForthicValue::Int(1));
}

#[test]
fn test_str_length_rejects_containers() {
    let err = run_err("[ 1 2 ] STR-LENGTH");
    assert!(err.to_string().contains("use LENGTH"), "got: {err}");
}

// ===== SUBSTR / SPLICE =====

#[test]
fn test_substr_js_slice_semantics() {
    assert_eq!(run("'hello' 1 3 SUBSTR"), s("el"));
    // Negative indices count from the end
    assert_eq!(run("'hello' -3 -1 SUBSTR"), s("ll"));
    // Out-of-range clamps; crossed range is empty
    assert_eq!(run("'hi' 0 99 SUBSTR"), s("hi"));
    assert_eq!(run("'hello' 3 1 SUBSTR"), s(""));
    assert_eq!(run("NULL 0 2 SUBSTR"), s(""));
    // Char indices — astral chars never split (ts can cut a surrogate)
    assert_eq!(run("'a🦀b' 1 2 SUBSTR"), s("🦀"));
}

#[test]
fn test_splice_replaces_a_char_range() {
    assert_eq!(run("'hello' 1 3 'XY' SPLICE"), s("hXYlo"));
    // NULL insert deletes the range
    assert_eq!(run("'hello' 1 3 NULL SPLICE"), s("hlo"));
    // Insert-at-point via an empty range
    assert_eq!(run("'ab' 1 1 'X' SPLICE"), s("aXb"));
    // Non-string inserts stringify
    assert_eq!(run("'ab' 1 1 42 SPLICE"), s("a42b"));
}

// ===== STARTS-WITH? / ENDS-WITH? / TRIM-PREFIX / TRIM-SUFFIX =====

#[test]
fn test_starts_and_ends_with() {
    assert_eq!(run("'hello' 'he' STARTS-WITH?"), b(true));
    assert_eq!(run("'hello' 'lo' STARTS-WITH?"), b(false));
    assert_eq!(run("'hello' 'lo' ENDS-WITH?"), b(true));
    // Non-string operands are false, not an error
    assert_eq!(run("NULL 'x' STARTS-WITH?"), b(false));
    assert_eq!(run("'x' NULL ENDS-WITH?"), b(false));
}

#[test]
fn test_trim_prefix_and_suffix() {
    assert_eq!(run("'foobar' 'foo' TRIM-PREFIX"), s("bar"));
    assert_eq!(run("'foobar' 'zzz' TRIM-PREFIX"), s("foobar"));
    assert_eq!(run("'foobar' 'bar' TRIM-SUFFIX"), s("foo"));
    // Trims at most ONE occurrence
    assert_eq!(run("'aaX' 'a' TRIM-PREFIX"), s("aX"));
    // Empty/non-string prefix: unchanged (including non-string values)
    assert_eq!(run("'foo' '' TRIM-PREFIX"), s("foo"));
    assert_eq!(run("42 'x' TRIM-PREFIX"), ForthicValue::Int(42));
}

// ===== Regex words =====

#[test]
fn test_re_match_q() {
    assert_eq!(run(r"'abc123' '\d+' RE-MATCH?"), b(true));
    assert_eq!(run(r"'abc' '\d' RE-MATCH?"), b(false));
    assert_eq!(run(r"NULL '\d' RE-MATCH?"), b(false));
}

#[test]
fn test_re_match_returns_groups_array() {
    // [full, group1, group2, ...]
    assert_eq!(
        run(r"'2026-07-11' '(\d+)-(\d+)' RE-MATCH"),
        strs(&["2026-07", "2026", "07"])
    );
    // Non-participating groups are NULL
    assert_eq!(
        run(r"'ab' '(a)(z)?(b)' RE-MATCH"),
        ForthicValue::Array(vec![s("ab"), s("a"), ForthicValue::Null, s("b")])
    );
    // No match / NULL input: NULL (ts pushes false — both falsy; divergence
    // documented in WORD-INVENTORY)
    assert_eq!(run(r"'abc' '\d' RE-MATCH"), ForthicValue::Null);
    assert_eq!(run(r"NULL 'a' RE-MATCH"), ForthicValue::Null);
}

#[test]
fn test_re_match_all_prefers_group_one() {
    // With a capture group: collect group 1 per match
    assert_eq!(run(r"'a=1, b=2' '(\w)=\d' RE-MATCH-ALL"), strs(&["a", "b"]));
    // Without groups: full matches
    assert_eq!(run(r"'a1b22' '\d+' RE-MATCH-ALL"), strs(&["1", "22"]));
    assert_eq!(run(r"'abc' '\d' RE-MATCH-ALL"), ForthicValue::Array(vec![]));
    assert_eq!(run(r"NULL 'a' RE-MATCH-ALL"), ForthicValue::Array(vec![]));
}

#[test]
fn test_re_replace_normalizes_js_backrefs() {
    assert_eq!(
        run(r"'hello world' 'o' '0' RE-REPLACE"),
        s("hell0 w0rld"),
        "replaces ALL matches"
    );
    // JS $1 backrefs work even when followed by a word character (raw rs
    // regex syntax would read $1x as a group NAMED '1x')
    assert_eq!(run(r"'ab' '(a)(b)' '$2$1x' RE-REPLACE"), s("bax"));
    // $& is the whole match; $$ is a literal dollar
    assert_eq!(run(r"'hi' 'hi' '<$&>' RE-REPLACE"), s("<hi>"));
    assert_eq!(run(r"'x' 'x' '$$5' RE-REPLACE"), s("$5"));
    // NULL contracts: null string stays NULL; null pattern is a no-op;
    // null replacement deletes matches
    assert_eq!(run(r"NULL 'a' 'b' RE-REPLACE"), ForthicValue::Null);
    assert_eq!(run(r"'ab' NULL 'x' RE-REPLACE"), s("ab"));
    assert_eq!(run(r"'a1b' '\d' NULL RE-REPLACE"), s("ab"));
}

#[test]
fn test_invalid_regex_is_a_clean_error() {
    // ts throws a raw SyntaxError; rs wraps it as InvalidOperation
    let err = run_err(r"'x' '(' RE-MATCH?");
    assert!(err.to_string().contains("Invalid regex"), "got: {err}");
}

// ===== LINES / UNLINES =====

#[test]
fn test_lines_splits_on_newline_exactly() {
    assert_eq!(run("'a\nb\nc' LINES"), strs(&["a", "b", "c"]));
    // "" is one empty line (JS ''.split('\n') parity)
    assert_eq!(run("'' LINES"), strs(&[""]));
    // \r\n is NOT normalized — the \r stays on the line
    assert_eq!(run("'a\r\nb' LINES"), strs(&["a\r", "b"]));
    assert_eq!(run("NULL LINES"), ForthicValue::Array(vec![]));
}

#[test]
fn test_unlines_joins_and_stringifies() {
    assert_eq!(run("[ 'a' 'b' ] UNLINES"), s("a\nb"));
    // NULL elements render empty; non-strings stringify
    assert_eq!(run("[ 'a' NULL 42 ] UNLINES"), s("a\n\n42"));
    assert_eq!(run("NULL UNLINES"), s(""));
}

// ===== GREP / GREP-V / SED / CUT =====

#[test]
fn test_grep_keeps_matching_strings_only() {
    assert_eq!(
        run(r"[ 'apple' 'banana' 'cherry' ] 'an' GREP"),
        strs(&["banana"])
    );
    // Non-string elements are dropped (they can't match)
    assert_eq!(run(r"[ 'a1' 42 'b2' ] '\d' GREP"), strs(&["a1", "b2"]));
    // Non-string pattern or non-array input: empty
    assert_eq!(run(r"[ 'a' ] NULL GREP"), ForthicValue::Array(vec![]));
    assert_eq!(run(r"NULL 'a' GREP"), ForthicValue::Array(vec![]));
}

#[test]
fn test_grep_v_keeps_non_matches_including_non_strings() {
    assert_eq!(
        run(r"[ 'a1' 42 'bb' ] '\d' GREP-V"),
        ForthicValue::Array(vec![ForthicValue::Int(42), s("bb")]),
        "deliberate asymmetry: -v keeps non-strings"
    );
    // Non-string pattern filters nothing
    assert_eq!(
        run(r"[ 'a' 42 ] NULL GREP-V"),
        ForthicValue::Array(vec![s("a"), ForthicValue::Int(42)])
    );
}

#[test]
fn test_sed_replaces_per_element() {
    assert_eq!(run(r"[ 'a1' 'b2' ] '\d' 'X' SED"), strs(&["aX", "bX"]));
    // Non-strings pass through untouched
    assert_eq!(
        run(r"[ 'a1' 42 ] '\d' 'X' SED"),
        ForthicValue::Array(vec![s("aX"), ForthicValue::Int(42)])
    );
    // Backref normalization matches RE-REPLACE
    assert_eq!(run(r"[ 'ab' ] '(a)' '<$1>' SED"), strs(&["<a>b"]));
}

#[test]
fn test_cut_extracts_a_field_per_line() {
    assert_eq!(run("[ 'a:b:c' 'x:y' ] ':' 1 CUT"), strs(&["b", "y"]));
    // Out-of-range field is NULL for that element
    assert_eq!(
        run("[ 'a:b' 'x' ] ':' 1 CUT"),
        ForthicValue::Array(vec![s("b"), ForthicValue::Null])
    );
    // String field numbers coerce (ts Number('1'))
    assert_eq!(run("[ 'a:b' ] ':' '1' CUT"), strs(&["b"]));
    // Empty separator splits into chars
    assert_eq!(run("[ 'ab' ] '' 1 CUT"), strs(&["b"]));
    // Non-string elements yield NULL
    assert_eq!(
        run("[ 42 ] ':' 0 CUT"),
        ForthicValue::Array(vec![ForthicValue::Null])
    );
}

// ===== INTERPOLATE =====

#[test]
fn test_interpolate_fills_holes_from_variables() {
    assert_eq!(
        run("'World' .name ! 'Hello ${name}!' INTERPOLATE"),
        s("Hello World!")
    );
    // The dot-symbol spelling works too; body whitespace trims
    assert_eq!(run("'x' .v ! '${.v}' INTERPOLATE"), s("x"));
    assert_eq!(run("'x' .v ! '${ v }' INTERPOLATE"), s("x"));
}

#[test]
fn test_interpolate_lookup_is_read_only() {
    // A miss renders as null_text (default "") and creates nothing
    assert_eq!(run("'a ${nope} b' INTERPOLATE"), s("a  b"));
    assert_eq!(run("NULL .v ! '<${v}>' INTERPOLATE"), s("<>"));
    // null_text opt-in makes misses/NULLs visible
    assert_eq!(
        run("'<${v}>' [ .null_text 'null' ] ~> INTERPOLATE"),
        s("<null>")
    );
}

#[test]
fn test_interpolate_needs_the_full_hole_shape() {
    // Bare dots, braces, and dollars are literal text — only ${...} is a
    // hole (the old bare-dot and {name}@ grammars are gone)
    assert_eq!(
        run("7 .x ! 'file.x {x} $x .x' INTERPOLATE"),
        s("file.x {x} $x .x")
    );
    // \${ escapes a literal hole
    assert_eq!(run(r"7 .x ! '\${x} = ${x}' INTERPOLATE"), s("${x} = 7"));
}

#[test]
fn test_interpolate_holes_are_names_not_expressions() {
    // The injection-safety rule: a non-name body is a hard error, so
    // templates can never execute Forthic
    let err = run_err("'${1 +}' INTERPOLATE");
    assert!(err.to_string().contains("not expressions"), "got: {err}");
    let err = run_err("'${x:-default}' INTERPOLATE");
    assert!(err.to_string().contains("not expressions"), "got: {err}");
    // __ names are reserved (same contract as ! / @)
    let err = run_err("'${__secret}' INTERPOLATE");
    assert!(err.to_string().contains("__secret"), "got: {err}");
}

#[test]
fn test_interpolate_containers_render_as_json() {
    assert_eq!(
        run("[ [ 'a' 1 ] ] REC .rec ! '${rec}' INTERPOLATE"),
        s(r#"{"a":1}"#)
    );
    // Arrays join with the separator option
    assert_eq!(run("[ 1 2 ] .items ! '${items}' INTERPOLATE"), s("1, 2"));
    assert_eq!(
        run("[ 1 2 ] .items ! '${items}' [ .separator ' | ' ] ~> INTERPOLATE"),
        s("1 | 2")
    );
    // json option renders any value as compact JSON
    assert_eq!(
        run("[ 1 2 ] .items ! '${items}' [ .json TRUE ] ~> INTERPOLATE"),
        s("[1,2]")
    );
}

#[test]
fn test_interpolate_null_template_stays_null() {
    assert_eq!(run("NULL INTERPOLATE"), ForthicValue::Null);
}

// ===== PRINT (shares INTERPOLATE's holes and rendering) =====

#[test]
fn test_print_pushes_nothing() {
    let mut interp = Interpreter::standard("UTC");
    interp.run("1 'msg ${x}' PRINT").unwrap();
    assert_eq!(interp.get_stack().items(), &[ForthicValue::Int(1)]);
    // Non-strings and options are accepted
    let mut interp = Interpreter::standard("UTC");
    interp
        .run("[ 1 2 3 ] [ .separator ' | ' ] ~> PRINT")
        .unwrap();
    assert!(interp.get_stack().items().is_empty());
}

#[test]
fn test_print_rejects_expression_holes_too() {
    let err = run_err("'value: ${6 * 7}' PRINT");
    assert!(err.to_string().contains("not expressions"), "got: {err}");
}
