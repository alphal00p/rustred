# Dimensional recurrences as shortcut proposals, 2026-10-01

## Decision and scope

Dimension shifts are a legitimate additional construction method, not an
already installed RustRed reduction service or a demonstrated performance win.
The useful near-term question is whether they can propose a better **fixed-d**
relation, avoiding new dimension-labelled obligations in the runtime queue.
Do not expand this investigation into master-integral evaluation or change the
frozen four-/five-loop input, rule pool, production executable or lifecycle.

The current two-mass bubble test uses ordinary fixed-d Laporta reduction;
dimension-dependent coefficients do not make it a dimensional recurrence. The
native momentum-map compiler explicitly rejects differing family dimensions:
`crates/rustred-core/src/sector/symmetry/integral_transport/compile.rs:44` and
its `verified_denominator_identity_does_not_authorize_a_dimension_shift` test.
Integrating a massless subloop into epsilon-dependent powers is also a distinct
operation, not evidence of a generic native d-to-d+2 identity service.

## Vacuum normalization and paired identities

Let J_d(n) use Euclidean measure `product_i d^d k_i / pi^(Ld/2)`, a complete
affine denominator basis, no external momenta, and zero analytic power shifts.
Write A_i f(n)=n_i f(n+e_i), B_i f(n)=f(n-e_i). Let U be the first Symanzik
polynomial and P(D)=det(k_i dot k_j), expressed in that denominator basis.
For this convention the two relations are

```
J_(d-2) = U(A) J_d
J_(d+2) = 2^L / [d(d-1)...(d-L+1)] * P(B) J_d .
```

These are the vacuum specialization of Lee's normalized formulas, with the
empty external Gram determinant equal to one. Pseudo-Euclidean conventions
carry the corresponding signature factors; never silently transplant the
Euclidean mass/sign convention into native denominators.
[Lee, section 2, equations (2)–(8)](https://arxiv.org/html/0911.0252).

Tarasov derives the dimension-lowering operator from the parametric polynomial.
If constructing it through mass derivatives, introduce independent masses and
differentiate before specializing to equal masses; a derivative in one common
mass is not a replacement for the separate indexed operators.
[Tarasov, section 2, equation (18)](https://arxiv.org/html/hep-th/9606018).

Lee also gives a graph-independent determinant construction of U(A), using the
linear coefficients of the denominators in scalar products. Its diagonal/off-
diagonal factors agree with constructing U from the loop quadratic-form matrix.
This is the relevant algebraic portion of that paper; its subsequent DRA
boundary/asymptotic evaluation procedure is outside the present task.
[Lee, section 3, equations (8)–(10)](https://arxiv.org/html/1007.2256).

Directly composing the two identities gives the following fixed-d candidates,
where `d_down_L = d(d-1)...(d-L+1)`:

```
[2^L U(A) P(B) - d_down_L] J_d = 0
[2^L P(B) U(A) - (d-2)_down_L] J_d = 0 .
```

These compositions are an explicit inference from the displayed operators,
not an assertion that a new native proof object exists. The two operator orders
must not be exchanged: `[A_i,B_i]=1`, and `A_i^r` contributes the rising factorial
`n_i(n_i+1)...(n_i+r-1)`, not `n_i^r`. Coefficient translation must therefore
occur before collecting equal shifts. The signature factors cancel in the
paired relation, but P still depends on the actual denominator and mass signs.

For L=1 and D=k^2+m^2, U=A and P=B-m^2. The first composition reduces to
`m^2 n J_d(n+1) = (n-d/2) J_d(n)`, precisely the ordinary Euclidean tadpole
IBP. This sanity check supports the normalization; it does not show a new
independent relation or any four-/five-loop speedup. Native D=k^2-m^2 has the
correspondingly different mass sign.

### Five-loop epsilon warning

At L>=5 the up-shift prefactor contains d-4. With d=4-2 epsilon it can introduce
a 1/epsilon pole and require additional Laurent depth. Never impose a strictly
four-dimensional Gram determinant identity or discard an evanescent Gram
numerator at dimensionally regulated d. Keeping the fixed-d composition
polynomial avoids dividing by that prefactor at this stage; solving for a
particular pivot can reintroduce poles. All such denominators and exceptional
specializations remain part of the authority and cost accounting.

## Existing primitives and the missing authority boundary

### Related constrained-identity literature

The Gram-determinant literature offers a different use of this geometry:
construct logarithmic vector fields that avoid changing dimension, then impose
additional divisibility constraints to avoid raised propagator powers. The
Laplace-expansion construction gives low-degree generators; its completeness
theorem concerns that algebraic module, not termination of RustRed's routed
domain worklist. [Böhm et al., section IV](https://arxiv.org/html/1712.09737)

Smith and Zeng explicitly describe polynomial-vector IBPs as combinations of
ordinary IBPs at shifted seeds. Our finite forbidden-column probe is therefore
a small, specialized row-span test inspired by the same goal, not an
implementation of a complete syzygy-module intersection. Restricting total
absolute index degree also differs from forbidding every raised denominator.
[Smith and Zeng, section 2.3](https://arxiv.org/html/2507.11140v2)

The diamond rule suggests summing repeated recurrences into shortcuts, but its
published construction requires massless lower and spectator lines. It cannot
be copied unchanged into the equal-mass vacuum campaign. A massive analogue
would require its own derived identity and cost test.
[Ruijl, Ueda and Vermaseren, section 2](https://arxiv.org/html/1504.08258)

Existing public native components provide most polynomial ingredients:

- `family::symanzik::SymanzikPolynomials::try_from_family_with_limits`, followed
  by `u()`, constructs authenticated U using Symbolica determinant arithmetic.
- `IntegralFamily::coordinate_index` and `scalar_product_expansion` give the
  exact affine scalar-product entries needed for P; Symbolica should own its
  determinant and coefficient collection as well.
- `ParametricIbpGenerator` and `solver::SourceSystem::from_family` regenerate
  ordinary exact sources. The current source replay can verify their retained
  translated linear combinations.

In particular, `BoundOwnerSearch::replay_overlay_rules` regenerates ordinary
IBPs with the bound family, saved root and order. It does **not** authenticate
an arbitrary caller-supplied dimensional identity merely because its result
has fixed-d integral labels. Nor does it prove descent, recursive RHS closure,
case coverage or that residuals are allowed terminals.

The economical publication route is to reconstruct a prospective fixed-d
shortcut as an exact combination of ordinary translated sources, retaining
that provenance and independently checking its complete weighted product.
There is a concrete API seam: `source_port/replay.rs:189` accepts a regenerated
forward-pivot row only when its canonical target and RHS equal the candidate.
It is not a general arbitrary-row-span membership entry point. Concatenating
parent and child traces therefore need not make an exact composed shortcut
pass unchanged `replay_overlay_rules`. Existing original-weight multiplication
machinery can inform a scoped extension, or each original replay can be joined
by a separately checked arithmetic-composition and guard-pullback certificate;
neither is silently supplied by today's entry point.

With such an ordinary-source certificate, dimensional recurrence has served as
a structured source/pivot proposal or conditioning method, not a new
completeness authority. If no certificate is obtained, failure of a bounded
search is inconclusive; publication through dimensional identities instead
requires an explicitly reviewed dimensional-source proof type. Relabelling a
custom SourceSystem row as an ordinary source would bypass the authority
boundary.

## Why a dense global shift can lose

For a vacuum monomial U_alpha with |alpha|=L and a Gram monomial P_beta with
|beta|<=L, the first composite produces shift alpha-beta. Before pivot choice
and recentering, its total signed-power change is
`Delta D = |alpha|-|beta| >= 0`. Thus it is not automatically a lowering rule
for the original J(n). Individual terms can add dots or numerator powers,
cross support boundaries, and create guards or routing work. A proposed pivot
must still satisfy the actual saved comparator on its entire proved case.

Naively multiplying the two sparse polynomials forms up to their term-count
product before exact coalescing; each shift has L1 size at most 2L. Native
measurement must count retained distinct shifts, coefficient size, source
support and guards, not advertise the small determinant notation as a small
rule. A graph-specific bubble/subgraph relation can be much sparser.

Reference FMFT really uses G(dp,...)->G(dp-2,...) relations in `drrG`, and tensor
reduction invokes that path. However, its published method also produces
connector-momentum denominators and additional mass scales; larger numerator
substitutions can become less efficient than index recurrences. This is an
important counterexample to treating dimension shifts as free simplification.
[FMFT, sections 2.2–2.3](https://arxiv.org/html/1707.01710v2).

## Smallest useful experiment, not an execution grant

First perform a bounded Symbolica-only structural test on one actual costly
four-loop case, with the one-loop formula solely as a normalization control.
Construct U and P using the admitted family's native primitives, compose with
the correct shifted coefficients, and seek an ordinary-source certificate for
one useful oriented relation. Retain every nonzero RHS term, original and
pulled-back guard, and unresolved target. No all-RHS-already-closed condition is
required: productive new successors are legitimate.

Stop before timing if the candidate is unchanged, cannot be replayed, has no
admissible pivot on the intended case, or merely moves work into a larger
unqualified shifted-dimension family. A genuinely smaller/reusable successor
structure warrants a separately audited owner proposal with explicit priority
and fallback outside its case. Only then compare complete matched four-loop
work and a declared finite five-loop query/control, including preparation,
extra algebra, exceptional branches, routing and cold successor coverage.
The bounded structural stages below have now been measured; no ordinary-source
certificate, reusable rule or performance benefit has been established.

## Native structural and fixed-point measurements

The ignored cached-rlib adapters at
`TMP/postlaunch-20261001/rule-quality-portfolio/dimensional-structure-v1/`
and `dimensional-paired-v1/` use the authenticated saved four-loop owner481
family, public native Symanzik/scalar-product services, and Symbolica's matrix
determinant and exact coefficient arithmetic. No custom CAS, coefficient-string
parsing, full engine build or production change was used. The separate L1
control with native denominator `D=k²−1` gives exactly
`(2n−d) I(n)+2n I(n+1)=0` for both operator orders.

For the actual four-loop family U has 132 degree-four monomials, and P has
395 monomials of degrees zero through four: 52,140 uncollected products.
Every U/P coefficient was checked dimension independent. Both fixed-d
compositions were then collected at the same fixed-index target as the
constrained-source diagnostic, `[-1,1,1,2,1,0,0,0,-1,2]`. Here support=5,
A=7, R=2, D=A−R=5, F=A+R=9 and E=F−support=4. This is an
owner-anchor point, not a point in the original physical owner481 D7/D8 rows.
The native saved
`rustred.spired-uncut-sector-order.v1` comparator classifies all retained terms;
none were discarded for lying outside the original support.

| Fixed-point quantity | U(A)P(B) | P(B)U(A) |
| --- | ---: | ---: |
| Products considered | 52,140 | 52,140 |
| Nonzero product contributions | 8,295 | 10,425 |
| Exactly zero collected shifts | 224 | 249 |
| Nonzero equation terms, including target | 4,157 | 4,195 |
| Harder than target | 2,224 | 2,279 |
| Simpler than target | 1,932 | 1,915 |
| Terms increasing F | 3,347 | 3,406 |
| Terms increasing R | 2,741 | 2,798 |
| Terms with different support | 1,758 | 1,741 |

The target coefficient is nonzero and quartic in d in each row, but the target
is **not leading** in either. Maximum coefficient numerator/denominator degrees
in d are 4/0. The rows share 4,021 nonzero shifts, of which 3,061 have equal
native coefficients and 960 differ; another 136/174 shifts occur only in
UP/PU. No division by the target coefficient or pivot solve was performed.
These are complete equation rows, not usable oriented RHS rules.

Both diagnostics passed their owned lifecycle and drained all groups. The
structural compile-plus-probe charged 26.385948s; the paired extension charged
28.395356s (25.494117s compilation plus 2.901239s probe invocation), with
1.182GB peak sampled compile RSS and 94.5MB probe RSS. These numbers document
CPU32/W1, 1.9GB and cumulative300s resource compliance only. The adapters use
opt-level0 orchestration with optimized cached dependencies and are explicitly
**not runtime benchmarks or performance comparisons**.

Decision: park this global paired-dimensional shortcut at the tested target:
its complete rows are dense and not oriented to reduce that target. This is a
local negative structural screen, not a generic impossibility claim for
dimension shifts or localized subgraph recurrences. No ordinary-source
certificate, reusable rule, basis/terminal change, recursive walk or work-gain
claim follows. Full raw rows and receipts are in each adapter's `probe/`, with
compact measured tables and pinned source/protocol hashes in `RESULTS.md`.
