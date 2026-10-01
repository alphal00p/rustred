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
This note implements none of those steps and claims no performance benefit.
