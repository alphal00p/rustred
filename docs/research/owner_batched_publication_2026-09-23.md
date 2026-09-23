# Sector-local publication: implementation and measurement status

## What this changes

The opt-in `--publication-policy owner-batched` separates mutable admission
queues and responsibility ledgers by `(phase, owner)`. Inspectors share one
immutable set of loaded rules, routing maps and initial-domain geometry. A
successor for another sector is admitted to that destination's queue; it is
not privately solved again by every originating family. Native inclusion and
delegation remain responsible for deduplication within the destination.

This removes the requirement to publish complete inspections in one global
ID order. That order was a scheduler policy, not a mathematical dependency
between all sectors. Exact rules and concrete reductions do not change.
Diagnostic IDs, domain fragmentation and resource-capped prefixes can change
with the scheduling policy, worker count or readiness interleaving at a fixed
worker count.

## Why the opportunity is substantial, but not yet a measured speedup

The existing ordered run can retain finished inspections while a much earlier
inspection is still publishing. An independent 899-second sample ending at
20,354.720 seconds saw a median of 181 finished-but-unpublished inspections
(maximum 253), and only 3.34 active inspectors on average out of 25. This is
evidence of scheduling pressure, not a measurement of ready independent work.
The same sample observed 32 publisher keys over time, but one Apply owner
accounted for 80.09% of publisher time. These are *publisher* observations:
they do not establish either 32 simultaneously ready owners or an 80% share
of the total remaining computation.

Independent sectors provide a plausible way to overlap useful work. For
example, while an Apply job in sector A is expensive, sector B can emit Route
successors to sector C, where other ready work can proceed. Shared immutable
rules do not need to be duplicated, and B need not wait for A to finish its
whole inspection. Incoming obligations can reopen an otherwise empty C queue.

The first published prototype retained a **chunk rendezvous**: it obtained one
bounded chunk or completion from each selected producer before delivering
that batch. A slow job that had not emitted its next chunk could therefore
hold up other producers. The tested ready-stream follow-up below removes this
barrier, but retains synchronous destination admission and one producer per owner.

## Remaining scaling constraints

- One active native producer is allowed per phase/owner bucket. A hot sector
  cannot by itself occupy all inspection workers.
- Destination admission is parallel across buckets, but requests to one hot
  destination are still processed serially within that bucket.
- With 50 configured compute slots and unlimited comparison budget, the split
  is 25 inspectors, 24 admission helpers and one coordinator. This allocation
  is a limit, not measured occupancy, and both pools need ready work.
- Coordinator accounting, synchronous destination admission and memory bandwidth can limit the
  benefit even when many distinct sectors exist.

Ready-stream delivery now preserves source-event ordering, pending deliveries,
fair scheduling and exact global termination without waiting for every selected
stream. If a single bucket dominates, within-owner domain concurrency is still
a separate issue; removing global ordering alone is not enough.

## First prototype validation and common timing boundary

Independent source review passes for destination-local admission, aggregate
event/domain/comparison allowances, cancellation, initial anchors and retained
failure responsibilities. The first integrated release run passed 491 library
tests, with one failure and one existing ignored diagnostic. The failure was
a cancellation diagnostic spelling mismatch; no false closure was reported.
Its correction, initial-comparison accounting and a common timing boundary
have been reviewed. The second release library gate passes **495 tests**, zero
failures and one existing ignored diagnostic, in 45.08 seconds. Do not describe
the first run as a complete passing gate.

The focused owner-batch gate passes 24 tests in 0.10 seconds, exercising native
controls with 1, 2, 6 and 50 workers without availability skips. Seven CLI parser
tests and 27 Python steering tests pass. These are correctness tests, not a
50-core performance result. The release CLI build also passes. The separate
clean-owned source gate passes 14 focused tests and 160 overall tests (one
existing diagnostic ignored), in 6.57 seconds for the overall suite. Completed
saved-rule comparisons are reported below.

The existing application/CLI parity and routed-campaign integration suites
also pass: eight tests, zero failures. These retain the default ordered path
and its existing error/cancellation behavior.

The Cargo gate uses the shared working tree, including unrelated pre-existing
ordered-escrow work which is excluded from this milestone. The narrow independent
gate instead uses the committed worker pool plus the owned wait helper and the
new owner-batch modules. It does not substitute for public CLI execution or
exercise the top-level public walker. The source ownership audit confirms that
the staged engine/pool match that snapshot and excludes unrelated escrow,
vendor/reference changes and formatting.

Compare **both policies from the same executable** on identical saved inputs.
The matched `traversal_seconds` begins after owner preparation and includes
initial admission, inspection/publication, ledger/report construction and
queue cleanup. It excludes owner unloading and output writing. The separate
`native_driver_seconds` field is not an equivalent timing boundary. Whole
process wall/CPU/RSS measurements should also be retained.

Start with real routed two-loop controls, then a saved four-owner five-loop
subset. Keep all descendants even when they exceed the starting rank bound.
Record complete versus capped/incomplete outcomes, useful native throughput,
pending trend, admission and barrier time, ready-owner diversity and RSS.
Changed domain counts mean equal diagnostic prefixes are not necessarily
equal workloads; completed identical starting domains are the stronger
comparison. No rule generation is involved.

## Completed routed two-loop controls

Both policies were run from the same frozen release executable on each of two
unchanged saved inputs, with one and six workers. All eight runs completed with
zero frontiers/errors/pending work and discharged ledgers. Saved rule/input
hashes remained unchanged. Native inspection counts were 26 for the baseline
input and 19 for the staircase input under every policy/worker configuration.

| Saved input | Workers | Ordered traversal (ms) | Owner-batched traversal (ms) |
|---|---:|---:|---:|
| Baseline | 1 | 4.694 | 3.128 |
| Baseline | 6 | 3.262 | 3.802 |
| Staircase | 1 | 2.342 | 2.784 |
| Staircase | 6 | 2.122 | 3.747 |

These are single observations of millisecond controls, **not a scaling or
speedup result**. Parallel overhead is visible, and the first ordered process
also incurred 1,757 major page faults versus 0–84 for the remaining processes.
The shared host and excluded pre-existing ordered-escrow work further limit
generalization to the pushed snapshot or the full five-loop campaign.

Every owner-batched run exercised 32 actual cross-owner successor deliveries;
the six-worker runs entered the parallel admission path in 26 and 22 batches.
That establishes path execution, not multiple busy helpers or fifty-core
utilization. The matched traversal boundary above was checked in every receipt.

Evidence: `TMP/owner-batched-comparison.DDcUMp/integrated-two-loop-run-01/`.
The frozen CLI is `TMP/owner-batched-release-v2.VK6nz6/rustred`, SHA256
`9e32efa792111f3cc2ed4f4209469077f9b28bc641de312051d566b7ce37ca9a`.
The initial five-loop launch rejected a missing query schema before owner
loading. This setup error is neither a missing-rule frontier nor a solver timing.

## Completed small five-loop control

The corrected four-owner input restricts starting keys to A<=9, R=0, A-R>=9:
162 starting tuples represented as four correlated regions. All four runs
complete with zero frontiers/errors/pending obligations. Scheduled descendant
regions have rank bounds up to R=1, without clipping to the starting rank;
this does not itself prove a concrete rank-one key is reached. Existing saved rules/routes are used
unchanged, with initial-D-band reuse enabled.

| Workers | Ordered traversal (s) | Owner-batched traversal (s) |
|---|---:|---:|
| 1 | 0.44546 | 0.47450 |
| 6 | 0.17447 | 0.40732 |

Ordered processes 621 native regions and owner-batched 620; both process 378
Route regions and 25 partial-initial inspections. The new policy uses 28 bucket
keys and delivers 1,056 cross-owner requests. Six-worker owner-batched execution
records 430 parallel admission batches, 0.35190 s chunk-barrier waiting and
0.01165 s delivery within a 0.39765 s native-driver interval. Waiting includes
legitimate concurrent native computation and cannot simply be called wasted CPU.
Preparation takes 2.68–2.76 s per process and is excluded from these matched
traversal timings.

This single small shared-host trial shows **no gain from owner batching**;
its six-worker traversal is slower than ordered. It does not establish fifty-core
performance or the total five-loop runtime. Combined with the code's demonstrated
all-producer rendezvous, it motivates bounded ready-stream delivery next: poll
all active streams, deliver the ready subset, publish/refill completed producers,
and wait only when no stream is ready. Keep source FIFO/acknowledgment, exclusive
destination mutation, global budgets, cancellation and pending responsibilities.
Readiness-driven diagnostic order/fragmentation may vary even at a fixed worker
count; mathematical artifacts and concrete reductions remain unchanged.

Evidence resides under `TMP/owner-batched-comparison.DDcUMp/` in the four
`five-a9-*-run-*` receipt directories. These are small integration controls,
not replacement runs for the full 67-owner A24/R15/D9 campaign.

## Larger rank-one control and ready-stream follow-up

The same frozen executable also completes both six-worker policies for
A<=10, R<=1, A-R>=9: exactly 3,852 starting tuples represented by four regions.
Both exhaust their obligations with no errors/frontiers; scheduled descendant
region rank bounds reach R=2, not a claim that a concrete rank-two key is reached.
Independent raw-receipt review verifies both this pair and the four A9 controls.

| Six-worker policy | Matched traversal (s) | Native regions | Delegated aliases | Scheduled IDs |
|---|---:|---:|---:|---:|
| Ordered | 2.372589 | 7,432 | 1,360 | 8,792 |
| Owner-batched, chunk rendezvous | 6.982075 | 8,518 | 365 | 8,883 |

The changed schedule changes the intermediate covers and native work count;
these are equal starting workloads, not identical native-job streams. Neither
the 20,000-domain nor the one-million-event diagnostic allowance was reached.
The new policy uses 65 phase/owner keys and 25,510 cross-owner requests, with
5.813670 s rendezvous waiting and 0.280267 s delivery in its 6.783586 s driver.
This is another negative performance result for the published prototype.

A read-only thread sampler observes a 6.616-second heartbeat-labelled walking
window for owner-batched execution: about 1.147 busy cores in total, comprising 0.921 native
inspection, 0.141 coordination and 0.075 admission-helper cores. These sampled
categories are not exact additive job timings; stale heartbeat labels can include
finalization/unload/output tails. Two Apply owners dominate visitor
wall time; only about 0.213 s is charged to all per-owner admissions together.
This points to native scheduling/owner concurrency before more admission work
on this input, but does not prove ready streams will cure the imbalance.

Only owner-batched execution had this observer attached; ordered finished before
attachment. Observer overhead was 0.309 CPU seconds across 9.729 elapsed seconds.
There is no measured per-role ordered counterpart and no statistical scaling
claim from this single shared-host pair. Future comparisons arm the same observer
before both processes. Detailed resources and receipts are in
`TMP/owner-batched-comparison.DDcUMp/A10_OLD_PROTOTYPE_RESULTS.md`.

The follow-up source now polls each active stream once without blocking, delivers
only ready chunks, acknowledges complete producers and refills their slots.
It waits on a mutex-protected any-stream condition only when none is ready.
Buffers, exclusive destination admission, one producer per owner and worker
budgets remain bounded. The metric becomes `idle_stream_wait_seconds`, explicitly
coordinator waiting with no active stream ready, not the old rendezvous wait or
summed worker idle time. Diagnostic interleavings may vary even at fixed worker
count; saved mathematical rules and exact reductions remain unchanged.

Independent source review passes. The release gate passes 501 library tests
(zero failures, one existing diagnostic ignored) in 45.46 s; its focused subsets
pass 28 owner-batch and three ready-stream tests, included in the total. The
release CLI builds successfully. The separate clean-owned optimized gate passes
166 tests (zero failures, one ignored), including all six new regressions,
without the pre-existing escrow scheduler source or hook. These are overlapping
test suites, not 667 distinct tests. Evidence is under
`TMP/ready-stream-release.rKqs5G/` and `TMP/clean-ready-owner.Kech66/`.
The frozen release CLI SHA256 is
`071efef5bd8505a1a316c992f3feaecb7b8db551dd5177a9ba2a8c39071525f6`;
it includes the separate existing ordered-escrow work in the shared tree.
Controlled production-coordinator tests require a fast peer to deliver multiple
chunks and actually run its next job before a silent peer is released; separate
controls cover cancellation, failures, event limits and wait notifications.
The completed measurements below do not establish a fifty-core improvement.

## Completed ready-stream comparisons

All twelve fresh runs use the same frozen `071efef5` executable, unchanged
saved rules/routes, identical per-input limits, and a symmetrically prearmed
read-only observer. The four A9 controls and eight A10 controls all exhaust their
scoped worklists without errors/frontiers or unresolved ledger responsibilities.
No rule generation or elapsed supervisor deadline is involved. Descendants remain
unclipped. These are finite controls, not the full 67-owner workload.

| A9/R0/D9, 162 starting tuples | Ordered (s) | Ready-stream owner-batched (s) |
|---|---:|---:|
| One worker | 0.44144 | 0.47602 |
| Six workers | 0.17816 | 0.32137 |

A10/R1/D9 has 3,852 starting tuples. After the first correctness pair, three
fresh six-worker pairs alternate policy order. Their median matched traversal
times are **2.30089 s ordered versus 4.56399 s ready-stream**; the median paired
ready/ordered ratio is **1.98357**. The repeat ranges are 2.21836–2.32269 s and
4.05655–4.82012 s respectively. These are small shared-host observations, not
confidence bounds. Including the first pair gives medians 2.31179/4.47117 s;
do not confuse the ratio of those medians with the median paired ratio.

Across all four A10 pairs:

| Quantity | Ordered | Ready-stream owner-batched |
|---|---:|---:|
| Native regions | 7,432 | 8,522–8,553 |
| Delegated aliases | 1,360 | 368–375 |
| Sampled busy cores, heartbeat-labelled window | 2.989–3.044 | 2.066–2.138 |
| Native-inspector portion of busy cores | 2.475–2.621 | 1.649–1.754 |
| Process peak RSS (KiB) | 195,216–204,320 | 213,460–216,760 |

RSS is GNU-time supervised-process peak, not a sum of simultaneously sampled
tree members. CPU windows use the last heartbeat's walking label: stale progress
can include finalization, unload or output. They are observational windows, not
exactly isolated traversal or native-job boundaries. Both policies use the same
observer; the host also runs the unchanged full five-loop attempt. The shared
binary includes the pre-existing Ordered escrow work excluded from this source
milestone.

Ready-stream coordination waits with no ready source for 2.649–3.226 s and spends
0.413–0.657 s delivering chunks. It visits 65 phase/owner keys, but this lifetime
count does not imply 65 simultaneously runnable sectors. Two Apply owners dominate
native visitor wall time, about 3.0–3.8 s each; their per-owner admission times are
only about 0.042–0.100 s. Visitor wall includes blocking, so these numbers alone
do not isolate pure computation. Together with the CPU samples, they motivate
examining concurrent inspections within a hot owner before adding admission
threads. The readiness scheduler also produces more native work and fewer
delegated aliases; useful-work amplification must be included in the assessment.

**Decision:** retain Ordered as the default and do not replace the full campaign
with this prototype. The barrier removal is independently tested and useful as
infrastructure, but has not solved utilization. The next bounded experiment
should examine within-owner native concurrency and compute-budget allocation,
preserving source responsibility, local publication order, global limits and
cross-owner reuse. No hardware-saturation or full-completion estimate follows
from these controls. Raw evidence is in
`TMP/owner-batched-comparison.DDcUMp/ready-stream-matrix/`.

### Reservation-window follow-up: no engine changes

Four additional completed A10 controls vary the existing positive lookahead H,
using the same frozen executable, entry/routing/rule inputs, six-worker budget,
observers and native allowances. All four exhaust their worklists with zero
errors/frontiers and discharged obligations. These are single observations:

| Policy | H | Traversal (s) | Native regions | Delegated aliases |
|---|---:|---:|---:|---:|
| Ordered | 1 | 5.90121 | 7,171 | 1,598 |
| Ready-stream | 1 | 3.89936 | 7,778 | 1,083 |
| Ordered | 4 | 4.04349 | 7,174 | 1,595 |
| Ready-stream | 4 | 3.91356 | 7,801 | 1,042 |

The owner-local H256 policy protects up to 256 queued IDs **per owner** from
delegation, despite running only one native job per owner. Smaller H reduces
ready-stream native work by about 9% and increases aliases versus its H256
controls. This supports excess reservation as a contributor, not the entire
explanation. H1/H4 ready-stream remains slower in these observations than the
earlier Ordered H256 controls; that cross-parameter comparison is not paired.

H also limits dispatch: Ordered H1 admits only its global cursor job, whereas
ready-stream H1 can run one job in each owner. The above rows are therefore not
an isolated retirement-policy comparison at equal effective concurrency.
Heartbeat-labelled CPU windows show approximately 1.05/2.12/1.51/2.14 busy cores
in table order, with the same stale-progress limitations as above. Neither a
default change nor a fifty-worker restart is justified by these observations.

Independent source review confirms H changes scheduling/reservation only.
Containment proofs, protected initial anchors, started-job protection, forward
representatives, descendant scope and final ledger discharge remain unchanged.
Evidence: `TMP/owner-batched-comparison.DDcUMp/lookahead-study/` and
`HORIZON_SCOPE_AUDIT_finite.md` alongside it. No IBPs were regenerated.

## Full five-loop attempt: subsequently stopped for optimization

At heartbeat 31,592.362 seconds, the unchanged full campaign had inspected
8,130,247 native regions, with 10,239,937 native obligations pending and zero
observed frontiers. This executable contains neither the new publication
policy nor the recently committed coordinate-block admission filter.

The growing queue does not establish a completion ETA, and zero observed
frontiers does not establish closure. The original attempt subsequently stopped
cooperatively, with exit4 and an explicit incomplete result: 8,274,060 completed
native inspections, one cancelled partial, 10,413,595 pending native obligations
and zero observed frontiers. Supervisor time including final reporting is
32,948.858 s; peak sampled aggregate RSS is about 235 GB. It was not hard-killed.
The replacement must pass native tests and demonstrate useful progress. Full finite
completion still requires exhausted work, no pending deliveries, discharged
responsibilities, no errors/frontiers and the planned narrow runtime handoff.
