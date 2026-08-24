# Symbolica-native `SolvejSector` design for RustRed

Date: 2026-08-13

## Status and claim boundary

This document specifies the generic sector-wide rule-derivation layer that
must follow RustRed's parametric IBP/LI generator.  It is based directly on
LiteRed's `SolvejSector`, `WhenBad`, and `SmartReduce` implementation in
`vendor/LiteRed2/Source/LiteRed2026.m:2323-2578`.

RustRed already has exact, topology-independent `K(n)` identity generation,
sparse elimination, concrete specialization, symmetry/zero quotients, and a
structurally replayable polynomial case partition.  Those pieces do **not**
yet constitute sector-wide `SolvejSector` parity.  Parity requires every
inhabited symbolic leaf to carry either a replayed descending rule, a proved
zero/symmetry transport, an explicitly selected master policy, or an
`Uncovered` result.  Search exhaustion is never a proof of master status.

All production inputs are a family, generated identities, a sector, cuts,
ordering, assumptions, and checked resource policies.  Named topologies,
loop-count dispatch, expected recurrence coefficients, and known master
counts are forbidden production inputs.

## LiteRed behavior being reproduced

For a sector corner `corner`, LiteRed uses the exact integer orthant

```text
corner_i = 1: n_i >= 1
corner_i = 0: n_i <= 0
```

at `LiteRed2026.m:2384`.  The main loop keeps a list of symbolic uncovered
cases (`:2430-2438`), submits generated identities at a bounded stencil of
partly symbolic points (`:2471-2481`), asks the ordered equation database for
a candidate (`:2481`), and constructs the rule condition with `WhenBad`
(`:2484-2505`).  Covered cases are removed; the complement is normalized back
to new symbolic cases (`:2522`).

`WhenBad` (`:2565-2568`) has two logically distinct hazards:

1. a coefficient denominator factor vanishes identically in the kinematic
   parameter field at the current indices; or
2. an RHS shift activates an index that is inactive in the source sector and
   the exact coefficient numerator does not vanish on that boundary.

It does not independently retest generic same-sector ordering.  That property
comes from how `Solvej` selected the LHS.  RustRed does not inherit the
Mathematica database's implicit invariant, so it must retain an explicit
uniform descent proof or reject the candidate as unsupported.

At fully numeric points LiteRed specializes identities and applies zero-sector
and symmetry rules **before** elimination (`:2475`).  RustRed's existing
certified concrete quotient provider follows this timing.  The symbolic
solver must do the analogous operation on each exceptional leaf, rather than
reuse a generic pivot whose required guard is false there.

## Exact coefficient-field convention

Let the family base field be

```text
K = Q(theta_1, ..., theta_s)
```

and the free integral-index field be `K(n_1,...,n_N)`.  A polynomial involving
only `theta` is a coefficient-field constant.  A nonzero such polynomial is
not an index case split; it is a typed kinematic/domain assumption.  In
particular, RustRed must not manufacture a generic branch `theta = 0` merely
because Symbolica stores `K(n)` as a rational function of all `theta,n` over
the integer polynomial ring.

Every coefficient operation uses the exact authenticated variable map.
Symbolica's automatic variable-map unification is rejected at proof
boundaries.  Algebraic cancellation never erases the pre-cancellation
nonzero conditions retained by RustRed.

## Candidate admissibility certificate

A candidate admissibility compiler consumes a complete
`ParametricReductionRuleCandidate`, not an arbitrary solved equation.  Replay
must bind all of the following:

- family and `K(n)` context fingerprints;
- exact ordered generated source rows and their guard provenance;
- elimination source manifest, ordering, anchor, pivot ordinal, and trace;
- centered unit-LHS relation;
- source sector and its integer orthant;
- compiler limits and the complete hazard transcript.

For every non-LHS coefficient `c_delta(n)=a_delta(n)/b_delta(n)` it records:

- `b_delta(n) != 0`, including inherited relation-domain conditions; and
- for each inactive position `i` with `delta_i > 0`, all finite boundary
  values

  ```text
  n_i in {1-delta_i, ..., 0}.
  ```

At a boundary value `v`, specialize only `n_i -> v` in the exact numerator
`a_delta`.  The leak is bad precisely on

```text
n_i = v AND a_delta(...,v,...) != 0.
```

RustRed intentionally keeps this sharper complementary split:

```text
n_i != v                         safe for this event
n_i = v AND a_delta(...,v,...)=0 safe for this event
n_i = v AND a_delta(...,v,...)!=0 bad for this event
```

LiteRed conservatively drops a whole boundary clause only when the numerator
becomes identically zero after substitution.  RustRed's refinement is sound
because it represents the remaining numerator-zero sublocus explicitly; the
certificate must document and replay this deliberate difference.

If the specialized numerator is a nonzero member of `K`, the equality branch
is impossible in the formal coefficient field and the boundary is
unconditionally bad.  If it is zero, the boundary is unconditionally safe.
If it still depends on other indices, it becomes a neutral polynomial
equality/nonzero split.

Denominator hazards use the opposite polarity: `b=0` is bad and `b!=0` is
safe.  This is why the underlying case-partition API must expose neutral
`equal_zero_case` and `nonzero_case` names rather than semantic `bad/good`
names.

The compiler classifies each final leaf as one of:

- `Applicable`: every domain condition and leak event is discharged and all
  surviving RHS terms have a uniform strict-descent proof;
- `Inapplicable`: at least one domain or leak hazard is proved on the leaf;
- `Unsupported`: exact applicability would require an unimplemented integer
  or ordering proof; or
- `Empty`: the leaf has an independently replayed emptiness certificate.

An `Unsupported` leaf is never treated as applicable or as a master.

## Uniform descent

For the persisted RustRed order, a candidate must prove that every surviving
same-sector target `J(n+delta)` is strictly easier throughout an applicable
leaf.  The proof may use:

- the exact source sector orthant;
- fixed-coordinate equalities carried by the leaf;
- the candidate's authenticated elimination ordering; and
- checked affine arithmetic on the order-key differences.

No finite sample is a uniform proof.  If a key component has an unresolved
sign, the compiler returns `UnsupportedDescent` or refines on a supported
integer boundary.  Subsector targets are descending by the persisted sector
lattice only after cuts/zero/symmetry policy has been applied consistently.

## Sector-wide coverage algorithm

The first exact implementation can use a bounded refinement work queue:

1. Start from one leaf containing the sector orthant and no polynomial
   predicates.
2. Authenticate freshly generated `IBPLI`; optionally add translated rows in
   LiteRed's exact diamond order under checked stencil limits.
3. Apply proved-zero and verified-symmetry quotients valid on the current
   leaf.
4. Eliminate the resulting rows and compile every candidate's admissibility
   certificate.
5. Select a candidate by the persisted deterministic rule priority.
6. Refine the live leaf only on predicates needed to decide that candidate.
7. Attach the candidate to applicable children and return inapplicable or
   unsupported children to the work queue.
8. On a child carrying fixed index equalities, partially specialize the
   **generated source identities**, repeat zero/symmetry quotienting, and
   re-eliminate.  Do not inherit the parent pivot.
9. Continue until every inhabited leaf is terminal or a configured search
   resource is exhausted.
10. Freeze and replay the complete partition, every attached derivation, every
    quotient/specialization transcript, and aggregate resource census.

Trying several generic candidates can cover an exceptional leaf without
re-elimination when a second candidate's guard is structurally known there.
Re-elimination is still mandatory when the rank/pivot structure changes on a
fixed-coordinate exceptional case and no retained generic candidate covers
it.

## Partial symbolic specialization

The common LiteRed exceptional cases fix a subset of indices to exact integer
values.  RustRed's partial specialization must:

- substitute those values into every coefficient numerator and denominator;
- specialize every retained domain polynomial and preserve its origins;
- reject a row when a required nonzero condition becomes zero;
- turn nonzero base-only conditions into typed `K` assumptions;
- retain unspecialized index variables on the same authenticated map;
- collect colliding monomials exactly and enforce prospective integer-bit,
  term, operation, and retained-byte limits; and
- catch any Symbolica panic and return a typed failure.

For a row used at a shifted point, translation and specialization order must
be explicit in the transcript.  Centering a pivot changes the source index of
the rule; fixed-coordinate conditions must be translated with it.  A proof
must never substitute `n_i=c` into an uncentered row and then silently claim a
rule centered at `n_i=c` if its pivot shift is nonzero.

## General polynomial exceptional loci

A predicate such as `p(n)=0` need not solve to finitely many coordinate
assignments.  Three levels are kept distinct:

1. **Structural decision.** An explicit `p=0` or `p!=0` predicate can decide
   the same authenticated polynomial without algebraic geometry.
2. **Bounded normalization.** Coordinate equalities and simple univariate
   integer roots can be solved and replayed exactly.
3. **Polynomial-ideal reasoning.** General equality conjunctions require a
   checked Groebner/quotient calculation, saturation for nonzero predicates,
   and an integer-lattice inhabitation argument.

Symbolica provides polynomial reduction and Groebner bases, but its public
constructor is not resource-aware and contains panic surfaces.  RustRed must
wrap it with input/degree/term/variable/operation limits, panic containment,
exact output authentication, and independent reduction replay before using it
in a certificate.  Until that wrapper exists, a general unresolved locus is
typed `UnsupportedPolynomialLocus`; it is not pruned and is not a master.

Nonzero predicates cannot be modeled by simply adding their polynomial to an
ideal.  A future saturation proof may introduce witnesses `t` and equations
`t*p-1=0`, under explicit variable and resource budgets.

## Terminal leaf types

A complete sector certificate may attach only the following terminals:

- `DescendingRule`: exact candidate derivation plus admissibility proof;
- `Zero`: replayed zero-sector certificate;
- `Symmetry`: verified affine family map to a strictly preferred key/sector;
- `SelectedMaster`: explicit caller policy and exact key/case, not search
  exhaustion;
- `CertifiedMaster`: a future independent proof of irreducibility/rank
  deficit under the declared identity/search space;
- `Uncovered`: inhabited or potentially inhabited leaf for which no proof was
  found within supported algorithms/resources;
- `Unsupported`: a typed missing proof capability; or
- `Empty`: exact contradiction/inhabitation proof.

`Uncovered` and `Unsupported` propagate through reduction APIs as incomplete
reduction errors.

## Replay and resource requirements

The final certificate retains and rechecks:

- full partition transcript and leaf predicates;
- generated source-row authentication;
- every translation and partial specialization assignment;
- zero/symmetry quotient evidence;
- every elimination and centered candidate replay;
- every admissibility hazard, boundary value, polynomial substitution, and
  leaf classification;
- terminal attachments and strict-descent witnesses;
- exact counts of rows, terms, guards, origins, polynomial terms/exponents,
  case predicates, retained bytes, arithmetic operations, stencil points,
  and replay work.

All counts are aggregate, checked before allocation where possible, and use
checked integer arithmetic.  A failed refinement or specialization is
transactional: no prefix of the proof becomes visible.

## Validation ladder for this layer

1. Synthetic hand-built candidates exercise denominator guards, several
   inactive shifts, numerator-zero safe subloci, formal base-field constants,
   unsupported descent, and replay tampering.
2. The generated one-loop tadpole rules prove both the active dot recurrence
   and inactive numerator boundary behavior over the complete orthants.
3. Multiple generated one-loop families with symbolic masses/power shifts
   exercise rank-changing fixed-index leaves.
4. Concrete non-parametric powers sample every certified leaf and compare
   with direct generated-row specialization.
5. Atom-level tensor numerators are lowered and reduced using only the
   certified sector rules, then compared structurally with Vakint golden
   outputs while masters remain unsubstituted.
6. Repeat for connected two- and three-loop massive-vacuum families, using
   alphaLoop's hardcoded rules only as an oracle.
7. Advance to connected four- and five-loop families after the lower rungs
   pass and scaling limits are measured.

Concrete powers and topology names occur only in tests/oracles.  Every
accepted production rule remains derivable from the generic family and its
freshly generated parametric identities.
