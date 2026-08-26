# RustRed governing scope and acceptance criteria

Date: 2026-08-13. Reconciled with the LiteRed/Symbolica/Vakint source audits
on 2026-08-20 and reprioritized for the six-loop single-scale vacuum campaign
on 2026-08-24. Implementation status was reconciled with pushed checkpoint
`c593865` and the Direct singleton stable-identity/exact-session Ready-ingress
checkpoint on 2026-08-25.
Owner-bound non-publishing mapped-condition materialization and its
production-derived sector-011 acceptance were reconciled on 2026-08-26.

## Reading status

This document is normative: its feature lists and validation ladder define
governing acceptance criteria, not evidence that those features are already
implemented or validated.  A module name, test fixture, or concrete oracle
does not establish parity by itself.  Current source coverage and known gaps
are tracked separately in
[`rustred_source_surface_gap_audit_2026-08-14.md`](rustred_source_surface_gap_audit_2026-08-14.md).

## Governing objective

RustRed is a pure-Rust, fully Symbolica implementation of the functionality
provided by LiteRed.  It must not be a collection of built-in reductions for
particular loop counts or topologies.  Algorithms may differ from LiteRed
where Symbolica provides a better exact representation, but the public scope
and mathematical behavior are defined by the LiteRed implementation vendored
in `vendor/LiteRed2`.

RustRed also owns tensor-numerator reduction and the application of discovered
scalar reduction rules.  The behavioral reference for that layer is Vakint
and its alphaLoop integration under `vendor/gammaloop/crates/vakint`.  FORM
sources may be read to understand the algorithms and conventions, but RustRed
must never invoke or depend on FORM.  The implementation is Rust plus the
vendored Symbolica Rust API.

The immediate deployment priority is highly efficient reduction of up to
six-loop, single-scale massive vacuum graphs produced after GammaLoop's
general BPHZ R-operation.  This priority does not permit loop-count or topology
dispatch in production.  It changes the order of implementation: reusable
vacuum-family/sector rule derivation and a separate batched rule-application
runtime take precedence over non-vacuum examples and broad Feynman-parametric
polishing.  The detailed two-stage architecture and benchmark contract are in
[`six_loop_single_scale_vacuum_priority_2026-08-24.md`](six_loop_single_scale_vacuum_priority_2026-08-24.md).

## Source-of-truth order

1. LiteRed's Mathematica source defines basis construction, scalar-product
   conversion, fully parametric IBP and LI relations, sectors, symmetries,
   guarded recurrence discovery, rule application, masters, and persistence.
2. Symbolica's vendored Rust source defines the exact algebra, patterns,
   substitutions, polynomial/rational-polynomial operations, serialization,
   and performance facilities available to the implementation.
3. Vakint's Rust, tests, FORM resources, and alphaLoop resources define tensor
   input/output conventions, tensor projection behavior, topology
   normalization, and the expected application of parametric rules.

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

Cached discovered rules are allowed only as versioned artifacts carrying the
family/kinematics/order fingerprint, domains, guards, and replayable source
provenance.  They must be reproducibly derivable from the generic engine.

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
- stable, authenticated persistence and recovery;
- Feynman-parametric and dimension-shift facilities where LiteRed exposes
  them; and
- no implicit promotion of an uncovered integral to a proved master.

The first generator milestone is specifically LiteRed `GenerateIBP` parity:
for `L` loops and `E` external momenta it emits `L*(L+E)` ordinary parametric
relations in contraction-major order, plus `E*(E-1)/2` LI relations.  It
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

Validation advances only after the preceding rung passes:

1. synthetic generic family and algebra properties;
2. one-loop parametric relation generation and scalar reductions;
3. varied one-loop tensor numerators, compared with Vakint while leaving
   master topologies unsubstituted;
4. representation-closure checks in which a numerator factor equal to a
   propagator (for example `q_i^2-m_i^2`) is compared exactly with the same
   input after explicit propagator-power cancellation, before IBP and again
   on the unreplaced-master result and semantic guard loci;
5. two-loop scalar and tensor reductions, including the same cancellation
   closure checks;
6. three-loop scalar and tensor reductions, including comparison with the
   alphaLoop parametric-rule behavior;
7. four-loop massive-vacuum families; and
8. general five-loop massive-vacuum families, not only the banana; and
9. a declared six-loop GammaLoop/BPHZ-derived single-scale vacuum corpus.

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
  validation caveat, RustRed must use public `SparseRowReducer` with independent
  rank/residual and transcript checks rather than implementing another CAS or
  matrix package.

## Current implementation gate and next generic slice

The raw generic generator is real rather than a recurrence table: licensed
GMP run `5ae578f9-5bff-4cf9-bf3f-7013730923ee` passed 20/20 parallel tests
covering one- through five-loop generated IBP/LI identities, symbolic power
shifts and external-momentum families, the Symbolica tensor-numerator
boundary, and FORM-free one-loop Vakint scalar/tensor oracles.  Those concrete
families validate generation; they do not establish complete LiteRed
`SolvejSector` parity.

Published LiteRed examples are now a separate, explicit acceptance lane.  The
eight LiteRed 1.x notebooks and the LiteRed2 example notebooks will be tracked
from input normalization through identity, sector/symmetry, parametric-rule,
target-reduction, and auxiliary-recurrence parity.  Current ingredient tests
do not count as complete notebook passes.  The initial inventory and honest
status baseline are recorded in
[`litered_examples_acceptance_matrix.md`](litered_examples_acceptance_matrix.md).

The post-cylindrical-elimination library checkpoint is licensed GMP nextest
run `2d77c75d-173b-4aea-9c44-063afe03703d`: 499/499 library tests passed with
four workers in 120.315 seconds.  This includes the anchor-free persistent
cylindrical elimination certificate, exact structural-identity utility, and
the refactored typed V2 relation manifest.  It is a regression checkpoint,
not evidence that the later recentering, `WhenBad`, exceptional recursion, or
provider seams are complete.

The strongest complete concrete multiloop acceptance remains the equal-mass
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

The immediate generic semantic slice is now the relative `WhenBad` partition
and atomic guarded-rule/residual publication, followed by solved-subsector
feedback into supersectors. It must not reconstruct V4, V5, the live-leaf
queue, or old Boolean/DPLL certificates along the way.

The first scaling gate remains one declared arity-21 sector reaching exact
Ready through that direct hand-off. The successful `K=21` cursor fixture stops
at its first certified formula-residual path: it creates no affine inventory,
does not enter Ready, publishes no guarded rule, and is not a physical vacuum
topology. Neither that milestone nor the existing lower-arity fixtures
establishes a complete reduction.

The following publication milestone remains the topology-neutral
`GeneratedFamilySymbolicResidualSolveV1`.  It will connect the authenticated
normalized frontier, cylindrical symbolic start, cumulative translated IBP/LI
row system, preordered persistent elimination, generated `WhenBad` candidates,
exceptional residual work, and the generic provider.  Its first accepted solve
mode is an independent integer cylinder; dependent symbolic starts remain a
typed pending outcome rather than being replaced by an arbitrary integer
sample.  The artifact may contain no loop-count/topology tag, expected
recurrence, or inferred master.

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
-> relative WhenBad partition and atomic guarded publication      [next]
-> exceptional-domain recursion and solved-subsector feedback
-> replayed coverage certificate
-> generic provider and descending application
```

Increasing prepare-point depth extends the same residual-case database; it
does not restart elimination.  Finite prepare points discover candidates but
never prove their symbolic domains.  A failed search remains typed uncovered
state and is not promoted to either a zero proof or a certified master.

Acceptance first derives the complete one-loop tadpole recurrence family and
checks powers two through four with only `I(1)` explicitly selected, then
passes tensor ranks zero, one, two, and four through the Symbolica parser,
generic projector/lowering, generated provider, and Vakint oracle.  Each
supported numerator rank must also pass a metamorphic cancellation oracle:
an uncancelled numerator factor equal to a propagator and the explicitly
power-lowered input independently rebuild and replay their proof graphs, have
identical exact lowering maps, and reduce to the same unreplaced-master map on
the same semantic domain.  Raw source-origin ordinals are not compared because
equivalent presentations legitimately have different provenance histories. A
second one-loop family with an external momentum and automatically completed
ISP guards against accidentally specializing the solver to vacuum or one
denominator.  This remains a compact genericity test rather than the next
deployment feature.  Only after the generic symbolic path replays and passes
its resource/tamper matrix does validation advance through two and three
loops, the complete Vakint four-loop corpus, general five-loop families, and a
declared six-loop GammaLoop/BPHZ corpus.  Engineering effort between those
gates prioritizes the shared vacuum foundry and batch-application path rather
than an arbitrary non-vacuum pentagon milestone.

## Legacy loop-specific oracle surface

The current source still exposes authored one-, two-, and three-loop reducers
and hardcoded IBP weights from `src/lib.rs`.  The canonical-`I2L`
`VakintTwoLoopAdapter` has been removed from the default surface and is
available only through the non-default `legacy-authored-oracles` feature.
Those implementations are regression/oracle fixtures, not admissible
production derivation.  They must move behind an explicit test-support
boundary (and ultimately out of the default public library surface).  No
acceptance milestone may call them on the RustRed subject path; only the
freshly generated generic rules may produce the result being compared.
