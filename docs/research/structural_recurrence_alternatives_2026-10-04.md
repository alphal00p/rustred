# Structural alternatives to another source-order sweep

October 4, 2026. Research proposals, not implemented algorithms or a claim of
five-loop closure. Root coordinates the proposals; `exact_and_cost_audit`
independently reviews their mathematical and measurement boundaries.

The pressure target remains the frozen physical request. No proposal below
removes a required query, changes the master basis, replaces Symbolica, or
changes production. The aim is to remove whole recurring calculations, not
only to make their scheduling faster.

## 1. Compact moment recurrences for a recognized graph subclass

The profiled owner0 is a six-line five-loop banana. Recognizing a banana from
its incidence/momentum structure is a loop-generic operation; dispatching on
this owner number would not be acceptable. This makes the configuration-space
literature a more relevant lead than an unmotivated new matrix algorithm.

Groote, Körner and Pivovarov reduce sunset-type propagator-power problems to
Bessel-product moments and derive integration-by-parts recurrences for those
moments. Section 4 works after an epsilon expansion with integer-order K0/K1
and logarithmic moments. It leaves a one-parameter moment family; it does not
establish a finite basis for every remaining moment index or supply the
symbolic-nu algorithm sketched below.
[Configuration-space recurrences, section 4](https://arxiv.org/pdf/hep-ph/9903412).

The following is a direct algebraic derivation, not a RustRed result. For unit
mass, introduce

\[
 J_{p,t}=\int_0^\infty r^p K_\nu(r)^{N-t}K_{\nu+1}(r)^t\,dr,
 \qquad 0\le t\le N.
\]

Using the two first-derivative identities for adjacent Bessel orders and
integrating a total derivative gives, **when its boundary is justified**,

\[
 [p+1+N\nu-t(2\nu+1)]J_{p,t}
 -(N-t)J_{p+1,t+1}-tJ_{p+1,t-1}=0.
\]

The attraction is a bounded number of product types at each p, rather than a
separate state for every distribution of indices over propagators. This is
finite *width*, not finite closure: p still varies, and independent symbolic
propagator powers have different Bessel orders. A bounded integer-order
reduction into these two adjacent orders is not automatically a formula for
arbitrary independent symbolic powers.

### A useful negative already found

For N=6 the off-diagonal transfer matrix has entries
`T[t,t+1]=6-t` and `T[t,t-1]=t`. It is singular. A right null vector is
`(1,0,-1/5,0,1/5,0,-1)` and a left null vector is
`(1,0,-3,0,3,0,-1)`. These can be checked by seven elementary row products.
Consequently, saying “there are seven types, invert a seven-by-seven matrix”
would be wrong. Compatibility relations, additional shifts and exceptional
p/d conditions must be handled before claiming a usable recurrence.

### A checked way around that singular matrix

Shift the moment index together with the product type:
`p0=d−1−Nν`, `M_t(q)=J_(p0+t+2q,t)`. The same identity becomes

\[
 [d+2q-2t\nu]M_t(q)
 =(N-t)M_{t+1}(q)+tM_{t-1}(q+1)+B_t(q).
\]

Forward elimination in t divides only by the integers N−t. The final t=N
equation is a scalar recurrence for `F(q)=M_0(q)`, with shifts through
`ceil(N/2)`. Symbolica checks for N=2,…,6 give orders1,2,2,3,3. For six
lines the coefficient of F(q+3) is `(16/5)(2d−2q−9)`, nonzero at d=4 for
every nonnegative integer q. A common convergence strip0<d<2 justifies
vanishing scalar endpoints for q≥0 before meromorphic continuation in d.
The [explicit derivation and checks](banana_moment_nomination_2026-10-04.md)
retain the shift-operator convention, exceptional loci and source terms.

This is materially stronger than the failed matrix-inversion idea: the
positive scalar moment sequence has a finite generating set. It is still
not an installed original-family rule or a statement that the whole hot
numerator sector has three masters. Already two contracted first derivatives
produce M_2(−1), outside this chart, and a nonzero contact term. The original
family map and its ordinary-IBP certificate remain the next discriminator.

There is a second, more physical trap. In the Euclidean convention of the
configuration-space numerator paper, the propagator obeys
`ΔD=m²D−δ`. Section 5, equation 48, retains the resulting contribution
`∫D³ΔD=m²∫D⁴−D(0)³`. Dropping that term loses a pinched product integral.
Higher numerator derivatives likewise need their trace and contact pieces.
[Numerators and contact terms, section 5](https://arxiv.org/pdf/hep-ph/0403122).

**Proposed smallest test:** nominate one moment-derived relation on a bounded
integer-index chart of an already profiled banana sector, keeping d symbolic.
Use the existing numerator-bearing owner0 point with active powers
`(3,1,1,1,1,2)` and one D13 numerator, rather than an all-unit-power scalar
example that might only reproduce a trivial symmetry. Its existing rank-two
neighbor supplies an adjacent-case check, not an unseen performance holdout.
Map every term, including the contact/pinch terms, back to the existing family.
Then seek a certificate with the existing ordinary-source exact machinery and
test native descent and the complete downstream boundary. No numerical moment
values or new terminals are introduced. Failure to obtain the map, source
certificate, compatible order, or useful boundary parks the proposal before a
new representation or solver is built.

This differs from the earlier finite-frame proposal: its starting compression
is a graph-class-specific coordinate-space identity, not a generic guessed
Pfaffian matrix. It still must earn its cost on the unchanged shared workload.
No moment representation, special-function CAS, or new tensor reducer is being
implemented as part of this research note.

### Related results that must not be overinterpreted

Flieger proves a compact set of annihilating operators for generic-mass banana
integrals and reports ranks through eight loops, while conjecturing that the
operators generate the full annihilator. Specializing to equal masses and
zero external momentum needs its own analysis; neither rank evidence nor that
conjecture proves our guarded numerator reduction.
[Generic-mass banana D-ideal](https://arxiv.org/abs/2508.04309).

De la Cruz's polytope study finds only relabelling symmetries for fully
generic-mass bananas through five loops. This gives no evidence of an extra
hidden polytope symmetry that would by itself collapse our hot sector. More
general parameter transformations can also leave the original integral
representation, so they are not automatically valid routing maps.
[Polytope symmetries, conclusions](https://arxiv.org/abs/2404.03564).

## 2. Choose an entire rule using its downstream context

The prior saved-rule subset trials are negative or negligible; their results
are preserved in the [shared-policy study](shared_rule_policy_and_rank_frames_2026-10-04.md).
A state-dependent policy is different from globally preferring rule110 or450:
one alternative could be useful on one guard/degree region and harmful on
another. The comparison must retain **all** terms of the selected rule and
the original baseline fallback. One cannot select a cheap individual RHS term.

SAILIR frames reduction as a sequence of expression-dependent actions and uses
bounded episodes with memoized lower problems. Its benchmark uses specialized
finite-field coefficients and a two-loop family; that is not guard-complete
parametric authority or five-loop evidence. The transferable idea is to score
already valid complete actions using context, not to introduce neural training
or copy its quantitative benchmark as an expected RustRed gain.
[SAILIR, sections III.1 and III.5–III.6](https://arxiv.org/html/2604.05034v1).

This direction was already proposed in the
[bold-directions study](profile_guided_bold_directions_2026-10-03.md); it is not
new merely because the paper was reread. The new question for the retained134
native observations is whether the same exact domain has materially different
admissible choices with context-dependent benefit. The evaluation agent is
screening that evidence before a policy interface or a further sweep.

**Completed negative screen:** all67 recorded same-domain alternatives still
have incomparable whole-child sets after subtracting only exact child
obligations with finite acyclic discharge witnesses available at the actual
recorded decision version. None sends its complete boundary into already
discharged work. All six rule110 observations occur at version0 or1, without
earlier closed children. A smaller number of remaining children is not a
downstream cost proof. Consequently, the cheap “switch only if all new tails
are already closed” policy is parked for this cohort. The result does not
exclude a more informative cost model, but supplies no reason to implement
another global preference sweep. The pinned inventory is
`TMP/rule-optimizer-20261003/profiles/preferred-overlap-applied-v2/historical-closed-choice-screen-v2.json`.

**Falsifier:** if changed choices mostly flow into work required anyway, or
their applicability cannot be described without fragile point exceptions,
do not build a policy optimizer for this cohort. Recorded-but-unresolved work
must not be assigned zero cost. Closed-region information must come from an
actual valid snapshot, not a final graph retrospectively treated as available
at every earlier publication.

## 3. Discharge a finite demand without materializing its abstract graph

The existing exact tracer completed the four original singleton inputs in
0.312s of tracing, reaching44,139 physical keys. The symbolic control used
19,112 domains and about1.006s of traversal. There is no smaller-node result,
and preparation/cold-verification boundaries differ; this is not a matched
speedup. Full details and input pins are in the
[demand/SCC study](demand_directed_symbolic_sccs_2026-10-04.md).

The distinct hypothesis is a hybrid **finite-demand summary**: reuse the
prepared rule bank to follow a small exact region to existing terminals,
without publishing every intermediate abstract domain. This would accelerate
coverage checking while leaving the reusable parametric rules themselves
unchanged. It is not permission to replace a family artifact by a table of
four successful reductions.

The current trace report is not such a certificate. A sound implementation
would need exact finite-region enumeration, immutable context, all nonzero
local successors and guard obligations, and cold reinspection. Retaining every
edge is not essential: a compact replay recipe can regenerate the entire
finite trace, provided the verifier checks every local operation and complete
discharge, not merely trusted success counters. Bind the exact demand,
program/routing/terminal/order context and replay algorithm; never turn sampled
points into a region claim. These are real implementation costs. No independent
algebra kernel is needed; the native Symbolica-backed tracer already supplies
exact local support.

### Completed harder H55 discriminator

The original H55 singleton, unchanged admission caps, same optimized binary
and full67-owner/8246-route/two-overlay context give:

| Diagnostic | Native traversal | Whole attempt | Output and verification |
| --- | ---: | ---: | --- |
| exact physical trace,16 workers | 2.759s | 95.087s | 355,584 physical keys; no frontier/error/debt; no retained cold certificate |
| symbolic walk,16 workers | 34.760s | 251.594s | 385,815 domains; all native records independently cold-reinspected,1/1 root verified |
| exact physical trace,1 worker | 13.627s | 139.877s | identical final trace counters and admission to16 workers, apart from timing/worker fields; not a retained full-key-set comparison |

These are one completed run each, not repeated matched equal-output speedups.
The symbolic whole attempt includes an additional cold process absent from
the exact traces. In particular,2.759 versus34.760s is a12.6-fold difference
between different traversal kernels, not a certified campaign improvement.
The one-worker trace matters because a summary run inside an existing inspector
must not silently create16 nested workers. This measured the shared FIFO
campaign API, which still spawns one worker; the separate inline serial API
uses another path and lacks a cancellation argument. A cancellation-aware
caller-thread implementation is therefore a required integration detail, not
already measured code. The observed one-worker traversal is cheaper than this
symbolic traversal, but shared downstream reuse, replay cost and failed
attempts could erase that benefit. Native preparation also dominates these
small standalone jobs; preparation is reused in a long-running campaign.

The cold symbolic control verifies all314,985 native records and4,838,646
admissions, with zero uncovered branches/errors/frontiers. Its70,840 abstract
cycle nodes are not evidence of cycles of concrete integrals. No finite-summary
engine change or production recommendation has been made. The next minimal
slice must preserve ordinary symbolic fallback, cumulative work budgets,
cancellation and hard-error semantics; then measure total cold-verified work
on the four-loop controls and held-out five-loop requests.

Receipts are under `TMP/rule-optimizer-20261003/`:
`profiles/h55-concrete-trace-v1/`,
`profiles/h55-concrete-trace-w1-v1/`, and
`candidates/h55-samebuild-symbolic-baseline-v1/`.

## Decision discipline

The source-order generation control measured about4% less solve time in one
pair; that is useful evidence but not the goal's20% shared-work improvement.
The four-root exact trace rejects a simple smaller-graph explanation. The
moment recurrence is a structurally different research proposal with concrete
counterchecks, not a breakthrough claim. Keep the live campaign unchanged
until a candidate passes completed controls, independent review and held-out
five-loop measurements.
