# Local parametric owner-domain matching

`owner-domain-match` classifies explicit coordinate boxes against the ordered
rules and terminals in saved owner programs. It cold-loads the selected immutable
owners once and uses the existing native guard machinery. This is a local
applicability operation by default, not an RHS expansion, source solve or closure
campaign. The optional `--follow-successors` mode described below adds a shared
symbolic successor worklist without source generation or a closure claim.

```sh
python examples/python/match_shared_owner_domains.py \
  --executable /path/to/gated/rustred-cli \
  --manifest selection.json --queries queries.json \
  --output result.json --events events.jsonl --stop-file stop
```

The equivalent Rust command is `rustred-cli owner-domain-match` with the same
arguments except `--executable`. The generic selection format is the same as
`routed-campaign` and `owner-domain-scan`. For literal local queries, a selection
may contain only the required saved owners and an empty
`initial_frontier_routes` array; native loading still validates family, context,
ordering and saved policy. A full routing selection remains usable, but adds
preparation work. Routing requires the explicit `--route-domain-overcover`
worklist option described below.

## Query input

```json
{
  "schema": "rustred.owner-domain-queries.json.v2",
  "queries": [
    {
      "id": "child-prefilter-r11",
      "owner": "10",
      "lower": [0, 11],
      "upper": [null, 11],
      "max_numerator_rank": 11
    }
  ]
}
```

This illustrative two-coordinate query is the ray `n0>=1, n1=-11` for a family
with a saved owner `10`. All arrays and masks must have the loaded family's
arity. Local coordinates are `n=1+x` on active axes and `n=-x` on inactive axes;
lower bounds are unsigned integers and a null upper bound means infinity.
Every query explicitly supplies its own rank cap: a nonnegative 32-bit integer,
or null for unbounded numerator rank. Rank is the sum of **inactive** local
coordinates, not positive dots. Descendant queries are not clipped to the
saved program's entry rank. IDs are unique and contain 1–128 UTF-8 bytes; input
is bounded to 1 MiB by default, with an explicit `--max-query-bytes` override.

An optional `power_bounds` object retains correlations that cannot be represented
by independent coordinate limits:

```json
"power_bounds": {
  "max_positive_power": 24,
  "min_power_difference": 9,
  "max_power_difference": null
}
```

Here A is the sum of positive physical powers and D=A-R is the sum of all
physical indices. Positive local coordinates contribute **1+x** to A, not x.
The example intersects the supplied box/rank with A<=24 and D>=9; the numbers
are input, not topology-specific engine policy. Missing individual bounds or
null bounds are unbounded; omitting the object means no additional predicates.
The whole object must be an object, not null. Unknown fields, reversed D bounds,
negative A bounds and out-of-range integers are rejected before native loading.
Query schema v2 supersedes v1; no compatibility interpretation is applied.

Matching and successor walking retain these predicates after coordinate
projection. Exact fixed-cell IBP shifts translate the current A/D bounds; the
original starting limits are never reapplied to children. Admitted affine
routing conservatively retains the A upper and D lower bounds and charges
weighted pinches, but may widen D upper because affine constants lower numerator
degree. Domain identity, containment and per-job reuse include the predicates.
Results and diagnostics expose `power_bounds` (or explicitly named source/target
variants). `correlation_empty_cells` records proven-empty native intersections,
separately from rank-only emptiness.

This compact representation does not encode every affine guard or exact routed
image. It still permits conservative extra points. The separate optional
`owner-guarded-apply` diagnostic does not support these predicates and rejects
them explicitly, rather than silently discarding them. Initial nonliteral inputs
with source conditions retain the existing conservative source-validity
obligation before routing; this may also refuse an empty constrained input.

Boxes and a rank cap cannot encode arbitrary inherited native equalities,
nonzero guards or reachability. A query extracted from a conservative successor
scan remains a **prefilter** unless those other obligations are established
separately. In particular, an exact local gap does not establish that an earlier
source rule reaches it or justify treating it as a reached missing-rule frontier.
Searching an explicitly over-covering input can still be valid, but may do work
that the original source domain never needs.

## Result and exit status

Without `--follow-successors`, the full atomic JSON result distinguishes `selected_rule`, `terminal`,
`exact_zero_sector`, `exact_gap`, `unresolved`, and `invalid_source_condition`
pieces. An undecidable earlier guard remains unresolved, not a fall-through gap.

- Exit 0 means `classification_complete=true`: all requested local domains have
  an exact classification. This **can include exact gaps or invalid conditions**.
- `all_queries_locally_applicable=true` additionally excludes gaps and invalid
  source conditions. Even then, RHS specialization/cancellation, descent and
  recursive successor coverage have not been established.
- Cancellation, an unresolved piece, resource exhaustion or another incomplete
  classification produces a nonzero exit (normally 4 for an incomplete result).
  The saved result retains the available prefix and error; no partial success
  is silently promoted to a complete classification.

The local-match result always keeps `family_closure_claim=false`, `ibp_generation=false`
and `rhs_successors_expanded=false`. A completed empty domain may be vacuously
locally applicable; this is not evidence that the queried region is inhabited.

## Operational controls

The result and event destinations must be fresh paths; neither is overwritten.
The full result is atomically installed. Events contain compact scalar progress,
not retained piece arrays. A terminal-only dashboard shows classification counts;
`--no-progress` disables it. Without `--events`, JSON heartbeat events go to
stdout; the result still requires `--output`.

The stop file requests cancellation through the existing shared atomic flag;
native calls are not forcibly preempted. No elapsed deadline is introduced.
Local matching is serial; the optional shared walk has a bounded worker pool.
The thin Python wrapper replaces itself with the Rust process, inherits the
license without printing or persisting it, and sets native inner thread pools
to one. Direct CLI callers must also respect the native inner-pool contract.
For affinity and process-tree memory supervision, use the existing
[`shared_owner_campaign.py` driver](shared_owner_campaign_driver.md) with
`--queries` instead; it keeps a supervising Python process and invokes the same
native shared walk, without a solve timeout.

`--max-queries` defaults to 256 and `--max-query-bytes` to 1,048,576. Both accept
explicit positive, platform-representable allowances; matching/walking no longer
impose an additional 10,000-query ceiling. The Rust request exposes the same
`max_queries` and `max_query_bytes` fields, and both Python steering scripts
forward the options. For example, `--max-queries 60000 --max-query-bytes 33554432`
admits a larger query document without changing any native-work allowance.
The CLI bounds the read, and the library checks actual UTF-8 bytes before parsing;
every query is validated before owner loading. Allocation follows actual admitted
rows, not the requested count. Text, parsed JSON and typed queries can coexist,
so this is not a process-memory guarantee. Progress and final receipts expose
`requested_max_queries` and `requested_max_query_bytes`. The separate guarded
diagnostic and concrete finite-entry interface retain their existing fixed limits.

`--max-total-pieces` defaults to 100,000 (ceiling 1,000,000 retained output
pieces). Per-query counters can be
set explicitly with `--max-rules-per-query`, `--max-terminal-checks-per-query`,
`--max-predicates-per-query`, `--max-pieces-per-query`, `--max-cells-per-query`,
`--max-split-operations-per-query`, and `--max-coordinate-cells-per-query`.
Unspecified values retain native defaults. These are operational allowances,
not mathematical rank restrictions or hard RSS guarantees. The positive
`--max-guard-univariate-degree` option sets the existing native per-predicate
univariate degree allowance (default 16), in either local-match or shared-walk
mode; the Python wrapper forwards it. For example, 64 admits a native degree-17
guard that exceeds the default, but does not change the guard, rank scope or
native algebra algorithm. Other native guard-algebra limits remain unchanged.
Raising a work cap never changes an unresolved
answer into a claim of applicability without actually completing that work.

### Optional exact bounded-coordinate refinement

`--max-bounded-refinement-cells-per-query N` opts into resolving an undecided
predicate by fixing a bounded **inactive** coordinate to each admitted integer
value and retrying the same predicate. Its default is **0**, preserving the
explicit conservative diagnostic. The Python wrapper accepts the same flag.
For example, append `--max-bounded-refinement-cells-per-query 64` to the command
above to allow up to 64 cumulative singleton faces per query.

By default the matcher considers only polynomial-supported inactive coordinates, chooses
the smallest finite interval deterministically, and uses the query's actual rank
simplex to bound an otherwise unbounded inactive interval. Every face retains
that exact simplex and all other bounds. Positive axes are never enumerated,
even when their box bounds are finite; unbounded positive powers stay symbolic.
Descendant R11 queries are not reduced to the saved entry R10 scope.

The complete split's face and geometry allowances are reserved before any child
is processed. Insufficient optional allowance leaves the original unresolved
piece unchanged; it does not claim coverage of a partial split. Faces are
processed lazily rather than retained as a width-sized pending stack. Ordinary
native errors, other work limits and cancellation remain explicitly incomplete.

The same allowance now permits refinement after selected native guard
GCD/factorization **preflight** refusals (degree, prospective work or storage).
The failed attempt remains charged; each face retries the identical predicate
at the identical dispatch priority. If refinement cannot be admitted, the
original typed refusal is retained. Backend faults, allocation failures, native
output limits, cancellation and aggregate matcher limits are not converted into
refinement opportunities. No CAS algorithm is implemented by this fallback.
Local-match errors include `error_predicate_context` with the active predicate,
coordinate box and rank when available. This provenance is currently specific
to local matching; the RHS inspection wrapper retains its existing failure
representation.

`--bounded-refinement-axes finite-axes` additionally admits polynomial-supported
positive coordinates **only when their box upper bound is explicitly finite**.
The default is `inactive-only`; neither mode does refinement until the separate
face allowance is positive. In the Rust API this is
`OwnerDomainMatchLimits.refinement_axes = OwnerDomainRefinementAxes::FiniteAxes`.
The Python matcher and symbolic campaign supervisor forward the same option;
it is rejected in the concrete-target supervisor mode. Positive coordinates
never borrow a bound from the inactive numerator-rank constraint. Selection
still uses smallest admitted width, then original axis index, across eligible
coordinates. The complete split, predicate cursor and whole guard conjunction
are preserved exactly as in inactive-only mode.

For example, a coupled condition `n_0-n_1=0` on two bounded positive powers can
be classified by exact finite-coordinate faces. This is not sampling, and
does not permit splitting an unbounded positive axis. Results record the
requested policy and allowance, including preparation errors and compact
progress; successor reports also retain them in `applied_limits.matching`.
This option controls **local matching**, separately from the bounded native
routing described below. Enabling refinement alone does not establish a finite
recursive campaign or family closure.

Per-query `stats.refinement_steps` counts admitted coordinate splits, while
`stats.refinement_cells` counts all admitted singleton faces across levels,
including prepaid faces left unvisited after cancellation. These are not counts
of unique final output pieces or IBP searches. Coupled predicates can remain
unresolved after refinement; no new rule, source job, or recursive closure claim
follows merely from enabling this allowance.

## Optional shared symbolic successor worklist

Append `--follow-successors` to reuse the same input domains and immutable owner
snapshot in a shared symbolic worklist. The Python wrapper forwards this mode.
Each scheduled domain is matched in exact dispatch order, then its selected
rules' RHS terms are inspected using native specialization and coalescing.
Supported successor domains retain their translated bounds and actual rank;
unbounded positive powers remain symbolic. The mode does not enumerate a list
of concrete positive-dot targets.

Literal installed-owner successors are scheduled once when an already admitted
domain contains them. The containing domain may still be pending: this is work
deduplication, not a declaration that pending work is already solved. Inclusion
is local to the same immutable snapshot and includes the actual rank scope.
The queue first checks an exact-domain hash index, then a rank-compatible
full-orthant representative for the same owner and Apply/Route phase, before
scanning more general containing boxes. Exact keys share their coordinate
storage with queued domains; hashing never replaces full-key equality. Indexed
reuse can choose a different valid containing domain than the older linear
scan, but never removes already scheduled work or treats it as completed.
Queue admission and result publication remain deterministic on successful runs.
With multiple workers, native inspection may run ahead of publication; its
uncommitted results do not count as completed domains.
Unresolved dispatch, RHS validity or descent, and successors needing owner
routing remain explicit frontiers. A coefficient that is not uniformly nonzero
keeps a conservative successor-domain over-cover, so its frontier is not by
itself a proved reachable missing-rule domain.

The full result has distinct schema `rustred.owner-domain-walk.json.v2`.
`recursive_worklist_exhausted` reports that every admitted domain was inspected
without cancellation or an operational error. It can still be true with
explicit frontiers. `all_scheduled_domains_resolved=true` additionally requires
no such frontiers and gives exit 0; otherwise the result is incomplete and
normally exits 4. Neither field claims family closure, provenance certification,
or coverage beyond the submitted domains and inspected dependencies. The result
keeps `family_closure_claim=false`, `ibp_generation=false`,
`routing_expanded=false`, and `independent_certification=false`.

The positive work allowances `--max-domains` (default 100,000, caller-selected ceiling),
`--max-frontiers` (default 100,000, ceiling 1,000,000),
`--max-successor-events` (default 1,000,000) require `--follow-successors`.
They separately bound admitted domains, retained frontier records and streamed
callback events. A large explicit domain allowance is not an eager allocation:
the queue grows incrementally with fallible reservations, and campaigns should
use the resource supervisor's RSS limits. There is no hidden one-million-domain
ceiling on a caller-selected storage budget. Aggregate general box-containment comparisons are unlimited by
default (`OwnerDomainWalkRequest.max_containment_checks=None`). An explicit
`--max-containment-checks N` sets a positive finite diagnostic allowance;
`--max-containment-checks unlimited` selects the default explicitly. Both forms
require `--follow-successors`. Admitted, live and final reports expose the
effective `max_containment_checks` as a positive number or `null` for unlimited,
with a `containment_check_policy` label. The comparison counter remains checked:
integer overflow is an explicit incomplete outcome, never wraparound. Unlimited
comparisons do not remove domain/storage, event, native-work or resource limits,
and do not change actual rank. Aggregate event allowances have no fixed
ten-million ceiling: the
event stream is not retained in memory. The frontier limit is checked before
retaining any initial, application or routing frontier. Indexed
exact/full-orthant reuse does not spend
an explicit finite comparison allowance and can still succeed after that allowance
is spent; a request requiring another general scan then fails explicitly. Live and final
reports separate `exact_domain_hits` and `full_orthant_hits` from total
`deduplication_hits` and `containment_checks`. These are scheduling counters,
not solved-domain counts. Existing per-query matching allowances apply independently to
each scheduled domain; the optional bounded-refinement allowance therefore
also applies per scheduled domain. `--max-total-pieces` is a local-match report
allowance and does not replace the worklist's aggregate event limit. These are
not hard memory bounds. Stop-file cancellation, fresh atomic output, and compact
progress remain active; the full output document is the source of truth.

Unlimited-comparison mode maintains a stable maximal-domain containment index
using cached exact coordinate, A, R and D extrema. Raw descriptions that imply
the same set need not have the same fields: a numerator coordinate bounded by
four implies R<=4 even if its supplied rank cap is ten. The fixed-template
summary recognizes this without running geometry or algebra per comparison.
It retains true infinities, wide aggregate bounds and explicit empty sets;
invalid geometry fails rather than becoming an empty-domain shortcut.
The aggregate filter groups retained candidates by exact
`(A_max,R_max,D_min)` extrema. A necessary aggregate test skips whole groups
before native exact inclusion, in both forward lookup and reverse retirement.
Exact-key and dominant-orthant shortcuts keep priority; otherwise the lowest
valid retained ID is selected across eligible groups, regardless of group
storage order. Empty and unbounded signatures are explicit, not finite sentinels.
By default, when a new domain contains an earlier one, only the earlier lookup candidate
is retired: its exact key, original ID and pending FIFO inspection remain.
This can avoid admissions that the former componentwise comparison retained.
Exact lookup still returns the original ID; a nonexact hit
may return another valid containing domain. Pending work is never counted as
completed. Explicit finite comparison caps retain the historical raw scan
policy and do not construct summaries; their diagnostic comparison prefix is
unchanged. Phase, owner and immutable program snapshot still delimit reuse.

Live and final reports expose `containment_index_policy`,
`containment_candidates`, `containment_retired_candidates` and
`containment_maintenance_checks`. The last counter measures reverse-dominance
maintenance and is **included** in total `containment_checks`, not free work.
These count full containment comparisons, not aggregate-group probes. Filter
work is included in measured wall/CPU time; fewer full comparisons alone do
not prove a performance improvement. Test instrumentation separately counts
group probes/rejections without changing production progress schemas.
The unlimited policy label is `maximal_candidates_semantic_unlimited`.
`containment_summary_builds` counts successful projections after exact-key
misses, including requests later reused or rejected. `containment_semantic_hits`
and `containment_semantic_retirements` count forward reuse and reverse retirement
respectively that the old raw predicate would miss. Summaries are cached once
per admitted ID, outside the raw diagnostic descriptor and exact-key identity.
Counter/allocation checks precede candidate retirement. A large incomparable
set can still be expensive; this index does not establish closure or preserve
physical sum constraints that were absent from an input box.

### Experimental owner-local publication

`--follow-successors --publication-policy owner-batched` selects independent
phase/owner admission and publication queues sharing one immutable reducer.
The Rust field is `OwnerDomainWalkRequest::publication_policy`; the default is
`OwnerDomainWalkPublicationPolicy::Ordered`. Both Python steering scripts
forward the option, without requiring a rebuilt Python extension.

Only successor geometry moves to its destination queue; producer diagnostics
and local alias/initial-anchor identities remain with the source. Native
workers stream bounded chunks. The coordinator polls every active stream
without blocking, delivers the ready subset and refills finished slots without
waiting for a silent peer. It waits only when no active stream is ready, using
a mutex-protected condition to avoid lost notifications. Source chunks are
admitted before successful source publication. Each pass collects at most one
chunk per active producer. Synchronous destination admission and one active
producer per owner remain possible utilization limits.

Started owner-batched walks emit v4 receipts with `(bucket, local id)` identities
instead of a single global diagnostic ID. Initial input handles use the same
composite identity. Existing rules and concrete reduction semantics do not
change, but diagnostic IDs, cover fragmentation and capped prefixes can differ
between policies, worker counts and readiness-driven runs at a fixed worker
count. A failure before owner-batched traversal
starts retains its earlier receipt schema and explicitly marks the requested
policy. Pending aliases/anchors or incoming successors cannot establish success.

The ready-stream follow-up passes independent source review, 501 integrated
release library tests and 166 clean-owned tests (one existing diagnostic ignored
in each overlapping suite), plus the release CLI build. All twelve fresh
saved-input controls complete successfully, but the repeated six-worker A10
median is 4.564 s ready-stream versus 2.301 s ordered. One active producer per
hot owner and extra native work remain limitations. Ordered stays the default.
The earlier chunk-barrier prototype
completed all eight saved routed
two-loop policy/worker controls successfully; their millisecond timings
do not establish a speedup. The small four-owner five-loop canary also completes,
but owner batching was slower at six workers. These are measurements of the
earlier barrier version, not the new ready-stream scheduler. No full-campaign
speedup has been established. See the
[current scheduling gate and scope](finite_starting_domains.md#owner-batched-prototype-and-measurement-boundary).

### Opt-in delegation of unreserved domains

`--transfer-unreserved-lookahead H` enables
`OwnerDomainWalkSchedulingPolicy::TransferUnreserved { lookahead }` in the Rust
request. Both Python steering scripts forward the option. H must be positive
and remains fixed independently of worker count; omission keeps `InspectAll`.
The option requires successor walking and unlimited containment comparisons.
Finite diagnostic comparison caps are rejected before preparation in this mode.

When a newly admitted region provably contains an older region under the same
owner, phase and immutable program snapshot, the older obligation may transfer
to the newer one only if it has not entered the logical dispatch lookahead.
Reserved or started inspections are never cancelled by containment. Forward-only
links prevent alias cycles; a chain remains an obligation until its final
representative completes publication without unresolved frontiers or failures.
The exact containment test is unchanged, and routing and application phases
remain distinct. No source checks or entry/descendant predicates are dropped.

With ordered publication this mode emits `rustred.owner-domain-walk.json.v3` with the requested
`scheduling_policy`, native-publication counts and a separate `delegation`
summary. A `delegated_not_inspected` record is not a native finished result.
The default still emits v2; owner-batched publication uses v4 as described above.
A representative with unresolved guard/source/route
frontiers cannot discharge its aliases. Cancellation, partial event publication
and resource failures retain incompleteness; global frontier/error checks remain
mandatory even if the ledger's local obligations are discharged.

These changes pass release validation, including native execution and public
CLI/Python tests, plus a separate 120-test check against the committed scheduler
without unrelated local escrow changes. Fewer pending inspections remain a
performance hypothesis, not a measured speedup or a five-loop closure claim.

`--reuse-initial-d-bands` additionally permits exact partial reuse against
immutable initial Apply regions, while retaining the native residual and the
anchor's outstanding responsibility. It requires transfer-unreserved scheduling.
Its optional index counts only initial Apply descriptors against the unchanged
4,096-entry/2 MiB logical-payload limits; all Route inputs still belong to the
queue and ledger. Original IDs and all Apply exclusion membership are retained,
including Apply descriptors that cannot supply a usable anchor. Fallback leaves
ordinary inspection enabled rather than dropping work.

The `initial_overlap_prepared` event and final `initial_overlap_index` report
expose eligibility, retained membership, usable anchors, logical charge and
active/fallback status. An interrupted counting prefix is explicitly incomplete.
Ordered indices use global initial IDs; OwnerBatched reports per-bucket indices
and local IDs, with later-created buckets marked `not_built`. These diagnostics
are not coverage authority or an RSS measurement.

The per-domain RHS budgets `--max-rhs-cells-per-query` (default 100,000),
`--max-term-visits-per-query` (1,000,000),
`--max-native-operations-per-query` (4,000,000),
`--max-rhs-events-per-query` (1,000,000),
`--max-shift-groups-per-query` (1,000,000), and
`--max-sign-splits-per-query` (1,000,000) are explicit CLI/Python options.
Raising the aggregate event allowance does not change these per-domain limits.
RHS cells include every refined cell, not just pinches or boundaries.
Other `OwnerAppliedLimits` retain their native defaults and remain configurable
through the Rust API. None is an elapsed timeout or an input-rank restriction.

### Bounded parallel inspection

`--workers N` requires `--follow-successors` and defaults to one. The native
request also accepts optional `--inspection-workers I` (Rust field
`OwnerDomainWalkRequest::inspection_workers`). For N>1 this reserves exactly I
inspectors, N-1-I admission helpers and one coordinator; for N=1 only I=1 is valid
and execution stays inline. Omission preserves the existing policy-specific
automatic split. For example, N=50 normally means 25/24/1, while I=40 requests
40/9/1. This is a configured reservation, not a claim about busy cores or speed.
With a finite containment-comparison allowance, an explicit partition must have
no admission helpers (I=N-1 for N>1), preserving sequential cap accounting.
The CLI, both Python steering scripts and completion report expose this choice.
Inspectors and helpers retain separate bounded pools because an inspector can
block awaiting publication; sharing that same blocking pool would risk deadlock.

The native request accepts 1–64 subject to the existing CPU-affinity and Symbolica-license
checks; the campaign supervisor additionally enforces its 50-compute-worker
budget, including declared concurrent jobs. Workers borrow the same immutable
prepared owner library. They do not clone that library or transmit native
coefficients through the scheduler.

Each running worker has at most one queued chunk and one local chunk. Independent
per-chunk limits are 16,384 physical records, 1,048,576 logical events and 8 MiB
of logical payload; reaching any limit causes a flush. These are internal bounds,
not request/CLI options. Increasing only one would not remove the other limits.
Logical buffer size excludes allocator usage, native scratch space, the
coordinator's current chunk and one just-converted descriptor waiting to enter
a full worker chunk. That descriptor is itself capped at 8 MiB; process RSS
remains separately monitored. The coordinator admits successors in stable
domain-ID and callback order.
Successfully finished, non-running slots can move their final chunk and result
to a separate completed-result escrow, bounded by 65,536 entries and 8 GiB of
accounted storage. This releases physical slots without declaring their work
published or their obligations discharged. The escrow does not accumulate output
from running visitors: raising its limit alone cannot unblock a producer waiting
to publish a second chunk. Its accounting includes entry metadata and vector
spare storage, but is not an RSS bound.
Later workers can block behind an expensive earlier domain, so worker count
alone is not a promised speedup. Progress reports expose this backpressure.

Attempted events and returned native work are distinct from committed events
and completed domains. Native-call totals become available when that inspection
returns, including cancelled or uncommitted inspections; they are not live
instruction counters inside Symbolica. The first observed error requests
cooperative cancellation and all workers are joined before return. Failure
prefixes may differ across worker counts and must not be compared as complete
equivalent workloads. Neither parallelism nor a larger allowance changes the
meaning of unresolved domains, source validity or closure.

### Optional coefficient-support classification

The RHS visitor can keep a conservative successor when **optional numerator
zero-locus classification** declines the next native factor/GCD operation under
its existing work allowance. Exact coefficient construction, fixed-coordinate
specialization, numerator admission, original denominators, child-source
conditions, descent, and cancellation remain mandatory. Backend faults,
allocation failures, output/replay limits, aggregate budgets and unknown future
error categories remain errors. This changes neither the exact rule nor its
application to a concrete integral; it avoids requiring an unnecessary uniform
nonzero proof merely to discover an over-cover of possible dependencies.

The core visitor retains the exact coefficient during its callback. The shared
worklist keeps the conservative domain and conditional label, **not** that exact
predicate. Consequently an uncovered point of the requested child domain is
still not evidence of a reached missing rule and cannot become a terminal.

Per-domain stats and final aggregate reports expose
`optional_coefficient_refusals`, `optional_original_refusals`, and
`optional_coalesced_refusals`. Attempted work stays charged, including a refusal
followed by cancellation. Each Apply record retains at most the first original
and first coalesced refusal in `optional_refusals`, separately from `frontiers`.
These ordinary budgeted events contain the source bounds/rank, shift, original
term ordinal when applicable, and numeric native resource diagnostic; they do
not serialize coefficients. `optional_refusal_provenance_truncated` is true
when the count exceeds the number of retained records, including a first event
that could not be delivered before a stop. No new mode flag is needed.

### Shared admitted-route domain covers

Add `--route-domain-overcover` alongside `--follow-successors` to reuse the
selection's already admitted momentum maps without expanding numerator powers.
This is a conservative sufficient-cover operation, not an exact route image.
For example:

```sh
python examples/python/match_shared_owner_domains.py \
  --executable /path/to/gated/rustred-cli \
  --manifest selection.json --queries queries.json \
  --output result.json --events events.jsonl --stop-file stop \
  --follow-successors --route-domain-overcover --max-route-masks-per-query 100000
```

An admitted map permutes positive denominator powers into a nonnegative base
`B` and substitutes degree-at-most-one polynomials for numerator factors. For
incoming numerator rank `R`, every surviving monomial has degree `|e|<=R` and
endpoint `B-e`. Its numerator rank is
`|e| - sum_i min(e_i, B_i)`. Losing `k` active denominators consumes at least
`k` units of that degree, so a strict pinched support needs rank at most `R-k`
when the **actual incoming** rank is bounded by `R`. This special case streams only
supports with at most `R` lost active axes. The full mapped root retains `R`;
an unbounded incoming rank stays unbounded. R11/R12 successors of an R10 input
are tightened from their actual rank, never from the saved generation scope.
Mass constants can lower monomial degree and cancellations can remove
endpoints; neither enlarges this bound.

The full mapped root goes directly to `Apply`; strict subsupports reenter
`Route`. Route/Apply are distinct queue keys. Pending containing domains can
reuse work but do not count as completed. The route queue preserves supplied
finite boxes instead of replacing them with full orthants. Literal owners
retain the complete box. For a nonliteral map, each surviving positive
coordinate keeps the upper bound from its mapped source axis. Unconstrained
routing uses lower bound zero because numerator cancellation can lower a
positive power; the power-bounded lane uses the tighter support-aware bound
below.
Inactive coordinate bounds cannot generally be permuted through an affine
numerator substitution and are conservatively controlled by the actual rank.

The bounded native visitor `visit_bounded_domain_route_overcover` also tightens
pinches by their minimum required powers. For a lost source subset P,
`R_child <= R - sum(source_local_lower[j]+1 for j in P)`. For example, minimum
positive powers two and four cost six degree units to pinch simultaneously:
at incoming R6 their simultaneous-pinch child has R0, not the old R4 bound.
An impossible weighted pinch is skipped, never saturated to R0. Unbounded rank
remains unbounded. The full-orthant visitor delegates with zero lowers and
unbounded uppers, recovering the usual R-minus-number-of-pinches bound.

With nondefault `power_bounds`, the native power-bounded visitor also uses the
verified affine numerator map's exact support. For inactive source row i,
write `Q_i=c_i+sum_j M_ij T_j`, with source powers `L_i<=t_i<=U_i`. Only rows
with nonzero `M_ij` can supply numerator degree on target axis j, so

```
C_j = min(sum(U_i for M_ij != 0),
          source_R_max - sum(L_i for M_ij == 0)).
```

All sums here run over inactive source rows. They use the source domain's
implied bounds, not the original starting restrictions. A surviving mapped
positive axis with source local lower l keeps lower `max(0,l-C_j)`, and cannot
be pinched if `C_j<l+1`. For example, `(T1+1)^3/(T0^2*T1)` cannot pinch T0:
its column has zero numerator support even though the total source rank is
three. Empty supports, genuinely infinite bounds and checked wide sums retain
their mathematical meanings.

The prepared map caches only source-axis incidence using the existing native
Symbolica coefficient zero test. There is no numerator expansion, coefficient
cloning per worker, new CAS kernel, or new artifact/schema requirement.
Each child derives its bounds from the source, independently of projecting
the all-positive root; pinched axes reset their local lower to zero. The
existing joint weighted pinch-cost check remains necessary because separate
column maxima need not be jointly attainable. Every examined candidate still
counts against the mask allowance, even when support proves it impossible.
Calls without additional power predicates keep the former bounded visitor's
behavior. These bounds reduce overcoverage; they do not make the image exact.

The application retains boxes on initial nonliteral admission, every RHS
successor, mapped Apply and subsequent Route reentry. Empty rank intersections
emit no work; inverted/wrong-arity boxes fail explicitly. Every
over-covered point need not be reached, and an uncovered point is not thereby
a reached missing rule or a new terminal.

The cover theorem does not prove source conditions. Original RHS terms retain
the existing validity checks. Initial nonowner queries and route-generated
reentries with unchecked source conditions remain explicit obligations; known
zero sectors do not bypass them. Missing maps likewise remain frontiers.

`--max-route-masks-per-query` defaults to 100,000 and requires the route option.
It caps examined candidates, including weighted-impossible pinches; per-domain
`masks_pruned` distinguishes these from emitted `events`. Aggregate events and admitted-domain budgets still
apply. The native visitor has a separate logical coordinate-cell budget. No
native expansion counts are fabricated: `routing_expanded` remains false,
while `route_domain_overcover`, `routed_domains` and `route_masks` report this
distinct operation. Queue exhaustion is still not a family-closure claim.
