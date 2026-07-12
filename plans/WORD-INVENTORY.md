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
UNDEFINED (JS-specific), MAP interps option (parallel interpreters — rs is
deliberately synchronous).

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

**Batch 3 — records & JQ paths (rs record module is thinnest):**
JQ@ (null on miss, [] iterates+flattens), JQ! (auto-creates, no []), JQ-DEL,
MERGE (shallow, rec2 wins), PICK, OMIT, HAS-KEY?, DELETE (supersedes <DEL),
REC>ENTRIES (sorted-by-key per current docstring — note tension with #33
insertion order; current ts is the spec), ENTRIES>REC (alias, low priority).

**Batch 4 — strings & interpolation:**
INTERPOLATE (string version: {.var}@ holes), STR-LENGTH, SUBSTR, SPLICE,
STARTS-WITH?/ENDS-WITH?, TRIM-PREFIX/TRIM-SUFFIX, RE-MATCH? / RE-MATCH
(decide rs match representation) / RE-MATCH-ALL / RE-REPLACE (REPLACE stays
literal), LINES/UNLINES, GREP/GREP-V, SED, CUT. Regexes are documented
trusted input (#34).

**Batch 5 — math & datetime round-out:**
PRODUCT, SQRT, CLAMP, FORMAT-FIXED, AM/PM, DAYS-BETWEEN, YEAR, MONTH
(1-based), DAY-OF-WEEK (ISO 1=Mon), USE-MODULES (may belong with runtime
work). Verify rs >DATE vs ts #35 (absolute instants in interpreter tz) and
NOW vs #29.

## Present-but-verify list

TAKE (rs lacks with_key), OR/AND (verify rs rejects arrays), >BOOL edge
cases, SLICE allocation bound (#34), MEAN input breadth, @ unknown-variable
error parity.
