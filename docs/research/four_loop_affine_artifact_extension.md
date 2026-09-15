# Contingent affine-domain extension for four-loop artifacts

## Status and decision boundary

This is a design investigation, not an implementation or closure claim. It
is contingent on the result of the actual **literal-unit-mass** four-loop
campaign. Earlier exploratory H runs retained a symbolic common mass and
must not be treated as measurements of the literal-unit input now being
run through the generic CLI.

The source-port artifact bridge currently authenticates rectangular integer
domains. An unsupported affine search candidate may be discarded when the
independently retained, replayed, descending coordinate rules and finite
terminals already prove a complete whole-sector cover. The candidate itself
is not published or counted as a verified identity. If that sufficient
subsystem closes the literal-unit campaign, this extension can be deferred.

If the literal-unit campaign instead encounters an indispensable affine
branch, the following is a proposed bounded extension. It must continue to
reject incomplete coverage, unsupported geometry, or failed exact replay.

## A genuinely infinite exceptional locus

One earlier H reverse-order investigation encountered the sector
`0010011001`, with zero-based index coordinates satisfying

```text
n0 = n1 = n3 = 0
n2 = n5 = n6 = n9 = 1
n4, n7, n8 <= 0
1 + n4 - 2*n7 = 0
```

The coupled equation is feasible. Its complete integer solution on these
remaining sign bounds is

```text
n7 = -t
n4 = -1 - 2*t
n8 = -u
t, u = 0, 1, 2, ...

n = (0, 0, 1, 0, -1-2*t, 1, 1, -t, -u, 1).
```

For example, `(0,0,1,0,-1,1,1,0,0,1)` and
`(0,0,1,0,-3,1,1,-1,-2,1)` belong to the locus. When an additional condition
fixes `n8 = 0`, one infinite ray remains. Without that condition it is a
two-dimensional lattice face. Neither a sign test nor integer divisibility
can justify discarding it or declaring its origin a universal terminal.

This example uses only input-derived equations. It must not become a
topology-specific production case.

## Existing Symbolica and RustRed services

The local dependency is Symbolica 3.0.0, patched to `vendor/symbolica`, with
Numerica provided by `vendor/symbolica/lib/numerica`. The investigation
checked the dependency wiring, native matrix/integer APIs, and public exact
equation-solving API and tests. Reference implementations are not copied
into RustRed by this proposal.

| Required operation | Existing implementation to reuse |
| --- | --- |
| Exact rational row reduction and primitive integer row normalization | `vendor/symbolica/lib/numerica/src/tensors/matrix.rs`: `Matrix::row_reduce`, `primitive_part`, solving and inversion |
| Exact substitution and coefficient arithmetic | Symbolica native polynomial `replace`, `replace_with_poly`, rational-polynomial arithmetic and factorization |
| Integer gcd and divisibility | `vendor/symbolica/lib/numerica/src/domains/integer.rs` |
| Exact equation solutions with retained validity conditions | `vendor/symbolica/src/solve.rs` and `solve/solution_set.rs`: `Atom::solve`, `SolveDomain::Integers`, `SolutionSet`, `SolveCoverage`, `SolutionCondition` |
| Case RREF, fixed face, exact restriction and equality implication | `crates/rustred-core/src/solver/case/affine.rs`: `AffineCase` |
| Integral and rational computational charts | `crates/rustred-core/src/solver/case/affine/chart.rs` |
| Conservative sign-bound impossibility checks | `crates/rustred-core/src/solver/case/affine/bounds.rs` |
| Shifted source construction with affine restriction | `crates/rustred-core/src/solver/instantiate.rs` |

No general public integer-polyhedron, Presburger, or arbitrary Boolean
inequality-coverage service was identified in this audit. Polynomial
Diophantine helpers, LLL lattice reduction and PSLQ integer-relation finding
are not substitutes for such a service. Symbolica's integer solve domain
can retain unresolved domain and positivity conditions;
`SolutionSet::is_empty()` can return `IncompleteCoverage`. A returned
parametric expression or branch count alone is not an integer-feasibility
certificate.

The existing comment in `case/affine/bounds.rs` referring specifically to
`InequalitiesNotSupported` predates the current public solve API. The
current API uses `UnsupportedProblem` and rejects general inequality and
Boolean inputs. This documentation observation does not require modifying
the vendored implementation.

## Computational charts are not integer domains

The authoritative domain consists of the original integer indices, their
sector signs, fixed coordinates and exact equality conditions. A finite
description of a rational computational chart does not make its domain
finite, nor guarantee that arbitrary integer free coordinates produce
integer dependent coordinates.

For `2*n4 - n7 = 0`, the rational chart `n4 = n7/2` may restrict coefficients
exactly, but odd integer `n7` does not give an allowed integral index `n4`.
Congruence conditions remain implicit in the original integer equality.
The particular chart `n4 = 2*n7 - 1` is integral and avoids this congruence
issue, but still describes infinitely many integral targets.

Likewise, an IBP's integral columns must retain their original index vectors.
Using a chart for coefficient substitution does not identify two distinct
shifted integrals. Ordinary sources must be translated before restriction:

```text
g(n) = 1+n4-2*n7
g(n+s) restricted to g(n)=0 is s4-2*s7, not automatically zero.
```

Recentring a rule on this affine case must preserve its equalities. The
existing `AffineCase::is_tangent` implements the needed displacement test.

## Core invariant: certify the guard partition

The proposed small extension avoids implementing a general integer
polyhedron solver. For an exact current domain `D` and a guard `g`, use

```text
D = (D intersect {g != 0}) union (D intersect {g = 0}).
```

A verified rule owns the nonzero part. Independently verified exceptional
rules must own the zero part. The equality holds for every integer point
without first solving a general feasibility problem. Overlapping child
domains are harmless for coverage; an omitted child is not.

For the concrete locus above, a parent rule valid off
`1+n4-2*n7=0` and an affine-target rule valid on that locus may jointly
cover the surrounding coordinate face. Their validity and every further
exceptional branch still require proof.

Multiple exceptional conjunctions must retain the exact union/conjunction
structure produced by existing guard extraction. Reuse the coefficient-
system treatment of the dimension parameter: a generic coefficient pole
such as `d-2*n4` must not be mistaken for an index-only exceptional
hypersurface. Rational functions of generic `d` and genuine integer-index
exceptional cases retain their existing separate contracts.

## Minimal implementation boundaries

1. **Source-port case evidence.** Extend the existing combined-original-
   domain evidence with required equality polynomials alongside its
   rectangular bounding carrier. An empty equality list preserves the
   existing coordinate path. The bounding box is a filter, not an affine
   coverage proof.

2. **Exact replay.** In `artifact/source_port/replay.rs` and `ordinary.rs`,
   admit the existing `Case::Affine` and pass its chart to the existing
   shifted-source constructor. Regenerate ordinary-source identities;
   do not trust search rows or omit affine constraints from their proof.

3. **Original-domain lowering.** Carry equality restrictions through
   `source_port/lower/verification.rs` and original-combination checking.
   Use Symbolica restriction to compare coefficients on the exact face.
   Keep denominator conditions before cancellation; an identically zero
   restricted denominator fails closed.

4. **Runtime membership.** Add exact required-equality checks to the
   source-port `RuleCell` path, after the cheap rectangular membership
   filter and before selection. Existing nonzero guards remain mandatory.
   An off-face target must never use a rule proved only on an affine face.

5. **Coverage certificate.** Keep `BoxCover` for coordinate-only coverage.
   For a remaining source-port box, reconstruct or validate a bounded
   guard-partition tree from retained rules and independently recomputed
   guard conditions. Each child is exactly the declared intersection of
   its parent. Each leaf must be covered by a replayed applicable rule,
   independently proved empty/zero, or a fully fixed explicit terminal.
   Positive-dimensional unresolved leaves and cycles prevent publication.
   Search queue exhaustion alone remains insufficient.

6. **Persistence.** Extend the existing source-port payload in
   `artifact/persistence/source_port.rs` and its `plans.rs` with exact case
   constraints and the necessary cover evidence. Cold loading rebuilds
   charts and repeats replay and partition verification. Do not persist an
   unchecked Boolean claim that an affine domain is covered.

These additions belong in the current source-port authority path; they do
not require rewriting unrelated foundry algorithms, topology matching, or
the IBP application engine. No topology names or hard-coded relations enter
these interfaces.

## Conservative descent first

Reuse the existing stronger rectangular descent proof wherever it
succeeds. For terms that vanish only on the affine face, apply native exact
coefficient restriction before zero testing. For ambiguous sign-partition
pieces, prove impossibility using a sound bound or return an unsupported
proof result. Do not discard pieces merely because sampled points miss
them.

This particular chart implies `n4 <= -1` directly from `n7 <= 0`. A narrow
exact interval pullback can exploit that fact without an LP solver.
General rational-chart inequality feasibility can remain unsupported until
an actual campaign requires it. Such conservatism may miss a valid rule;
it must never admit an invalid one.

## Required tests before publication

- Preserve the explicit two-parameter family and its `n8=0` ray; neither
  may become a single finite terminal.
- Prove the combined nonzero/zero guard partition covers its parent, and
  reject installation after removing a necessary exceptional child.
- Check affine-face membership at both near-origin and larger integral
  targets; check immediately adjacent off-face targets are rejected.
- Test translated coefficients before restriction using `g(n+s)` above.
- Test rational-chart congruences with `2*n4-n7=0`, including odd `n7`.
- Test genuinely impossible integer equalities, for example
  `2*n4-2*n7=1`, through native divisibility checks.
- Test intersecting and overlapping exceptional conjunctions without
  losing zero-domain branches or accepting a self-referential cover cycle.
- Mutate a persisted equality, guard, source translation or exceptional
  child; cold replay/coverage must reject each unsound payload.
- Test strict descent and zero-projection validation on affine faces,
  including sign-boundary pieces where a bounding hull is larger than the
  actual domain.
- Verify identical artifacts/reductions across supported worker counts and
  unchanged coordinate-path K1/K3/K6 behavior.

Implementation should be chosen only after the literal-unit campaign
identifies a surviving indispensable obstruction. An input-derived
regression for that actual obstruction should then accompany each change.
