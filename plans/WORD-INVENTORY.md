# Word Inventory: forthic-ts vs forthic-rs (2026-07-11)

Full audit of both standard libraries. ts canonical = 169 unique words
(8 modules) + 34 classic words; rs = 106 unique words. Missing from rs
(canonical, portable): ~75 words. Word-name extraction verified against the
ts decorator implementation; both runtimes resolve bare-name collisions
last-registration-wins.

## CRITICAL: same-name-different-meaning collisions (fix before porting)

| Word | ts meaning | rs meaning |
|---|---|---|
| DROP | core: pop top of stack | array: skip first n (= ts SKIP). `1 2 DROP` and `[1 2 3] 2 DROP` mean opposite things across runtimes. rs's pop is POP (the ts CLASSIC name). |
| CONCAT | string: ( strings[] -- str ) one-arg join | Registered TWICE in rs: array.rs two-array concat + string.rs variable-arity version. StringModule registers last, so rs's array CONCAT is unreachable by bare name. The two-string fallback has arity instability ts deliberately removed. |
| RANGE | math: inclusive, EMPTY if start > end, allocation-bounded (ts #34) | rs array module: start > end yields a reversed descending range; no allocation bound. |
| FLATTEN | full flatten by default, depth option, records | rs: exactly one level, arrays only, no options. Quiet data corruption on nested input. |
| INTERPOLATE | registered in BOTH ts core (`.name` style) and ts string (`{.var}@` style); string registers later and WINS | absent in rs. The string version is the porting spec. |

Naming divergences (same semantics, different name): rs POP = ts DROP;
rs DROP = ts SKIP; rs <DEL = ts DELETE; rs SUBTRACT-DATES = ts DAYS-BETWEEN;
rs IN (item-first) = ts CONTAINS? (haystack-first, args reversed).

## ts classic module — do NOT port (owner-confirmed concept)

ts's classic_module.ts (34 words) exists explicitly for back-compat, with
canonical siblings: ADD/SUBTRACT/MULTIPLY/DIVIDE (→ + - * / + SUM/PRODUCT),
POP (→ DROP), INTERPRET (→ RUN), *DEFAULT (→ DEFAULT-RUN), SELECT (→ FILTER),
IN (→ CONTAINS?), <REPEAT (→ TIMES-RUN), <DEL (→ DELETE), >FIXED
(→ FORMAT-FIXED), REC-DEFAULTS (→ MERGE), SUBTRACT-DATES (→ DAYS-BETWEEN),
plus PROFILE-* (ts-runtime tooling) and a tail without designated siblings
(SHUFFLE, ROTATE, INFINITY, UNIFORM-RANDOM, RE-MATCH-GROUP, XOR, NAND,
RELABEL, INVERT-KEYS, JSON-PRETTIFY, /R, URL-ENCODE, URL-DECODE, DATE>INT).

**rs already carries 15 classic words** — DECIDED (2026-07-11): drop them
as their canonical replacements land, no aliases (rs is pre-1.0). POP and
IDENTITY dropped in Batch 0 (DROP/NOP are the canonical names). Remaining
with-sibling classics drop with their batch: IN (→ CONTAINS?, batch 1),
<DEL (→ DELETE, batch 3), REC-DEFAULTS (→ MERGE, batch 3), SUBTRACT-DATES
(→ DAYS-BETWEEN, batch 5). The 9 no-sibling classics (XOR, NAND, RELABEL,
INVERT-KEYS, DATE>INT, JSON-PRETTIFY, /R, URL-ENCODE, URL-DECODE) stay —
dropping them would remove functionality with no replacement.

## Never port

|REC@ (removed ts #27, injection), push_error (removed ts #38 → outcomes/TRY),
UNDEFINED (DECIDED 2026-07-11: stays in ts as a documented JS-only
host-interop word — serializes as null on the wire; rs never implements it,
and UnknownWord is the honest non-portability signal; portable programs use
NULL), MAP interps option (parallel interpreters — rs is deliberately
synchronous).

## Porting batches (canonical words only)

**Batch 0 — collision fixes first: DONE (feat/word-batch0):**
array DROP → SKIP; core POP → DROP; IDENTITY dropped (NOP remains); array
CONCAT deleted + string CONCAT array-only (two-string form rejected with a
helpful message); RANGE empty on start > end + #34 allocation bound;
FLATTEN full-depth default + depth option + records as tab-joined key
paths. (ts TAKE's with_key turned out to be declared-but-dead in ts —
nothing to port.)

**Batch 1 — control flow & predicates: DONE (feat/word-batch1):**
IF (pure value selection), IF-RUN, WHEN, RUN, DEFAULT-RUN, NULL?, EMPTY?,
STRING?, NUMBER? (Infinity yes, NaN no), RECORD?, ANY? (false on empty),
ALL? (true on empty), CONTAINS? (haystack-first; classic IN dropped),
PEEK!, STACK!. Bonus: is_truthy moved to ForthicValue with two JS-parity
fixes (empty arrays are truthy, NaN is falsy — affects >BOOL/IF/ANY?).
PRINT + core INTERPOLATE deferred to Batch 4: they share the
variable-interpolation machinery with string INTERPOLATE.

**Batch 2 — higher-order & sorting: DONE (feat/word-batch2):**
FILTER, FOREACH (no push_error — 'W' TRY FOREACH per #38), REDUCE, FIND,
COUNT, SORT (comparator option, CoW #32), SORT-BY, MIN-BY/MAX-BY (null on
empty), UNIQUE-BY (keeps first), SORT-U, GROUP-BY, GROUP-BY-FIELD, BY-FIELD,
GROUPS-OF, INDEX, KEY-OF, NUMBERED, ZIP-WITH, TIMES-RUN (2-arg), MAP-AT
(single key or path array, jq |=). Sanctioned deviations (documented in
code): one shared natural_cmp total order instead of JS relational
coercion (NULL sorts last); structural values_equal instead of === for
KEY-OF/UNIQUE-BY/SORT-U dedupe; value_to_key_string for group-key
coercion; fractional counts truncate. SORT's comparator option is a KEY
FUNCTION (the ts docstring's "SWAP -" example is stale).

**Batch 3 — records & JQ paths: DONE (feat/word-batch3):**
JQ@ (null on miss, [] iterates+flattens conditionally), JQ! (auto-creates
by NEXT-segment kind, no [], pads arrays with NULL), JQ-DEL (silent no-ops,
no []), MERGE (shallow, rec2 wins), PICK, OMIT (drop keys stringify — ts's
=== Set bug fixed by design), HAS-KEY? (presence, not non-null), DELETE
(copy-on-write; classic <DEL dropped — note it MUTATED in place), REC>ENTRIES
+ ENTRIES>REC (round-trip identity). Classic REC-DEFAULTS dropped for MERGE
(migration note: REC-DEFAULTS also overrode NULL/"" values). Documented
divergences: records iterate/index/enumerate in INSERTION order everywhere
(ts sorts as a JS-object-order workaround — including REC>ENTRIES); strict
integer parse in [n] (no parseInt leniency). Also this batch: the
register_words! macro (backlog item 22 Tier 1) replacing all registration
tables.

**Batch 4 — strings & interpolation: DONE (feat/word-batch4):**
STR-LENGTH, SUBSTR, SPLICE (JS-slice clamping over CHAR indices —
host-native units, item 18), STARTS-WITH?/ENDS-WITH?,
TRIM-PREFIX/TRIM-SUFFIX (one occurrence), RE-MATCH? / RE-MATCH (array
[full, g1, ...], NULL for non-participating groups AND for no-match/null
input — ts pushes false there, both falsy) / RE-MATCH-ALL (group 1 else
full match) / RE-REPLACE (REPLACE stays literal; JS backrefs $&/$n
normalized to ${n} for the rs engine), LINES/UNLINES, GREP (matching
strings only) / GREP-V (keeps non-strings — deliberate asymmetry), SED
(non-strings pass through), CUT (literal separator; '' splits into chars;
out-of-range field -> NULL).

INTERPOLATE + PRINT (Batch 1 deferrals) — REDESIGNED with Rino
2026-07-11, replacing both the ts bare-dot grammar and the `{.var}@`
grammar with ONE contract in both runtimes (core module; the string
module's INTERPOLATE is gone):
- `${name}` holes (ts-template-literal style; `${.name}` dot-symbol
  spelling also accepted; body whitespace trims). `\${` escapes.
- Holes are variable names ONLY — a non-name body (`${1 + 2}`,
  `${x:-default}`) is a hard error, so templates can NEVER execute
  Forthic (injection-safe by construction; same reasoning as JQ paths).
  `__` names reserved, as with ! / @.
- READ-ONLY lookup via new InterpreterContext::find_variable_value
  (module-stack walk) — a miss renders as null_text and creates nothing
  (no @-style get-or-create; typos can't mint variables).
- Rendering: null_text default "" (template-first — misses/NULLs render
  empty; opt into '[.null_text "null"]'); arrays join with separator
  (", "), elements recursively; records -> compact JSON; [.json TRUE]
  renders anything as compact JSON.
- PRINT = same options + rendering; strings interpolate first; pushes
  nothing; stdout is safe under the jsonrpc transport (HTTP, not stdio).

Regexes are documented trusted input (#34) — rs regex is linear-time, so
the ReDoS caveat is ts-only; compile failures are clean InvalidOperation
errors (ts throws raw SyntaxError).

**Batch 5 — math & datetime round-out: DONE (feat/word-batch5):**
PRODUCT (empty -> 1; deliberate ts asymmetry with SUM: non-array -> NULL
and a NULL/non-numeric element NULLs the whole result), SQRT (negative ->
NaN, not an error), CLAMP (exactly max(min, min(max, value)) — min WINS
when min > max; NaN propagates like JS, pinned against rust's
NaN-swallowing f64::min/max), FORMAT-FIXED (JS toFixed: half-AWAY-from-
zero ties, pinned against rust's ties-to-even; digits outside 0..=100 and
non-numeric num ERROR like ts; >=1e21 exponential quirk not reproduced),
AM/PM (Time and DateTime adjust; everything else passes through
UNCHANGED, not NULL), DAYS-BETWEEN (pure rename of classic SUBTRACT-DATES
— same operand order, same date1-date2 sign; classic name dropped, the
LAST scheduled classic drop), YEAR / MONTH (1-based, confirmed both
sides) / DAY-OF-WEEK (ISO 1=Mon..7=Sun via number_from_monday; Date or
DateTime, strings -> NULL), USE-MODULES (entries 'name' or
['name' 'prefix']; [.prefixed TRUE] self-prefixes plain names; explicit
pair prefix ALWAYS beats the option; imports into app module + live
module_stack clone via new InterpreterContext::use_module; unknown name
-> UnknownModule).

Verify items resolved: NOW already matched ts #29 (DateTime carries the
interpreter tz; rs-only nuance: unparseable tz name falls back to UTC
where ts throws — documented). >DATE had four #35 gaps, all closed: trim;
ISO datetimes without zone and with explicit numeric OFFSET take the date
AS WRITTEN; trailing-Z instants resolve in the INTERPRETER timezone;
month-name forms ("Oct 21, 2020") parse. ts's arbitrary new Date()
leniency beyond that (e.g. bare "20240115") is NOT reproduced —
sanctioned strict-parsing divergence. `0 >DATE` stays NULL (deliberate ts
falsy asymmetry with `0 >DATETIME` = epoch). Also fixed:
number_to_value now collapses to Int only within the f64-exact range
(2^53) — beyond that `as i64` silently saturated.

## Present-but-verify list

TAKE (rs lacks with_key), OR/AND (verify rs rejects arrays), >BOOL edge
cases, SLICE allocation bound (#34), MEAN input breadth, @ unknown-variable
error parity. NEW (Batch 5 spec sweep): rs >DATETIME / AT /
TIMESTAMP>DATETIME hardcode UTC where ts resolves in the interpreter
timezone (context_tz helper is right there — cheap fix, unscheduled).
