# Concurrent inspections within an owner

## Motivation and bounded design

The preceding ready-stream controls completed, but did not outperform Ordered.
Two shared Apply owners dominate the small A10 workload. Their cumulative wall
times are spread over 1,110 and 1,800 native records, rather than one indivisible
long job. This motivates testing concurrency, but final record counts do not
prove how many jobs were simultaneously ready. See the
[completed policy and reservation studies](owner_batched_publication_2026-09-23.md).

The opt-in OwnerBatched lane now separates inspection dispatch from publication.
Multiple workers may inspect one owner using the same immutable reducer. Each
owner retains a single FIFO publisher, separate admission index and responsibility
ledger. Only its current publisher's chunks are delivered. Later chunks/results
stay in existing bounded worker slots; no new completion reservoir or duplicate
Symbolica expression graph is introduced.

Selection prefers the least-occupied eligible owner and breaks ties round-robin.
It then uses spare inspector slots for additional work in busy owners. Existing
ledger reservation fences remain authoritative: H1 still permits only one active
inspection per owner, while larger H can admit more. Delegated IDs may be skipped
for dispatch but are published only at their canonical FIFO position.

Default Ordered execution, the native mathematical visitor, loaded rules,
entry/descendant geometry and worker-budget split are unchanged. At six workers,
the configured split remains three inspectors, two admission helpers and one
coordinator. At fifty it remains 25+24+1. These are limits, not measured occupancy.
No CAS feature or topology/loop-count special case was added.

## Failure and completion invariants

Per-owner frontier/refusal diagnostics belong only to its FIFO publisher. The
coordinator waits on eligible publisher tickets, not later ready chunks, avoiding
a spin behind a quiet head. A later native failure still stops the pool promptly.

During failed cleanup, at most the original eligible head can publish a failed
record. Later completed or unavailable native attempts remain explicitly
uncommitted; they cannot advance the cursor, consume head diagnostics or
discharge a responsibility. Partial-initial-overlap metadata is retained even
when its result cannot publish. No alias is published during cleanup merely
to bridge a missing head. Existing aggregate budgets and global termination
requirements remain unchanged.

Metrics distinguish multiple native inspectors from one native publisher per
bucket. Per-owner peak outstanding jobs and a global peak of FIFO-held jobs
describe actual dispatch; outstanding includes unpublished work and is not a
measurement of busy cores. Inline execution does not maintain the pooled
outstanding gauge while its visitor runs. The idle-wait counter covers periods
with no eligible publisher ready, not necessarily no later source data ready.

## Validation status

Independent source/mathematical review passes. The optimized clean-owned retry
passes **176 tests, zero failures, one existing diagnostic ignored** in 8.55 s.
The 28 owner-batch and two pool-wait focused passes overlap that total. All ten
new tests exercise the actual production coordinator or its real selector:

- reverse completion and frontier provenance within an owner;
- bounded later streams and same-owner child inspection before parent completion;
- cancellation, later failure and a panicked/missing head;
- H1 dispatch fencing and alias gaps;
- fairness across ready owners;
- global event exhaustion with an unpublished partial-overlap result.

The first focused gate had 27 passes and one invalid fairness fixture: it varied
an inactive coordinate while requiring numerator rank zero, producing an empty
region correctly removed by containment. The retry varies the active coordinate
and asserts fresh admission, preserving the original scheduling assertion. No
production change was needed. The first full Cargo compilation was intentionally
stopped before completion to incorporate this test-only correction; it is not a
passing gate. The corrected full release library gate passes 511 tests (one
existing ignored) in 46.32 s, including 38 focused owner-scheduler passes in
1.95 s; these counts overlap. Test compilation took 10 min 10 s and is not
solver time. The release CLI build completed in 3 min 29 s. The full gate
terminated successfully at 21:08:49 UTC and rechecked all nine owned source
hashes. The native 1/2/6/50-worker acceptance tests ran without unavailable-budget
messages. Those tests check correctness, not parallel scaling.

Clean-owned evidence is in `TMP/owner-concurrent-audit.ttsLt0/retry/`, with
52 input hashes checked before/after execution. It excludes unrelated escrow
work. The full shared-worktree Cargo gate is in
`TMP/owner-concurrent-release-v2.F7RZhv/` and includes that existing work, which
is not part of this implementation slice.

## Completed six-worker performance controls

The frozen release CLI has SHA256
`58002e5136efc622f8d3a33a6e979550da79e108cfbf8bfe704e741e28c2db7d`.
Both policies use this executable, the same four saved owner programs/routes,
query JSON, H256 reservation horizon and resource limits. No rules were generated.
Native affinity is CPU43 for one worker and CPUs38–43 for six. The read-only
observer uses CPU44 for both policies, unlike historical controls where it
overlapped CPU43. The old full campaign is stopped, but the host has unrelated
work. This is a fresh paired comparison, not an isolated comparison against the
older executable's timings.

The executable includes pre-existing, uncommitted completed-result escrow work
in Ordered. The clean-owned correctness gate excludes it, but these native
timings do not. Thus the table compares the two current worktree policies; it
does not isolate the performance of committed-only code or the effect of this
one change against an otherwise identical scheduler.

All four A9 controls (162 starting tuples) and eight A10 controls (3,852 starting
tuples) completed with no frontiers, errors, pending responsibilities or
uncommitted attempts. Entry bounds do not clip descendants. Timings cover the
existing matched interval after owner preparation through initial admission,
native traversal, publication, report construction and queue cleanup. Compilation,
owner preparation/unload and final output writing are excluded.

| A10 repeat, six workers, H256 | Ordered (s) | Concurrent owner (s) | Ordered native visits | Concurrent native visits |
|---|---:|---:|---:|---:|
| 1 | 2.560058 | 3.976878 | 7,432 | 8,494 |
| 2 | 2.336121 | 3.833151 | 7,432 | 8,573 |
| 3 | 2.334292 | 3.676820 | 7,432 | 8,551 |
| 4 | 2.324137 | 3.444322 | 7,432 | 8,542 |

Repeats 2–4 alternate execution order. Their medians are **2.334292 s Ordered
versus 3.676820 s concurrent owner**, about 1.575 times slower for the new lane.
This is descriptive repeated measurement, not a confidence interval. Concurrent
execution really dispatches three jobs in one owner and retains up to two jobs
behind its FIFO head, but extra native work and head waiting remain. The first
pair's heartbeat-labelled sample averages 2.36 busy cores versus 2.52 Ordered;
these windows are not exact phase-isolated CPU measurements. Dispatch counts
are not CPU occupancy. Default Ordered therefore remains unchanged.

A9 serial timings are 0.442992 s Ordered / 0.471104 s owner-local, and six-worker
timings are 0.179227 / 0.203712 s. These small controls are not full five-loop
solves. A separate completed H4 pair gives 3.989065 s Ordered versus 3.108559 s
concurrent owner, with 7,174 versus 7,855 native visits. The latter improves on
Ordered at the same H4 but remains slower than Ordered H256. H changes dispatch
and reuse together; it is not an isolated retirement-policy experiment.

A bounded same-input fifty-worker comparison also completes. Native affinity is
CPUs0–49 (distinct physical cores), observer CPU50, with the same A10 input,
H256, 20,000-domain/one-million-event allowances and 6/8-GB soft/hard memory
limits. There is no elapsed deadline. These small-control limits are not the
full-campaign resource settings.

| A10 H256, fifty workers, one pair | Ordered | Concurrent owner |
|---|---:|---:|
| Matched traversal (s) | 1.403919 | 2.950783 |
| Native inspections | 7,432 | 8,797 |
| Delegated publications | 1,360 | 288 |
| Sampled mean busy cores | 15.24 | 10.27 |
| Sampled inspector busy cores | 12.43 | 8.62 |
| Sampled admission-helper busy cores | 1.96 | 0.95 |
| Whole supervised-process CPU (s) | 19.61 | 33.31 |
| Whole supervised-process peak RSS (KiB) | 194,684 | 192,888 |

Concurrent owner reaches **25 outstanding native inspections in one owner**
and 24 FIFO-held jobs. Its head-wait wall time is 1.251 s and delivery is 0.503 s.
The dispatch restriction is genuinely lifted, but useful throughput is still
worse: about 2.102 times Ordered's traversal time. CPU figures remain
heartbeat-labelled sampled windows, not exact traversal CPU; whole-process CPU
and RSS include preparation/teardown. One short pair with different native
affinity from the six-worker tests is not a strong scaling model or an ETA.

Both fifty-worker receipts report zero producer-buffer backpressure time.
Ordered reclaims 7,401 completed worker slots through its pre-existing escrow
path (peak 254 retained entries); concurrent owner reclaims none. The latter's
sampled heartbeat reports show 15–17 finished-but-unpublished jobs occupying
slots. These are repeated heartbeat snapshots, not independent pool reads.
This supports investigating **finished-slot retention**, not blaming full
producer buffers. Safe bounded reclamation must retain all buffered effects,
diagnostic ownership and unpublished responsibility while making the physical
worker available. It cannot publish later jobs early or create an unbounded
result reservoir. The two policies also do different work: 110,252 versus
135,414 events and 448 versus 557 partial-initial inspections.

The first fifty-worker harness launch was rejected before starting RustRed:
the Python supervisor accepts comma-separated CPU IDs, not the `0-49` range
notation supplied by the new steering wrapper. The failed launch and observer's
missing-receipt result remain under `matrix50/`; they are not native solver
results. Only the untracked driver was corrected. The completed retry is in
`matrix50-retry/` with fresh receipts and independent syntax/workload review.

Raw receipts and steering are retained under
`TMP/owner-concurrent-controls.tD1zsL/`. Independent receipt auditing checks the
raw native/alias and initial-anchor responsibilities as well as controller
summaries. All sixteen native controls pass independent raw-receipt review.

## Remaining limits

One slow owner head can still block later buffers, and mutation of a single
destination queue remains serial. A fixed helper reservation can also leave
inspector capacity unused. Neither dispatching more jobs nor a passing small
control proves useful fifty-core scaling or full finite-envelope closure. The
all-67-owner campaign must only be replaced after correctness and useful
throughput justify it; the original attempt is now explicitly incomplete and
stopped as recorded in the [active plan](../finite_starting_domains.md).

Keep Ordered default. Before another scheduler redesign, use the completed
controls to separate extra native work from finished-slot retention and the fixed
25-inspector/24-helper split. Most helper capacity is idle in this small case,
but reallocating it is not automatically a speedup: native work, buffered outputs
and serialized destination admission would also increase. A larger saved-input
control is needed to establish stable bottlenecks before any full 67-owner
restart. No new rule generation, terminal minimization or evaluation is implied.

The user's follow-up explicitly prioritizes parallel profiling and optimization
alongside monitored pilots. Investigate bounded finished-slot reclamation in one
lane while an independent lane prepares/runs larger matched saved-input controls
and audits accounting. Require measured elapsed-time improvement, useful work
and memory limits; a larger active-thread count is not sufficient. Source changes
must not alter frozen executables or evidence of an already-running pilot.
