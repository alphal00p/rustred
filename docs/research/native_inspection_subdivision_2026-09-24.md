# Native inspection cost and physical subdivision

24 September 2026. Follow-up to the
[all-owner controls](all_owner_regional_sharing_2026-09-24.md) and the independent
[architecture](radical_parallel_architecture_2026-09-24.md) /
[critique](radical_parallel_critique_2026-09-24.md) lanes.

## Decision

Challenge the size of a native inspection, not just the queue supplying it.
The stopped controls show both output-blocked producers and a later expensive
head with 255 already-finished inspections behind it. More buffering can help
the first situation but cannot divide the second. Copying the same hard anchor
into independent campaigns duplicates it. A batch barrier can wait for it too.

The next small falsifier divides one source region into exact physical parts
while keeping one logical coverage obligation. It does not regenerate IBPs,
change the entry envelope, discard descendants or establish family closure.
The full 67-owner A24/R15/D9 objective remains incomplete, without a defensible
completion ETA.

## Profile attempt: incomplete capture, not a new hotspot ranking

One frozen all-owner run reached its prescribed scheduled-work stop (exit 4).
The attached profiler enabled recording, but the controller rejected its enable
acknowledgement and stopped the recorder. It was not reattached and the native
run was not restarted. Retained evidence contains 348 samples over only
0.187718118 seconds: all from 25 inspector threads, with 333 samples lacking
recovered stacks and no reported sample loss. This startup burst cannot rank
sustained costs, identify the later expensive head, or establish that helpers
and publication are inexpensive.

The controller required exactly `ack\n`. Upstream perf writes the size of the
string including its terminating NUL; that is a plausible explanation, not proof
of the actual reply bytes, which were not saved. A future controller should log
and parse the framing explicitly. The original failed evidence stays unchanged.
See the primary [acknowledgement implementation](https://raw.githubusercontent.com/torvalds/linux/master/tools/perf/util/evlist.c)
and [tag definition](https://raw.githubusercontent.com/torvalds/linux/master/tools/perf/util/evlist.h).

The profile-affected native run reports 103.074 seconds preparation, 114.473
seconds traversal until cancellation, and 223.564 seconds whole command. It
ends with 385,460 scheduled responsibilities, 109,488 committed native
inspections, 265,957 queued, zero observed frontiers and one cancellation.
These are incomplete observations, not a speed control or closure claim.
Independent raw review agrees. Local evidence:
`TMP/all67-early-profile.erXnWf/{RESULTS.md,INDEPENDENT_AUDIT.md,run/}`.

## Small source-level optimization: classify an unchanged coefficient once

An applied RHS shift group classifies each restricted coefficient before checking
its original child validity. It then used to classify the coalesced sum again,
even when exactly one nonzero contribution survived. Reuse that first result
only while the coefficient is unchanged, on the same source cell and rank.
Zero neighbours do not change it. Any actual addition permanently invalidates
reuse, including a sum that later happens to return to the first coefficient.

The optimization preserves original coefficient, source-condition, pole,
child-root and descent checks. An unknown result remains conditional; cancellation
is checked at the reused boundary. It introduces no algebra primitive or cache
across contexts. Actual additions still use the existing Symbolica-backed
arithmetic and final classifier.

Operation counts and duplicate optional coalesced-refusal diagnostics legitimately
decrease: no unperformed work is charged. Mathematical outputs must agree on
completed comparisons; historical diagnostic equality is not fabricated.
Independent source/mathematical review passes. The release gates pass 2,818 core
tests (32 existing ignored), 562 application/CLI library tests (one existing
ignored), and the CLI build. The first gate found one new fixture using the full
family root while expecting a restricted-root failure. Setting that fixture's
saved root explicitly corrected its setup without changing any assertion or
production code; the failed receipt is retained.

The gated CLI is `TMP/singleton-classification-gate.FrnWbw/rustred`, SHA256
`7b448d84a1863b33ac46060c2c58b6f3e48f625aed13953884aef20d1f91cf76`.
All six alternating completed-workload controls now finish successfully.
Compilation and test-suite timings are not solver timings.

The first pair completed mathematically but its original comparison stopped on
one additional work-counter difference: 21 fewer containment checks out of about
38 million. Independent field-level review found no other unexplained difference.
Removing duplicate optional diagnostics changes 256-event preparation boundaries;
prepared lookups can perform more or fewer comparisons than a fresh lookup after
index retirement. This changes charged work, not necessarily the representative
or admitted domain. For this **unlimited-comparison** experiment, retain and
report the count separately while comparing every domain and ledger record.
Do not generalize this exception to a finite containment allowance or erase
failed comparator evidence. A separately reviewed analysis adapter retained the
completed first pair and ran only the remaining four planned controls, without
a solver change, retry or altered native allowance.

### Completed release comparison

The same 762 input regions (34 Apply, 728 Route) retain all four original
A12/R3/D9 roots and every escaping descendant. Both executables use W50,
the default 25-inspector/24-helper/one-coordinator split, Ordered publication,
H256, the same CPU affinity, serial nested pools and the same memory/work
allowances. Three alternating pairs run on the shared host; timings below are
medians over each executable's three runs.

| Metric | Before | After |
|---|---:|---:|
| Traversal wall time | 11.550 s | 10.873 s |
| Whole-command wall time | 15.239 s | 14.235 s |
| Whole-command CPU time | 129.28 s | 117.57 s |
| Cold preparation | 2.696 s | 2.670 s |
| Peak RSS | 772,920 KiB | 775,164 KiB |
| Sampled traversal busy cores | 10.75 | 10.37 |
| Charged native operations | 12,519,140 | 8,990,749 |

Median traversal falls 5.86%, whole wall 6.59%, and CPU 9.06%; peak RSS rises
0.29%. Each paired traversal is faster (3.22%, 3.75%, 5.86%), but three shared-host
pairs do not establish a confidence interval or a full-family forecast. Less
busy CPU accompanies less work here; it is not evidence of improved fifty-core
occupancy. Cold preparation and output remain part of the whole-command cost.
The supervised whole-command wall measurement also includes exit detection
through a one-second polling loop; its approximately one-second median difference
must not be attributed entirely to native computation or treated as millisecond
precision. The direct traversal timer and whole-process CPU support the more
useful comparisons above.

All controls schedule 29,718 responsibilities and complete 23,863 native
inspections, with 1,727,821 successors, 23,157 conditional successors and zero
frontiers, failures or pending obligations. Every original handle remains.
Same-executable non-timing structural results and counters repeat exactly.
Across executables, the explicitly explained diagnostic changes are 3,528,391
fewer native operations, 81 fewer duplicate coalesced refusals, 24 fewer emitted
first-phase diagnostic events, and the 21-comparison lookup-work difference.
Original refusals, domains, successors, support checks and ledger records agree.
This is local operational exhaustion, not the full 67-owner goal or its concrete
runtime/delivery check.

The original comparator stop is retained under
`TMP/singleton-classification-compare.J33lfC/run/`. The four continuation receipts
and combined measurements are under `continued-run/`; `RESULTS.md` records every
run and both executable hashes. Independent final raw review **passes**, including
a separate full-result comparison, all six scope/drain/resource checks and
unchanged input hashes; see that directory's `INDEPENDENT_AUDIT.md`.

## Proposed discriminator: broad, split serial, split parallel

Use the observed expensive initial anchor with owner mask `000011001001011`,
A≤7, R≤6 and no D floor. These are **input data**, not an engine specialization.
Its local coordinate x0 is a nonnegative numerator index. Partition it into
x0=0 and x0≥1, retaining all other box, A/D and rank bounds:

```
one required region Q (35,035 integer points)
          /                         \
     Q0: x0 = 0                 Q1: 1 <= x0 <= 6
     21,021 points              14,014 points
```

The public ordered Apply-successor visitor already supports both parts. A small
ignored diagnostic can load one immutable owner snapshot and compare Q against
the same Q0/Q1 inspected serially and in two scoped threads. A bounded stats-only
sink avoids retaining millions of event payloads; it still counts conditional
successors, failures and unsupported transitions. It deliberately does not follow
children and cannot establish recursive coverage or campaign speed.

Broad-versus-split work can differ because splitting changes guard partitions
and conditionality. Serial-versus-parallel must retain identical per-part statuses,
statistics and bounded diagnostics. The stats-only check is not full coefficient
or event replay; mathematical behavior comes from the unchanged native visitor.
One fixed half/remainder allocation of the parent's cumulative allowances
is used for both split modes; independently resetting the full allowance in each
child would silently double the budget. Per-operation algebra limits remain
unchanged, and both calls share one process memory allowance. An imbalanced
budget refusal is incomplete evidence, not a mathematical counterexample.

A first block tests feasibility. Rotate execution order and repeat completed
controls before attributing a speed gain; count initialization, work, memory
and the slowest part rather than only CPU occupancy. Reject this particular
split if it mostly duplicates algebra, leaves essentially all work in one part,
or fails to reduce completed inspection time. A native-only improvement still
needs an end-to-end publication/reuse control.

The first standalone attempt has now stopped **before any native inspection**.
Its helper inherited a 1 GiB default aggregate input allowance rather than reading
the frozen manifest's explicitly declared 2 GiB allowance; the 67 bundles total
1,280,854,595 bytes. This is a diagnostic-loader mismatch, not a mathematical or
performance result. The old source and exit-1 receipt remain preserved under
`TMP/one-hop-subdivision.yvZSvc/execution.U1FNOE/`. A fresh helper correction must
honor the same manifest loader limits as the production path without changing
the native query allowances, source partition or process resource policy.

## Production integration constraints

Physical parts must not be admitted as ordinary successor domains that immediately
reuse their own pending broad parent: that would skip the intended inspection.
Keep the parent responsibility and give inspectors distinct physical work tickets.
Finish the parent only after every part has completed and all outgoing obligations
are published. Preserve original guard priority, aggregate accounting, cancellation
and failure states. Genuine RHS successors may still use ordinary global sharing
under the existing descent and completion conditions.

This need not introduce a general Boolean proof engine, independent per-sector
Symbolica copies, or a new CAS. It does require distinguishing logical obligations
from physical tasks, including partial-result accounting. Fixed publication order
can still create a head; subdivision is a hypothesis to measure, not a guarantee
of fifty-core utilization. Local readiness/design notes are in
`TMP/all67-performance-observer.wPENvq/{PHYSICAL_SUBTASKS.md,ONE_HOP_SUBDIVISION_READINESS.md}`.

## Competing idea: activate optional reusable regions only on demand

The architecture lane also challenges eager preparation itself. The all-owner
anchors cover far more points than the required input. Keep candidate anchors as
inactive descriptors; at ordinary admission, first seek active reuse and only
then consider activating a containing candidate. Admit that candidate as a real
obligation **before** letting the original request reuse it. Keep every original
input handle, all candidate failures and all outgoing descendants. Activation
must bypass its own candidate lookup to avoid a circular zero-work justification.

This might avoid work rather than merely divide it, but the independent critic
identifies a strong counterexample: one cheap request activates the same broad
expensive head, whose extra descendants activate nearly everything else. Current
reports also do not preserve enough ordinary reuse-target history to infer which
eager anchors were truly unnecessary. A baseline demand census cannot predict
the work introduced after activation.

The existing partial-D initial index assumes pinned, real earlier obligations;
it cannot be repurposed for inactive descriptors without changing that contract.
A first whole-containment-only lazy policy would therefore lose some demonstrated
sharing. A future completed direct/eager/lazy comparison should charge cold-index
construction, activation and all descendant work. This is an independently
challenged alternative, not an implemented feature or a reason to restart the
full campaign. Local assessment:
`TMP/all67-performance-observer.wPENvq/LAZY_ANCHOR_ASSESSMENT.md`.
