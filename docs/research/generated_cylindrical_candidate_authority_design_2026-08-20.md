# Generated cylindrical pivot authority

Date: 2026-08-20

Status: implementation contract for the next generic solver slice.

## Purpose

`GeneratedCylindricalPersistentEliminationCertificate` authenticates exact
preordered pivots without choosing a representative integer point.  It does
not, by itself, prove that every pivot is a global identity.  The next layer
must preserve the distinction between an empty source assignment and a
partially specialized residual cylinder before recentering or compiling
`WhenBad`.

No loop count, topology name, expected recurrence, or master label is an input
to this layer.

## Authority split

For every retained pivot, compile exactly one of two proof-bearing outcomes:

```text
source assignment is empty
    -> global parametric candidate

source assignment is nonempty
    -> equality-locus-bound candidate
```

This is a semantic split, not a heuristic.  A nonempty assignment may never
be replaced by a sector corner or any other concrete anchor, and there is no
conversion from the locus-bound arm to the global arm.

The global arm retains the cylindrical persistent-elimination certificate,
pivot ordinal, original pivot, centered unit relation, source sector, exact
ordering identity and policy, complete source/base assumptions, limits, and
statistics.  Its source rows are globally valid because partial specialization
fixed no index coordinate.

The locus-bound arm retains the same derivation plus the centered sparse
assignment.  If the source assignment contains `n_i=c_i` and the uncentered
pivot shift is `s`, the centered rule is valid on

```text
n_i = c_i + s_i
```

for every fixed position.  Checked `i64` overflow is a typed compilation
error.  This is the same translation law already used by
`GeneratedPartialReeliminationCertificate`.

## The global source cannot depend on an anchored residual search

An empty-assignment arm is useful only if RustRed can construct it before an
anchored candidate has already partitioned the sector.  For example, the
current depth-zero active-sector tadpole discovery finds an anchored dot rule
and leaves only the exceptional coordinate assignment `n=1` in its live-leaf
queue.  Re-eliminating that residual can produce only a locus-bound result; it
cannot be cited as an independent derivation of the global tadpole recurrence.

The generic solver therefore needs two authenticated cylindrical start
origins:

```text
sector root + empty PartialIndexAssignment + fresh generated row span
    -> eligible source for global candidates

replayed live residual + extracted PartialIndexAssignment
    -> global only when that assignment is genuinely empty;
       otherwise locus-bound
```

The root origin must bind the family, parametric context, sector, restrictions,
power-shift policy, generated IBP/LI row-span identity, ordering policy, and
prepare-point schedule.  It must not obtain any of those fields by erasing the
anchor from a legacy `GeneratedSectorDiscoveryCertificate`.  An inactive or
unsupported residual with an accidentally empty assignment is a useful schema
test, but is not the one-loop global-recurrence acceptance test.

## Base-field assumptions are inseparable

`PartialParametricRelationSpecialization` stores base-field nonzero
assumptions separately from `relation_for_bound_reelimination()`.  Therefore
the persistent elimination/candidate boundary must retain every assumption
with its expanded-row and within-row ordinal, exact polynomial, complete
origin set, and context-relative typed manifest.  Retaining only the
specialized relation is insufficient.

Assumptions constant in the remaining index field do not become structural
case predicates.  They remain formal `K=Q(theta)` domain obligations attached
to either candidate arm and are specialized/returned with every concrete
application.

## Ordering authority

Pivot selection is authenticated by
`CylindricalParametricEliminationOrdering`; runtime descent is proved under
its persisted `IntegralOrderingPolicy`.  The candidate binding must therefore
be versioned over an ordering-authority enum rather than pretending every
candidate has `ParametricEliminationOrdering::anchor()`:

```text
AnchoredV1 { anchored-order manifest, discovery point }
CylindricalV1 { cylindrical-order manifest }
```

The cylindrical arm has no discovery-anchor field, including for the empty
assignment.  Existing anchored artifacts retain their schema and replay path.

## Recentring and `WhenBad`

Recentring translates integral shifts, coefficient index variables, guards,
and (for the locus arm) fixed-coordinate equalities together.  A centered row
is still only a candidate.

The generated `WhenBad` compiler then derives, rather than imports:

- all pre-cancellation coefficient-pole conditions;
- solved-coefficient denominator conditions;
- coefficient-aware inactive-boundary leaks;
- strict same-sector descent; and
- target-sector descent obligations.

For a global candidate these conditions describe a sector-wide parametric
domain.  For a locus-bound candidate they are conjoined with the centered
assignment and may only produce a conditional concrete reduction.  An
unsupported condition remains typed unsupported; it is not a master or zero
proof.

## Persistence and resource contract

Construction and replay must bound and retain at least:

- candidates and centered RHS terms;
- centered assignment entries and integer-bit work;
- source/base assumptions, origin references, typed-manifest bytes, and
  actual owned retained bytes;
- recentering translations and Symbolica algebra work;
- ordering-authority identity bytes;
- candidate-binding bytes; and
- replay comparisons.

Every retained relation/condition allocation must be fallibly preflighted or
shared from an authenticated owning certificate.  Limits are checked before
deep Symbolica work or cloning.  Replay regenerates the source row system and
persistent elimination, reconstructs the selected pivot and authority arm,
and compares the complete payload rather than trusting an identity string.

## Acceptance tests

The first test matrix is topology-neutral and includes:

1. empty assignment produces a global arm with no anchor;
2. nonempty assignment produces only a locus arm;
3. centered assignments use `c+s`, including negative shifts and checked
   integer extremes;
4. base assumptions survive both arms and concrete specialization;
5. foreign family/context/sector and dependent symbolic starts fail before
   recentering;
6. every pivot ordinal is replayed and out-of-range pivots fail typed;
7. source, pivot, relation, assignment, assumption, ordering-authority,
   limits, and statistics tampering fails replay;
8. exact and one-below evidence exists for every positive resource limit; and
9. one-loop and external-momentum/ISP families validate the generic surface,
   while concrete loop topologies remain oracle fixtures only.

In addition, the massive one-loop tadpole must derive its global dot recurrence
from the authenticated empty sector-root origin with legacy anchored discovery
disabled on the subject path.  Reducing `I(2)`, `I(3)`, and `I(4)` through a
rule first found by the anchored fixed-point compiler does not satisfy this
test, even if a later cylindrical certificate can replay an unrelated empty
residual.

Only the global arm may enter the ordinary parametric-rule/coverage provider.
Only the locus arm may enter the conditional provider.  This type boundary is
the next prerequisite for the recursive LiteRed-like `SolvejSector` driver.

## Cross-check against LiteRed2's solver loop

The split above is a proof-preserving Rust interpretation of the current
LiteRed2 implementation, not a topology-specific recurrence scheme.  In
`vendor/LiteRed2/Source/LiteRed2026.m`:

- lines 2375--2377 obtain the power-shift policy and the generic `IBPLI`
  function before sector solving;
- lines 2430--2469 maintain symbolic/numeric cases and the set of still-free
  index variables;
- lines 2471--2481 grow `preparepoints`, instantiate the complete identity
  function at those points, and submit the cumulative equations to the
  elimination database;
- lines 2484--2499 translate the selected pivot back to the symbolic index
  origin, select the case on which it was found, and attach the applicability
  condition; and
- lines 2565--2569 compute `WhenBad` from coefficient-denominator zeros and
  cross-sector leaks rather than importing a prewritten recurrence domain.

RustRed deliberately separates the roles that Mathematica expressions combine
there.  The finite prepare-point transcript authenticates pivot discovery;
the recentered generated identity authenticates the symbolic equality; the
transitive base-assumption closure authenticates all divisions used to obtain
it; and the later generated `WhenBad` certificate authenticates its domain.
A prepare point, concrete topology fixture, or expected oracle formula is not
allowed to stand in for any of those four proof objects.
