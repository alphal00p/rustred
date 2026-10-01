# Epoch S5: parallel preparation and immutable publication

This document describes the implemented S5 path, not a claim of five-loop
closure. The frozen launch recipe and its measured trade-offs are in the
[generation-first runbook](five_loop_optimized_generation_runbook.md). The mathematical
scope remains the explicitly declared starting queries and their reachable
obligations. A drained work queue alone is not a closure certificate.

## Validation status

As of 2026-09-30, the previously completed core native suite reports 2,864
passed and 32 ignored. Following the production union-preflight repair, the
repair application suite reports **1,267 passed, zero failed and 12 ignored**;
candidate and routed CLI suites add 19 and 6 passing tests. The earlier
1,229-test milestone supplied 50 distinct physical cores and exercised all
worker-availability subcases. These are separate receipts, not a claim that
every later run repeated that allocation. Full receipts, fixture corrections
and failed attempts remain in the progress log.

The subsequent P1 local-union witness and optional P2 observation milestone
passes1,285 application tests plus19 candidate and6 routed CLI tests, with
zero failures and the same12 ignored research tests. That run supplies50
distinct physical CPUs and emits no worker-availability skip markers. Focused
union9/observer8/layout1 checks also pass; these are subsets of the full suite.
The private one-use witness avoids evaluating the same immutable G2 union twice
inside P1; inspector preflight and independent cold/replay checking remain
unchanged. The bounded `RUSTRED_EPOCH_PROFILE=1` census observes exact image
repetition without supplying proof or changing publication decisions. These
changes are correctness-tested in an application-opt-level1 build; their
optimized runtime effect has not yet been measured. The user's frozen repaired
production executable predates them and is not changed by this development work.

The compact source-row transport then passes1,291 application and25 CLI tests
(zero failures, the same12 ignored), including six new layout/iterator tests.
The actual token tag is16 bytes: at arity15,256 all-Stored rows reserve4,096
bytes instead of348,160; the all-Candidate case reserves352,256. These are vector
backing capacities, not RSS or a runtime benchmark. This later test also uses
50 physical cores and application opt-level1; fully optimized measurement is
pending, and the production binary remains unchanged.

The first full-input trial exposed an optional G2 union-proof budget mismatch:
the discovery procedure could succeed while P1's separate bounded proof was
inconclusive. The repair conservatively inspects the whole domain in that case;
publication and cold checking remain strict. All 58 four-loop queries pass
independent full reinspection. A separate bounded full-input five-loop pilot
exercised four such fallbacks, saved cleanly, and passed structural cold checks
with zero violations. That latter check deliberately re-inspected no native
records and does not prove all-query closure. The optimized replacement build
has succeeded and the user's fresh walk is running. Additional optimized checks
were deferred until after launch by explicit user direction. Those four-loop
ABBA controls are now complete: every arm passes independent cold-All for all
58 queries and 32 roots. On the repaired CLI (`23d836d7…`), Epoch is 9.63% and
9.89% slower in the two native+cold pairs, despite lower CPU/domain counts.
This remains a correctness pass, not four-loop wall-time non-regression.
The old failed checkpoint must not be resumed.

The subsequent optimized full-input W32 h0/h2 causal screen compares31
inspectors with29 inspectors plus2 preparation helpers. Both twenty-minute
native windows save and pass structural cold checks with zero violations;
neither closes the requested scope or repeats native inspections. Helpers cut
P2 time312.469→261.632s but complete only3.13% more inspections, with4.64% more
pending domains and5.27% more sampled supervised-tree CPU. That misses the
preregistered useful-work threshold and does not justify changing the running
default. The graph prefixes differ, so this is not fixed-work speedup evidence.
See [the complete screen and measurement limits](research/epoch_preparation_screen_2026-09-30.md).

The earlier fully optimized CLI (`38e0e763…`) passed correctness in the repeated
four-loop Ready/Epoch comparison with 1,024 extra completed-result slots.
Median native-launch-through-drain plus cold-All time is 15.494 s for Ready
and 16.293 s for Epoch: **Epoch is 5.16% slower**, not a demonstrated wall-time
winner. It uses 34.14% less native CPU and retains 33.04% fewer domains, while
performing 17.03% more native inspections. All four arms independently verify
every required query. The lower CPU/domain cost motivates a full-scale Epoch
trial, not a claim that it is universally preferable to Ready.

The final optimized finite-five-loop single pair also cold-verifies: Ready
319.826 s versus Epoch 311.466 s native+cold. Epoch's native command is slower
(174.275 versus 168.634 s); faster cold graph checking supplies the total saving.
Native CPU falls 545.133→482.216 s, while sampled RSS rises 6.233→6.582 GB.
This control covers one bounded 1,324-point query and its reachable descendants,
not the production 116-query scope. The user-selected launch reserves 32 physical
cores/600 GB; those final comparisons used16 cores and did not measure32-core
performance. The newer h0/h2 screen above supplies limited32-core evidence,
not a whole-campaign completion time. The user trial generated and admitted all67
owners (9,966 rules and 939 finite residual cases), then hit the reuse-check
failure described above. Saved rules can be reused for recovery; full-scope
completion remains unknown. See the runbook for the running repaired release
rather than treating these earlier performance measurements as a recovery instruction.

Focused preparation, binary framing/sidecar, immutable snapshot, bulk dependency,
record restoration, helper-repartition, and real native replay checks ran. The
first 16-CPU reservation emitted W50 skip markers. The later full suite used 50
distinct physical cores and exercised all ten intended W50 subcases successfully,
with no skip markers. The first optimized matched four-loop comparisons are
complete: runtime is effectively neutral, while the checkpoint is about 61%
smaller. The first finite-five-loop fixed-work pair also passes with identical
graphs and neutral runtime, with a56% smaller checkpoint. Before bounded extra
lookahead, the earlier optimized
Ready/Epoch rolling controls cold-verify successfully but favor Ready: the
four-loop ABBA median is8.04% slower for Epoch and the finite-five first pair
13.43% slower. Rolling
overlap reduces Epoch's own inspector waiting substantially without making it
faster than Ready. These historical controls are retained separately from the
final release comparison;
see [the measured controls](research/final_order_s5_pilots_2026-09-30.md).
The application test build uses optimization
level 1 for correctness checks; it must not supply production performance numbers.

The correction review also found stale nested CP6 metadata in the public CLI
identity probe. Its format and schema now come from the native checkpoint
writer, and both advertised Epoch semantics fields use the same native
constant. The final rebuilt CLI probe, help, candidate/routed integration tests,
Rust application API and ordering-inspector tests all pass. A fresh installed
Python wheel at the preceding milestone passes all 19 candidate API tests without skips, including cold
subprocess loading and CLI parity; the focused four-test order gate also passes.
These interface checks do not establish a production performance gain. The
existing selected-profile installed wheel has subsequently passed all 23
compatibility checks against the new CLI; it was not rebuilt and does not
expose a new PyO3 owner-walk method. The real public Python pipeline also passed
fresh combined-four-loop generation, admission, Epoch traversal and cold-All:
16 new owners, 523 rules, 58 required queries, and all 17,957 native inspections
verified. This is actual execution evidence, separate from mock steering tests.

The new bounded-lookahead screen compares Epoch against itself, not against
Ready. At 16 workers, an extra inventory of 1,024 complete results reduced
finite-five traversal from 182.661 to 123.745 seconds and native run plus cold
verification from 430.500 to 371.751 seconds, with 0.74% more inspections and
nearly unchanged CPU time. Both arms independently cold-verify. This is one
pair using the same application-opt-level-1 executable. It is evidence for the
mechanism, not the final fully optimized engine choice. The four-loop screen
with 32 extra slots was traversal-neutral and cold-verified. The repeated
optimized four-loop Ready/Epoch result above supersedes the earlier pending
qualification, without changing the provenance of this diagnostic ablation.

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
immutable snapshots. Limited optimized controls do not establish full-production
speed or saturation. Current receipts,
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

Source-block transport keeps verified existing-domain tokens in a compact
tag vector and new-candidate query payloads in a separate, source-ordered
vector. Both capacities are reserved before resolving the first row. An input
with a supplied target still goes through the unchanged full resolver; target
presence bounds allocation only and is not proof of containment. The owning
iterator preserves tag/payload pairing when advancing or skipping, and a missing
payload cannot silently truncate the output. Candidate-heavy blocks can cost
more than the old inline-enum layout, so this change requires matched runtime
measurement rather than a speed claim from type sizes. Native validation and
optimized comparison status are tracked in the progress log; the running frozen
production build does not contain this change.

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

## Bounded complete-result lookahead

Rolling oldest-prefix execution can have idle inspectors while a slow result
blocks publication of its prefix. Reusing physical worker slots alone does not
solve this: the logical reservation window can already be full. Optional extra
inventory allows inspection to proceed further without publishing out of order.

| Rust field / CLI flag | Meaning |
| --- | --- |
| `epoch_result_escrow_jobs` / `--epoch-result-escrow-jobs` | Extra logical reservations beyond the base window; defaults to zero. |
| `epoch_result_escrow_bytes` / `--epoch-result-escrow-bytes` | Admission threshold for retained complete-result buffer capacity; mandatory with positive extra inventory. |

These controls are also forwarded by the Python campaign supervisor to the
CLI. There is no direct owner-walk PyO3 method. Positive inventory requires
rolling checkpoint Epoch with oldest-prefix publication. Bytes without extra
jobs are rejected. The base window plus extra inventory must remain within the
existing 4,096-job operational limit.

Extra work is dispatched only when a prefix is missing, work remains unreserved,
an inspector is idle, no work is already queued, and both inventory and byte
admission checks allow it. Results remain indivisible: P1/P2/P3 still process
whole jobs in the original ordered prefix. The cut remains bounded by the base
window, not enlarged by extra inventory. One-worker execution dispatches no
extra work; zero inventory preserves the prior path.

Logical reservations outlive reusable physical slots. Keys and generations bind
callbacks to their reservations, while immutable snapshot leases are released
when inspection returns. Capacity accounting follows result buffers, including
buffers waiting in the result channel, until P1 takes ownership. This byte limit
is **not a hard RSS ceiling**: already-running jobs may return after admission
stops and overshoot it. The campaign-wide RAM guard is still necessary.

Checkpoint, cancellation and error handling retain unresolved obligations for
replay or recomputation; discarded speculative results cannot discard required
work. A stopped/error report may miss late buffer peaks during worker joining,
so those observed peaks are lower bounds. Completed, fully drained runs record
the final accounting. More lookahead is beneficial only if saved waiting exceeds
the cost of additional domain work and coordinator processing.

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
| Native scalar metadata | 5 |
| Typed record schema | 1 (`ERB1` frames) |
| Diagnostic export schema | 2 |

Do not use this build to resume an earlier semantics-3 Epoch checkpoint or a
scalar-metadata-4 checkpoint. Scalar version 5 explicitly binds the base window,
extra inventory and nullable byte threshold; the native reader rejects version
4 rather than guessing defaults. A fresh campaign is the intended deployment
path; no historical-format compatibility project is required. Existing typed
checkpoints bind mathematical requests, logical preparation allowances,
scheduling choices, and canonical authority.

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

The subsequent optimized P1-witness/compact-source package passes the repeated
four-loop cold-All controls but is performance-neutral: old/new primary medians
15.792/15.598 seconds, with pair changes of −2.40% and +0.08%. This comparison
is Epoch versus Epoch and does not satisfy the outstanding Ready comparison
by substitution. See [the measured result and limits](research/epoch_p1_compact_2026-10-01.md).

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

The performance gate uses frozen fully optimized executables, identical
owner/query inputs, worker budgets and CPU placement, and independent cold
reinspection. The final selected rolling/lookahead four-loop comparison and
single finite-five-loop pair are recorded above. Strict four-loop wall-time
non-regression has not passed; repeated five-loop and current hot-control
qualification remain incomplete. The frozen release is an explicitly qualified
user-run trial, not a declaration that these performance gates passed.
Checkpoint-only finalization and Ready's full result generation are different
workloads, so report native and cold-validation costs separately and together.

Known risks still to measure include large single antichain buckets, serial
canonical folding and P3 mutation, cohort-compaction carries, exact-key binary
search costs, retained-root pressure, and the loss of inspectors to helper
reservations. Wider rolling windows or out-of-order publication can increase
total domain work. Their ability to keep more cores busy is not sufficient
evidence of a better campaign. The user relaxed the former mandatory 1.5×
throughput threshold on 2026-09-30. Deployment still requires a reproducible
useful benefit, four-loop non-regression, honest work/memory accounting, and all
required mathematical checks. An Epoch-versus-Epoch improvement alone cannot
establish a preference over Ready, nor prove useful 20-core or 200-core scaling.

### Live lookup profile: October 1

A single 60.79-second, 19 Hz user-cycle capture of the unchanged repaired
production executable provides a qualitative lead, not a timing comparison.
Among 364 coordinator samples with visible P2-source ancestry, 218 include
`Block::forward`, four include digest/BLAKE3, and six include native-summary
construction. Worker stacks also prominently show forward-index traversal,
snapshot lookup and envelope tests. These inclusive counts overlap; they are
not a disjoint wall-time breakdown. Twelve further coordinator hash samples
have unknown callers and are not assigned to P2.

Independent review caught an offline rendering omission: decoding each thread
separately recovered worker stacks from the same capture. All 5,372 samples and
their recorded period sums reconcile. Zero reported lost samples does not prove
unbiased coverage; throttle records likewise do not count known missed samples.
Unresolved ancestry, optimized/inlined frames, stack-depth limits, unmeasured
sampling perturbation and concurrent compilation remain limitations.

The 337 coordinator samples labelled `Tracker::scan` initially lacked phase
markers. Subsequent caller-address and frozen-disassembly inspection places
all of them in post-commit periodic closure monitoring, with `force=false`.
They cluster in one 17.8166-second interval, consistent with the reported
17.833-second refresh. At the later 01:15 UTC status check, cumulative refresh
wall time was 98.417 seconds out of 9,052.275 seconds elapsed, about 1.09%.
This is not queue lookup or evidence of a sustained 30% monitoring cost; the
short profile happened to include an infrequent refresh.

Frozen-binary disassembly confirms two digest calls on the successful source-row
path, but their existence does not establish a worthwhile optimization. Current
evidence prioritizes inspecting lookup/index costs while the already-tested
P1/compact-transport package undergoes optimized matched comparisons. It does
not justify bypassing containment, quarantine or summary validation. Raw local
evidence and the separate interpretation audit are retained under
`TMP/postlaunch-20260930/digest-perf-tooling/`; no second capture was needed.
