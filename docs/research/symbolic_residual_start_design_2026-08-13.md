# Replayable symbolic residual starts for RustRed

Date: 2026-08-13

Status: staged implementation in progress. The canonical integer-cylinder
ordering, exact single-cylinder prepare-point layers, cumulative through-depth
schedule, live-work-item-bound start certificate, and private point-major
generated row-system certificate are implemented and replay-tested.
Cylindrical elimination, grouped target matching, affine-locus substitution,
and effective coverage remain outstanding. A unit-affine map extractor now
implements only the first dependent-start foundation; it does not yet build
affine-locus rows or rules.

## Decision and staged scope

The next mergeable `SolvejSector` slice should stop replacing the common
integer-coordinate residual case by one or more concrete corner completions.
It should represent that cylindrical subcase directly:

```text
startp_i = a_i       when the residual case proves n_i = a_i,
startp_i = n_i       otherwise.
```

This object is an integer cylinder: some coordinates are fixed to literal
integers and every remaining coordinate stays symbolic in the authenticated
field `K(n)`. The
generated equation at a prepare-point displacement `delta` is

```text
R_(r,delta)|A =
  sum_s c_(r,s)(startp + delta) J(startp + delta + s) = 0,
```

where `A = {(i,a_i)}` is the sparse fixed-coordinate assignment. In RustRed
this must be implemented, in that order, as

```text
canonical_row
  .translated(delta)
  .partially_specialized_on(A).
```

The first mergeable slice should compile and replay this cylindrical equation
system and expose only condition-bound pivots. It should not yet claim that a
residual leaf is closed. The following slice should compose those pivots into
an effective symbolic coverage overlay and let that overlay, rather than a
concrete sample, update the fixed point.

This is not the complete LiteRed start vocabulary. `Reduce[..., Integers] //
ToRules` may bind an index to an expression in other indices, for example
`n1 -> 3-n2`. LiteRed substitutes that expression into `startp`; it does not
merely retain the equality as an outer predicate. Full parity therefore also
requires the dependent symbolic substitution layer specified below. Integer
cylinders are the first implementation slice and a sound fallback, not the
final claim.

This is topology- and loop-count-independent. A tadpole and the equal-mass
sunset are validation fixtures only.

## Source reading: exact LiteRed behavior

The governing implementation is
[`LiteRed2026.m:2271`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2271)
through
[`LiteRed2026.m:2568`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2568), with
`preparepoints` at
[`LiteRed2026.m:2682`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2682) and
`diamond` at
[`LiteRed2026.m:6094`](../../vendor/LiteRed2/Source/LiteRed2026.m#L6094).

The relevant semantics are more specific than “search near an anchor”:

1. `noRules` is a Boolean residual condition represented as groups of cases
   ([`LiteRed2026.m:2372`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2372)).
   `gatherRules` groups contiguous starts and prioritizes groups by the number
   and position of remaining symbolic indices
   ([`LiteRed2026.m:2419`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2419)).
2. For the selected group, `inds` is exactly the set of indices not fixed by
   a rule LHS in the first case; `numeric` means that set is empty. A fixed
   rule RHS need not be an integer—it may depend on a remaining index
   ([`LiteRed2026.m:2430`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2430)).
3. `startps=(indices/.#)&/@cases` constructs the mixed
   integer/symbolic starts, and `startp=First[startps]` selects the current one
   ([`LiteRed2026.m:2446`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2446)).
4. The shell is exact L1 radius `depth`, not a box or a topology-specific set.
   `diamond[l,d]` is built from compositions of `d` and all signs of nonzero
   components
   ([`LiteRed2026.m:6094`](../../vendor/LiteRed2/Source/LiteRed2026.m#L6094)).
5. For a partly symbolic start, `preparepoints` tests the sector sign only in
   components that are literal integers. A free expression such as `n_i-1`,
   or a dependent fixed component such as `3-n_j`, is not sign-filtered by
   `preparepoints`
   ([`LiteRed2026.m:2698`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2698)).
6. For a fully numeric group, the diamonds around all remaining numeric starts
   are unioned and every coordinate is tested against the sector
   ([`LiteRed2026.m:2682`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2682)).
7. Equations are produced point-major by applying the complete selected
   identity list to each new prepare point. `SR` and `ZerojRule` are added only
   in the fully numeric branch
   ([`LiteRed2026.m:2471`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2471)).
8. The equation database persists solved rows while successive exact shells
   are submitted. A candidate LHS must match the active case pattern
   ([`LiteRed2026.m:2467`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2467)).
9. After a pivot is found, only the still-symbolic coordinates are reflected
   to recenter the LHS
   ([`LiteRed2026.m:2484`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2484)).
   The fixed coordinates are not reflected. The resulting LHS must match one
   of the remaining cases at
   [`LiteRed2026.m:2486`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2486).
   A case group may contain starts such as `(a,n2)` and `(a+1,n2)`, so a
   fixed-coordinate displacement of `+1` can legitimately select the second
   case. Zero displacement is required only by RustRed's proposed first,
   one-source-leaf-at-a-time adaptation.
10. The rule is attached to that case, its exact `WhenBad` condition is
    computed, and only its good domain is removed
    ([`LiteRed2026.m:2488`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2488)).
11. A changed `startp` resets the depth. When symbolic indices remain, depth
    grows up to `MaxDepth`; the default is unbounded
    ([`LiteRed2026.m:2508`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2508)).
12. The accumulated bad domains are normalized back into the global residual
    case list
    ([`LiteRed2026.m:2522`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2522)).

The `NMIs`/remaining-point master inference at
[`LiteRed2026.m:2519`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2519) and
[`LiteRed2026.m:2544`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2544) is a
LiteRed heuristic, not a correctness certificate. RustRed must keep bounded
failure as `Uncovered`, `Unsupported`, or `ResourceLimited` unless an explicit
master policy is supplied.

LiteRed also does not substitute solved proper-subsector rule tables into this
`SolvejSector` elimination. Lower-sector back-substitution occurs later in
`IBPReduce`, notably at
[`LiteRed2026.m:3970`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3970).
Consequently, lower-sector feedback during cylindrical elimination would be a
separate Symbolica enhancement, not required for literal start-point parity.

## What RustRed already has

Most of the algebraic foundation is implemented and should be reused rather
than rewritten:

- [`CylindricalParametricEliminationOrdering`](../../src/cylindrical_ordering.rs)
  implements the exact signed formal V1 key for canonical integer cylinders,
  with no concrete-anchor API.
- [`CylindricalPreparePointLayer`](../../src/cylindrical_prepare_points.rs)
  directly enumerates weak compositions and nonzero signs for one exact L1
  shell, filters only literal assigned coordinates, precomputes exact order
  keys once, and uses a versioned deterministic sort transcript.
- [`CylindricalPreparePointScheduleCertificate`](../../src/cylindrical_prepare_point_schedule.rs)
  compiles depths `0..=through_depth` with remaining cumulative work and
  retained-payload budgets passed before each layer.
- [`GeneratedCylindricalResidualStartCertificate`](../../src/generated_cylindrical_residual_start.rs)
  binds that schedule to one replayed live residual queue item and its exact
  coordinate extraction. Unresolved equality predicates are exposed as a
  typed dependent-start-pending status, never sampled or inferred to be
  masters.

- [`PartialIndexAssignment`](../../src/parametric_coefficient.rs) is a
  canonical, arity-bound sparse map of fixed indices.
- `ParametricCoefficientContext::partially_specialize_coefficient` substitutes
  only those index variables and preserves the mapped pre-normalization
  denominator as a provenance-bearing condition
  ([`parametric_coefficient.rs:1400`](../../src/parametric_coefficient.rs#L1400)).
- [`ParametricRelation::translated`](../../src/parametric_relation.rs#L754)
  translates integral shifts, coefficients, and guards together.
- [`ParametricRelation::partially_specialized_on`](../../src/parametric_relation.rs#L896)
  produces a locus-bound row, separates base-field assumptions, retains
  source provenance, and replays.
- [`GeneratedPartialReeliminationCompiler`](../../src/conditional_reelimination.rs)
  regenerates canonical IBP/LI rows, translates, partially specializes, and
  runs exact sparse elimination.
- [`ConditionalParametricRule`](../../src/conditional_rules.rs) keeps a pivot
  inseparable from its equality locus and applies it only after concrete guard
  and strict-descent checks.
- [`GeneratedSectorConditionalRuleProvider`](../../src/generated_sector_conditional_provider.rs)
  installs those rules as a fallback rather than promoting an unsuccessful
  leaf to a master.
- [`ParametricSectorCoverageCertificate`](../../src/parametric_sector_coverage.rs)
  and [`GeneratedSectorLiveLeafQueueCertificate`](../../src/generated_sector_live_leaf_queue.rs)
  already own the source residual case, exact predicates, coordinate
  extraction, and replayable queue order.

The implementation gap is therefore not partial polynomial substitution. It
is the symbolic `startp` scheduler, formal ordering, LiteRed-compatible pivot
eligibility, and feedback of condition-bound rule domains into effective
coverage.

## Current mismatches that the new slice must not hide

### Corner completion is a heuristic, not a symbolic ordering

The live-leaf queue currently fills every free coordinate with its sector
corner before constructing `ParametricEliminationOrdering`
([`generated_sector_live_leaf_queue.rs:940`](../../src/generated_sector_live_leaf_queue.rs#L940)).
The family fixed point likewise converts assignments and empty-assignment
leaves to concrete points
([`generated_family_fixed_point.rs:2141`](../../src/generated_family_fixed_point.rs#L2141)).

That can change the pivot order. In active sector `11`, compare shifts

```text
s = (-1, 2),    t = (0, 0).
```

At the concrete corner `(1,1)`, `s` appears to enter sector `01` and is
ordered below `t` by propagator count. At a symbolic start `(n1,n2)`, LiteRed
keeps both coordinates in the default sector and the dot excess of `s`
relative to `t` is `+1`, so `s` is harder. A finite corner is therefore not a
faithful representation of symbolic `jComplexity`.

### Empty assignments are currently skipped by conditional re-elimination

A queue item with no recognized equality is emitted as
`PreservedWithoutEqualityAssignment`; the queue does not attempt the generated
partial system. But an empty assignment is exactly the fully symbolic start
`(n_1,...,n_N)`, not absence of a start. It should be the easiest cylindrical
case to compile.

### Existing condition-bound pivots do not bind a displaced target case

`GeneratedPartialReeliminationCertificate` currently exposes every centered
pivot. Centering changes an assignment `n_i=a_i` to `n_i=a_i+p_i`. That is a
sound generated identity on the translated cylinder. LiteRed may use it only
when the displaced assignment matches another remaining case in the same
group. The current RustRed queue owns only the source leaf, so it cannot prove
that target-case membership. The existing one-loop boundary test deliberately
observes the broader behavior: starting from `n=1`, its pivot `+1` creates a
rule on `n=2`. A group-aware symbolic-start path may accept that pivot only
with an authenticated target-case locator. The first mergeable one-leaf path
must record it as `RejectedNoTargetCase`, not as a solution of `n=1`.

### Translation and row priority differ from `preparepoints`

The current partial compiler sorts translations lexicographically and expands
canonical-row-major. LiteRed sorts each prepare-point shell by integral order
and submits equations point-major. Row order changes Gaussian pivot choices
even though it does not change the identity span. The new transcript should
retain LiteRed-style point order. Reusing the old canonicalized stencil as an
optimization would need an explicit “same span, different pivot heuristic”
schema, not a parity claim.

### Conditional runtime coverage is not fixed-point coverage

The conditional provider can reduce a concrete query, but the root coverage
still calls its source leaf `Uncovered` or `Unsupported`. The current family
fixed point therefore cannot use a successful cylinder rule as proof that a
symbolic sublocus is covered. A later effective-coverage layer is required
before the fixed point can close such a leaf.

## Mathematical representation

### First slice: independent integer cylinders

Let `S` be the source sector and let `A` be a canonical partial assignment.
Define

```text
x_i(A,n) = a_i  if (i,a_i) is in A,
x_i(A,n) = n_i  otherwise.
```

The residual cylinder is

```text
C(S,A) = { n in Z^N : n is in S and n_i=a_i for every (i,a_i) in A }.
```

The other predicates in the source residual case are not discarded. They
define a possibly smaller source cell `L subseteq C(S,A)`. Generated IBP/LI
identities specialized on `A` are valid on the whole cylinder; a rule found
because of `L` should nevertheless be attached to `L` in the first parity
slice. Widening it to other cells in the same cylinder is sound only after its
own complete domain proof is composed there and should be a later
optimization.

For canonical generated row

```text
R_r(n) = sum_s c_(r,s)(n) J(n+s) = 0,
```

and prepare-point displacement `delta`, the exact cylindrical row is

```text
R_(r,delta,A)(n) =
  sum_s c_(r,s)(x(A,n)+delta) J(x(A,n)+delta+s) = 0.
```

The operation order is essential:

```text
translate by delta, then partially specialize A.
```

Specializing first would incorrectly evaluate a fixed-index coefficient at
`a_i` instead of `a_i+delta_i`.

The sparse term key may remain the total shift `delta+s` on the authenticated
N-dimensional index map. The row must remain private behind an object that
also owns `A`; otherwise a caller could mistake it for a global `K(n)`
identity.

### Full parity: dependent symbolic starts

A general LiteRed case supplies a triangular rule map rather than only a
sparse integer assignment:

```text
T = { n_i -> f_i(n_free) for i in bound positions }.
```

The `f_i` originate from an exact integer-locus case and may depend on other
indices. Treating `n_i=f_i` only as an outer predicate while eliminating on
the wider independent cylinder is algebraically sound, but can change or miss
pivots and is not full `startp` parity.

The general layer needs a replayable `ResidualIndexSubstitution` bound to the
exact source case. It must retain:

- ordered bound and free positions;
- each typed Symbolica polynomial/rational index expression and its variable
  map;
- an acyclic dependency/normalization transcript, or a typed unsupported
  outcome when the case cannot be represented safely;
- integer-valued and denominator assumptions inherited from the source case;
- the exact predicates from which the map was extracted; and
- resource bounds on terms, degrees, coefficient bits, substitutions, and
  retained bytes.

Generated equations still use translate-then-substitute:

```text
R_r(n).translated(delta).substituted_on(T).
```

For coefficients this sends `n_i` to `f_i(n_free)+delta_i`. Integral labels
also become vectors of typed index expressions, so the existing
`IndexShift`-only relation key is insufficient in the general case. A V2
case-bound relation must either support affine/polynomial index expressions
directly or prove a coordinate change that restores a free-index shift
lattice. Ordering and pivot matching must be defined on that transformed
case, not on an invented integer representative.

The integer-cylinder schema must embed into this general map as `f_i=a_i`
without claiming every residual case has that form. Until V2 lands, a
non-integer equality remains an explicit
`DependentSymbolicStartNotYetSupported` completeness status; it is never
sampled away or interpreted as a master.

## Exact prepare-point geometry

For a partly symbolic start, a shell displacement `delta` is admitted iff

```text
sum_i |delta_i| = depth
```

and every fixed coordinate remains in the source sector:

```text
S_i = 1  => a_i + delta_i >= 1,
S_i = 0  => a_i + delta_i <= 0.
```

No sign filter is applied to free coordinates. This is the precise behavior
of LiteRed's symbolic `preparepoints` overload. In particular:

- `A` empty: every point on every exact shell is admitted;
- `A` nonempty but not full: only assigned coordinates are filtered;
- `A` full: the rule reduces to the ordinary concrete sector filter for one
  start.

The compiler should enumerate exact shells with a heap-resident iterator,
charge all iterator transitions as well as emitted offsets, sort accepted
offsets by the cylindrical integral order below, and append only offsets not
seen at earlier depths. The retained row order is

```text
for prepare point in ordered new points:
    for generated IBP/LI row in canonical order:
        append translated-and-specialized row
```

This is point-major like LiteRed. Re-eliminating the cumulative row vector at
every depth is an acceptable Symbolica adaptation to LiteRed's incremental
`Solvej` database, provided the complete ordered source vector and elimination
are replayed.

## Formal cylindrical ordering

### Required API

The concrete-only [`ParametricEliminationOrdering`](../../src/parametric_elimination.rs#L30)
should not be overloaded by inventing a “representative” corner. Introduce a
v2 persisted ordering mode:

```rust
pub enum ParametricEliminationOrderingV2 {
    ConcreteV1 {
        policy: IntegralOrderingPolicy,
        anchor: Box<[i64]>,
    },
    CylindricalV1 {
        policy: IntegralOrderingPolicy,
        sector: SectorMask,
        assignment: PartialIndexAssignment,
    },
}
```

V1 certificates can retain the current type and schema. New cylindrical
eliminations should use a V2 certificate so replay cannot confuse a concrete
anchor with a symbolic start. A compatibility constructor may wrap the old
ordering as `ConcreteV1`; there must be no `anchor()` API that silently returns
a corner for `CylindricalV1`.

### Exact normalized key for `RustRedUnshiftedV1`

For a column shift `s`, define its formal sector bit by

```text
B_i(s) = S_i                    if i is free,
B_i(s) = [a_i+s_i >= 1]        if i is fixed.
```

For a free coordinate, the source-symbol contribution is common to every
column and can be removed from a comparison. The normalized index-excess
offset is

```text
e_i(s) =  s_i   for a free active coordinate,
e_i(s) = -s_i   for a free inactive coordinate.
```

For a fixed coordinate, use the ordinary exact excess of `a_i+s_i`:

```text
e_i(s) = a_i+s_i-1   when B_i(s)=1,
e_i(s) = -(a_i+s_i)  when B_i(s)=0.
```

The normalized cylindrical key is the existing V1 field sequence with signed
offsets:

```text
arity,
propagator count from B(s),
sector bits B(s),
sum_i e_i(s),
sum_{B_i(s)=1} e_i(s),
sum_{B_i(s)=0} e_i(s),
[e_0(s), ..., e_(N-1)(s)].
```

The omitted free-symbol contribution is identical in the corresponding field
of every column, so comparing these signed normalized keys is exactly the same
as comparing at every sufficiently interior concrete realization of the free
variables. Use checked `i128` arithmetic for the signed sums and fall back to
the lattice shift as a deterministic tie-breaker. Persist the key schema and
policy identifier.

This ordering is not an attempt to reproduce a caller-customized LiteRed
`jsOrder` matrix. It is the exact cylindrical extension of RustRed's named V1
order. The difference is an intentional, replayed Symbolica/Rust adaptation.

### Why a “deep enough” concrete representative is inferior

One could inspect all current shifts and choose free coordinates far enough
inside the sector that no column crosses a boundary. That produces the same
order for that one finite column set, but it makes the proof depend on extrema
of a transient system and can overflow `i64`. The signed normalized key is
direct, independent of a fabricated point, and remains valid as cumulative
depth adds columns.

## Pivot eligibility and recentering

Let `p` be an elimination pivot shift relative to the first symbolic start in
a case group. On every fixed coordinate, it induces a target value

```text
a'_i = a_i + p_i.
```

The pivot is LiteRed-eligible iff one of the remaining cases in the active
group has exactly those fixed values and the same free-coordinate positions.
Record every pivot with one of:

```rust
pub enum CylindricalPivotEligibility {
    Eligible {
        pivot_ordinal: usize,
        target_case: GeneratedResidualWorkItemLocator,
        target_assignment: PartialIndexAssignment,
        checked_fixed_positions: usize,
    },
    RejectedNoTargetCase {
        pivot_ordinal: usize,
        displaced_assignment: PartialIndexAssignment,
    },
}
```

Rejected pivots remain part of the elimination replay but cannot be installed
as rules for this group. The candidate-selection transcript must retain the
deterministic search through target cases.

For an eligible pivot, reflecting/recentering the free coordinates leaves the
fixed LHS at the authenticated target assignment. The rule is valid only on
that target case plus all retained coefficient/base assumptions. It must
remain a
`ConditionalParametricRule`-like object and must not implement conversion to
the global `ParametricReductionRuleCandidate` type.

The first mergeable slice intentionally processes one source leaf at a time.
Its only authenticated target is the source leaf itself, so its eligibility
predicate reduces to `p_i=0` in every fixed coordinate. That is a conservative
Symbolica/Rust staging restriction, not universal LiteRed behavior. The data
model must retain a target-case field so grouped-case scheduling can remove
this restriction without changing the algebraic certificate schema.

The current `ConditionalCenteredPivotLocus` formula `a_i+p_i` remains useful
for the older broader conditional path.  The initial one-leaf constructor
should require that the centered assignment equals the source assignment
exactly.  The grouped-case constructor must instead require equality with an
authenticated target case, so this first-slice restriction is not encoded as
universal LiteRed semantics.

## Proposed replay structures

Names below are recommendations; the invariants are mandatory.

### `GeneratedCylindricalResidualStartCertificate`

```rust
pub struct GeneratedCylindricalResidualStartCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    source: GeneratedResidualWorkItemLocator,
    sector: SectorMask,
    assignment: PartialIndexAssignment,
    free_positions: Box<[usize]>,
    ordering: CylindricalParametricEliminationOrdering,
    layers: Box<[CylindricalPreparePointLayer]>,
    limits: GeneratedCylindricalResidualLimits,
    stats: GeneratedCylindricalResidualStats,
}
```

`GeneratedResidualWorkItemLocator` should resolve through the enclosing
fixed-point material graph:

```rust
pub struct GeneratedResidualWorkItemLocator {
    material: GeneratedFixedPointMaterialLocator,
    work_item_ordinal: usize,
    source_case: SymbolicSectorCaseId,
}
```

Replay must resolve the locator, replay the queue/extraction, require
`NotProvedEmpty`, reproduce the exact assignment and free-position complement,
and compare the complete source case and disposition. It must not trust a
stored `assignment` detached from its equality witnesses.

Construction must also prove that every equality consumed by this V1 object
is a literal integer coordinate assignment. A dependent symbolic equality
takes the typed V2-pending path above rather than being silently left among
generic predicates while the certificate claims complete start parity.

The enclosing family fixed-point certificate should own immutable material by
`Arc`; a start stores a locator, not another deep copy of the discovery,
partition, and queue. Pointer identity is an in-memory scaling invariant;
payload replay remains the persistence invariant.

### `CylindricalPreparePointLayer`

```rust
pub struct CylindricalPreparePointLayer {
    depth: usize,
    enumeration_steps: usize,
    enumerated_offsets: usize,
    rejected_fixed_sector_offsets: usize,
    ordered_translations: Box<[IndexShift]>,
}
```

Replay regenerates the exact L1 shell, repeats checked fixed-coordinate sign
tests, derives the cylindrical key for every accepted translation, and
re-sorts. An offset is retained once, at its exact depth.

The implemented schedule stores its ordering in one `Arc`; every retained
layer holds a shallow clone of that same allocation.  The ordering's own
arity, assignment, key-component, and manifest limits are therefore charged
once, while `through_depth <= max_depth` bounds the number of retained layer
references.  Schedule cloning does not multiply the assignment,
free-position table, sector mask, or stable manifest.  Tests assert pointer
sharing in memory; replay still compares structural payloads and never treats
pointer identity as a persistence proof.

Public `compile` and `replay` retain full reconstruct-and-compare
authentication.  Composition now uses crate-private constructors whose input
ordering/source has already been replayed by the parent: a schedule constructs
unreplayed child layers, and a generated residual start constructs an
unreplayed schedule.  This removes the former nested automatic replay tree
(where every parent reconstruction recursively compiled and replayed every
child) without weakening explicit public replay.  Each public boundary first
replays its owned dependency, reconstructs its complete child payload once,
and compares all persisted fields.

Pending unresolved equality ordinals use a two-pass bounded collector.  The
first pass is allocation-free and rejects a count above
`max_pending_dependent_equalities`; only a successful exact census permits
allocation and retention of the ordinal array.  The focused tests cover a
tight one-below limit and a nonzero work-item ordinal rejected before queue
lookup.

### `GeneratedCylindricalEliminationCertificate`

```rust
pub struct GeneratedCylindricalEliminationCertificate {
    schema: &'static str,
    start: Arc<GeneratedCylindricalResidualStartCertificate>,
    through_depth: usize,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    row_witnesses: Box<[GeneratedCylindricalSourceRowWitness]>,
    retained_rows: Arc<[PartialParametricRelationSpecialization]>,
    elimination: ParametricEliminationV2,
    pivot_eligibility: Box<[CylindricalPivotEligibility]>,
    base_assumptions: Box<[GeneratedPartialBaseAssumptionWitness]>,
    limits: GeneratedCylindricalResidualLimits,
    stats: GeneratedCylindricalResidualStats,
}
```

Each source-row witness binds:

- prepare-point depth and within-layer ordinal;
- canonical generated-row ordinal and row id;
- exact translation;
- canonical, translated, and partially specialized manifests;
- retained versus unsatisfiable-domain outcome; and
- base-assumption ordinals.

The row vector must be point-major and must contain every retained row through
`through_depth`. The elimination certificate stores the cylindrical ordering,
source manifest, pivot traces, guards, and resource policy. Replay regenerates
the row span, layers, every translation/specialization, then the elimination
and eligibility list.

The private row accessor already used by conditional re-elimination can be
reused. Do not make `PartialParametricRelationSpecialization::relation` public.

### `GeneratedCylindricalRuleCertificate`

This is a narrow sibling or stricter constructor of
`ConditionalParametricRule`:

```rust
pub struct GeneratedCylindricalRuleCertificate {
    schema: &'static str,
    derivation: Arc<GeneratedCylindricalEliminationCertificate>,
    pivot_ordinal: usize,
    source_case: GeneratedResidualWorkItemLocator,
    target_case: GeneratedResidualWorkItemLocator,
    centered_assignment: PartialIndexAssignment,
    centered_relation: ParametricRelation, // private
    base_assumptions: Box<[...]>,
    limits: ConditionalParametricRuleLimits,
}
```

Construction requires an `Eligible` witness and exact equality between its
authenticated target assignment and the centered assignment. In the initial
one-leaf implementation this also equals the source assignment. Concrete
application checks the sector, assignment, all retained guards, unit LHS,
coefficient-aware RHS support, and strict descent exactly as the existing
conditional rule does.

The fixed-point/effective-coverage claim is restricted to the authenticated
source cell and therefore retains every source predicate. The runtime
conditional provider may apply the same generated identity at another point
of its equality cylinder when the rule's own guards and descent proof succeed;
that is a sound pointwise widening, but it is a deliberate Symbolica runtime
optimization rather than evidence that the wider symbolic cell was solved.

For the first slice this certificate may be installed only through the
existing conditional fallback provider. It is a useful, replayable rule but
is not yet evidence that the symbolic source case disappeared.

## Effective symbolic coverage: the immediately following slice

Global V3 coverage understands only globally generated candidates. A
cylindrical row is valid on an equality locus, so it needs a separate overlay
rather than being smuggled into `GeneratedWhenBadCompiler` as a global row.

Introduce:

```rust
pub struct GeneratedEffectiveSectorCoverageCertificate {
    root: Arc<ParametricSectorCoverageCertificate>,
    conditional_attempts: Box<[GeneratedCaseBoundRuleAttempt]>,
    partition: SymbolicSectorCasePartitionCertificate,
    classifications: Box<[GeneratedEffectiveLeafClassification]>,
    limits: ...,
    stats: ...,
}

pub enum GeneratedEffectiveLeafDisposition {
    GlobalDescendingRule { candidate_ordinal: usize },
    CylindricalDescendingRule { attempt_ordinal: usize },
    ProvedEmptyLocus { ... },
    Uncovered,
    Unsupported { ... },
}
```

The compiler reconstructs the root partition and refines only the referenced
source case on the cylindrical rule's denominator, leak, and uniform-descent
conditions. The inherited source predicates remain in every child. The good
children receive `CylindricalDescendingRule`; bad or unsupported children stay
live. Earlier root descending leaves remain authoritative.

This is the Symbolica-native counterpart of LiteRed attaching a rule under
`RulesToCondition[{case}] && !WhenBad` and feeding
`RulesToCondition[{case}] && WhenBad` back into `noRules`.

The V3 live-leaf queue should then consume effective classifications, not only
the root coverage classifications. A fixed-point round may claim progress
only after replaying this overlay and proving that an exact source sublocus is
now descending. Counts or concrete witnesses are insufficient.

General source predicates do not block cylindrical equation derivation. They
remain structural restrictions on where the derived rule is attached. Thus an
empty coordinate assignment and an otherwise complicated polynomial leaf can
still produce a fully symbolic start without a Gröbner solver. Polynomial
ideal reasoning is needed only for additional contradiction/inhabitation
proofs, not for the validity of generated IBP identities.

## `WhenBad` on a cylinder

The existing low-level `WhenBad` logic should be factored so it can consume a
private centered relation plus an authenticated derivation origin. The
cylindrical compiler adds the source assignment as a premise and preserves all
source-case predicates in the outer overlay.

For each coefficient it must retain:

- every inherited and normalization denominator condition;
- base-only nonzero assumptions as `K` assumptions, never index branches;
- inactive-coordinate leak boundaries with coefficient-numerator zero
  refinement; and
- a uniform strict-descent proof on the good branch.

The cylindrical order makes same-formal-sector shift comparisons constant.
For fixed coordinates, target sector membership is numeric. For free inactive
coordinates shifted positively, the existing finite boundary split remains
necessary. A target in a strict subsector is descending under the named
sector order; a surviving supersector target is a leak. Any unresolved
ordering condition is `Unsupported`, never applicable.

The first implementation must not apply LiteRed's numeric `SR` quotient to a
partly symbolic start. Analytic zero-sector certificates and verified
whole-row symmetry transports that are valid parametrically may remain
separate authenticated inputs. When every coordinate is fixed, delegate to
the existing certified concrete zero/symmetry quotient path; add a numeric
multi-start cylinder only after that quotient is represented in the same
transcript.

## Symbolica API choices

The coefficient path should remain on Symbolica's native rational-polynomial
types rather than converting rows to strings or emulating Mathematica
conditions as untyped atoms:

- affine translation uses `MultivariatePolynomial::replace_with_poly` with
  `n_i -> n_i + delta_i`; RustRed already wraps it with prospective term/bit
  limits and panic containment in
  [`parametric_coefficient.rs:1940`](../../src/parametric_coefficient.rs#L1940);
- sparse integer specialization uses
  `MultivariatePolynomial::replace(variable, Integer)` from
  [`vendor/symbolica/src/poly/polynomial.rs:1780`](../../vendor/symbolica/src/poly/polynomial.rs#L1780);
- rational reconstruction uses
  `FromNumeratorAndDenominator::from_num_den` from
  [`vendor/symbolica/src/domains/rational_polynomial.rs:406`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L406), but source denominators must be copied into guards before normalization;
- exact polynomial equality, dependence, divisibility, and specialization
  should use the authenticated variable map already owned by
  `ParametricCoefficientContext`.

Symbolica's atom pattern matcher is useful at expression-facing boundaries,
but it should not encode the core `J(n+s)` lattice. Typed `IndexShift`,
`PartialIndexAssignment`, and polynomial predicates make arity, translation,
and replay explicit and avoid matching a foreign symbol namespace. The
Mathematica `Pattern`/`Condition` at LiteRed lines 2468--2489 corresponds in
RustRed to a typed source-case locator plus a condition-bound rule, not a
runtime string pattern.

No Symbolica `no_gmp` feature is involved. All exact integer/rational
operations remain on the GMP-enabled configuration.

## Limits and failure semantics

Add a coherent aggregate policy, nested around the existing arithmetic,
specialization, elimination, and conditional-rule limits:

```rust
pub struct GeneratedCylindricalResidualLimits {
    pub specialization: PartialParametricRelationSpecializationLimits,
    pub elimination: ParametricEliminationLimits,
    pub conditional_rule: ConditionalParametricRuleLimits,
    pub max_depth: usize,
    pub max_shell_offsets: usize,
    pub max_shell_enumeration_steps: usize,
    pub max_prepare_points: usize,
    pub max_prepare_point_components: usize,
    pub max_fixed_sector_checks: usize,
    pub max_order_key_components: usize,
    pub max_order_comparisons: usize,
    pub max_expanded_rows: usize,
    pub max_retained_rows: usize,
    pub max_pivots: usize,
    pub max_pivot_eligibility_checks: usize,
    pub max_eligible_pivots: usize,
    pub max_base_assumptions: usize,
    pub max_source_manifest_bytes: usize,
    pub max_transcript_bytes: usize,
}
```

All aggregate budgets must be charged before retaining or cloning payloads.
Shell step counts are cumulative across depths, not reset per call. Row counts
must be preflighted as `new_points * canonical_rows` before expansion. Signed
key arithmetic uses checked operations. Symbolica calls remain panic-contained
behind the existing wrappers.

Typed terminal outcomes are:

```text
Certified { eligible rules... }
NoEligiblePivotWithinConfiguredDepth
EmptyConditionalSystem
UnsupportedOrdering
ResourceLimited { exact stage and locator }
Failed { exact non-resource cause }
```

`NoEligiblePivotWithinConfiguredDepth` is not a master proof and does not
erase the source case. An empty generated row system means only that all rows
were unavailable under their inherited guards; it is not proof that the
integral cylinder is empty.

## First mergeable production slice

The smallest useful implementation should contain all of the following; doing
only another concrete-frontier sampler would not advance parity.

Items 1, 2, and 4 below, including cumulative scheduling,
source-work-item authentication, point-major provenance, translate-before-
specialize construction, aggregate limits, direct concrete oracle checks, and
typed rejection of dependent starts, are now implemented. Items 3 and 5--8
remain the next integration work; therefore the current code is a replayable
cylindrical row-system certificate rather than a completed rule compiler.

1. Add `cylindrical_ordering.rs` with the signed normalized V1 key, exact
   comparator, replay manifest, and adversarial unit tests.
2. Add `generated_cylindrical_residual.rs` with a source-work-item-bound start
   certificate and exact, ordered `preparepoints` layers.
3. Add a V2 elimination entry point accepting cylindrical ordering while
   leaving concrete V1 certificates readable.
4. Reuse translation followed by
   `PartialParametricRelationSpecialization` to build cumulative point-major
   generated row systems. Accept the empty assignment.
5. Record every pivot's displaced fixed assignment. In the first one-leaf
   slice, construct rules only when it equals the source assignment; retain a
   target-case-ready eligibility schema for grouped scheduling.
6. Add a strict cylindrical constructor to `ConditionalParametricRule` (or a
   sibling type) that requires `centered_assignment == source_assignment`.
7. Let the conditional provider install these rules for concrete application,
   but keep the source leaf unresolved in the family fixed-point status until
   effective coverage lands.
8. Replay the complete source locator, start geometry, ordering, generated row
   lineage, partial specialization, elimination, eligibility, and rule.

This slice produces real parametric rules from integer-cylinder symbolic
starts and removes the need to guess one concrete point merely to discover
them. Its claim boundary remains honest: dependent symbolic expressions and
sector-wide effective closure remain later parity work.

## Validation plan

All test binaries should run with `cargo nextest run -j4` and the licensed,
GMP-enabled Symbolica build.

### Geometry and ordering tests

1. For several synthetic sectors and assignments, compare exact shell output
   with an independent enumerator implementing the two LiteRed
   `preparepoints` formulas. Assert that free coordinates are not sign-filtered
   and fixed coordinates are.
2. Use active sector `11` and shifts `(-1,2)` versus `(0,0)` to prove that the
   cylindrical comparator differs from corner completion and agrees with
   several sufficiently interior concrete realizations.
3. Exhaustively compare cylindrical keys with deep concrete keys for small
   arities, assignments, and shifts. Only comparisons whose fixed coordinates
   are representable are admitted.
4. Tamper with sector, assignment, free-position order, shell depth,
   translation order, or key schema and require replay failure.
5. Drive every new resource limit to one below the observed count and require
   a typed failure before durable partial state is returned.

### Algebra and pivot tests

1. For generated rows, compare

   ```text
   translate(delta) -> partial specialize(A) -> specialize remaining free n
   ```

   with direct full specialization of the original row at
   `x(A,n)+delta`. Compare terms, coefficients, guards, and origins.
2. Construct a small two-index system containing both zero and nonzero fixed
   pivot displacements. Assert that the one-leaf compiler accepts only zero
   displacement, then add a group-level fixture proving that a nonzero
   displacement is accepted only when an authenticated target case exists.
3. Retain the current tadpole `n=1`, pivot `+1` regression for the legacy
   broad conditional path, and add a parity regression proving that the new
   start-bound constructor rejects it as a solution of `n=1`.
4. Verify that an empty assignment compiles and replays as a fully symbolic
   start instead of `PreservedWithoutEqualityAssignment`.

### One-loop validation

Use the massive tadpole family with no topology-specific recurrence input.
The fully symbolic active-sector start should derive the dot recurrence and
reduce concrete `I(2)`, `I(3)`, and `I(4)` to the unspecialized `I(1)` master.
Compare coefficients with the existing Vakint oracle assertions. Then route
the same provider through the tensor-numerator composer and compare the
one-loop tensor matrix with Vakint while leaving master topology substitution
disabled.

The `I(1)` boundary must remain explicit unless selected by master policy; a
failed start search cannot make it a master.

### Two-loop validation

For the connected equal-mass sunset:

1. select a residual `011` leaf with an empty coordinate assignment;
2. require a fully symbolic start and a depth-one prepare-point transcript;
3. derive the rule used at `J(-1,1,1)` without a concrete frontier origin;
4. replay the exact generated row lineage and candidate;
5. reduce the concrete query through the conditional provider and compare
   with the existing Vakint oracle result;
6. separately retain the `J(2,1,1)` scalar oracle check.

A partial-assignment fixture should additionally bind one index, leave at
least two free, and assert fixed-coordinate-zero pivot eligibility. The
fixture may be selected from an actual sunset residual leaf or built from a
synthetic partition; topology names remain test-only.

### Progression

After the effective-coverage overlay is replay-tested on one loop and sunset,
replace the concrete-anchor path as the primary fixed-point driver. Retain it
only as an explicitly labeled bounded heuristic/fallback. Then advance the
same generic machinery to connected three-loop vacuum families before four-
and five-loop validation.

## Exact parity versus deliberate adaptations

The boundary should remain visible in code and documentation.

Exact LiteRed behavior currently reproduced:

- mixed literal-integer/free-symbol `startp` for the integer-cylinder subset;
- exact L1 `preparepoints` shells;
- sector filtering only on fixed coordinates for a symbolic start;
- point-major generated IBP/LI equations;
- translation before partial specialization;
- cumulative depth growth;
- no numeric `SR` use while symbolic indices remain.

Still required before this slice can claim LiteRed rule-discovery parity:

- persistent elimination in the cylindrical ordering;
- pivot LHS matching one of the remaining grouped cases;
- recentering only free coordinates;
- conditional-rule construction and `WhenBad` good/bad feedback; and
- effective symbolic coverage of the source residual cases.

Deliberate Symbolica/Rust adaptations:

- RustRed's named deterministic order replaces mutable Mathematica `jsOrder`;
- a direct signed cylindrical key replaces symbolic expression comparison;
- the first implementation solves one authenticated source leaf at a time,
  so it admits only zero fixed-coordinate displacement until grouped target
  case scheduling lands;
- dependent symbolic rule RHSs such as `n1 -> 3-n2` remain a typed unresolved
  V2 case until case-bound expression relations and ordering are implemented;
- exact sparse Gaussian elimination may rebuild the cumulative system instead
  of mutating LiteRed's `Solvej` database;
- typed shifts, assignments, and predicates replace Mathematica patterns;
- every generated row and partial specialization is authenticated and
  replayed;
- all algebra and retained payloads have checked limits and panic containment;
- each leaf is handled independently before any contiguous-case grouping
  optimization; and
- bounded failure stays explicitly unresolved instead of invoking LiteRed's
  master-count heuristic.

These adaptations may change which valid pivot is found first. They must not
change the generated identity space, weaken rule-domain guards, turn a
conditional row into a global identity, or infer a master from search
exhaustion.
