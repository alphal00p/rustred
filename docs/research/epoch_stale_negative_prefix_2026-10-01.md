# Reusing stale snapshot-negative lookup prefixes

Status, 2026-10-01 06:16 UTC: isolated implementation `c51926be` was integrated
as `f328844f` after independent source review. Native correctness passed:
1,326 unique application/CLI tests, including the ten new targeted tests;
12 pre-existing ignored tests remain ignored. No qualified whole-campaign
speedup or production change. The optimized four-loop ABBA passed correctness and was
performance-neutral. One full-input bounded five-loop pair reduced source time
per accepted row by 17.29%, but native throughput improved only about 2.7%:
this is a local cost improvement, not a qualified campaign switch or closure.
The local same-start continuation pair also passed structural cold checking:
source cost/row fell 37.22%, but native throughput rose only about 6%, still
below the screening threshold. Production remains untouched.

## Why investigate this

The repaired five-loop run exposes substantial containment-index work on both
the coordinator and inspectors. A short sampled profile motivates this direction
but cannot supply an unbiased sustained cost breakdown. See
[the current measured controls](epoch_p1_compact_2026-10-01.md).

In the completed historical full-A1 h0 pilot:

| Quantity | Observed value |
|---|---:|
| Accepted source rows | 178,474,588 |
| Rows already carrying a stored target | 162,715,550 |
| Targetless rows | 15,759,038 |
| Same-view rechecks skipped | 824,583 |
| Remaining stale or unprobed full-store searches | 14,934,455 |
| Source-resolution wall time | 221.497 s |
| Entire P2 preparation wall time | 312.469 s |
| Native elapsed wall time, including preparation | 1,202.645 s |
| Traversal wall time alone | 1,138.826 s |

Source resolution is 18.42% of native elapsed time (19.45% of traversal), but
includes query construction, hashes, and positive verification as well as
lookup. This is not a lookup-only timer or an achievable speedup. The rows
above do not measure snapshot lag.

Currently, a negative inspector result from an older snapshot causes P2 to
search the current store from ID zero. The Ready implementation already has
the analogous append-watermark optimization, and the shared aggregate index
already accepts a `first_id` lower bound. This proposal reuses that primitive;
it introduces no new algebra or CAS implementation.

## Mechanism and proof boundary

Suppose a worker has searched a snapshot containing IDs `0..w` and found no
container for its successor query. While it works, the coordinator may append
IDs `w..t` and retire old live IDs. It need not repeat the old-prefix search
if all of the following conditions hold:

- Published domain geometry and summaries are immutable.
- Existing live fallback candidates can be removed, but cannot become newly
  eligible during the invocation.
- Quarantine is fixed for that worker session; rescue runs before workers.
- The negative belongs to the actual session, query, snapshot and watermark.
- The worker performed the normal complete lookup, not a synthetic AllMiss
  policy.

Then every old fallback candidate still eligible at commit was eligible in the
snapshot. The honest negative excludes it, so searching only newly appended
IDs preserves the canonical minimum-ID fallback winner.

For example, a negative search of IDs 0 through 999 need only revisit the
general containment index from ID 1000 after eight new domains are appended.
It still performs the following checks over the **whole current store**:

1. Collision-confirmed exact-image lookup, including retired exact aliases.
2. The current dominant-orthant shortcut and its normal priority.
3. Independent exact containment verification of every positive winner.

Only general forward containment gets the lower bound. Summary/digest checks,
quarantine, local deduplication, antichains, dependency publication and strict
coverage obligations are unchanged.

## Minimal integration

Use a private invocation-local context from the native controller's existing
snapshot publication and dispatch session. After existing P1 job checks, carry
an optional eligible negative-prefix bound into P2. Do not reconstruct this
eligibility merely from deserialized `LookupReport` fields.

Unbound, reference, synthetic AllMiss and raw restored stale results do not
receive the new suffix shortcut and retain full lookup. The existing same-view
exact-only shortcut and its validation contract remain unchanged. Resume
reissues unfinished jobs with new session/sequence/version/watermark bindings;
only freshly dispatched jobs may acquire the new eligibility.
No checkpoint schema change, retained per-query proof, extra snapshot-root pin
or authentication framework is intended.

This context is **not a certificate that arbitrary worker bytes are truthful**.
An invented nonexact negative may create redundant unresolved work or different
IDs; the existing same-view shortcut already has that limitation. It cannot
discharge a dependency or certify closure. Positive coverage remains checked.
Exact winner equivalence is required for legitimate worker results, not claimed
for arbitrary forged negative contents.

## Tests and decision rule

- Differential full versus suffix lookup over append/retirement sequences,
  layered snapshots and group reorderings; compare exact hit kind and ID.
- New exact/nonexact containers, retired exact aliases, dominant orthants and
  quarantine exclusions.
- Old-session rejection, unbound fallback, and fresh replay eligibility after
  restore/rescue.
- Invalid metadata, forged negatives, malformed positives and validation-error
  precedence; no false coverage may result.
- Cancellation leaves no partially published plan. Physical scan counters and
  polling frequency may decrease; they need not remain byte-identical.
- Fixed-cut canonical graph/record comparisons, then optimized four-loop and
  representative five-loop controls against the same baseline.

Reject or revise the implementation if old IDs can become newly eligible,
session binding is insufficient, or canonical positive selection changes for
legitimate results. Park it if saved scans do not improve total campaign cost.
CPU utilization alone is not a success criterion.

The earlier frozen old/new compact-row benchmark finished independently;
its results do not validate this later implementation.

### Executed correctness checks

The cached release test build completed in 1,057.516 seconds with the
application package at opt-level 1, followed by 120.175 seconds for the guarded
test sequence. This is a correctness profile, not a performance comparison.
The strict inherited Symbolica license setting was enabled; there were no
license-related skips, compiler errors or guard stops.

| Test group | Passed | Existing ignored |
|---|---:|---:|
| Targeted stale-prefix tests (also included below) | 10 | 0 |
| Full application library | 1,301 | 12 |
| Candidate CLI | 19 | 0 |
| Routed-campaign CLI | 6 | 0 |

The full suite includes real native snapshot-versus-AllMiss equivalence,
rolling interruption/resume in both width directions, durable result-escrow
restore, quarantine refusal, saved-sequence replay and cancellation checks.
The new layered-snapshot test compares actual P3 records, dependencies and
ledger output. The forged-negative test verifies that missing reuse creates
pending work, not false closure. The focused ten are a subset of the 1,326
unique passes, not ten additional unique tests.

Evidence is under `TMP/postlaunch-20260930/stale-negative-prefix/` in
`native-build/` and `native-tests/`. Both owned process groups fully drained.
These checks establish the tested correctness boundary. The optimized
four-loop results below add completed-work evidence; the five-loop results
remain different-prefix bounded screening, not completed-scope verification.

### Optimized four-loop comparison

The campaign-profile build (opt-level 3, fat LTO, one codegen unit, no package
override) finished in 3,702.453 seconds, excluded from all run timings. The
frozen candidate is `2cd97ff7`, source `f328844f`; the contemporaneous baseline
is P1+compact `0f2536a7`, source `58e63614`. This is **not** a Ready comparison.

Both builds used the same 16 owners, 508 routes, 58 required queries and 32
roots, with 16 workers on CPUs 32–47, 15 inspectors, no preparation helpers,
and profiling disabled. Each arm completed, passed cold-All reinspection of
all 17,957 native inspections, and had zero violations, frontiers or pending
work. Both pairwise CP6 mathematical-state comparisons were equal, including
canonical record/edge digests. Full typed-record bytes were not compared.

| Run order | Native launch through drain, s | Cold-All, s | Primary total, s | Traversal, s | P2 source, s |
|---|---:|---:|---:|---:|---:|
| Baseline 1 | 7.551 | 8.140664 | 15.691664 | 3.755685 | 0.519947 |
| Candidate 1 | 6.754 | 8.141529 | 14.895529 | 3.748644 | 0.523784 |
| Candidate 2 | 7.154 | 9.137257 | 16.291257 | 3.756996 | 0.523804 |
| Baseline 2 | 6.755 | 8.141198 | 14.896198 | 3.797546 | 0.519761 |

Primary medians are 15.293931 seconds baseline and 15.593393 candidate
(candidate +1.96%). Pairwise changes are −5.07% and +9.37%; native traversal
and P2 remain effectively flat. Source resolution is slightly higher in both
candidate arms. This is **no demonstrated four-loop speedup**, not reliable
evidence of a two-percent regression. Launcher/finalization and cold timing
variation matter at this short duration; retain the original timing boundary.

All arms produce the same 26,025 domains, 495,898 edges, 872,486 events,
731,181 P2 rows and 1,123 cuts. Reported global forward candidates decrease
about 6.38%, while actual test callbacks remain approximately 375,102. These
are aggregate diagnostics, not isolated coordinator cost or unique predicates.
Reduced candidate counts have not translated into reduced measured source time.

Every owned process drained. Host available memory remained above 593 GB;
the short native arms do not establish steady-state utilization or foreign-CPU
contention. Native summary exit 4 and the secondary Python `INCOMPLETE` label
are preserved; independent native cold-All provides the stated scoped
acceptance, not an unrestricted five-loop closure claim.

Raw receipts, both state comparisons and the independent first-pair/ABBA audits are
under `TMP/postlaunch-20260930/stale-negative-prefix/performance/four/`.
`ABBA_RESULTS.md` includes CPU, sampled RSS and inclusive-arm measurements.
Independent ABBA review accepted both correctness and the neutral performance
interpretation. Production is unchanged.

### Bounded full-input five-loop comparison

The same optimized old/new binaries were compared on all 67 A1 owners,
8,246 routes and 183 declared queries (116 required, 67 auxiliary). Both
fresh runs used 32 workers on CPUs 0–31 (31 inspectors, no preparation helpers,
one coordinator), unchanged FIFO/oldest-prefix scheduling, profiling off and
all 78 input pins checked. Neither rules nor inputs were regenerated or narrowed.

Each arm had a 1,200-second cooperative stop and an 1,800-second inclusive
budget covering setup, save, structural cold checking and process drainage.
Both stopped cleanly, saved a resumable generation-1 checkpoint and passed
structural cold checks with zero violations. They did **not** undergo full
native reinspection: raw cold exit 9 / `INCOMPLETE` / reinspection `None` are
retained. No root was independently verified closed by these checks.

| Metric | Baseline 0f | Candidate 2cd |
|---|---:|---:|
| Inclusive arm, s | 1,428.792 | 1,439.186 |
| Native launch through drain, s | 1,207.798 | 1,209.831 |
| Preparation / traversal, s | 65.725 / 1,136.059 | 65.736 / 1,137.339 |
| Structural cold check, s | 218.087 | 227.401 |
| Native inspections | 3,691,024 | 3,796,448 |
| Committed / scheduled domains | 6,619,452 / 9,583,667 | 6,789,442 / 9,822,219 |
| Pending domains | 2,964,215 | 3,032,777 |
| Accepted P2 rows | 183,317,557 | 188,630,476 |
| P2 / source resolution, s | 320.754 / 224.940 | 293.245 / 191.430 |
| P2 / source per row, ns | 1,749.718 / 1,227.053 | 1,554.602 / 1,014.842 |
| Sampled native-plus-supervisor CPU, s | 6,444.43 | 6,600.43 |
| Mean sampled native cores | 5.344 | 5.464 |
| Native-guard sampled process-tree peak RSS, GB | 15.106 | 15.373 |

Source time per accepted row falls **17.29%** and P2 time per row **11.15%**.
The 33.51-second raw source-time reduction occurs despite 2.90% more rows,
consistent with the intended prefix optimization. However, P1, reverse-index
work, P3, boundary maintenance and publication waiting increase. Native
inspections rise 2.86%, or 2.68% per native-launch-through-drain second
(2.74% per recorded traversal second); committed domains rise 2.57% and
pending work 2.31%. CPU rises 2.42%, and the reached graphs differ. This does
not establish reduced total domain work, faster eventual closure or a useful
twenty-core scaling result. The preregistered 10% throughput screen is not met.

Global forward candidates per lookup classification decrease 13.19%; actual
test callbacks per classification decrease only 0.64%. Those counters combine
inspector, admission and coordinator work and include bulk-prefilter rejection.
They cannot isolate P2 comparisons or measure suffix eligibility. The same
native source tasks also encounter different geometry/work mixes, so per-row
normalization is useful evidence, not fixed-work causal isolation.

Minimum sampled host availability was 573.68 / 565.23 GB, with production and
LC2 active on disjoint cores. Foreign-CPU contention was not independently
measured. CPU excludes cold checking and the outer Python adapter; RSS is
sampled, not an exact peak. All owned processes drained. This is one pair,
with no actual checkpoint resume, repeat or unrestricted closure claim.

Raw receipts and formulas are in
`TMP/postlaunch-20260930/stale-negative-prefix/performance/five/FIRST_PAIR_RESULTS.{md,json}`.
Independent review accepted the protocol, arithmetic and deliberately limited
interpretation; no further correction remains.

### Same-start local checkpoint continuation

A separate comparison uses two private copies of the same completed local
pilot checkpoint, not a production checkpoint. The starting state has
3,718,128 native inspections, 9,646,709 scheduled domains and 2,986,261 pending
or reserved domains. Inputs, worker budget and policy match the fresh pair.
Each arm includes copying, restoration, traversal, saving and structural cold
checking within its 30-minute ceiling; the cooperative stop is at 900 seconds.

Both arms completed on October 1, establishing actual productive continuation
within the pilot budget. Baseline is `0f2536a7`, candidate is `2cd97ff7`:

| Component or outcome | Baseline | Candidate |
|---|---:|---:|
| Whole arm, inclusive | 1,269.389 s | 1,284.921 s |
| Private checkpoint copy | 0.528 s | 0.537 s |
| Native process through drainage | 911.115 s | 911.413 s |
| Owner/routing preparation | 64.880 s | 64.991 s |
| Runtime restore phase | 100.758 s | 100.583 s |
| Recorded traversal, restore/save included | 838.935 s | 838.682 s |
| Checkpoint phase | 5.764 s | 5.583 s |
| Structural cold checking | 354.314 s | 369.527 s |
| Additional native inspections | 2,733,674 | 2,896,522 |
| Additional committed domains | 3,959,308 | 4,214,789 |
| Additional scheduled domains | 5,398,249 | 5,710,722 |
| Additional pending/reserved domains | 1,438,941 | 1,495,933 |
| Invocation-local P2 rows | 94,328,650 | 102,925,410 |
| P2 wall time | 241.526 s | 197.125 s |
| P2 source resolution | 162.485 s | 111.311 s |
| Supervised native-tree CPU | 4,636.52 CPU s | 4,857.13 CPU s |
| Mean sampled native cores | 5.10 | 5.34 |
| Supervisor-reported sampled tree peak RSS | 18.174 GB | 18.488 GB |

Native phases are nested within the native process time, not additive to it.
Recorded `traversal_seconds` includes restore/reconstruction/save/report and
is not a useful-walking-only timer. CPU excludes the outer adapter and cold
checking. These are timed continuations, not completed five-loop workloads.

Source cost/row falls from 1,722.55 to 1,081.47 ns (**−37.22%**); P2/row
falls from 2,560.47 to 1,915.22 ns (**−25.20%**). This is a stronger local
signal than the fresh-start pair, consistent with avoiding more stale prefix
work. It is not an isolated causal measurement: the two stopping prefixes
and their row/work mixes differ despite the identical initial checkpoint.

Additional native inspections rise **5.96%**, or **5.99%** per recorded
traversal second (5.92% per native-through-drain second). Added pending rises
3.96%, and CPU rises 4.76%; CPU per new inspection falls about 1.13%.
Pending added per new inspection falls about 1.88%. The raw source phase saves
51.17 seconds and P2 saves 44.40 seconds, while P1, inspection/wait, P3 and
boundary time increase with the changed work. Mean activity remains about
five cores, not twenty. The whole arm is 1.22% longer, chiefly because cold
checking takes 15.21 seconds more. This does not pass the 10% useful-work
screen or establish faster eventual closure. No production switch is advised.

Both arms save resumable generation 2, with no errors, frontiers or abandoned
obligations. Structural cold-None returns INCOMPLETE with zero violations and
zero native reinspections. Both graphs report 13/67 oracle-closed starting
roots, not independently native-verified roots or complete scoped closure.
All owned processes drained and the original local checkpoint controls remained
unchanged. Independent review accepted the arithmetic, lifecycle and these
limits. Foreign-CPU contention was not independently measured; minimum sampled
host availability was 585.63/592.00 GB. This is one pair, not a repeated result.

Full table, formulas, raw receipts and process-drain evidence:
`TMP/postlaunch-20260930/stale-negative-prefix/performance/resume/PAIR_RESULTS.{md,json}`
and `PAIR_DRAIN.json` in the same directory.

### Mature production feedback

A read-only sample from the unchanged repaired production run spans October 1,
04:13:13–04:23:15 UTC (300 telemetry frames, sequences 9805–10104). Mean native
CPU use is 3.33 cores of 32 reserved. Mean active inspector callbacks is 1.81;
225 frames report none. Returned results awaiting publication average 324.22.
These returned results include recycled reservations and the current uncommitted
cut; they are not independently publishable proofs or recursively closed domains.

Actual accumulated coordinator phase clocks over the nearly identical native
interval, rather than dashboard-label frequencies, show:

| Coordinator phase | Seconds | Share |
|---|---:|---:|
| P1 validation | 58.23 | 9.69% |
| P2 preparation | 353.24 | 58.80% |
| P3 publication | 115.27 | 19.19% |
| Inspection/prefix/publication waiting | 45.00 | 7.49% |
| Boundary maintenance/refill | 28.96 | 4.82% |

This supports a coordinator/merge bottleneck in that interval, rather than
primarily an expensive inspector straggler. The clocks are coordinator wall
occupancy, not thread CPU; worker activity can overlap them. Dashboard activity
samples are up to five seconds old, while the recursive-closure snapshot is
much older. An unchanged closure count therefore does not prove no progress.

P2 occupies a larger share than in the fresh bounded pilot (roughly 27–28%).
However, live events do not export P2 source/reverse/dedup subphase clocks.
Neither the stale-negative frequency nor the source-only mature share can be
deduced. Do not transfer the earlier source/P2 ratio or predict a speedup from
these totals. Existing preparation helpers and prefix reuse target relevant
work, but their mature benefit remains unmeasured. The prior fresh helper
screen's weak overall gain remains a valid negative result, not superseded by
these observations. No new profiler attachment or production change was made.

Definitions, bounded evidence windows and source references are in
`TMP/postlaunch-20260930/stale-negative-prefix/MATURE_POOL_TELEMETRY_INTERPRETATION.md`.

### Four-loop opportunity limit

The earlier optimized Ready/Epoch ABBA still fails its primary non-regression
gate: Epoch is 9.63% and 9.89% slower. A receipt-only decomposition of the mean
1.386618-second gap is:

| Component | Epoch minus Ready, seconds |
|---|---:|
| Recorded native traversal | +1.316925 |
| Recorded preparation | -0.079670 |
| Internal cold-verifier total | +0.529294 |
| Combined external/untimed residual | -0.379930 |

The residual combines startup, untimed finalization and completion detection;
it is not a measurement of removable polling overhead. The primary boundary
is unchanged, and the consistent traversal penalty is real in these receipts.

Epoch's entire four-loop source-resolution phase averages 0.534137 seconds.
Prefix reuse targets a subset, so its direct savings alone cannot close the
measured fixed-trace gap. The roughly 2.668-second inspect/wait phase includes
oldest-prefix waiting and overlaps worker activity; it is not all removable
overhead. Changed timing might change overlap or the reached work, but that
requires whole-campaign evidence, not an assumed multiplier. The larger
five-loop opportunity must therefore be evaluated separately.

The later P1/compact-row comparison is neutral and is not a replacement for
the Ready control. Local detailed accounting is in
`TMP/postlaunch-20260930/FOUR_LOOP_GAP_COST_BUDGET.txt`.

### Follow-up candidates not justified by the current evidence

Two inexpensive source/report investigations prevent reopening speculative
optimizations from ambiguous counters:

- Source preparation already flattens tasks across all checked entries in a
  publication cut. Roughly two tasks per wave with zero preparation helpers
  follows the existing `2 * max(helpers, 1)` wave width, not a per-entry
  barrier. Existing two-helper data gives about 3.92 tasks per wave. A join
  and ordered fold remain, but no measurement establishes wave granularity
  as the bottleneck. Do not implement duplicate cross-entry batching.
- In the measured fresh full-A1 prefix arm, about 91.15% of accepted source
  rows carry stored targets, but their second exact containment check is not
  separately timed. The retained sampled
  profile highlights index-forward/envelope work, which the stored-positive
  branch does not execute. Neither that count nor aggregate source time
  establishes the cost of positive revalidation. Native positive-proof reuse
  remains parked pending attribution; a negative-prefix eligibility marker
  cannot authorize a positive that discharges a mathematical obligation.

The source does discard the inspector's checked positive token before byte
transport and reverify it during merge. Removing this check safely would need
native result/proof ownership bound to the exact query, target, snapshot and
session, with unchanged untrusted/restored fallback and quarantine handling.
Its transport/memory cost is unmeasured. These are reopening conditions, not
an approved implementation or a predicted speedup. No additional capture,
native run, build or production mutation was performed for these inquiries.

Local reports: `SOURCE_WAVE_GRANULARITY_INTERPRETATION.md` and
`NATIVE_POSITIVE_PROOF_REUSE_EVIDENCE.md` under
`TMP/postlaunch-20260930/stale-negative-prefix/`.

The existing frozen candidate already contains a bounded exact cross-entry
query census, enabled by `RUSTRED_EPOCH_PROFILE=1`; no rebuild is needed.
The completed performance comparison arms kept profiling off, so omitted
census output there does not mean zero duplication. A separate local-checkpoint
diagnostic adapter passed independent review and14 pure/mock tests. Its single
diagnostic run has now completed and passed independent interpretation/lifecycle
audit; the results below are observations, not an additional performance pair.
It preserved
the full input and resource policy, enables profiling only for the native resume invocation,
and keeps structural cold checking unprofiled. Profiling enables additional
inspection/wait diagnostics too, so it is not an isolated observer-overhead test.

The census samples global epoch versions below eight or divisible by 64, with
bounded source-order prefixes (16,384 rows, 262,144 exact comparisons, 8 MiB
vector capacity per selected cut). Counts distinguish stored proposals,
current-view misses and stale/unprobed misses, with geometry and same-context
repeats reported separately. Truncation can conceal later repeats; neither
counts nor matching context confer reusable proof authority or demonstrate
saved time. The diagnostic is therefore a way to test an opportunity, not
approval for a broad lookup cache. Details and the reviewed command are in
`TMP/postlaunch-20260930/stale-negative-prefix/cross-entry-diagnostic/README.md`.

### Completed duplicate-query diagnostic

The full-scope, same-original-checkpoint W32/h0 diagnostic completed in
1,285.140 s inclusive, including private copy, cooperative native save and
structural cold read. All183 queries and the67 owners/8,246 routes remain bound.
Native saved generation2 without a frontier or error; cold-None returned
INCOMPLETE with zero violations and no native reinspection. All owned process
groups drained and resource locks were released. This is not scoped closure.

| Observed source stratum | Rows | Cross-entry identical image | Identical image and target/snapshot context |
| --- | ---: | ---: | ---: |
| Stored proposals | 1,295,568 | 134,638 (10.3922%) | 123,454 (9.5289%) |
| Stale/unprobed misses | 108,753 | 3,895 (3.5815%) | 3,583 (3.2946%) |
| Current-view misses | 0 | No denominator | No denominator |
| All observed rows | 1,404,321 | 138,533 (9.8648%) | 127,037 (9.0462%) |

The observer sampled2,829 of181,036 successful P2 preparations. Of1,473,324
eligible rows in sampled cuts,95.3165% were observed; four truncated cuts
skipped69,003 rows. Periodic cut sampling and source-order prefixes can bias
these rates. Failed P2 is excluded; observed successful preparation need not
have reached publication. The last16 detailed samples are retained, not the
full sample history. Counts are not weighted by operation cost.

Decision: this does not establish a large duplicate-only scaling opportunity.
Stored positives dominate repetition; safe positive-proof reuse still needs
separate cost evidence and correct authority transport. Expensive repeats or
later censored rows could differ, so this is not a universal rejection either.
Do not multiply the observed frequency by P2 wall share and call it a speedup.
No new cache or additional diagnostic run is justified by these counts alone.
Prioritize downstream-aware generated rules alongside measured source/reverse
lookup costs. Full local receipt: `cross-entry-diagnostic/RESULTS.md`; independent
audit: `TMP/postlaunch-20260930/DIAGNOSTIC_AND_SINGLETON_INDEPENDENT_AUDIT.txt`.

## Source and evidence pointers

- `walking/epoch/merge.rs`: `resolve_miss` and existing P1 checks.
- `walking/epoch/store.rs`: `lookup_controlled`, exact and orthant priority.
- `walking/queue/index.rs`: `find_from` / `find_controlled(first_id)`.
- `walking/queue/prepared.rs`: Ready's watermark precedent.
- `walking/epoch/dispatch.rs`: fresh replay session binding.
- `walking/epoch/checkpoint/restore/runtime/rescue.rs`: pre-worker amendments.

All source paths above are relative to
`crates/rustred-app/src/application/routed_campaign/`.
Historical timing evidence is in ignored
`TMP/postlaunch-20260930/full-a1-preparation-h0-h2/`; profiling evidence is in
`TMP/postlaunch-20260930/digest-perf-tooling/`. The independent mathematical
review was by `final_requirements_audit`; the independent source/lifecycle and
cost assessment was by `runtime_order_pilots`. The latter implemented the
isolated patch; the former independently reviewed the code and tests. Native
correctness checks and optimized four-loop controls have passed. Five-loop
screening shows a local source-cost reduction, not a qualified whole-campaign
gain. Root coordinates
integration and final verification, without modifying production.
