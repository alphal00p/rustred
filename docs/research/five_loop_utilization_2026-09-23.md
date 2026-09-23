# Five-loop full-owner utilization investigation — September 23

## Scope and reproducibility

The resumed investigation uses all 67 prepared starting regions together,
with `A<=24`, `R<=15`, and `A-R>=9`, keeping the saved owner programs and
8,246 admitted routes unchanged. These bounds describe the documented
conditional full-jet envelope, not a catalogue of QCD diagrams. Its exact
planning count is 3,258,551,484,224 labelled starting keys; the run processes
symbolic regions, not that many individual integrals.

The release executable is the previously gated combined-tree binary:
`TMP/parallel-admission-gate.53B924/rustred`, SHA-256
`6e3791ad6a4aee482b5486ddf8946bee101da4fd2992c91364b550455c8a713a`.
It includes the same pre-existing uncommitted scheduler/escrow changes as the
September 22 pilot. No solver implementation or saved IBP was changed for
these runs. The query SHA-256 is
`c80d332a20bf9ed5b957106d96c3e8db1c255ac873d51dbc6f18827a3a94e570`;
the manifest SHA-256 is
`d2de73414a0cc5652d9124a32dd978759daf75abc305b705b53d759a3aad7f7f`.

The Python supervisor pins the native process to CPUs 0–49, sets inner pools
to one, and monitors its process tree. The 50-slot compute budget reserves
25 inspectors, 24 admission helpers and one coordinator. The memory policy
is 450 GB cooperative / 500 GB sampled hard RSS with a 480 GB child address-
space bound; sampled RSS is not an exact high-water measurement. A separate
read-only thread sampler runs on CPU 50. The 15-hour objective is telemetry,
not a timeout. Other host workloads remain active, so these are observational
measurements, not controlled scaling experiments.

All receipts are under `TMP/full-jet-symbolic67.CwBp4L/` and are not shipped.
The input is broader than the earlier 980-point diagonal: comparing their
elapsed times would not measure equal work.

## Completed diagnostic attempts

| Measurement | Original per-query allowances | Raised per-query allowances |
| --- | ---: | ---: |
| Receipt suffix | `yiunlg31` | `v88q1vsx` |
| Preparation | 104.727 s | 103.683 s |
| Traversal including failure/drain | 6.107 s | 61.646 s |
| Native total | 110.834 s | 165.330 s |
| Supervisor lifetime | 114.047 s | 170.097 s |
| Sampled peak process-tree RSS | 6.046 GB | 6.591 GB |
| Completed regions | 0 | 7 |
| Partial failed region | 1 | 1 |
| Scheduled / pending | 13,753 / 13,752 | 500,000 / 499,992 |
| Stop reason | 100,000 predicates per query | 500,000 scheduled regions |
| Observed frontiers | 0 | 0 |

Both attempts exited with status 4, explicitly incomplete. Neither exhausted
memory, hit an elapsed-time deadline, or proved the family closed. Pending
obligations cannot be dismissed because no frontier has yet been reported.

The first attempt's default predicate allowance was too small for a broad
root. Related allowances were raised together through existing public CLI
options: ten million rules/predicates per query, 100 million geometry cells,
3.2 billion coordinate entries, ten million RHS boundary cells, and 400 million
native operations. Exact geometry, ordering, rank and source conditions did
not change. These are work budgets, not mathematical restrictions.

With those allowances, root 0 finishes its local inspection: 342,309
predicates, 9,157 selected rule pieces and 202,895 successors in 10.348 s
worker wall time. Only three bounded-refinement steps (39 faces) occur, with
no reported problem. The original stop therefore did not identify an
exceptional unsolved integral. By root 6, a single local inspection emits
649,259 successors. Worker wall times include waiting for output publication
and are not standalone serial solver timings.

## What limits utilization now

The initial native-heavy phase briefly reaches about 24.8 busy cores: it can
fill the 25 inspection slots, but not the 24 helper slots reserved for later
admission work. Once output buffers fill, 24 inspectors repeatedly wait behind
the currently published inspection. Completed-result escrow cannot unblock a
producer that is still streaming a large result: it stores only finished jobs.

For the raised-budget run, 25 two-second sample intervals aligned with
heartbeat labels 115–164 s cover 50.003 s. Mean measured utilization is
**2.696 cores**: 1.060 native inspection, 1.302 admission helpers and 0.330
coordinator, with a small non-atomic sampling difference. This is not 50-core
saturation. Preparation takes 3.224 coordinator-wall seconds and ordered
commit 17.605 s out of the 61.646 s traversal. Those timers exclude native
waits, dispatch, observation and drain; they are not CPU-time partitions.

The raised-budget run admits 1,833,022 successor events and reuses 1,333,088
scheduling requests. It still reaches half a million distinct obligations
before the eighth initial root finishes. There are 326,261 live containment
candidates and 173,739 retired lookup candidates; retirement does not delete
the corresponding pending obligations. No Route-phase region has committed
yet. Rank-16 descendants are retained even though the starting rank is 15,
as required: entry bounds never clip intermediate work.

The earlier observation that 99.699% of *previously seen* Apply regions fit
the new broader roots was useful motivation, not a prediction for this much
larger input. The output retains processed and attempted inspections, not the
entire pending queue. In particular, these receipts do not establish the
phase/mask distribution of the 499,992 pending regions.

## Larger diagnostic: cooperatively stopped, not a completed workload

Receipt `shared-owner-campaign.lgvisscc` used the same input, ordering,
executable and expanded per-query policy, with aggregate allowances increased
to ten million domains and 200 million events. All 67 initial regions have
finished local inspection and the worklist progressed to descendants.
This is neither a new artifact nor closure of those regions under recursion.

The operator subsequently stopped it for a measured optimization checkpoint,
not because of a deadline, memory exhaustion or mathematical failure. Native
and supervisor processes have exited; the durable result reports:

| Measurement | Final incomplete result |
| --- | ---: |
| Preparation | 104.028 s |
| Traversal including cancellation/drain | 1,865.666 s |
| Native logical total | 1,969.694 s |
| Supervisor lifetime | 2,029.718 s |
| Completed regions | 2,522,397 |
| Cancelled partial / pending | 1 / 6,029,467 |
| Scheduled regions | 8,551,865 |
| Committed logical events | 146,807,714 |
| General containment comparisons | 126,279,058,490 |
| Reverse maintenance comparisons, included above | 9,755,315,281 |
| Sampled peak process-tree RSS | 50.119 GB |
| Observed missing-rule frontiers | 0 |

The final population balances exactly: scheduled = completed + partial +
pending. The partial is cancellation fallout, not a discovered missing rule.
The roughly 60 s between native logical reporting and supervisor exit includes
output serialization, teardown and supervision; it is not a separately timed
serialization benchmark. RSS rose during final output from roughly 36 GB.
The final observed native CPU counter was 17,628.86 s, not fifty cores times
wall time. No hard kill or resource-ceiling failure occurred.

The measured completion rate fell from approximately 3,036 regions/s in an
earlier window to 60/s in the late 1,799.51–1,926.15 s window, while admissions
still exceeded completions. Even the artificial no-new-work drain estimate
for roughly six million pending regions at that late rate exceeds 27 hours.
This is a reason to investigate the bottleneck, **not** a reliable completion
forecast: region costs are heterogeneous and future descendants remain unknown.

All fields except top-level per-record `seconds` match for the first seven
completed regions shared with `v88q1vsx`. Independent streaming extraction also
retains the first 60,000 complete records for a subsequent same-prefix test.
Audit scripts, hashes and compact evidence are in
`TMP/five-loop-utilization-readonly.vV9E5N/`. The result does not persist a
durable pending-work resume or all rejected admission proposals.

The earlier live observations below describe intermediate phases of this
now-terminal diagnostic, not its final state.

At the 778.47 s heartbeat, 1,287,561 regions are complete, 5,591,880 scheduled
and 4,304,319 pending, with zero observed frontiers and approximately 22.8 GB
native-process RSS. These are **live** counters, not a completed workload or
maximum memory measurement. Rank-17 descendants are retained. No input or
saved rule was changed between attempts.

Weighted post-root utilization is increasing, but remains below 50 cores:

| Heartbeat window (seconds after launch) | Native inspection cores | Lookup helper cores | Coordinator cores | Process cores |
| --- | ---: | ---: | ---: | ---: |
| 421–480 | 3.74 | 5.22 | 0.62 | 9.59 |
| 540–600 | 3.86 | 3.34 | 0.76 | 7.97 |
| 601–660 | 5.94 | 5.67 | 0.70 | 12.32 |
| 662–718 | 6.15 | 5.84 | 0.69 | 12.69 |
| 720–777 | 7.31 | 6.39 | 0.67 | 14.37 |

Each entry is summed sampled CPU time divided by summed interval duration,
not a count of reserved or apparently active threads. Small discrepancies
between the component sum and process total reflect non-atomic snapshots.
Pending work grew in every complete minute window in this early selection.
Memory headroom alone did not predict completion. The ten-million-domain
allowance was not a completion denominator.

The separate two-loop test target was compiled/tested on CPUs 74–77 during
part of preparation and early traversal, and an isolated geometry prototype
used CPUs 74–75 later. These were outside the native process's 0–49 affinity;
their CPU is not included in the process-tree solver timings. Neither these
measurements nor the shared host should be described as an exclusive-host
50-core benchmark.

## Implementation implications

The present implementation can split an input region into disjoint coordinate
boxes while retaining its A/R/D predicates. That can shorten indivisible
jobs, but removes the broad parent from the single-region containment index:
a child crossing a partition boundary fits the union, not necessarily one
shard. Adding both parent and shards simply deduplicates the shards under the
parent. A principled scheduler improvement would retain the broad parent's
reuse obligation while dispatching exact partitions and require **every**
partition to finish before resolving the parent. An isolated native-geometry
prototype passes six tests covering 111,132 configurations and 4,000,752
integer-membership checks (`TMP/parent-partition-design.xkm6A5/`). This validates
the partition geometry, not a production scheduler or a performance gain.
Since initial roots already finish, partitioning cannot be assumed to cure
the later admission/Route fanout and could create more fragments.

A smaller experiment tested exact-summary hashing before general containment
searches. It preserved domain geometry and queue obligations and passed its
correctness gates, but the matched full-owner prefix was slower. The experiment
has therefore been removed from production; the evidence below records why.

## Exact-summary equality lookup: measured and rejected

The experimental unlimited comparison lane used an optional phase/owner-local
digest lookup over live candidate summaries. A hit had to pass full native
`DomainPowerSummary` equality and current live-index membership. Since live
candidates are an inclusion antichain, this identifies the same unique
containing candidate as the original scan, not an arbitrary alternative ID.
Digest collisions, stale entries and optional allocation failure fell back to
the normal search. The cache stored only digest/ID entries, not duplicated
geometry. Retirement dropped lookup entries, never pending obligations or old
exact keys. Finite-cap mode and near-counter-overflow behavior were unchanged.

The experimental `containment_summary_equality_hits` counter measured successful
committed shortcut reuse; speculative hits did not increment it. This was not
a new admission count or a mathematical coverage claim. Independent
implementation/mathematical review passed. The actual-source optimized queue
gate passed 56 tests, with one existing replay diagnostic ignored, including
twelve new tests and the existing full-proposal-stream
comparisons against an independent linear semantic index and prepared workers.
The candidate's full application gates also passed: 400 application-library
tests plus 82 integration tests (482 total), and 72 Python tests. These gates
establish tested correctness, not a performance benefit.

A synthetic complete-proposal benchmark compared the prepatch queue and new
queue against the same unchanged native geometry. Six alternating-order pairs,
batched to avoid sub-millisecond CPU-counter noise, ran on CPU 70, outside the
application build's 74–77 affinity. All returned IDs and admission decisions
match. These are shared-host observations, not campaign speedups:

| Synthetic stream | Equality hits | Median wall before / after | Paired speedup |
| --- | ---: | ---: | ---: |
| Large mixed set, 12,928 proposals | 7,680 | 40.216 / 23.817 ms | 1.688x |
| Same size, strict subsets instead of equal descriptions | 0 | 40.167 / 42.416 ms | 0.947x |
| Small mixed set, 808 proposals | 480 | 0.717 / 0.877 ms | 0.817x |

The negative controls matter: hashing adds cost for small sets or rare equality.
The production decision therefore used the instrumented full-owner prefix
test, not just the favorable synthetic case. The synthetic timing includes
fresh queue admission, input cloning, destruction and intra-batch identity checks; it
excludes input construction and cross-version postchecks. These timing streams
do not exercise retirement, which is covered by separate correctness tests.
Evidence: `TMP/equality-queue-audit.Hv3jrC/RESULTS.md` and
`compare-isolated.log`. Integration evidence is under
`TMP/summary-equality-gate.eIgC5f/`.

The real comparison used the identical 67-owner input, ordering, work budgets
and CPU 0–49 affinity. Both binaries were actual release CLI builds with
opt-level 3 and LTO off. Compilation was outside the timing boundary. The
first 60,000 completed records match in **every field except the top-level
per-inspection `seconds`**; their complete non-timing payload hash is
`c4f051665c8910ccd86bcb3827d043b1d01b814bff4953f54a20dab21323246a`.

| Matched first-60,000-record boundary | Baseline | Equality experiment |
| --- | ---: | ---: |
| Traversal-time bracket, preparation excluded | 251.3576–252.3632 s | 275.7406–276.7462 s |
| Sampled CPU envelope near boundary, preparation included | 1,007.51–1,050.48 CPU-s | 1,090.90–1,123.33 CPU-s |
| Peak sampled process-tree RSS through the envelope | 9.7656 GB | 9.7557 GB |

The candidate was **9.3–10.1% slower** over this exact common prefix. The
brackets include heartbeat age; CPU/RSS samples bracket nearby observations
rather than an exactly synchronized record boundary. This was one observational
shared-host pair, not a repeated controlled scaling study. The candidate was
cooperatively stopped after the comparison point; neither run completed the
five-loop workload.

Near that boundary, only about **0.39% of summary constructions** produced a
committed equality shortcut (31,649–31,661 hits for 8.048–8.109 million summary
builds). This counts successful cache shortcuts, not a full equality census:
collisions and optional fallback can send equal summaries through the ordinary
index. Its complement therefore cannot be called a measured strict-inclusion
fraction. The ordinary semantic-hit counter is also a different population.

Decision: remove the production experiment and its extra telemetry rather than
retain a measured regression. Its source/tests are preserved only under the
ignored `TMP/equality-queue-audit.Hv3jrC/rejected-experiment/` directory; the
standalone comparison in the parent evidence directory now uses that archive.
The complete
matched audit is
`TMP/five-loop-utilization-readonly.vV9E5N/MATCHED_SUMMARY_EQUALITY.md`.
These negative results do not prove hashing is universally unhelpful, but they
do not justify enabling this implementation on the target workload.

## Pending-obligation transfer: proposal, not implemented

Lookup retirement currently leaves every admitted inspection obligation alive.
The final baseline had 5,427,770 retired lookup candidates but only 2,522,398
processed IDs, including its partial cancellation. Thus **at least 2,905,372**
retired IDs were still among the 6,029,467 pending obligations (48.19%). This
is a counting lower bound, not measured avoidable CPU time or proof that those
IDs were undispatched.

A prospective opt-in scheduler policy could transfer an untouched obligation
`A` to a later containing obligation `B`, for the same owner, phase and frozen
rule snapshot, using existing exact native containment. It must retain `A`'s
raw identity/provenance, explicitly require `B` to finish, and distinguish
delegation from actual inspection. Forward-ID aliases are acyclic, but that
does not prove descending IBP reductions or rule closure. Started work, failures
and partial output cannot simply be discarded.

Worker-count-independent decisions require a fixed logical dispatch fence and
sticky reservations/aliases, not a timing-dependent test for whichever job is
currently undispatched. New report semantics would be necessary: skipping an
inspection intentionally cannot preserve the previous complete event prefix.
The design and required tests are in the ignored
`TMP/parent-partition-design.xkm6A5/RETIREMENT_TRANSFER.md`. **No transfer policy
has been implemented, and no performance gain is claimed.** It would optimize
the local domain worklist, not replace missing finite-root admission or closure
requirements.

There is still no evidence-backed five-loop completion ETA. Dividing pending
regions by processed regions per second is invalid while the pending set grows;
extrapolating seven heterogeneous roots to 67 is also unjustified. Even a
completed symbolic walk would not by itself implement the explicit finite-root
admission, reachable-witness feedback and final closure contract described in
[the active plan](../finite_starting_domains.md). Finite-root integration is now
underway as a separate implementation track; it is not yet a completed
five-loop solve or a basis for a runtime promise.
