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

## Expanded aggregate control

A subsequent run of the same frozen CLI raises only aggregate allowances to
one million admitted domains, ten million callback events and ten billion
general containment comparisons. The six inputs, complete owner library,
per-query native policies, CPU41 and memory envelope are unchanged. This is
not another matched-workload speedup measurement.

The larger prefix completes **3,886 domains**, with **31,710 admitted and
27,823 queued**, then stops at the unchanged per-query matcher allowance:
`rules: requested 100001, limit 100000`. The failed job is Apply domain 3886,
owner `110010101101011`, with the full R11 orthant. This counter measures
repeated rule visits while partitioning the requested domain, not 100,000
distinct generated IBPs. The local inspection ran 2.336 seconds; no new rule
was generated and no missing-rule conclusion follows from this refusal.

The run inspects 812,490 RHS successors, reuses 791,794 requests and performs
716,747,943 general containment comparisons, below the raised ten-billion
allowance. It records zero frontiers in its processed prefix. Its 34 routed
jobs emit 11,008 masks; the scheduling ledger closes exactly:
6 + 812,490 + 11,008 = 31,710 + 791,794. Rank R12 remains a scheduled overcover
bound, not a reached-rank witness or a truncation of input R10 dependencies.

Preparation takes **105.53 s**, traversal **40.44 s**, whole command **150.21 s**,
CPU **147.26 s**, and peak RSS **5,808,368 KiB**. No elapsed or RSS stop occurs;
the process is terminal and reaped. The first 1,055 completed records agree
with the preceding indexed trial apart from elapsed seconds. Evidence and
independent review: `TMP/six-query-index-expanded.g2mRlZ/`. An earlier
preparation-only license setup cancellation is separate and excluded.

## Isolated replay of the stopped R11 domain

The exact full R11 orthant of owner `110010101101011` completes local matching
with the larger allowances already used by the full-census study: 32 million
rule visits and predicates, one million terminal checks and pieces, 100 million
cells, three billion coordinate cells, 100 million splits, and 65,536 bounded-
refinement cells; the guard degree allowance remains 64. These are operational
work allowances, not restrictions on the requested integral domain.

It yields **6,309 selected-rule regions and 43 declared-terminal regions**,
with zero local gaps, unresolved predicates, invalid source conditions or
exact-zero regions. The native work includes 212,438 rule visits, 301,027
predicates, 963,981 cells and 96,480 splits; no bounded refinement is needed.
The earlier 100,000-rule-visit allowance was simply too small for this query.

Preparation takes **1.091 s**, local matching **1.680 s**, whole command
**3.04 s**, CPU **2.88 s**, and peak RSS **103,076 KiB**. The process exits 0
and is reaped, with no elapsed or RSS stop. Its unchanged saved payload is
5,416,079 bytes; no IBPs are regenerated. Evidence:
`TMP/failed-orthant-local-match.9lBujm/`.

This isolated run loads one owner with its family and saved-root zero context,
not the full 67-owner routed snapshot. It does not evaluate RHS coefficients,
check recursive descent/application, route successors or establish complete R11
closure. Its matching time is therefore not comparable to the earlier shared
matching-plus-RHS traversal.

## Shared retry with sufficient local matching allowances

The subsequent full-snapshot retry uses the same six queries, frozen CLI,
expanded aggregate allowances and native RHS budgets, but adopts the larger
matching allowances above. It stops in the same Apply domain 3886, now during
RHS inspection: `guard separable factor work`, requested 134,217,728 versus
the 64,000,000 native guard allowance. This is distinct from both the earlier
rule-visit limit and the successful matching-only replay. Local applicability
does not discharge every original RHS guard or recursive successor obligation.

The prefix still completes **3,886 domains**, with **31,850 admitted and 27,963
queued**. It inspects 906,826 RHS successors, reuses 885,990 requests and records
zero frontiers. The ledger remains exact:
6 inputs + 906,826 RHS requests + 11,008 route requests =
31,850 admissions + 885,990 reuses. Its largest scheduled finite overcover is
now R13; intermediate rank is not clipped to entry rank R10. The queue has not
been exhausted, and no complete-family or reached-rank conclusion follows.

Preparation takes **103.81 s**, traversal **44.96 s**, whole command **152.19 s**,
CPU **151.06 s**, and peak RSS **5,807,780 KiB**. The process exits incomplete
status 4 and is reaped, with no elapsed, operator or RSS stop. Evidence:
`TMP/six-query-generous-match.SzCKVZ/`. The next task is to inspect the actual
refused guard expression and test the narrow native guard improvements, not
regenerate the saved rule library.

Separately, [inspection of the actual guard expressions](guard_obstruction_triage_2026-09-22.md)
identifies inexpensive native guard improvements and distinguishes existing
affine branch representation from genuine missing-rule discovery.

## The applied refusal is an inexpensive affine coefficient

A read-only inspection of the stopped rule identifies one candidate consistent
with the recorded refusal: original RHS term 12, shift `+e5-e14`, with numerator

```
-1 + n10 - n9 + n8 - n7 + n6 - n5 + 2*n1 - 2*n0
```

and denominator `n14-1`. It has nine terms, eight occurring indices, actual
total degree one, and only one base-coefficient equation. The other 27 original
numerators each involve at most one index; there are no equal-shift sums or
common source conditions in this rule. The selected box keeps the denominator
nonzero. Term attribution and the unchanged boundary cell are deductions from
saved data and matching receipts: the original error did not log a term field.

The existing prospective factor-work formula uses the sum of individual degrees
and a dense monomial box. Here it charges `256 * 256^2 * 8 = 134217728`, exactly
the refused amount. A direct call to Symbolica's public factorizer takes
**0.000274131 s** and returns a constant times the same primitive linear factor.
Native multiplication reproduces the original polynomial exactly. The coupled
factor would remain unresolved by the coordinate-zero-locus service, giving a
conditional successor rather than uniform nonvanishing. That latter behavior
is inferred from source; the private guard service was not called by the probe.

This is one factor-call measurement, not an IBP-generation or complete-walk
timing. Its monitored command took 1.04 s, mostly a one-second polling floor;
native child peak RSS was 9,796 KiB. No production cap or saved rule changed.
Evidence: `TMP/applied-rule-native-inspection.ufy0Fs/` and
`TMP/applied-affine-factor-probe.wQvN07/`.

The preferred follow-up is to distinguish optional numerator support proofs
from mandatory validity checks. An explicitly recorded, eligible native-work
preflight refusal in the optional numerator check can conservatively retain
the successor as conditional. Original denominators, child source conditions,
exact algebra, descent, input/output admission and backend errors remain strict.
This avoids making uniform-nonzero classification a prerequisite for dependency
discovery; it neither treats a conditional successor as reached nor proves
closure. Implementation and its separate tests are still pending.
