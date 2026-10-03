# Profile-guided protected-source experiments

October 3, 2026. Read-only mechanism study for the
[optimization plan](../../PROFILE_GUIDED_RULE_OPTIMIZATION_PLAN.md), informed by
the [bounded cost profile](five_loop_rule_cost_profile_2026-10-03.md).
This document authorizes no implementation, native job or campaign change.
The first two experiments and nonradial appendix preserve their proposal-time
reasoning. Subsequent native outcomes are recorded in the
[independent audit](profile_guided_rule_optimizer_audit_2026-10-03.md): the
secondary75 projection gives a bounded negative, while the optional gradient
toy and broader radial charts pass their separate native proofs. Those results
do not convert untested proposals below into performance claims. The final
finite multi-loop kernel section remains a new prospective mechanism.

## Selective protection, not a blanket ban on raised powers

The principled extension is logarithmic tangency on a selected protected set S:

```text
sum_l V_l · derivative_l(D_j) = h_j D_j,  j in S.
```

For polynomial V and h, differentiating D_j^(-n_j) does not raise its power.
Exact tangency V(D_j)=0 is stronger. Protecting nominated transfer destinations
is less restrictive than protecting every active denominator. Neither condition
establishes useful target orientation, recurrence coverage or lower worklist cost.

Smith–Zeng formulate the constraint as a polynomial-module kernel containing
ordinary derivative contractions and a -D_j diagonal (equations 12–17).
Their sector definition fixes positive powers, not just positive support.
This supports selective source design; their target-neighborhood heuristic
does not establish a five-loop termination bound.
[Primary paper, sections 2.2–2.3](https://arxiv.org/html/2507.11140v2#S2.SS3).

Böhm et al. give complete generators for the dimension-preserving
Gram-determinant logarithmic module. Preventing raised propagator powers adds
a divisibility-module intersection (equations 4.8–4.10). Completeness of the
first module is not completeness of our constrained reduction or worklist.
[Primary paper, section 4.1](https://arxiv.org/html/1712.09737v2#S4.SS1).

## Existing native services and limits

`IntegralFamily::derivative_contraction` and `scalar_product_expansion` in
[`family/kinematics.rs`](../../crates/rustred-core/src/family/kinematics.rs)
provide authenticated affine denominator-basis entries.
`identity::TangentSourcePlan` constructs signed 2×2 minors for exactly two
protected denominators and three contraction directions, verifies exact zero
tangency, and materializes complete translated ordinary sources and conditions.
It is not an arbitrary selected-set logarithmic-module solver.

The public entrypoints are:

- `TangentSourcePlan::try_new(&family, &spec, limits)` in
  [`construction.rs`](../../crates/rustred-core/src/identity/tangent/construction.rs).
- `.contributions()` and `WeightedTangentSource::{request,row_id,weight}` in
  [`model.rs`](../../crates/rustred-core/src/identity/tangent/model.rs).
- `.materialize(&generator, &completed, translated_limits)` in
  [`materialize.rs`](../../crates/rustred-core/src/identity/tangent/materialize.rs).
  Its result exposes `sources`, `weights`, `product` and `conditions`.

The current research `geometry_tangent.rs` requires exactly one dependent
radial active denominator. Its incidence classifier is not a general tangent
constructor. `projected_bank.rs` regenerates specified ordinary RowIds and
translations, uses Symbolica `SparseRowReducer`, and compares F-only with F+H.
F contains every non-target key not lower under the unchanged saved order;
H is a nominated finite set of lower transfer keys. Recovered weights multiply
all original source columns to check the full RHS. This is a finite source-span
experiment, not a complete polynomial syzygy computation.

A targeted current-vendor search found polynomial arithmetic/F4, dense solves
and sparse row reduction, but no public free-module syzygy/module-Gröbner API.
This is a source-search result, not a claim about every Symbolica release.
Ideal F4 must not be treated as a provenance-preserving module solver.

## Prior negative evidence

The earlier single-loop affine-coefficient logarithmic ansatz for two massive
denominators was analytically zero; adding a third denominator cannot rescue
that ansatz. Higher-degree minors or multiple differentiated loops are different
tests, not permission for automatic degree escalation. A source-proved broad
tangent priority increased complete four-loop domains from 26,025 to 26,659
(+2.44%). The fixed 752-row Laporta bank did not pivot the 24-term hard block;
17 isolated target solves and symmetry quotienting did not prove completion.
Raw/GCD/component preconditioning also produced no structural benefit on the
tested 16-row fixture. See the
[study workboard](../../CODEX_NEW_RULES_STUDY.md) and
[prior tangent/Laporta study](global_degree_order_literature_2026-10-01.md).

## Test 1: finite projection after complete mixed native tails

Use the two frozen training mixed points, unchanged targets and saved orders.
Only after guarded inspection passes all completeness/refusal gates, derive
H1/H2 under the frozen `profiles/mixed-H-policy.json`: D1-increasing lower keys
plus every same-owner unit dot redistribution. Neither category is yet measured
expensive. Keep empty sets; do not guess replacement keys or infer weights from
formatted expressions.

Preregister one 75-row bank per point: all 25 native ordinary RowIds at centers
`{0,+e_D6,+e_D14}` (physical one-based D labels; zero-based axes 5 and 13).
This extends the nominated 15 k1-only views to all differentiated loops without
a seed grid. Compare F-only and F+H on the identical bank and common order.
Retain full coalesced tails, source/pivot guards, missing-H keys, weight support
and native exact replay. Proposed limits are 75 rows, 4,096 physical columns,
50,000 input matrix nonzeros and 60 seconds on one CPU. Root must admit the
limits before execution; no automatic bank or degree enlargement.

The existing runtime interface is
`TMP/rule-optimizer-20261003/candidates/projected-bank-build-v1/projected-bank`
with `probe OWNER.rrbin REQUEST.json`, schema
`rustred.projected-source-bank.v1`. Revalidate the successful-build receipt and
binary pin before an approved run. An input/guard receipt, not new algebra, is
required.

No-go outcomes are no target pivot, source/guard refusal, cap exhaustion,
retained H or a non-descending full tail. If F-only already avoids H, the added
constraint is vacuous. A clean row is not a cost win: unchanged completed shared
cohorts must establish that separately. A bank miss is not a master-integral claim.

## Test 2: native two-protected constructor

At the frozen primary mixed point, family geometry nominates differentiated k2.
Its dependent active denominators are D10=(k2-k4)^2-1 and D11=(k2-k5)^2-1;
active D1/D3/D5/D15 are independent. The nominated API spec is
`differentiated_loop=1`, `protected_denominators=[9,10]`,
`contractions=[Loop(1),Loop(3),Loop(4)]`, `recenter=+e_D14`, `multiplier=None`.
The API indices are zero-based. This is an independently reviewed geometric
nomination, not an authenticated source row or a descent proof.

Choosing k2 makes D6=(k1-k3)^2-1 and D13 spectators, trading the k1
two-dependent-numerator problem for two-active-denominator tangency. It does
not solve both problems simultaneously or imply cheapness. Retain D14 and the
inactive D2/D9 derivative terms before specialization: point zeros do not
justify dropping them on a free chart. Preserve the full product rule.

For a component monomial c*D^alpha, the ordinary-source offset is
`recenter-alpha`. Quadratic minors can therefore require
`+e_D14-e_i-e_j` or `+e_D14-2e_i`, besides constant/linear shifts.
Use the native plan's actual contributions and RowIds; do not guess weights.
The 75-row bank of Test 1 is **not assumed to contain these translations**.
Proposed limits are emitted-vector degree2,128 translated contributions,
10,000 original product terms, one point and60 seconds. The native tangency
check multiplies a degree1 derivative by a degree2 minor, so its intermediate
polynomial allowance must permit degree3 without permitting a degree3 emitted
vector or an optional multiplier. Reject a zero/degenerate vector,
unavailable target pivot, new source poles, non-lower tails or resource refusal.

No current driver directly admits this mixed-point two-protected request:

- `TMP/postlaunch-20261001/global-degree-order/local-tangent-source-v1/tangent-r3`
  accepts `OWNER REQUEST`, schema `automatic-local-tangent-source-v1`, arity
  10/15 and protected/direction indices, but requires other inactive powers zero.
  It rejects this point's D6=-2 and D13=-1; do not alter the point to fit it.
- `TMP/postlaunch-20261002/tangent-integration/owner-pilot-v1/owner-export-r1`
  is arity-10 only; sibling `after-baseline-owner-v1/owner-export-r1` also pins
  the previous 46-term proof.
- The current projected producer accepts explicit ordinary rows/offsets; the
  prescribed producer accepts rational weights; the geometry branch is
  single-radial-active only. The implementer confirmed no two-protected mode.

A future approved adapter could use native `contributions`/`materialize` and
the existing source/proof boundary, with a fresh compile. No new CAS kernel or
implementation is proposed here.

## Appendix: routing can hide a one-active radial geometry

A loop's sole dependent active denominator need not be radial in the saved
momentum coordinates. For example, H1 has D7=(k1-k4)^2-1 as its sole k1-dependent
active denominator. This explains a classifier limitation only: H1 is held out,
so it supplies no choice of p, source centers, fitted guards or new test target.

An elementary polynomial nomination for one protected denominator P is

```text
g = derivative_ki(P),       V = (g·g) p - (g·p) g,
g·V = (g·g)(g·p) - (g·p)(g·g) = 0.
```

For P=D7, g=2(k1-k4); no division by g² or square root is needed. For quadratic
P and p a fixed base-field linear combination of family momenta, g is also a
linear combination and the scalar-product coefficients of V can be expanded
with the existing native family service. Their coefficients are affine in
denominator variables; a more elaborate polynomial p could increase the degree.
Retain mass/external-invariant constants and inverse-basis guards; the native
denominator basis must be invertible and bound to the same family.
All differentiated coefficients, dependent numerators and source conditions
must survive the complete native product. Zero/degenerate nominations, missing
pivots, new tails and failure under the unchanged common order remain possible.
Tangency alone does not select a useful p or establish a cheaper recurrence.

The public `TangentSourcePlan` exposes no one-protected mode: its spec fixes
two distinct existing denominators and three distinct directions. Duplicate
denominators are refused; pairing P with a ki-independent denominator makes
the second matrix row zero and all 2×2 minors vanish, giving
`DegenerateProtectedSystem`. Choosing a second dependent inactive denominator
would impose an additional restriction, not the general one-protected projector.
The input/degeneracy tests are in
[`identity/tangent/tests.rs`](../../crates/rustred-core/src/identity/tangent/tests.rs).
This audit identifies a future nomination-adapter question, not a need for a
new algebra primitive or permission to weaken the current radial guard.

Finally, writing k1'=k1-k4 is a change of **momentum** coordinates. It is not an
affine translation of integral **indices**, not the source offset
`recenter-alpha`, and not authority to remap saved domains or common order.
If used operationally, family/denominator/owner binding would need its own native
proof. The proposed vector can instead be expressed in the original coordinates.

## Evidence boundary

The measured rule19 tail motivated protecting D10/D11, but its cost cannot be
assigned to other mixed rules. All source-bank tests are training experiments.
Heldouts must not select weights, centers, protected axes or guards. Preserve
ordinary fallback, common order, source conditions and every unprojected tail.
Do not generalize local C19 savings to unresolved broader scopes.
Compact discovery inputs, the frozen H policy and the original research note
are under ignored `TMP/rule-optimizer-20261003/profiles/`; this tracked document
contains no campaign record payloads.

## Future bounded multi-loop logarithmic kernel

This is a proposal, not an implemented source service or successful pilot.
The newly sampled owner011101110111000 has nine active denominator axes;
its native loop0 incidence has three dependent active axes. Other-loop
incidence must be authenticated before calling the whole sector trivalent.
Neither the early isolated-loop rule's validity nor its C19 cost improvement
generalizes to this owner.

For each ordinary direction G_i, nominate a denominator polynomial P_i of
degree at most k and impose `sum_i P_i*G_i(D_j)=h_j*D_j` for every protected j.
This is the logarithmic no-raised-power constraint in
[Smith and Zeng, section2.3, equations11–17](https://arxiv.org/html/2507.11140v2#S2.SS3),
restricted to a finite degree, not their full module-syzygy computation.
Because D_j is an independent native denominator variable, the finite
constraint can be assembled by equating coefficients after D_j=0. This
eliminates h_j without polynomial division. Protecting all nine active axes
is different from protecting a selected three-axis subset.

For15 denominator variables and25 ordinary directions, the structural counts
before sparsity are:

| Maximum weight degree | Coefficient unknowns | Constraints for nine protected axes, at most |
| --- | ---: | ---: |
| 0 | 25 | 135 |
| 1 | 400 | 1,080 |
| 2 | 3,400 | 6,120 |

The counts are `25*binomial(15+k,k)` and
`9*binomial(15+k,k+1)`. Each native derivative contraction is affine in the
denominator basis. Symbolica owns polynomial arithmetic and sparse reduction;
finite monomial enumeration and coefficient-matrix assembly are the proposed
adapter work. Existing native ingredients are `derivative_contraction`,
`MultivariatePolynomial<RationalPolynomialField<...>,...>` as used in
[`tangent/polynomial.rs`](../../crates/rustred-core/src/identity/tangent/polynomial.rs),
and `SparseRowReducer::{add_row,u,l,pivots}` with augmented identity columns
as already used by `projected_bank.rs`. There is no existing public generic
free-module kernel wrapper, and ideal F4 is not a substitute for one.

The smallest falsifiable first test freezes one original sampled owner66
target and its common order, then examines degrees0 and1 with all25 directions
and all nine active protections. Record coefficient-kernel dimension, complete
original-source image rank, all guards and the actual target-pivot result.
Global rotational/Lorentz combinations can have a nonzero coefficient kernel
but an identically zero full source product; polynomial source multiples can
also be useless. Neither counts as a reduction rule. Require elimination of
every non-lower physical column F and exact full source replay, followed by an
explicit parametric chart proof if a useful point circuit exists. This reuses
the saved order rather than replacing it with no-dot heuristics.

A zero kernel, zero full-source image, missing target pivot or bounded refusal
is a valid negative. Degree2 is a separate3,400-unknown resource decision, not
automatic escalation. The prior one-loop affine negative does not rule out
joint multi-loop degree1 weights, but it remains negative evidence against
blind ansatz repetition. Completed shared downstream cohorts, not kernel size
or local tail length, would eventually decide whether an admitted rule helps.
