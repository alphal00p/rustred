# RustRed

RustRed is a pure-Rust implementation of the LiteRed approach to parametric
integration-by-parts (IBP) identities for Feynman integrals. Symbolica already
supplies the main exact coefficient algebra and is the required target for all
remaining production algebra; the status and roadmap below identify the
pre-existing custom layers that still have to be migrated.

The primary conceptual and mathematical reference is
**[rnlg/LiteRed2](https://github.com/rnlg/LiteRed2)**, vendored in
[`vendor/LiteRed2/Source/LiteRed2026.m`](vendor/LiteRed2/Source/LiteRed2026.m).
LiteRed 1.x is tracked only as a historical source of published notebook
acceptance oracles; it does not define RustRed's implementation target.

RustRed does not aim for bug-for-bug, source-level, or Mathematica-API
compatibility with LiteRed2. It preserves the generic mathematical capability
and validated conventions while deliberately using a typed, Symbolica-native
Rust architecture, scalable data structures, explicit resource bounds, and
parallel algorithms. In this repository, “LiteRed parity” means semantic
agreement for an explicitly stated acceptance surface, not identical
intermediate rules, pivot order, global state, or implementation quirks.

RustRed does not invoke FORM or a Mathematica kernel. The checked-in build uses
Symbolica's licensed GMP backend and deliberately does not enable `no_gmp`.

## Status

RustRed is an active implementation project and does not yet cover LiteRed2's
complete mathematical workflow.

| Capability | Status |
|---|---|
| Parse compact integral-family descriptions | Available |
| Infer scalar parameters from family-defining expressions | Available |
| Derive raw generic ordinary parametric IBP identities | Available through `rustred derive` and the library |
| Derive raw generic Lorentz-invariance identities | Available through `rustred derive` and the library |
| Authenticate and deduplicate multiple campaign roots | Available through roots-only `rustred campaign plan`; dependency discovery and execution are not started |
| Plan, admit, and settle RAM-aware campaign waves | Available as low-level library primitives with move-only core/estimated-memory guards and stable bounded dispatch; calibrated physical estimation and the production frontier coordinator are not started |
| Derive a coverage-closed guarded replacement-rule system | **Not yet complete**; exceptional recursion, subsector feedback, and a proved fixed point remain pending |
| Preserve symbolic nonzero conditions and proof-component replay evidence | Available in the library |
| Search authenticated normalized coverage formulas without V4/V5 materialization | Implemented internally as a bounded, replayable cursor; public library/CLI integration is pending |
| Bind integral ordering into normalized coverage authority | Implemented and independently validated in normalized-source V2 at pushed checkpoint `c593865`; public library/CLI integration is pending |
| Bridge a direct actionable residual through case authority, ordering, physical frame, solve plan, and exact session | Implemented internally for a singleton case with allocation-independent stable identity; selector-independent compact affine maps feed condition/materialization/partition owners and the compact application-event commit |
| Project physical-parameter identities, schedule/materialize exact Ready conditions, and partition their relative bad domain | Implemented internally: Symbolica projects both denominator identities, exact lazy hazards become boundary events, and the current-lineage arbitrary-width OR-of-AND `WhenBad` formula is compiled into final relative cases without topology or loop-count dispatch |
| Prepare, commit, and inspect a compact application event | Implemented internally: one move-only input advances the exact database and consumes one selected target; its shallow event owner exposes zero-copy applicable-rule and exceptional-residual views, target premises/geometry, pivot metadata, canonical loci, final cases, and one-byte routes |
| Compile committed events into a bounded parallel handoff wave | Implemented internally: move-only receipts are canonically ordered by campaign job, exact-session lane, and event; duplicate or mis-scoped owners reject transactionally; one atomic byte tracks each leaf; and non-cloneable borrowed tickets have a hard live-count ceiling. This acknowledges handoff acceptance only. Exceptional-source re-entry, result-buffer admission, provider application, closure, and durable rule artifacts remain pending |
| Compile an acknowledged handoff into an algebra-free publication-epoch owner | Implemented internally: a fully acknowledged wave is consumed without copying algebraic payload; one event handle remains per slot, applicable and exceptional leaves become compact flat indices, and one atomic byte per exceptional source supports bounded retry-only leases with drop, panic-unwind, and explicit stranded-lease recovery. Applicable-provider admission/results, `CampaignWorkKey` result staging, fresh narrowed-domain mathematical re-entry/continuation, rule application, closure, and physical reductions remain pending |
| Chronologically replay committed generated-affine exact-session transitions | Available for Dependent, NoTarget, and affine-equality transitions in the implemented slices; compact application events are retained for the forward path, but their optional audit replay is pending |
| Process a concrete numerator through tensor projection and scalar lowering | Available through the direct library path |
| Track published LiteRed notebook semantic acceptance coverage | Eight LiteRed 1.x plus three LiteRed2 notebooks are inventoried at level 0; no translated notebook acceptance fixture or complete workflow passes yet |
| Automatically reduce every arbitrary integral to masters | **Not yet complete** |
| Complete generic guarded sector solving, `WhenBad` closure, and rule publication | **In progress** |

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
  --relations ordinary \
  --n-cores 1
```

For the current one-family `derive` command, `--n-cores N` is a positive,
invocation-wide worker budget. `N=1` is the inline deterministic serial oracle;
`N>1` requires a Symbolica license and creates one private local Rayon pool,
and the RustRed-owned scheduler neither reads nor configures Rayon's
process-global pool. Vendored restricted/unlicensed Symbolica itself currently
initializes a one-thread global fallback; the licensed production path does
not rely on it. Ordinary rows are
collected by fixed row ordinal, the complete ordinary phase precedes LI
construction, and selected relations are rendered through the same execution
context. Licensed `N=1`, `N=2`, and `N=4` runs are tested to produce
byte-identical TOML. `derive` does not schedule sectors or case lanes and does
not publish rule shards or bundles. The separate roots-only campaign planner
below accepts multiple topologies without claiming those later stages.

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

This `v1` label is an unstable implementation revision, not a backward-
compatibility promise; development versions may reject or replace it.

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

The CLI accepts three compact modes which normalize into the same validated
typed family representation:

- raw Symbolica `I(...)`, as above;
- hybrid TOML containing one compact `integral = """I(...)"""` plus metadata;
- fully explicit compact TOML for generated configurations.

Parameter declarations are optional when they can be inferred. They remain
useful as an explicit allowlist or for symbols used only by a retained target
numerator. Grammar, examples, output schema, exit codes, and atomic-I/O rules
are documented in [`docs/CLI.md`](docs/CLI.md).

## Roots-only campaign planning

[`examples/cli/campaign.toml`](examples/cli/campaign.toml) is a compact
multi-start input made of ordinary Symbolica expressions:

```toml
schema = "rustred.campaign-input.toml.v1"

[[roots]]
id = "tadpole-scalar"
integral = """
I(
  name(tadpole),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1)
)
"""
```

Build its deterministic static plan with:

```bash
cargo run --quiet --bin rustred -- campaign plan \
  --input examples/cli/campaign.toml \
  --output campaign.plan.toml
```

Each `integral` string goes unchanged through RustRed's existing Symbolica
compiler and affine-family lowering. Fully explicit existing
`rustred.project.toml.v1` schema and fields can be used under the per-root
`[roots.project]` prefix as well; there is no second expression parser. A raw
one-root convenience is
`rustred campaign plan --input-format symbolica --root-id NAME`.

The output interns identical exact family representations and identical
declared-power-sector jobs, and records canonical Symbolica expressions. It
labels power-sign classification as `declared_power_sector`, never as a
normalized target sector. It says `scope = "roots_only"` and marks target normalization,
dependency discovery, derivation, closure, and publication `not_started`.
Numerators are retained but are not tensor-reduced, scalar-lowered, or
cancelled against propagators. This command neither enumerates sectors nor
derives or applies IBPs, and roots-only output contains no dependency counts.
Accordingly it deliberately rejects `--n-cores` and `--max-memory`; those
budgets belong to the future heavyweight campaign executor.

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

Raw source generation also exposes the public `ParallelExecution` owner. A
library caller constructs it with `ParallelExecution::try_new(N)` and passes the
same value to `ParametricIbpGenerator::generate_with_execution`,
`generate_ordinary_ibp_with_execution`, or
`generate_lorentz_invariance_with_execution`. The existing methods without an
execution argument remain the serial library entry points. This bounded source
executor does not yet constitute the planned multi-topology campaign scheduler.

## Tested milestones

The repository currently includes tests for:

- generic ordinary IBP and LI generation with exact replay and identical
  relations through the serial API and `N=1`/`N=2`/`N=4` execution contexts;
- byte-identical `rustred derive` output at `N=1`/`N=2`/`N=4`, including a
  check that `RAYON_NUM_THREADS` cannot override the private execution context;
- deterministic roots-only `rustred campaign plan` output, raw Symbolica and
  mixed compact/nested-explicit multi-root ingress, exact family and declared-
  power-sector job interning, root-order independence, and strict
  rejection of execution resource flags;
- raw Symbolica, hybrid TOML, and explicit TOML input normalization;
- parameter inference without a required `parameters(...)` clause;
- affine denominator-basis completion for independent short lists;
- zero-sector, symmetry, guarded specialization, and sparse-elimination proof
  components;
- a live checked boundary over Symbolica's public incremental
  `SparseRowReducer`: the exact database builds a complete stage-local
  physical-key catalog, maps hardest keys to the lowest native columns, and
  rebuilds its immutable normalized pivots with `LuLMode::Full`. Symbolica
  authoritatively supplies the ordered pivot factors and dependent/independent
  outcome; RustRed replays that transcript through the guarded path as an
  independent provenance and differential check. Licensed default-GMP runs
  with four test threads passed 13/13 focused adapter tests, 39/39 focused exact
  database tests, and 2/2 direct-session tests;
- an exact-GMP, session-owned `Solvej` recentering transaction: authenticated
  post-top-reduction leaders are matched against persisted targets and return
  sealed NoTarget, affine-equality-refinement, or Ready outcomes;
- typed NoTarget commit and affine-equality suspension: NoTarget commits the
  algebraic pivot while preserving every target, whereas an equality-bearing
  first target commits the pivot and seals the old solve epoch for refinement;
- an opaque, session-owned recipe for the existing Dependent, NoTarget, and
  affine-equality transitions, with exact dependent-reduction or new-pivot
  evidence retained by shared ownership. Compact publication events instead
  retain only application data and deliberately drop the derivation recipe and
  pivot evidence;
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
  workers, including affected Ready/session regressions. The next foundation
  adds allocation-independent arbitrary-width OR-of-AND routing with stable
  three-valued provenance, plus a source-neutral affine-boundary kernel that
  constructs exact arbitrary-width boundaries, composes compact affine maps,
  and asks Symbolica for exact polynomial divisibility. Its focused licensed
  default-GMP tests passed 12/12 routing cases and 8/8 boundary cases, and that
  pre-partition checkpoint's complete parallel library suite passed 1091/1091.
  The subsequent owner-bound
  materialization phase now maps sources in schedule order, retaining the full
  schedule for a partition-ready outcome or the decisive prefix for an
  identically-bad outcome. It keeps distinct physical-parameter projections
  for the pre-normalization and normalized denominators and specializes
  admitted lazy hazards into exact boundary events with Symbolica-backed
  numerator classification. Its
  production-derived sector-011 acceptance owner has seven sources, four
  hazard ranges, and five ordered events: one suppressed by the numerator and
  four retained bad boundaries. Exact/one-below resource, retry-ownership,
  replay, foreign-owner, and global retained/peak accounting gates passed
  16/16 focused tests with default GMP and four Rust test threads. This phase
  consumes no target and publishes no rule. The following owner-bound phase
  now constructs the relative partition of its current-lineage arbitrary-width
  OR-of-AND bad formula, retaining every mapped-source/locus/formula occurrence
  and classifying applicable versus exceptional leaves without topology
  dispatch. The outer phase proves exact and Symbolica-associate canonicality
  once and seals the first-seen loci in an opaque, non-cloneable authority. The
  inner arbitrary-width compiler authenticates that authority and performs only
  linear payload validation. On both fresh compilation and full terminal
  replay, outer canonicalization runs exactly once; the nested compiler and its
  nested replay repeat no pairwise equality/associate scan and make no native
  associate call. Exact capacity,
  GMP-copy, duplicate-heavy replay, panic/retry, and aggregate resource
  ownership are bounded and tested. Licensed default-GMP focused Nextest run
  `b0217edc-a9e8-4a7d-9c5c-82b824a636b3` passed 19/19 tests with four workers;
  an independent superset passed 20/20. Authoritative licensed default-GMP
  Nextest run `e9004c32-5a51-4705-a2f9-e39bcac40c49` then ran 1,651 tests
  with four workers and passed all 1,651 (52 slow), with 5 additional
  configured cases skipped; the following doctest phase also passed. The next
  compact preparation step distills the sealed owner into commit state plus
  canonical loci, final relative cases, and a one-byte
  applicable/domain/leak tag per leaf. Operational failures return the exact
  move-only input; its licensed default-GMP preparation suite passed 3/3 tests
  with four Rust test threads. The following internal atomic commit advances
  the exact database, consumes exactly one selected target, and moves the
  centered relation terms, target locator/offset, loci, cases, and tags into one
  chronological event. Derivation-only row translation, guards, statistics,
  source recipe, and pivot evidence are not retained by that event. The frozen
  post-commit licensed default-GMP gate used four Nextest workers and passed
  all 1,658 runnable tests, with 5 configured cases skipped; the following
  doctest phase also passed. Subsequent focused slices added one shallow event
  `Arc` plus zero-copy rule/residual projections and retained the already-proved
  pivot-term ordinal and immutable target domain/geometry, then compiled those
  event owners into a sealed handoff wave. Its licensed default-GMP gate passed
  10/10 focused tests, including bounded out-of-order 1/2/4-worker resolution
  with identical full semantic transcripts and exact/one-below transactional
  memory limits. The handoff keeps one event handle per slot and one atomic byte
  per leaf; a live-ticket ceiling is independent of `--n-cores`. The following
  algebra-free publication-epoch owner consumes only a fully acknowledged
  wave, retains that single event handle per slot, replaces the obsolete
  handoff state by compact applicable/exceptional flat-leaf indices, and keeps
  one atomic byte per exceptional source. Its bounded retry-only leases return
  to pending on normal drop or unwind; explicit quiescent recovery handles a
  deliberately forgotten lease. Its checked component gates cover transferred
  event payload, retained shallow buffers, compilation peak, and live lease
  bytes, not process RSS. Its licensed default-GMP validation passed 6/6
  focused epoch-owner tests and the 16/16 parent handoff-module superset with
  four test threads; `cargo check --tests -j4` also passed. It does not admit
  or retain applicable-provider results, stage results under a stable
  `CampaignWorkKey`, construct fresh
  narrowed-domain mathematical epochs or rejected-candidate continuation,
  apply rules, prove closure, reduce a physical topology, or solve six loops;
  and
- a static, topology-neutral multi-root campaign core in
  [`src/campaign_plan.rs`](src/campaign_plan.rs): exact-representation family
  and job interning, distinct ingress roots, replayable strict
  proper-subsector dependencies, and deterministic dependency-ready
  antichains. Its companion resource selector computes checked stable
  first-fit candidate waves without constructing heavy task owners; a
  synthetic 100-job/100-core/1-TiB test admits 57 jobs and intentionally leaves
  cores idle under RAM pressure. The roots-only campaign CLI now authenticates
  declared user ingress through this static plan. A separate move-only atomic
  controller revalidates a selected wave and charges its cores, retained
  successors, transient memory, and old/new overlap with panic-safe release
  guards. Its stable bounded executor now settles move-only tasks and performs
  a genuine whole-session Symbolica dependent-row transition through a
  canonical post-worker commit barrier. This remains a low-level seam, not a
  calibrated production frontier coordinator or campaign execution CLI; no
  complete derivation scheduler, closure proof, checkpoint, or rule bundle is
  claimed; and
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
families with the common mass set to one. The target architecture has two
stages: an offline, topology-generic foundry will derive and verify guarded
parametric rules for canonical families/sectors; a separate online runtime
applies the compiled artifacts in batches to the many concrete numerator
integrals produced by GammaLoop.  See the
[six-loop single-scale vacuum priority](docs/research/six_loop_single_scale_vacuum_priority_2026-08-24.md).

The planned foundry output will be a multi-start campaign bundle, not one
flattened rule table and not a serialized `PreparedPublication`. Each fully
coverage-closed canonical `(family, sector, ordering, domain)` job becomes one
immutable rule shard. User-supplied starting topologies retain small verified
ingress maps,
while routing-equivalent roots and shared subsector, factorization, and
cross-family dependencies reuse the same shards through a strict dependency
DAG. Incomplete or resource-limited derivations remain resumable workspaces
and cannot be loaded as closed reduction bundles. Topology/family metadata is
valid routing and inspection data; it must never select a hardcoded algorithm
or recurrence.

The planned parallel derivation will use a deterministic campaign DAG, not an
arrival-ordered pool of equations. Generic IBP/LI sources will be generated
once per canonical family; unique sectors will derive intrinsic rules
concurrently while leaving proper-subsector terms on the right-hand side; ready
subsector and factorization antichains will then close bottom-up. One affine
case lane will own one serial retained Symbolica reducer, while independent
families and sectors, frozen-epoch exceptional case proposals, fixed modular
samples, and exact-verification blocks may run concurrently. Case-lane-local
reducer controllers will avoid shared coefficient-ledger serialization. The
single `--n-cores N` budget covers the complete invocation rather than granting
`N` workers to every root or lane; inner parallel work must borrow from that
same lease. Stable work keys, sorted frontier barriers, ordered
reclassification/merges, strict-descendant closure epochs, and separate
scheduler memory admission must make the `N=1` serial oracle and 2/4-core runs
produce identical semantic hashes and mathematical output.
For the intended roughly 100-core, 1-TiB EPYC six-loop runs, readiness will
never mean eager fork-all: a bounded deterministic wave must acquire both core
leases and conservative memory permits before any reducer clone or other
heavyweight task owner is constructed. The configured RAM ceiling reserves
headroom for Symbolica's opaque scratch and the operating system. Future
campaign execution will choose an effective execution width `E <= N` before
pool construction, charge the coordinator plus every possible worker's warmed
TLS/Workspace reserve, and leave cores idle under memory pressure. If even the
inline `E=1` baseline plus one minimum task cannot fit, it will return a typed
memory-capacity pause without creating a pool. Requested/effective widths and
the estimator revision are physical run metadata, not mathematical identity.
The current raw `derive --n-cores N` path is distinct: for `N > 1` it constructs
the requested `N` workers directly and does not yet derive a memory-limited
effective width.
See the
[parallel campaign foundry plan](docs/research/parallel_campaign_foundry_design_2026-08-26.md).

The optimized online runtime is not the next scalability claim. RustRed must
first close the topology-neutral foundry, derive complete replacement systems
for the families and target domains exercised by Vakint's one- through four-
loop single-scale vacuum corpus, and compare normalized reductions against
Vakint without invoking FORM or copying its authored rules. Derivation-only
physical five-/six-loop gates then follow. The six-loop topology and numerical
resource manifest is frozen before execution and includes representative QCD-
valid connected 1PI quartic and cubic 21-coordinate vacuum roots. Each reaches
all 36 IBP sources, closes every reachable exceptional and lower-family
dependency onto a finite selected or independently certified terminal set,
verifies every rule by an exact regenerated-IBP residual, and meets predeclared
wall-time, memory, and parallel-scaling thresholds on named hardware. The
existing synthetic `K=21` first-residual fixture is only a generator/frontier
stress test and does not satisfy that gate.

The transactional exact pivot database, hardest-first top reduction,
session-owned exact recentering, typed NoTarget commit, sealed affine-equality
suspension, private chronological event ledger, and shadow replay are
implemented for the generic generated-affine exact-session slice. Dependent,
NoTarget, and affine-equality events own opaque source recipes and exact
evidence. A compact application event instead keeps only the data needed by
future application: centered relation terms, target locator/offset, canonical
loci, final relative cases, and one-byte leaf tags.

That correctness-first storage layout is not the final high-loop layout. Each
current event append copies the preceding event-`Arc` vector, and each target
successor copies the complete disposition vector. Before the multi-loop
foundry is scaled, these paths must become a chunked/persistent event log and a
shared or paged copy-on-write target state. The compact application event now
retains its application payload once, and its rule/residual projections are
shallow views rather than duplicate deep payload vectors. The repeatable event
views themselves do not claim exactly-once consumption; the following sealed
handoff wave now provides exactly-once acceptance tickets without copying that
payload. A subsequent algebra-free owner consumes a quiescent, fully
acknowledged wave and offers bounded retry-only exceptional-source leases over
compact locators. Those leases schedule access to existing event-bound data;
they do not yet create the fresh narrowed-domain database/reducer, carry the
monotone candidate-exclusion continuation witness, admit a result, or close an
exceptional domain.

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
custom `exact_sparse_elimination` and `parametric_elimination`. The live
generated-affine exact database has now crossed the first sparse boundary. For
each stage it builds the complete physical-key catalog, rebuilds immutable
normalized pivots plus one unused sentinel column, and calls public
`SparseRowReducer` with `LuLMode::Full`. Symbolica is authoritative for ordered
pivot factors, normalization, and the dependent/independent decision. RustRed
then replays the returned transcript through the existing guarded arithmetic
to authenticate provenance and differentially verify the native result. The
boundary borrows authenticated coefficient rows, uses shallow `Arc` clones for
Symbolica's dense scratch, meters native coefficient work separately, and
turns unused `Field` callbacks into typed failures. Licensed default-GMP runs
with four test threads passed 13/13 adapter, 39/39 exact-database, and 2/2
direct-session tests.

The checked-field ownership prerequisite for retaining that reducer is now
implemented. `CheckedParametricField` owns an
`Arc<ParametricCoefficientContext>` and its clones share a `Send + Sync`
controller with one serialized, per-stage coefficient-work ledger. An RAII
stage guard clears the active ledger after success, a typed checked-field
abort, or an unrelated unwind panic, so a retry always starts with fresh work
limits and counters. Five focused tests cover owned context lifetime and
`Send + Sync`, inactive callbacks, serialized sibling clones, unknown-panic
recovery, and deterministic typed-abort/retry cleanup. This makes independent
reducers safe to schedule across campaign shards; it does not parallelize the
ordered forward pass inside one reducer.

The private retained adapter is now implemented. It owns one already-admitted
context `Arc`, a permanent final sentinel, and a `SparseRowReducer` in Full-L
mode. Each stage clones the committed native state, inserts complete-catalog
columns in Symbolica's old-coordinate convention, submits one row, and returns
a successor only for an independent outcome. Dependent, empty, rejected, and
failed trials are discarded. Exact post-native validation authenticates the
entire historical U/L/pivot prefix, duplicate insertion remaps, sentinel,
normalization, and resource-failure ordering. An insertion-aware differential
sequence and focused corruption/resource tests pass 15/15; the existing sparse
bridge passes 18/18 and the exact database passes 39/39 with four test threads.

The live database still uses the reconstructing correctness bridge until the
retained reducer and complete easiest-first catalog are integrated into its
transaction. `forward_reduce_last_row` therefore still has a cumulative
`O(P^2)` tendency, while Symbolica's reducer owns an `O(K)` dense scratch and
performs serial forward reduction. Clone/add-column allocation is infallible in
the public Symbolica API and cannot honestly be reported as a typed recoverable
OOM. Fixed-size telemetry remains outside replay identity and excludes catalog
sorting, wall time, and RSS. None of this yet establishes complete topology
reduction, Vakint reproduction, or physical six-loop scalability.

The next vacuum-critical solver milestone is the topology-neutral
`compact application event -> native Symbolica incremental reducer -> owning
residual/subsector scheduling -> exactly verified closure` slice. Exact fixed-chamber
descent and lazy hazard geometry now cover authenticated selector-independent
compact affine target maps. An owner-bound identity/compact transform and
deterministic condition-source schedule now preserve the Ready lineage, while
Symbolica projects physical-parameter identities into exact index-polynomial
loci. The
source-neutral core now also routes arbitrary-width direct formulas and
specializes lazy affine boundaries by exact Symbolica polynomial divisibility.
The owner-bound non-publishing materializer binds those kernels into a mapped-
condition transcript, including both denominator-identity projections and the
ordered boundary events. The subsequent move-only owner now proves and seals
canonical loci once per fresh compilation or full proof replay, then builds the
relative `WhenBad` partition for the assembled current-lineage arbitrary-width
OR-of-AND formula without a second inner Symbolica-associate scan. Compact
preparation now distills the application loci, cases, and one-byte leaf tags;
the atomic session transition advances the database, consumes one selected
target, and stores those values with the centered relation terms and target
locator/offset in one event. It drops derivation-only translation, guards,
statistics, source recipe, and pivot evidence. Shallow applicable and
exceptional projections are implemented. The active path must retain and fork
Symbolica's already-authoritative incremental reducer instead of rebuilding it
from prior pivots at every stage, add the topology-neutral shared-job campaign
plan, then schedule residual and solved-subsector work and prove a coverage
fixed point. Multi-start shard/bundle
compilation and physical six-loop derivation profiling follow before the
optimized concrete batch-application runtime. The retained `SparseRowReducer`
transaction is now the immediate algebra-scaling gate; graph-lifted
symmetry discovery and unrelated Feynman/non-vacuum algebra
migrations stay required but no longer displace the six-loop vacuum critical
path.

The first of two measured architectural scaling blockers in this seam is now
closed. Previously, the outer first-seen locus interner and defensive raw-
problem validator each performed an all-pairs Symbolica associate scan. The
new authenticated canonical-locus owner lets the trusted nested path skip that
duplicate `O(N^2)` CAS work while retaining the raw validator as a defensive/
test entry path. A future censused use of Symbolica's exact monic `K[n]`
normalization could index the remaining outer interning scan, but its current
normalization API has no fallible workspace census. The remaining measured
blocker is that Symbolica's rational-polynomial division and projected
`try_div` APIs expose no pre-allocation GCD/quotient workspace bound.
The resource-bounded arbitrary path therefore performs complete exact
splitting without divisibility-based pruning; the public V1 compatibility path
retains its historical behavior. Restore that optimization only through a
censused Symbolica seam and a bounded ordinal-pair index.

Alongside that staged algebra migration, historical pushed checkpoint
`c593865` binds one explicit `IntegralOrderingPolicy` into every normalized
source, including an empty-attempt source, and authenticates every present
candidate's policy. Subsequent current-lineage work additionally carries an
`Actionable` Direct singleton from
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
projection supplies the exact physical-parameter identity clauses. Generic
arbitrary-width formula routing and exact affine-boundary specialization are
implemented as non-publishing kernels. The owner-bound materializer maps the
scheduled payload, retains both denominator projections, and specializes exact
boundary events without consuming a target or publishing a rule. The
remaining generic LiteRed-style foundry work is to:

1. integrate the tested private clone-on-stage retained `SparseRowReducer` and
   complete easiest-first column catalog into the live exact database, expose
   its telemetry to physical campaign profiles, and retain exact regenerated-
   residual checks around the Symbolica transcript authority;
2. integrate the implemented topology-neutral plan, resource admission, and
   low-level bounded executor into the production frontier coordinator; keep
   plan/results identical under root permutation and 1/2/4-worker execution,
   and retain one serial Symbolica reducer per independently schedulable affine
   case lane;
3. build on the implemented algebra-free publication-epoch owner by adding
   independently RAM-admitted applicable-provider work/results, a stable
   `CampaignWorkKey` result table with atomic result-charge transfer, sealed
   exceptional ingress into fresh narrowed-domain database/reducer epochs, and
   separate same-database rejected-candidate continuation;
4. feed solved subsectors into supersectors, iterate residual cases, and prove
   closure onto a finite enumerated set of selected or independently certified
   terminal keys (or finite products), never a symbolic residual domain;
5. extend that plan with verified routing/dependencies and compile closed
   family/sector shards into deterministic multi-start campaign bundles;
6. pass the complete Vakint one- through four-loop replacement-system lane and
   derivation-only physical five-/six-loop scalability gates; and
7. implement optimized application-time specialization plus optional
   publication audit replay and expose a coverage-backed reduction result.

This current-lineage state does not complete RustRed's stated mathematical
capability goal, `WhenBad` closure, rule publication, an arbitrary one-loop
pentagon reduction, or the two- through six-loop reduction campaign. Concrete
multi-loop families
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
listed above passed. Subsequent checkpoints add an allocation-independent
stable-value identity to the coordinate-affine terminal and reach the source-
profiled exact session through Direct solve-plan V2, ordering V3, and physical frame V2,
with exact `Arc` ancestry kept separate and no fake inventory. Authenticated
selector-independent compact affine maps reach the unpublished
`ReadyForConditions` gate, and the new owner-bound plan schedules its exact
condition sources without consuming the target. Symbolica-backed
physical-parameter identity projection, arbitrary-width direct-formula
routing, and the exact affine-boundary mapping/divisibility kernel are also
implemented. The current owner-bound materializer maps the scheduled payload,
retains both denominator projections, and specializes exact boundary events
without consuming a target or publishing a rule. A separate opaque move-only
owner now proves the canonical parametric loci once per fresh compilation or
full proof replay; the nested relative-`WhenBad` compiler validates that
authority linearly and performs no duplicate pairwise Symbolica-associate scan.
Compact route preparation is now a single move-only owner transition rather
than a separately authenticated manifest, and the atomic compact-event commit
now consumes one selected target. Its shallow event owner exposes applicable
and exceptional views without copying the row or partition. The complete
physical-key catalog and differential shadow now make the tested Symbolica
adapter authoritative in the exact database. Its deterministic committed
native-sparse snapshot is a crate-private campaign seam; wall time and RSS stay
outside algebraic state. The static shared-child campaign plan, stateless
core-plus-memory wave selection, roots-only declaration CLI, and atomic
move-only admission controller are now implemented. A first stable indexed
executor and resident-transform seam now move a complete exact session through
a genuine generated-row Symbolica dependent transition while retaining old,
successor, and transient charges. This is not yet the campaign CLI runtime:
the immediate gates are a calibrated physical estimator with requested-versus-
effective execution-width selection, the full frontier coordinator, then RAM-
admitted mathematical exceptional/subsector ingress and result scheduling on
top of the implemented algebra-free epoch owner, followed by a proved coverage
fixed point.
Closed-shard campaign bundling and a physical six-loop derivation gate
precede optimized application and optional publication audit replay. The
`rustred derive` command remains a raw parametric-IBP/LI generator and does not
run this closure path or emit native-sparse telemetry. No arity-21 case has
reached Ready, no coverage-closed durable guarded-rule shard has been
published, and the current generated-affine exact-session lineage has not
closed or reduced a complete physical family.

Further capability coverage informed by LiteRed2 includes broader symmetry
discovery, partial fractions for dependent or overcomplete propagator lists,
master inference, persistent proof serialization, dimension shifts, and
differential equations.

## Testing

Run the licensed test suite in parallel with the bounded default of four jobs:

```bash
export SYMBOLICA_LICENSE='your-symbolica-license'
./scripts/test.sh
```

Override concurrency with `RUSTRED_TEST_JOBS`. When `cargo-nextest` is
available, the script runs test binaries concurrently; it otherwise keeps
Cargo's parallel test workers. This test-runner setting is deliberately
separate from the RustRed execution contract `--n-cores`; it does not configure
campaign concurrency. No test path enables `no_gmp`.

## Documentation

- [CLI contract and input formats](docs/CLI.md)
- [RustRed scope and acceptance criteria](docs/research/rustred_scope_and_acceptance.md)
- [Six-loop single-scale vacuum priority](docs/research/six_loop_single_scale_vacuum_priority_2026-08-24.md)
- [Deterministic parallel campaign foundry](docs/research/parallel_campaign_foundry_design_2026-08-26.md)
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
