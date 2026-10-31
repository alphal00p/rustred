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

The modular modes (coefficient contexts with at most one variable) share the
options `ModularCommon` (`threads`, `seed`, `verification_primes`, `max_primes`,
`cross_check_exact`) and one lift over Q. Their canonical text form is
`exact | modular_sampled[:key=value,...] | modular_polynomial[:key=value,...]`
(`FromStr`/`Display`, keys = field names, `modular` = `modular_sampled`).

`ModularSampled` runs the scheduler once as a recorder. Each weight carries two
values modulo a probe prime, used only for the zero-erasure decision, and the
propagation is recorded as a straight-line trace over interned exact step
coefficients; each distinct coefficient is validated once. The trace is replayed
over 64-bit primes at batches of sample points and every terminal coefficient is
interpolated per prime by Thiele continued fractions. The first prime discovers the
degrees (half of `batch_points` points, doubling until every interpolation is
confirmed by `verification_points` further points); every later prime replays
exactly the points needed, in batches of at most `batch_points` points that fill
whole waves of the threads. Every replay re-checks each recorded erasure at every point and
fails closed.

`ModularPolynomial` uses the same recording pass and replays the trace with
weights in `Z_p(d)`, so the adapters' steps run once whatever the number of
primes. Every non-constant step coefficient's denominator is factored over Q
once per request into a basis of primitive irreducible factors (checked by
multiplying back). A weight modulo `p` is a polynomial part plus partial
fractions `a_{f,j}(d) / f(d)^j` (`deg a < deg f`) over the monic basis images:
sums are coefficient-wise, with no common denominator and no gcd, and a weight
is zero exactly when all its coefficients are. A product with a step
coefficient multiplies by its numerator (multiplication by `d` shifts each
chain by one power) and divides by each denominator factor (closed recurrences
for linear factors, a small CRT step otherwise). A prime is unusable when the
basis images are not pairwise coprime of unchanged degree or a coefficient
denominator vanishes modulo `p`. A round requests between `threads` and
`max(threads, lanes)` primes, one replay per thread; up to `lanes` primes share
a replay when there are fewer threads. Every replay checks each recorded
erasure to be exactly zero modulo each prime and fails closed otherwise. The
terminal images are the reduced numerator and monic denominator of each lane.
`denominators=dense` keeps one reduced numerator/denominator pair per weight
instead (a gcd per operation; a reference). `max_degree` bounds every weight
and fails closed.

The lift consumes per-prime images (reduced numerator and monic denominator of
every terminal, plus a structural signature: the image source's structure and the
degrees) strictly in the order of a seeded prime stream. Images are grouped by
signature; the group holding a strict majority of all usable images is lifted by
CRT and rational number reconstruction, a coefficient is accepted once one further
prime confirms it, and the result is checked on `verification_primes` fresh primes
(fresh points for the sampled mode) never used to build it. A disagreement
re-opens the lift; more than `max_primes` primes fail closed.

`threads = 1` runs everything on the calling thread (no pool, no spawned thread);
more threads use one private pool per request. Images computed ahead of need are
kept for later rounds, so results, consumed primes and points per prime do not
depend on the thread count. Keys, steps, children and guards depend only on
integer keys and exact step coefficients, so every arithmetic expands the same
keys and returns the same terms and conditions; only the monomial census differs
(one per modular weight). `cross_check_exact` also runs the exact arithmetic in
the same pass and fails on any differing zero decision or terminal coefficient.

== Concurrent execution

`OrderedSchedulerOptions { threads, window_keys }`
(`SavedApplicationOptions::ordered_scheduler`,
`PreparedSavedApplication::set_ordered_scheduler_options`,
`CandidateReducer::reduce_linear_combination_ordered_with_scheduler`) runs the
saved routed and candidate adapters on a private pool of `threads` workers.
`threads == 1` (the default) is the sequential scheduler on the calling thread;
no pool or thread is created. Closed artifacts always run sequentially (their
complete factorizations own request-local depth-first state), and so do requests
with diagnostic limits, which are checked against aggregate counts inside steps.
A caller with one core budget passes it here and to the modular replay threads;
the two pools are never busy at the same time.

Every setting reproduces the sequential execution exactly: the same expanded keys,
terms, conditions in the same first-appearance order, statistics, counters
including peaks and cancellations, observations in the same order, phase
inspections and first error. A phase is processed in windows: the greatest
`window_keys` pending keys of the phase, closed under same-phase descent above the
lowest of them. Steps are pure functions of their key and are computed by the
workers, ahead of time and possibly for keys that later cancel; only the steps of
keys the sequential scheduler expands contribute. Window keys settle in a
dataflow order: a key merges its window contributions in the sequential order
(parent pop order, then child order) once all its window parents have settled,
and is expanded iff its weight is then nonzero. Contributions leaving the window
are merged per destination, concurrently across destinations and again in the
sequential order per destination; a destination receiving a large share of a
window is merged by exact prefix sums on several threads when weight sums are
canonical (exact weights). A serial scan then replays, in pop order, every
accounting, counter, condition, observation and error of the sequential
scheduler from the recorded outcome of each merge. Recorded modular weights keep
their sequential trace journal: their merges stay sequential and only their steps
are computed ahead by the workers, so the recorded trace is identical.

A window holds at most `window_keys` popped weights plus the steps of its keys;
products are formed by the merging thread, so the frontier at every window
boundary is the sequential one. Resource budgets active on the calling thread
are installed around every worker task. `RUSTRED_ORDERED_WINDOW_STATS=<file>`
appends one diagnostic JSON line per window (stage times, sizes, heaviest
destination).

Requests with a one-step record cache (below) run concurrently as well. The
workers consult the cache without changing it: a key's record is replayed when
present (its shard is loaded at the first such lookup), otherwise the adapter's
step runs. Counting the lookup, the hit accounting (for saved applications the
operational-node charge and cancellation check of a checked step), the request's
hit and miss counters and the insert of a new record happen in the serial scan,
at the key's sequential turn. Steps of keys that cancel before their turn
therefore leave no trace: the records, the shard hit, miss and insert counters
and the progress counters are those of the sequential run for every thread count.
Only shard loading can differ: a worker may load a shard for a key that then
cancels, and under a resident-byte ceiling or a Symbolica memory budget that
earlier load can change which shards and records fit; results never change.

== Phases and canonical identities

Saved applications process support sizes downward, draining routing before the
owners at a given size. Within an owner, the saved local order strictly decreases.
Proper pinches move to smaller support. Verified routing may expand a numerator
into several weighted endpoints; all of them are validated, and every endpoint
outside a proved zero sector enters the common canonical frontier. A zero-sector
endpoint keeps the conditions of its validation but carries no weight, exactly as
if it had been expanded. Route and Apply are distinct operational identities.

=== Layered Route transport

A Route identity of source sector $S$ is $D^(-b) product_a (c_a + sum_l L_(a l) D_l)^(n_a)$
in the owner's denominators: its positive powers $b$ mapped by the route's unit
bijection and one affine factor per numerator. With
`OrderedRouteLayering` other than `off` (saved applications, `SavedApplicationOptions`
or `set_ordered_route_layering`), a Route identity with at least two numerator
factors is transported one factor per step: the step expands one factor exactly
(the same native expansion and limits as the whole transport) and emits partially
transported identities $(S, t, r)$ (owner powers so far, numerator powers not yet
expanded). Partial identities of the same sector and state merge on the frontier,
so the remaining factors are expanded once for the merged weight; the last factor
emits the Route and Apply children of the whole transport. The coalesced product
is the same polynomial, hence every Route and Apply weight, zero decision, guard
and result is that of the whole transport, in every weight arithmetic and thread
count. The policy picks the next factor from $(S, r)$ only (`index`,
`smallest-power`, `fewest-terms`, `largest-power`). Partial identities live in the
Route phase of their source; a layered Route phase is processed one source sector
at a time (its Route identities, then its partial identities by decreasing
remaining power), so the pop order of pending later-phase Route identities differs
from `off` while the pending identities and weights do not. A partial identity whose
owner support has only installed zero subsectors is dropped: further factors only
lower powers, so all of its final children would be zero-sector children.
Layering is used only for applications without source conditions (endpoint
validation is the only way a transport retains guards); otherwise every Route
identity takes the whole transport. Layered steps are never recorded in or replayed
from the one-step record cache, so caches written with and without layering hold
the same kind of records and are interchangeable.

Layering is opt-in; the default is `off`. A layered run neither records nor reads the
whole transports of multi-factor Route identities, which are the heaviest records of a
whole-transport cache: on a cache written with the default it reads only the Apply,
owner-literal and single-factor Route records and recomputes the others from memoized
factor expansions, and its own cold run records fewer records. With the default, a
cold regeneration records exactly the records of a cache written without layering
(shipped caches), so their content stays complete and comparable.

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
Every file carries a stamp. Validity is versioning only: the stamp is the BLAKE3
digest of the cache schema (`ORDERED_STEP_CACHE_SCHEMA`), the hand-bumped step
semantics version (`ORDERED_STEP_SEMANTICS`) and cheap identity fields of the
binding (adapter, family fingerprint, coefficient variables, unresolved policy).
No source, build or program-file digest enters it, so a cache stays valid across
RustRed builds. `ORDERED_STEP_SEMANTICS` is bumped by hand whenever an adapter can
return a different step for the same identity and program (children, their order,
step coefficients, terminal terms, nonzero conditions and their order; e.g. a
changed route or rule selection, dropped zero-sector endpoints, or a coefficient
normal form changed by a Symbolica update); golden digests of the records of
closed, candidate, weighted-normalization and saved routed fixtures fail until it
is bumped. The schema is bumped when the file layout, the record encoding or the
stamp description changes. Program files (rule tables, owners, routes) and a
candidate program's terminal convention are not digested: callers keep one cache
per program and data version (for instance one directory per program), and
terminal conventions cannot be installed on a `CandidateReducer` while a cache is
attached. Attachment and every request check the identity fields. A mismatched or
corrupt file is rejected with
`ReductionError::OrderedStepCache` and never used. A resident-byte ceiling and the
Symbolica RSS budget limit growth; skipped records only cost recomputation.
Progress reports per-request `step_cache_hits` and `step_cache_misses`; the cache
reports per-shard hits, misses, inserts and bytes.

== Adjoint tail and tail table

With `OrderedSchedulerOptions::adjoint_tail` (or `RUSTRED_ORDERED_ADJOINT_TAIL=1`)
the scheduler evaluates a single-terminal tail by its adjoint. A tail is a suffix of
phases whose steps emit terminal contributions to at most one terminal `T`; it is
detected from the steps (a phase emitting a second terminal moves the candidate
behind it), not from names. The frontier at its first phase holds every weight
`w(K)` that enters it, and the tail adds exactly `sum_K w(K) u(K)` to `T`, where
`u(K)` is the complete reduction of `K` onto `T`: a function of `K` and the program.

- Recorded arithmetics keep the forward recording pass (zero decisions, guards,
  counters) and replace the recorded tail program by `sum_(D,j) (d^j / D) S_(D,j)`,
  `S_(D,j) = sum_(K: D_K = D) n_(K,j) w(K)` for `u(K) = sum_j n_(K,j) d^j / D_K`: almost
  every recorded product has a constant coefficient. `u` is computed exactly by
  back-substitution over the expanded steps, completed by the steps of reached keys
  the forward pass erased (so the value no longer depends on any erasure decision).
  If an erased key reached by the forward pass carries a guard not retained at the
  start of the tail, the forward program (with its replayed erasure checks) is kept.
- A tail table, kept in the step cache's directory and populated with it, stores per
  tail identity `u`, the guards of its whole reduction and its compact step. It is
  bound by versioning only (file schema, hand-bumped `ORDERED_TAIL_SEMANTICS`, and
  the step-cache binding's adapter, family, variables and unresolved policy; like
  the step cache, keep one directory per program and data version); a file of
  another version or a corrupt one is rejected loudly and the run starts cold. At every phase start the scheduler closes the tail from the table
  when it covers the frontier with one terminal and either every tabulated guard is
  already retained (any arithmetic) or the arithmetic decides erasures by probes:
  then the forward zero decisions are repeated over the tabulated steps (same probe
  values, same linear map) and the guards of the keys the forward pass would expand
  are interned in pop order. Terms and nonzero conditions (values and order) equal
  the forward run's; processed and emitted counters too for probe closing.

Exact field weights must be provided as weights rather than separated into
unrelated scalar-carrier calls. Application frontends are responsible for
collecting compatible family, mass, dimension and spectator conventions before
calling the weighted reducer. Distinct coefficient fields remain distinct.
