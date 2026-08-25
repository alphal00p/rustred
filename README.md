# RustRed

RustRed is a pure-Rust implementation of the LiteRed approach to parametric
integration-by-parts (IBP) identities for Feynman integrals. Symbolica already
supplies the main exact coefficient algebra and is the required target for all
remaining production algebra; the status and roadmap below identify the
pre-existing custom layers that still have to be migrated.

RustRed does not invoke FORM or a Mathematica kernel. The checked-in build uses
Symbolica's licensed GMP backend and deliberately does not enable `no_gmp`.

## Status

RustRed is an active implementation project, not yet a drop-in replacement for
all of LiteRed.

| Capability | Status |
|---|---|
| Parse compact integral-family descriptions | Available |
| Infer scalar parameters from family-defining expressions | Available |
| Derive generic ordinary parametric IBPs | Available through `rustred derive` and the library |
| Derive generic Lorentz-invariance identities | Available through `rustred derive` and the library |
| Preserve symbolic nonzero conditions and proof-component replay evidence | Available in the library |
| Search authenticated normalized coverage formulas without V4/V5 materialization | Implemented internally as a bounded, replayable cursor; public library/CLI integration is pending |
| Bind integral ordering into normalized coverage authority | Implemented and independently validated in normalized-source V2 at pushed checkpoint `c593865`; public library/CLI integration is pending |
| Bridge a direct actionable residual through case authority, ordering, physical frame, solve plan, and exact session | Implemented internally for a singleton case with allocation-independent stable identity; authenticated selector-independent compact affine maps reach the unpublished `ReadyForConditions` gate, with boundary hazards retained for later `WhenBad` partitioning |
| Project physical-parameter identities and schedule exact Ready condition sources | Implemented internally with Symbolica polynomial projection and an owner-bound identity/compact-affine plan; mapped condition materialization, boundary specialization, Boolean partitioning, and publication remain pending |
| Chronologically replay committed generated-affine exact-session transitions | Available for both the legacy-inventory slice and the source-profiled Direct singleton slice; rule publication remains pending |
| Process a concrete numerator through tensor projection and scalar lowering | Available through the direct library path |
| Track published LiteRed notebook parity | Eight LiteRed 1.x examples inventoried with staged acceptance levels; no complete notebook workflow passes yet |
| Automatically reduce every arbitrary integral to masters | **Not yet complete** |
| Reproduce full LiteRed sector solving, `WhenBad` closure, and rule publication | **In progress** |

The important distinction is that `derive` constructs universal, fully
parametric identities for the declared family. It does not choose masters or
claim that a concrete target has been reduced.

## What `derive` generates

For a complete affine denominator basis with `L` loop momenta and `E` external
momenta, the generic generator constructs:

- all `L(L+E)` ordinary total-derivative IBP identities;
- all `E(E-1)/2` Lorentz-invariance identities;
- coefficients over the authenticated Symbolica field `K(n)`;
- integer shift vectors for every integral term; and
- exceptional nonzero conditions with typed provenance.

There is no topology name, vacuum-only branch, or loop-count dispatch in this
derivation path. Concrete topologies are fixtures used to validate the generic
algorithms.

## Quick start

Rust 1.89 or newer is required. Clone the Symbolica source together with the
repository:

```bash
git clone --recurse-submodules https://github.com/alphal00p/rustred.git
cd rustred
```

The supplied Nix development shell includes Rust, GCC, `m4`, `pkg-config`,
Perl, and `cargo-nextest`:

```bash
nix develop
```

Export a valid Symbolica license, then build the GMP-enabled CLI:

```bash
export SYMBOLICA_LICENSE='your-symbolica-license'
export SYMBOLICA_HIDE_BANNER=1
cargo build --release --bin rustred
```

`SYMBOLICA_HIDE_BANNER` only keeps machine-readable CLI output clean; it does
not configure or replace the license.

RustRed itself is distributed under the [MIT license](LICENSE). Symbolica is a
separate dependency with its own licensing terms.

## Tested CLI derivation

[`examples/cli/one_loop.symbolica`](examples/cli/one_loop.symbolica) contains:

```text
I(
  name(tadpole),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1),
  numerator(sp(k,k))
)
```

Run the literal checked example from the repository root:

```bash
cargo run --quiet --bin rustred -- derive \
  --input examples/cli/one_loop.symbolica \
  --input-format symbolica \
  --relations ordinary
```

The tested output contains these fields:

```toml
schema = "rustred.derive-output.toml.v1"
status = "ok"
relation_selection = "ordinary"
target_disposition = "not_processed_by_derive"

[relation_counts]
generated_ordinary = 1
generated_li = 0
emitted_ordinary = 1
emitted_li = 0
emitted_total = 1

[[relations]]
stable_id = "ordinary-ibp:0:0"
```

That result proves that RustRed parsed the family, inferred `d` and `m2`, and
derived the one ordinary parametric identity expected for `L=1, E=0`. The
emitted relation has terms at shifts `[0]` and `[1]`, with coefficients that
remain symbolic in `d`, `m2`, and the abstract index `n0`.

In ordinary mathematical notation, the emitted identity is

```text
0 = (d - 2 n0) I(n0) - 2 m2 n0 I(n0 + 1)
```

It also proves a deliberate boundary: the powers and `sp(k,k)` numerator are
retained in `[target]`, but their disposition is
`not_processed_by_derive`. The CLI has derived an IBP; it has not claimed a
complete reduction of that target.

## Input modes

The CLI accepts three compact modes which normalize into the same authenticated
family representation:

- raw Symbolica `I(...)`, as above;
- hybrid TOML containing one compact `integral = """I(...)"""` plus metadata;
- fully explicit compact TOML for generated configurations.

Parameter declarations are optional when they can be inferred. They remain
useful as an explicit allowlist or for symbols used only by a retained target
numerator. Grammar, examples, output schema, exit codes, and atomic-I/O rules
are documented in [`docs/CLI.md`](docs/CLI.md).

## Direct library path

The Rust library exposes more of the work-in-progress pipeline than the CLI:

```text
family declaration
  -> affine basis authentication / optional ISP completion
  -> parametric IBP and LI generation
  -> sector, zero, symmetry, and exact-elimination certificates
  -> tensor projection and scalar-product lowering
  -> guarded application of whatever reduction coverage was proved
```

[`examples/generic_symbolica_tensor_ibp.rs`](examples/generic_symbolica_tensor_ibp.rs)
shows this direct path. Its APIs are still evolving and should not be confused
with a finished arbitrary-family reducer.

## Tested milestones

The repository currently includes tests for:

- generic ordinary IBP and LI generation with exact replay;
- raw Symbolica, hybrid TOML, and explicit TOML input normalization;
- parameter inference without a required `parameters(...)` clause;
- affine denominator-basis completion for independent short lists;
- zero-sector, symmetry, guarded specialization, and sparse-elimination proof
  components;
- an exact-GMP, session-owned `Solvej` recentering transaction: authenticated
  post-top-reduction leaders are matched against persisted targets and return
  sealed NoTarget, affine-equality-refinement, or Ready outcomes;
- typed NoTarget commit and affine-equality suspension: NoTarget commits the
  algebraic pivot while preserving every target, whereas an equality-bearing
  first target commits the pivot and seals the old solve epoch for refinement;
- an opaque, session-owned recipe for every committed production or synthetic
  source, with the exact dependent-reduction or new-pivot evidence retained by
  shared ownership rather than reconstructed from public metadata; production
  recipes admit the complete uniquely retained physical-row/re-elimination
  graph while excluding only pointer-proven shared plan/frame/inventory
  ancestry;
- an owning prepared-database commit whose fallible authentication, resource
  accounting, and allocation finish before its infallible commit tail;
- a private chronological event ledger for the generic generated-affine exact
  session, plus shadow replay which re-stages each opaque source and re-executes
  its dependent, NoTarget, or affine-equality transition before comparing the
  resulting evidence and terminal state;
- the first exact Ready-analysis checkpoint: for an authenticated independent
  affine cylinder it locates the unit pivot, proves every RHS strictly descends
  through the persisted arbitrary-precision physical-key ordering, and retains
  finite inactive-orthant hazard intervals without narrowing to machine
  integers; this checkpoint neither consumes a target nor publishes a rule;
- one-loop scalar and tensor comparisons against frozen Vakint-derived oracles,
  including an independent public-Symbolica rank-six (15 by 15) projector
  reconstruction;
- concrete two-loop sunset and three-loop tetrahedron scalar/tensor fixtures;
- elementary factorized four- and five-loop fixtures which exercise the same
  loop-count-neutral library stack;
- a fast test-only `L=6`, `K=21` coordinate-family gate proving the generic
  generator emits all 36 ordinary IBPs in deterministic row order with stable
  replay manifests; it does not claim sector coverage or reduction;
- a bounded authenticated residual-path cursor over the existing coverage
  MTBDD, with compact replay/resource tests and an ignored all-36 `K=21`
  stress oracle; the latter measures 49 normalized atoms, 268,427 retained
  nodes, and an exact 43-decision first Unsupported path, and is not a Ready or
  reduction result;
- a sealed, replayable normalized-coverage source and a second bounded cursor
  that walks its authenticated candidate bad-formulas directly, retaining only
  one three-valued assignment table and DFS frontier and constructing neither
  the V4 materialized partition nor the V5 MTBDD;
- the pushed `c593865` one-pass candidate-to-normalized-source ingress
  with a safe sealed replay token. It performs `N` construction
  authentications for `N` candidates rather than the legacy `2N`; focused run
  `b2ba7679-e7c8-4e64-ba25-c451024843bf` passed 6/6 tests and independent
  affected-suite run `db2a98a5-d473-4cdc-b2b7-fe2f444357e8` passed 44/44. The
  honest all-36 `L=6`, `K=21` primary run
  `37d85ddb-c356-4c79-a6f4-d428828db039` passed 1/1 in 58.109 seconds with 36
  construction authentications, 49 loci, 36 attempts, 15 Certified and 21
  Unsupported outcomes. Candidate-to-source construction took 17.4507
  seconds, direct-cursor initialization 16.756 microseconds, and first-residual
  search 832.37 microseconds; the path used 30 decisions with 19 loci free and
  a 1,841-byte peak cursor, and all 524,288 completions were checked. An
  independent rerun, `e00cdbea-6312-4fb3-9856-0c2f3bf2ef25`, also passed in
  56.359 seconds.
  Explicit source and path stress-validation replays took 18.51 and 17.57
  seconds; those replays are not part of production direct-search cost. The
  earlier two-stage run `e7378e6e-5df5-47c3-8fe9-686bbaa8ef30` took 72.935
  seconds, split 17.29 + 16.21 seconds across its two construction phases and
  performed 72 construction authentications. This pushed checkpoint invokes
  no MTBDD compiler and constructs no MTBDD owner or DAG; it is not an
  arity-21 Ready result, published rule, reduction, or physical-topology
  calculation;
- normalized-source V2 ordering-policy binding at pushed checkpoint `c593865`.
  Every source owns one explicit `IntegralOrderingPolicy`, including
  an empty-attempt source, and every present candidate's policy is
  authenticated. Owner-focused run
  `8ad499a3-339e-4e0b-a04f-ccf754406516` passed 21/21 tests, the
  formula/residual suite `6a5267d1-fe75-4854-8b98-9a03b1bb2370` passed 14/14,
  and independent audit/validation run
  `430af297-b806-431e-a169-bd0f19a9f9c8` passed 30/30. Policy-bound all-36
  `L=6`, `K=21` run `88a73ec1-52c2-4771-8a21-75e1b2a848b6` passed 1/1 with
  36 construction authentications, the unchanged 15 Certified/21 Unsupported
  semantics, and a 1.405-millisecond first-residual search. This remains a
  pushed internal checkpoint, not a Ready result, reduction, or physical
  topology calculation;
- the generic direct formula-path coordinate-affine terminal and its
  direct-singleton authority path. The terminal now has a complete,
  allocation-independent stable-value identity from the authenticated
  normalized source through the terminal proof. The row span is serialized
  once and subsequent occurrences use typed identity references. Direct
  authority carries that identity through generated ordering V3, physical
  frame V2, and solve-plan V2 without fabricating a legacy inventory. Stable
  value equality does not replace proof ancestry: replay still authenticates
  the exact retained terminal, authority, and frame `Arc` allocations. Exact
  relation entry points that require a legacy inventory reject Direct plans
  rather than manufacturing compatibility state. Source-profiled V2 target,
  database, and session owners instead retain and authenticate the exact Direct
  plan/authority allocations. Authenticated selector-independent compact
  affine maps, including constrained maps, now reach the unpublished
  `ReadyForConditions` gate. The V2 analysis verifies exact selector geometry,
  fixed-chamber physical-key descent, and lazy inactive-orthant hazards; replay
  reauthenticates the exact owner allocation and rebuilds the transcript.
  Independent licensed default-GMP run
  `b60b4fbd-f7b9-4656-ade0-6a476a7b7805` passed all 18 focused tests with four
  workers. The next foundation now retains an ownership-safe deterministic
  source and lazy-hazard schedule for identity and compact-affine target maps,
  and uses Symbolica polynomial projection to classify a coefficient as
  identically zero, never identically zero, or conditional in arbitrary-width
  integral indices. Its focused licensed default-GMP gates passed 6/6
  condition-plan tests with four Nextest workers and 6/6 projector tests with
  four Rust test threads. Independent combined run
  `f6c4a9e7-fcc1-4c48-ae3c-5f2c0d781e42` passed 22/22 tests with four Nextest
  workers, including affected Ready/session regressions. This milestone does
  not yet compose mapped
  conditions, specialize affine boundaries, construct a relative partition,
  publish a rule, perform a reduction, or establish six-loop support; and
- seven end-to-end numerator-spelling closure pairs in
  [`tests/one_loop_numerator_cancellation_closure.rs`](tests/one_loop_numerator_cancellation_closure.rs):
  scalar and squared denominator cancellation, rank-two/rank-four/rank-six
  tensor cancellation, metric-contraction cancellation, and traced-rank-six
  versus scalar-product spelling.

Those closure pairs independently rebuild generated parametric IBPs on both
sides and compare unreplaced-master output. They pass through compact numerator
parsing, the generic tensor projector, scalar lowering, and guarded rule
application. Projector coefficient powers and Gram algebra use public
Symbolica APIs; FORM is not invoked. They currently use the direct library
path; `rustred derive` only retains their numerator metadata.

The multi-loop fixtures demonstrate concrete validated computations. They are
not proofs of complete symbolic coverage for all integer powers or all
topologies at those loop counts. Legacy authored finite-oracle modules are
feature-gated validation material, not production topology dispatch.

## Current blockers and roadmap

The priority deployment is now a six-loop QCD beta-function campaign after
GammaLoop's general BPHZ R-operation.  RustRed will keep its full generic
LiteRed scope, but near-term work emphasizes single-scale massive vacuum
families with the common mass set to one.  The architecture is deliberately
two-stage: an offline, topology-generic foundry derives and verifies guarded
parametric rules for canonical families/sectors; a separate online runtime
applies the compiled artifacts in batches to the many concrete numerator
integrals produced by GammaLoop.  See the
[six-loop single-scale vacuum priority](docs/research/six_loop_single_scale_vacuum_priority_2026-08-24.md).

The transactional exact pivot database, hardest-first top reduction,
session-owned exact recentering, typed NoTarget commit, sealed affine-equality
suspension, private chronological event ledger, and shadow replay are
implemented for the generic generated-affine exact-session slice. Committed
events own opaque source recipes and exact evidence, and the owning prepared
database transition has an infallible commit tail after all fallible work has
completed.

The B0 algebra blocker has been removed. [`src/exact.rs`](src/exact.rs) now
keeps only a nominal RustRed wrapper around Symbolica's GMP `Rational`; scalar
arithmetic and exact matrix inverse, rank, determinant, multiplication, and
transpose cross the public `Q`/`Matrix<Q>` APIs. The old fixed-width rational,
gcd, Gaussian-elimination, and determinant implementations have been deleted.
Checked row-major conversion, allocation/shape admission, panic containment,
and the independent singularity guard remain RustRed-owned boundaries.

B0 is not the end of the Symbolica-first cleanup. Generated-affine composition,
strict polynomial-associate checks, generic-family matrices, automatic ISP
rank, tensor-projector matrices, and the affine-family/symmetry verifier have
since crossed public Symbolica APIs. The latter now delegates determinants,
transpose, Gram congruence, and denominator-coordinate products through an
authenticated V2 boundary while retaining an independent physics replay.
Native Symbolica dense and sparse solves must likewise replace the older
custom `exact_sparse_elimination` wherever the public API applies. The pinned
sparse solve has a documented validation caveat, so the production migration
must use public `SparseRowReducer` together with independent rank/residual and
transcript checks; RustRed does not extend this into a parallel CAS or matrix
implementation.
The next vacuum-critical solver milestone is the topology-neutral
`Ready -> condition/WhenBad partition -> atomic guarded-rule/residual
publication -> replay-certified closure` slice. Exact fixed-chamber descent
and lazy hazard geometry now cover authenticated selector-independent compact
affine target maps. An owner-bound identity/compact transform and deterministic
condition-source schedule now preserve the Ready lineage, while Symbolica
projects physical-parameter identities into exact index-polynomial loci. The
active slice must compose those sources, specialize the lazy affine boundaries
by exact polynomial divisibility, build complete relative `WhenBad` partitions,
and publish atomically. Graph-
lifted symmetry discovery and the `SparseRowReducer` transcript-equivalence
path follow on the scaling route; unrelated Feynman/non-vacuum algebra
migrations stay required but no longer displace the six-loop vacuum critical
path.

Alongside that staged algebra migration, pushed checkpoint `c593865` binds one
explicit `IntegralOrderingPolicy` into every normalized source, including an
empty-attempt source, and authenticates every present candidate's policy. The
current checkpoint additionally carries an `Actionable` Direct singleton from
the allocation-independent terminal stable-value identity through ordering V3,
physical frame V2, solve-plan V2, and source-profiled exact-session staging and
recentering. The row span is emitted once through typed identity references;
the Direct path fabricates no V4/V5, Boolean/DPLL, integer-system, or legacy
inventory certificate. Stable identity remains separate from exact retained-
`Arc` authority. Authenticated selector-independent compact affine target maps
reach the existing unpublished `ReadyForConditions` gate, while chamber exits
remain explicit hazards rather than being sampled away. The current owner-bound
condition plan authenticates identity and compact maps and schedules premises,
row guards, pivot/RHS coefficients, and lazy hazard locators; Symbolica-backed
projection supplies the exact physical-parameter identity clauses. The
remaining generic LiteRed-style solver work is to:

1. compose the scheduled conditions, specialize affine boundary clauses,
   close `WhenBad` exceptional branches, and atomically publish guarded rules
   and residual work;
2. feed solved subsectors into supersectors and iterate residual cases; and
3. expose a replay-certified complete reduction result.

This checkpoint is not a complete LiteRed port, complete `WhenBad` or rule
publication, an arbitrary one-loop pentagon reduction, or completion of the
two- through six-loop reduction campaign. Concrete multi-loop families
currently serve only as bounded validation fixtures. Non-vacuum pentagon work
remains in scope but is behind the vacuum rule-foundry and batch-application
critical path.

A genuine all-inactive `K=21` probe first exposed the legacy Boolean-cover cap
at split 65,537 of 65,536. The replacement MTBDD avoids that explicit
`2^K` partition, and its new cursor avoids flattening all terminal paths, but
the real all-36 source still constructs 49 atoms and 268,427 nodes before the
cursor can return its first residual. The cap is not being raised. The
backend-neutral normalized source is now a separate replayable owner shared by
the MTBDD backend, with exact source-Arc fast paths on the common case. Sealed
fresh normalization, the bounded direct formula cursor, the safe one-pass
candidate ingress and sealed replay token, normalized-source V2
ordering-policy binding, and the K21 evidence above are implemented at pushed
checkpoint `c593865`. The all-36 comparison above reduces
construction authentications from 72 to 36 and replaces the old 17.29 + 16.21
second two-stage ingress with one 17.4507-second candidate-to-source phase. The
direct cursor remains millisecond-scale. The 18.51-second source replay and
17.57-second path replay in the fixture deliberately reauthenticate for stress
validation and are not production direct-search phases. Normalized-source V2
now carries and authenticates `IntegralOrderingPolicy`; the focused 21/21 and
14/14 suites, independent 30/30 audit/validation, and policy-bound K21 1/1 run
listed above passed. This checkpoint's coordinate-affine terminal now has an
allocation-independent stable-value identity and reaches the source-profiled
exact session through Direct solve-plan V2, ordering V3, and physical frame V2,
with exact `Arc` ancestry kept separate and no fake inventory. Authenticated
selector-independent compact affine maps reach the unpublished
`ReadyForConditions` gate, and the new owner-bound plan schedules its exact
condition sources without consuming the target. Symbolica-backed
physical-parameter identity projection is also implemented. The immediate
gates are mapped condition composition, boundary specialization, relative
`WhenBad` partitioning, and atomic guarded publication. No arity-21 case has
reached Ready, no guarded rule has been published, no physical topology was
reduced, and no complete reduction is claimed.

Further LiteRed parity includes broader symmetry discovery, partial fractions
for dependent or overcomplete propagator lists, master inference, persistent
proof serialization, dimension shifts, and differential equations.

## Testing

Run the licensed test suite in parallel with the bounded default of four jobs:

```bash
export SYMBOLICA_LICENSE='your-symbolica-license'
./scripts/test.sh
```

Override concurrency with `RUSTRED_TEST_JOBS`. When `cargo-nextest` is
available, the script runs test binaries concurrently; it otherwise keeps
Cargo's parallel test workers. No test path enables `no_gmp`.

## Documentation

- [CLI contract and input formats](docs/CLI.md)
- [RustRed scope and acceptance criteria](docs/research/rustred_scope_and_acceptance.md)
- [Six-loop single-scale vacuum priority](docs/research/six_loop_single_scale_vacuum_priority_2026-08-24.md)
- [Full LiteRed scope specification](docs/research/litered_full_scope_spec.md)
- [LiteRed example parity acceptance matrix](docs/research/litered_examples_acceptance_matrix.md)
- [LiteRed2 algorithm report](docs/research/litered2_algorithm_report.md)
- [Generic IBP parity audit](docs/research/generic_ibp_litered_parity_audit_2026-08-13.md)
- [Symbolica Rust API reference](docs/research/symbolica_rust_api_for_litered.md)
- [Symbolica exact linear-algebra API inventory](docs/research/symbolica_exact_linear_algebra_api_inventory.md)
- [Symbolica-first algebra migration audit](docs/research/symbolica_first_algebra_migration_audit_2026-08-24.md)
- [Symbolica upstream correctness and embedding-gap audit](docs/research/symbolica_upstream_gap_audit_2026-08-25.md)
- [Vakint/alphaLoop tensor and IBP audit](docs/research/vakint_alphaloop_tensor_ibp_audit.md)
- [One- and two-loop validation audit](docs/research/one_two_loop_vacuum_validation_and_legacy_quarantine_2026-08-20.md)
- [Exact-group database design](docs/research/litered_solvej_exact_group_database.md)
- [Exact-session `WhenBad` and publication port plan](docs/research/exact_session_when_bad_port_plan_2026-08-24.md)
- [Residual recentering design](docs/research/litered_solvej_residual_recentering_2026-08-13.md)
- [Latest persistent quotient checkpoint](docs/research/persistent_cylindrical_numeric_quotient_checkpoint_2026-08-20.md)

## Contributing

Contributions should keep production algorithms topology-independent, retain
exact replay evidence, and add concrete topologies only as validation inputs.
Please run formatting, checks, and the relevant parallel tests before opening a
change.

## License

RustRed is available under the [MIT License](LICENSE), copyright RustRed
contributors.
