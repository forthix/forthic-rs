# TS-Parity Backlog

Derived from the July 2026 forthic-ts correctness/security scrub (PRs #20–#33)
plus general repo state. Every item below was verified against the forthic-rs
code (file:line), not assumed from the ts diffs. The post-fix ts behavior is
the spec for anything ported.

## Tier 1 — Real bugs in forthic-rs today (small fixes, high value)

> **Status: all five fixed** (fix/tier1-correctness branch, with regression
> tests in tests/tier1_correctness_test.rs).

1. **Error formatter can abort the process.** All three caret builders do
   unchecked `end_pos - start_pos` (`errors.rs:296-298`, `333-335`, `351-353`).
   A degenerate CodeLocation (`end_pos < start_pos`, constructible by any
   word/module — fields are pub) panics in debug (subtract overflow) and in
   release (capacity overflow in `"^".repeat`). Formatting an error must never
   crash. Fix: `saturating_sub(...).max(1)`, ideally one shared caret helper.
   (ts #26 "crash-proof formatter" parity.)
2. **Temporal values are never equal.** `values_equal` (`boolean.rs:345-362`)
   has no arms for `Date`/`Time`/`DateTime`/`Record` → falls to `false`.
   `TODAY TODAY ==` is `false`; also poisons `IN`/`ANY`/`ALL`. chrono types
   are `PartialEq` — trivial arms. (ts #29 parity.)
3. **`ANY` returns true for empty second array** (`boolean.rs:282-286`) —
   the exact bug ts fixed in #31, ported verbatim. Delete the special case.
4. **`IntentionalStop` loses its identity inside definitions.**
   `DefinitionWord::execute` wraps every inner error in `WordExecution`
   (`module.rs:242-254`); hosts matching on IntentionalStop after a debug
   stop won't match. `ModuleWord::execute` rethrows it but discards the
   original message (`module.rs:492-497`). (ts #26 parity.)
5. **`NOW` and `TODAY` can disagree on what day it is.** NOW hardcodes UTC
   (`datetime.rs:66-70`); TODAY uses host-local `Local::now()`
   (`datetime.rs:60-64`). The interpreter has a timezone
   (`interpreter.rs:263`) but doesn't expose it through `InterpreterContext`.
   Plumb it through and use it in both. (ts #29 parity.)

## Tier 2 — Semantic divergences from the post-scrub ts contract

> **Status: all four fixed** (fix/tier2-record-semantics branch, tests in
> tests/tier2_record_semantics_test.rs). FIRST and TAKE-LAST also landed
> (were Tier 4 item 14) since the #33 contract covered them.

6. **`Record` should be `IndexMap`, not `HashMap`** (`literals.rs:29`).
   ts #33 made record words rely on insertion order; HashMap has none, so
   rs sorts keys in `NTH`/`LAST` (`array.rs:112,139`) and emits
   nondeterministic `KEYS`/`VALUES`/`>JSON` order. Cross-runtime: a record
   through an rs RPC comes back reordered. Switch to `indexmap::IndexMap`
   (drop the sorts), then this is free.
7. **`>STR` diverges** (`string.rs:63-77`): `Null` → `"null"` (post-#31 ts:
   `""`); containers/dates → Rust `{:?}` debug output. Align with ts.
8. **Container words silently no-op on records.** `TAKE`/`DROP`/`SLICE`/
   `UNPACK` return records unchanged (`array.rs:199-200, 217-224, 240-247,
   545-557`); `DIFFERENCE`/`INTERSECTION` return `[]` for record operands
   (`array.rs:406-444`) instead of the #31 PICK/OMIT semantics. Implement
   record arms per post-fix ts (record in → record out). Depends on item 6.
9. **`reset()` doesn't clear `tokenizer_stack`** (`interpreter.rs:473-479`).
   Latent today, cheap insurance. (ts #26 parity.)

## Tier 3 — Infrastructure

10. **No CI.** ts added build/test/smoke in #24; rs has no
    `.github/workflows` at all. Add: `cargo build`, `cargo test
    --all-features`, `cargo fmt --check`, `cargo clippy` on stable.
11. **Box the ForthicError payload.** The enum is large (source snippets +
    locations in every variant), so every `Result<_, ForthicError>` moves
    ~hundreds of bytes on the happy path too (clippy: result_large_err,
    allowed at crate level for now). Standard fix: box the big fields or the
    whole error. Mechanical but touches every error site.
12. **Tokenizer mixes byte and char indices** (`tokenizer.rs:142-151`
    and around): `input_string.len()` (bytes) vs `chars().nth` — can
    misbehave or index out of bounds on multibyte UTF-8 input. Deserves a
    dedicated robustness pass with UTF-8 tests.

## Tier 4 — Missing features (port later; post-fix ts is the spec)

13. **Word locations**: per-definition location tracking (ts #30 design:
    `word_locations` vec parallel to `words` in DefinitionWord, thread
    `token.location` through `handle_word`) + the interpreter-error
    `with_location` work already listed in JSONRPC-PLAN follow-ups. Note the
    ts *race* (shared Word mutated at compile time) is uncompilable in rs
    (`&mut self` through `Arc` is refused) — only the feature is missing.
14. **Missing stdlib words**: `MAP`, `SORT`, `FIRST`, `TAKE-LAST`,
    `NUMBER?`, JQ path words, etc. Port to post-#31/#32/#33 contracts
    (e.g. MAP's fixed depth semantics), never pre-fix behavior. Never port
    `|REC@` (removed in #27 for injection).
15. **Marked-string redirect + streaming** (ts #20 + #21 + #26 EOS
    validation) — port as one coherent unit. First fix the dormant rs
    streaming tokenizer's ambiguity: it returns incomplete strings as
    normal String tokens (`tokenizer.rs:467-474, 509-515`), which will
    double-push if streaming is ever wired up as-is.
16. **Recovery loop** (ts #26 fixed semantics: budget check before the
    recoverable region; never recover from TooManyAttempts).
    `ForthicError::TooManyAttempts` exists but is dead code today.

## Immune — do not port (Rust semantics already close these)

- **Copy-on-write aliasing (ts #32)**: rs values are owned; variable fetch
  deep-clones (`module.rs:643-646`, `core.rs:184-186`). Aliasing between a
  variable and the stack is impossible. Optional: port the ts regression
  tests to pin the semantics.
- **`|REC@` injection (ts #27)**: never ported; no rs module builds Forthic
  source by string interpolation (words are native fns).
- **Prototype pollution / prototype-less registries (ts #23, #28)**:
  HashMap has no prototype chain. JS-specific.
