# Finite-cover falsifier — 24 September 2026

## What was tested

The [whole-system architecture review](ibp_generation_architecture_review_2026-09-24.md)
and its [independent critique](ibp_generation_independent_critique_2026-09-24.md)
recommend trying a different unit of coverage work before building another
scheduler. Instead of discovering many overlapping regions along descendant
paths, admit a small list of larger regions up front and reuse them. This is a
test of that idea through the **existing release CLI**, not a new solver or a
change to the rules. All escaping descendants remain obligations.

The required set E is unchanged: four saved owner sectors, positive-power sum
A≤11, numerator rank R≤2 and D=A−R≥9. It contains 45,342 distinct sector-labelled
integer tuples. This is a completed local control, not the full 67-owner goal.
Let P=A+R. The first alternative C13 admits P≤13 in the four installed sectors.
For a sector with m positive indices, the native input uses one slab
`A≤k, R≤13−k` for each integer k=m,…,13. Their union is exactly P≤13:
every slab is contained in it, and any tuple in it belongs to the slab k=A.
Since E has A≤11 and R≤2, E is contained in C13.

The 30 overlapping slabs represent 730,626 distinct tuples, including 685,284
not in E. Their summed memberships, 1,075,590, are not a unique-integral count.
This compares strategies for the **same required E with different checked
inputs**, not identical native workloads. All coordinates are finitely bounded;
the native finite-axis guard refinement remains enabled.

| Owner mask | Positive indices | Slabs | Required E tuples | C13 tuples |
|---|---:|---:|---:|---:|
| `111100000011100` | 7 | 7 | 10,234 | 54,264 |
| `111010100100101` | 8 | 6 | 4,616 | 15,504 |
| `000010011001001` | 5 | 9 | 15,316 | 490,314 |
| `001100101110000` | 6 | 8 | 15,176 | 170,544 |

These masks are diagnostic inputs; no engine dispatch or production relation
was specialized to them.

## Completed first pair and routing-source follow-up

The E and C13 fresh-process walks completed with an exhausted queue, all retained ledger
obligations discharged and no frontiers, failures or pending worker results.
The baseline reproduces the earlier exact structural digest and semantic
counters. Independent raw audit passes. These are single shared-host diagnostic
observations, not repeated-run confidence bounds. A separately admitted third
fresh process, C13-R, adds the same P≤13 slabs for the manifest's 86 routing
source masks. It also completes and passes independent raw audit; its scope and
audit correction are described below. No production implementation or saved
bundle changed between runs.

| Metric | Required E | Four-owner C13 | C13-R: Apply + Route seeds |
|---|---:|---:|---:|
| Unique sector-labelled initial tuples | 45,342 | 730,626 | 29,816,130 |
| Owner preparation | 2.690 s | 2.699 s | 2.688 s |
| Post-load traversal/report/queue cleanup | 5.846 s | 12.174 s | 6.799 s |
| Whole supervised command wall | 9.08 s | 16.10 s | 10.11 s |
| Whole command CPU | 50.98 s | 94.77 s | 83.67 s |
| Whole command peak RSS | 587,964 KiB | 921,692 KiB | 509,956 KiB |
| Scheduled records | 37,436 | 50,958 | 11,557 |
| Native Apply inspections | 10,146 | 3,856 | 3,870 |
| Native Route inspections | 17,660 | 32,991 | 5,888 |
| Committed events | 695,918 | 1,504,106 | 1,346,005 |
| Containment charges | 41,875,786 | 22,062,847 | 7,161,653 |
| Attempted native operations | 3,227,881 | 8,653,550 | 8,738,977 |
| Conditional successors | 1,916 | 18,248 | 18,398 |
| Mean heartbeat-labelled busy cores | 8.51 | 7.24 | 11.52 |

For C13, Apply inspections decrease 62.0%, but Route inspections increase 86.8%
and traversal takes 2.08 times as long. Larger Apply regions also require more
predicate/operation work. Inspection counts do not measure exclusive phase CPU.
**This cover does not demonstrate a speedup or better fifty-core scaling.**
Optional coefficient-refinement refusals are 36/62/62, with conservative
conditional successors retained; they are not discarded obligations.

Traversal starts after owner preparation and ends after initial admission,
walking, report assembly and queue cleanup. It excludes owner unload and file
serialization. Whole-command wall/CPU/RSS include those costs. CPU occupancy
uses sampled intervals labelled by the latest native heartbeat, not exact
isolated traversal CPU. Controller wall, which also includes polling/draining,
was 9.223/16.239/10.227 s and must not be substituted for the GNU command-wall figures.

## Does providing the routing regions up front help?

C13-R preserves the original 30 Apply slabs and appends 728 Route slabs, one
P≤13 cover for each of the 86 distinct manifest route-source sectors. The input
contains 758 queries and 499,378 bytes. The only new admission setting is
`--max-queries 758`, since the default 256 cannot admit that document. Traversal,
per-query, resource and cooperative diagnostic allowances are unchanged. Existing
native admission automatically starts uninstalled source sectors in Route phase
after checking shared source-validity obligations; no new phase schema was used.

Its 90 sector-labelled covers contain 29,816,130 distinct geometric tuples;
44,329,812 is instead the sum of overlapping slab memberships. They are not
symmetry-quotiented integral counts. The required E remains 45,342. Every initial
handle retains its full geometry and expected phase (30 Apply, 728 Route).
The execution reports no source-validity frontiers or other frontiers, and all
native, alias and worker responsibilities discharge. No diagnostic gate is hit.

This reduces native inspections from 27,806 in E to 9,758 and containment charges
by 82.9%, with lower peak RSS and a 15.93 MB native result rather than 49.31 MB.
It repairs much of C13's routing overhead. However, compared with E, traversal
is still 16.3% slower and process CPU 64.1% higher in this single observation.
Attempted native operations are 2.71 times E. Thus the remaining work inside
broader Apply regions matters; lower record counts and higher occupancy do
not demonstrate a faster same-required-E solver. This is a useful reduction
in queue/report size, not a delivered speedup.

The explicit 86 routes are not all possible future Route jobs. The preceding
C13 walk visited 227 distinct Route masks: 46 manifest sources and 181 other
masks resolved by the native zero-sector path. This input also pre-checks 40
manifest sources not visited in that walk. New masks and escaping descendants
remain normal obligations. Neither the original C13 nor C13-R is an invariant
cutoff or a replacement for the complete 67-owner delivery.

## The proposed starting cover is not a certified invariant

The completed walk admits symbolic descendant regions extending to P=16,
compared with P=15 in the baseline. In the C13 result, 8,410 retained records
contain points with P>13: 4,836 Apply and 3,574 Route records, including aliases.
Ninety-seven records lie wholly above P13. All were retained and discharged.
These are overlapping domain-record counts, not distinct or demonstrably
concretely reachable integrals; conditional and routing overcovers can include
extra points. They show that C13 is **not certified closed under this
conservative walk**, not that every such point is a necessary actual successor.

For example, native Route record 42,416 in sector `111100000001100` admits
A=14,R=2,P=16 while satisfying its D≤13 bound. Exact aggregate extrema account
for the joint box, rank and A/D constraints; adding independent maxima without
joint feasibility would be invalid. A P13 descendant cutoff would remove
recorded obligations. No such cutoff was used. C13-R similarly retains 8,419
records admitting P>13 (4,843 Apply and 3,576 Route), with 97 wholly above P13
and the same maximum P=16. Its additional initial routing covers do not remove
the escape-handling requirement.

## What the guard diagnostic clarified

A separate existing-CLI diagnostic displayed the original saved conditions
behind the earlier eight uncertain broad-domain cells. Relevant components
include `n0−n1≠0`, `n0−n1−n10≠0` and the complementary affine branch
`n0−n1=0`, using zero-based original-index names. These are coupled affine
diagonals, **not demonstrated nonlinear Diophantine obstructions**. Other guards,
original denominators and source conditions remain required.

As a simple illustration, a box containing n0<n1, n0=n1 and n0>n1 cannot attach
one truth value to `n0−n1≠0`. Restricting the box to concrete points resolves
that question, but repeating such refinement can be costly. Native rule 232 is
excluded at the inspected diagonal point; the previous ordered diagnostic uses
the existing later rule 239. Directly selecting rule 232 is not a missing-rule
witness. The saved program also already contains an affine rule at 285.

All four nominated-rule diagnostics completed without errors, and the original
eight fixed-point ordered inspections succeeded. Those points belong to the
broader documented finite entry envelope, not necessarily this A11 control E;
one has A=12,R=1,D=11. Neither diagnostic proves coverage of its parent boxes.
The finite C13 run can use existing finite-axis refinement
and did complete; no new affine solver was required. Symbolica-backed affine
charts and native guarded one-hop application already exist. A future ordered
guarded-program service would still have to preserve earlier-rule rejection,
power bounds, original validity and routed source preimages. General numerator
routing is not an affine bijection on exponent tuples.

## Controls and retained evidence

All three runs used the unchanged release executable with SHA256
`02a1c86642d6d22d68c2e304b2a7b67454bd149e72bdba162b20d31f38634716`,
four installed owners and 86 saved routes. All used Ordered W50 with the
default 25 inspectors, 24 helpers and one coordinator, CPUs 0–49, observer CPU50
and inner pools set to one. Native address space was limited to 28.8 GB,
supervised memory to 24 GB soft/32 GB hard. There was no elapsed-time cutoff.

The same cooperative diagnostic gates were 112,308 scheduled records, 83,418
returned native inspections or 2,087,754 events, three times the earlier
baseline counts. No run crossed a gate. These gates reject an uneconomical
pilot, not mathematical inputs; a stopped prefix is never complete. Existing
per-query and native hard allowances remained unchanged.

One earlier wrapper setup failed CPU-affinity validation **before any native
launch**. It was retained and excluded from solver timings. The correction only
gave the wrapper access to CPUs 0–50; native and observer affinity stayed as
specified. No source or mathematical input changed.

C13-R's native process and observer both exited zero, but its initial post-run
comparison script failed: the inherited option normalizer assumed the baseline
had an explicit `--max-queries` option. It did not. A separately reviewed offline
adapter checks and removes exactly the approved `--max-queries 758` pair before
comparing the remaining arguments. It then checks all retained native inputs,
hashes, ledger chains, counters, limits and drain state. Original files and the
failure remain available; **the native run was not repeated**. Independent raw
review confirms the corrected comparison and the stated result.

## Architectural decision

The experiment establishes that admitting shared routing regions can remove
substantial queue/history work. It does not yet establish a faster strategy for
the required entry set. Do not broaden the P bound, change scheduler defaults
or restart the all-67 campaign on this evidence alone.

The unchanged broad Apply regions retain about 8.7 million attempted native
operations in C13 and C13-R, compared with 3.2 million for E. Term visits are
3,073,498 / 3,107,373 versus 1,479,853 for E. Bounded guard-refinement cells are
only 20 / 20 versus 10; the earlier diagonal examples do **not** establish that
singleton refinement dominates these runs. Reusable inner predicate/term work
and shared per-phase transfers remain candidates to measure, not demonstrated
hotspots with a promised payoff. Keep exact rule priority and original validity.

The proposed next cheap test is amortization, not another broader cover: compare
the existing larger A12/R3/D9 control against that **same original entry input
plus these fixed P13 anchors**. A12 allows P14–15, so its original queries must
remain present and descendants remain uncut. Only 60,390 of its 357,192 entries
lie within P13; the hypothesis is downstream reuse, not prior coverage of most
entries. Use a fresh baseline in the same measurement block. Predeclare equal
A12-specific work gates for both runs: the old A11 event gate is smaller than
the already observed complete A12 event count, so blindly reusing it would be
an invalid experiment. Do not increase gates after seeing the result.
Charge cold loading, every added
anchor inspection, algebra/events, output, CPU and memory. If that still only
compresses records without an acceptable runtime tradeoff, stop this cover
sequence and profile reusable Apply interiors before implementing a compiler.
No such fourth/fifth run or new production implementation is included here.

Local evidence (ignored TMP, not a portable artifact dependency):

- `TMP/finite-cover-c13-input.yNn5pp/`: input, exact counts and verifier.
- `TMP/finite-cover-c13-pair.Zx8kPt/`: frozen runner, pair receipts, results,
  independent raw audit and exact represented-domain extrema.
- `TMP/guard-inspection.LT843n/`: native guard display and independent audit.
- `TMP/finite-cover-c13-r-input.SJEqxs/`: routing-source input and exact counts.
- `TMP/finite-cover-c13-r-run.0JX9MN/`: third receipt, frozen runner, offline
  comparison correction and independent raw audit.

No new IBP generation, all-67 completion, global concrete termination,
original-source replay certificate or final cold package is claimed here.
