# Reusing sector programs through verified momentum maps

Status: independently reviewed design, **not implemented**. This is a route to
avoiding repeated sector search, not terminal minimization or a closure proof.
Complete the observed exceptional-geometry fixes and their end-to-end reruns
before starting this implementation.

## Motivation and existing evidence

The frozen five-loop input census has 8,246 distinct labelled sectors not
proved zero by its screening, but only 67 graph classes. Existing offline witnesses route every labelled
sector to a representative by an exactly verified unimodular loop-momentum
map. The recorded native verification, independently audited, checks 57,890
active-line identities.
These witnesses are not generally permutations of the complete denominator
basis: inactive numerator coordinates can transform into affine combinations.

At the non-atomic September 21, 11:12 UTC checkpoint snapshot, 5,404 labelled
sectors are saved, representing 65 classes. Only 29 literal published
representative masks are saved; other classes have differently labelled
owners. Choosing a saved owner per class could avoid much remaining duplicate
search. It does not establish coverage of arbitrary positive powers or of
all recursive descendants. The separate finite-retention campaigns must not
be silently merged into the broad-search campaign's single-policy artifact.

## Route concrete integral keys, not every symbolic rule

Keep each owner's program, guards and ordering in its original coordinates.
Transform requested integral keys into exact finite combinations in those
coordinates, then use the existing guarded rule application. This avoids
copying and re-expressing every symbolic rule under every routing.

For the first implementation, admit only verified maps with:

- authenticated source/target families and no unresolved conditions;
- unit Jacobian and zero analytic power shifts;
- a unit-coefficient bijection of source active denominators onto the owner's
  active denominators;
- rational-constant affine images for the remaining denominator coordinates.

Compose source-to-representative and owner-to-representative witnesses with
Symbolica matrix arithmetic, then reverify the composed map with the existing
`sector::symmetry::verify`. A stored verification boolean is not a substitute
for an executable verified map. Unsupported maps remain explicit errors.

## Numerators, pinches and rank

Let the source's positive powers be arbitrary integers `a_i`, and its inactive
powers be `-b_j`, with `b_j >= 0` and `sum b_j <= R`. Map positive powers by
the active bijection, giving a nonnegative base vector `a'`. Expand only the
finite numerator product

```text
product_j (c_j + sum_k M_jk D'_k)^b_j.
```

Symbolica already provides the powers, multiplication and coalescing. Each
resulting monomial has exponent vector `e >= 0` with `sum e <= R`, and its
integral key is `a' - e`. Thus its numerator rank is at most R, its positive
dot excess cannot increase, and its active support cannot grow. Propagator
cancellation can produce lower sectors and must be preserved.

For an algebraic illustration, suppose an admitted map has active images
`D0=D'0`, `D1=D'1` and numerator image `D2=D'2+D'0-D'1+1`. Then

```text
I_source(a,b,-1)
  = I_owner(a,b,-1) + I_owner(a-1,b,0)
    - I_owner(a,b-1,0) + I_owner(a,b,0).
```

At `a=1` or `b=1`, some terms are pinched. This is an illustration of an
already-admitted affine denominator map, not a claim that arbitrary affine
matrices are realizable loop-momentum transformations.

This proof concerns entry transport only. IBP descendants can exceed the
entry rank R; they must remain reachable and may not be clipped or rejected
by reapplying the public entry check at every owner transition.

The actual uncut ordering does not itself guarantee rank nonincrease. Within
one support it first compares total corner distance `D+R`, where D is positive
dot excess. For example, `(3,1,-10)` has D=2, R=10, whereas `(1,1,-11)` has
D=0, R=11 and strictly descends. This is an ordering counterexample, not a
claim that a saved IBP contains that edge. Repeated hypothetical shifts
`(-2,0,-1)` could spend arbitrary input dots to grow numerator rank while every
individual path still terminates. Rank-10 input admission cannot justify
discarding those children.

A cheap sufficient check, to investigate on actual saved programs, is that
every same-support RHS shift has nonnegative sum over inactive source axes:
its rank change is exactly minus that sum. Combined with strict support
containment, this would bound rank growth to the finitely many pinch events.
For constant shift vector delta and active set A, a conservative pinch bound is

```text
B(delta,A) = sum_(i inactive) max(-delta_i,0)
           + sum_(i active)   max(-1-delta_i,0).
```

If every same-support term passes the check, a finite library with maximum B
and p initial active lines has the sufficient envelope `R_entry+p*B`.
No such universal property has been checked for the current saved programs.
A failing conservative check would mean "not proved by this criterion", not
proof of an actual unbounded dependency or permission to invent terminals.

## A sufficient cycle-free application schedule

1. Select one immutable owner per class and route a non-owner entry to it once.
2. Apply same-support IBPs in that owner's original exact ordering.
3. Route to another owner only after the active-line count strictly decreases.
4. Apply the same rule recursively to every induced pinch.

The well-founded measure is active-line count, then a one-way routing phase,
then the owner's exact descending integral order. Finite numerator expansion
does not itself introduce an infinite branch.

Enforce active-support containment. The current generic integral order can
permit a same-count move to a different labelled support; the proposed narrow
scheduler must reject that transition rather than silently invoking routing.
A more general transition policy would require another termination argument.
No decoded-program census has yet established that every saved program meets
this additional admission condition.

## Reuse existing components

- `VerifiedMap` already owns exact denominator images, conditions and Jacobian.
- The native `multi_affine_expansion` helper already implements bounded
  numerator expansion. Move it mechanically to a lower shared family module
  if needed; do not implement a second polynomial engine.
- The existing candidate reducer owns exact guards, coefficient specialization,
  strict descent, zero handling and memoization. Reuse its internal one-step
  operation and successor checks rather than recursively calling its public
  entry API with reset budgets.
- The complete-checkpoint loader must retain its completeness requirement.
  A separately named selective owner-library loader can admit chosen shards
  against their original manifests, sources, orderings, rank and policy.

Use shared immutable family/map/program owners. One aggregate request budget
must cover routing, expansion, application, memoization and the missing
dependency frontier. Missing owners, unsupported maps, exhausted budgets and
uncovered children remain distinct incomplete results.

## Risks and acceptance

Sparse expansion can still be large: the unrestricted count of monomials of
degree at most R in 15 variables is 3,268,760 at R10 and 3,247,943,160 at R20.
Actual verified maps may be much sparser; these counts are bounds, not measured
workloads. Keep existing native-expansion resource checks and explicit limits.

Test identity/permutation agreement, non-involutive map composition, multiple
numerator factors, cancellation, pinches, arbitrary positive powers, rejected
foreign/unsupported maps, same-count routing cycles, above-R internal children,
shared budgets and deterministic results. Old shards must remain unchanged.

Even a successful finite dependency trace establishes only those requested
reductions. Full rank-bounded family closure additionally needs parametric
coverage of the positive-power directions and their complete successor set.
Neither 67 saved owners nor master-count agreement proves this.

Local design evidence: `TMP/full-routing-owner-audit.84kF2V/RECOMMENDATION.md`
and `TMP/verified-key-transporter.3AmZKz/API_DESIGN.md`. No reference-only source
code or unpublished PDF content is included in this design.
