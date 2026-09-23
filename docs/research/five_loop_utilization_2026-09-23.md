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

## Larger diagnostic: live snapshot, not a completed timing

Receipt `shared-owner-campaign.lgvisscc` uses the same input, ordering,
executable and expanded per-query policy, with aggregate allowances increased
to ten million domains and 200 million events. All 67 initial regions have
now finished local inspection and the worklist is processing descendants.
This is neither a new artifact nor closure of those regions under recursion.

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
Pending work grows in every complete minute window measured so far. The
memory headroom supports continuing this diagnostic, not predicting that it
will finish. The ten-million-domain allowance is not a completion denominator.

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

A smaller candidate for investigation is exact-summary hashing before general
containment searches. Live candidates form a containment antichain: a newly
admitted region was not contained by a live candidate, and retires candidates
that it contains. Thus equality with a live native summary identifies the
unique containing live candidate. Any optimization must preserve raw exact-key
and full-orthant priority, phase/owner identity, liveness checks, finite-cap
behavior and ordinary fallback. Existing semantic-inclusion hit counts do not
measure the fraction of exact-summary equality hits; collect that evidence
before promising a speedup. No new hash index has been implemented here.

There is still no evidence-backed five-loop completion ETA. Dividing pending
regions by processed regions per second is invalid while the pending set grows;
extrapolating seven heterogeneous roots to 67 is also unjustified. Even a
completed symbolic walk would not by itself implement the unfinished explicit
finite-root admission, reachable-witness feedback and final closure contract
described in [the active plan](../finite_starting_domains.md).
