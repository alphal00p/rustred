# Affine `WhenBad` and Effective Coverage Design

Date: 2026-08-14

Status: the complete transactional target-local compiler and sequential group
effective-coverage owner are implemented and independently audited. Together
they own authenticated signed descent, ordered condition accumulation, exact
affine boundary pullbacks, mapped numerator gates, target-relative
partitioning, persisted-order target consumption, sealed applicable handles,
explicit residual work, replay, redaction, and aggregate resource accounting.
The V2 sector-effective owner, rule application, and outer family fixed point
remain pending.

## 1. Purpose and claim boundary

This document specifies the next generated residual-affine RustRed layer:

1. compile LiteRed-style `WhenBad` applicability for one private,
   split-recentered affine pivot and one exact target inventory case;
2. select and consume target cases in the same order as LiteRed;
3. retain the good part as a sealed conditional-rule handle; and
4. retain every bad or unsupported part as explicit residual work.

The implementation remains generic in topology, loop count, number of
denominators, masses, and coefficient parameters. The equal-mass two-loop
sunset is only a generated validation fixture. No recurrence may be supplied
by a topology-specific production module or copied from Vakint/FORM.

This layer does not yet claim:

- a public affine rule-application engine;
- a complete outer `SolvejSector` fixed point;
- a master integral from an unsuccessful search;
- arbitrary nonlinear or rational dependent starts;
- general Presburger or radical-ideal reasoning; or
- permission to expose a private residual-affine `ParametricRelation`.

The implemented checkpoint covers the complete target-local portion through
`GeneratedResidualAffineWhenBadCompilation` and the sequential group owner in
section 10. The local compiler consumes no target by itself; the group owner
alone performs the exact consume/no-consume transition and publishes only
sealed crate-private applicable handles. The next production stage is the V2
sector-effective owner and residual queue in section 16; concrete rule
application and the outer family fixed point follow that owner.

The licensed GMP validation checkpoints on 2026-08-20 passed `40/40` focused
target-local tests and `8/8` final group-owner tests under
`cargo nextest run -j4`, plus `cargo check --tests`. Independent reviews found
no remaining code blocker in either saved layer. The group suite includes a
generated `001` acceptance/exhaustion path, generated `011`/`101` two-root
negative paths, exact/one-below resource checks, authority-shape tampering,
and topology-free reject/accept and two-target transition models. A bounded
generated-family census found no genuine multi-target pending pivot or local
unsupported/identically-bad outcome, so those transitions are not claimed as
generated-fixture coverage. The two-loop family remains validation-only; this
result does not claim complete integer-lattice coverage or a complete
two-loop reduction.

The implementation must use licensed, GMP-enabled Symbolica. It must not use
FORM or Mathematica at runtime and must not enable Symbolica's `no_gmp`
feature.

## 2. Audited source semantics

### 2.1 LiteRed target selection and consumption

The controlling Mathematica code is `SolvejSector` in
`vendor/LiteRed2/Source/LiteRed2026.m`, especially the block around lines
2419-2522 in the audited tree.

For one gathered contiguous case group, LiteRed does the following:

1. It obtains the current group in persisted priority order with
   `cases = Reverse@First@noRules`.
2. It asks the equation solver for provisional pivots in solver order.
3. It recenters a provisional rule and selects the first exact matching case.
4. It constructs the installed condition

   ```text
   target case && !WhenBad[RHS]
   ```

5. If `WhenBad[RHS]` is literal `True`, it excludes only that provisional
   pivot. The selected target case is not removed, so a later pivot may target
   it again.
6. If `WhenBad[RHS]` is not literal `True`, LiteRed accepts the conditional
   rule and removes the complete exact target case from `cases` for the
   current group pass. This happens even when `WhenBad` has a nonempty bad
   subset.
7. The accepted target's bad subset is accumulated as

   ```text
   target case && WhenBad[RHS]
   ```

   and is reintroduced only when `noRules` is rebuilt after the current group
   pass.

Therefore target matching, target consumption, and effective coverage are
three distinct operations. Matching alone must never consume a target. A
partially applicable accepted rule consumes one target for the current pass,
while its bad children remain future residual work.

The exact Rust transition for one pending pivot `p` is:

```text
target = first(p.matching_targets - consumed_targets)

no target:
    retain NoRemainingTargetCaseForPivot

affine WhenBad is unsupported or identically true on target:
    retain the rejection
    do not insert target into consumed_targets
    do not try p's second matching target

affine WhenBad has at least one structural good child:
    accept one conditional rule
    insert target into consumed_targets
    retain all bad children as residual work
```

A later pivot falls through to the next matching target only after an earlier
pivot accepted and consumed the first one.

### 2.2 LiteRed `WhenBad`

The audited `WhenBad` definition is near lines 2565-2569 of
`LiteRed2026.m`. Its semantic order is:

1. collect all RHS integral terms and coefficients;
2. identify coefficient-denominator zero domains;
3. identify inactive-index activations caused by RHS shifts;
4. remove a boundary event only when the corresponding numerator vanishes;
5. reduce the resulting Boolean bad formula inside the sector's integer
   orthant; and
6. fail closed when the result cannot be represented safely.

The current global RustRed `src/when_bad.rs` is soundly sharper at a boundary:
instead of marking a whole boundary exceptional whenever the specialized
numerator is not identically zero, it splits numerator-zero from
numerator-nonzero children. The affine implementation should preserve that
Symbolica-native refinement.

The six steps above describe construction of LiteRed's Boolean bad domain
after an ordering-admissible provisional recurrence exists. The generated
affine layer adds a conservative admissibility gate: it must prove uniform
strict descent before beginning those zero-locus steps. This does not change
the bad formula; it prevents coefficient zero loci from rescuing a recurrence
that is not uniformly simpler under the authenticated ordering.

### 2.3 Existing Rust authentication boundaries

The following existing boundaries must remain intact:

- `GeneratedResidualAffinePendingWhenBad` publishes pivot and target metadata
  but no applicable rule.
- Its recentered relation is available only through the crate-private
  `relation_for_affine_when_bad()` seam.
- `GeneratedResidualAffinePivotTargetMatchingCertificate` owns the replay
  lineage to the generated row span, branch re-elimination, static inventory,
  and exact target matching.
- `GeneratedResidualAffineInventoryCase` owns the selected target's exact
  source cover, affine branch, composed guards, affine map, and stable locator.
- `source_branch_premises_for_provenance()` on the matcher is explicitly not
  an applicability domain. Those premises belong to the unrecentered source
  branch.

The affine `WhenBad` layer must use the selected target case's guard
composition. It must never install the source case's unrecentered common
premises as target applicability conditions.

## 3. Mathematical model

Let the selected target inventory case provide the authenticated integer map

```text
G(t) = b + A*t,
```

where `t` is represented by the original ambient variables at the map's
identity-row `free_positions`.

The pending split-recentered private relation has the form

```text
J(G(t)) + sum_s c_s(t) J(G(t) + s) = 0.
```

The zero shift is the centered pivot. Each nonzero `s` is an RHS label. The
coefficients and relation guards have already undergone the matcher-required
free-coordinate translation. They are functions of the target free
coordinates and coefficient-field parameters.

The good domain is

```text
exact target case
AND all candidate-required nonzero conditions
AND no coefficient-aware RHS leak
AND uniform strict descent.
```

The target case already includes its source Boolean terminal, affine equality
locus, source sector chamber, and composed target nonzero premises. Conditions
outside that exact target are out of scope; they are not unsupported and are
not shadowed by this attempt.

## 4. Module boundaries

### 4.1 Relation-free shared core

Extract only the pure, relation-free parts of `src/when_bad.rs` into a
crate-private core, or make equivalent helpers crate-private:

```rust
pub(crate) fn finite_boundary_hazard_range(
    source_active: bool,
    delta: i64,
    coordinate: usize,
) -> Result<Option<BoundaryHazardRange>, WhenBadCoreError>;

pub(crate) fn prove_uniform_same_sector_descent(
    sector: &SectorMask,
    rhs_ordinal: usize,
    shift: &IndexShift,
) -> Result<Result<WhenBadUniformDescentWitness, WhenBadUnsupportedReason>, WhenBadCoreError>;
```

The descent extraction is not a visibility-only change. The current global
helper allocates its excess-component vector infallibly, clones the shift,
and then converts the vector into a boxed slice. Before sharing it, make
shift and component retention fallible (or return an allocation-free core
decision), fallibly reserve the caller's witness vector, and precharge the
aggregate `rhs_count * ambient_arity` witness-component envelope. Add a
dedicated aggregate component limit/statistic; a per-witness limit is not a
substitute.

Add a separate crate-private `direct_bad_formula` module with an
owner-independent, allocation-free tri-valued router. Its route must report
the first true clause ordinal or the first unresolved `(clause ordinal,
atom)`, not merely `Bad`/`Split`, because target-relative exceptional leaves
must retain their exact condition or pullback cause. Keep global orthant
state, coordinate pruning, divisibility caches, and coverage statistics in
`parametric_sector_coverage.rs`; affine leaves are relative to the selected
target map and must not reuse global `GlobalCaseState`.

An optional further shared seam is a generic bounded accumulator for
deduplicated nonzero polynomials and their source records.

Do not share or expose:

- `WhenBadCertificate`;
- `WhenBadCandidateBinding`;
- `ParametricReductionRuleCandidate`;
- the current global `boundary_polynomial` helper;
- a raw centered or recentered relation; or
- a public arbitrary algebraic affine-`WhenBad` entry point.

The current `WhenBadCertificate::candidate()` publication boundary makes it
unsuitable as an affine wrapper around the private pending relation.

### 4.2 `generated_residual_affine_when_bad.rs`

This module compiles one exact tuple:

```text
(Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
 pending pivot ordinal,
 selected target inventory case ordinal).
```

There is no constructor accepting an arbitrary `ParametricRelation`.

Its result is target-relative. The root is implicitly the exact authenticated
inventory case; local splits refine only that root.

### 4.3 `generated_residual_affine_group_effective_coverage.rs`

This module:

- iterates matcher outcomes in persisted pivot order;
- owns the consumed-target set;
- invokes the target-local affine `WhenBad` compiler;
- records accepted and rejected attempt transitions;
- produces one final disposition for every case in the matcher's exact affine
  geometry group; and
- retains accepted good leaves and residual bad leaves separately.

The static inventory remains immutable. Matcher statistics must continue to
report zero consumed targets because matching is a pre-`WhenBad` operation.

### 4.4 Target-relative case partition

Do not treat target free coordinates as a new standalone sector orthant.
Dependent rows of `G(t)` need not have the same chamber meaning as their
ambient variable names outside the target equality locus.

Use a relative structural partition whose root authority is the target
inventory case:

```rust
pub struct AffineWhenBadRelativeCaseId(u64);

pub struct AffineWhenBadRelativePredicate {
    kind: SymbolicPolynomialPredicateKind,
    polynomial: ParametricPolynomial,
}

pub struct AffineWhenBadRelativeSplit {
    ordinal: usize,
    parent: AffineWhenBadRelativeCaseId,
    polynomial: ParametricPolynomial,
    equal_zero_child: AffineWhenBadRelativeCaseId,
    nonzero_child: AffineWhenBadRelativeCaseId,
}

pub struct AffineWhenBadRelativeCase {
    id: AffineWhenBadRelativeCaseId,
    predicates: Box<[AffineWhenBadRelativePredicate]>,
}

pub struct AffineWhenBadRelativePartitionCertificate {
    schema: &'static str,
    context_fingerprint: Arc<str>,
    target_case_ordinal: usize,
    target_locator: GeneratedResidualAffineCaseLocator,
    splits: Box<[AffineWhenBadRelativeSplit]>,
    cases: Box<[AffineWhenBadRelativeCase]>,
    limits: AffineWhenBadRelativeCaseLimits,
    stats: AffineWhenBadRelativeCaseStats,
}
```

These are opaque authority/certificate types, not a promise that their exact
polynomial payloads are publicly inspectable. Exact predicates, source
associations, and split payloads remain crate-private. Public inspection uses
redacted predicate ordinals, predicate kinds, and leaf/pullback classes. In
particular, do not derive a payload-printing `Debug` implementation for the
certificate merely because the outer certificate type is public.

The root has no new predicates. Its full semantics come from the retained
target cover, target Boolean terminal, target affine branch, and target guard
composition. Each split replaces one live relative parent with complementary
`p=0` and `p!=0` children, giving structural disjointness and conservation
relative to that target.

## 5. Binding and authentication

### 5.1 Local binding

The local certificate should retain a compact binding such as:

```rust
pub struct GeneratedResidualAffineWhenBadBinding {
    source_case_ordinal: usize,
    source_group_ordinal: usize,
    pivot_ordinal: usize,
    target_case_ordinal: usize,
    target_position_in_matching_list: usize,
    target_locator: GeneratedResidualAffineCaseLocator,
    target_ordinal_within_group: usize,
    sector: SectorMask,
    coefficient_translation: IndexShift,
    key_center: IndexShift,
    target_ordering_manifest: Arc<str>,
    private_relation_manifest_bytes: usize,
}
```

The persisted target-list position is part of the binding, not a derived
convenience: local replay must prove that the same target occurs at the same
position in the pending pivot's priority-ordered matching list. It must not
sort or deduplicate that list.

The private relation manifest or digest may be retained privately for replay,
but no public accessor may reconstruct or borrow the relation.

Fresh compilation must verify:

1. family and context match the matcher and inventory;
2. matcher replay succeeds;
3. the outcome at `pivot_ordinal` is `PendingAffineWhenBad`;
4. the target ordinal occurs in that pending outcome's persisted matching
   list;
5. the target belongs to `source_group_ordinal`;
6. target constants equal the pending transformed target constants;
7. target cover, branch, and guard-composition allocations agree with the
   inventory case;
8. target branch outcome is a guarded affine map;
9. target guard composition is noncontradictory;
10. the private relation contains the zero centered pivot with unit
    coefficient;
11. all shifts have the ambient arity; and
12. every index variable surviving in a selected-target free-index guard, a
    recentered relation guard, or a normalized coefficient numerator or
    denominator is one of the selected target map's authenticated free
    positions.

Malformed authenticated payload is a hard error, not an unsupported
mathematical case.

The last check is defense in depth. Matcher lineage is expected to imply the
free-position property, but the local compiler must validate it directly
before using those polynomials as target-relative predicates. The same check
is repeated on every composed boundary result. A nonfree index surviving
either check is a hard authority/algebra mismatch, not
`BoundaryPullbackNotRepresentable`.

### 5.2 Target-specific ordering binding

Construct an `AffineStartParametricEliminationOrdering` from the selected
target's cover and branch using the inventory queue's ordering policy. Retain
its stable manifest in every descent witness or in a shared certificate field.

Do not reuse only the source re-elimination ordering manifest. Cases in one
geometry group share `A` and free-position order, but target constants `b` and
branch identity remain target-specific.

The V1 signed descent proof is valid only for
`IntegralOrderingPolicy::RustRedUnshiftedV1`. Any other policy must produce a
typed unsupported result unless it supplies its own authenticated descent
proof.

### 5.3 Mandatory local compiler phase order

The target-local compiler must preserve this phase order:

1. authenticate the matcher tuple, selected target, private relation, target
   ordering policy, RHS order, and arities;
2. census, precharge, and fallibly reserve all descent witnesses and their
   aggregate `rhs_count * ambient_arity` component storage;
3. prove uniform strict descent for every nonpivot RHS in retained relation
   order, returning the first typed unsupported outcome if any proof fails;
4. only after all RHS terms pass descent, collect and classify inherited and
   candidate-required conditions, including zero-polynomial outcomes; and
5. only then count hazards, build pullbacks and numerator gates, and compile
   the direct bad formula and relative partition.

In particular, a zero candidate guard, a zero coefficient numerator, or any
other zero-locus fact must not rescue a provisional pivot whose RHS ordering
is not uniformly strict. A fixture containing both a failed descent witness
and an identically-zero required condition must therefore return
`Unsupported`, not `IdenticallyBad` or `Certified`.

## 6. Domain conditions and target guards

### 6.1 Typed condition records and publication boundary

Retain each deduplicated required nonzero polynomial with all exact sources in
a crate-private replay payload:

```rust
pub enum AffineWhenBadConditionScope {
    InheritedTargetPremise,
    CandidateRequired,
}

pub(crate) enum AffineWhenBadConditionSourcePayload {
    TargetBranchGuard {
        entry_ordinal: usize,
        structural_locus_ordinal: usize,
    },
    RecenteredRelationGuard {
        guard_ordinal: usize,
    },
    CoefficientDenominator {
        term: AffineWhenBadRelationTerm,
        shift: IndexShift,
    },
}

pub enum AffineWhenBadRelationTerm {
    Pivot,
    Rhs { rhs_ordinal: usize },
}

pub(crate) struct AffineWhenBadDomainConditionReplayPayload {
    polynomial: ParametricPolynomial,
    sources: Vec<AffineWhenBadConditionSourcePayload>,
    scope: AffineWhenBadConditionScope,
    index_dependent: bool,
}
```

The term locator is typed because denominator discovery scans every relation
coefficient, including the centered pivot. An `rhs_ordinal` alone cannot
authenticate the pivot-denominator source.

`TargetBranchGuard::structural_locus_ordinal` is the original ordinal carried
by the authenticated target guard-composition entry. It is not an ordinal in
the new target-local structural-locus table and must not be rewritten when
conditions are canonicalized.

Public inspection is deliberately redacted to ordinal and class information:

```rust
pub enum AffineWhenBadConditionSourceView {
    TargetBranchGuard {
        entry_ordinal: usize,
        structural_locus_ordinal: usize,
    },
    RecenteredRelationGuard {
        guard_ordinal: usize,
    },
    CoefficientDenominator {
        term: AffineWhenBadRelationTerm,
    },
}

pub struct AffineWhenBadDomainConditionView {
    ordinal: usize,
    scope: AffineWhenBadConditionScope,
    index_dependent: bool,
    sources: Vec<AffineWhenBadConditionSourceView>,
}
```

The public view has no raw condition polynomial, coefficient shift, or
private-relation borrow. Its retained vector is illustrative; construction
must still use bounded fallible allocation. Exact payload access remains
crate-private for replay and a future authenticated application engine.

Process condition inputs in one deterministic order: selected-target guard
entries, recentered relation guards, the centered-pivot denominator, and then
RHS denominators in retained relation order. Deduplication first uses exact
polynomial equality and then bounded coefficient-field-associate recognition
under the existing coverage policy. The first representative wins and every
source is merged under explicit origin, comparison, and retention limits.

Retain a bounded crate-private typed input transcript in that encounter order,
including inputs which classify as discharged nonzero constants. Canonical
condition rows alone cannot replay either those discharged inputs or their
position relative to retained sources. A compact transcript may redact raw
polynomials and shifts from `Debug`, but it must preserve the input class,
typed source locator, classification, and selected canonical-row ordinal when
one exists. Public views remain row-oriented and redacted.

Deduplication across scopes uses **inherited dominance**. If any source of one
canonical row is an inherited target premise, the row's final scope is
`InheritedTargetPremise`, even when candidate-required sources were seen
first. Candidate provenance remains attached to the private row, but that row
does not emit a candidate-failure clause: the exact target already proves it
nonzero. A candidate-only canonical row remains `CandidateRequired`.

Charge the condition-input limit before deduplication, and charge the unique-
row and source-retention limits only when their respective payloads are about
to be retained. An arbitrarily long stream of associate duplicates must not
bypass resource limits merely because it produces one canonical row.

### 6.2 Inherited target premises

Process every selected target guard-composition entry:

- `Contradiction`: hard replay/inventory invariant failure; actionable
  inventory cases must not contain it.
- `DischargedNonzeroIntegerConstant`: retain only its source entry if required
  for replay census; no condition or split.
- `BaseAssumption`: retain as a formal assumption in `K`; no index split.
- `FreeIndexDependent`: retain as an inherited nonzero target premise. It is
  already guaranteed by the exact target Boolean terminal and must not be
  redundantly treated as a candidate failure outside that target.

The eventual sealed rule handle must carry the inherited target premises and
base assumptions through its target-case authority.

### 6.3 Candidate-required conditions

Collect:

1. every nonzero guard retained by the private recentered relation; and
2. the normalized denominator condition of every relation coefficient,
   including the centered pivot coefficient.

Scanning coefficient denominators remains required even if relation insertion
normally discovered equivalent guards. Deduplication preserves both source
records and prevents duplicate splits.

For a candidate-required polynomial:

- zero means the candidate is identically bad on the target;
- a nonzero base-only polynomial is a formal coefficient-field assumption;
- a nonzero index-dependent polynomial contributes a bad atom `p(t)=0`.

Base-only assumptions must never become branches in the index partition.

## 7. Affine boundary pullback

### 7.1 Hazard enumeration

For every nonzero RHS shift `s`, reuse the current finite hazard-range logic
for each ambient coordinate `i`.

The source value `n_i=v` is dangerous when:

- an inactive source line shifted upward would activate; or
- adding the fixed `i64` shift would exceed RustRed's concrete integral-key
  representation near an integer boundary.

Active-line pinches are lower-sector targets and are not leaks.

Hazard handling is a mandatory two-pass operation:

1. **Census pass:** inspect finite ranges without materializing individual
   events. Check each range count, each per-RHS sum, and the candidate-wide
   sum with checked arithmetic. Enforce all per-RHS and aggregate event,
   composition, witness, retained-polynomial, numerator-copy, and byte limits,
   then fallibly reserve the outer storage.
2. **Construction pass:** enumerate the already-counted values in deterministic
   RHS/coordinate/value order. For each event, construct `n_i-v`, preflight its
   composition under limits derived from the still-unspent aggregate budget,
   reserve the exact prospective output envelope, and only then enter native
   algebra and retain the event.

The two passes must use the same inclusive range iterator or an equivalent
checked endpoint rule, and the construction pass must finish with exactly the
censused count. Arithmetic/count failure is a hard error. No event allocation,
numerator deep copy, or Symbolica call is permitted during the census pass.

### 7.2 Pullback construction

Compile exactly one source-neutral residual-affine composition plan from the
selected target branch's exact integer-system `Arc`. Authenticate pointer
identity between the plan's certificate and that selected target allocation.
Do not use a source-case, group-anchor, legacy unit-affine, or per-pullback
plan; target constants are target-specific. The one plan is reused for every
boundary preflight and composition, and its construction statistics are
retained for replay.

For each dangerous value `v` in the construction pass:

1. construct the ambient polynomial `n_i-v` using the existing parametric
   coefficient context;
2. preflight its composition under the remaining aggregate budget;
3. compose it through the target plan; and
4. retain

   ```text
   B_{s,i,v}(t) = G_i(t)-v.
   ```

This is the boundary polynomial. The global `n_i-v` polynomial is not valid
on a dependent target unless it is pulled back through `G`.

Preflight and the selected Symbolica composition backend must use the same
plan and the same source polynomial. Validate selected-backend statistics
against the preflight envelope before
committing outer aggregate statistics. After composition, validate again that
all index variables in `B_{s,i,v}` belong to the target's authenticated free
positions.

The exact pullback replay record is crate-private:

```rust
pub enum AffineBoundaryPullbackClass {
    EmptyBoundary,
    WholeTarget,
    FreeIndexDependent,
}

pub(crate) struct AffineBoundaryPullbackReplayPayload {
    ordinal: usize,
    rhs_ordinal: usize,
    rhs_shift: IndexShift,
    kind: WhenBadBoundaryHazardKind,
    ambient_coordinate: usize,
    boundary_value: i64,
    pullback: ParametricPolynomial,
    class: AffineBoundaryPullbackClass,
    numerator_gate: AffineWhenBadNumeratorGatePayload,
}
```

Public access is an ordinal/class view only:

```rust
pub struct AffineBoundaryPullbackView {
    ordinal: usize,
    rhs_ordinal: usize,
    hazard_class: WhenBadBoundaryHazardKind,
    pullback_class: AffineBoundaryPullbackClass,
    numerator_gate_class: AffineWhenBadNumeratorGateClass,
}
```

The public view deliberately omits the RHS shift, ambient boundary
coordinate/value, pullback polynomial, and numerator polynomial. Those exact
objects are replay/application evidence, not public relation metadata.

Classify the pullback in `K[t]`:

- nonzero base-only pullback: `EmptyBoundary`; omit its bad event;
- zero pullback: `WholeTarget`; the boundary atom is identically true;
- index-dependent pullback: retain the ordinary equality/nonzero split.

### 7.3 Numerator gate

Let `N_s(t)` be the exact numerator condition of the already mapped and
recentered coefficient `c_s(t)`.

Do not specialize its original ambient variable `n_i` to `v`. That variable
may be absent or dependent after affine substitution. Instead retain the exact
Boolean event

```text
B_{s,i,v}(t) = 0 AND N_s(t) != 0.
```

The three exact branches are:

```text
B != 0:             continue
B = 0 AND N = 0:   continue because this RHS term vanishes
B = 0 AND N != 0:  exceptional leak
```

Use this exact crate-private payload together with a public class enum:

```rust
pub(crate) enum AffineWhenBadNumeratorGatePayload {
    CoefficientFieldNonzero(ParametricPolynomial),
    FreeIndexNonzero(ParametricPolynomial),
}

pub enum AffineWhenBadNumeratorGateClass {
    CoefficientFieldNonzero,
    FreeIndexNonzero,
}
```

A nonzero base-only numerator makes the whole nonempty boundary exceptional.
An index-dependent numerator creates the zero/nonzero refinement. If `B` is
identically zero, the event reduces to `N!=0`; if both `B` is identically zero
and `N` is a coefficient-field nonzero element, the complete candidate is
identically bad.

Using the complete mapped numerator is mathematically exact. It may retain a
structurally empty `B=0, N!=0` child when `N` lies in the ideal generated by
`B`; it cannot create false applicability. Existing bounded exact
divisibility reasoning may prune the common principal-divisibility case
without requiring a general ideal solver.

Copy `N_s(t)` from the normalized coefficient that is already mapped and
recentered. Do not specialize an ambient coordinate to the hazard value and
do not compose the numerator through the target plan a second time. A
nonzero RHS coefficient is already authenticated, so a zero normalized
numerator at this point is a hard invariant failure.

## 8. Uniform descent

Every nonpivot RHS shift must have a uniform strict descent witness before the
candidate can be accepted.

For the RustRed unshifted order, within one source chamber the relevant
complexity deltas depend only on the ambient shift:

1. signed corner-distance delta;
2. signed dot-power delta;
3. signed numerator-power delta; and
4. signed per-coordinate excess deltas.

The first nonzero component must be negative. A positive first nonzero
component gives `NonUniformSameSectorDescent`; all-zero components give
`ZeroSameSectorComplexityDelta`.

Boundary events remove inactive activations before the witness is used.
Active pinches are lower-sector targets. Thus the relation-free proof from
global `WhenBad` remains valid, but each affine witness must also bind the
selected target ordering manifest:

```rust
pub(crate) struct AffineWhenBadDescentReplayPayload {
    shift_witness: WhenBadUniformDescentWitness,
    target_ordering_manifest: Arc<str>,
}
```

This is a semantic statement about the domain on which a successful witness
will later be applied; it does not require constructing boundary events before
proving descent. Compiler phase order remains Section 5.3.

A public descent view may report the RHS ordinal, ordering-policy identity,
and decisive component/delta class. It must not expose the exact RHS shift;
the shift-bearing witness remains private replay evidence.

V1 follows LiteRed's conservative ordering: if any retained RHS fails the
uniform proof, reject the whole provisional pivot before collecting or using
condition or coefficient zero loci to rescue it. This is the phase boundary
specified in Section 5.3, not merely a preference in relative split order.

This V1 result is only a conservative checkpoint, not the final completeness
claim.  A shift can fail the global same-sector proof while the authenticated
target affine map fixes an active coordinate at its pinch value on every
target point.  Such an RHS is uniformly lower-sector on that exact target and
must not remain terminally `Unsupported` in a LiteRed-parity implementation.
The later boundary-aware upgrade must therefore distinguish
`NonDescendingInGlobalOrthant` from a final rejection, prove any
target-universal active pinch from the authenticated affine constants and
rows, and accept that RHS as lower-sector without consulting coefficient or
condition zero loci.  Until that upgrade is implemented and validated, the
local compiler may publish only a clearly staged V1 unsupported outcome and
must not claim complete target-local LiteRed coverage.

## 9. Direct bad formula and relative compilation

The semantic bad formula is an OR of candidate-domain failures and exactly
one leak clause per boundary event:

```text
Bad(t) =
    OR_j [candidate_required_guard_j(t) = 0]
    OR
    OR_e Leak_e(t).
```

For one boundary pullback `B_e(t)` and mapped numerator `N_e(t)`, the leak
clause is selected by the already authenticated pullback and numerator
classes:

```text
EmptyBoundary:
    Leak_e = False

WholeTarget + coefficient-field-nonzero N_e:
    Leak_e = True

WholeTarget + free-index N_e:
    Leak_e = [N_e(t) != 0]

FreeIndexDependent B_e + coefficient-field-nonzero N_e:
    Leak_e = [B_e(t) = 0]

FreeIndexDependent B_e + free-index N_e:
    Leak_e = [B_e(t) = 0 AND N_e(t) != 0].
```

One event contributes exactly one clause or one constant result. In
particular, never insert both `B_e=0` and
`B_e=0 AND N_e!=0`: the first clause would absorb the second and erase the
numerator-zero rescue branch. Literal-false events are omitted; a
literal-true event makes the provisional pivot identically bad.

Use a retained direct formula rather than treating one local split order as
the semantics:

```rust
struct AffineWhenBadAtom {
    locus_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
}

enum AffineWhenBadClause {
    CandidateRequiredGuardZero {
        condition_ordinal: usize,
        guard_zero: AffineWhenBadAtom,
    },
    CoefficientFieldLeakBoundaryZero {
        pullback_ordinal: usize,
        boundary_zero: AffineWhenBadAtom,
    },
    FreeIndexLeak {
        pullback_ordinal: usize,
        boundary_zero: AffineWhenBadAtom,
        numerator_nonzero: AffineWhenBadAtom,
    },
    WholeTargetFreeIndexLeak {
        pullback_ordinal: usize,
        numerator_nonzero: AffineWhenBadAtom,
    },
}

struct AffineWhenBadFormula {
    clauses: Box<[AffineWhenBadClause]>,
    atom_count: usize,
}
```

Each owner-specific clause maps allocation-free to a generic one-atom or
two-atom direct clause. The shared tri-valued router must return both the
route and the clause ordinal, so an exceptional affine leaf retains its exact
condition or pullback provenance. The existing private direct-formula
tri-evaluator in `parametric_sector_coverage.rs` is the semantic model:

- any true bad disjunct classifies the current relative leaf exceptional
  without splitting irrelevant earlier unknown clauses;
- all false clauses classify it applicable; and
- otherwise split on the first unresolved atom under deterministic ordering.

Seed formula evaluation with inherited target nonzero premises. Candidate
conditions and pullbacks are added to one target-local structural-locus table
under bounded exact associate recognition.

Insert canonical condition loci first in condition order, followed by
boundary and free-index numerator loci in pullback order. The first exact or
coefficient-field-associate representative owns the local locus ordinal;
later users retain that ordinal and their own condition/pullback provenance.
Inherited target facts seed the canonical locus as known nonzero without
emitting a candidate-failure clause. Preserve one owner-level clause per
nonfalse boundary event even when two event formulas share loci, because the
decisive pullback ordinal is replay evidence.

Relative leaf dispositions are:

```rust
pub enum AffineWhenBadRelativeLeafDisposition {
    Applicable,
    ExceptionalDomain { condition_ordinal: usize },
    ExceptionalLeak { pullback_ordinal: usize },
}
```

Compilation outcomes are:

```rust
pub enum GeneratedResidualAffineWhenBadCompilation {
    Certified(GeneratedResidualAffineWhenBadCertificate),
    IdenticallyBad(GeneratedResidualAffineWhenBadIdenticallyBad),
    Unsupported(GeneratedResidualAffineWhenBadUnsupported),
}
```

`Certified` requires at least one structurally applicable relative leaf.
`IdenticallyBad` retains a replayable reason, such as a zero required guard or
a universal coefficient-nonzero leak. `Unsupported` retains an unimplemented
proof boundary. Neither outcome consumes a target by itself.

A relation with no nonpivot terms is valid: it is a conditional zero rule if
its domain is nonempty. It is not a descent failure.

## 10. Sequential group effective coverage

### 10.1 Attempt records

Retain every matcher outcome and transition:

```rust
pub struct GeneratedResidualAffineTargetAttempt {
    attempt_ordinal: usize,
    pivot_ordinal: usize,
    selected_target_case_ordinal: Option<usize>,
    outcome: GeneratedResidualAffineTargetAttemptOutcome,
}

pub enum GeneratedResidualAffineTargetAttemptOutcome {
    MatcherRejectedNoTarget,
    MatcherRejectedRecenteringBoundary,
    NoRemainingTargetCase,
    WhenBadUnsupported(Arc<GeneratedResidualAffineWhenBadUnsupported>),
    WhenBadIdenticallyTrue(Arc<GeneratedResidualAffineWhenBadIdenticallyBad>),
    Accepted {
        certificate: Arc<GeneratedResidualAffineWhenBadCertificate>,
    },
}
```

For a pending matcher outcome, the effective compiler must call the pending
object's persisted first-available-target operation. It must not sort, dedup,
or reinterpret the target list.

### 10.2 Final target dispositions

Every case in the matcher's exact source group receives one final state:

```rust
pub enum GeneratedResidualAffineGroupTargetDisposition {
    Consumed {
        accepted_attempt_ordinal: usize,
        when_bad: Arc<GeneratedResidualAffineWhenBadCertificate>,
    },
    Unconsumed {
        rejected_attempt_ordinals: Box<[usize]>,
    },
}
```

An accepted local certificate partitions its target root into applicable and
exceptional leaves. The target is consumed exactly once, while exceptional
leaves remain residual. An unconsumed target remains one complete residual
root.

### 10.3 Effective set equations

Let `a(T)` be the one accepted attempt for a consumed target `T`. Then:

```text
effective safe coverage
  = disjoint_union over consumed T of [T AND NOT Bad_a(T)]

residual work
  = disjoint_union over unconsumed T of T
    disjoint_union
    disjoint_union over consumed T of [T AND Bad_a(T)].
```

Do not take the union of every provisional pivot's good set. Rejected pivots
do not change effective coverage. After acceptance, later pivots cannot cover
the accepted target's bad subset during the same group pass.

The outer fixed-point layer will later convert residual relative leaves into
new work, reduce/regroup them, and rerun the generated search. This document
does not authorize declaring any residual root or leaf a master.

### 10.4 Sealed rule publication

Only `Applicable` leaves of a consumed target may produce a sealed
conditional-rule handle. Public accessors may expose:

- family/context fingerprints;
- pivot and target ordinals;
- target locator and ordering identity;
- redacted condition, structural-locus, pullback, and leaf ordinal/class
  views;
- RHS term count; and
- replay/statistics metadata.

They must not expose `&ParametricRelation`, `ParametricReductionRuleCandidate`,
an unauthenticated coefficient/shift iterator, or exact private predicate
polynomials. A future application engine may use a crate-private accessor
after it has authenticated the target point and effective leaf.

### 10.5 Public redaction and private replay payloads

Public outcomes are opaque authority objects with custom redacted views and
custom redacted `Debug`. They must not jointly publish the three ingredients
that reconstruct private relation structure:

1. exact RHS shifts;
2. exact denominator-condition/source payloads, especially a denominator
   source paired with its shift; and
3. exact numerator-gate polynomials or their raw coefficient association.

The normative public surface is ordinals, counts, scopes, predicate kinds,
hazard/pullback/gate classes, outcome reasons, locators, fingerprints, and
bounded statistics. The complete shift-bearing condition sources, pullback
polynomials, numerator gates, formula atoms, and relation manifest remain in
crate-private replay payloads. A crate-private future rule-application path
may borrow them only after authenticating the retained target authority and
an `Applicable` leaf.

Do not derive `Debug` on a public outcome if it recursively formats private
fields. Its manual implementation must print only the redacted views and
counts above. Likewise, public replay failure text identifies the failing
ordinal/class and stage, while exact private payload differences remain
internal.

## 11. Typed unsupported outcomes and hard errors

### 11.1 Retained completeness boundaries

The target remains available after these outcomes:

```rust
pub enum GeneratedResidualAffineWhenBadUnsupportedReason {
    NonUniformSameSectorDescent {
        rhs_ordinal: usize,
        first_nonzero_component: WhenBadDescentComponent,
        delta: i128,
    },
    ZeroSameSectorComplexityDelta {
        rhs_ordinal: usize,
    },
    UnsupportedOrderingPolicy {
        policy_id: Arc<str>,
    },
    BoundaryPullbackNotRepresentable {
        rhs_ordinal: usize,
        hazard_ordinal: usize,
    },
    GeneralCongruenceCaseNotSupported,
}
```

These are public reason views. The exact shift, coordinate, boundary value,
and algebraic payload are retained in a crate-private unsupported replay
record. Public formatting must not recover them.

For currently actionable inventory maps, affine pullback should normally be
representable. The explicit variant preserves a future proof boundary; a
malformed current certificate remains a hard error.

`NoRemainingTargetCaseForPivot` is an effective-attempt outcome, not a local
algebraic failure.

### 11.2 Identically bad reasons

Examples include:

```rust
pub enum GeneratedResidualAffineWhenBadIdenticallyBadReason {
    RequiredNonzeroConditionIsZero { condition_ordinal: usize },
    UniversalCoefficientNonzeroLeak { pullback_ordinal: usize },
    NoStructurallyApplicableRelativeLeaf,
}
```

These mirror LiteRed's `WhenBad === True` transition: exclude this pivot, do
not consume the target.

### 11.3 Hard errors

Hard errors abort construction transactionally and publish no partial target
consumption:

- schema, family, context, or arity mismatch;
- matcher/inventory/target allocation mismatch;
- target not present in the pending matching list;
- target in the wrong group;
- malformed target geometry;
- missing or nonunit centered pivot;
- malformed private relation;
- boundary or descent arithmetic overflow;
- resource count overflow or resource limit;
- bounded allocation failure;
- Symbolica panic at a named stage;
- exact-algebra or child-certificate error; and
- replay mismatch.

No hard error or unsupported outcome is a master-integral certificate.

## 12. Resource model

### 12.1 Local affine `WhenBad` limits

The local limit structure should contain nested limits for:

- parametric arithmetic;
- target affine ordering;
- residual-affine composition-plan construction;
- one polynomial composition; and
- target-relative case splitting.

It must also contain aggregate caps for:

- family/context fingerprint bytes and comparison bytes;
- ambient arity, free positions, and map entries inspected;
- private relation terms, guards, origins, and manifest bytes;
- RHS terms, descent witnesses, and aggregate retained descent components;
- target guard entries and inherited conditions;
- candidate condition inputs, unique canonical conditions, source references,
  and origins;
- boundary values per RHS and across the candidate;
- pullback compositions and leak witnesses;
- source polynomial terms and exponent entries;
- expanded-contribution and output-term bounds;
- output exponent entries;
- power calls, native heap pairs, multiplication pairs, and addition visits;
- native and aggregate integer-bit work;
- bad clauses and atoms, plus cumulative formula-clause visits and atom
  truth queries across all relative leaves;
- structural loci; exact-equality, associate, and divisibility checks; and
  polynomial term-pair and exponent-entry comparison visits;
- relative splits, live leaves, predicate instances, and classifications;
- retained polynomial terms, exponent entries, integer bits, and display bytes;
- complete certificate-owned retained bytes; and
- payload-comparison units, bytes, integer bits, and private manifest bytes.

Composition allowances are cumulative. Before each Symbolica operation,
derive an effective nested limit from the unspent aggregate allowance.
Preflight prospective expansion before invoking native substitution or power
operations.

Term/check counts do not by themselves bound sparse-polynomial comparison.
Every exact-equality, canonical-deduplication, associate, divisibility, and
replay comparison must also charge exponent-entry visits. A one-term
polynomial can still carry an ambient-arity exponent vector. Preflight the
comparison with checked term-pair and exponent-entry bounds before reading
the payload, and commit the measured comparison statistic only after the
operation succeeds.

Every nested child receives limits derived from the caller's still-unspent
aggregate allowance, never a fresh copy of the original top-level maxima.
This applies to composition-plan construction, each pullback preflight and
composition, associate/divisibility helpers, and the target-relative formula
compiler. Validate child statistics against the derived envelope, charge them
to the outer statistics, and only then retain the child certificate. The
relative compiler's formula visits, atom queries, loci, comparisons, splits,
predicates, retained polynomial shapes, and bytes are part of the same local
aggregate budget.

### 12.2 Group effective-coverage limits

The outer group layer separately caps:

- matcher outcomes inspected;
- pending target selections;
- checked and matching target references;
- local affine-`WhenBad` compilations;
- accepted and rejected attempts;
- consumed targets;
- rejected-attempt references per target and in aggregate;
- aggregate child source terms, output terms, integer work, leaves, and bytes;
- group target dispositions;
- sealed conditional-rule handles;
- residual relative leaves; and
- outer payload-comparison work and retention.

A per-child limit must not reset an exhausted group aggregate budget.

### 12.3 Transactional order

For each operation:

1. validate source metadata and counts;
2. calculate prospective term, exponent, integer-bit, origin, and byte bounds;
3. check the remaining aggregate limits;
4. reserve fallible Rust storage;
5. enter Symbolica only after all available preflights pass;
6. validate the returned exact payload; and
7. commit stats and retained state.

Do not finish a transaction with an infallible deep polynomial/shift clone or
a shrink-to-fit conversion of user-sized storage. Use precharged fallible
copies and retain the reserved vector when a shrink could allocate.

The consumed-target set is updated only after a complete local certificate
has been built and authenticated as non-identically-bad.

## 13. Replay requirements

### 13.1 Local replay

A local certificate replay must:

1. validate its schema and scope fingerprints;
2. replay the retained matcher;
3. resolve the same pending pivot outcome;
4. resolve the same target ordinal and exact target-list position;
5. replay the target cover, branch, and guard composition;
6. rebuild the target-specific ordering and compare its manifest;
7. privately recover the recentered relation;
8. revalidate the centered unit pivot, RHS order, arities, and target
   free-position restriction;
9. recensus and rebuild every descent witness, reproducing an unsupported
   result before any condition-zero or boundary reasoning;
10. recompile domain conditions, inherited-dominance canonicalization, and
    exact private source provenance;
11. repeat the two-pass hazard census/construction discipline using one newly
    authenticated target integer-system plan, rebuilding every pullback and
    numerator gate and revalidating free-position support;
12. rebuild the direct bad formula and relative split transcript;
13. rebuild all leaf dispositions and statistics; and
14. perform checked complete private-payload equality under comparison-term,
    exponent-entry, byte, integer-bit, and manifest limits.

Replay compares exact crate-private payloads. Equality of public redacted
views is necessary but not sufficient, and replay mismatch diagnostics must
not print the private operands.

### 13.2 Group replay

The group effective-coverage certificate replay must:

1. validate and replay the matcher and its inventory lineage;
2. reset `consumed_targets` to empty;
3. iterate every matcher outcome in the retained order;
4. recompute the first available target for every pending pivot;
5. recompile every selected target-local result;
6. repeat the exact consume/no-consume transition;
7. rebuild accepted and rejected attempt records;
8. rebuild every final target disposition;
9. verify that each consumed target has one accepted attempt;
10. verify that no unconsumed target has a rule handle;
11. verify relative structural conservation for every consumed target;
12. verify that every exceptional accepted child remains residual;
13. verify that all group targets have exactly one final disposition; and
14. perform a bounded complete payload comparison.

The recursively authenticated chain is:

```text
generated IBP/LI row span
  -> generated sector discovery and global coverage
  -> live residual queue
  -> Boolean product-locus cover
  -> residual affine branch and integer map
  -> target guard composition
  -> static affine case inventory and geometry group
  -> affine prepare-point schedule
  -> generated branch-bound rows
  -> branch re-elimination and private pivots
  -> split recentering and exact target matching
  -> target-local affine WhenBad
  -> sequential group effective coverage.
```

## 14. Test matrix

### 14.1 Pure unit tests

Test the relation-free shared core exhaustively on small masks and shifts:

- every finite inactive-activation interval;
- every finite concrete-overflow interval;
- no hazard in the two mathematically safe directions;
- `delta = 0, +/-1, i64::MIN, i64::MAX` on active and inactive slots;
- exact boundary counts and arithmetic-overflow errors;
- strict descent, harder first component, zero delta, wrong arity, and
  fallible witness retention;
- ordering-policy rejection; and
- zero-RHS acceptance.

Test the target-relative partition independently:

- root conservation;
- deterministic child order;
- repeated predicate routing;
- inherited nonzero facts;
- empty direct formula, the complete three-valued conjunction table,
  false-left right-atom short circuit, later-true after earlier-unknown, and
  first unresolved atom/clause provenance;
- exact `B!=0`, `B=0,N=0`, and `B=0,N!=0` dispositions;
- structural divisibility contradiction; and
- every split/leaf/retention limit at exact and one-below values.

Test the condition accumulator independently:

- exact and coefficient-field-associate forms from a target guard, relation
  guard, pivot denominator, and RHS denominator produce one canonical row
  with every typed source;
- a candidate-first row is promoted to `InheritedTargetPremise` when an
  inherited associate arrives, retains candidate provenance privately, and
  emits no candidate-failure clause;
- a candidate-only row remains `CandidateRequired`;
- nonzero integer constants are discharged, nonzero base-only expressions are
  formal assumptions without index splits, a zero candidate condition is
  identically bad, and a zero inherited condition is a hard invariant error;
  and
- centered-pivot and RHS denominator sources remain distinguishable.

Test the mandatory phase order with one synthetic input that has both a
failed RHS descent witness and a zero candidate condition. It must return the
typed descent `Unsupported` result without condition or pullback work. A
zero-RHS relation remains valid and can produce the root `Applicable` leaf.

Test sequential target state with fabricated metadata only:

- accepted first target makes the next pivot fall through;
- identically-bad first pivot leaves the first target available;
- unsupported first pivot leaves the first target available;
- one pivot never tries its second target after failure on its first selected
  target;
- no remaining target is typed and consumes nothing; and
- a target cannot be accepted twice.

### 14.2 Generated two-loop sunset integration tests

Reuse the existing equal-mass sunset construction from
`tests/generated_residual_affine_pivot_target_matching.rs`. Production code
must receive only `IntegralFamily`, context, generated certificates, and
limits.

#### A. Generated `001` condition and local replay fixture

Use the existing generated `001` pending-matcher fixture and compile its first
available exact target. Through crate-private test access, require:

- every selected-target guard entry and every recentered relation guard to
  appear under the correct typed source ordinal;
- every coefficient denominator, including the centered pivot, to be scanned
  even when its condition is discharged or associate-equivalent;
- inherited-dominance deduplication and the absence of unrecentered matcher
  source premises from applicability;
- replay of every reachable `Certified`, `IdenticallyBad`, or `Unsupported`
  outcome; and
- public views and `Debug` to remain redacted.

No expected condition polynomial, coefficient, recurrence, or shift table may
be hardcoded.

#### B. Measured multi-case rejection fixtures

For sectors `011` and `101`:

1. generate discovery at the existing bounded depth;
2. build the live queue and complete affine inventory;
3. build an eliminated branch and pivot-target matcher;
4. select the real multi-case group `[1,3]` explicitly; and
5. replay all four retained matcher outcomes and the two final residual roots.

The current depth-zero/radius-zero fixtures have four
`RejectedNoTargetCase` outcomes for either source case in that group.  Assert
that both targets remain unconsumed complete residual roots and that neither
can own a sealed rule handle.  Do not use these fixtures to claim generated
multi-target consumption.  A later genuine fixture may replace this negative
oracle only after its exact bounded schedule has been measured and frozen.

#### C. Independent affine pullback oracle

For every retained sunset pullback reached from generated `001` (and from any
later measured `011` or `101` pending fixture), and several small
free-coordinate integer assignments:

1. evaluate the target map independently as `b+A*t` using Symbolica `Integer`;
2. specialize the retained pullback polynomial at the same free values; and
3. assert equality with `G_i(t)-v`.

Cover `EmptyBoundary`, `WholeTarget`, and free-index-dependent cases when
present. Verify that the pullback polynomial uses only target free-coordinate
index variables and coefficient parameters.

#### D. Numerator gate transcript

Locate a generated pending sunset term with an index-dependent numerator gate
by scanning a fixed small list of sunset sectors if necessary. Assert that
the retained relative transcript contains:

```text
off boundary                         -> continue
on boundary and numerator zero       -> continue
on boundary and numerator nonzero    -> exceptional
```

No expected recurrence coefficient or shift table may be hardcoded.

#### E. Sequential multi-case consumption transition

Use the topology-free fabricated-metadata transition fixture required in
section 14.1, with two target ordinals and persisted matching lists such as
`[17,23]`.  Independently replay the consumed-target set from those persisted
lists and compare every selected target and final target disposition to the
effective certificate. Require:

- failed attempts consume nothing;
- accepted attempts consume exactly one target;
- a later pivot falls through only after acceptance; and
- no target receives two accepted attempts.

Separately, the generated `001` fixture must produce one accepted target
followed by three `NoRemainingTargetCase` attempts.  A genuine generated
two-target consumption test remains pending until a bounded family/schedule
actually produces at least two pending matches; `011` and `101` do not do so
under the current bounded builders.

#### F. Structural conservation and residual retention

For every case in the matched group:

- an unconsumed target remains one complete residual root; or
- a consumed target's applicable and exceptional relative leaves form a
  structural cover of that target root.

Require every exceptional leaf to remain in residual work and every
applicable leaf to cite the one accepted attempt.

#### G. Concrete nonparametric powers

Enumerate a small bounded set of concrete integer powers in the sunset sector.
For each point:

1. authenticate its unique inventory Boolean terminal;
2. evaluate the final target-relative classification if its target was
   consumed; and
3. require exactly one effective result.

Boundary points with nonzero numerator must be residual. Safe points must
agree with direct evaluation of the retained bad formula. This is validation
only; concrete powers must not enter production algorithm decisions.

Vakint comparison begins only after the sealed conditional-rule application
path exists. Vakint/FORM output may serve as a test oracle then, but RustRed
must not invoke FORM or embed Vakint's hardcoded recurrences.

### 14.3 Replay, tamper, and resource tests

Replay must reject independent tampering with:

- pending pivot ordinal;
- selected target ordinal or target-list position;
- target locator or group ordinal;
- coefficient translation or key center;
- target ordering manifest;
- domain-condition polynomial, scope, or source;
- pullback coordinate, value, kind, polynomial, or class;
- numerator gate polynomial or class;
- descent witness;
- formula atom or clause;
- relative split child or leaf disposition;
- accepted attempt ordinal;
- consumed-target transition; and
- final target disposition.

For every meaningful statistic, compile with exact measured limits and replay
successfully. Then lower representative limits by one and require the exact
typed resource failure before partial retained state is published.

Representative one-below tests must include aggregate descent components,
condition inputs versus unique rows, source records, hazard census counts,
composition work, exponent-entry comparison visits, child-relative formula
and split work, retained bytes, and replay comparison work. A child helper
must fail once its parent's remaining aggregate is exhausted even when the
child's standalone maximum would allow the operation.

Add a `compile_fail` public-API test demonstrating that external code cannot
call `relation_for_affine_when_bad()` or obtain a `ParametricRelation` from a
pending, local, or effective certificate.

Add public-API and formatting tests demonstrating that external code cannot
obtain exact RHS shifts, denominator source payloads, numerator-gate
polynomials, or exact relative predicate polynomials from an outcome. Format
every public outcome and certificate with `Debug` and reject private relation
manifests, coefficient text, raw shifts, denominator payloads, and numerator
payloads in the result.

### 14.4 Parallel licensed test command

The eventual focused suite should run in parallel with the supplied license
and default GMP build:

```bash
SYMBOLICA_LICENSE='your-license' \
SYMBOLICA_HIDE_BANNER=1 \
cargo nextest run -j4 \
  --test generated_residual_affine_when_bad \
  --test generated_residual_affine_effective_coverage
```

No `no_gmp` feature and no FORM process are permitted.

## 15. Recommended implementation order

The current audited checkpoint completes items 1-6 below. Its production
inputs remain an arbitrary authenticated family/matcher/target tuple; no loop
count, topology label, recurrence, or concrete power selects the algorithm.

The remaining implementation order is:

1. **Complete.** Orchestrate the authenticated local compiler so all descent witnesses are
   precharged and proved before any zero-condition or boundary reasoning.
2. **Complete.** Add inherited and candidate guard/denominator collection with deterministic
   source provenance, bounded equality/associate comparison, and inherited
   dominance.
3. **Complete.** Add the two-pass hazard census/construction pipeline, one shared selected-
   target integer-system plan, exact `n_i-v` pullbacks, and defense-in-depth
   free-position validation.
4. **Complete.** Add exact mapped numerator gates, typed identically-bad and unsupported
   outcomes, derived child limits, and complete private-payload local replay.
5. **Complete.** Implement sequential persisted-order target selection, an
   O(1) consumed-position state, consume-only-on-certified transitions, and
   bounded O(G+R) rejected-reference distribution.
6. **Complete.** Add effective target dispositions, sealed opaque rule
   handles, exact root/leaf conservation, exhaustive Arc/leaf authority
   checks, redacted formatting, aggregate limits, and outer replay.
7. **Partially complete.** Generated `001`, `011`, and `101` acceptance or
   negative paths, privacy, tamper, and representative exact-limit tests are
   present. Keep searching future families for an authentic generated local
   reject-then-accept and multi-target pending path; topology-free transition
   tests remain the honest coverage until then.
8. **In progress.** Implement the V2 sector affine effective-coverage owner
   and effective residual queue from section 16 without changing V1 global
   coverage semantics.
9. Add crate-private sealed affine rule application and a conditional-provider
   tier authenticated by the V2 owner.
10. Integrate the effective residual queue into the V2 family fixed point and
    dependency/back-substitution transcript, then compare reductions against
    Vakint without substituting masters.

The essential invariant is:

> A matched affine pivot is not a rule. It becomes one conditional rule only
> after target-local guards, pulled-back boundaries, numerator gates, strict
> descent, and sequential effective coverage all replay for one exact target
> case. Every excluded or exceptional domain remains explicit residual work.

## 16. V2 sector affine effective-coverage owner

The group transaction is not itself a sector fixed point. The minimal outer
owner is a new V2 certificate; the existing V1 global-coverage and family
fixed-point schemas must continue to replay unchanged. In particular, an
affine-locus rule must never be appended to the V1 globally valid candidate
list or passed back through the global `GeneratedWhenBadCompiler` path.

`GeneratedSectorAffineEffectiveCoverageCertificate` owns one canonical source
authority, one complete affine inventory `Arc`, and group transactions in
inventory order. The initial source is a live global residual queue. A later
epoch may use a prior effective residual queue, but must retain the exact
upstream locator rather than fabricate a global source case. Every group
transaction's matcher must own the same inventory allocation. A target is
processed at most once in one epoch; exceptional children re-enter only in a
new epoch sourced from the previous effective result.

The owner retains one final disposition for every inventory terminal:

- a proved-empty terminal stays empty;
- an unsupported or unprocessed terminal stays one residual terminal;
- an unconsumed actionable target stays one complete residual root; and
- a consumed target is replaced, in child order, by all of its applicable and
  exceptional relative leaves.

Applicable leaves yield sealed rule locators. Every exceptional-domain or
exceptional-leak leaf yields a residual locator. Flattened locators contain
only owner-relative ordinals; they do not clone predicates or relations and
carry no authority outside the owner. A suitable initial vocabulary is:

```rust
enum GeneratedSectorAffineResidualLeafLocator {
    UnsupportedInventoryTerminal { terminal_ordinal: usize },
    UnprocessedActionableCase { case_ordinal: usize },
    UnconsumedTargetRoot {
        group_pass_ordinal: usize,
        target_case_ordinal: usize,
    },
    ExceptionalTargetChild {
        group_pass_ordinal: usize,
        accepted_attempt_ordinal: usize,
        leaf_ordinal: usize,
    },
}

struct GeneratedSectorAffineRuleLocator {
    group_pass_ordinal: usize,
    accepted_attempt_ordinal: usize,
    leaf_ordinal: usize,
}
```

The exact conservation equation is therefore:

```text
inventory terminal
  = proved empty
  | residual root
  | disjoint union(applicable rule leaves, exceptional residual leaves).
```

Concrete classification first respects the global root result. Only a global
uncovered or unsupported cell enters the affine overlay. The owner resolves
its exact inventory case, authenticates the target affine map, and classifies
the point in the accepted target-relative partition. The result must be
exactly one sealed rule locator or one residual locator. No negative search
outcome is a master proof.

A crate-private application seam resolves a sealed locator, reauthenticates
the target and its exact `Applicable` leaf, specializes the private relation,
checks all guards and strict descent, and returns the existing conditional
concrete-reduction type. It publishes neither `ParametricRelation` nor a
conversion to a globally valid candidate.

The following V2 stage is an effective residual queue whose work items are
addressed by the residual locators above and expose authenticated predicate,
affine-coordinate, and point-classification operations. That queue feeds the
next inventory epoch and the V2 fixed-point anchor scheduler. Replay rebuilds

```text
source queue -> inventory -> ordered group passes -> terminal census
             -> sealed/residual locators -> effective residual queue.
```

The family fixed point commits a V2 material locator only after this whole
chain replays. Hard/resource failures leave the prior material current.
Cumulative limits must separately cover inventory/group references, matcher
outcomes, selected-target scans, local compilations, rule/residual locators,
predicate and affine-map evaluations, frontier work, retained transcript
bytes, private replay comparison, and provider installation/query work.
Requested table bytes are checked before fallible allocation, and nested
limits are projected from the remaining owner budget rather than reset for
each group.
