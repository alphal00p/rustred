# Epoch S5: parallel preparation and immutable publication

This document describes the implemented S5 path, not a claim of five-loop
closure or a recommendation to replace a running campaign. The mathematical
scope remains the explicitly declared starting queries and their reachable
obligations. A drained work queue alone is not a closure certificate.

## Validation status

As of 2026-09-30, the core native suite reports 2,864 passed and 32 ignored.
The final application suite reports **1,198 passed, zero failed, and 12 ignored**
on 50 distinct physical cores, with no worker-availability skips. This includes
the repaired callback fixtures, cold verification, resume and cancellation.
The ordering, version handling and 22 controller tests pass, including the
selected-proposal telemetry correction with exact save/publication/replay
assertions preserved. Full receipts and failed attempts remain in the progress log.

Focused preparation, binary framing/sidecar, immutable snapshot, bulk dependency,
record restoration, helper-repartition, and real native replay checks ran. The
first 16-CPU reservation emitted W50 skip markers. The later full suite used 50
distinct physical cores and exercised all ten intended W50 subcases successfully,
with no skip markers. The first optimized matched four-loop comparisons are
complete: runtime is effectively neutral, while the checkpoint is about 61%
smaller. Larger five-loop and current Ready/Epoch qualification remain open;
see [the measured controls](research/final_order_s5_pilots_2026-09-30.md).
The application test build uses optimization
level 1 for correctness checks; it must not supply production performance numbers.

The correction review also found stale nested CP6 metadata in the public CLI
identity probe. Its format and schema now come from the native checkpoint
writer, and both advertised Epoch semantics fields use the same native
constant. The final rebuilt CLI probe, help, candidate/routed integration tests,
Rust application API and ordering-inspector tests all pass. A fresh installed
Python wheel passes all 19 candidate API tests without skips, including cold
subprocess loading and CLI parity; the focused four-test order gate also passes.
These interface checks do not establish a production performance gain.

The combined four-loop saved-rule control also passes independent full
reinspection with zero and two preparation helpers: all 58 required queries and
32 roots, 51,166 domains, 31,826 native inspections and 1,149,999 dependency
edges. Both arms have identical durable mathematical state and lookup/verification
diagnostics, with no frontiers or pending obligations. This opt-level-1 smoke
overlapped compilation: it establishes scoped correctness, not a timing gain or
unrestricted four-loop closure.

The finite five-loop saved-rule control also passes with both helper counts:
all 743,502 native inspections are independently repeated over 910,957 domains
and 6,861,296 dependency edges, with zero uncovered obligations. Its single
required query/root is covered, and both runs retain identical durable domain,
dependency and anchor state plus lookup/verification diagnostics. This control
loads 67 owners but covers only the declared R2/A11 query, not the 116-query
production scope. Cached recursive-closure counts differ because maintenance is
time-driven; both cold checks recompute closure over the entire same graph.
These concurrent opt-level-1 runs are correctness evidence, not benchmarks.

Independent source reviews covered preparation cancellation, helper
repartition on restore, typed record authority, publication failures, and
immutable snapshots. Optimized performance qualification remains open. Current receipts,
commands, and outstanding decisions are maintained in [CODEX_PROGRESS.md](../CODEX_PROGRESS.md).

## The merge boundary

Inspectors consume immutable inputs and return proposals. They do not assign
canonical successor IDs or mutate the owner ledger. The coordinator publishes
one selected cut of results through four stages:

| Stage | Work | Authority boundary |
| --- | --- | --- |
| P1: validate | Decode results; check source identities, dispatched geometry, result class, guards, frontiers, and anchor claims. | Invalid results cannot become a merge plan. |
| P2: prepare | Resolve obligations; deduplicate candidates; construct the cut's containment antichain; choose representatives; find retirements and dependency-transfer targets. | Reads canonical state only; a failure discards the unpublished plan. |
| P3: publish | Preflight reservations, then install prepared IDs, ledger transitions, typed records, anchors, and dependency edges in canonical order. | The only mutable publication boundary; it makes no new containment choice. |
| P4: refresh lookup | Build and publish a complete immutable lookup root for later inspectors. | A new snapshot becomes visible only after it is complete. |

`epoch::merge::p2_plan` remains a scalar differential reference. The production
preparation implementation is `epoch::merge::preparation::Engine`. Parallel
tasks do not retain store borrows or snapshot leases across P3.

### P2 parallel work and deterministic folding

Preparation separates independent work from canonical mutation:

1. Source blocks resolve shipped obligations against one immutable state.
2. Results fold in source order; exact duplicate elimination is canonical.
3. Independent `(phase, owner)` buckets prepare containment antichains.
4. Representative and reverse-retirement work runs on the resulting candidates.
5. Transfer targets and verification tokens fold into one immutable merge plan.

Tasks use one private, invocation-local Rayon pool. They do not use the global
pool, clone a CAS context per preparation task, or perform new computer algebra.
All owned tasks join before preparation returns. The failure-selection order is
explicit: source order, sorted bucket/member order, then survivor order. This
avoids making an error depend on hash-table iteration or which helper finishes
first.

Successful serial and parallel preparation must agree on the canonical plan,
IDs, dependencies, ledger transitions, and deterministic lookup accounting.
Parallelism is not permission to omit an obligation or accept a partial
retirement list.

### Typed records and bulk dependencies

P3 emits typed native records instead of building a generic JSON value for
every publication. Record frames are length-delimited binary data, buffered in
bounded write batches. The sidecar hashes the same bytes it writes and seals
immutable segments at a checkpoint boundary. Diagnostic payloads already
produced by inspectors may still contain opaque JSON bytes; binary framing does
not imply that every diagnostic is a newly defined binary schema.

Cold restoration reads typed authority directly. It validates the record's
geometry, result class, counters, anchor scope, and ledger relationship without
parsing diagnostic JSON. JSON projection remains available at explicit
diagnostic/export boundaries. It is not the native authority format.

Dependency runs are preflighted and appended in bulk to the closure tracker.
This reduces repeated reservation and per-edge mutation overhead while keeping
the same dependency graph and sealed/inspected semantics. It does not replace
recursive closure with a count of locally completed inspections.

## Worker allocation

The total `workers` value includes the coordinator and preparation helpers.
For `workers > 1`, the resolved partition is:

```text
workers = 1 coordinator + inspection workers + preparation helpers
```

The new controls are available in the Rust request, CLI, and Python steering:

| Rust field / CLI flag | Meaning |
| --- | --- |
| `epoch_preparation_workers` / `--epoch-preparation-workers` | Reserved helpers inside the total budget; zero selects serial P2. |
| `epoch_preparation_max_obligations` / `--epoch-preparation-max-obligations` | Maximum retained obligation/candidate slots for one cut. |
| `epoch_preparation_max_retirements` / `--epoch-preparation-max-retirements` | Maximum retained retirement IDs for one cut. |

Omitting both an explicit inspector partition and helper count preserves the
Epoch default of **zero helpers**. An explicit inspector partition retains its
helper complement; specifying both values requires them to agree. For example,
16 workers and 2 helpers resolve to 13 inspectors, 2 helpers, and 1 coordinator.
One-worker operation remains inline with zero helpers and no separate
coordinator thread. Finite containment-comparison caps require zero helpers.

Scratch allowances default to `u32::MAX`; they count logical items, not bytes,
numerator rank, or cumulative campaign work. Exceeding an allowance stops before
publication rather than truncating coverage. They do not replace the campaign's
RAM guard.

Helper allocation trades inspector capacity for preparation capacity; it is
not automatically a speedup. For a controlled rolling comparison, freeze the
cut and window explicitly: changing the inspector count can otherwise also
change the automatically selected fresh window.

## Immutable lookup roots

P4 replaces the earlier full lookup-replica scheme with persistent geometry,
summary, live-bit, and quarantine pages plus immutable index cohorts. Cloning a
root shares its unchanged data. Appends and retirements copy only affected
paths; geometric cohort compaction keeps the number of index layers bounded
logarithmically.

Each inspector pins one exact root and version. Older readers can continue while
the coordinator publishes newer canonical state. Every positive lookup is
checked by the shared exact containment verifier. Cohort traversal preserves
the canonical lookup precedence: the oldest admissible exact match (including
retired IDs), then the dominant orthant with its existing rank/latest-ID tie
policy, then the minimum currently live general container. Digest collisions
are checked against actual geometry. Quarantining the dominant orthant must not
resurrect a retired shadow as a fallback.

The snapshot manager limits retained roots, version lag, and accumulated logical
refresh charge. Reaching those limits waits for readers to drain; it does not
discard obligations or force unsafe publication. The charge is an admission
heuristic, **not a physical RSS bound**. Rust `Arc` allocation also retains normal
global out-of-memory behavior; this design does not promise to recover from
every allocation failure.

Snapshots are process-local acceleration structures, not checkpoint authority.
Cold restore rebuilds them from canonical geometry and live state. A failed or
interrupted refresh cannot expose half-applied retirements to old readers.

## Checkpoints, interruption, and failures

The native generation uses these distinct versions:

| Layer | Version |
| --- | --- |
| Walk semantics | 4 |
| CP6 manifest / checkpoint summary | 3 |
| Native scalar metadata | 4 |
| Typed record schema | 1 (`ERB1` frames) |
| Diagnostic export schema | 2 |

Do not use this build to resume an earlier semantics-3 Epoch checkpoint. A fresh
campaign is the intended deployment path; no historical-format compatibility
project is required. Existing typed checkpoints bind mathematical requests,
logical preparation allowances, scheduling choices, and canonical authority.

The native restore API permits changing a valid execution-only helper partition,
including a helper-enabled checkpoint resumed with one inline worker. Saved
receipts remain intact and new execution uses the newly resolved partition.
The production Python wrapper deliberately freezes its steering policy more
strictly: native support for repartition does not imply that arbitrary wrapper
resume overrides are accepted. Use a fresh campaign unless that wrapper path
has separately been validated.

During helper-enabled P2, preparation runs on an already reserved helper while
the coordinator polls progress and operational stops. Serial P2 instead threads
a coordinator-local polling callback through its scans. There is no extra
unreserved observer thread. The nominal polling interval is 50 ms; clock reads
are amortized, and an individual sort or other indivisible operation may delay a
poll. This is not a hard 50 ms interruption-latency guarantee.

Cooperative interruption before P3 discards the unpublished cut, joins all owned
tasks, and saves replayable state through the existing controller. Periodic and
memory-triggered checkpointing use durable merge boundaries. P3 preflight
reserves known mutation capacity; an unexpected fatal publication, write, or
seal failure fails closed and must not advertise a new resumable generation.
Partial disk tails and unsealed frames cannot become accepted checkpoint
authority. A poisoned invocation cannot be resumed merely because it left
checkpoint-looking files; recovery must respect the native reader's refusal
and the last independently valid durable boundary.

## Measurements and acceptance

Preparation reports sequential wall phases for source resolution, canonical
deduplication, antichain construction, representative selection, reverse
retirement lookup, and transfer work. It also reports source tasks/waves,
obligations, candidates, bucket size, survivors, and retirements. Summed worker
times or configured reservations are not whole-campaign speedups or observed
CPU utilization.

Physical cohort lookup candidate/test counts may differ from a monolithic
lookup index even when the selected mathematical target is identical. Compare
logical state separately: geometry, winning IDs, guards, anchors, ledger,
dependency edges, required-query scope, frontiers, and independently rechecked
closure. Document counter exclusions narrowly; never discard a mathematical
mismatch as telemetry. Serial/parallel P2 over the same canonical store still
requires exact deterministic accounting.

The outstanding performance gate uses frozen fully optimized executables,
identical owner/query inputs, worker budgets and CPU placement, and independent
cold reinspection. First compare old/new lockstep with the same cut and no
helpers, then compare helper allocations, then test rolling scheduling. Compare
the selected Epoch configuration against a contemporaneous Ready baseline;
checkpoint-only finalization and Ready's full result generation are different
workloads, so report native and cold-validation costs separately and together.

Known risks still to measure include large single antichain buckets, serial
canonical folding and P3 mutation, cohort-compaction carries, exact-key binary
search costs, retained-root pressure, and the loss of inspectors to helper
reservations. Wider rolling windows or out-of-order publication can increase
total domain work. Their ability to keep more cores busy is not sufficient
evidence of a better campaign. Epoch deployment retains the agreed 1.5× matched
throughput gate, with all required mathematical checks passing.
