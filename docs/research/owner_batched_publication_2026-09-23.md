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
with the scheduling policy or worker count.

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

The prototype deliberately retains a **chunk rendezvous**: it obtains one
bounded chunk or completion from each selected producer before delivering
that batch. A slow job that has not emitted its next chunk can therefore still
hold up other producers. This is not yet a fully asynchronous sector scheduler.

## Remaining scaling constraints

- One active native producer is allowed per phase/owner bucket. A hot sector
  cannot by itself occupy all inspection workers.
- Destination admission is parallel across buckets, but requests to one hot
  destination are still processed serially within that bucket.
- With 50 configured compute slots and unlimited comparison budget, the split
  is 25 inspectors, 24 admission helpers and one coordinator. This allocation
  is a limit, not measured occupancy, and both pools need ready work.
- Chunk barriers, coordinator accounting and memory bandwidth can limit the
  benefit even when many distinct sectors exist.

If measurements show chunk waiting dominates, the next bounded experiment is
ready-stream delivery rather than waiting for every selected stream. It must
preserve source-event ordering, pending deliveries, fair scheduling and exact
global termination. If a single bucket dominates, within-owner domain
concurrency is a separate issue; removing global ordering alone is not enough.

## Validation and timing boundary

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
existing diagnostic ignored), in 6.57 seconds for the overall suite. Actual
saved-rule comparisons are now running.

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
complete with zero frontiers/errors/pending obligations. Descendants reach R=1;
they are not clipped to the starting rank. Existing saved rules/routes are used
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

## Full five-loop attempt remains distinct

At heartbeat 21,998.052 seconds, the unchanged full campaign had inspected
6,776,948 native regions, with 8,160,683 native obligations pending and zero
observed frontiers. This executable contains neither the new publication
policy nor the recently committed coordinate-block admission filter.

The growing queue does not establish a completion ETA, and zero observed
frontiers does not establish closure. The prototype must pass native tests
and demonstrate useful progress before replacing this campaign. Full finite
completion still requires exhausted work, no pending deliveries, discharged
responsibilities, no errors/frontiers and the planned narrow runtime handoff.
