# Solved-subsector feedback: exact boundary and minimal generic design

Status: source audit and implementation design.  No production code was
changed for this note.  The equal-mass sunset appears only as a black-box
validation fixture; every proposed production type and algorithm is topology-
and loop-count-independent.

## Decision in one page

There are three distinct operations, and RustRed must name and certify them
separately.

1. **LiteRed-faithful reduction/application.**  `SolvejSector` derives a rule
   table for one sector while leaving proper-subsector integrals on the right-
   hand side.  `IBPSelect` discovers the dependency graph and `IBPReduce`
   substitutes already reduced lower-sector tables into higher-sector tables.
   RustRed's recursive `ParametricReductionEngine` already implements the
   essential demand-driven application behavior, although a family-level,
   replayable dependency/back-substitution transcript would make the parity
   claim explicit.

2. **Concrete pre-elimination quotient.**  At a fully fixed integer point it
   is sound to erase certified zero terms, canonicalize concrete terms with
   verified symmetries, and recursively normalize proper-subsector terms with
   an immutable lower-sector provider before exact base-field elimination.
   The zero/symmetry part matches LiteRed's numeric source order.  Feeding
   solved lower rules into this elimination is a useful RustRed extension,
   not something `SolvejSector` itself does.

3. **Parametric solved-subsector feedback.**  A shifted symbolic term belongs
   to a proper subsector only on particular index loci.  Therefore a lower
   rule may be used as a *subsector rewrite* only after a parent leaf proves
   that locus and, for a conditional lower rule, proves the pulled-back lower
   equality locus.  The resulting rows and pivots remain bound to that leaf,
   like `GeneratedPartialReeliminationCertificate`; they must never enter the
   global `GeneratedSymbolicRowSpanCertificate`.

The smallest honest implementation sequence is therefore:

- make family scheduling immutable and subsector-first, with committed lower
  material snapshots and no current-sector access;
- add concrete lower-sector normalization to the existing concrete quotient
  certificate, usable only for fully numeric residual leaves;
- extend conditional partial re-elimination with leaf-proved parametric lower
  rewrites and a new nested source-authentication mode; and
- only then use that conditional material in the residual fixed point.

A concrete-only implementation is a sound first slice and an excellent
sunset regression.  It is **not** sufficient to claim generic parametric
`SolvejSector`-style derivation.

## What LiteRed actually does

### Sector-local derivation

The basis-level `SolvejSector[nm]` iterates `UniqueSectors[nm]` at
[`LiteRed2026.m:2304-2318`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2304).
Inside one sector:

- `ids` is one of freshly generated `IBPLI`, `IBP`, `LI`, or `FPIBP` at
  [`LiteRed2026.m:2377`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2377);
- the equation database is initialized for the current sector and cleaned for
  each symbolic case at
  [`LiteRed2026.m:2428-2440`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2428);
- equations are constructed at
  [`LiteRed2026.m:2475`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2475) as

  ```mathematica
  If[useSR && numeric,
    Join[ids @@ point, SR[nm] @@ point] /. ZerojRule[nm],
    ids @@ point
  ]
  ```

- a successful pivot is recentered, patternized on the current residual case,
  passed through `WhenBad`, and appended to the current sector table at
  [`LiteRed2026.m:2484-2500`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2484).

`Solvej` substitutes pivots already present in the passed database and adds a
new pivot to that database
([`LiteRed2026.m:2121-2196`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2121)).
In `SolvejSector`, that database is the table under construction for the
current sector: `initdb` creates a fresh empty database and `cleandb` resets it
for the next symbolic-variable set at
[`LiteRed2026.m:2602-2616`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2602).
No previously saved proper-subsector `jRules` table is joined to `eqs` or to
that database.

Thus the exact source statement is:

> With `SR -> True` (the default), LiteRed applies analytic zero and
> self-symmetry relations before fully numeric elimination, but it does not
> quotient `SolvejSector` equations by already solved proper-subsector
> reduction rules.

### Proper-subsector application

Proper-subsector substitution happens later:

- `IBPSelect` repeatedly applies the current sector table to reachable
  integrals and records newly reached sector dependencies at
  [`LiteRed2026.m:3801-3917`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3801).
- `IBPReduce` topologically selects sectors whose dependencies are ready, then
  explicitly substitutes the saved lower-sector tables into the current
  table at
  [`LiteRed2026.m:3960-3973`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3960).
  It subsequently layers and inlines the remaining same-sector rules at
  [`LiteRed2026.m:3974-4003`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3974).

`FindSymmetries` separately constructs `UniqueSectors`, `MappedSectors`,
mapped-sector `jRules`, self-symmetries, and `SR` around
[`LiteRed2026.m:3234-3472`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3234).
Same-rank symmetry mapping is not a proper-subsector dependency and should not
be used to justify a lower-sector ordering edge.

This corrects any broader reading of “feed solved subsectors into later
eliminations” as a literal port requirement.  Such feedback can be a sound
Symbolica-oriented enhancement, but the certificate and documentation must
label it as one.

## Current RustRed boundary

The relevant production seams are already strong:

- [`FamilySectorInventoryCertificate`](../../src/family_sector_inventory.rs)
  orders unresolved sectors by exact corner complexity and verifies that no
  later entry is a strict subsector of an earlier entry
  (`verify_unresolved_solve_order`, lines 504 onward).
- [`GeneratedFamilyRuleSystemCompiler`](../../src/generated_family_rule_system.rs)
  creates one shared authenticated generated row span, then independently
  compiles each raw unresolved sector in that order.  It does not pass earlier
  material into `compile_unresolved_sector`.
- [`GeneratedFamilyFixedPointCompiler`](../../src/generated_family_fixed_point.rs)
  constructs working states for all sectors, performs all phase-zero
  preparations, then visits every active sector in each global round.  Its
  residual search calls
  `AdaptiveParametricRuleProvider::candidate_layers_for_quotient` on the same
  raw shared row span at lines 1387--1410; no quotient is performed there.
- [`CertifiedFamilyRuleProvider`](../../src/certified_rule_provider.rs) already
  implements the correct numeric source order: specialize fresh generated
  rows, prove zero/cut/symmetry fates for every concrete term, collect, and
  eliminate exactly.  The durable proof is
  `CertifiedConcreteRewrite::from_concrete_quotient_elimination` in
  [`certified_rewrite.rs`](../../src/certified_rewrite.rs).
- [`GeneratedPartialReeliminationCompiler`](../../src/conditional_reelimination.rs)
  regenerates canonical IBP/LI rows, translates them, partially specializes
  them on a sparse equality assignment, and keeps the resulting rows and
  elimination private.  This is the correct containment model for parametric
  lower feedback.
- [`ParametricRelation::translated`](../../src/parametric_relation.rs#L756)
  already performs the essential exact operation: coefficients, exceptional
  conditions, and integral shifts are translated together.  Translating a
  centered lower rule by `s` produces an identity with left side `J(n+s)`.
- [`ParametricReductionEngine`](../../src/reduction_engine.rs) recursively
  reduces every concrete right-hand-side term, retains proof traces and all
  guards, and leaves missing rules explicitly `Uncovered`.  This is already a
  close demand-driven analogue of LiteRed's `IBPSelect`/`IBPReduce` behavior.

Two existing restrictions must remain intact:

1. `GeneratedWhenBadCompiler` and generated discovery authenticate raw
   canonical/translated/whole-row-symmetry sources.  A row changed by a lower
   rewrite is not in that raw span and must not be smuggled through this
   authenticator by manifest equality or a new unverified label.
2. Conditional re-elimination rows cannot escape as ordinary global
   `ParametricReductionRuleCandidate`s.  Lower-feedback rows need the same
   restriction.

## Algebraic boundary

Write a generated row as

```text
R(n) = sum_s c_s(n) J(n+s) = 0
```

over the exact Symbolica rational-function field `K(n)`.  Let `S` be the
parent sector and `D` a certified parent case/leaf.

### Fully concrete row

After `n=a`, every term has a concrete key `J(a+s)`.  For each term, the sound
normalization order is:

1. prove analytic zero or cut zero and erase the term;
2. otherwise canonicalize the concrete key through a verified symmetry path;
3. determine the *raw pre-symmetry* sector `T` and require `T` to be a strict
   subsector of `S` before consulting lower material;
4. if symmetry maps `T` to a canonical lower representative `U`, retain the
   complete path and require the material for `U` to be in the frozen
   predecessor snapshot;
5. recursively apply only generated global/conditional rules from that
   snapshot, with no master declarations and no current/supersector material;
6. keep every `Uncovered` terminal unchanged; and
7. multiply by the original coefficient and collect exactly.

Every applied rule is an equality, so partial normalization is sound:
uncovered leaves do not have to be guessed as masters or erased.  The output
row may then enter exact base-field elimination.

All specialized nonzero conditions, certified zero/symmetry domain
conditions, and lower application traces become conditions/provenance of the
new row and selected pivot.  Dropping them would silently enlarge the
validity domain.

The feedback provider must have the stable semantic stack

```text
zero(symmetry(conditional(global)))
```

with no nonempty master declarations.  The existing shared builder may retain
its semantically inert empty `MasterPolicyProvider` wrapper.  An explicit
master is an application policy, not a generated identity source.  A master
terminal would merely leave `J` unchanged, but allowing it into derivation
would make the derivation transcript depend on a caller's chosen basis for no
algebraic benefit.

### Why the parametric case is conditional

For an active parent coordinate, `n_i >= 1`.  If `s_i < 0`, then
`n_i+s_i <= 0` only at the finite boundary values

```text
n_i = 1, ..., -s_i.
```

For an inactive coordinate, `n_i <= 0`.  If `s_i > 0`, activation likewise
depends on finitely many boundary values.  Therefore the sector of `J(n+s)`
is generally not fixed throughout `S`.

A minimal implication checker may prove a target sector from only:

- the parent orthant;
- exact coordinate equalities retained by `PartialIndexAssignment`; and
- structural sign stability (`active + nonnegative shift` stays active and
  `inactive + nonpositive shift` stays inactive).

If any coordinate's sign remains undecided, the compiler leaves the term
unchanged.  A later version may split the parent case on the finite boundary
equalities.  It must never pick the sector observed at one concrete anchor and
apply it to the whole symbolic leaf.

Once the leaf proves the exact target sector, zero/cut quotienting precedes
lower-rule lookup, just as it does for a concrete term:

- `ZeroSectorDecision::ProvedZero` erases the term only with the complete
  replayed `ZeroSectorCertificate` and propagates every base-domain condition;
- an excluded target is erased only when the exclusion proves a cut
  violation; a pattern-only exclusion is not a zero proof and leaves the term
  unchanged; and
- resource-limited/failed zero analysis is a typed interruption, never an
  erased term.

The row proof retains a `TargetSectorMembershipWitness` derived from the
parent orthant and equality assignment, followed by the zero/cut witness.
This is leaf-bound parametric zero quotienting: it does not assert that the
shift has the same sector on another parent case.

Suppose the leaf proves that `J(n+s)` lies in proper subsector `T`, and a
committed lower candidate has centered unit relation

```text
L(m) = J(m) + sum_u b_u(m) J(m+u) = 0.
```

The exact pulled-back relation is

```text
L(n+s) = J(n+s) + sum_u b_u(n+s) J(n+s+u) = 0.
```

RustRed should construct it with `ParametricRelation::translated`, not by
editing shifts or Symbolica expressions independently.  If the coefficient
of `J(n+s)` in the parent row is `c_s(n)`, adding
`-c_s(n) L(n+s)` eliminates that term.  The lower relation's translated guard
set is unioned into the parent row before collection.

For a globally derived lower candidate, the centered relation is itself an
identity on its guarded rational-function domain even outside the region in
which it is a descending rewrite.  Nevertheless, RustRed should call this a
*solved-subsector quotient* only when the parent leaf proves lower-sector
membership and lower `WhenBad` coverage.  Adding such relations globally is
an algebraically sound source-row augmentation, but it is a different
algorithm, is redundant with an unbounded raw generated span, and loses the
termination/dependency meaning of a lower rewrite.

For a condition-bound lower pivot, its lower equality assignment must be
pulled back by `m=n+s`.  The parent assignment must imply every pulled-back
equality; otherwise the parent leaf must be split or the rewrite skipped.
The minimal implementation should initially consume only global lower
candidates whose exact `WhenBad` leaf is implied.  Conditional-to-conditional
feedback can follow after this implication/pullback path is independently
tested.

### Predicate implication policy

The first implementation does not need a general integer theorem prover.  It
can be complete for the supported fragment and fail closed elsewhere:

1. select a specific replayed lower coverage leaf with disposition
   `DescendingRule { candidate_ordinal }`, preserving the candidate priority
   and the `EqualZero`/`NonZero` kind of every predicate;
2. translate each index-locus predicate polynomial with the same Symbolica
   index substitution `m=n+s`;
3. partially specialize it by the parent coordinate assignment;
4. accept an equality if it becomes the zero polynomial, and a nonzero
   predicate if it becomes a certified nonzero constant;
5. otherwise accept only an exact normalized predicate of the same polarity
   already present in the parent case; and
6. return `NotProvedApplicable` for every other predicate.

Equivalently, a later implementation may prove that the selected candidate's
complete bad formula is false on the parent case.  It may not infer coverage
from a candidate ordinal alone.  Pure base-field nonzero assumptions and
translated rational-function guards are not index-locus predicates: they are
retained as output guards instead of being required to follow from the index
case.

`NotProvedApplicable` preserves the original row term.  It is not a failure,
zero conclusion, or master conclusion.

### Symbolic symmetry

Concrete term-wise canonicalization is sound because every index is fixed.
Generic term-wise replacement is not:

```text
J(n+s) -> J(P n + P s),
```

not `J(n+P s)`.  Therefore the minimal parametric lower-feedback layer must
not term-wise canonicalize symbolic keys.  It may use:

- existing verified whole-row symmetry transport before elimination; or
- a term-wise symmetry only on an equality locus that proves the required
  affine index equalities and retains them in the conditional certificate.

The latter is not needed for the first sunset slice.  Concrete feedback may
use the existing symmetry provider and retain the raw-lower-sector plus
symmetry-path dependency witness.

## No-circularity contract

The correctness argument is an immutable DAG, not a dynamic provider check.

### Committed material

Before processing parent sector `S`, construct a
`GeneratedSubsectorSnapshotCertificate` containing only committed material
whose dependency is proved lower than `S`.  A material may be partially
covered; “committed” means its complete bounded fixed-point attempt has
finished and its latest replayable material is immutable, not that every leaf
was solved.

For a direct dependency, require

```text
material_sector.is_strict_subsector_of(S)
```

and `material_solve_ordinal < parent_solve_ordinal`.  For a symmetry-mapped
concrete dependency, require a raw term sector `T < S`, a verified path
`T -> U`, and committed material for `U` at an earlier lower-rank barrier.
Merely having an earlier ordinal is not enough.

Never include:

- current-sector material;
- a supersector;
- a same-rank sector merely because it sorts earlier;
- mutable material from another in-progress sector; or
- a caller-selected/certified master policy.

### Atomic scheduling

`GeneratedFamilyFixedPointCompiler` currently prepares every sector and then
runs global rounds over all active sectors.  A feedback-enabled V2 compiler
should instead process strict-subsector ranks:

```text
for rank in increasing active-propagator count:
    snapshot = freeze(all committed lower ranks)
    compile every selected sector of this rank against snapshot
        (the sectors are an antichain and may run in parallel)
    replay every completed result
    sort results by the stable solve ordinal
    atomically commit the rank
```

Within each sector, run its phase-zero preparation and residual rounds to its
configured stop before the rank barrier.  This preserves parallelism for
independent sectors while preventing one same-rank task from observing
another's mutable state.  Zero/excluded inventory entries are globally
available because they are independently certified.

If unique-sector symmetry orbits are added later, orbit representatives are
chosen before this schedule.  Mapping within an orbit remains a symmetry
operation, not a lower-rank feedback edge.

Every lower application must also retain the rule's existing strict descent
witness and prove that its emitted target sectors stay within its source
sector on the applicable leaf.  This gives both a sector-rank decrease across
feedback dependencies and ordinary integral-order descent inside each
reduction.

## Proposed certificate boundary

Names below are design names, not a requirement to expose all fields publicly.

Durable derived proofs must continue to replay after the final provider and
family compiler are dropped.  A bare `GeneratedFixedPointMaterialLocator`
cannot satisfy that contract because current standalone conditional/concrete
replay receives only `(family, context)`.  Therefore V2 uses an owned,
reference-counted dependency bundle.  The family transcript owns the
canonical bundle, and every derived rule shares the same `Arc`; compact
material ids are meaningful only inside that bundle.

```rust
struct GeneratedSubsectorDependencyBundleCertificate {
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    shared_row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    materials: Box<[GeneratedCommittedSubsectorMaterial]>,
    limits: GeneratedSubsectorFeedbackLimits,
}

struct GeneratedCommittedSubsectorMaterial {
    id: GeneratedSubsectorMaterialId,
    sector: SectorMask,
    rank: usize,
    solve_ordinal: usize,
    commit_ordinal: usize,
    fixed_point: GeneratedCommittedSectorFixedPointCertificate,
}

struct GeneratedSubsectorSnapshotCertificate {
    parent_sector: SectorMask,
    parent_solve_ordinal: usize,
    bundle: Arc<GeneratedSubsectorDependencyBundleCertificate>,
    dependencies: Box<[GeneratedSubsectorMaterialReference]>,
    limits: GeneratedSubsectorFeedbackLimits,
    stats: GeneratedSubsectorSnapshotStats,
}

struct GeneratedSubsectorMaterialReference {
    raw_dependency_sector: SectorMask,
    material_sector: SectorMask,
    material_solve_ordinal: usize,
    material: GeneratedSubsectorMaterialId,
    symmetry_path: Box<[Arc<VerifiedInternalFamilyPermutationSymmetry>]>,
}
```

`GeneratedCommittedSectorFixedPointCertificate` is a V2 sector-local slice of
the phase-zero/residual history, including its final latest discovery/queue;
owning only those final two objects would authenticate their algebra but would
not prove the commit schedule or absence of circular feedback.  The bundle may
itself contain lower rules that share still-earlier-rank bundles by `Arc`; the
rank invariant makes this an acyclic DAG because a parent bundle never
contains its parent sector.  Replay first regenerates the shared row span,
then replays every owned sector history once, resolves each bundle-local id,
checks its rank/solve/commit ordinals and latest material, and validates every
strict dependency/symmetry path.  Aggregate retained-payload and DAG-edge
limits are charged before cloning.  Persistence must serialize stable
material ids and the DAG, never pointer identities.

### Concrete normalization proof

Extend the concrete quotient with an internal term-normal-form witness rather
than teaching `QuotientTermWitness::canonical` to hide a many-term rewrite:

```rust
struct GeneratedConcreteSubsectorTermWitness {
    original: ConcreteIntegralKey,
    normalized_terms: BTreeMap<ConcreteIntegralKey, Coefficient>,
    operations: Box<[GeneratedConcreteNormalizationOperation]>,
    terminal_statuses: BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>,
    required_nonzero: Box<[SpecializedNonZeroCondition]>,
    certified_domain: Box<[CertifiedRewriteDomainCondition]>,
}

enum GeneratedConcreteNormalizationOperation {
    Zero {
        requested_key: ConcreteIntegralKey,
        proof: CertifiedZeroReduction,
    },
    Symmetry {
        requested_key: ConcreteIntegralKey,
        proof: CertifiedConcreteRewrite,
    },
    LowerGlobal {
        requested_key: ConcreteIntegralKey,
        raw_requested_sector: SectorMask,
        material: GeneratedSubsectorMaterialReference,
        proof: ConcreteReduction,
    },
    LowerConditional {
        requested_key: ConcreteIntegralKey,
        raw_requested_sector: SectorMask,
        material: GeneratedSubsectorMaterialReference,
        proof: ConditionalConcreteReduction,
    },
}
```

Only the two lower variants carry a committed-material reference and must
prove a strict predecessor edge.  Recursive normalization may encounter zero
or symmetry operations inside a lower rule; forcing those traces to carry a
fictitious lower material id would corrupt the dependency proof.

Construction remains crate-private.  It regenerates the raw IBP/LI row,
specializes it, runs the restricted lower provider, collects the normalized
row, and only then invokes `ExactSparseElimination`.  Replay reconstructs the
snapshot provider and repeats the complete normalization; it must not merely
call `verify_application` on stored traces.

`ParametricReductionResult` is useful working output but should not itself be
accepted as a certificate because its provider decisions are not replayed by
that type.  The enclosing concrete quotient certificate retains the owned
dependency bundle, material ids, and application proofs needed to rebuild
those decisions deterministically.

### Leaf-bound parametric normalization proof

Extend `GeneratedPartialReeliminationCertificate` (preferably as a V2 schema)
with nested source provenance:

```rust
enum GeneratedPartialSourceAuthentication {
    CanonicalIbpLiExactTranslationsAndSparseSpecialization,
    SharedGeneratedRowSpanExactTranslationsSparseSpecializationAndSubsectorFeedback,
}

struct GeneratedParametricSubsectorRowWitness {
    parent_case: SymbolicSectorCaseId,
    parent_assignment: PartialIndexAssignment,
    raw_source: GeneratedSourceRowWitness,
    operations: Box<[GeneratedParametricRowOperationWitness]>,
    normalized_manifest: Arc<str>,
}

enum GeneratedParametricRowOperationWitness {
    AnalyticZero {
        target_shift: IndexShift,
        membership: TargetSectorMembershipWitness,
        proof: Arc<ZeroSectorCertificate>,
    },
    CutZero {
        target_shift: IndexShift,
        membership: TargetSectorMembershipWitness,
        proof: SectorExclusion,
    },
    Lower(GeneratedParametricSubsectorApplicationWitness),
}

struct GeneratedParametricSubsectorApplicationWitness {
    target_shift: IndexShift,
    membership: TargetSectorMembershipWitness,
    proved_target_sector: SectorMask,
    lower_material: GeneratedSubsectorMaterialReference,
    lower_candidate_ordinal: usize,
    pulled_predicates: Box<[PulledBackPredicateWitness]>,
    translated_lower_manifest: Arc<str>,
}
```

The actual normalized rows and elimination remain private, as they are now.
A successful pivot can be materialized only through a condition-bound rule
that owns the source parent assignment, nested feedback certificate, and the
per-pivot centered assignment.  As in the current
`ConditionalCenteredPivotLocus`, centering a pivot by `-p` transforms every
source equality `n_source[i]=a` into `n_center[i]=a+p[i]`; binding the
unshifted parent assignment directly to the centered rule would be wrong.  A
V2 pivot cannot be converted into a global
`ParametricReductionRuleCandidate`.

Do not weaken `GeneratedSourceAuthenticator`.  Add a separate derived-source
authentication arm whose root proof is the immutable shared generated row
span and whose row-operation chain is the feedback certificate.  The chain
must show, in order:

1. exact raw source authentication;
2. exact leaf specialization/predicate implication;
3. exact target-sector membership and any zero/cut operation;
4. exact lower material and descending-leaf lookup;
5. exact translation and same-leaf partial specialization of the lower
   centered relation, including all new base assumptions;
6. exact guarded row addition and collection; and
7. exact elimination, pivot centering, and centered-locus construction.

This proves that the final row lies in the localized module generated by the
raw IBP/LI identities and previously certified lower identities.  A stable
manifest is diagnostic serialization, not the proof.

## Deterministic normalization algorithm

For a parent leaf and ordered source rows:

```text
snapshot := immutable committed proper-subsector material

for raw row in authenticated row order:
    row := translate and partially specialize raw row on the parent leaf

    repeat in deterministic hardest-term-first order:
        determine target sector from parent orthant + retained equalities
        if no exact target sector is proved:
            preserve term
            continue

        analyze the proved target sector for analytic/cut zero
        if proved analytic zero or cut zero:
            stage erasure plus its complete certificate/domain conditions
            commit the staged row only after all checks and budgets pass
            continue
        if zero analysis is resource-limited or failed:
            return the typed interruption with the input row unchanged

        if the surviving target sector is not a strict subsector:
            preserve term
            continue

        choose the first specific descending lower leaf in the committed
        material's certified priority order whose polarity-preserving
        pulled-back predicates are implied
        if no such candidate exists:
            preserve term
            continue

        translate the candidate's unit centered relation to the target shift
        partially specialize that translated relation on the same parent
        assignment used for the raw row
        retain every new partial-specialization base assumption
        preflight the lower sector/descent proof, guards, sparse updates,
        coefficient bytes, and all other resource limits
        on a staged clone, add the exact multiple that cancels the target term
        union every translated/specialized guard and provenance origin
        collect exactly with Symbolica
        commit the staged row only if every check succeeds

    retain the row (or an explicit eliminated-to-zero witness)

exactly eliminate the retained normalized rows
for each pivot, recenter the source assignment by the pivot shift
compile pivots only as rules bound to those centered conditional loci
run the normal WhenBad/descent checks on each resulting rule
```

Translation alone does not impose the parent equality assignment.
`ParametricRelation::translated` must therefore be followed by
`partially_specialized_on` before its row is combined with an already
specialized source row (or, equivalently, all additions must be completed in
the unspecialized representation and the combined row specialized once).
Mixing an unspecialized lower relation into
`relation_for_bound_reelimination()` is invalid.

Every rewrite is transactional.  `NotProvedApplicable`, a guard that
specializes to zero, a non-descent result, or a resource interruption leaves
the input row byte-for-byte unchanged.  The implementation must not mutate a
row and then attempt to “skip” the failed operation.

The first implementation may make one pass over terms rather than recursively
normalizing every newly introduced term, provided it records that policy and
leaves all new terms explicit.  Recursive normalization is an optimization;
soundness comes from every witnessed equality, not from reaching a canonical
normal form.

At a fully concrete leaf, use the concrete provider/engine path instead.  A
point-local rewrite may close that leaf, but it cannot close a symbolic parent
case merely because the anchor point was covered.

## Fixed-point integration

The integration point is the live residual queue, not the shared global row
span.

1. Compile/replay the shared generated row span once.
2. Process sectors by committed strict-subsector rank.
3. Run existing global discovery for the current sector unchanged.
4. For each residual work item with an authenticated coordinate assignment,
   run feedback-aware partial re-elimination against the frozen snapshot.
5. Install successful pivots in that sector's conditional queue.
6. Recompute the exact effective residual partition/queue.  Coverage of an
   anchor is not coverage of its parent case.
7. Repeat the current sector's bounded residual fixed point.
8. Commit its final latest material only after the complete transcript
   replays.

The current call to
`AdaptiveParametricRuleProvider::candidate_layers_for_quotient` can remain as
the search for globally valid raw-span candidates.  Feedback-derived
candidates need a separate outcome/origin variant and must be composed only
into conditional material.  Passing them to
`GeneratedWhenBadCompiler::compile_with_replayed_row_span` would be an
authentication error because their immediate source rows are no longer the
raw shared span.

The current live-leaf queue is an application fallback: it retains conditional
pivots, but its root `ParametricSectorCoverageCertificate` is not rewritten to
subtract their equality loci.  A feedback-enabled V2 fixed point therefore
needs an effective-coverage overlay (for example,
`GeneratedSectorEffectiveCoverageCertificate`) that composes the nested
derived-source `WhenBad` partitions with the raw global partition.  Until that
overlay exists, feedback-derived conditional rules may be installed and used
safely, but the compiler must conservatively keep the corresponding root leaf
residual and must not claim fixed-point closure or strict residual
improvement.

The V1 fixed-point schema should keep replaying with its current all-sector
round semantics.  Introduce a V2 transcript rather than silently changing the
meaning of old round/material locators.

## Resource limits and typed outcomes

Add explicit per-operation and aggregate limits for:

- snapshot dependency references and symmetry-path entries;
- lower provider queries and rule applications;
- recursive normalization calls and active depth;
- normalized sparse updates and collected terms;
- translated lower relations and aggregate relation terms;
- pulled-back predicates and implication checks;
- retained lower application traces, bundle-local material references, and
  existing fixed-point history locators;
- owned dependency-bundle materials and DAG edges;
- retained coefficient/guard/provenance bytes;
- normalized row manifests and private normalized rows; and
- rank tasks/commits in the family transcript.

All counts use checked arithmetic and are preflighted before large clones or
Cartesian expansions.  Outcomes must distinguish:

```text
NotProvedApplicable       preserve the term
NoLowerRule               preserve the term
LowerRuleApplied          retain complete proof
ResourceLimited           typed bounded interruption
Failed                    algebra/replay inconsistency
```

Neither `NotProvedApplicable`, `NoLowerRule`, depth exhaustion, nor a lower
sector with residual leaves is evidence for a master integral.

## Minimal equal-mass sunset black-box regression

The first end-to-end test should prove that feedback happened during
derivation, not merely during the existing recursive application stage.

1. Construct the connected equal-mass sunset under an opaque family name.
   Supply no loop count, topology tag, recurrence coefficient, or hardcoded
   rule to production APIs.
2. Compile the shared generated IBP/LI row span and verified internal
   symmetries, then compile the feedback-enabled family fixed point with all
   selected sectors and bounded generic policies.
3. Replay the family certificate.
4. Inspect the public feedback provenance for parent sector `111` and require
   at least one concrete or fully assigned conditional source-row term to be
   normalized through committed lower material.  Require:
   - the raw target sector is a strict subsector of `111`;
   - its bundle-local material id resolves to an earlier committed lower-rank
     sector;
   - no application trace references sector `111` or a supersector; and
   - the source row still authenticates back to generated IBP/LI material.
5. Build the ordinary final provider with explicit application-only terminals
   `J(1,1,1)` and `J(0,1,1)`.
6. Reduce the concrete request `J(2,1,1)` and require the frozen Vakint oracle

   ```text
   J(2,1,1) = (d-3)/(3*m2) J(1,1,1).
   ```

7. Require complete reduction and replay every retained feedback proof.
   Inspect the *applied top-sector trace* and require that its committed
   pivot's nested source-authentication chain contains a successful
   strict-subsector rewrite.  A counter of attempted or even retained row
   operations is not causal evidence: the existing provider already reduces
   this `J(2,1,1)` oracle without derivation feedback.
8. If the bounded configuration cannot force that derived pivot to win the
   provider priority order, compile the identical fixture and bounds once with
   feedback disabled and once enabled, and require an exact accepted-pivot or
   residual-effective-coverage delta whose newly applied pivot owns the
   feedback chain.  Do not claim success from the unchanged oracle alone.

Concrete powers and the frozen coefficient are test/oracle data only, which
matches the project requirement.  The production compiler remains generic.

Add focused adversarial tests alongside it:

- a same-sector material id is rejected even if its solve ordinal is
  earlier;
- a proper-subsector rule whose pulled predicate is undecided leaves the term
  unchanged;
- an uncovered lower terminal survives with its coefficient;
- a vanished translated pivot guard prevents the lower application;
- a mutated material id, candidate ordinal, symmetry path, or row manifest
  fails replay;
- a leaf-proved zero/cut target is erased with its domain proof, while a
  pattern-only exclusion is preserved;
- a feedback resource limit returns a typed interruption without committing
  the parent sector; and
- sectors in the same rank observe the same frozen lower snapshot when their
  compilation is executed in parallel.

The Rust test runner should be invoked without `--test-threads=1`; independent
test binaries/cases remain parallel.  No `no_gmp` feature is part of this
design.

## Risk ledger

### High: accidentally promoting a conditional row globally

This is the main soundness risk.  Keep normalized rows private, add no public
conversion to `ParametricReductionRuleCandidate`, and make replay compare the
complete source parent assignment, per-pivot centered assignment, and
predicate pullbacks.

### High: provider leakage creates circular derivations

A general final family provider can see the current and higher sectors.  Do
not reuse it for feedback.  Build a dedicated provider from a frozen allowlist
of dependency-bundle material ids and validate the raw requested sector before
symmetry.

### High: treating a concrete anchor as a symbolic proof

A concrete quotient certifies only that point.  It may close a fully numeric
leaf or guide search, never delete a symbolic parent leaf.

### Medium: hidden guard loss

Every translated lower pivot guard, partial-specialization assumption, zero
domain condition, and symmetry-map condition must be retained and propagated
to the final pivot.  Exact coefficient equality without identical guard
provenance is not replay equality.

### Medium: symmetry confuses the partial order

The material representative after symmetry need not be a bitwise subsector of
the parent.  Retain both the raw strict-subsector witness and the complete map
to the representative.  Never infer a lower edge from the representative mask
alone.

### Medium: bounded partial material is called “solved”

Feedback may safely use certified branches from a sector that still has
residual leaves.  Name the ledger entry `CommittedSubsectorMaterial`, not a
complete solution, and preserve all uncovered terms.

### Medium: source-row explosion

Substituting full lower normal forms can grow rows dramatically.  Start with
one deterministic pass and explicit budgets.  Exact replay and useful partial
normalization are preferable to an unbounded attempt at a canonical normal
form.

## Concrete-only verdict

Concrete-only feedback is:

- sound;
- topology-independent;
- directly reusable from the existing concrete quotient and reduction-engine
  machinery;
- sufficient for a strong sunset black-box validation; and
- faithful to LiteRed's zero/self-symmetry ordering at numeric points.

It is not:

- a port of solved-lower-rule substitution inside `SolvejSector` (LiteRed has
  no such step);
- a proof of a fully parametric recurrence on a symbolic leaf; or
- sufficient for the stated goal of generic parametric IBP rule derivation.

The complete claim requires the leaf-bound parametric extension.  Conversely,
if strict LiteRed parity is the immediate milestone, the smaller and more
faithful task is to make the current recursive application into an explicit
replayable `IBPSelect`/`IBPReduce`-style family dependency and
back-substitution certificate, while leaving derivation sector-local.
