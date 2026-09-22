# Shared symbolic-domain admission index

## Purpose and scope

The preceding six-query routed control stopped after 100 million general
containment comparisons: 870 domains had finished and 8,724 remained queued.
It had not encountered a missing-rule frontier. This slice optimizes that
measured application-level bottleneck without regenerating IBPs, changing
native Symbolica algebra, or narrowing the requested rank/domain.

The immutable worklist now checks:

1. An exact full-key hash index, retaining equality checks on collisions.
2. A dominant full-orthant entry for the same owner and Apply/Route phase,
   whose rank bound contains the new request's actual rank bound.
3. The existing ordered general-box containment scan if neither index applies.

The exact index and the queue share each domain through `Arc`; coordinate
vectors are not copied into a second retained index. Only newly admitted
domains acquire index entries. Arbitrarily many already-contained requests do
not grow the index. Hash-table iteration never controls scheduling.

All admitted jobs remain in their original FIFO order. Reusing a pending job
does not mean it is solved, and a broader job does not erase previously admitted
narrower jobs. The full-orthant shortcut may identify a different valid containing
job from the old first-match scan; no exact reduction coefficient or rule choice
depends on that representative ID. With unlimited operational allowances, the
sequence of newly admitted domains is unchanged.

## Resource and monitoring semantics

`containment_checks` and `--max-containment-checks` now count general geometric
scan comparisons. Exact-index and full-orthant hits do not charge that scan
allowance; they can succeed after it is exhausted. A new request that needs
another general scan still returns the explicit allowance error. This is an
intentional change to the work-budget definition, not a rank relaxation or a
claim that lookup is computationally free.

Live and final Rust/CLI/Python-driver reports expose `exact_domain_hits` and
`full_orthant_hits` separately from total `deduplication_hits`. Containment
comparisons are also visible during traversal. Domain/rank telemetry changes
only after successful admission. Operationally refused requests never install
an index entry or an apparently completed job.

## Validation status

Independent static review and the standalone optimized queue harness pass all
10 queue tests. They cover mixed-domain comparison against a naive scan,
finite-versus-unbounded rank and endpoints, phase/owner separation, storage
sharing, failed admission and indexed reuse at the scan cap. Existing application
tests gain assertions for the new live/final counters; no prior mathematical
assertion is removed. The 19 Python steering tests also pass.

The full application release gate passes **320 tests** (239 unit and 81
integration), with zero failures. Its 340.41-second command includes about
278 seconds of compilation; that is not a solver timing. The production CLI
was frozen after the full test stage and a final successful executable build.
The previous core gate remains 2,623 passing tests, with 32 existing ignored
diagnostics; this slice changes no core algebra or core solver source.

## Same-input routed retry

Both runs use the same six R10/R11 query boxes, full 67-owner library, 8,246
routing records, per-query native allowances and aggregate allowances: 100,000
domains, two million events and 100 million general containment comparisons.
Both use CPU41, native pools of one, a 64 GiB address-space limit and the
same 48/60 GiB sampled RSS stop policy. Neither uses an elapsed deadline.

| Measurement | Previous linear scan | Indexed queue |
|---|---:|---:|
| Completed local domains | 870 | 1,055 |
| Admitted / still queued | 9,595 / 8,724 | 10,473 / 9,417 |
| Total reused requests | 157,372 | 183,496 |
| Exact-index / full-orthant hits | Not available | 55,644 / 99,660 |
| General containment comparisons | 100,000,000 | 100,000,000 |
| Inspected RHS successors | 161,842 | 188,844 |
| Observed frontiers | 0 | 0 |
| Preparation | 106.39 s | 105.12 s |
| Traversal | 7.42 s | 8.74 s |
| Whole command | 120.72 s | 118.16 s |
| CPU time | 119.21 s | 116.48 s |
| Peak RSS | 5,806,264 KiB | 5,805,140 KiB |

Both terminate with typed incomplete status 4 at the comparison allowance,
with one partially processed failed domain. The index gets about 21% further
in completed domains at that allowance, but **does not eliminate the general
containment bottleneck**. It is not a completed-workload speedup: the amount of
processed work differs, and the earlier shared-host run overlapped another
native control. No full-family generation timing or R10 completion is claimed.

The new scheduling ledger is exact:
6 inputs + 188,844 RHS requests + 5,120 route requests = 10,473 admissions +
183,496 reused requests + 1 refused request. The 28,192 remaining reuse hits
still need general containment scans. Ten route jobs complete; the largest
admitted finite rank remains R12. These are conservative scheduled domains,
not claims that every enclosed integral is actually reached.

An independent comparison finds the first **870 completed domain records
identical**, omitting only their elapsed seconds. This includes every native
counter, phase, owner, rank, box and frontier field; the six input mappings
also agree. The earlier partially processed 871st record is not compared as
though it had completed.

The next control raises aggregate work allowances without changing the binary
or native per-query policy. Eight seconds of traversal is too short a prefix
to judge the complete campaign's feasibility; the larger control must report
its changed allowances and next actual obstacle separately. More general
containment indexing and bounded domain-worker scheduling remain possible
optimizations, informed by that run rather than assumed necessary for closure.

Evidence: `TMP/domain-admission-index.jygnmr/` contains the gate, frozen CLI,
runner, result, timings and independent reviews. The earlier baseline is
`TMP/six-query-routed-owner-walk.yOCjF1/`. Both native processes are terminal
and reaped; neither reached its RSS boundary.
