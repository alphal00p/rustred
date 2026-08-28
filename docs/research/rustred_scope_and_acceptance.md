# RustRed governing scope and acceptance criteria

Date: 2026-08-13. Reconciled with the LiteRed/Symbolica/Vakint source audits
on 2026-08-20 and reprioritized for the six-loop single-scale vacuum campaign
on 2026-08-24. Implementation status was reconciled with pushed checkpoint
`c593865` and the Direct singleton stable-identity/exact-session Ready-ingress
checkpoint on 2026-08-25. Owner-bound non-publishing mapped-condition
materialization, its production-derived sector-011 acceptance, and the
subsequent current-lineage relative-partition slice were reconciled on
2026-08-26. The relative-partition gate passed 8/8; the subsequent
canonical-locus owner gate passed 19/19, with an independent 20/20 superset.
Authoritative licensed default-GMP Nextest run
`e9004c32-5a51-4705-a2f9-e39bcac40c49` used four workers, ran 1,651 tests,
and passed all 1,651 (52 slow), with 5 additional configured cases skipped;
the following doctest phase also passed. Compact publication preparation and
the internal atomic application-event commit were reconciled on 2026-08-26.
The frozen licensed default-GMP gate then passed all 1,658 runnable tests with
four Nextest workers, with 5 configured cases skipped; doctests also passed.
The owning queue, scheduling, closure, bundle, application, audit-replay, and
complete-reduction gates remain open. Shallow event-bound rule/residual domain
views are implemented in the subsequent worktree slice.

The 2026-08-27 private source-neutral mapped-`NonZero` worker adds
complete-stream mapping/output preflight, simultaneous Symbolica substitution,
first-zero diagnostics, constant discharge, corrected `Q*`/`Q(theta)*` locus
routing, and parent peak accounting for replay, allocator, GMP-output, and
accumulator temporary memory, including the mapped-zero diagnostic overlap.
Native Symbolica integer work is budgeted separately from total
collection/conversion work throughout every aggregate composition caller. It
is not source-bound or proof authority yet.
The mandatory current gate is the repository-wide structural reorganization:
generic production code, loop-specific validation campaigns, fixtures, and
current documentation must become visibly separate before resident equality
integration resumes. Its typed transport-neutral application layer and PyO3
package over exactly the operations used by the CLI are now complete for
development use. The binding is not a second algebra or reduction
implementation; portable publication remains a separate licensing and
manylinux gate.

## Reading status

This document is normative: its feature lists and validation ladder define
governing acceptance criteria, not evidence that those features are already
implemented or validated.  A module name, test fixture, or concrete oracle
does not establish parity by itself.  Current source coverage and known gaps
are tracked separately in
[`rustred_source_surface_gap_audit_2026-08-14.md`](rustred_source_surface_gap_audit_2026-08-14.md).

## Governing objective

RustRed is a pure-Rust, fully Symbolica implementation of the generic
mathematical capabilities exemplified by LiteRed2.  It must not be a
collection of built-in reductions for particular loop counts or topologies.
The vendored `vendor/LiteRed2` source is a conceptual guide, capability map,
and source of independently checked conventions and acceptance cases; it is
not a bug-for-bug, source-level, architecture, pivot-order, or Mathematica-API
compatibility contract. RustRed should choose better typed algorithms and data
structures whenever they improve generality, auditability, parallelism, or
runtime efficiency without changing the explicitly accepted mathematical
semantics.

Internal forward-path values should use sealed constructors, move ownership,
and typestate rather than repeated schema/fingerprint/replay authentication.
Runtime authentication is for untrusted imports, durable artifacts, and live
mutation boundaries. Backward compatibility of internal formats is explicitly
not an acceptance requirement during this implementation stage. More broadly,
pre-release RustRed APIs, CLI/Python details, and artifact schemas may be
replaced without compatibility shims; their callers and fixtures move in the
same milestone. Vakint's established user-facing behavior remains backward
compatible when its additive RustRed mode is introduced.
Older internal schema/replay/token layers are historical scaffolding, not a
design precedent; simplify or remove them when their code is next changed.

Throughout this document, “parity” means semantic agreement on the named
acceptance surface after an explicit convention map. It does not require
identical intermediate rules, traversal order, mutable global state, or
implementation quirks.

RustRed also owns tensor-numerator reduction and the application of discovered
scalar reduction rules.  The behavioral reference for that layer is Vakint
and its alphaLoop integration under `vendor/gammaloop/crates/vakint`.  FORM
sources may be read to understand the algorithms and conventions, but RustRed
must never invoke or depend on FORM.  The implementation is Rust plus the
vendored Symbolica Rust API.

RustRed must expose its application services through both the CLI and Python.
Those frontends share owned request/result types, typed errors, resource
limits, deterministic serialization, and the `n_cores` execution contract.
Python-specific types, callbacks, and GIL state remain outside the core, and
the final dependency graph must retain Symbolica's GMP backend without
`no_gmp`. The detailed binding contract is in
[`python_api_directive_2026-08-27.md`](python_api_directive_2026-08-27.md).

The immediate deployment priority is highly efficient reduction of up to
six-loop, single-scale massive vacuum graphs produced after GammaLoop's
general BPHZ R-operation.  This priority does not permit loop-count or topology
dispatch in production.  It changes the order of implementation: reusable
vacuum-family/sector rule derivation and a separate batched rule-application
runtime take precedence over non-vacuum examples and broad Feynman-parametric
polishing. The foundry must prove closure, produce reusable multi-start
campaign bundles, and pass derivation-only physical six-loop scalability gates
before optimized online application becomes the priority. The detailed
two-stage architecture and benchmark contract are in
[`six_loop_single_scale_vacuum_priority_2026-08-24.md`](six_loop_single_scale_vacuum_priority_2026-08-24.md).

## Source-of-truth order

1. LiteRed2's vendored
   [`LiteRed2026.m`](../../vendor/LiteRed2/Source/LiteRed2026.m) source maps the
   intended capability envelope and supplies mathematical conventions and
   reference cases for basis construction, scalar-product conversion, fully
   parametric IBP and LI relations, sectors, symmetries, guarded recurrence
   discovery, rule application, masters, and persistence. Its implementation
   accidents are not authoritative.
2. Symbolica's vendored Rust source defines the exact algebra, patterns,
   substitutions, polynomial/rational-polynomial operations, serialization,
   and performance facilities available to the implementation.
3. Vakint's Rust tests and tensor FORM routines define tensor input/output
   conventions, tensor projection behavior, and topology normalization.
   Vakint/alphaLoop's authored FORM recurrence tables are frozen concrete
   output oracles only: they must never supply RustRed source rows, weights, or
   production replacement rules.

No new production implementation begins until the relevant source paths have
been audited and their semantics recorded with exact references.

## Production versus validation

Production APIs must be topology- and loop-count independent.  Their inputs
may include family definitions, loop and external momenta, kinematic
relations, denominators, power shifts, cuts, sector policies, orderings,
resource limits, and parameter assumptions.  They must not include expected
ranks, expected masters, preselected recurrence weights, golden reduction
coefficients, or dispatch on a named built-in topology.

Concrete topologies and concrete integer powers are permitted only for:

- exact specialization tests of parametric identities and rules;
- randomized or exhaustive finite-point validation;
- regression fixtures;
- comparisons against LiteRed examples or Vakint outputs; and
- performance benchmarks.

During development, cached rules are disposable revision-tagged artifacts.
They record explicit family/kinematics/order conventions, domains, guards,
routes, and dependencies. Exact regenerated-IBP validation is mandatory when a
bundle is finalized as `Closed`, for explicit `verify --exact`, and on first
trust of an external artifact. Ordinary loading of a locally finalized artifact
uses lightweight schema/revision, convention, format-local checksum, and DAG
structural checks and may reuse a checksum- and revision-bound exact-
verification receipt. Full source provenance and chronological replay remain
optional audit payloads. No migration promise applies until an external
artifact format is intentionally declared stable. Every cached rule must remain
reproducibly derivable from the generic engine; signatures are optional.

`PreparedPublication` is an internal move-owned live-session value, not the
durable format and not a complete rule set. The implemented static
`CampaignPlan` core is likewise not a durable rule artifact: it is a
topology-neutral scheduling value
containing roots, exact job identities, dependencies, and deterministic ready-
job antichains, with no rules and no `Closed` claim. This first slice uses exact
family-representation identity and identity ingress, shares witnessed strict
proper-subsector children, and rejects non-descending edges. Cross-family edges,
dependency discovery, execution state, closure, and durable artifacts remain
future work.

A durable multi-start campaign contains immutable coverage-closed shards keyed
by the topology-neutral `(convention, family, sector, ordering, coefficient
specialization, declared domain, terminal policy)` job identity; verified
ingress maps connect every user root, and a strict DAG shares proper-subsector,
factorization, and cross-family dependencies. Incomplete, unsupported,
resource-limited, or interrupted derivations belong to a separate resumable
workspace type and cannot be opened as a closed reduction bundle. Shards are
written independently and the lightweight root manifest is installed last.
The cross-root canonical family ID is constructed only after verified routing,
denominator-order, and parameter canonicalization and excludes user/root names
and momentum-label aliases; the existing label-sensitive family fingerprint
remains a representation/session identity. Same-rank family maps become ingress
aliases before DAG construction, while cross-family dependency edges require a
strict well-founded rank decrease. Each closed shard retains a compact sparse
source-combination/residual witness for exact checking against freshly
regenerated generic IBPs. Rules composed with solved children additionally
retain the strict dependency/transport path and recursively replay the child's
source witness (or an equivalent flattened exact combination); full
chronological transcripts remain optional.

Campaign merge is deterministic and transactional. Equal job keys with equal
payloads deduplicate; equal keys with different payloads conflict, as does a
reused root ID with a different ingress map. A shared child is stored once with
multiple incoming edges. Incompatible conventions or coefficient contexts
remain distinct unless an exact transport is verified; same-rank equivalences
remain ingress aliases. Cycles, non-descending dependencies, and incomplete
shards reject a proposed closed-manifest update.

Exact `Closed` admission reconstructs every family/context, verifies ingress
and dependency maps, regenerates generic IBP/LI sources and proves every rule
residual exactly zero in Symbolica, proves strict RHS/dependency descent, and
proves complete declared-domain routing to rules or a finite selected/certified
terminal set (including finite products). Cycles and unresolved routes are
rejected. It runs at finalization, explicit `verify --exact`, or first external
trust; a later ordinary local load may reuse the checksum- and revision-bound
receipt after lightweight structural checks. One-worker and multi-worker
evaluation must agree semantically; full derivation replay is optional.

## Required scalar scope

The completed scalar implementation covers at least the corresponding
LiteRed facilities:

- denominator sets and independent bases, including optional ISP completion;
- loop and external momenta and their scalar-product basis;
- Symbolica rational-function kinematics and external Gram relations;
- conversion between scalar products and denominator powers;
- cuts, sector patterns, and power shifts;
- fully parametric ordinary IBPs and separate LI relations;
- AB shift-operator conversion and exact round-trip semantics;
- partial fractioning for overcomplete denominator sets;
- integral and sector ordering;
- zero-sector analysis;
- internal and external symmetry discovery and sector mappings;
- adaptive guarded parametric rule discovery from generated identities;
- exact rule provenance, exceptional domains, and termination/descent checks;
- rule application, master candidate handling, and user-selected masters;
- inspectable persistence with one-time load validation and recovery;
- Feynman-parametric and dimension-shift facilities where LiteRed exposes
  them; and
- no implicit promotion of an uncovered integral to a proved master.

The first generator milestone is semantic identity-generation agreement with
LiteRed2 `GenerateIBP`:
for `L` loops and `E` external momenta it emits `L*(L+E)` ordinary parametric
relations, plus `E*(E-1)/2` LI relations.  RustRed's contraction-major order is
a documented deterministic convention for reproducible artifacts and oracle
normalization, not a requirement to mirror LiteRed2's internal enumeration. It
supports symbolic power shifts, never applies sector/symmetry rules during raw
generation, and specializes exactly to an independent concrete generator.
LiteRed's later `GenerateFPIBP` syzygy facility is a distinct eventual scope
item; it is not another spelling of these symbolic-index IBP/LI rows and is not
part of the first solver milestone.

## Required tensor and application scope

RustRed natively parses or receives tensor numerator structures, contracts
metrics, projects loop tensors, rewrites scalar products in the selected
family, applies the generated scalar rules, and returns coefficients times
unsubstituted master topologies.  The implementation must cover the tensor
structures and conventions exercised throughout Vakint's tests and alphaLoop
paths.  It may use a different Symbolica-native algorithm, but comparisons
must be structural and exact after a documented convention map.

Vakint's `TensorReduce` is specifically an oracle for a vacuum subgraph: it
retains numerator-only external vectors and opaque factors as spectators.
That does not by itself validate the more general covariant decomposition for
a denominator family containing external momenta; RustRed must implement and
test that case independently.

## Validation ladder

Validation has a derivation/foundry lane followed by an application/oracle
lane. The latter must not conceal that the former is incomplete.

The ordered foundry lane is:

1. synthetic generic family and exact-algebra properties;
2. coverage-closed one- through three-loop family/sector shards with exact
   regenerated-source residuals, exceptional recursion, solved-subsector
   feedback, and a finite explicitly enumerated terminal-key set (or finite
   products) whose members are user-selected or independently certified;
3. complete derived replacement systems for Vakint's one- through four-loop
   single-scale vacuum family corpus, without FORM or copied authored rules;
4. deterministic multi-start bundles proving routing aliases, shared
   subsectors/factorizations, strict dependency descent, incremental reuse,
   and equivalent one-worker/multi-worker semantics;
5. representative general five-loop families, including ISP-rich and
   duplicate-denominator cases rather than only the banana;
6. a pre-run-frozen, structurally representative QCD-valid six-loop corpus
   spanning quartic and cubic 21-coordinate vacuum roots, with every reachable
   dependency closed under predeclared numerical time, memory, artifact, and
   parallel-scaling thresholds; then a small GammaLoop/BPHZ-derived corpus.

Only a closed shard or bundle may satisfy the application/oracle acceptance
lane. Existing direct-library fixtures remain useful regressions but do not
pass this lane:

1. scalar and varied tensor/numerator inputs from one loop upward, with masters
   left unsubstituted;
2. representation-closure checks in which a numerator factor equal to a
   propagator (for example `q_i^2-m_i^2`) is compared exactly with the same
   input after explicit propagator-power cancellation, before IBP and again on
   the unreplaced-master result and semantic guard loci;
3. exact normalized reductions against Vakint through its four-loop support,
   using Vakint only as an external behavioral oracle, comparing the final
   expression over unsubstituted master/terminal symbols and its semantic guard
   domain after the convention map—not the identity of authored FORM
   recurrences, pivot order, or intermediate rules; and
4. held-out routings, loop bases, numerator shells, primes, and specializations
   beyond that oracle through five and six loops.

At every rung, accepted parametric rules must replay symbolically from freshly
generated source relations.  Agreement at finitely many concrete powers is an
independent validation layer, not the proof of a rule.

## Runtime and build constraints

- RustRed never invokes Mathematica or FORM.
- Symbolica is built with GMP, not `no_gmp`.
- Tests use the configured Symbolica license and run in parallel.
- Every caller-controlled search or algebraic expansion has an explicit,
  checked resource budget.
- Exceptional parameter or index loci are preserved as typed guards rather
  than silently assumed away.
- Exact dense and sparse linear solves use public Symbolica APIs wherever they
  apply; older custom `exact_sparse_elimination` code is migration debt, not a
  production algebra authority. Because the pinned sparse solve has a known
  validation caveat, RustRed uses a checked borrowed-input adapter over public
  `SparseRowReducer` with independent transcript checks rather than
  implementing another CAS or matrix package. The live generated-affine exact
  database owns a complete easiest-first physical-key catalog and a clone-on-
  stage `SparseRowReducer` under `LuLMode::Full`. Each trial extends only its
  ordered native columns, and only an independent outcome carries a move-owned
  reducer/catalog successor into commit; empty, dependent, rejected, and failed
  trials leave the committed owner unchanged. Guarded Rust replay authenticates
  Symbolica's factors and divisor, the complete historical U/L/pivot prefix,
  and the appended normalized `U` row coefficient-for-coefficient. The rebuild-
  every-stage exact-database glue/use is a `cfg(test)` oracle; the generic legacy
  adapter remains compiled outside the live path. Licensed default-GMP runs
  with four test threads pass 15/15 retained-reducer, 18/18 complete sparse-
  adapter, and 41/41 exact-database tests.

This live boundary is a correctness milestone, not a closure or scaling
claim. Every stage still makes a full native reducer clone, forward elimination
is serial, and Symbolica's native allocation and scratch bytes remain opaque.
Fixed-size committed telemetry records entry/fill and coefficient-work counts
but is not an authoritative native-byte, wall-time, or RSS census. No complete
physical-topology reduction, Vakint one- through four-loop reproduction, or
physical six-loop scalability result has been demonstrated.

## Current implementation gate and next generic slice

The raw generic generator is real rather than a recurrence table: licensed
GMP run `5ae578f9-5bff-4cf9-bf3f-7013730923ee` passed 20/20 parallel tests
covering one- through five-loop generated IBP/LI identities, symbolic power
shifts and external-momentum families, the Symbolica tensor-numerator
boundary, and FORM-free one-loop Vakint scalar/tensor oracles.  Those concrete
families validate generation; they do not establish complete LiteRed
`SolvejSector` parity.

Multi-topology derivation will use the
[deterministic parallel campaign foundry](parallel_campaign_foundry_design_2026-08-26.md).
The semantic unit is a canonical family/sector/domain job, not a loop-count or
topology-specific reducer. Intrinsic sector derivations are independent in the
LiteRed2 sense and may leave proper-subsector integrals unresolved; shared
subsector/factorization jobs then close bottom-up. Every concurrently active
affine case lane owns one serial Symbolica reducer/controller and runs only as
a staged proposal from a frozen coverage epoch unless independence is proved.
Campaign acceptance therefore includes invariance under root order,
idempotence of one repeated `RootId`/payload, distinct ingress rows for distinct
routing-alias root IDs with one shared job DAG, randomized task delay, and
1/2/4 workers, plus proof that a shared child was derived once.

Published LiteRed examples are now a separate, explicit acceptance lane. The
eight LiteRed 1.x notebooks and three LiteRed2 example notebooks are inventoried
at level 0 and will be tracked
from input normalization through identity, sector/symmetry, parametric-rule,
target-reduction, and auxiliary-recurrence parity.  Current ingredient tests
do not count as complete notebook passes, and no translated notebook acceptance
fixture is checked in yet. The initial inventory and honest status baseline are
recorded in
[`litered_examples_acceptance_matrix.md`](litered_examples_acceptance_matrix.md).

The post-cylindrical-elimination library checkpoint is licensed GMP nextest
run `2d77c75d-173b-4aea-9c44-063afe03703d`: 499/499 library tests passed with
four workers in 120.315 seconds.  This includes the anchor-free persistent
cylindrical elimination certificate, exact structural-identity utility, and
the refactored typed V2 relation manifest.  It is a regression checkpoint,
not evidence that the later recentering, `WhenBad`, exceptional recursion, or
provider seams are complete.

The strongest end-to-end concrete multiloop fixture remains the equal-mass
three-loop tetrahedron.  Freshly generated rows and discovered `S4`
symmetries reduce, among other inputs,
`J(2,1,1,1,1,1)=(d-4)/(4 m2) J(1,1,1,1,1,1)` and reduce the rank-two dotted-B4
tensor fixture to `3/8 g(mu,nu) B4`, with masters left unsubstituted.  That
test uses certified demand-time concrete quotient elimination and an explicit
five-class master selection.  It therefore validates the generated algebra
at three loops but does not claim a reusable three-loop parametric
`SolvejSector` database.

The topology-neutral normalized-source owner, sealed fresh-normalization seam,
bounded direct formula-residual cursor, one-pass sealed ingress/replay token,
and normalized-source V2 ordering-policy binding are implemented at pushed
checkpoint `c593865`. The source owns and replays one exact row-span
allocation, every ordered attempt, the normalized IR/locus table, and the
original resource envelope. The direct cursor searches authenticated candidate
bad-formulas with one three-valued assignment table and one resumable DFS
frontier.

That pushed checkpoint's one-pass candidate-to-normalized-source ingress
performs `N` construction authentications for `N` candidates instead of the
legacy `2N`.
Focused run `b2ba7679-e7c8-4e64-ba25-c451024843bf` passed 6/6 tests, and
independent affected-suite run `db2a98a5-d473-4cdc-b2b7-fe2f444357e8` passed
44/44. Honest all-36 `L=6`, `K=21` primary run
`37d85ddb-c356-4c79-a6f4-d428828db039` passed 1/1 in 58.109 seconds. It
performed 36 construction authentications, constructed the same 49 loci over
36 attempts with 15 Certified and 21 Unsupported outcomes, and reached the
first residual after 30 decisions with 19 loci free and a 1,841-byte peak
cursor. Candidate-to-source construction took 17.4507 seconds, cursor
initialization 16.756 microseconds, and first-residual search 832.37
microseconds. The independent semantic oracle exhaustively checked all
524,288 completions. Independent K21 rerun
`e00cdbea-6312-4fb3-9856-0c2f3bf2ef25` also passed in 56.359 seconds.

The same pushed checkpoint advances the persisted authority to
normalized-source V2. Every source now owns one explicit
`IntegralOrderingPolicy`, including an empty-attempt source, and replay
authenticates every present candidate's policy. Owner-focused run
`8ad499a3-339e-4e0b-a04f-ccf754406516` passed
21/21 tests, formula/residual run `6a5267d1-fe75-4854-8b98-9a03b1bb2370`
passed 14/14, and independent audit/validation run
`430af297-b806-431e-a169-bd0f19a9f9c8` passed 30/30. The policy-bound all-36
`L=6`, `K=21` run `88a73ec1-52c2-4771-8a21-75e1b2a848b6` passed 1/1 with
36 construction authentications, the same 15 Certified/21 Unsupported
semantics, and a 1.405-millisecond first-residual search. These are pushed
`c593865` results; public library and CLI integration remain pending.

For comparison, the earlier two-stage run
`e7378e6e-5df5-47c3-8fe9-686bbaa8ef30` took 72.935 seconds, spent 17.29 +
16.21 seconds in its two construction phases, and performed 72 construction
authentications. The new fixture's 18.51-second source replay and 17.57-second
path replay are deliberate stress-validation replays, not production
direct-search cost. Neither path invokes an MTBDD compiler or constructs an
MTBDD owner or DAG.

The direct backend itself constructs neither the legacy explicit V4 partition
nor the complete V5 MTBDD and does not invoke the residual Boolean/DPLL owner.
That bypass is a production requirement, not merely a performance preference.
V4 remains a small-fixture differential oracle, and V5 remains an optional
repeated-query classifier only when its separately measured construction
budget is acceptable. The currently published generated-sector discovery
entry still eagerly creates V4, but the pushed one-pass ingress is the intended
production source API ahead of that materialization and preserves exact
binding/replay guarantees through its sealed token. Public library and CLI
integration of that API remain pending.

This checkpoint completes the allocation-independent stable-value identity for
the generic direct formula-path terminal. The authenticated row span is emitted
once; later occurrences are typed identity references. Direct singleton
authority carries that identity through ordering V3, physical frame V2,
solve-plan V2, and source-profiled exact target/database/session V2 without
fabricating V4/V5, Boolean/DPLL, integer-system, or legacy inventory
certificates. Stable mathematical identity is not an allocation capability:
exact terminal, authority, frame, plan, and catalog `Arc` ancestry is
authenticated separately. Authenticated selector-independent compact affine
maps, including constrained maps, now reach the existing unpublished
`ReadyForConditions` boundary. V2 checks exact selector geometry and proves
physical-key descent inside the authenticated source chamber; inactive
positive-shift crossings remain explicit hazards for later condition
partitioning. Replay rebuilds the transcript only after authenticating the
exact retained target-state allocation. This is still a pre-publication
boundary, not a reduction result.

The next owner-bound, non-publishing slice is also implemented. It maps sources
in condition-plan order, retaining the full schedule for a partition-ready
outcome or the decisive prefix for an identically-bad outcome. It keeps
distinct physical-parameter identity projections for the pre-normalization and
normalized denominators and specializes admitted arbitrary-width hazards into
ordered exact boundary events through the Symbolica mapping and numerator-
divisibility kernels. Its
production-derived sector-011 acceptance owner has seven sources, four hazard
ranges, and five events: one suppressed by the numerator and four retained bad
boundaries. The focused default-GMP suite passed 16/16 tests with four Rust
test threads, including exact/one-below aggregate and boundary limits,
retry-owner recovery, replay, foreign-owner rejection, and global retained/
peak accounting. This materializer consumes no target, publishes no rule, and
does not establish a reduction.

The following move-only, non-publishing slice is now implemented as well. It
assembles the current-lineage arbitrary-width OR-of-AND bad formula from the
mapped owner, interns first-seen structural loci through Symbolica-backed
exact/associate tests while retaining every occurrence provenance, and builds
its replayable applicable/exceptional relative partition. The canonical locus
table is an opaque, non-cloneable authority: the outer compiler proves exact
and associate canonicality once per fresh compilation or full proof replay,
while authenticated inner compilation and its nested replay perform linear
validation and no duplicate pairwise/native associate work. Compact
fixed-capacity copies, spare-GMP payloads, duplicate-heavy
replay, panic/retry ownership, and exact/one-below aggregate limits are tested.
Licensed default-GMP focused Nextest run
`b0217edc-a9e8-4a7d-9c5c-82b824a636b3` passed 19/19 tests with four workers;
an independent superset passed 20/20. Authoritative licensed default-GMP
Nextest run `e9004c32-5a51-4705-a2f9-e39bcac40c49` then passed 1,651/1,651
tests with four workers (52 slow), with 5 additional configured cases skipped;
the following doctest phase also passed. This slice consumes no
target, publishes no rule, and has no topology or loop-count dispatch. It does
not establish complete LiteRed `WhenBad` closure.

Compact routing preparation is now one consuming transition. It distills the
sealed derivation owner into move-only commit state plus canonical loci, final
relative cases, and a one-byte applicable/domain/leak tag per leaf. Typed
operational failure returns the exact input owner. The licensed preparation
suite passed 3/3 tests with four Rust test threads.

The following internal atomic commit is also implemented. It advances the
exact database, consumes exactly one selected target, and stores one compact
application event containing centered relation terms, target locator/offset,
loci, cases, and the one-byte tags. The publication event does not retain the
derivation row translation, row guards, derivation statistics, source recipe,
or pivot evidence. One shallow event owner now exposes zero-copy applicable
and exceptional leaves and event-bound domains containing the parent premises
and resolved relative predicates. It does not schedule exceptional work
through the provider, apply a rule, support optional publication audit replay,
or establish a complete reduction.

The current exact-session event replacement and target successor are
correctness-first, not six-loop-scalable: they copy the prior event-`Arc`
vector and the full target-disposition vector on every transition. The compact
publication payload is retained only once in its event, and the implemented
handles are shallow views rather than deep rule/residual duplication. Before
high-loop deployment, session storage must
move to a chunked/persistent event log and shared or paged copy-on-write target
dispositions.

The next low-level scaling gate remains one declared arity-21 sector reaching
exact Ready through that direct hand-off, but it is not a foundry scalability
acceptance by itself. The successful `K=21` cursor fixture stops
at its first certified formula-residual path: it creates no affine inventory,
does not enter Ready, publishes no guarded rule, and is not a physical vacuum
topology. Neither that milestone nor the existing lower-arity fixtures
establishes a complete reduction.

The following publication milestone remains the topology-neutral
`GeneratedFamilySymbolicResidualSolveV1`.  It will connect the owned
normalized frontier, cylindrical symbolic start, cumulative translated IBP/LI
row system, preordered persistent elimination, generated `WhenBad` candidates,
exceptional residual work, and the generic provider.  Its first accepted solve
mode is an independent integer cylinder; dependent symbolic starts remain a
typed pending outcome rather than being replaced by an arbitrary integer
sample. An artifact may carry loop count and topology/family metadata for
routing, inspection, and benchmark reports. Those values may not dispatch to
an algorithm, expected recurrence, or inferred master.

The exact first-mode pipeline is:

```text
generated IBP/LI rows
-> proved-zero and self-symmetry canonicalized prepare-point rows
-> ordered candidate attempts plus one authenticated row span
-> one-pass sealed normalized-source ingress/replay token          [pushed c593865]
-> sealed normalized-source V2 + ordering-policy binding          [pushed c593865]
-> bounded direct formula-residual cursor                        [implemented]
-> coordinate-affine direct formula-path terminal                [implemented]
-> direct singleton case authority + sealed premises             [implemented]
-> terminal stable identity -> ordering V3 -> frame V2
   -> Direct solve-plan V2, no inventory                          [implemented]
-> Direct solve-plan -> exact-session staging and recentering
   -> authenticated compact-affine ReadyForConditions             [implemented]
-> one persistent cylindrical elimination database per residual case
-> symbolic pivot recentering
-> owner-bound mapped conditions, dual denominator projections,
   and exact RHS-boundary events                                  [implemented, nonpublishing]
-> one-shot canonical-locus authority -> owner-bound relative partition
   of the current-lineage arbitrary-width OR-of-AND formula       [implemented, nonpublishing]
-> move-bound commit state + one-byte-per-leaf route preparation  [implemented]
-> atomic target consumption + compact application event          [implemented internally]
-> shallow event-bound rule/residual domains                      [implemented internally]
-> complete stage-local physical-key catalog
   -> live Symbolica SparseRowReducer/LuLMode::Full authority
   -> guarded differential/provenance replay                     [implemented]
-> committed native sparse telemetry                            [implemented]
-> benchmark export + persistent-reducer scaling study          [next]
-> topology-neutral CampaignPlan: shared-child DAG + ready antichain       [implemented, static only]
-> owning provider/residual scheduling
-> exceptional-domain recursion and solved-subsector feedback
-> coverage fixed point (publication replay is an optional audit)
-> immutable closed family/sector shard
-> exact-gated deterministic multi-start Closed bundle
-> physical six-loop derivation-only closure and scaling gate
-> generic provider and descending application
```

Increasing prepare-point depth extends the same residual-case database; it
does not restart elimination.  Finite prepare points discover candidates but
never prove their symbolic domains.  A failed search remains typed uncovered
state and is not promoted to either a zero proof or a certified master.

Acceptance first derives and proves closure of the complete one-loop tadpole
recurrence family. A minimal generic application seam then checks powers two
through four with only `I(1)` explicitly selected and passes tensor ranks zero,
one, two, and four through the Symbolica parser, generic projector/lowering,
generated provider, and Vakint oracle. Each
supported numerator rank must also pass a metamorphic cancellation oracle:
an uncancelled numerator factor equal to a propagator and the explicitly
power-lowered input independently rebuild and replay their proof graphs, have
identical exact lowering maps, and reduce to the same unreplaced-master map on
the same semantic domain.  Raw source-origin ordinals are not compared because
equivalent presentations legitimately have different provenance histories. A
second one-loop family with an external momentum and automatically completed
ISP guards against accidentally specializing the solver to vacuum or one
denominator. This remains a compact genericity test rather than the next
deployment feature. The foundry then derives complete one- through four-loop
replacement systems for the Vakint corpus and checks normalized reductions
against that external oracle without FORM. General five-loop and physical
six-loop derivation-only closure/profile gates follow before optimization of
the concrete batch runtime. Engineering effort between those gates prioritizes
the shared vacuum foundry and multi-start campaign bundles rather than an
arbitrary non-vacuum pentagon milestone.

## Legacy loop-specific oracle surface

All authored loop/topology reducers and hardcoded IBP weights, including the
canonical-`I2L` `VakintTwoLoopAdapter`, are excluded from the default surface
behind the non-default `legacy-authored-oracles` feature. Those implementations
are regression/oracle fixtures, not admissible production derivation. No
acceptance milestone may call them on the RustRed subject path; only freshly
generated generic rules may produce the result being compared.
