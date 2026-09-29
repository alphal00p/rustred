# Runtime discovery strategies and persisted integral orders

Date: 2026-09-29. Status: architecture proposal; no implementation or performance
claim. The current frozen `3428b519` campaign executable, its validation, LC2,
and every saved campaign remain independent of this proposed work.

The user wants richer runtime choices without rebuilding the full engine for
each experiment. There are two distinct deliverables. A changes discovery and
row-arrival choices. B changes the earlier integral priorities that determine
column pivots and accepted descent. **A alone does not implement B.** A necessary
framework build is different from subsequent parameter/input experiments,
which should reuse that compiled interpreter.

## Existing surfaces and authority boundaries

- `crates/rustred-core/src/solver/search.rs` already provides
  `solve_case_with_source_order_and_observer`. It validates a complete basis-row
  permutation before work and keeps `SeedSource::basis_row` as the original
  stored ordinal. Its present scope is an isolated-case diagnostic.
- `solver/sector.rs` still calls ordinary `solve_case_with_observer`;
  `solver/numeric.rs` separately visits `self.basis` in the shared finite tail.
  A new general row strategy must cover both, not only symbolic search.
- `solver/execution.rs` already has `SectorScheduling::{InputOrder,ActiveFirst}`.
  Job ordering is separate from original sector ordinals and result ordering.
  Rayon may alter actual start/completion chronology; a priority is not a
  serial dependency barrier.
- `solver/index.rs::IntegralOrder` currently compares sector/cut priorities,
  aggregate degrees, then coordinate ties. It drives preconditioning, modular
  discovery columns and exact lifting. Existing `--permutation` changes only
  the final coordinate ties.
- `sector/ordering/{policy,comparison}.rs`, `sector/ordering.rs` and
  `sector/shift_ordering.rs` define persisted concrete/shift order and descent
  witnesses. Candidate loading maps the saved permutation into the uncut
  SpIRed policy; `foundry/artifact/source_port` replays source combinations and
  checks this order. Changing only the discovery comparator is insufficient.

Neither strategy labels, callback results, modular success, nor a favorable
benchmark confer rule or closure authority. Exact source replay, coefficient
conditions, exceptional-domain coverage, descent and independent scoped cold
verification remain separate gates.

## A. Finite, materialized discovery strategy

Add a bounded, canonical `DiscoveryStrategyV1` runtime descriptor with two
independent components:

1. Sector job priority: retain input/active-first choices, with an optional
   weighted support score and deterministic original-ordinal tie. Keep the
   manifest's original sorted sector list and all output identities unchanged.
2. Source-row priority: a bounded lexicographic list of feature/direction pairs,
   evaluated once on each sector's immutable preconditioned basis. Examples
   are row term count, total coefficient monomial count, and weighted absolute,
   positive or negative shift degree. Original basis ordinal is the final tie.

These weighted features rank a **finite row inventory**. They are not a new
well-founded order on all integrals. No row is dropped, no seed shell is clipped,
and ordinary first-hit stopping remains explicit. Scores use bounded integer
parameters and checked arithmetic; equal scores cannot inherit hash-map order.
Cache a validated permutation, not a callback invocation per term or CAS step.
The ordinary default retains its contiguous basis traversal.

### Rust callbacks and Python selection

A Rust caller may supply a one-shot callback over immutable row/sector features
that returns keys or an ordinal permutation. Evaluate it once and materialize
the complete finite plan. Validate bijectivity, bounds and deterministic final
ties, then bind the result to the family, sector inventory, mathematical order,
prepared basis identity and generation recipe. A callback can be stateful or
opaque without becoming authority: only its validated result is retained.

Persisting that materialized plan allows resume without rerunning or trusting
the original closure. Use the existing bounded/atomic checkpoint machinery;
the plan must be durable before search whose continuation depends on it. On
resume, regenerate/check the prepared basis identity before applying stored
ordinals. Original source IDs and exact replay traces remain unchanged.

A callback that changes with cases, seeds, elapsed time or observed algebra is
different: its potentially unbounded future decisions cannot be replaced by a
single finite permutation. Initially reject such callbacks for resumable
generation, or restrict them to a finite validated declarative interpreter.
Do not serialize executable closures, accept Python `eval`, load plugins, or
pretend a user-supplied name reconstructs an opaque function.

Python selects named/parameterized descriptors and invokes the same generic
Rust evaluator through existing CLI/application machinery. It need not run
inside an elimination loop. Changing descriptor parameters or a materialized
plan requires input regeneration but not recompiling the engine. A new Rust
callback implementation may require compiling its small caller; it does not
require rebuilding all generic solver machinery.

### Persistence and scope

Keep discovery recipe separate from mathematical generation scope and proof
ordering. Record the canonical descriptor/materialized-plan identity in the
generation checkpoint and report. The selected source rows already provide
mathematical provenance in candidate records; a recipe is not a replacement
for that provenance. Do not overload `solver_policy` so that two mathematically
compatible owners become incompatible merely because their row schedules differ.

There is no requirement here to preserve every historical RustRed schema.
Choose a clear new generation version if necessary and reject incompatible
resumes explicitly. The invariant is that existing frozen executables and
campaigns are not rewritten or silently reinterpreted.

### Smallest implementation and tests

Core surfaces: `solver/search.rs`, a small strategy module, `solver/numeric.rs`,
`solver/execution.rs`, and their existing tests. Application surfaces:
`candidate_bundle/{model,generate,policy,checkpoint/manifest}.rs`, CLI argument
binding, generation reports, and a thin Python selector. Estimate approximately
400–700 lines including focused tests, subject to exact persistence decisions.
No CP6 walker, admission, resolver or production checkpoint change is needed.

Test identity/default equivalence; invalid/missing/duplicate ordinals rejected
before observed work; deterministic ties; row conservation; original source IDs
after reordering; direct-hit and modular exact replay; shared finite-tail use;
W1/multiworker canonical output; materialized callback resume without callback;
and altered recipe/basis/plan refusal. Then one bounded public generation and
existing native loader/source-replay checks. Different first hits may change
rules and terminal inventories, so exact terminal keys and full selected-scope
closure/performance are measured outcomes, never presumed equal.

## B. Persisted declarative integral-order program

A finite validated `OrderProgramV1` can express earlier priorities without
accepting arbitrary unpersistable comparison functions. One useful first
language, in simpler-first notation, is:

1. Support cardinality. Keep this first so every strict pinch lowers the key.
2. A parameterized positive weighted support score, followed by an explicit
   coordinate/mask priority, among equal-cardinality supports. This changes
   sector priorities before within-sector aggregate degrees.
3. A bounded lexicographic list of nonnegative integer weighted excess-degree
   rows. For physical index `n`, use excess `n-1` on an active coordinate and
   `-n` on an inactive coordinate. These values are nonnegative in their sector.
4. A validated coordinate permutation and explicit forward/reverse tie direction,
   with the denominator and numerator tie groups stated separately. The current
   default compares denominator ties before numerator ties, with their existing
   signs; a single interleaved coordinate lexicographic comparison is not an
   equivalent default. Encode the group priority in the descriptor and test it
   against `IntegralOrder::compare_coordinate_ties`.

Require every excess coordinate to have a positive coefficient in at least one
aggregate row. Thus fixing the entire aggregate tuple leaves only finitely many
coordinate vectors. The aggregate tuple lies in a finite lexicographic product
of natural numbers, which is well-founded; reversed coordinate ties act only
inside finite fibres. Finite support choices precede it. This gives an actual
well-foundedness argument, not just finite test evidence.

No negative aggregate weights, floating scores, user comparators, live timing
feedback, or incomplete coordinate coverage are admitted. Bound arity, number
of rows and integer weights; use checked `u128` concrete and `i128` shift-offset
arithmetic, rejecting overflow. An all-zero row is redundant and should be
rejected or removed canonically. Canonicalize equivalent identity/default forms.
Full descriptor bytes define semantics; a caller-provided label alone does not.

### Symbolic shifts and cuts

On a fixed support and symbolic/numeric pattern, each weighted excess sum at
`n+shift` has the same symbolic `n` contribution for both candidates. Comparing
the remaining signed weighted offsets must agree exactly with concrete keys.
The same evaluator and weights must be used for both; boundary sign changes
still require the existing sector/domain splitting, not extrapolation of an
interior comparison. Preserve the existing numeric/symbolic-pattern checks.

Current discovery also has delta/cut priority before aggregate degrees, while
the persisted candidate adapter explicitly uses an **uncut** policy. Therefore
the first B implementation should reject nonempty delta/removed-delta
configurations rather than silently claim cut support. A later cut-capable
descriptor must encode its cut mask, removed-coordinate constraints and fixed
cut-priority prefix, and replay those semantics in every concrete/shift proof
consumer. A may keep existing cut handling unchanged because it only reorders
row visits; it must not alter fixed-coordinate or removed-delta checks.

### Representation, codec and proof impacts

`OrderingPolicy` is currently a small `Copy` enum with a packed permutation.
A variable-length program cannot simply be added as an unchecked string. Choose
either a deliberately bounded packed payload or an immutable validated compiled
program with an explicit reference/ownership refactor. Review comparison/key
allocation and policy-copy costs before choosing; avoid a hidden process-global
registry whose numeric handles are mistaken for portable identities.

B touches `IntegralOrder` and its preconditioning/discovery/exact consumers;
`OrderingPolicy`, concrete and shift complexity keys, decisive-component
witnesses; candidate request/record/load paths; source-port cold replay; and
artifact/owner compatibility checks. New order records need explicit versioning
and fail-closed parsing. A wrong or unknown order on resume/load is an error,
not a fallback. No general legacy-schema compatibility layer is required, but
current frozen campaigns remain untouched and new semantics cannot reuse an
old identity. Physical integral coordinates are not renumbered by this program.

Tests must cover finite-cube totality/antisymmetry/transitivity, strict-pinch
descent, weighted-level finite-fibre conditions, all supported concrete versus
symbolic-shift comparisons, overflow and malformed descriptors, exact stable
identity round trips, wrong-order cold-load rejection, default semantic
equivalence, exact lifted-row descent, source replay and terminal inventory.
These supplement the mathematical argument rather than replacing it. A
complete bounded owner-generation→load→walk→cold-All experiment is necessary
before claiming a usable new order. Terminal count alone is not equivalence.

## Symbolica and implementation sequencing

No new coefficient algebra is needed for A's initial features. The pinned
Symbolica polynomial `nterms()` returns the stored coefficient-vector length
(`vendor/symbolica/src/poly/polynomial.rs`); native `exponents()` already supplies
exponent slices used by `solver/precondition.rs`. Reading term counts and
bounded degree features is sufficient. Do not add factoring, GCD, simplification,
CAS substitutes or coefficient serialization to the priority hot path. Any
later algebraic feature must use native Symbolica APIs with separately measured
preparation cost and cancellation/resource bounds.

Recommended sequence: finish current3428 lifecycle/qualification gates; isolate
A and execute its tests; independently review the B descriptor/proof design;
then decide whether B's broader implementation fits the user's next work slice.
One framework build per implemented capability is legitimate. Recompiling the
engine for each named strategy, weight vector or coordinate priority is not.
No speed, termination, closure or optimal-order claim follows from this design.
