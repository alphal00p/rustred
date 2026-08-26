# RustRed capability-reference and implementation gap audit

Date: 2026-08-14

## Status and reading rule

This is a source-only audit of the governing RustRed scope against the
vendored LiteRed2, Symbolica, and Vakint trees.  No Cargo command, Mathematica,
or FORM was run while preparing it.  Consequently:

- **foundation present** means that a generic, reusable Rust implementation
  with the stated boundary exists in `src/`;
- **partial** means that useful generic machinery exists but does not yet
  implement the complete adopted mathematical capability;
- **missing** means that the audited production surface does not provide the
  corresponding behavior; and
- none of those labels is a test-pass claim.

The acceptance scope remains
[`rustred_scope_and_acceptance.md`](rustred_scope_and_acceptance.md).  This
document prevents its requirements from being mistaken for completion
evidence and supplies a source-informed closure roadmap.  LiteRed2 entries are
conceptual and mathematical evidence, not an API, architecture, sequencing,
ordering, global-state, or bug-compatibility contract.  Priorities order
implementation work within RustRed's independently stated capability scope.

## Executive result

### Completed foundations, but not complete RustRed capability coverage

The following foundations are genuinely topology- and loop-count independent:

- authenticated complete affine families in
  [`generic_family.rs:1-6`](../../src/generic_family.rs#L1) and
  [`generic_family.rs:420-486`](../../src/generic_family.rs#L420);
- deterministic completion of a short independent propagator list with ISPs
  in [`automatic_isps.rs:1-19`](../../src/automatic_isps.rs#L1);
- raw fully parametric ordinary IBP and LI generation in
  [`parametric_ibp.rs:1-19`](../../src/parametric_ibp.rs#L1);
- guarded sparse-elimination foundations with replay in
  [`parametric_elimination.rs:1-25`](../../src/parametric_elimination.rs#L1);
- a topology-independent, honest-uncovered rule-application kernel in
  [`reduction_engine.rs:1-17`](../../src/reduction_engine.rs#L1);
- a topology-independent Symbolica tensor parser in
  [`symbolica_tensor_numerator.rs:1-9`](../../src/symbolica_tensor_numerator.rs#L1),
  family lowering in
  [`generic_tensor_family.rs:1-10`](../../src/generic_tensor_family.rs#L1), and
  tensor/scalar composition in
  [`tensor_reduction_engine.rs:1-10`](../../src/tensor_reduction_engine.rs#L1);
- generic vacuum Feynman-polynomial construction in
  [`feynman_polynomials.rs:1-23`](../../src/feynman_polynomials.rs#L1); and
- Symbolica built explicitly with GMP, not `no_gmp`, in
  [`Cargo.toml:13-17`](../../Cargo.toml#L13).

These components are suitable building blocks.  They do **not** collectively
implement `SolvejSector`, completed `ToAB`, the denominator-set/PF layer, full
sector and symmetry discovery, the complete Vakint input/application layer,
or the rest of RustRed's adopted mathematical capabilities.

### Highest-priority incomplete paths

| Priority | Gap | Concrete evidence | Required closure |
|---|---|---|---|
| P0 | Generated affine `WhenBad` rule compilation is not integrated end to end. | Condition accumulation and compilation/descent remain crate-private at [`lib.rs:68-80`](../../src/lib.rs#L68).  The descent phase explicitly stops before domain conditions and boundary pullbacks at [`generated_residual_affine_when_bad_descent.rs:1-6`](../../src/generated_residual_affine_when_bad_descent.rs#L1); the compilation header calls those later slices at [`generated_residual_affine_when_bad_compilation.rs:1-12`](../../src/generated_residual_affine_when_bad_compilation.rs#L1). | Connect generated rows, condition accumulation, boundary pullbacks, strict descent, direct bad formula, relative partition, recursive subsector feedback, fixed-point closure, and replay into one public generic rule provider. |
| P0 | The production-facing Vakint adapter is a two-loop special case. | It accepts canonical `I2L` and a deliberately small syntax at [`vakint_adapter.rs:1-15`](../../src/vakint_adapter.rs#L1), owns `VakintTwoLoopAdapter`/`TwoLoopReductionPipeline` at [`vakint_adapter.rs:154-186`](../../src/vakint_adapter.rs#L154), and reduces through that pipeline at [`vakint_adapter.rs:231-280`](../../src/vakint_adapter.rs#L231). | Replace the production path with generic family/topology matching, generic tensor parsing/lowering, and application of freshly discovered authenticated rules.  Keep loop-specific adapters only as oracle fixtures. |
| P0 | Completed LiteRed `ToAB`/s-basis semantics are absent. | The current layer explicitly says coefficients remain in `K(n)` and must not be serialized as `ABIBP` at [`shift_operators.rs:20-27`](../../src/shift_operators.rs#L20). | Implement free-index elimination through `A_i B_i`, common-shift choice, left factorization of lowerings, `FromAB`/tilde conjugation, and exact round trips. |
| P0 | A reusable, authenticated rule artifact is not yet closed. | The append-only database is a replay oracle that rebuilds every prefix and accepts only structurally prevalidated rows at [`persistent_parametric_elimination.rs:1-16`](../../src/persistent_parametric_elimination.rs#L1).  Symbolica `serde`/`bincode` features are not enabled in [`Cargo.toml:13-17`](../../Cargo.toml#L13). | Add stable schema/version, family and ordering fingerprints, variable map, guards, provenance, replay proof, atomic recovery, and a demonstrated Symbolica state remap/round trip. |
| P1 | Overcomplete/dependent propagator sets and partial fractions are absent. | ISP completion sends these inputs to a future layer at [`automatic_isps.rs:14-19`](../../src/automatic_isps.rs#L14), while LiteRed implements them at [`LiteRed2026.m:465-686`](../../vendor/LiteRed2/Source/LiteRed2026.m#L465). | Port denominator-set relations, basis enumeration, PF Groebner construction/reduction, and subsector mapping generically. |
| P1 | Full sector, zero-sector, symmetry, and cross-basis closure is incomplete. | Current symmetry discovery is a bounded internal-vacuum integer search and treats exhaustion honestly as resource-limited at [`symmetry_discovery.rs:1-18`](../../src/symmetry_discovery.rs#L1).  LiteRed's full paths are [`LiteRed2026.m:2936-3520`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2936). | Port `AnalyzeSectors`, proved zero/nonzero/simple/basis status, polynomial-signature candidates, internal/external/cross-basis maps, orbit rules, and authenticated incompleteness state. |
| P1 | Arbitrary Vakint spectator weights cannot yet pass through exact projection/application. | Vakint retains numerator atoms separately at [`vakint/src/lib.rs:2187-2197`](../../vendor/gammaloop/crates/vakint/src/lib.rs#L2187).  RustRed preserves them while parsing, but `try_weighted_sources` rejects anything not convertible to the family coefficient map as `DeferredWeight` at [`symbolica_tensor_numerator.rs:307-337`](../../src/symbolica_tensor_numerator.rs#L307). | Factor every term into an opaque spectator Atom and an exact family coefficient; project/reduce only the latter and reattach the former without registering it as a kinematic variable. |
| P1 | Tensor projection is vacuum-only and not yet scalable to the full Vakint rank surface. | External indexed vectors are rejected at [`generic_tensor_projector.rs:1-17`](../../src/generic_tensor_projector.rs#L1); defaults stop at rank eight/105 pairings at [`generic_tensor_projector.rs:45-91`](../../src/generic_tensor_projector.rs#L45), and orbit reduction is future work at [`generic_tensor_projector.rs:953-956`](../../src/generic_tensor_projector.rs#L953). | Add the external-vector covariant basis, orbit-reduced projector generation, cached exact Gram solves, and rank-10 validation while retaining explicit guards and resource limits. |
| P2 | Feynman-parametric syzygies, dimensional recurrence, differential systems, denominator-only APIs, and graph utilities are incomplete or absent. | LiteRed source regions are catalogued below. | Implement these after the P0/P1 reduction engine is generic and validated; do not substitute loop-specific formulas. |

## LiteRed2 capability/reference inventory

The public package declaration occupies
[`LiteRed2026.m:39-303`](../../vendor/LiteRed2/Source/LiteRed2026.m#L39).
The table below inventories every uncommented LiteRed-context symbol listed as
public-facing in that header.  It excludes private setup helpers, the internal
`Global` to-do flag, and commented prototypes that the source itself says are
unimplemented.  The inventory is non-normative: rows help identify possible
capabilities and acceptance oracles, but do not require RustRed to reproduce
Mathematica names, APIs, globals, internal algorithms, sequencing, or bugs.

Classification keys:

- **C0**: central family/IBP/reduction mathematical capability; blocks the
  first complete RustRed scalar engine.
- **C1**: adopted extended mathematical capability.
- **S**: reference state or configuration; a typed RustRed equivalent is
  required only where it changes mathematical results, artifact identity, or
  reproducibility.
- **A**: graph, import/export, reporting, or visualization adapter.  It is an
  optional product capability unless independently adopted by RustRed and can
  never define the algebraic core.
- **X**: alternative accelerator entry point.  There is no compatibility
  obligation for the named executable; any adopted result capability must
  remain available through pure Rust and Symbolica.
- **H**: deprecated, raw, or experimental surface.  It creates no compatibility
  obligation unless RustRed independently adopts the underlying capability.

| LiteRed public surface | Class / sequence | RustRed acceptance interpretation | Current source-level status | LiteRed source |
|---|---|---|---|---|
| `$LiteRedVersion`, `$LiteRedReleaseDate`, `$LiteRedLog`, `$LiteRedMonitor`, `$NamingFunction`, `NamingFunction`, `$LiteRedSyzygy` | S / P3 | Versioned schema, diagnostics policy, deterministic index naming, and syzygy mode where these affect adopted semantics or reproducibility. | Partial typed configuration; no Mathematica-surface compatibility goal. | [`:42-43,243-285`](../../vendor/LiteRed2/Source/LiteRed2026.m#L42) |
| `$ActiveBases`, `BasisDirectory`, `Definitions`, `ExecuteDefinitions`, `CheckDefinitions`, `CurrentState` | S / P1 | Explicit session/catalog object, validated family definitions, and queryable state; no process-global mutable truth. | Partial family objects; complete session/catalog behavior missing. | [`:57,92-98`](../../vendor/LiteRed2/Source/LiteRed2026.m#L57) |
| `CompleteMomentaFlow`, `GraphToDs`, `GraphToAmplitude` | A+C1 / P2 | Generic graph/routing intake that produces an authenticated family/amplitude. | Missing. | [`:60-62`](../../vendor/LiteRed2/Source/LiteRed2026.m#L60), [`:940-1149`](../../vendor/LiteRed2/Source/LiteRed2026.m#L940) |
| `PowerShifts` | S+C0 / P0 | Symbolic denominator shifts are family data and participate in fingerprints, mappings, and generated relations. | Foundation present for affine families; downstream capability integration incomplete. | [`:65`](../../vendor/LiteRed2/Source/LiteRed2026.m#L65) |
| `NewDsSet`, `NewDsBases`, `SetToBasesRule`, `DsSetQ`, `Relations` | C0 / P1 | Dependent/overcomplete denominator-set model, its independent bases, relations, and checked predicates. | Missing beyond independent-family foundations. | [`:68-73`](../../vendor/LiteRed2/Source/LiteRed2026.m#L68) |
| `GeneratePFGB`, `PFGB`, `PFReduce`, `PFjSubsectors` | C0 / P1 | Reproducible partial-fraction ideal/basis, reduction, and integral-subsector mapping. | Missing. | [`:76-79`](../../vendor/LiteRed2/Source/LiteRed2026.m#L76) |
| `NewBasis`/`NewDsBasis`, `DsBasisQ` | C0 / P0-P1 | Construct, authenticate, query, persist, and recover generic bases, including short-list ISP completion and denominator-set selection. | Partial: complete affine basis and short independent completion only. | [`:82-83`](../../vendor/LiteRed2/Source/LiteRed2026.m#L82) |
| `SectorsPattern`, `Ds`, `NDs`, `LMs`, `EMs`, `SPs`, `Parameters`, `MIs` | S+C0 / P0-P1 | Typed, immutable family/sector/master views whose values are included in fingerprints. | Partial typed equivalents; adopted state capabilities incomplete. | [`:85-92`](../../vendor/LiteRed2/Source/LiteRed2026.m#L85) |
| `j`, `Toj`, `Fromj`, `js`, `jSector`, `jSubsectors` | C0 / P0 | Typed integral keys and exact expression conversion, sector and subsector maps, including optional dimension slot semantics. | Generic key/sector foundations present; conversion capability incomplete. | [`:101-108`](../../vendor/LiteRed2/Source/LiteRed2026.m#L101) |
| `jsSignature`, `jsSignaturePermutations`, `jSignature` | C0 / P1 | Stable polynomial/integral signatures used only as candidate indices, followed by exact verification. | Partial signature/fingerprint machinery; adopted signature capability not closed. | [`:111-113`](../../vendor/LiteRed2/Source/LiteRed2026.m#L111) |
| `ToAB`, `FromAB`, `FromTildeAB`, `AtoLeft`, `A`, `B`, `InverseTildeConjugate`, `TildeConjugate`, `ABIBP`, `ABLI`, `ABIBPLI` | C0 / P0 | Completed noncommutative s-basis conversion, inverse conversions, conjugations, and persisted generated operator systems. | Partial primitive shift-word semantics only; completed `ToAB` missing. | [`:116-119`](../../vendor/LiteRed2/Source/LiteRed2026.m#L116) |
| `MakeOrderMatrix`, `jsOrder`, `jComplexity`, `jVars`, `Highj`, `Highjs`, `HighjIndex` | C0+S / P0 | Persisted total ordering and exact complexity queries used consistently by elimination, descent, masters, and replay. | Generic ordering foundations present; solver-wide ordering authority incomplete. | [`:122-128`](../../vendor/LiteRed2/Source/LiteRed2026.m#L122) |
| `Collectj`, `CollectjList`, `SimplifyFunction`, `SimplifyAlways`, `jPattern`, `Factor1`, `Factor2`, `Factor3` | H+C1 / P2 | Evidence for deterministic collection/factor/simplification semantics; `Collectj` is marked outdated in source. | Symbolica primitives exist; no reason to reproduce these Mathematica API names. | [`:131-137`](../../vendor/LiteRed2/Source/LiteRed2026.m#L131) |
| `Solvej`, `SolvejSector`, `SubstituteAlways`, `CheckZeroAlways`, `CheckZeroFunction`, `NMIs`, `RRs`, `NoRules`, `MaxDepth`, `BloodhoundSearch` | C0+S, `BloodhoundSearch` H / P0 | Adaptive guarded recurrence derivation, persistent residual system, configurable substitution/zero checks, honest no-rule outcomes, depth budgets, and diagnostic search. | Strong elimination/rule foundations, but complete recursive sector solver is missing. | [`:141-152`](../../vendor/LiteRed2/Source/LiteRed2026.m#L141) |
| `GenerateIBP`, `IBP`, `LI`, `IBPLI` | C0 / P0 | Generate all raw parametric ordinary and separate LI relations for arbitrary `L,E`; store their authenticated combined system without applying sector rules. | Foundation present for complete affine families; overcomplete integration and full downstream consumption missing. | [`:155-158`](../../vendor/LiteRed2/Source/LiteRed2026.m#L155), [`:1799-1831`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1799) |
| `AnalyzeSectors`, `ZeroSectors`, `NonZeroSectors`, `SimpleSectors`, `BasisSectors`, `ZerojRule`, `CutDs`, `BiggestSectors` | C0+S / P1 | Proved sector classification, cut-aware zero rules, and explicit unknown/resource-limited state. | Partial providers; full analysis missing. | [`:161-169`](../../vendor/LiteRed2/Source/LiteRed2026.m#L161), [`:2936-3108`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2936) |
| `SectorHierarchy`, `SectorLayer`, `jLevel` | C0+S / P1 | Stable hierarchy/layer queries tied to the persisted ordering and cut policy. | Partial sector inventory; adopted query capability incomplete. | [`:172,292`](../../vendor/LiteRed2/Source/LiteRed2026.m#L172) |
| `FindShifts`, `FindSymmetries`, `UniqueSectors`, `MappedSectors`, `SectorsMappings`, `jSymmetries`, `jRules`, `SR` | C0+S / P1 | Candidate generation plus exact momentum-map proof, complete orbit classification, and reusable integral rules. | Partial bounded internal-vacuum search; complete candidates/orbits missing. | [`:175-183`](../../vendor/LiteRed2/Source/LiteRed2026.m#L175), [`:3111-3475`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3111) |
| `ZeroSectorQ`, `NonZeroSectorQ`, `MappedSectorQ`, `UniqueSectorQ` | S+C0 / P1 | Typed four-state-or-better queries that distinguish proved yes/no from unknown/resource-limited. | Partial providers; adopted query capability incomplete. | [`:186-189`](../../vendor/LiteRed2/Source/LiteRed2026.m#L186) |
| `AddjRule`, `RefreshMIs`, `IdentifyMIs`, `ToMIsRule`, `MyMIs`, `ToMyMIs` | C0+S / P1 | Authenticated user rules, recomputed master candidates, user-selected masters, and exact conversions without treating uncovered leaves as masters. | Partial master policy/application foundations; workflow incomplete. | [`:192-196`](../../vendor/LiteRed2/Source/LiteRed2026.m#L192), [`:3626-3798`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3626) |
| `MIsHierarchyGraph` | A / P3 | Possible deterministic export of the master hierarchy from semantic state. | Optional adapter not currently adopted. | [`:199`](../../vendor/LiteRed2/Source/LiteRed2026.m#L199) |
| `FindExtSymmetries`, `ExtUniqueSectors`, `ExtMappedSectors`, `jExtRules`, `ExtSectorsMappings` | C0+S / P1 | Cross-basis/external symmetry search, exact proof, orbit state, and transport rules. | Partial symbolic transport; complete search/orbit closure missing. | [`:202-206`](../../vendor/LiteRed2/Source/LiteRed2026.m#L202), [`:3476-3520`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3476) |
| `AttachGraph`, `jGraph`, `GraphSort`, `FeynGraphContract` | A+C1 / P2 | Graph metadata attachment, canonicalization, contraction, and conversion that preserve the underlying family fingerprint. | Missing. | [`:209-212`](../../vendor/LiteRed2/Source/LiteRed2026.m#L209), [`:5527-6042`](../../vendor/LiteRed2/Source/LiteRed2026.m#L5527) |
| `FeynGraphPlot`, `jGraphPlot`, `jGraphFeynMP`, `SawToothLine`, `WavyLine`, `CurlyLine` | A / P3 | Optional deterministic visualization/export adapters; never a reduction dependency. | Missing. | [`:213-218`](../../vendor/LiteRed2/Source/LiteRed2026.m#L213) |
| `GenerateFeynParUF`, `FeynParUF`, `FeynParGdG`, `FeynParUVM` | C1+S / P2 | Generic authenticated `U`, `F`, `G`, derivatives/Gram data, and related matrices with exact convention maps. | `U/F/G` foundation present; adopted mathematical capability incomplete. | [`:221-223`](../../vendor/LiteRed2/Source/LiteRed2026.m#L221), [`:4205-4401`](../../vendor/LiteRed2/Source/LiteRed2026.m#L4205), [`:4796-4804`](../../vendor/LiteRed2/Source/LiteRed2026.m#L4796) |
| `LP`, `GramP`, `GramPFunction`, `GramM` | C1 / P2 | Lee--Pomeransky and Gram utilities over the authenticated coefficient field. | Partial data through family/Feynman code; adopted utility capability incomplete. | [`:226-229,295`](../../vendor/LiteRed2/Source/LiteRed2026.m#L226) |
| `FactorizeFP`, `FactorizejSector`, `PolyNForm`, `PolySignature`, `PolySignaturePermutations`, `CRulesLE` | C1 / P2 | Exact factorization/normal forms/signature candidates followed by exact verification and explicit budgets. | Missing as a coherent public layer. | [`:232-240`](../../vendor/LiteRed2/Source/LiteRed2026.m#L232), [`:4451-4609`](../../vendor/LiteRed2/Source/LiteRed2026.m#L4451) |
| `DiskSave`, `DiskRecover` | C0 / P0 | Stable authenticated artifacts, atomic recovery, schema and source hashes, guards, provenance, and mandatory replay. | Partial in-memory/replay artifacts; durable Symbolica round trip incomplete. | [`:246-247`](../../vendor/LiteRed2/Source/LiteRed2026.m#L246), [`:1153-1264`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1153) |
| `IBPReduce`, `IBPSelect` | C0 / P0-P1 | Demand-driven rule selection and bottom-up exact reduction with explicit uncovered leaves and master policy. | Generic application kernel present; discovery/selection integration incomplete. | [`:250-251`](../../vendor/LiteRed2/Source/LiteRed2026.m#L250), [`:3801-4134`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3801) |
| `FermatIBPReduce`, `SparxIBPReduce` (and commented `FlintIBPReduce`) | X / P3 | Evidence that optional acceleration can preserve exact results; RustRed's pure Rust/Symbolica path remains authoritative. | External executables and compatibility names are out of scope. | [`:252-255`](../../vendor/LiteRed2/Source/LiteRed2026.m#L252) |
| `LoweringDRR`, `RaisingDRR`, `LowerDim`, `RaiseDim` | C1 / P2 | Guarded dimensional recurrence and exact dimension-shift rules. | Missing. | [`:258-260`](../../vendor/LiteRed2/Source/LiteRed2026.m#L258), [`:4612-4669`](../../vendor/LiteRed2/Source/LiteRed2026.m#L4612) |
| `Dinv`, `MakeDSystem` | C1 / P2 | Differential inverse/system construction over exact kinematics with singular-locus guards. | Missing. | [`:263`](../../vendor/LiteRed2/Source/LiteRed2026.m#L263), [`:4670-4739`](../../vendor/LiteRed2/Source/LiteRed2026.m#L4670) |
| `GenerateFPIBP`, `jsFPIBP`, `FPIBP` | C1 / P2 | Feynman-parametric/syzygy IBP generation using Symbolica-native polynomial algorithms. | Missing. | [`:285-286`](../../vendor/LiteRed2/Source/LiteRed2026.m#L285), [`:1834-1924`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1834) |
| `ToDShifts`, `FromDShifts`, `NumeratorsToDShifts`, `FindSymmetriesDen`, `UniqueSectorsDen`, `MappedSectorsDen`, `SolvejSectorDen`, `SolvejSectorD`, `jRulesDen`, `jSymmetriesDen`, `IBPReduceDen`, `NumDepth` | C1+S / P2 | Complete denominator-only representation, shift conversion, numerator conversion, symmetry, solving, rule application, and depth state. | Missing as a complete alternative interface. | [`:289`](../../vendor/LiteRed2/Source/LiteRed2026.m#L289), [`:4741-5518`](../../vendor/LiteRed2/Source/LiteRed2026.m#L4741) |

Before declaring RustRed's stated mathematical capability goal complete, each
adopted C0/C1 capability needs linked RustRed evidence, a mathematical/oracle
comparison where applicable, tests for success and typed failure/resource
exhaustion, and an explicit status.  Inventory rows classified S, A, X, or H
create no name/API compatibility requirement unless RustRed independently
adopts the underlying behavior.

## Symbolica API surface that still needs to enter the durable reference

The current
[`symbolica_rust_api_for_litered.md:1-5`](symbolica_rust_api_for_litered.md#L1)
is a useful implementation-oriented boundary, not an exhaustive inventory of
the Rust API.  It also marks multiple items as compile/behavior probes rather
than compiled claims at
[`symbolica_rust_api_for_litered.md:518-531`](symbolica_rust_api_for_litered.md#L518).

### Public prelude inventory and disposition

Symbolica's public prelude is the minimum checklist, not the whole crate.  It
reexports the following categories at
[`symbolica/src/lib.rs:90-175`](../../vendor/symbolica/src/lib.rs#L90):

| Prelude/API category | RustRed relevance | Audit action still required |
|---|---|---|
| license/init and construction macros (`LicenseManager`, `initialize`, `parse`, `try_parse`, `symbol`, `function`, namespaces/tags) | Required boundary | Keep deterministic licensed startup and symbol registration; compile-probe exact imports/features. |
| atoms, views, builders, callback metadata, conversion/errors | Required boundary | Existing report is substantial; add an API-index manifest and error/panic tests for every used conversion. |
| coefficient traits and exact domains (`Z`, `Q`, finite fields, algebraic, float, dual, rational-polynomial) | Required exact core plus optional validation domains | Finish derivative/PF details below; classify algebraic/float/dual facilities as validation/acceleration unless a LiteRed feature requires them. |
| evaluator/JIT/export types | Optional validation/performance, not proof | Add the ownership/error policy below; never use numerical agreement or compiled code as proof of a recurrence. |
| patterns/replacements | Required Atom boundary | Add `Pattern::Transformer` and transformer RHS composition; keep typed integral matching authoritative. |
| numerical integration grids/RNG | Oracle/benchmark only | Catalog and keep outside the exact reduction core. |
| parser/printer | Required I/O boundary | Retain checked parser and canonical output policies; never persist implicit symbol ids. |
| multivariate/univariate polynomials, series, factor/GCD/Groebner | Required or C1-dependent | Compile-probe chosen type/feature combinations and wrap unbounded algorithms in RustRed budgets. |
| solve/state/streaming | Required building blocks | Existing report covers much of this; close artifact feature/probe gaps. |
| tensors/matrices | Required building blocks | Keep Lorentz physics in RustRed; Symbolica tensor canonicalization is an optional canonicalizer. |
| transcendental functions | Evaluation/C1 only | Needed for ancillary analytic/numerical output, not parametric IBP discovery. |
| `Transformer` | Useful Atom rule composition | Add the concrete contract below and tests before production use. |

The crate also reexports `graphica` and `numerica` at
[`symbolica/src/lib.rs:177-180`](../../vendor/symbolica/src/lib.rs#L177).
Those trees require separate inventory if RustRed adopts their graph or
numerical facilities.  Their presence in the crate does not authorize an
uncatalogued dependency in the exact core.

### `Transformer`: concrete contract and safe RustRed use

`Transformer` is an owned, `Clone` enum for expression-to-expression chains.
Its variants include conditional branches, expansion, differentiation,
series, collection, one or multiple replacements, custom maps, per-term maps,
partition/sort/deduplication/permutations, repeat, and diagnostics at
[`transformer.rs:216-282`](../../vendor/symbolica/src/transformer.rs#L216).
`Pattern` can itself contain a transformer chain at
[`id.rs:45-64`](../../vendor/symbolica/src/id.rs#L45), and a
`Replacement` owns a `Pattern`, a `ReplaceWith<'static>`, conditions, and
match settings at [`id.rs:240-259`](../../vendor/symbolica/src/id.rs#L240).

Ownership and execution rules:

- `Map` is a cloneable `Send + Sync` closure receiving a borrowed `AtomView`, a
  borrowed `TransformerState`, and an output `&mut Atom`; it returns
  `Result<(), TransformerError>`
  ([`transformer.rs:28-40`](../../vendor/symbolica/src/transformer.rs#L28)).
- `TransformerState` owns an optional `Arc<Mutex<dyn Write + Send>>` statistics
  sink ([`transformer.rs:52-56`](../../vendor/symbolica/src/transformer.rs#L52)).
- `execute` borrows the input and returns an owned `Atom`;
  `execute_with_ws`/`execute_chain` reuse a caller workspace/output and return
  `ControlFlow<()>` inside `Result<_, TransformerError>`
  ([`transformer.rs:511-569`](../../vendor/symbolica/src/transformer.rs#L511)).
- `TransformerError` currently contains `ValueError(String)` and `Interrupt`
  and derives `Clone, Debug`; this source does not implement `Display` or
  `std::error::Error` for it
  ([`transformer.rs:209-214`](../../vendor/symbolica/src/transformer.rs#L209)).
- parallel and single-core `MapTerms` call nested chains with `unwrap`, so an
  inner fallible map can panic instead of returning `TransformerError`
  ([`transformer.rs:617-637`](../../vendor/symbolica/src/transformer.rs#L617)).
  `Repeat` has no iteration budget and stops only on `BreakChain` or structural
  equality ([`transformer.rs:1019-1029`](../../vendor/symbolica/src/transformer.rs#L1019)).

RustRed may use bounded transformer chains for simultaneous Atom-level syntax
normalization, spectator-preserving outer rewrites, and diagnostic canonical
forms.  It must not encode parametric recurrence guards, termination, or
family-dependent rule authority in an opaque chain.  Production wrappers must
forbid unbounded `Repeat`, avoid fallible `MapTerms` until its panic behavior is
contained, catch and translate errors at the boundary, and retain the typed
rewrite/provenance transcript.  Upstream unit examples cover derivative,
series, replacements, custom maps, repetition, linearization, and cyclic
canonicalization at
[`transformer.rs:1084-1252`](../../vendor/symbolica/src/transformer.rs#L1084),
but not the required resource or nested-error contracts.

### Function maps, evaluators, and native/external functions

`FunctionMap` is an owned `Clone + Debug` map from `(Symbol, Vec<Atom>)` tags
to owned expression bodies, with consistent tag arity enforced through
`Result<(), EvaluationError>`
([`function_map.rs:28-173`](../../vendor/symbolica/src/evaluate/function_map.rs#L28)).
`EvaluatorBuilder<'a>` borrows one or more source `AtomView<'a>` values, owns
its parameter `Atom`s, function map, and optimization settings, and consumes
itself to return an owned
`ExpressionEvaluator<Complex<Rational>>`
([`function_map.rs:175-227`](../../vendor/symbolica/src/evaluate/function_map.rs#L175),
[`function_map.rs:230-353`](../../vendor/symbolica/src/evaluate/function_map.rs#L230)).
Builder errors include invalid/undefined variables or functions, arity/tag
mismatches, unsupported coefficients, and construction failures
([`tree.rs:5-65`](../../vendor/symbolica/src/evaluate/tree.rs#L5)).

`EvaluationFn<A,T>` owns a boxed evaluation closure over argument, constant,
function, and expression-cache maps at
[`function_map.rs:5-25`](../../vendor/symbolica/src/evaluate/function_map.rs#L5).
The separately registered `ExternalFunction<T>` is a cloneable `Send + Sync`
`Fn(&[T]) -> T` trait object
([`external.rs:229-233`](../../vendor/symbolica/src/evaluate/external.rs#L229)).
External-function containers do **not** serialize closures; decode reparses
tags and resolves implementations from the process-global symbol metadata
([`external.rs:13-110`](../../vendor/symbolica/src/evaluate/external.rs#L13)).
If no implementation is resolved, ordinary evaluation can panic
([`evaluator.rs:292-299`](../../vendor/symbolica/src/evaluate/evaluator.rs#L292)).

`ExpressionEvaluator<T>` owns mutable stack/cache state and therefore requires
`&mut self` for evaluation
([`evaluator.rs:6-16`](../../vendor/symbolica/src/evaluate/evaluator.rs#L6)).
Use `try_evaluate`, which checks parameter and output cardinality and returns
`EvaluationError`, rather than `evaluate`/`evaluate_single`, which unwrap or
panic ([`evaluator.rs:143-173`](../../vendor/symbolica/src/evaluate/evaluator.rs#L143),
[`evaluator.rs:254-269`](../../vendor/symbolica/src/evaluate/evaluator.rs#L254)).

RustRed use cases are finite-field/numerical screening of candidate pivots,
randomized validation after exact replay, and optional coefficient-evaluation
benchmarks.  These evaluators must not decide that a symbolic coefficient is
identically zero, erase exceptional loci, or become the persisted form of an
IBP rule.  Native callback symbols remain process-global and must be
registered before parallel work, as already required by the main Symbolica
audit.

### JIT, export, and compiled evaluators

The public prelude exposes evaluator/JIT/export types at
[`symbolica/src/lib.rs:125-131`](../../vendor/symbolica/src/lib.rs#L125).
The concrete boundary is:

- `JITCompilationSettings` owns direct-translation, optimization-level, and
  option-map settings; `jit_compile` returns `Result<JITCompiledEvaluator<T>,
  String>` and rejects unresolved external functions
  ([`backend.rs:320-375`](../../vendor/symbolica/src/evaluate/backend.rs#L320),
  [`backend.rs:377-431`](../../vendor/symbolica/src/evaluate/backend.rs#L377)).
- `ExportSettings` controls headers, inline assembly, and a custom header;
  `export_cpp` writes a source file and returns `io::Result<ExportedCode<F>>`
  ([`export.rs:141-190`](../../vendor/symbolica/src/evaluate/export.rs#L141),
  [`export.rs:192-247`](../../vendor/symbolica/src/evaluate/export.rs#L192)).
- `ExportedCode::compile` launches the configured compiler process and returns
  `io::Result<CompiledCode<T>>`
  ([`backend.rs:2786-2877`](../../vendor/symbolica/src/evaluate/backend.rs#L2786),
  [`backend.rs:2913-2985`](../../vendor/symbolica/src/evaluate/backend.rs#L2913)).
  Default options enable fast/unsafe math and native architecture at
  [`backend.rs:2804-2814`](../../vendor/symbolica/src/evaluate/backend.rs#L2804).
- `EvaluatorLoader` loads a shared library and returns `Result<_, String>`;
  `BatchEvaluator` has an explicit result for batch failures
  ([`backend.rs:1323-1369`](../../vendor/symbolica/src/evaluate/backend.rs#L1323)).

In-process Symbolica JIT can be an optional sampling/performance accelerator
after a feature and determinism probe.  C++/CUDA export and compiler launching
are not part of the pure Rust reference reduction path, must never be required
for tests, and must not use unsafe/fast math for validation of exact identities.
Generated filenames, compiler availability, architecture, and shared-library
loading also make these unsuitable as authenticated rule artifacts.

### Rational-polynomial derivative, partial fractions, and integration

`RationalPolynomial<R,E>` owns public numerator and denominator
`MultivariatePolynomial`s, whose variable maps are shared through
`Arc<Vec<PolyVariable>>`; `RationalPolynomialField<R,E>` owns/clones its
coefficient ring
([`rational_polynomial.rs:38-58`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L38),
[`rational_polynomial.rs:90-122`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L90),
[`rational_polynomial.rs:187-200`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L187)).
RustRed must continue to validate that numerator and denominator use the same
authenticated variable map before and after every call.

The previously omitted APIs are:

| API | Ownership/result/error semantics | RustRed use and restriction | Source |
|---|---|---|---|
| `RationalPolynomial::derivative(&self, var)` and field `Derivable::derivative` | Borrow receiver, return an owned normalized rational polynomial; absent variables yield field zero through the trait.  No `Result` or explicit index-bound error. | Useful for Feynman/differential systems and exact derivative verification.  Validate `var < nvars` before the direct method and retain denominator guards. | [`:1122-1173`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L1122) |
| `apart(&self, var)` | Returns owned `Vec<Self>` reconstructed from the factored-denominator form. | Useful for coefficient presentation, not LiteRed denominator-set `PFReduce` by itself.  Verify exact recombination and variable map. | [`:1175-1188`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L1175) |
| `apart_factored_denominators(&self, var)` | Returns owned `(numerator, denominator, exponent)` triples and factors internally; indexes `fs[0]`, later has `assert!(!fs.is_empty())`, returns no `Result`, and exposes no resource budget. | Candidate primitive for guarded univariate PF.  Wrap input validation/catch boundary, budget factorization externally where possible, and always recombine exactly. | [`:1190-1277`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L1190) |
| `apart_multivariate(&self)` | Returns owned parts after factorization and a Groebner reduction; calls `rearrange_with_growth(...).unwrap()` and exposes no work limit. | Relevant to overcomplete/PF research, but unsafe as an unbounded production endpoint.  Use only behind a process/budget boundary or implement a budgeted RustRed layer with mandatory exact recombination. | [`:1280-1377`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L1280) |
| `integrate(&self, var)` | Returns owned `RationalIntegral` containing rational parts and `LogarithmicIntegralTerm`s; algebraic root sums are encoded by a temporary-variable defining polynomial.  No `Result` or resource budget. | C1 analytic/differential utility only, not needed to derive IBPs.  Validate variable index, preserve temporary-variable identity, differentiate/recombine the result, and never silently flatten root sums. | [`:1380-1663`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L1380) |

The safe field API already offers `try_inv` and `try_div` for zero divisors at
[`rational_polynomial.rs:910-919`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L910);
RustRed should prefer those over unchecked division where the divisor is
caller- or solver-controlled.

### Missing upstream-test inventory and required RustRed probes

The earlier report cites pattern and import/export tests but did not inventory
these relevant suites:

- `vendor/symbolica/tests/evaluation.rs`: finite-field-ring evaluation,
  merging evaluators with external functions, and float/error-propagating
  transcendental evaluation at
  [`evaluation.rs:16-171`](../../vendor/symbolica/tests/evaluation.rs#L16);
  large real/complex evaluation and GCC/CUDA export/load at
  [`evaluation.rs:1214-1363`](../../vendor/symbolica/tests/evaluation.rs#L1214).
  CUDA tests self-skip unless `CUDA_TEST` is set; GCC/CUDA tests create files
  and invoke external compilers, so they are not RustRed's ordinary pure-Rust
  test baseline.
- `vendor/symbolica/tests/rational_polynomial.rs`: nine tests covering
  conversion with non-polynomial exponents, exponent splitting/common bases,
  default inputs, large GCDs, and factorized rational polynomials at
  [`rational_polynomial.rs:12-281`](../../vendor/symbolica/tests/rational_polynomial.rs#L12).
  These do not directly cover derivative, multivariate apart, or RustRed's
  authenticated-map/guard requirements.
- source unit tests cover `integrate` and factored-denominator recombination at
  [`domains/rational_polynomial.rs:1686-1985`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L1686),
  but no dedicated `apart_multivariate` test was found in that source.
- transformer source tests exercise representative chains at
  [`transformer.rs:1084-1252`](../../vendor/symbolica/src/transformer.rs#L1084),
  not nested-error propagation, cycle/resource limits, or deterministic
  parallel output.

Required RustRed probes before adopting these APIs are therefore:

1. bounded transformer chain and typed error translation; a deliberately
   failing `Map` under `MapTerms` must not take down the test process;
2. `FunctionMap` tag/arity errors, builder lifetime independence after
   `build`, external-function re-resolution after serialization, and checked
   `try_evaluate` cardinality failures;
3. licensed parallel evaluation with independent mutable evaluator instances;
4. JIT determinism/error behavior in a disposable optional test, with no
   external compiler in the required suite;
5. rational derivative quotient-rule replay with fixed variable maps;
6. univariate and factored `apart` exact recombination, constant denominator,
   repeated factors, and invalid-variable handling;
7. multivariate apart in a disposable bounded test, including panic capture,
   exact recombination, and resource-limit policy; and
8. rational integration checked by differentiating all returned rational/log
   structures, including root-sum temporary variables.

## Vakint completeness and compatibility boundaries

The existing
[`vakint_alphaloop_tensor_ibp_audit.md`](vakint_alphaloop_tensor_ibp_audit.md)
is strong on production flow, tensor conventions, and alphaLoop recurrence
tables.  Three corrections/boundaries remain necessary.

### Two omitted tests

The claim that all test functions were inventoried at
[`vakint_alphaloop_tensor_ibp_audit.md:522-525`](vakint_alphaloop_tensor_ibp_audit.md#L522)
is not exact.  `integral_alphaloop_vs_matad_tests.rs` also contains:

- `test_eval_matad_masters`, which evaluates individual MATAD master constants
  with and without direct substitution at
  [`:352-547`](../../vendor/gammaloop/crates/vakint/tests/integral_alphaloop_vs_matad_tests.rs#L352); and
- `test_eval_matad_one_master_combination`, which checks a nontrivial master
  combination through both paths at
  [`:549-600`](../../vendor/gammaloop/crates/vakint/tests/integral_alphaloop_vs_matad_tests.rs#L549).

They are evaluation/master-substitution tests, not new IBP-derivation
algorithms.  RustRed should classify them as later normalization/evaluation
oracles; they do not belong in the first unsubstituted-master reduction gate.

### Dot-power semantics

Vakint rewrites `dot(a,b)^c` to an internal power, turns negative powers into
the reciprocal of a positive `dot_pow`, and expands remaining powers by fresh
dummy indices at
[`vakint/src/lib.rs:4492-4595`](../../vendor/gammaloop/crates/vakint/src/lib.rs#L4492).
RustRed's current matrix accepts only bounded nonnegative integer powers of
tensor-valued dot factors and rejects negative, noninteger, or symbolic ones
at
[`vakint_tensor_atom_validation_matrix.md:163-179`](vakint_tensor_atom_validation_matrix.md#L163).

The compatibility contract must distinguish:

- nonnegative integer powers that can become explicit tensor factors;
- negative powers of a **scalar** dot product, which may remain an exact
  rational scalar weight after classifying both operands;
- negative powers that would require an inverse tensor object, which are typed
  unsupported inputs; and
- symbolic/noninteger powers, which remain opaque spectator weights only when
  they contain no reducible loop-tensor structure.

This decision needs tests and must be made before claiming Vakint input parity.

### Tensor/topology/application coverage

Vakint simultaneously applies numerator routing substitutions at
[`vakint/src/lib.rs:2258-2364`](../../vendor/gammaloop/crates/vakint/src/lib.rs#L2258),
canonicalizes the topology at
[`vakint/src/lib.rs:2366-2384`](../../vendor/gammaloop/crates/vakint/src/lib.rs#L2366),
identifies loop/external vectors at
[`vakint/src/lib.rs:2386-2424`](../../vendor/gammaloop/crates/vakint/src/lib.rs#L2386),
and then enters its FORM tensor bridge at
[`vakint/src/lib.rs:2426-2568`](../../vendor/gammaloop/crates/vakint/src/lib.rs#L2426).
RustRed must reproduce those semantics with Symbolica/Rust, never by invoking
the bridge.

The readable tensor oracle preloads ranks two through eight and can load rank
ten tables at
[`tensorreduce.frm:32-199`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm#L32).
Its actual contraction/projection/application procedure is
[`tensorreduce.frm:211-392`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm#L211).
RustRed's required endpoint is a generated, orbit-reduced, exact projector
with convention-mapped structural comparisons at ranks 2, 4, 6, 8, and 10;
the FORM tables are oracles, not production data.

Likewise, alphaLoop's readable rules cover one loop at
[`integrateduv.frm:17-29`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm#L17),
two loops at [`:31-153`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm#L31),
and three loops at [`:155-1127`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm#L155),
with descending 3L/2L/1L dispatch and an unsubstituted-graph failure at
[`integrateduv.frm:1129-1139`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm#L1129).
RustRed must derive corresponding parametric rules from generated IBPs,
authenticate them, and only then compare concrete specializations.  Copying
these recurrences into production would fail the governing scope.

## Validation evidence policy

Static inventory shows a large RustRed test surface and concrete one-, two-,
and three-loop Vakint oracle fixtures, but this audit did not execute it.  The
existence of those files establishes neither a clean parallel suite nor full
capability coverage.  A completion claim must link a reproducible parallel run with
the configured Symbolica license and distinguish:

1. exact generic algebra/unit properties;
2. symbolic replay of every emitted parametric relation/rule;
3. finite concrete specializations as independent checks;
4. Vakint structural comparisons before master substitution;
5. later normalization/master-evaluation comparisons; and
6. explicit resource-limited or unsupported outcomes.

At minimum the closure matrix must include all Vakint Rust test functions,
the decorated arbitrary-spectator fixture, dot-notation boundaries, topology
matching/pinches/LMB mappings, tensor ranks through ten, and the complete
one-to-three-loop alphaLoop rule set.  Four- and five-loop massive-vacuum
validation begins only after the generic lower-loop derivation and application
path passes without loop-specific production recurrences.

## Ordered implementation closure

1. Finish and publicly integrate the generic affine generated-rule fixed
   point: conditions, boundary pullbacks, strict descent, residual partition,
   subsector feedback, persistent row state, exact replay, and stable artifacts.
2. Complete `ToAB`/tilde semantics and make ordering/guard/provenance objects
   the single authority used by discovery, application, and persistence.
3. Replace the I2L adapter path with generic Vakint topology/routing input;
   preserve opaque spectators and apply only generated scalar rules.
4. Add overcomplete denominator sets and PF reduction, then full zero-sector,
   internal/external/cross-basis symmetry, demand selection, and master policy.
5. Replace dense tensor pairing inversion with orbit-reduced cached projectors,
   add external-vector covariants, and validate ranks 2 through 10.
6. Close the Symbolica probes and enable only the serialization features proven
   necessary for authenticated artifacts; keep JIT/export optional and outside
   the exact proof path.
7. Implement adopted C1 capabilities informed by LiteRed2: FPIBP/syzygies,
   remaining Feynman/Gram/factor
   utilities, dimensional recurrences, differential systems, denominator-only
   APIs, and graph adapters/visualization.
8. Run the complete suite in parallel with the configured Symbolica license,
   record exact commands/results, and advance the loop-validation ladder only
   after the preceding generic gate is clean.

This sequence preserves the central invariant: concrete topologies and
Vakint/alphaLoop formulas validate generated behavior; they never define the
RustRed production algorithm.
