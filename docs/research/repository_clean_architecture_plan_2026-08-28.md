# RustRed clean-repository architecture plan

**Status:** authoritative Phase-0 execution plan, subordinate only to
[`GOAL.md`](../../GOAL.md). This document supersedes the package-preservation
and gradual-move policy in
`repository_reorganization_directive_2026-08-27.md`. It is deliberately a
reset plan, not a promise to preserve the present Rust API, schemas, fixtures,
tests, or file history.

## Decision

RustRed will become a **virtual Cargo workspace**. The package whose Cargo
name is `rustred` will live at `crates/rustred-core`; the repository root will
have no package and no `src/` or `tests/` tree. The live workspace contains
only:

1. `rustred` at `crates/rustred-core`: generic mathematical/domain services;
2. `rustred-app`: typed application composition and the `rustred` CLI; and
3. `rustred-python`: the thin PyO3 adapter whose user-facing import is
   `import rustred`.

The recently created `rustred-legacy-oracles` package will be deleted in its
entirety, together with the core bridge and feature that exist only for it.
Git already preserves that experiment. Maintaining it would spend effort on
authored loop-specific recurrences and old tests precisely when Vakint's real
one- through four-loop end-to-end results should become the lower-loop north
star.

No current root module earns retention merely because it compiles or has a
test. The reset retains a module only when it has a real caller in the generic
family/IBP/campaign/reduction application spine, implements a still-required
RustRed domain responsibility, and—after a public-API audit—does not duplicate
a usable Symbolica facility. Everything else is deleted without a
compatibility shim. The new public facade is written from the actual use
cases; the 750-line root re-export facade is not moved.

A completed production-liveness/SCC audit found that `src/solver/**`, the
exact-session/closure/publication/re-entry machinery, and its dependent
generated-affine/cylindrical/residual provider stack form one private dead
prototype island: there is no app, CLI, Python, or Vakint production caller.
The entire island is deleted during this reset, not relocated. Phase 0 ends
without a production foundry or closed-artifact publisher. Those services are
built cleanly after the reset around retained generic algebra, family,
topology-neutral IBP/LI, sector, campaign, tensor, and reduction primitives.

This cleanup is a stop-the-line prerequisite. Fresh exact-closure foundry
work, six-loop optimization, and Vakint implementation begin only after the new
workspace, ownership DAG, minimal tests, documentation, warning gate, and
rustdoc gate are green.

## Measured baseline

The inventory below is from tracked files at commit `dda284a`.

| Surface | Measured size | Architectural reading |
|---|---:|---|
| repository | 489 tracked files | history has accumulated as product surface |
| root `src/` | 166 Rust files, 402,790 lines | oversized root package |
| files directly in root `src/` | 127 | flat namespace rather than owned domains |
| flat `generated_*` / `residual_*` / `parametric_*` files | 50 / 6 / 15 = 71 | chronological or state adjectives obscure ownership |
| `src/solver/` | 29 files, 73,216 lines | audited private prototype island with no production caller; delete wholesale |
| root `tests/` | 103 files, 42,777 lines | mostly old public-surface and self-oracle maintenance burden |
| `rustred-legacy-oracles` | 84 tracked files, including 82 Rust files and about 61,400 lines | wholly deletable authored/finite historical lane |
| `rustred-app` | 22 tracked files, about 5,750 lines | live frontend boundary |
| `rustred-python` | 9 tracked files, about 1,660 lines | live frontend boundary; `_rustred` is implementation-only |
| `docs/research` | 78 Markdown files, about 40,900 lines | research log, not a navigable design manual |
| standalone probes | 3 `tools/*.rs` files, about 3,960 lines | loop-authored finite-field discovery oracles |
| test wrappers | 2 `scripts/*` files, 40 lines | one deprecated compatibility entry point and one local runner |
| tracked gitlinks | Symbolica, LiteRed2, GammaLoop | only Symbolica is a build dependency |

A licensed `cargo check -p rustred --lib --locked` succeeds today but emits
approximately 2,710 warnings, overwhelmingly dead-code warnings. Building the
library tests emits approximately 236 warnings. Strict rustdoc is not green
because of pre-existing private/redundant intra-doc links. These are evidence
that “compiled” is not a sufficient liveness criterion. Zero-warning check and
strict rustdoc are Phase-0 completion gates; they are not regressions caused by
the documentation-only planning commit.

## Why the flat names are wrong

`generated`, `residual`, and `parametric` are properties of data at particular
points in a computation, not stable owners:

- almost every foundry relation is parametric, so `parametric_` does not
  distinguish a package boundary;
- generated rows, guards, conditions, and rules belong respectively to
  identity generation, exact solving, closure, or artifact publication;
- a residual is a solver state, not a subsystem; and
- names such as V1/V2, “initial”, “next”, “persistent”, or “when bad” often
  encode implementation chronology instead of responsibility.

Retained types will therefore be placed under mathematical owners and renamed
for their role. Old root paths and re-exports disappear. We will not create a
`generated/`, `residual/`, `parametric/`, `legacy/`, or `misc/` dumping-ground
directory.

## Phase-0 tracked tree

The intended repository skeleton at the reset gate is:

```text
/
├── Cargo.toml                 # [workspace] only; no [package]
├── Cargo.lock
├── pyproject.toml             # repository-level maturin authority
├── .gitignore                 # keeps local reference trees untracked
├── .gitmodules                # vendor/symbolica only
├── GOAL.md
├── README.md
├── LICENSE
├── flake.nix
├── flake.lock
├── crates/
│   ├── rustred-core/
│   │   ├── Cargo.toml         # package.name = "rustred"
│   │   ├── src/
│   │   │   ├── lib.rs         # small intentional public facade
│   │   │   ├── algebra/
│   │   │   ├── family/
│   │   │   ├── input/
│   │   │   ├── identity/
│   │   │   ├── sector/
│   │   │   ├── campaign/
│   │   │   ├── tensor/
│   │   │   └── reduction/
│   │   ├── tests/             # few black-box contract tests only
│   │   └── benches/           # measured lanes only, when introduced
│   ├── rustred-app/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   ├── tests/
│   │   └── examples/          # CLI input fixtures owned by this package
│   └── rustred-python/
│       ├── Cargo.toml
│       ├── src/
│       ├── python/rustred/     # public package; native _rustred is private
│       └── tests/
├── docs/
│   ├── architecture.md
│   ├── algebra.md
│   ├── foundry.md
│   ├── interfaces.md
│   ├── validation.md
│   └── references/
│       └── litered2.md
└── vendor/
    └── symbolica              # sole retained tracked gitlink
```

`FOR_REFERENCE_ONLY_DO_NOT_PUSH/` remains an ignored local reference area and
is never part of this tree. In particular, the local LiteRed2, GammaLoop,
Vakint, and FORM material is not copied into RustRed history.

Post-reset work adds fresh `foundry/` and `artifact/` owners only when their
new contracts and first production callers exist. No empty shell or relocated
prototype is created merely to prefigure that future tree.

## Domain dependency DAG

Arrows mean “depends on”. There may be narrower leaf modules inside a box, but
there may be no reverse edge and no public compatibility route around this
direction.

```text
rustred-python ──> rustred-app ──> input, identity, sector, campaign,
                                     tensor, reduction
Vakint RustRed mode ───────────────────────────────────────> input, tensor,
                                                             reduction

reduction ──> identity, sector, tensor, family, algebra
input ──────> tensor, family, algebra
tensor ─────> family, algebra
identity ───> family, algebra
sector ─────> family, algebra
family ─────> algebra
algebra ────> Symbolica public Rust API
```

More precisely:

- `algebra` wraps checked Symbolica coefficient/polynomial/matrix operations;
  it knows no integral family, sector, rule, or campaign. Its coefficient API
  has one base field and one index-extended field with role names rather than
  parallel historical `coefficient` and `parametric_coefficient` stacks.
  Stored exponents use Symbolica's native `u16`; prospective widening uses
  checked `u32`/`u64`, never a fictitious `u128` exponent domain. No self-only
  sparse reducer or wrapper survives without a production caller.
- `family` owns authenticated kinematics, coordinates, denominators, shifts,
  target keys, and specialization.
- `input` parses, lowers, and normalizes external/core project descriptions
  through tensor/family-owned types. It contains no solving or frontend
  transport policy; tensor never imports a parser.
- `identity` generates topology-neutral IBP/LI rows from a family.
- `sector` owns sectors, graph/routing symmetries, zero/factorization proofs,
  affine loci, and transport witnesses. It calls Symbolica's public
  `symbolica::graph` APIs directly: `Graph::canonize()` and the resulting
  `CanonicalForm` supply canonical labels and vertex-automorphism generators
  for an existing topology; `GenerationSettings` with `Graph::generate` is
  reserved for optional finite topology-domain enumeration. A physics-colored
  subdivision/flag graph represents lines, line ports, and topology vertices
  explicitly, so parallel-line exchange and orientation reversal become
  vertex automorphisms. RustRed interprets those permutations as routing
  candidates and certifies them with exact affine/momentum-map replay.
  GammaLoop/feyngen is reference-only usage evidence for coloring, bucketing,
  and orchestration, never a dependency or graph implementation authority.
- `campaign` owns calibrated resource metadata, preflight width selection, and
  bounded ordered execution. The live roots-only family/sector/job interning
  is application composition rather than a replayable core plan; the dead
  work-key wave admission/controller layer is not retained. Campaign never
  imports foundry; a future freshly built foundry may compose it.
- `tensor` owns a lean typed tensor IR, Symbolica-Atom decoding/rendering,
  pairing/orbit combinatorics, vacuum projection, and scalar lowering, while
  delegating polynomial and matrix CAS work to Symbolica through algebra. Its
  stable children are `model`, `atom`, `lowering`, and
  `projector/{pairing,contraction,orbit,vacuum}`; opaque user weights remain
  separate from exact family coefficients.
- `reduction` retains generic guarded-rule application, tensor/scalar
  reduction, and typed master-substitution primitives with real callers.
  Callers lower input before invoking it. It does not own a foundry, solver,
  or algebra engine. `rustred-app` is the composition layer; the core
  deliberately has no confusing `application/` directory.
- There is no core `runtime` wrapper. RustRed calls Symbolica's public Rust API
  directly; application-only rendering and memory census policy stays in
  `rustred-app`. Add a runtime boundary later only if a real global
  initialization, licensing, or execution-policy caller requires one.

After Phase 0, a fresh `foundry` may depend on identity, sector, campaign,
family, and algebra, and emit values owned by a new stable `artifact`
domain. Reduction may then consume those artifacts. Artifact models never
depend on foundry internals. This future direction is a contract to design,
not a reason to preserve the audited prototype's solver/session hierarchy.

## Current-to-final cluster classification

The mapping below covers every architectural source cluster; it is not a claim
that a prose glob is a complete file manifest. Before R3 changes code, generate
a tracked-path liveness ledger from `git ls-files` in which every source path is
classified as retain/move, split, or delete. An unclassified path stops that
tranche. “Move if live” is intentionally conditional: the reset first proves a
symbol is needed; dead or duplicate variants in the cluster are deleted rather
than carried to the destination.

The ledger is regenerated and challenged after every R2-R4 milestone, not
written once and trusted. A `split` classification expires: before the facade
gate, every surviving symbol must have a live caller, a fresh sentinel, and a
role-named final owner. Long state/chronology paths such as
`generated_residual_affine_group_effective_coverage` receive no presumption of
value from their size or recent history; if their unique stable semantics
cannot be isolated, the whole cluster is deleted. The completed solver-island
audit resolves the generated-affine/cylindrical/residual and publication/
re-entry clusters to deletion, so they receive no destination module. A flat
set of repeated long prefixes is not accepted, but neither is a prefix-named
dumping-ground module.

| Current files | Final owner/action |
|---|---|
| root `Cargo.toml` package sections | delete; retain only virtual workspace, shared package metadata, patches, and profiles |
| root `build.rs` | delete; the application producer calls Symbolica's public `LicenseManager::get_version()` directly, so reported version follows the dependency Cargo actually resolved |
| root `src/lib.rs` | delete and write a small `crates/rustred-core/src/lib.rs`; do not preserve wholesale re-exports or schema compatibility |
| `src/legacy_oracle_support.rs` | delete with `legacy-oracle-support` feature and every reference to it |
| `src/coefficient.rs`, `exact.rs`, `exact_identity.rs`, `parametric_coefficient.rs` and its subtree, `symbolica_coefficient_matrix.rs` | move live provenance-neutral checked primitives to `algebra/` and identity-owned relation-condition handling to `identity/`; delete bespoke operations available through Symbolica and automatic recompilation of an already authenticated denominator |
| `src/symbolica_affine_denominator.rs` | split parsing and family-coordinate lowering under `input::affine`; move only genuinely family-neutral coefficient primitives to `algebra/`, and audit handwritten exponent projection against Symbolica rather than relabeling the complete compiler as algebra |
| `src/generic_family.rs`, `automatic_isps.rs`, `feynman_polynomials.rs`, `sectors.rs`, `guards.rs` | move live model/normalization code to `family/`, sector responsibilities to `sector/`, and identity-row provenance to `identity/` as indicated by the DAG; trusted family and ISP constructors authenticate ingress and check their local shape, algebra, and rank invariants once, while exact replay remains an explicit proof/tamper audit; the dead handwritten `base_specialization.rs` prototype has been deleted and remaining handwritten polynomial operations must be checked against Symbolica before Phase 0 closes |
| `src/shift_operators.rs` | delete wholesale; it is a closed, uncalled intermediate with only self-tests and is not a reduction implementation |
| `src/symbolica_integral_input.rs` | move core parsing/normalization to `input/`; authenticate each ingress once, keep canonical round-trip as a focused sentinel rather than automatic constructor replay, and leave transport/CLI decoding in `rustred-app` |
| `src/parametric_ibp.rs`, `parametric_relation.rs`, and the topology-neutral part of `generated_symbolic_row_span.rs` | move live raw IBP/LI row construction and stable row provenance to `identity/`; one prepared source-batch type covers ordinary and LI-only layouts, seals ordered results once by semantic scope/layout/ordinal, and supplies one completed source token to LI without replay; remove “generated” naming |
| `src/parametric_sector_normalized_source.rs`, `generated_cylindrical_family_source_set.rs`, `generated_cylindrical_row_system.rs`, `generated_affine_residual_source_authority.rs`, and high-level portions of `generated_symbolic_row_span.rs` | retain only raw topology-neutral identity values already used outside the prototype under `identity/`; delete exceptional scheduling, coverage, source-authority, and provider orchestration with the solver island |
| `src/coordinate_equality_loci.rs`, `symbolic_sector_cases.rs`, `symmetry.rs`, `symmetry_discovery.rs`, `symbolic_symmetry_transport.rs`, `zero_sectors.rs`, `zero_sector_provider.rs`, `product_locus_boolean_cover.rs`, `residual_affine_{atom_rows,integer_system,integer_lattice_kernel}.rs` | retain only independently live locus/transport/zero/factorization responsibilities under `sector/`; the legacy V1 residual-unit map and its public adapters have been deleted, and unsupported integer-normal-form cases remain typed boundaries rather than private CAS |
| `src/family_sector_inventory.rs` | retain only independently used stable sector decisions/evidence under `sector/`; delete family-wide unresolved-work orchestration with the prototype island |
| `src/residual_affine_branch_system.rs`, `residual_affine_branch_guard_composition.rs` | retain only independently used affine-locus evidence primitives under `sector/`; delete solver branch scheduling/state transitions |
| `src/affine_parametric_ordering.rs`, `affine_prepare_point_schedule.rs`, `affine_prepare_points.rs`, `cylindrical_ordering.rs`, `cylindrical_prepare_point_schedule.rs`, `cylindrical_prepare_points.rs`, `canonical_parametric_locus_table.rs`, `affine_locus_bound_relation.rs` | delete the solver-only ordering/prepare/locus prototype stack; any future ordering contract is designed afresh under the new foundry |
| `src/parametric_elimination.rs`, `persistent_parametric_elimination.rs`, `conditional_reelimination.rs`, `direct_bad_formula.rs`, `direct_bad_formula_arbitrary.rs`, `when_bad.rs`, `exact_sparse_elimination.rs` | delete solver-only engines and handwritten algebra; retain only a generic Symbolica-backed row primitive if an outside-island caller proves it belongs in `algebra/` |
| all `src/generated_affine_*.rs` not assigned above | delete with the audited solver island; do not split or rename into a new foundry |
| all `src/generated_cylindrical_*.rs` not assigned above | delete with the audited solver island |
| all `src/generated_residual_*.rs` and same-name child test directories such as `src/generated_residual_affine_group_effective_coverage/**` | delete with the audited solver island, including coverage, queue, publication, and re-entry wrappers |
| `src/generated_family_*.rs`, `generated_sector_*.rs`, `generated_provider_stack.rs`, `generated_when_bad.rs` | delete the prototype fixed-point/provider stack; no Phase-0 artifact-emission replacement |
| `src/coverage_decision_dag.rs`, `adaptive_rules.rs`, `certified_rewrite.rs`, `certified_rule_provider.rs`, `certified_symmetry_provider.rs`, `conditional_rules.rs`, `parametric_rules.rs`, `parametric_sector_coverage.rs`, `parametric_sector_provider.rs`, `master_policy.rs`, `master_product.rs` | retain only independently used reduction-side rule/master values with real callers; delete closure/provider and speculative artifact layers |
| `src/parametric_sector_formula_{ir,residual,affine_terminal}.rs`, `parametric_sector_mtbdd.rs`, `parametric_sector_mtbdd_certificate.rs`, `parametric_sector_one_pass_tests.rs`, `parametric_sector_k21_test_support.rs` | delete the eager/legacy sector and synthetic-test stack; future lazy closure formula types are designed afresh |
| existing `src/solver/**` | delete wholesale as the audited dead prototype island; do not preserve the exact-session transaction, rollback, exceptional publication, or committed-re-entry machinery |
| `src/campaign/{mod,execution,execution_width,resource_profile,resources}.rs` and `crates/rustred-app/src/application/campaign/plan.rs` | retain checked resource/preflight metadata, width selection, and bounded ordered execution under core `campaign/`; delete core `plan.rs` with replay/schema/stats/dependency machinery and keep the sole live roots-only family/sector/job interning directly in the application; `admission.rs`, `work.rs`, the work-key wave policy/planner, move-owned reservation mapper, and width-plan-to-executor bridge are also deleted |
| `src/generic_tensor_family.rs`, `generic_tensor_polynomial.rs`, `generic_tensor_projector.rs`, `tensor.rs`, `symbolica_target_numerator.rs`, `symbolica_tensor_numerator.rs` | delete the complete uncalled and untested prototype SCC rather than moving it. Rebuild `tensor/` fresh from explicit pairing/cycle, covariant-precontraction, affine-lowering, and configurable-Atom contracts, using Symbolica-native CAS and the custom-head plus Vakint vertical sentinels |
| `src/reduction_engine.rs`, `tensor_reduction_engine.rs` | do not preserve as compatibility engines; extract only generic compiled-rule application/tensor/master services into `reduction/`, with Vakint end-to-end tests defining results |
| `src/symbolica_runtime.rs` | delete; direct public Symbolica API calls replaced its trivial wrappers, and no global initialization boundary is currently required |

### Non-source repository surfaces

| Current surface | Classification |
|---|---|
| root `pyproject.toml` | retain as the repository-level maturin authority; keep `module-name = "rustred._rustred"`, the package source under `crates/rustred-python/python`, and the private-extension/public-package split working after workspace virtualization |
| root `.gitignore` | retain; keep `FOR_REFERENCE_ONLY_DO_NOT_PUSH/` ignored |
| root `.gitmodules` | retain after deleting the LiteRed2/GammaLoop entries; it declares only `vendor/symbolica` |
| `crates/rustred-legacy-oracles/**` | delete wholesale before relocating the core; do not repair it for the new path |
| `crates/rustred-app/**` | retain, update its dependency to `../rustred-core`, narrow it to composition/CLI, and freely replace obsolete request/output schemas |
| `crates/rustred-python/**` | retain; `python/rustred` is public and `_rustred` remains a private extension implementation detail, never the user import |
| root `tests/*.rs` | delete in their current form; extract only a small generic contract matrix into the new package instead of porting 103 binaries or preserving old APIs |
| root `examples/**` | delete wholesale, including CLI inputs, the broad tensor example, and the license probe; delete/rebuild app tests that `include_str!` those fixtures rather than repairing paths, then create fresh app-owned examples only for the supported new schema |
| `tools/*.rs` | delete all three loop-authored, pure-std finite-field probes |
| `scripts/test.sh`, `scripts/test-serial.sh` | delete; use documented Cargo/nextest commands or a maintained task runner later, without compatibility wrappers |
| `vendor/LiteRed2`, `vendor/gammaloop` gitlinks and their `.gitmodules` entries | delete; the ignored reference area and separate GammaLoop branch are the proper boundaries |
| `vendor/symbolica` gitlink | retain as the pinned production CAS dependency; `no_gmp` remains forbidden |
| `README.md` | rewrite against the real three-package tree and current capabilities; delete historical achievement log |
| `docs/CLI.md` | replace with stable `docs/interfaces.md` covering CLI plus Python and core application contracts |
| all 78 current `docs/research/*.md` | distill unique current decisions into the six stable documents in the final tree, then delete the research log; this execution plan itself may be deleted once its gates are captured and complete |

The documentation distillation is allowlist-by-subject, not file preservation.
The only source notes that must be consulted before deletion are the current
LiteRed algorithm/scope reports, parametric-IBP design, parallel-foundry plan,
Symbolica API/compliance inventories, and Vakint/GammaLoop integration audits.
Former exact-session/exceptional notes may contribute failure cases but do not
define the fresh foundry architecture. Still-valid content moves into
`references/litered2.md`, `foundry.md`, `algebra.md`, `validation.md`, and
`interfaces.md`; their original dated files then disappear. Loop-authored
reduction notes, old V1/V2/generated pipeline plans, the prior repository
reorganization directive, compatibility directives, and completed milestone
logs are deleted rather than indexed as an archive.

## Fresh test and evidence policy

The reset does not attempt to make every legacy test compile under new paths.
It creates a small matrix from the essential cases:

1. family normalization and automatic ISP completion;
2. topology-neutral ordinary IBP and LI source generation at `L=1,2` plus the
   synthetic structural `L=6, K=21, 36-source` count (not a closure claim);
3. exact Symbolica coefficient/row operations;
4. exact zero/symmetry/routing witnesses, including native graph candidates;
5. deterministic campaign planning and resource preflight for `n_cores = 1,2,4`;
6. generic reduction-rule/master primitives with real callers;
7. app/CLI/Python byte- and error-parity for the supported operations; and
8. black-box artifact load/application tests only after the fresh artifact API
   exists.

Phase 0 has no transaction/rollback or exceptional-child sentinel promise;
those belonged to the deleted prototype island.

Tests live beside private units unless they exercise a true public/package
boundary. Giant inline historical campaigns, captured schema fingerprints,
compatibility spelling checks, duplicate artifact-authentication ceremonies,
and RustRed-output-as-RustRed-oracle fixtures are deleted.

For lower-loop reduction results, the authoritative direction is external:
extract independently authored expectations already present in Vakint, map
conventions explicitly, and compare the RustRed mode's result. Do not label a
test that only compares two live backends as a frozen golden. The supplied
local path
`FOR_REFERENCE_ONLY_DO_NOT_PUSH/form5` has been inspected and is a React/Node
project, not a runnable FORM5 installation. Therefore a live FORM-backed
alphaLoop/MATAD/FMFT oracle is presently unavailable from that path. Inline
expected values and non-FORM existing tests remain usable immediately; live
oracle regeneration stays a typed external prerequisite rather than prompting
a fake FORM implementation or blocking repository cleanup.

### Exact Vakint north-star seams

The integration must follow Vakint's existing staged pipeline rather than
inventing a parallel frontend:

```text
Vakint::to_canonical
    -> Vakint::tensor_reduce
    -> Vakint::evaluate_integral
```

- Reuse the public `Topologies::match_topologies_to_user_input` entry point and
  its private `Integral::match_integral_to_user_input` implementation for
  topology matching, canonicalization, and routing. Fix and generically extend
  that engine; do not bypass it with a RustRed topology table.
- Keep the existing public exhaustive `EvaluationMethod` enum and
  `VakintSettings` layout unchanged initially. Expose opt-in RustRed
  methods/builders with `RustRedOptions` that enter the same staged pipeline;
  adding an enum variant is not source-compatible and may occur only under an
  explicitly versioned Vakint API decision with downstream audit.
- Add explicit RustRed tensor dispatch at the existing tensor-reduction seam.
  It calls RustRed-core tensor/scalar services and has no FORM dependency or
  fallback.
- Apply compiled RustRed rules and master substitution at the existing
  evaluation seam, preserving Vakint's normalization and presentation layers.

The comparison-source matrix is:

| Vakint evidence | Availability and RustRed use |
|---|---|
| `tests/input_matching_tests.rs` | immediately reusable inline structural expectations: compare accepted family, canonical topology, routing/permutation witness, and rejected-input class; start with `test_1l_matching`, `test_2l_matching_3prop`, and `test_3l_matching_with_zero_powers_in_short_form` |
| `tests/tensor_reduction_tests.rs` | expected scalar expressions are inline and may be extracted into an explicitly versioned corpus; the current producer executes FORM, so do not run it in the RustRed lane; start with `test_reduction_1l_a`, `test_reduction_1l_b`, and `test_reduction_2l_a` |
| `tests/integral_evaluation_analytic_tests.rs` | immediately extractable inline analytic/numerical expectations where master substitution is authoritative; use the existing 1L, 2L, 3L, and representative 4L ladder |
| `tests/integral_alphaloop_vs_matad_tests.rs` | live AlphaLoop-versus-MATAD differential only, with no embedded golden; unavailable until real FORM is installed, then use it to produce a separately reviewed raw corpus rather than calling the test itself a golden |
| `tests/integral_comparison_vs_pysecdec_tests.rs` | optional corroborating one-/two-loop numerical differential; availability depends on its external PySecDec assets/runtime and it is never the sole exact oracle |
| existing non-RustRed regression suite | unchanged input/result/runtime behavior for every old path; add compile fixtures around public steering types where source compatibility is promised |

Inline expectations can seed the new corpus immediately. A live FORM-backed
differential is an opt-in oracle-regeneration job only and needs a real pinned
FORM version at least 4.2.1; that executable is currently unavailable. This
does not authorize FORM in RustRed or its ordinary Vakint mode.

## Vakint dependency during parallel development

After the reset, the local uncommitted dependency from
`FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop/crates/vakint/Cargo.toml` is:

```toml
rustred = { package = "rustred", path = "../../../../crates/rustred-core" }
```

That path is relative to Vakint's manifest and is valid only in this ignored
co-development layout. No machine-specific absolute path is committed.

Before a reproducible GammaLoop milestone is committed or pushed on
`vakint_rustred`, switch to the exact validated RustRed commit:

```toml
rustred = { package = "rustred", git = "https://github.com/alphal00p/rustred.git", rev = "<40-hex validated commit>" }
```

The committed dependency must resolve the package named `rustred` from
`crates/rustred-core`; its directory name is not its Cargo package name.

The single-Symbolica graph is implemented, not merely asserted:

1. `rustred-core` requests the exact registry package version
   `symbolica = "=2.2.0"` through `[workspace.dependencies]`, with GMP enabled
   and `no_gmp` absent. It does not use a path dependency itself.
2. When RustRed is the workspace root, root `[patch.crates-io]` replaces
   `symbolica`, `graphica`, and `numerica` with the pinned
   `vendor/symbolica` checkout. Thus standalone development remains offline
   and reproducible.
3. On `vakint_rustred`, GammaLoop replaces its present moving `branch =
   "dev"` patches for those same packages with one jointly tested exact Git
   `rev`. GammaLoop's generated `crates/gammaloop-workspace-hack/Cargo.toml`
   currently also contains four direct `branch = "dev"` Symbolica/Numerica
   dependencies (normal and build); a crates.io patch cannot replace those.
   Regenerate the Hakari workspace-hack after pinning, or edit its generator
   inputs and regenerate it, so every direct and patched Symbolica-family
   dependency has the same exact source/revision. Only then do the path- or
   Git-sourced RustRed package and Vakint receive one Symbolica package
   identity. The current RustRed vendor revision is
   `77c137481904b8a5531ede86e3ef36b82beed7fd`, whereas the current GammaLoop
   lock resolves `0441bd7a511209dce2ca99925fe87f8b18e4bf03`; adapter work may not
   pretend these are unified. Test one revision against both workspaces and
   pin both sides before exposing shared CAS types.
4. Delete RustRed's manifest-scraping `build.rs`; producer metadata uses the
   resolved Symbolica public API `LicenseManager::get_version()`.
5. Until `cargo tree` proves exactly one Symbolica source/revision, the
   cross-repository boundary exposes owned RustRed domain values rather than
   `Atom`/`AtomView`. Vakint owns conversion. A zero-copy Symbolica-`Atom`
   boundary is only a later measured optimization after unification.

This scheme lets GammaLoop's root choose the exact compatible source while
RustRed's standalone workspace uses the pinned submodule, without two nominally
different Symbolica `Atom` types.

The GammaLoop gate scans every tracked `Cargo.toml` for direct
`symbolica-dev/symbolica` dependencies, rejects all moving `branch` selectors,
checks `Cargo.lock` source revisions, and audits `cargo tree -d` plus the full
source-qualified `cargo tree` output for duplicate `symbolica`, `graphica`, or
`numerica` package identities. The generated workspace-hack is part of that
gate, not an exception.

## Rollback-sized execution order

The order is chosen to avoid updating code that will be deleted.

### R0 — freeze this decision

- Commit `GOAL.md` plus this plan only.
- Record the measured baseline and do not mix code movement into the decision
  commit.

### R1 — delete the obsolete validation lane

- Delete `crates/rustred-legacy-oracles/**`.
- Delete `src/legacy_oracle_support.rs`, the core feature, dependency edges,
  live README promises, and default/non-default workspace configuration that
  exists only for that package. Do not repair dated research logs that are
  already assigned to R7 deletion.
- Delete the matrix-conversion and retained-heap helpers in `src/exact.rs`
  whose only production caller was that bridge; retain coefficient-degree and
  sparse-elimination code that still has a live core caller.
- Do not update any legacy path for the future core location.

Gate: default core/app/Python check only; no legacy-suite repair.

### R2 — establish sentinels, then delete historical repository surfaces

- Generate the tracked-path liveness ledger and identify the retained
  capability spine.
- Before deleting an old test surface for a retained cluster, write a compact
  fresh sentinel beside the live owner for that cluster. The initial sentinels
  cover family normalization, generic IBP/LI generation, the structural
  six-loop source count, exact Symbolica row operations, zero/symmetry, generic
  reduction primitives, and deterministic campaign plan/resource preflight. They are new
  contract tests, not mechanically repaired legacy binaries. Do not preserve
  transaction/rollback or exceptional-child sentinels from the solver island.
- Remove the LiteRed2 and GammaLoop gitlinks and their `.gitmodules` entries;
  retain Symbolica and the one-entry `.gitmodules` file.
- Delete the entire root `tests/`, `examples/`, `tools/`, and `scripts/` trees.
  Recreate only fresh app fixtures or package boundary tests when a supported
  schema/use case needs them; do not salvage old paths first.
- Verify ignored reference material remains unstaged.

Gate: every retained capability touched by the deletion has its fresh sentinel
green; tracked-file/gitlink audit, default core/app/Python checks, and the
public Python import smoke test pass.

### R3 — form and prune the retained domain spine in place

The current deletion checkpoint has already removed the legacy sparse solver,
coefficient projection/parser compatibility, partial-specialization/replay,
aggregate concrete-specialization census, retained-payload serialization,
exact-Integer translation, two-phase guarded division, and unused integer-
matrix adapter. The remaining coefficient and relation migration units retain
only complete K(n) translation/specialization, guard provenance, direct
Symbolica coefficient operations, and mathematical pre-operation bounds. They
must still be renamed and split into the semantic algebra/identity owners;
their shorter current files are not accepted as final root modules.

The retained exponent boundary now matches Symbolica's representation:
stored and configured caps are `u16`, pairwise prospective degree sums use
checked `u32`, and degree-by-power projections use checked `u64` before native
or repeated arithmetic. Term and memory counts remain `usize`; integer-bit
and sector-ordering bounds retain their independent wider domains. Native
coefficient powers additionally enforce Symbolica's `u32` exponent ceiling,
conservatively preflight componentwise degree-box support and
per-multiplication term work, and charge its current linear multiplication
schedule before entry, including for degree-zero coefficients. Every returned
power is reauthenticated and charged to the native output-retained-byte
budget.

This audited checkpoint deletes the closed shift-operator intermediate and
the four-file, 7,829-line tensor prototype SCC, plus the uncalled campaign
admission/work modules, their work-key wave/resource helpers, and the core
replay/dependency plan after moving its sole live roots-only interning into the
application. It also deletes the matrix
module's now-orphaned scalar-power wrapper and its self-tests; direct native
coefficient power remains only at Symbolica's required field boundary. The
nominal exact-rational wrapper is deleted, and the sole half-coefficient use is
constructed by checked division in the Symbolica rational-polynomial field
rather than by an infallible unchecked-native conversion. Tensor is
rebuilt only after its fresh custom-head covariant/scalar-lowering and
rank-four pairing sentinels exist; internal self-use and root re-exports were
not accepted as liveness. The retained core campaign boundary stops at
resource/preflight values, width selection, and bounded ordered execution;
application composition owns roots-only interning and any later runtime
scheduling.

- Delete `src/solver/**` plus the dependent exact-session, closure,
  publication, re-entry, generated-affine/cylindrical/residual, and provider
  SCCs. Do not relocate or preserve them.
- Under the current root package, create the acyclic owners `algebra`,
  `family`, `input`, `identity`, `sector`, `campaign`, `tensor`, and
  `reduction`. There is no Phase-0 `foundry`, `artifact`, or wrapper-only
  `runtime` owner.
- Work cluster by cluster. Move/split only symbols justified by a live app,
  retained-core caller, upcoming Vakint use case, or fresh retained-primitive
  sentinel; delete the rest in the same rollback-sized tranche. A hypothetical
  future foundry is not sufficient liveness evidence. An unclassified
  liveness-ledger path blocks the tranche.
- At the start and end of every cluster milestone, exhaustively enumerate all
  tracked `*.rs` files, publish the changed counts in the commit audit, and
  revisit every remaining flat or long state/chronology name. No earlier
  `split` decision carries forward automatically.
- Audit every concrete topology-name and fixed-loop literal match in retained
  core sources. Test/benchmark fixtures may name topologies; production code
  may use loop count only as generic data or a measured size threshold and may
  specialize only on proved semantic family classes.
- Put a narrow domain facade in place as each cluster moves, update internal
  imports to domain paths, and add no old-path aliases. Do not optimize for a
  minimal file count: split large retained implementations into cohesive
  role-named children under their stable parent domain. Every multi-thousand-
  line survivor must receive an explicit cohesion audit before R3 closes.
- Keep mechanical movement and dependency inversion separate from the later
  fresh-foundry work. Every retained cluster's sentinel exists before its old
  tests or implementation variants disappear.

Gate: focused licensed sentinels stay green after every cluster; static imports
move toward the declared DAG; no new prefix dumping ground or compatibility
facade appears.

### R4 — replace the root facade and finish liveness pruning

- Replace the 750-line facade with the smallest intentional `rustred` API
  required by the retained spine, app/CLI/Python, and Vakint boundary.
- Delete every unreferenced source, old schema generation, compatibility
  constructor/re-export, duplicate provider stack, and eager MTBDD path. Do
  not silence dead code with broad `allow` attributes.
- Delete CAS duplicates only when they are dead or a completed Symbolica API
  audit plus differential/sentinel evidence has already transferred authority.
  Keep any still-live migration explicit for Phase 1 instead of performing a
  rushed rewrite during structural cleanup.

Gate: the core library has zero warnings and no compatibility feature;
app/Python compile against the new facade; the retained-path ledger is closed.

### R5 — virtualize the workspace and relocate the now-small live package

- Make root `Cargo.toml` virtual, add `crates/rustred-core` to `members` and
  `default-members`, and create its manifest with `package.name = "rustred"`.
- Move the already structured live source tree to
  `crates/rustred-core/src/`; do not move the former 402k-line tree as an
  intermediate architecture.
- Implement the consumer-overridable Symbolica scheme above, delete `build.rs`,
  and replace its version call with `LicenseManager::get_version()`.
- Update `rustred-app` to `path = "../rustred-core"`; keep Python depending on
  app. Retain root `pyproject.toml` and update/verify its manifest and package
  paths so users still run `import rustred` while `_rustred` stays private.
- Leave no root `src/`, `tests/`, `examples/`, `tools/`, or `scripts/`
  directory.

Gate: `cargo metadata`, core/app/Python checks, one-source Symbolica tree,
package identity, public Python import, and final tree shape. Apart from the
explicit manifest/version-source changes, this commit is a mechanical move.

### R6 — complete the fresh evidence suite

- Complete only the generic contract matrix listed above; earlier sentinels
  stay as its nucleus.
- Keep private tests beside their modules and a small number of true black-box
  tests under package `tests/`.
- Validate app/CLI/Python and stage immediately extractable Vakint expectations
  without copying legacy recurrences or calling live differential tests
  goldens.

Gate: licensed default-GMP workspace tests, deterministic concurrency cases,
exact retained-primitive evidence, and no FORM in the default graph.

### R7 — replace the documentation corpus

- Write the six stable documents and a concise README describing only actual
  capabilities.
- Delete every dated research/log document after its live decision is captured.
- Remove broken/private/redundant doc links and stale package/path names.

Gate: strict rustdoc and link/path scan are green.

### R8 — Phase-0 acceptance and push

- Independent architecture, CAS-authority, deletion-scope, and capability
  audits.
- Commit and push each prior gate separately; do not squash the reset into one
  opaque change.
- Only after all gates pass may the fresh foundry work and the parallel Vakint
  RustRed mode begin.

## Phase-0 acceptance commands and invariants

Use the license from the environment; do not write it into commands, files,
or logs.

```text
cargo fmt --all --check
cargo metadata --format-version 1 --no-deps
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
git diff --check
```

Required structural assertions:

- root `Cargo.toml` has `[workspace]` and no `[package]`;
- `cargo metadata` reports package `rustred` at
  `crates/rustred-core/Cargo.toml`;
- root `src/`, `tests/`, `examples/`, `tools/`, and `scripts/` do not exist;
- no package, feature, module, or manifest mentions
  `rustred-legacy-oracles` or `legacy-oracle-support`;
- no default-production target contains a loop/topology-authored recurrence;
- the only tracked vendor gitlink is `vendor/symbolica`;
- no tracked path is under `FOR_REFERENCE_ONLY_DO_NOT_PUSH/`;
- no package enables Symbolica `no_gmp`;
- core-domain and frontend dependencies follow the declared DAG;
- compiler and rustdoc warnings are zero without blanket warning suppression;
- public docs expose `import rustred`, never `import _rustred`; and
- capability statements remain limited to the fresh passing evidence.

The reset succeeds by producing a small comprehensible base, not by preserving
the current file count or test count. Git is the legacy archive. Vakint
end-to-end agreement, exact regenerated-source replay, and eventually closed
artifacts are the forward evidence.
