# Forthic Rust Runtime

A Rust implementation of the Forthic stack-based concatenative programming language.

## Overview

Forthic is a stack-based, concatenative language designed for composable transformations. This is the official **Rust** runtime, sharing a cross-runtime contract with the [TypeScript reference implementation](https://github.com/forthix/forthic-ts): the same words, the same semantics, the same wire format — so Forthic programs and the systems that host them can move between runtimes.

**[Learn more at forthix.com →](https://forthix.com)**

## Quick start

```rust
use forthic::prelude::*;

fn main() -> Result<(), ForthicError> {
    let mut interp = Interpreter::standard("America/Los_Angeles");
    interp.run("[ 1 2 3 4 5 ] '2 *' MAP SUM")?;
    let result = interp.get_stack_mut().pop()?;
    assert_eq!(result, ForthicValue::Int(30));
    Ok(())
}
```

The interpreter is deliberately **synchronous** — words execute on the calling thread with no async runtime required. Async lives only at transport edges (like the JSON-RPC server below), which wrap the interpreter in `spawn_blocking`.

## Language highlights

* **Words and modules**: `: NAME ... ;` definitions, `{module ... }` scoping, `USE-MODULES` imports (optionally prefixed)
* **Records and JQ paths**: `[["k" "v"]] REC`, with data-driven path access — `record 'a.b[0]' JQ@` (paths are data, never interpolated source)
* **Error handling as data** (Rust `Result` semantics): `'CODE' TRY` yields `{"ok": value}` or `{"error": {...}}`; `'CODE' TRY UNWRAP ≡ CODE`. Error-tolerant mapping via MAP's `.outcomes` option
* **Injection-safe interpolation**: `"Hello ${name}!" INTERPOLATE` — holes are variable names only, never expressions, with read-only lookup
* **Word options**: `[.with_key TRUE] ~> MAP`, `[.separator " | "] ~> PRINT`

## Standard library modules

* **core**: stack ops, variables, control flow, TRY family, INTERPOLATE/PRINT, USE-MODULES
* **array**: MAP, SELECT, SORT, GROUP-BY, ZIP, and the rest of the higher-order vocabulary
* **record**: REC, JQ@/JQ!/JQ-DEL, MERGE, PICK/OMIT, entry conversions
* **string**: SPLIT/JOIN, substrings, regex (RE-MATCH etc.), shell-flavored text tools (GREP, SED, CUT, LINES)
* **math**: arithmetic, aggregates (SUM, PRODUCT, MEAN), SQRT/CLAMP, FORMAT-FIXED
* **boolean**: comparison, logic, membership
* **datetime**: timezone-aware dates and times (via `chrono` / `chrono-tz`), date math, components
* **json**: serialization and parsing (via `serde_json`)

## JSON-RPC server

The `jsonrpc` cargo feature adds a hardened HTTP JSON-RPC 2.0 server compatible with the forthic-ts client:

```bash
cargo run --features jsonrpc --bin forthic-jsonrpc -- --port 8765
# forthic-jsonrpc [--port 8765] [--host 127.0.0.1] [--token SECRET]
```

Defaults are conservative (loopback only). The server executes caller-supplied Forthic code — binding a non-loopback host without `--token` logs a security warning.

## Cross-runtime notes

Runtime behavior is aligned with forthic-ts, with a small set of documented, deliberate divergences:

* **Host-native string units**: rs measures strings in Unicode code points; ts uses UTF-16 code units. They agree on all BMP text and diverge only on astral characters (`'🦀' STR-LENGTH` is 1 in rs, 2 in ts)
* **Strict parsing**: no `parseInt`/`new Date()` leniency — malformed numbers and dates are errors or NULL, never guesses
* **Insertion-order records**: records preserve insertion order (ts inherits JS integer-key hoisting)
* **"null", never "undefined"**: rs has no `undefined`; ts's `UNDEFINED` word is a documented host-interop word that does not cross the wire

See `plans/WORD-INVENTORY.md` for the word-by-word porting map and `plans/TS-PARITY-BACKLOG.md` for open parity items.

## Development

```bash
make test          # cargo test --all-features
cargo fmt --check
cargo clippy --all-features --all-targets -- -D warnings

# Cross-runtime smoke: drives this server with the real forthic-ts client
# (needs a built forthic-ts checkout; FORTHIC_TS_DIR defaults to ../forthic-ts)
make smoke-ts
```

## License

BSD 2-Clause

## Links

* **[forthix.com](https://forthix.com)** — Learn about Forthic and Categorical Coding
* **[Category Theory for Coders](https://forthix.com/blog/category-theory-for-the-rest-of-us-coders)** — Understand the foundations
* [Forthic Language Specification](https://github.com/forthix/forthic)
* [TypeScript Runtime](https://github.com/forthix/forthic-ts) (reference implementation)
