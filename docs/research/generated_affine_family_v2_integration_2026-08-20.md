# Generated affine family V2 integration

Date: 2026-08-20

Status: architecture and source audit. This note records the smallest sound
integration path from the implemented generated residual-affine target-local
and group-local layers to a sector owner, recursive residual epochs, concrete
application, and a family fixed point. It does not claim that those V2 layers
are implemented. No Rust source was changed and no tests were run as part of
this note.

## Implementation update (2026-08-21)

The original status paragraph above describes the tree when this audit was
written. The following generic one-epoch layers are now implemented and
replay-tested:

- `GeneratedSectorAffineEffectiveCoverageCertificate`, including every-group
  orchestration, terminal/child conservation, exact point classification, and
  owner-relative rule/residual locators;
- owner-authenticated sealed concrete specialization with retained guards,
  unit-LHS/subsector/strict-descent checks, durable replay, redacted public
  provenance, and complete pre-allocation/peak-memory limits;
- `GeneratedSectorAffineConditionalRuleProvider`, including borrowed-owner
  preflight, independent owner replay, exact routing statistics, and one owner
  per sector;
- the separate `build_generated_affine_provider_stack` assembly with order
  `zero(symmetry(master(conditional_v1(affine(global_v1)))))`, exact shared
  row-span `Arc` identity, and no change to the V1 stack builder or schemas;
- `GeneratedSectorAffineEffectiveResidualQueueCertificate`, which retains one
  exact sector-owner `Arc`, emits residual roots and exceptional children in
  terminal-expansion order, omits proved-empty and rule leaves, replays in
  place without rebuilding an output vector, and delegates exact point
  membership to the owner before resolving one private queue authority slot;
- `GeneratedAffineResidualSourceAuthority`, a sealed wrapper over exactly one
  initial-global or prior-effective source `Arc`. Its concrete variant is
  module-private, it fabricates no V1 queue, and its ordinal-only dispatch
  returns lifetime-bound source-neutral views without exposing a raw `Arc`,
  queue, partition, extraction, relation, or unrelated global locus;
- lifetime-bound initial-global views which distinguish the defensive
  coordinate-proved-empty sum from a Ready Boolean conjunction, authenticate
  exact queued/global dispositions, and expose each retained predicate plus
  only its canonical singleton/product atoms under explicit lookup bounds;
- lifetime-bound prior-effective source views for unsupported terminals,
  unprocessed/unconsumed target roots, and exceptional domain/leak children.
  Construction and replay authenticate the complete owner-to-leaf chain and
  the projected Boolean/affine payloads under explicit comparison budgets;
  per-item resolution is allocation-free, direct-indexed, and returns neither
  a broad certificate nor an `Arc` or private relation.

This is one certified affine epoch plus the sealed authority and unified
narrow resolver needed to consume initial or prior-effective residual output.
The remaining
critical path begins at sections 4 and 7 after that seam: compile
source-neutral Boolean/branch/guard/inventory ownership over the unified views
without synthesizing a V1 queue, then implement ordered multi-epoch
owners per sector, a transactional affine family scheduler and certificate,
exact configured-fixed-point statuses, and family-level provider installation.
In particular, the current one-owner-per-sector stack and its next-source queue
must not be described as recursive LiteRed closure or master discovery.

The accepted source seam was validated on 2026-08-21 with licensed,
GMP-enabled Symbolica. `cargo fmt --all -- --check` and
`cargo check --tests --example generic_symbolica_tensor_ibp` passed. The
unified-view focused parallel run (`cargo nextest run -j4`, run
`0602c35b-fbda-4001-aa23-f3aa433fc269`) passed all nine tests. These cover both
source variants, exact `Arc` lifetime, redaction, exact disposition binding,
all three reachable nonempty initial outcomes, the defensive empty-sum versus
zero-predicate Ready distinction, natural sunset-sector product factors,
private authority tampering, and exact/one-below lookup bounds. The runnable
generic tensor/IBP example also completed with certificate replay and zero
uncovered leaves. An independent code audit found no remaining concrete
defect. The frozen coverage-DAG hash remained unchanged.

The design is independent of topology, loop count, masses, number of
denominators, and numerator rank. A one-loop family, the equal-mass two-loop
sunset, and later three- through five-loop vacuum families are validation
fixtures only. No production recurrence, closure, exceptional case, or search
decision may be selected by a topology name or a loop-count-specific module.
Loop count may determine input size and configured resource/search bounds; it
must not determine the algebraic rules.

This note refines section 16 of
[`affine_when_bad_effective_coverage_design_2026-08-14.md`](affine_when_bad_effective_coverage_design_2026-08-14.md)
and is consistent with
[`solved_subsector_feedback_design_2026-08-13.md`](solved_subsector_feedback_design_2026-08-13.md).

## 1. Claim boundary

### 1.1 Implemented now

The following generic layers exist. Every certificate-bearing layer in this
list has its own replay boundary; the final sector-owner item is currently
vocabulary only:

- V1 generated family rule-system and global fixed-point material;
- V1 globally valid parametric sector coverage and its concrete provider;
- equality-locus partial re-elimination and its conditional provider;
- product-locus Boolean splitting for a V1 live residual queue;
- exact residual affine integer recognition and guard composition;
- affine prepare-point ordering and branch re-elimination;
- matcher-bound, target-local affine `WhenBad` compilation;
- persisted-order group effective coverage, including consume-only-on-certified
  transitions, sealed applicable handles, and explicit residual work; and
- the first generic sector-owner locator/disposition vocabulary.

### 1.2 Not implemented yet

The following are future V2 work:

- the complete `GeneratedSectorAffineEffectiveCoverageCertificate` and its
  compiler;
- a point-classification seam owned by that certificate;
- sealed concrete affine rule application;
- an effective residual queue which can be the source of another affine
  epoch;
- source-neutral V2 product-cover/inventory/matcher ownership;
- an affine conditional-provider tier;
- a V2 provider stack and family provider;
- an outer generated affine family fixed point; and
- replayable family dependency/back-substitution material.

No unsuccessful or bounded search is presently a master proof. In particular,
the existence of a source file named after a loop count or a sunset validation
fixture is not evidence of a topology-specific reduction algorithm and must
not become one.

## 2. Exact current seams

Line anchors below refer to the audited tree on 2026-08-20. Type and function
names are the stable anchors; nearby line numbers may move as files evolve.

| Current seam | Location | What it owns now | Missing V2 seam |
|---|---|---|---|
| `GeneratedFixedPointMaterialLocator` | [`generated_family_fixed_point.rs:159`](../../src/generated_family_fixed_point.rs#L159) | Only `BaseRuleSystem`, `BasePreparation`, and `ResidualRound` discovery/live-queue material | A separate V2 affine material locator; the V1 enum must not be extended or reinterpreted |
| `GeneratedFamilyFixedPointCertificate` | [`generated_family_fixed_point.rs:904`](../../src/generated_family_fixed_point.rs#L904) | V1 base, global preparations/rounds, final statuses | V2 owner which embeds/references the replayed V1 certificate and affine epochs |
| `latest_materials` | [`generated_family_fixed_point.rs:1024`](../../src/generated_family_fixed_point.rs#L1024) | Resolves only discovery plus `GeneratedSectorLiveLeafQueueCertificate` | Resolution of committed affine owner/effective-queue material in a new V2 API |
| `compile_with_replayed_base` | [`generated_family_fixed_point.rs:1090`](../../src/generated_family_fixed_point.rs#L1090) | Global residual-anchor search and recomposition against the shared generated row span | A separate affine epoch scheduler; affine rules must not be appended to the global candidate list |
| `GeneratedFamilyFixedPointProvider` | [`generated_family_fixed_point_provider.rs:117`](../../src/generated_family_fixed_point_provider.rs#L117) | Installs exact latest V1 coverages and live queues | V2 family provider which also installs certificate-owned affine authorities |
| `resolve_latest_materials` | [`generated_family_fixed_point_provider.rs:550`](../../src/generated_family_fixed_point_provider.rs#L550) | V1 material resolution/preflight | V2 material resolution with Arc-identity checks and affine retention budgets |
| `build_generated_provider_stack` | [`generated_provider_stack.rs:59`](../../src/generated_provider_stack.rs#L59) | Builds the V1 global/equality-conditional/master/symmetry/zero stack | A separate V2 stack builder with the affine tier at the exact order specified below |
| `GeneratedSectorConditionalRuleProvider` | [`generated_sector_conditional_provider.rs:231`](../../src/generated_sector_conditional_provider.rs#L231) | Equality-locus rules derived from `GeneratedPartialReeliminationCertificate` | It must delegate residual global cells to an inner affine provider before the global provider is reached |
| `GeneratedSectorConditionalRuleProvider::decision_for` | [`generated_sector_conditional_provider.rs:564`](../../src/generated_sector_conditional_provider.rs#L564) | Classifies the V1 global root, delegates descending cells, then tries equality-locus rules on residual cells | No change in V1 semantics; its inner provider becomes the affine wrapper in the V2 stack |
| `ConditionalConcreteReduction` | [`conditional_rules.rs:436`](../../src/conditional_rules.rs#L436) | Concrete conditional reduction authenticated only by `Arc<ConditionalParametricRule>` | A private authority variant for an affine sector owner plus sealed locator |
| `ConditionalConcreteReduction::rule` | [`conditional_rules.rs:449`](../../src/conditional_rules.rs#L449) | Assumes every conditional reduction has a coordinate-equality rule | A safe optional coordinate-rule accessor plus authority-independent `ordering_policy()` |
| V1 product-cover source | [`product_locus_boolean_cover.rs:321`](../../src/product_locus_boolean_cover.rs#L321) | Hard-binds `Arc<GeneratedSectorLiveLeafQueueCertificate>` | A source-neutral V2 residual-leaf authority |
| `ready_terminal_for_indices` | [`product_locus_boolean_cover.rs:431`](../../src/product_locus_boolean_cover.rs#L431) | Evaluates the exact V1 Boolean terminal at an integer point | Reusable source-view operation for later residual epochs |
| `GeneratedResidualAffineCaseInventoryCertificate` | [`generated_residual_affine_case_inventory.rs:492`](../../src/generated_residual_affine_case_inventory.rs#L492) | Complete affine inventory for one V1 live queue | V2 inventory ownership over either an initial V1 queue or a prior effective residual queue |
| Inventory `source_queue` | [`generated_residual_affine_case_inventory.rs:494`](../../src/generated_residual_affine_case_inventory.rs#L494) | Hard-binds a V1 live queue | Versioned/source-neutral authority without fabricating a V1 queue |
| `GeneratedResidualAffineCaseInventoryCompiler` | [`generated_residual_affine_case_inventory.rs:699`](../../src/generated_residual_affine_case_inventory.rs#L699) | Accepts only a V1 live queue | V2 compiler or a shared internal algorithm over a sealed source view |
| `GeneratedResidualAffinePivotTargetMatchingCertificate` | [`generated_residual_affine_pivot_target_matching.rs:515`](../../src/generated_residual_affine_pivot_target_matching.rs#L515) | One inventory, source case/group, and ordered matcher outcomes | A V2 wrapper or source-neutral inventory authority while preserving V1 replay |
| Matcher's `inventory` | [`generated_residual_affine_pivot_target_matching.rs:546`](../../src/generated_residual_affine_pivot_target_matching.rs#L546) | Hard-binds the V1 inventory allocation | Every group pass in an epoch must retain the one exact V2 inventory Arc |
| Private recentered relation seam | [`generated_residual_affine_pivot_target_matching.rs:450`](../../src/generated_residual_affine_pivot_target_matching.rs#L450) | Crate-private relation access only for affine `WhenBad` | Must remain private and become reachable only through an authenticated owner application path |
| `AuthenticatedGeneratedResidualAffineWhenBadInput` | [`generated_residual_affine_when_bad_compilation.rs:908`](../../src/generated_residual_affine_when_bad_compilation.rs#L908) | Private matcher, target map/guards, ordering, relation, and manifest | Crate-private specialization/classification helpers for the owning sector certificate |
| Private authenticated relation | [`generated_residual_affine_when_bad_compilation.rs:950`](../../src/generated_residual_affine_when_bad_compilation.rs#L950) | Available only inside the local compiler module | No public getter or conversion to a globally valid candidate |
| `GeneratedResidualAffineWhenBadCertificate` | [`generated_residual_affine_when_bad_compilation.rs:4642`](../../src/generated_residual_affine_when_bad_compilation.rs#L4642) | Certified target-local relative partition with redacted public views | Owner-only point classification and concrete specialization |
| `GeneratedResidualAffineWhenBadCompilation` | [`generated_residual_affine_when_bad_compilation.rs:4997`](../../src/generated_residual_affine_when_bad_compilation.rs#L4997) | `Certified`, `IdenticallyBad`, or `Unsupported`; consumes no target itself | Sector owner consumes only the group owner's certified transition |
| `GeneratedResidualAffineSealedConditionalRuleHandle` | [`generated_residual_affine_group_effective_coverage.rs:384`](../../src/generated_residual_affine_group_effective_coverage.rs#L384) | Crate-private locator plus shared local authority; relation remains sealed | Flattening into an owner-relative sector rule locator and an application seam |
| `GeneratedResidualAffineResidualWorkKind` | [`generated_residual_affine_group_effective_coverage.rs:441`](../../src/generated_residual_affine_group_effective_coverage.rs#L441) | Complete target roots, exceptional domains, and exceptional leaks | Conversion to an effective residual queue for the next epoch |
| `GeneratedResidualAffineGroupEffectiveCoverageCertificate` | [`generated_residual_affine_group_effective_coverage.rs:498`](../../src/generated_residual_affine_group_effective_coverage.rs#L498) | One matcher's sequential target-consumption transaction | Sector-wide inventory-ordered group orchestration and terminal conservation |
| Initial sector-owner vocabulary | [`generated_sector_affine_effective_coverage.rs:10`](../../src/generated_sector_affine_effective_coverage.rs#L10) | Schema constant and owner-relative root/rule/exceptional/terminal locators | Certificate, compiler, replay, classification, application, effective queue, limits, and stats |

The initial sector-owner file currently ends after
`GeneratedSectorAffineTerminalDisposition` at
[`generated_sector_affine_effective_coverage.rs:56`](../../src/generated_sector_affine_effective_coverage.rs#L56).
Its `...-v1` schema name denotes the first version of this new sector-owner
certificate vocabulary. Architecturally it is the V2 layer relative to the
existing global family/coverage path; this naming does not authorize changing
the existing V1 certificates.

## 3. Sector effective-coverage owner

### 3.1 Required ownership

The future `GeneratedSectorAffineEffectiveCoverageCertificate` must own:

1. one canonical source authority;
2. one complete source-neutral affine inventory `Arc`;
3. exactly one explicit pass outcome for every inventory group, in inventory
   order;
4. one final disposition for every inventory terminal, in source order;
5. ordered child outputs for every consumed target;
6. owner-relative sealed-rule and residual locators;
7. an exact conservation census; and
8. complete construction, replay, query, retained-payload, and private
   comparison limits/statistics.

Every successful group pass must retain the same inventory allocation. A
matcher compiled from an independently rebuilt equal inventory is not the
same authority. A group for which branch preparation or matching has no
usable rows must retain a typed negative pass outcome; silently omitting the
group would make the terminal census incomplete.

The sector compiler performs, generically for every inventory group:

```text
group anchor case
  -> affine ordering
  -> bounded prepare-point schedule
  -> branch re-elimination
  -> pivot/target matcher
  -> sequential group effective coverage
  -> terminal dispositions and ordered child outputs.
```

`EmptyBranch`, `NoAvailableRows`, a complete matcher with no accepted target,
and a resource/failure interruption are distinct outcomes. Only a complete
successful pass contributes sealed rules; resource/failure interrupts the
sector transaction and publishes no partial owner.

### 3.2 Exact conservation

For every input terminal, replay must prove exactly one of:

```text
input terminal
  = proved empty
  | one residual root
  | disjoint union(
      applicable sealed-rule leaves,
      exceptional residual leaves
    ).
```

The global/source root is evaluated first. Only a source point still
classified residual may enter the new affine overlay. Unsupported or
unprocessed inventory terminals remain residual. An unconsumed actionable
target remains one complete residual root. A consumed target is replaced in
relative child order by all applicable and exceptional leaves.

No negative search result is a master declaration.

### 3.3 Exact point classification

The owner needs a crate-private operation conceptually equivalent to:

```rust
fn classify_point(
    &self,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    limits: GeneratedSectorAffineQueryLimits,
) -> Result<GeneratedSectorAffinePointDisposition, GeneratedSectorAffineError>;
```

Classification must:

1. authenticate family, context, arity, sector, and source authority;
2. ask the source authority whether the point is already covered or lies in
   its residual union;
3. find exactly one source Boolean terminal;
4. resolve its exact inventory disposition;
5. for an actionable case, authenticate the target affine map and prove
   actual map membership by checking `F(n) == n` componentwise;
6. evaluate every predicate of the accepted target-relative partition under
   checked Symbolica specialization;
7. require exactly one matching relative leaf; and
8. return exactly one sealed-rule locator or residual locator.

The explicit map-membership check is essential. The existing
`guarded_affine_map_applies_at_original_indices` at
[`residual_affine_branch_system.rs:469`](../../src/residual_affine_branch_system.rs#L469)
checks the original Boolean terminal for a point documented as already
mapped; it does not itself prove `F(n) == n`.

A point matching a proved-empty terminal, two terminals, two relative leaves,
or no leaf inside a claimed exhaustive source is a hard replay/classification
error, not an uncovered result.

## 4. Source-neutral recursive residual epochs

### 4.1 Why a new abstraction is required

Changing only the sector owner is insufficient. The current product cover
and inventory own `Arc<GeneratedSectorLiveLeafQueueCertificate>`, the matcher
owns the current inventory type, and local affine `WhenBad` owns that matcher.
A prior affine exceptional leaf cannot soundly be converted into a synthetic
V1 work item: doing so would lose its target affine map, inherited premises,
relative predicates, and upstream locator.

Preserve every V1 schema and add a V2 source authority such as:

```rust
enum GeneratedAffineResidualSourceCertificate {
    InitialGlobal(Arc<GeneratedSectorLiveLeafQueueCertificate>),
    PriorEffective(Arc<GeneratedSectorAffineEffectiveResidualQueueCertificate>),
}
```

This enum is illustrative vocabulary, not an instruction to expose either
variant publicly. Its source-view operations must provide:

- family/context fingerprints, sector, ordering, and arity;
- deterministic replay;
- source-ordered residual leaf count and opaque locators;
- exact point classification;
- authenticated equality/nonzero predicate views for compilation;
- inherited target affine-map authority where present;
- exact upstream provenance for replay; and
- bounded retained-size and predicate-evaluation censuses.

The implementation may use new V2 wrapper certificates or refactor common
algorithms behind a crate-private sealed source-view trait. It must not change
the meaning or serialized/replay payload of the existing V1 product cover,
inventory, matcher, or family certificate.

### 4.2 Effective residual queue

`GeneratedSectorAffineEffectiveResidualQueueCertificate` owns an `Arc` to its
sector owner and addresses work only by the owner's residual locators. It
does not clone private predicates or relations.

Each residual kind retains enough authority for the next epoch:

- unsupported inventory terminal: the complete original source premises and
  typed unsupported outcome;
- unprocessed actionable case: the complete source terminal and target map;
- unconsumed target root: the target affine map and all inherited guards;
- exceptional target child: the target affine map plus every relative leaf
  predicate and its exceptional condition/pullback provenance.

Dropping the relative predicates from an exceptional child would enlarge the
next source and could publish a rule outside its certified domain. Treating
only coordinate equalities as the next source would be sound only if the
nonlinear predicates remain explicit inherited premises; it is not a complete
LiteRed-like recursion otherwise.

The queue replay chain is:

```text
source authority
  -> source-neutral product/affine inventory
  -> ordered group passes
  -> sector terminal census
  -> residual locators
  -> effective residual queue.
```

The next epoch consumes exactly that queue. It never re-enters the global
`GeneratedWhenBadCompiler` and never appends an affine rule to the globally
valid `GeneratedSymbolicRowSpanCertificate` candidate database.

## 5. Sealed concrete application and private authority

### 5.1 Current incompatibility

`ConditionalConcreteReduction` currently stores
`Arc<ConditionalParametricRule>` and its replay/verification assumes a sparse
coordinate-equality assignment. An affine leaf is instead authenticated by a
target affine map and a relative predicate conjunction. Constructing a dummy
`ConditionalParametricRule`, publishing the private relation, or approximating
the affine leaf by coordinate equalities would be unsound.

### 5.2 Private authority refactor

Keep the V1 `ConditionalParametricRule` schema and constructors unchanged.
Refactor only the concrete result's private authority:

```rust
enum ConditionalConcreteAuthority {
    CoordinateEquality(Arc<ConditionalParametricRule>),
    GeneratedAffine {
        owner: Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
        locator: GeneratedSectorAffineRuleLocator,
    },
}

pub struct ConditionalConcreteReduction {
    authority: ConditionalConcreteAuthority,
    // Existing concrete, public-safe payload follows.
}
```

Required public-safe operations are:

- `ordering_policy()` independent of the authority variant;
- `coordinate_rule() -> Option<&Arc<ConditionalParametricRule>>` for callers
  which specifically need the old coordinate proof;
- the existing source, RHS, specialized relation, specialized nonzero guards,
  and strict-descent witnesses; and
- custom redacted `Debug` which does not recursively format an affine owner.

The current total `rule()` accessor cannot honestly represent an affine
authority. It must not panic or return a fabricated coordinate rule. The only
current non-test consumer which extracts its ordering directly is the tensor
trace validation at
[`tensor_reduction_engine.rs:2117`](../../src/tensor_reduction_engine.rs#L2117);
it should use `ordering_policy()`.

The concrete result retains the owner `Arc` so replay survives provider drop
and reduction-cache retention. The limits must count retained authority
references and cached proof/debug size without deep-cloning the owner for
each application.

### 5.3 Owner-only application

The crate-private application seam resolves a sealed locator inside its exact
owner and then re-runs classification for the requested source point. Only an
exact `Applicable` leaf may borrow the private local certificate and relation.
It then:

1. specializes the split-recentered private relation at the concrete source
   indices;
2. retains all specialized relation, inherited-target, and candidate-domain
   guards;
3. rejects a vanished required guard as inapplicable, never as a valid rule;
4. checks that the concrete LHS is the requested source with unit coefficient;
5. constructs each negated RHS coefficient;
6. proves every RHS sector is a subsector of the declared sector;
7. proves strict descent for every RHS key under the certified ordering; and
8. returns `ConditionalConcreteReduction` with the affine owner/locator
   authority.

Replay dispatches by private authority. Public APIs may expose concrete
specialized output, but must not expose `&ParametricRelation`, exact private
relative predicates, shift-bearing denominator sources, or a conversion to a
globally valid candidate.

## 6. V2 provider order

The current stack aliases at
[`generated_provider_stack.rs:19-33`](../../src/generated_provider_stack.rs#L19)
and construction at
[`generated_provider_stack.rs:80-161`](../../src/generated_provider_stack.rs#L80)
produce:

```text
zero(symmetry(master(conditional_v1(global_v1))))
```

The V2 stack must be built separately as:

```text
zero(
  symmetry(
    master(
      conditional_v1(
        affine_v2(
          global_v1
        )
      )
    )
  )
)
```

This order is semantic, not a performance preference:

1. zero, symmetry, and explicitly installed master policies retain their
   current outer precedence;
2. a V1 equality-locus conditional rule is tried before the affine overlay;
3. the affine provider sees a residual global root before the global provider
   turns `Unsupported` into an error; and
4. after both conditional tiers are exhausted, the unchanged global provider
   supplies `Uncovered` or its typed `UnsupportedLeaf` error.

The distinction follows directly from
[`parametric_sector_provider.rs:289-298`](../../src/parametric_sector_provider.rs#L289):
`Uncovered` is a terminal decision, whereas `Unsupported` is an error. An
affine provider implemented outside the current global provider but after
calling it would never receive unsupported cells.

`GeneratedSectorAffineConditionalRuleProvider<Inner>` therefore must classify
the installed source/owner authority itself. For a missing sector, a globally
descending/proved-empty point, or an exhausted affine residual point it
delegates appropriately. It applies only a sealed locator returned by the
certificate-owned classifier. It stores owner `Arc`s, not copied relations or
public predicate tables.

A sector with multiple committed epochs installs their owner authorities in
certified epoch order. Each later owner is queried only on the residual domain
authenticated by its upstream effective queue. Conservation makes successful
rule domains disjoint from all later source queues. The provider must not use
an unguarded newest-to-oldest rule scan.

Provider limits separately bound installed sectors/epochs/owners, rule and
residual locators, retained transcript bytes, queries, source and relative
predicate evaluations, affine-map operations, specialized terms/guards, and
application traces. Family installation preflights borrowed owner metadata
before cloning any `Arc` and checks allocation identity against the replayed
family certificate graph.

## 7. V2 family fixed point

### 7.1 Preserve V1

Add a new family certificate and material vocabulary; do not add variants to
`GeneratedFixedPointMaterialLocator` or change
`GeneratedFamilyFixedPointCertificate::material/latest_materials`.

Conceptually:

```rust
enum GeneratedFamilyAffineMaterialLocator {
    InitialV1 {
        solve_ordinal: usize,
        material: GeneratedFixedPointMaterialLocator,
    },
    AffineEpoch {
        epoch_ordinal: usize,
        sector_attempt_ordinal: usize,
    },
}

struct GeneratedFamilyAffineFixedPointCertificate {
    base: Arc<GeneratedFamilyFixedPointCertificate>,
    epochs: Box<[GeneratedFamilyAffineEpoch]>,
    final_statuses: Box<[GeneratedFamilyAffineSectorStatus]>,
    // config, limits, fingerprints, stats
}
```

The initial affine source for a sector is the exact latest V1 live queue
resolved from the base certificate. A committed epoch owns the complete
sector owner and its effective residual queue. A hard or resource failure is
transactional: the failed attempt may be retained as an interruption record,
but the sector's current material remains the last fully replayed material.

Sectors are scheduled in the existing certified subsector-first solve order.
Same-rank antichains may be computed in parallel provided the result transcript
is committed in deterministic order and all resource accounting remains
replayable.

### 7.2 Exact configured fixed point

Do not stop because two residual counts, manifests, or locator-vector lengths
are equal. Those are not set-equality proofs.

An `ExactConfiguredFixedPoint` status is valid only if one complete,
deterministic epoch:

1. starts from the exact current residual queue;
2. visits every source leaf, inventory terminal, group, prepare-point layer,
   matcher outcome, and target permitted by the configured full schedule;
3. finishes without a resource/failure interruption;
4. produces a complete conservation certificate; and
5. accepts zero new affine rules.

An accepted local affine certificate has at least one `Applicable` leaf.
Therefore any accepted group transition removes a nonempty certified rule
domain from the current residual union and requires another epoch. Conversely,
under a complete configured pass, zero accepted transitions plus terminal
conservation proves that the effective residual output equals the input
residual union for that configured search.

If search depth grows by epoch, a no-change shallow epoch is not a fixed
point. Either each epoch uses the full configured maximum schedule, or the
scheduler delays the fixed-point status until the final configured depth has
also completed with no accepted transition.

Honest terminal statuses include:

- `CoveredByGeneratedRules`;
- `ExactConfiguredFixedPoint { residual_summary }`;
- `ExhaustedAtMaximumEpochs { residual_summary }`;
- `ExhaustedAtConfiguredDepth { residual_summary }`;
- `NotSelectedByPolicyBound { residual_summary }`;
- `ResourceLimited { interruption, latest_material }`; and
- `Failed { interruption, latest_material }`.

Maximum-epoch/depth exhaustion is not closure, a fixed point, or a master
proof. `CoveredByGeneratedRules` requires an empty effective residual queue,
not merely zero accepted rules in the last pass.

### 7.3 Reduction and lower sectors

Affine derivation remains sector-local and uses fresh generated IBP/LI row
material. It must not inject already solved lower-sector rules into the global
parametric row span merely to force closure. The concrete reduction engine
may recursively apply installed lower-sector providers to RHS integrals; a
later family dependency/table certificate can make that application order
explicit and replayable.

This matches LiteRed's separation between `SolvejSector` derivation and
`IBPSelect`/`IBPReduce` dependency substitution rather than requiring a
topology-specific closure module.

## 8. Staged implementation plan

1. **Freeze V1 behavior.** Add schema/replay/provider regression tests before
   changing any shared concrete-result API.
2. **Finish the sector owner.** Implement limits/stats, group-pass outcomes,
   exhaustive terminal census, payload comparison, replay, and Arc-authority
   validation around the vocabulary already in
   `generated_sector_affine_effective_coverage.rs`.
3. **Add owner point classification.** Include exact source classification,
   Boolean terminal resolution, `F(n) == n`, relative predicates, unique-leaf
   validation, and exact/one-below query budgets.
4. **Refactor private concrete authority.** Add the coordinate/affine enum,
   authority-independent ordering accessor, redacted formatting, and replay
   dispatch without exposing the private relation.
5. **Implement sealed application.** Specialize only after locator and leaf
   reauthentication; retain guards and verify unit LHS/subsector/descent.
6. **Add the affine provider tier and V2 stack builder.** Preserve the exact
   provider order in section 6 and preflight all installed owner Arcs.
7. **Introduce the source-neutral residual authority.** Reuse internal
   algorithms through sealed views or V2 wrappers while leaving V1 schemas
   unchanged.
8. **Implement the effective residual queue.** Preserve exceptional predicates
   and unconsumed target roots, then replay one topology-neutral second epoch.
9. **Implement the V2 family scheduler.** Own the replayed V1 base, commit
   sector epochs transactionally, certify exact configured fixed points, and
   retain honest exhaustion/interruption statuses.
10. **Add replayable dependency/back-substitution material.** Keep it separate
    from rule derivation and use the generic reduction provider recursively.
11. **Validate concrete families only after the generic chain passes.** Start
    with one-loop scalar/tensor reductions, then bounded two-loop sunset
    points, then three- through five-loop vacuum families.

## 9. Staged tests

### 9.1 V1 freeze

- existing family fixed-point schemas, material locators, replay, and statuses;
- current provider decisions for descending, uncovered, unsupported, and
  equality-conditional points;
- unchanged global candidate and shared-row-span allocation identity.

### 9.2 Sector owner and conservation

- generated `001` acceptance/exhaustion path;
- generated `011` and `101` negative/residual paths;
- topology-neutral reject-then-accept and multi-target models until genuine
  generated examples exist;
- one terminal classified exactly once as empty, residual root, or partition;
- one consumed target's applicable plus exceptional leaves exactly conserve
  its root;
- missing/duplicate group pass, wrong inventory Arc, locator tampering, and
  corrupted child range fail replay;
- measured exact limits pass and representative one-below limits fail before
  allocation or mutation.

### 9.3 Concrete application and privacy

- source point authenticates exactly one owner and rule locator;
- off-map point fails the `F(n) == n` membership check;
- applicable leaf yields a unit-LHS concrete reduction with complete guards
  and strict descent;
- exceptional-domain/leak leaves remain residual;
- replay survives provider drop and reduction-cache retention;
- `Debug` and all public accessors reveal neither relation nor exact private
  predicates/shift-bearing condition sources;
- tampered locator, source, RHS, guard, descent witness, or owner Arc fails.

### 9.4 Provider order

- zero, symmetry, and selected-master precedence remain unchanged;
- global descending rule delegates through both conditional tiers;
- V1 equality-locus rule wins before affine fallback;
- affine rule is reached from both global `Uncovered` and `Unsupported` roots;
- final unsupported error appears only after equality and affine tiers are
  exhausted;
- missing sector and residual leaves delegate exactly once;
- provider query/retention limits are exact and replayable.

### 9.5 Recursive residual epochs

- initial V1 queue -> V2 inventory -> owner -> effective queue replay;
- exceptional child re-enters with all relative predicates intact;
- unconsumed target root re-enters with its map and inherited guards intact;
- prior successful rule domain is absent from the next residual source;
- every concrete sample point belongs to exactly one upstream rule domain or
  latest residual leaf;
- two complete epochs are deterministic under replay and parallel test
  execution.

### 9.6 Family fixed point

- exact binding to the V1 latest material;
- certified subsector-first order and deterministic same-rank commit order;
- hard/resource failure does not commit partial material;
- full configured no-change epoch yields `ExactConfiguredFixedPoint`;
- shallow no-change and maximum-epoch exhaustion do not;
- empty effective queue yields `CoveredByGeneratedRules`;
- recursive RHS reduction reaches installed lower-sector rules while leaving
  genuine uncovered/master terminals explicit;
- material locator, epoch order, final status, and owner-Arc tampering fail
  replay.

### 9.7 Concrete validation fixtures

- one-loop scalar integrals and multiple tensor numerator ranks;
- bounded two-loop massive vacuum/sunset powers;
- subsequently bounded three-, four-, and five-loop massive vacuum fixtures;
- compare RustRed's reduced nonmaster terms and coefficients against Vakint
  without substituting master topologies;
- never copy Vakint's FORM recurrences into RustRed and never invoke FORM.

All validation must use licensed, GMP-enabled Symbolica. Do not enable
`no_gmp`. Test suites are expected to run in parallel, for example through
`cargo nextest run -j4`; this note itself ran no tests.

## 10. LiteRed `SolvejSector` correspondence

The controlling source is
[`LiteRed2026.m:2323-2526`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2323).
The correspondence is exact at the transition level:

| LiteRed behavior | Source anchor | RustRed V2 counterpart |
|---|---|---|
| Take the next persisted residual case group with `cases=Reverse@First@noRules` | [`LiteRed2026.m:2430-2434`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2430) | Source-ordered inventory groups and deterministic group-pass order |
| Build fresh IBP/LI equations at prepared points | [`LiteRed2026.m:2475`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2475) | Generic generated rows, affine prepare schedule, and branch re-elimination |
| Recenter a provisional pivot and select its exact case | [`LiteRed2026.m:2484-2494`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2484) | Pivot/target matcher and split-recentered private relation |
| Install the rule on `target case && !WhenBad` | [`LiteRed2026.m:2488-2495`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2488) | Applicable relative leaves become sealed rule locators |
| On accepted `WhenBad`, append `target case && WhenBad` to bad conditions and delete the complete target from the current group | [`LiteRed2026.m:2495-2500`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2495) | Consume target exactly once; exceptional children become next-epoch residual work |
| If `WhenBad` is literal `True`, exclude only the pivot and leave the target available | [`LiteRed2026.m:2501-2505`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2501) | `IdenticallyBad`/unsupported local attempt consumes no target; later matcher outcomes may try it |
| Rebuild `noRules` from prior residual cases or accumulated bad conditions | [`LiteRed2026.m:2522`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2522) | Effective residual queue becomes the exact source of the next V2 epoch |
| Compute denominator and numerator/boundary failure conditions | [`LiteRed2026.m:2565-2569`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2565) | Target-local condition accumulator, affine boundary pullbacks, numerator gates, and relative partition |

The central invariant is therefore:

> A generated affine pivot is not a rule when it is found or matched. It
> becomes one sealed conditional rule only after target authority, local
> `WhenBad`, strict descent, sequential target consumption, and sector-owner
> conservation all replay. Every bad, unsupported, or unconsumed part remains
> explicit residual work.

LiteRed also keeps derivation separate from lower-sector substitution.
`IBPSelect` begins at
[`LiteRed2026.m:3820`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3820),
discovers sector dependencies while selecting rule tables, and `IBPReduce`
starts at
[`LiteRed2026.m:3933`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3933).
It applies already prepared lower-sector tables at
[`LiteRed2026.m:3960-3972`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3960)
and then performs intra-sector layering/substitution at
[`LiteRed2026.m:3977-4003`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3977).
RustRed should obtain the same behavior from its generic certificate-backed
provider/reduction engine and a future replayable dependency transcript, not
from authored sunset, banana, or loop-count-specific recurrences.

## 11. Non-negotiable invariants

1. V1 certificate schemas and replay meanings remain unchanged.
2. No affine rule enters the globally valid candidate database.
3. No private relation or exact private predicate table becomes public.
4. Only a certificate-owned locator can authorize concrete application.
5. Every input terminal is conserved exactly once.
6. Exceptional and unconsumed work re-enters through its exact upstream
   authority.
7. A resource/failure interruption commits no partial epoch.
8. Fixed point means a complete configured no-transition proof, not equal
   counts or exhausted budget.
9. Exhaustion and unsupported leaves are not masters.
10. Production algorithms remain topology- and loop-count-independent.
11. Vakint is a concrete-result oracle only; FORM is neither a runtime nor a
    source of copied RustRed recurrences.
12. Symbolica is licensed and GMP-enabled, and tests run in parallel.

## 12. Source-neutral Boolean residual collection checkpoint

`generated_affine_residual_boolean_cover.rs` now implements the generic bridge
from one `GeneratedAffineResidualSourceAuthority` to the Boolean-refined source
set consumed by affine recognition. It contains no topology names, denominator
shapes, loop-count branches, or authored recurrences. Concrete one- and
two-loop families occur only in tests.

The collection performs exactly one authority-wide replay and one ordered scan
of source work ordinals. Its persisted locator is only
`(source_work_item_ordinal, terminal_ordinal)`: neither the initial/prior source
kind nor a private source-case identifier crosses the seam. Initial Ready
sources own actual sealed V1 product-locus covers and emit every V1 terminal in
node order. Prior-effective sources emit one identity terminal while retaining
unsupported atoms/reasons, actionable maps/guards/constants/free positions,
and exceptional predicates through narrow lifetime-bound views. The private
distinction between unprocessed and unconsumed actionable origins is used for
authentication but is intentionally collapsed in the outward outcome.

Construction and replay use complete checked censes for source navigation,
binding, prior positional references, aggregate V1 work, terminal
conservation, comparison work, and collection-owned memory. The memory model
excludes the pre-existing shared source graph, accounts allocator-independent
logical slots, and uses the V1 child-limit peak rather than trusting historical
child peak counters when admitting a fresh replay. Each sealed child's retained
size and comparison census is recomputed from its raw private V1 shape before
replay. Base terminal/comparison storage is admitted before source replay; each
prior item's complete positional reference count is admitted before its first
positional lookup.

The focused tests cover:

- differential terminal ordering against independent V1 compilation on the
  natural massive two-loop `011` fixture, including Boolean-empty and Ready
  terminals;
- initial and prior source drop followed by replay;
- natural prior exceptional payloads (`001`) and unconsumed plus
  exceptional-domain payloads (`011`), with field-by-field borrowed identity
  checks;
- four simultaneous replay/view traversals of one retained certificate;
- exact and one-below postconditions for every V2 limit, actual compiler
  thresholds for all V2 limits, and a formula-derived exact one-Ready-child
  preflight/replay envelope;
- parent statistics, terminal locator/binding, adjacent child retained/peak,
  and child comparison-census tampering;
- redacted certificate/view/error formatting and the deliberate absence of a
  delegated `Error::source`.

Current generated live queues prune coordinate-proved-empty leaves upstream,
so `SourceProvedEmpty` is a defensive collection outcome rather than a natural
queue fixture. The natural depth-zero `001`/`011` prior fixtures likewise do
not produce unsupported, unprocessed-actionable, or exceptional-leak roots;
those match arms remain defensive until a legitimate generated fixture reaches
them. No synthetic topology-specific authority is fabricated merely to mark
those branches covered.

Independent GMP-enabled parallel validation on 2026-08-21 completed without
changing any of the sealed sources:

- source-neutral collection: nextest run
  `2ca78899-296a-4854-b1fb-f3f10427bf69`, 7/7 passed, 629 skipped,
  `-j4`, 168.559 seconds;
- immediately affected product-cover and unified-source layers: nextest run
  `e5749edf-1b6c-4cdb-8cf2-5012715e712e`, 22/22 passed, 614 skipped,
  `-j4`, 38.346 seconds; and
- stable SHA-256 values: collection
  `4118a7cac06a3ffb79cc5109ef427d911782a86295a0316870d60f586b3d8808`,
  source authority
  `86581148138b2b736de9b693f252acc049b6d63116a3af4aab22cf01b06977e1`,
  product cover
  `50fabc5da3bbeb0a606a0b84a1b7f32b0fe9db1df594556f35e0cc88f60c4f98`,
  and frozen decision DAG
  `86fef90ed96d5c57de8775411150db59eab0d682d9acf9889d3618650a9e3025`.

## 13. Fresh residual integer-system checkpoint

The residual affine integer compiler now has a crate-private, consuming fresh
compilation path for the V2 chain. A successful fresh compilation can be split
exactly once into its retained certificate `Arc` and a non-cloneable plan
authorization. The corresponding parametric-composition entry point consumes
that authorization, checks exact `Arc` identity, and skips only the redundant
V1 integer-certificate replay. The public V1 compile, replay, error text,
schema, payload, limits, and statistics remain unchanged; the ordinary public
plan path still performs the complete replay.

The fresh path records the raw initialized allocation/state census, integer
bit work, frontier peak, retained representation census, two independently
allocated replay-comparison operands, and the actual logical peak. Its memory
envelope covers both retained and transient work. GMP payload bytes are bounded
with the conservative aggregate formula

```text
ceil(total_integer_bits / 8) + integer_count * word_bytes
  + max(integer_count - 1, 0)
```

where the last term covers the worst possible sum of per-integer byte-rounding
slack. The replay validator accepts the compiler's natural depth-first pivot
order rather than assuming numerically sorted pivots. It allocation-freely
proves that pivot and sorted-free positions form a unique, disjoint partition
of the ambient coordinates and binds both slices to the retained affine map.
Unsupported fresh attempts retain all four transient work scalars and the
actual peak instead of collapsing them to an unmeasured reason.

Focused generic tests cover fresh success and unsupported outcomes, consuming
authorization and exact retained-`Arc` binding, adjacent scalar tampering,
exact/one-below envelope limits, a natural unsorted pivot order `[1, 0]`, two
130-bit GMP integers (including aggregate rounding), and a valid one-index
identity plan proving that the fresh path omits only integer replay. No test or
production branch dispatches on a family name, propagator shape, topology, or
loop count.

Licensed GMP-enabled validation completed in parallel on 2026-08-21:

- implementation run `aa5e45c5-b8ae-41fa-a2ba-d4e97439f146`: 10/10 passed,
  632 skipped, `-j4 --no-fail-fast`;
- independent read-only audit run `cc3994bd-279c-4e0d-b33f-a6be76d72f30`:
  10/10 passed, 632 skipped, `-j4 --no-fail-fast`;
- `cargo fmt --all -- --check` and `cargo check --tests` passed; and
- stable SHA-256 values: residual integer system
  `b10aead59dbbe3d98f433328627f9d9a861879df63847a9d4bc360a78e247cfa`,
  parametric coefficient integration
  `472adefab46ad77bd85ffd7fbc279b65f9c65473e68fd5488f9fd42cfc6e763e`,
  freshness design
  `4494027b1062147a57dedadce87af3b5f022fec84f21723bf832daa93feed7e4`,
  and unchanged frozen decision DAG
  `86fef90ed96d5c57de8775411150db59eab0d682d9acf9889d3618650a9e3025`.
