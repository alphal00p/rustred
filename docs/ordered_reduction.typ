= Ordered weighted reduction

The opt-in ordered scheduler applies the existing authenticated programs to a
global weighted frontier. By default it uses exact coefficients (see Weight
arithmetic). Pending weights are indexed by key and executed in BTreeSet order of
their certified position, so a merge into a pending key reuses that position. It
does not generate identities, change dispatch priority, rewrite artifacts, or
import a depth-first completed-result cache. Loop count is data rather than a
scheduler specialization.

== Selection and shared semantics

```rust
use rustred::reduction::ReductionStrategy;
use rustred_app::SavedApplicationOptions;

let options = SavedApplicationOptions {
    reduction_strategy: ReductionStrategy::Ordered,
    ..SavedApplicationOptions::default()
};
// An already prepared application also has set_reduction_strategy(...).
```

The default remains `DepthFirst`. Existing weighted reduction methods retain
their output and error types. Closed artifacts, candidate bundles and saved
owner/route applications have thin adapters to one generic scheduler. Rule
selection, specialization, canonicalization, transport and sufficient nonzero
conditions use the existing checked operations.

For a depth-first timing reference without memoization, select
`DepthFirstWithoutMemoization`. It preserves checked child-occurrence order and
exact back-substitution, consuming each returned child value in its active parent.
Repeated descendants are recomputed, including dependent factor reductions;
completed-result lookup, retention, cache-weight accounting and capture are bypassed. Existing
in-memory DFS results remain untouched, while explicit backing/capture requests
are rejected. Immutable program preparation, guard membership and active-path
cycle detection remain necessary. This reference can do substantially more work
than ordinary DFS; no estimated bookkeeping time is subtracted from its timing.

Every original root is admitted and its original coefficient poles retained
before cancellation. At each step equal canonical operational identities combine
exactly, and zero weights disappear before their descendants are expanded.
Applied conditions remain in the proof even when later weights cancel. Different
valid schedules may require different sufficient conditions. Unknown residuals
are classified after exact final combination; completion is not inferred from
termination alone.

== Weight arithmetic

`OrderedWeightArithmetic` selects how frontier and terminal weights are combined
(`SavedApplicationOptions::ordered_weight_arithmetic`,
`PreparedSavedApplication::set_ordered_weight_arithmetic`, and the
`*_with_arithmetic` entry points of the closed and candidate reducers). The
default `Exact` combines normalized rational functions. Every input weight and
adapter step coefficient is validated once on entry, so merges and products never
rescan their operands, and an exact unit step coefficient copies the weight.

`Modular` (coefficient contexts with at most one variable) runs the scheduler
once as a recorder. Each weight carries two values modulo a probe prime, used only
for the zero-erasure decision, and the propagation is recorded as a straight-line
trace over interned exact step coefficients; each distinct coefficient is
validated once. The trace is replayed over 64-bit primes at batches of sample
points. Per prime, every terminal coefficient is interpolated by Thiele continued
fractions; over Q it is lifted by CRT and rational number reconstruction, accepted
once one further prime confirms it, and checked against fresh primes. Every replay
re-checks each recorded erasure at every point and fails closed. Keys, steps,
children and guards depend only on integer keys and exact step coefficients, so
both arithmetics expand the same keys and return the same terms and conditions;
only the monomial census differs (one per modular weight). `cross_check_exact`
also runs the exact arithmetic in the same pass and fails on any differing zero
decision or terminal coefficient.

== Phases and canonical identities

Saved applications process support sizes downward, draining routing before the
owners at a given size. Within an owner, the saved local order strictly decreases.
Proper pinches move to smaller support. Verified routing may expand a numerator
into several weighted endpoints; all of them are validated, and every endpoint
outside a proved zero sector enters the common canonical frontier. A zero-sector
endpoint keeps the conditions of its validation but carries no weight, exactly as
if it had been expanded. Route and Apply are distinct operational identities.

Closed/candidate artifacts use descending-support phases when their authenticated
order is support-primary. Other orders use one phase with their exact saved key
order. Every emitted nonterminal edge is checked against the scheduler position;
unsafe or non-descending transitions are errors. Existing complete factorization
is an opaque checked action with request-local state; this mode does not promise
cross-root grouping inside that action's dependent reductions.

== Observation and state ownership

The observer reports a fixed phase total, completed phases, current support and
owner, and current/peak operational frontier size. Separate counters describe
global merges and exact frontier versus terminal cancellations. Input and
intra-rule coalescence are not silently counted as global frontier cancellations.
Empty phases complete normally. Phase fraction is a structural measure rather
than elapsed-time percentage or an ETA. Independent requests have separate epochs.

Separate current/peak coefficient-monomial counters sum numerator and denominator
monomials of the resident rational frontier and terminal coefficients. Incremental
updates require no arithmetic, printing or frontier scans, and expose coefficient
swell even when the integral count is unchanged. This size proxy excludes the
active popped coefficient, guards, arithmetic temporaries, keys and byte storage.
Terminal counts precede saved terminal substitutions; process RSS remains the
memory measure.

An optional phase inspector (`*_with_inspector` entry points of the closed,
candidate and saved routed adapters) runs on the caller thread at both boundaries
of every frozen phase, never inside a step. It borrows the scheduler state: every
pending identity in pop order with its stage (integral, Route or Apply with the
owner sector), the terminals accumulated so far and the interned conditions.
Weights are exact values, except under modular arithmetic without the exact
cross-check, where only keys are visible. An inspector error stops the request
through the adapter's typed error; no partial result escapes. Unused, it costs
two no-op calls per phase. The rustred-app example `ordered_replay` uses it to
replay dumped Vakint requests phase by phase and to capture complete frontiers.

The ordered engine does not read, update or clear depth-first completed-result
caches. Explicit persistent depth-first cache attachment or capture in ordered
mode is unsupported. Switching back preserves the depth-first state. Phase-transfer
or completed-result caching is outside this implementation; the only Ordered cache
is the one-step record cache below.

== One-step record cache

An optional `OrderedStepCache` records, for every expanded identity, exactly the
checked step the adapter returned: the children in step order with their exact
step coefficients, emitted terminal step coefficients and the step's conditions in
first-appearance order (saved applications: after the zero-sector drop). A later
expansion of the same identity replays the record instead of running transport,
rule selection and specialization; for saved applications an owner whose Apply
steps all hit is never prepared. Records hold no frontier or terminal weights, no
zero-erasure decision and no final answer, so every merge, cancellation and guard
of an uncached request is reproduced in every weight arithmetic and scheduler
pass: replayed requests have identical frontier states at every phase boundary and
identical results. Closed-artifact product factorizations depend on request-local
memoized lower-factor reductions (guards are retained at first use only); they are
recorded as opaque and always recomputed.

The cache is attached per adapter (`RoutedCandidateApplication` and
`PreparedSavedApplication::set_ordered_step_cache`,
`CandidateReducer::set_ordered_step_cache`, inherited by forks, and
`Reducer::reduce_artifact_linear_combination_ordered_with_step_cache`) and shared
through `Arc` by every request. It is thread-safe; inserts are idempotent because a
record is a deterministic function of its identity. Misses populate it. Records are
sharded by stage, support and topology class (the owner sector for saved
applications: the Apply owner, or the owner a Route identity is transported to).
`OrderedStepCache::open` binds a directory, whose shards are read lazily at the
first lookup; `save` writes each changed shard as a deterministic,
content-addressed file (sorted identities, coefficients with factorized
denominators, records; byte-identical for the same records) and then the manifest.
Every file carries a stamp: the BLAKE3 digest of the binding (caller program
digest, family, coefficient variables, adapter, unresolved policy, schema and step
semantics version). A mismatched or corrupt file is rejected with
`ReductionError::OrderedStepCache` and never used. A resident-byte ceiling and the
Symbolica RSS budget limit growth; skipped records only cost recomputation.
Progress reports per-request `step_cache_hits` and `step_cache_misses`; the cache
reports per-shard hits, misses, inserts and bytes.

Exact field weights must be provided as weights rather than separated into
unrelated scalar-carrier calls. Application frontends are responsible for
collecting compatible family, mass, dimension and spectator conventions before
calling the weighted reducer. Distinct coefficient fields remain distinct.
