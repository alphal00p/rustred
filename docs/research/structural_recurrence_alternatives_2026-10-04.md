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

**October4 implementation discriminator:** the numerator-bearing chart now has
a native-checked ordinary-IBP certificate. A momentum-transfer vector field
gives eight source rows and four scalar tails on D13=-1, with all six active
powers free and D15≥2. Regenerated sources, both relevant sign cells, retained
guards and current-order descent pass; a one-rule isolated export also passes.
This is a useful outcome of the structural investigation without needing a
new moment engine. It is not evidence of less shared downstream work yet.
The existing repair overlay must be source-replayed against the replacement
owner and cold-loaded before the complete-context comparison. See the
[derivation and receipts](banana_moment_nomination_2026-10-04.md).

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

## 4. Eliminate a numerator variable before exploring its lattice

Jiang, Lian and Yang's top-sector ISP reduction keeps propagator variables
as parameters while reducing the numerator variables. Unlike a maximal-cut
calculation alone, it retains enough information to construct the pinched
subsector remainder. Their equations31–51 also expose a practical hazard:
intermediate generalized Baikov denominators must be removed before returning
to ordinary integral families. Their three-loop banana example does not prove
an efficient generic five-loop implementation. Its particularly easy lower
sectors are explicitly not representative of all graphs.
[Primary source, §§II,IV.1,IV.5](https://arxiv.org/pdf/2312.03453).

The following is **our proposed narrow discriminator**, not an implemented
intersection-theory engine or a result copied from that paper. For a quadratic
ISP weight `P(z,y)=A(y) z²+B(y) z+C(y)`, with the remaining factors independent
of z, write the partial integral `J_n(y)=∫z^n P(z,y)^γ dz`. A total derivative gives

```text
A(y) (n+2γ+2) J_(n+1)(y) + B(y) (n+γ+1) J_n(y)
  + n C(y) J_(n−1)(y) = 0,
```

provided the endpoint term vanishes in a justified convergence region and
the relation is continued consistently. This nominates a numerator-lowering
combination without searching a large rectangular cloud of seed indices.
It is not enough to divide by A and declare a new integral recurrence:
A,B,C can depend on other variables that still have to be integrated.
Their polynomial monomials correspond to simultaneous index shifts;
`1/A` may instead introduce an inadmissible new denominator. Other
z-dependent weight factors would also contribute missing derivative terms.

**Smallest test and falsifier.** On one already-profiled sector, inspect
quadratic ISP choices using existing Symbolica matrix/polynomial operations.
Prefer an A independent of integration variables, or an exactly manageable
monomial shift. Expand the complete polynomial identity into the existing
integral family, retaining pinches and exceptional factors; seek an ordinary
IBP source certificate and current-order descent using the existing checker.
Reject the candidate if this requires unrepresented denominators, unresolved
boundary terms, or cofinally nonlower tails. Only a checked candidate proceeds
to unchanged-context work measurement. A smaller partial-integral basis is
not itself evidence of fewer campaign domains or sufficient final masters.

This is related to, but not identical with, the earlier protected-source
module searches: it proposes a structural elimination variable and complete
identity rather than requiring every source to protect every denominator.
Those earlier negative results remain valid. No separate CAS primitive,
intersection-number package, tensor reducer, or new terminal basis is proposed
for this first discriminator. Independent mathematical critique accepts this
nomination with the explicit partial-integral convention above. Native
applicability, endpoint terms, dimension/prefactor bookkeeping and an ordinary
source certificate remain untested; no new rule follows merely from the formula.

### Completed D13 discriminator: quadratic, but not a descending direct rule

**No-go for this direct nomination, by the actual family coordinates.** This
is a hand-derived structural obstruction, independently checked against the
family and ordering code; it is not a native checker receipt or a CAS run.
The existing [banana source proof](banana_moment_nomination_2026-10-04.md)
is unaffected.

Use the original Minkowski-convention family `D_i=q_i²−1`, not a massless
replacement. Its owner0 active denominators are D5,D6,D9,D12,D14,D15.
In the proposed chart D13 carries an arbitrary numerator power, while the
other inactive indices are zero. Those zero indices do **not** set their
integration variables D_i to zero.

Let `G_ij=k_i·k_j`, `z=D13` and `P=det G` for the five loop momenta.
Only `G45=G54=(D4+D5−z+1)/2` depends on z. Selecting both of these
off-diagonal entries in the determinant therefore gives

```text
A = [z²]P = −¼ det G(k1,k2,k3)
  = −¼ (abc + 2uvw − aw² − bv² − cu²),
a=D1+1, b=D2+1, c=D3+1,
u=(D14+D4−D7−D10)/2,
v=(D1+D3−D6+1)/2, w=(D2+D3−D9+1)/2.
```

The expression for u uses the actual `q14=k1+k2−k4`; replacing D14 by
a pairwise-difference denominator would change this conclusion. In
particular, A contains `+(D3+1)D4²/16`, so P is genuinely quadratic in
D13, but A is neither scalar nor monomial. More strongly,
`+(D3+1)(D14+D4−D7−D10)²/16` contributes twenty distinct monomials
quadratic in `{D14,D4,D7,D10}`. No other displayed determinant term has
degree two in that set, so these terms cannot cancel. Twelve of them use
only the inactive variables D3,D4,D7,D10. This is a structural subset
count, not a full expanded-polynomial census.

For five-loop vacuum Baikov variables, `γ=(d−6)/2`. To isolate the original
numerator rank r in the proposed three-term identity, set `n=r−1`, so its
J_r coefficient is `(r+d−5)A`. The twelve inactive-only monomials leave
all active powers and the D13 power unchanged while adding two or three
other numerator powers. They consequently increase absolute degree/corner
distance and are nonlower in the saved SpIRed order. The B and C terms
carry D13 ranks r−1 and r−2, so cannot cancel these same literal columns.
The exceptional locus `r+d−5=0` removes the entire J_r coefficient; it
does not rescue target isolation. Dividing by A as though it were an
external coefficient would instead introduce an unrepresented denominator.
The Baikov exponent and partial-integral convention are from the
[primary paper, §II](https://arxiv.org/pdf/2312.03453); the coordinate/minor
calculation and descent obstruction above are our deduction.

Evidence is the original
`campaigns/five-loop-a1-epoch-20260930/shared/family.toml`
(SHA256 `9cca7932b85b38f374ab23845391527cdd04ef767bd5e33c5b2a311a3f89f55a`)
and the already registered chart/order in
`TMP/rule-optimizer-20261003/candidates/banana-owner0-all-positive-rank-proof-v2/request.json`
(SHA256 `ee97ca0e39b8a0b0f3704f1601a06a42503b431a4ce2049badca843a70314f52`).
The ordering contract is in
[`sector/ordering/policy.rs`](../../crates/rustred-core/src/sector/ordering/policy.rs).
Pinned Symbolica exposes native `Matrix::det` and polynomial coefficient
grouping, also described in its current public
[matrix](https://symbolica.io/docs/matrices.html) and
[polynomial](https://symbolica.io/docs/polynomials.html) documentation;
existing RustRed determinant wrappers authenticate contexts and budgets.
The minor identity makes a new algebra computation unnecessary here.

Park this direct D13 recurrence: no source nomination, CAS calculation,
native run, algorithm build or bank growth follows. This does not rule out
combinations that eliminate the troublesome A-weighted terms, another
variable/chart, or Baikov methods generally. Such alternatives would be
new proposals requiring their own complete source and descent proof, not
consequences of this negative.

## 5. Improve the lower-sector boundary without replacing the leading recurrence

October4 follow-up, after the complete260-integral check: the combined banana
rules save only0.129% of operational states on that region. Their larger local
gains do not justify deploying another full campaign. The next nomination
should change the expensive descendant mechanism, not replay a successful
small example with more workers.

### What the newer geometry literature contributes

Bree et al. order differential forms using localisation, residues and pole
structure. Their detailed paper explicitly identifies unused freedom between
forms equivalent under maximal-cut ordering but differing in lower-sector
couplings. Optimising those couplings is proposed, not demonstrated there.
The method also needs a map between differential forms and integral classes;
supersectors and symmetries complicate that map. It is not an arbitrary integer
weight vector to insert into the existing RustRed comparator.
[Detailed algorithm, §§3.2.3,4.1,6](https://arxiv.org/html/2511.15381v1).

The later overview illustrates how basis selection avoids spurious denominator
growth and organises elimination in smaller geometric pieces. That is relevant
to coefficient swell, but supplies no five-loop vacuum timing or bound on our
domain worklist. Implementing a new cohomology representation merely to change
the comparator would be a substantial project, not a justified near-term
optimization.
[Overview, §2](https://arxiv.org/html/2602.10651v1).

### A smaller native inference: an affine boundary-correction problem

Let a source-replayed incumbent be

```text
R0: I(n) + S(n) + B(n) = 0,
```

where S contains its same-support descendants and B its strict pinches.
Instead of asking a small new source bank to reduce I(n) independently, seek
ordinary-source weights w such that the complete image L=w A has **zero
same-support coefficients**, while B+L loses a nominated expensive class.
Then I(n)+S(n)+(B+L)(n)=0 has the same leading recurrence and a changed
lower-sector boundary. All terms of L outside the permitted root or above the
saved order must vanish as well. These are simultaneous exact linear
conditions over the existing Symbolica coefficient field, not a new CAS.

This is our finite-window inference, not the full geometric algorithm or a
novel mathematical identity. Laporta's subsystem-relation example already
motivates reduction of weighted combinations. The practical distinction from
the previous first-target search is that the known target circuit is retained
as an affine offset; the added bank need only improve its boundary. A bank
that misses I(n) can still contain useful lower-sector relations. Conversely,
relabeling a generic forbidden-column solve is not an improvement: the actual
incumbent, conserved same-support coefficients and complete changed tails
must be inspected.

**Prospective smallest test.** Use the observed scalar owner31/rule210 on its
actual two-free-index chart. It has93 nonzero rank-one raw descendants at the
fixed witness,82 of them pinches; the full routed boundary has91 rank-one
survivors. These are different accounting boundaries, not interchangeable
counts. Recover the native incumbent source circuit first. Freeze one source
window and its exact domain; require a **zero-correction control of the combined
representation** to reproduce the incumbent including authenticated zero-sector
equivalences and guards. The correction bank alone need not contain the
incumbent. Then nominate one complete rank-increasing pinch class, using typed
whole-chart/sign-cell geometry rather than extrapolating the82 singleton
pinches, while preserving every same-support coefficient. This does not
promise elimination of the11 same-support numerator terms or rank-zero closure.

For selected pinch columns P and prohibited columns F, the simultaneous
conditions are `w A_same=0`, `w A_F=0`, `w A_P=-B_P`. The present JSON
projector solves a homogeneous single-target problem; this affine use needs
a small explicitly reviewed adapter or an existing native linear-system
interface. It is not supplied by changing its forbidden-column list alone.

A miss confines the conclusion to that bank and objective. Success still needs
all coefficient poles, exceptional faces, source replay and strict descent;
follow it with complete weighted routing and a completed shared-work control.
No raw tail, auxiliary supersector or maximal-cut remainder is discarded.
No construction or run is authorized merely by this design section.

### October4 discriminator and the narrower implementation now authorized

The original150-row bank cannot touch any of the82 fixed-pinch numerator
columns: all its translations involve the two free axes, whereas those
columns require two distinct negative fixed-axis shifts. The recovered291
source pairs do reach a full-original-source target; after correcting a
proof allowance, the native checker proves a313-tail rule on four cells.
A matched preservation test on the six original zero axes then misses in
that same bank. These results motivate a different source space, not a
larger arbitrary shell or a reinterpretation of a failed solver run.

The implementation nomination now uses the typed313-tail full-source proposal
as R0, not the different330-tail saved rule modulo authenticated zero sectors.
At a fixed unit power, seed ordinary IBPs after pinching that denominator to
zero. Every derivative term that could restore it has that zero power as its
coefficient; hence all surviving terms stay in the pinched sector. These
identities cannot change R0's leading or same-support coefficients.

This permits a smaller adapter than a general affine solver. Use the existing
weighted source span with rows `[R0, pinched ordinary identities]`. Only R0
contains the target, so homogeneous target normalization forces its weight to
one. The existing Symbolica projector can impose the unwanted-boundary zeros,
after which native source composition and chart proof remain mandatory.
No new integral column, custom elimination, coefficient-display parsing or
zero-sector shortcut is needed. A zero-correction control must reproduce R0;
the actual313-term boundary determines the nominated class, not the old330
term census. The first test also forbids newly introduced numerator columns.

The geometric literature provides motivation, not this algorithm or a timing
prediction. In the related unequal-mass three-loop banana example, basis-dependent
projection changes tadpole couplings while preserving the top-sector content;
that concerns differential equations at nonzero external momentum. It does
not license dropping our pinched terms. Our correction instead adds identities
whose complete original-family image is retained.
[Explicit example, §3.5](https://arxiv.org/html/2507.23594v1#S3.SS5).

### Why mass differentiation is not an immediate escape

MERLIN generates higher propagator powers by covariant differentiation of a
precomputed mass-dependent master connection. Singular equal-mass limits use
series expansions, and can expose additional relations. However, the initial
connections still require reductions; its supplied vacuum examples stop at
three loops and its stated method does not yet cover general ISP numerators.
[Covariant differentiation, §§2,4,5](https://arxiv.org/html/2604.09810v1).
This was already listed in the earlier literature survey. For our newly
observed scalar-to-numerator branching it does not provide the missing generic
five-loop connection for free. Defer a new mass-space backend; reconsider if
a compact applicable connection can actually be constructed and checked more
cheaply than the current rules.

## Decision discipline

The source-order generation control measured about4% less solve time in one
pair; that is useful evidence but not the goal's20% shared-work improvement.
The four-root exact trace rejects a simple smaller-graph explanation. The
moment recurrence is a structurally different research proposal with concrete
counterchecks, not a breakthrough claim. Keep the live campaign unchanged
until a candidate passes completed controls, independent review and held-out
five-loop measurements.
