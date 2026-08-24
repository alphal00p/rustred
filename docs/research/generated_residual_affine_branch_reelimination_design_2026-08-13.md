# Generated residual-affine branch re-elimination

This note fixes the next production seam after
`GeneratedResidualAffineBranchBoundRelationCompiler`.  It is a generic
LiteRed-style derivation step.  It must never contain a loop-count dispatch,
a named topology, or a precomputed recurrence.

## Authenticated inputs

The future compiler consumes an `IntegralFamily`, its exact
`ParametricCoefficientContext`, an `Arc<AffinePreparePointScheduleCertificate>`,
and the matching `Arc<ResidualAffineBranchGuardCompositionCertificate>`.
The residual branch is derived from `schedule.ordering().residual_branch()`;
it is not an independently supplied row source.  The branch in turn owns the
cover, live-leaf queue, discovery certificate, and generated IBP/LI row span.

The guard certificate must share the exact branch allocation.  Replaying the
schedule with family authority and replaying the guard certificate therefore
authenticates the complete source chain before any equation is retained.

## Expansion order

Rows are submitted in the same nesting used by LiteRed's prepare-point loop:

```text
for schedule layer, in increasing depth:
    for translation in the layer's persisted affine order:
        for generated source row, in row-span order:
            compile one branch-bound relation
```

This point-major/source-minor order is part of the certificate and replay.
Every witness records layer ordinal, depth, prepare-point ordinal, generated
source-row ordinal, translation, and one of the exact one-row outcomes.

An `EmptyBranch` outcome terminates the whole branch.  An `UnavailableRow`
skips only that expanded row and retains its reason.  If no rows remain, the
result is unresolved (`NoAvailableRows`), not a zero-sector or master proof.

### Current generated-row reachability

For the current canonical `GeneratedSymbolicRowSpan`, IBP/LI construction
uses only sums and products of `n_i + rho_i` with family-field coefficients.
Its coefficient denominators and source guards are therefore base-field-only;
the residual affine substitution changes index variables and cannot map such
a nonzero base polynomial to zero.  Consequently canonical generated rows do
not currently produce `UnavailableRow`.  A valid family has at least one loop
and hence at least one ordinary IBP row, so the same fact makes
`NoAvailableRows` unreachable on a consistent branch.  The explicit outcome
state-machine test is therefore the appropriate coverage of those generic
control paths until a broader row source can carry rational index-dependent
domains.

`EmptyBranch` remains semantically possible when collective affine equalities
annihilate a distinct Boolean nonzero guard.  The bounded authentic fixture
search found no such generated branch, so this path also retains direct
state-machine coverage rather than a fabricated topology or recurrence.

## Premises

Two premise classes must remain distinct:

- common Boolean-branch nonzero guards apply to the complete branch and are
  later pulled back through affine boundaries;
- row-local base assumptions belong to the particular compiled equation and
  must propagate through any pivot trace that uses that row.

Neither premise class is globally true.  Neither may be discarded when the
private `J(F(t)+q)` relation enters sparse elimination.

## Elimination ordering

The ordinary `ParametricElimination` anchor is an `i64` sample and is not an
ordering proof for a dependent affine start.  The shared sparse algebra must
instead receive a closed, preordered set of columns.  Columns are the exact
union of retained row supports, sorted easiest-first by
`AffineStartParametricEliminationOrdering::key_for_shift`, with `IndexShift`
as a deterministic tie-break.  The preordered core authenticates arity,
uniqueness, exact support equality, and an opaque ordering identity.  It does
not expose a public arbitrary comparator.

## Affine recentering

For an affine start

```text
F(t) = b + A t
```

and an eliminated pivot shift `p`, let `p_F` be the components of `p` at the
free positions.  Centering the private equation requires all three coupled
operations:

```text
b' = b - A p_F + p
coefficient/guard variables: t -> t - p_F
integral shifts:             q -> q - p
```

The global `ParametricPivotEquation::centered_relation` operation is not valid
for this private affine geometry.  A centered pivot may become a rule only if
an exact contiguous affine-case inventory contains the target start `b'` and
the persisted affine ordering proves strict descent there.

## Affine WhenBad

The global `WhenBadCompiler` branches on ambient coordinate hyperplanes and
cannot be reused unchanged.  For a candidate boundary `n_i = v`, the affine
compiler pulls it back to

```text
F_i(t) - v = 0.
```

It then substitutes that equality into the candidate numerator and all
index-dependent premises, splits numerator-zero from numerator-nonzero
children, leaves base-only assumptions outside index branching, and proves
uniform descent against the same affine-ordering identity.  Good children
enter coverage; bad children remain residual work.

## LiteRed correspondence

The controlling Mathematica sequence is `SolvejSector` in
`vendor/LiteRed2/Source/LiteRed2026.m`:

- generate `IBPLI` near line 2377;
- gather contiguous cases near line 2419;
- clear the equation database per case group near line 2430;
- enumerate prepare-point shells and submit identities near line 2471;
- eliminate near line 2481;
- recenter and select the exact target case near line 2484;
- run `WhenBad`, installing the good locus and retaining bad work near line
  2488;
- rebuild residual case groups near line 2522.

RustRed may use Symbolica-native sparse data structures and a different fast
kernel, but these semantics and proof boundaries are invariant.

## Validation order

1. Replay point-major/source-minor expansion against direct one-row compiles.
2. Force retained, unavailable, empty, and all-unavailable outcomes.
3. Verify row-local and common premises through multi-row pivot traces.
4. Compare persisted column order to independently generated affine keys.
5. Specialize sources and pivots at several valid free points and replay their
   exact linear combinations.
6. Test affine recentering with `F(t) = (3-t,t)` for same, adjacent, and absent
   target cases.
7. Test pulled-back boundaries, numerator cancellation, child requeueing, and
   target-local coverage.
8. Use generic one-loop and sunset fixtures first, then three-loop fixtures;
   all concrete families remain validation oracles only.
