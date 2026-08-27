# Exact-session `WhenBad` and publication port plan

Status: authoritative implementation plan; Phase A event-ledger foundation and
selector-independent compact-affine Ready V2 geometry, fixed-chamber descent,
lazy hazards, and replay are implemented. Upstream sealed normalization,
direct formula-residual search, one-pass sealed candidate ingress/replay,
normalized-source V2 ordering-policy binding, and the K21 evidence were pushed
at historical pushed checkpoint `c593865`; subsequent current-lineage work
carries the allocation-independent Direct identity through ordering V3, physical frame V2,
solve-plan V2, and source-profiled exact-session Ready analysis without a fake
inventory. A non-publishing owner-bound Ready condition plan, a
Symbolica-backed physical-parameter identity projector, arbitrary-width direct
bad-formula routing, and the source-neutral exact affine-boundary
mapping/divisibility kernel are implemented. Subsequent checkpoints also own
and replay mapped conditions and coefficients, retain distinct
pre-normalization and normalized denominator projections, and specialize
admitted lazy hazards into exact boundary events. A subsequent move-only,
non-publishing owner assembles the current-lineage arbitrary-width OR-of-AND bad
formula, interns its structural loci with Symbolica-backed associate proofs,
and builds its replayable relative partition. Its licensed default-GMP
relative-partition gate passed 8/8 with four Rust test threads. The subsequent
authenticated canonical-locus owner gate passed 19/19, with an independent
20/20 superset. Authoritative licensed default-GMP Nextest run
`e9004c32-5a51-4705-a2f9-e39bcac40c49` then used four workers, ran 1,651
tests, and passed all 1,651 (52 slow), with 5 additional configured cases
skipped; the following doctest phase also passed. Compact route preparation is
now implemented as one move-only transition retaining one-byte leaf tags; its
licensed four-thread preparation suite passed 3/3. The internal atomic commit
now advances the database, consumes one selected target, and stores a compact
application event (the internal `Publication` disposition). The frozen
licensed default-GMP gate passed all 1,658 runnable tests with four Nextest
workers, with 5 configured cases skipped; doctests also passed. A subsequent
checkpoint makes the compact event shallow-ownable through one `Arc`, retains
the already-proved pivot-term ordinal, and exposes zero-copy applicable and
exceptional leaf views together with the parent target premises and affine
geometry. A further checkpoint compiles committed receipts into a bounded,
canonically ordered handoff wave with one atomic byte per leaf and bounded
borrowed tickets; its licensed four-thread focused gate passed 10/10. Handoff
acknowledgement proves acceptance only. A further algebra-free current-worktree
milestone consumes a quiescent, fully acknowledged wave into one
`ExactPublicationEpochOwner`: it retains one event handle per slot, compact
flat indices for applicable and exceptional leaves, and one atomic byte per
exceptional source. Exceptional access is bounded by retry-only leases; normal
drop and unwind restore pending state, and explicit quiescent recovery handles
a deliberately forgotten lease. Its transferred-event, retained-shallow,
compilation-peak, and live-lease byte limits are component gates, not process-
RSS admission. Its licensed default-GMP validation passed 6/6 focused tests
and the 16/16 parent handoff-module superset with four test threads, and
`cargo check --tests -j4` passed. A separate optional debug full-library
diagnostic reached 1,214 passes and 2 skips; the sole remaining pre-existing
equality-suspension test was deliberately interrupted after 1,834.832 seconds,
so this checkpoint makes no new complete full-library-gate claim.
Applicable-provider admission/results, general `CampaignWorkKey` result
publication, durable spill support, event-derived fresh narrowed-domain
re-entry, rejected-candidate continuation, rule application, publication audit
replay, subsector feedback, closure, and physical reduction remain pending. The
live exact
database now owns the complete easiest-first physical-key catalog and a clone-
on-stage Full-L Symbolica `SparseRowReducer` with its final sentinel. Each stage
inserts only new catalog columns and submits one candidate; only an independent
trial supplies the move-owned reducer/catalog successor that may commit.
Symbolica authoritatively supplies ordered pivot factors, normalization, and
disposition. RustRed authenticates the complete historical U/L/pivot prefix and
the appended normalized U row coefficient-for-coefficient before commit. The
exact-database rebuilding glue/use is now a `cfg(test)` differential oracle;
the generic legacy adapter remains compiled outside the live path. Licensed default-GMP
four-thread runs pass 15/15 retained-adapter, 18/18 complete sparse-adapter, and
41/41 exact-database tests. Fixed-size native telemetry remains outside replay
identity. Every stage still deep-clones the full native reducer, forward
elimination is serial, and opaque native heap/scratch bytes are not censused;
this is not evidence of physical-topology reduction, Vakint reproduction, or
six-loop scalability.
Bundling and physical six-loop derivation still precede optimized application;
updated 2026-08-26.

This document specifies the next topology-neutral RustRed seam after
`GeneratedAffineResidualGroupExactSessionRecenterOutcome`. It joins the
current exact session transaction to LiteRed-style `WhenBad`, target
disposition, exceptional residual work, and sealed rule publication.

The implementation governed by this plan is pure Rust using GMP-enabled
Symbolica. It must not use FORM, Mathematica at runtime, Symbolica's `no_gmp`
feature, loop-count-specific code, topology-specific recurrences, or
hard-coded reduction rules. Loop counts and concrete vacuum or scattering
topologies are validation inputs only; they never select a core algorithm.
Vakint's one- through four-loop FORM-backed replacement systems are therefore
black-box acceptance oracles and a required coverage target, not an input rule
source: RustRed must independently derive the corresponding parametric rules,
apply them natively in Rust/Symbolica, and only then compare normalized
concrete reductions with the frozen Vakint results. Acceptance is equality of
the final expression over unsubstituted master/terminal symbols and its
semantic guard domain after the explicit convention map, not identity with an
authored FORM recurrence, pivot order, or intermediate rule sequence.

Private in-memory Rust types are trusted to preserve invariants established by
their sealed constructors. New forward-path stages must not add schema strings,
fingerprints, replay passes, or binding capabilities merely to revalidate an
adjacent private owner. Explicit authentication is reserved for untrusted
input/import, durable cache loading, and the one live-session mutation
boundary. There is no backward-compatibility requirement during this design
stage; stale internal formats may be replaced rather than migrated.

The preceding database/recenter ownership contract remains
`docs/research/exact_group_solve_transaction.md`. This plan is narrower and
normative for the missing `WhenBad` and publication seam.

## 1. Current implementation checkpoint

The current-lineage components below are implemented and tested:

- `src/generated_affine_residual_group_exact_database.rs` stages and commits
  LiteRed-style hardest-only exact rows with consume-once transition identity.
- `src/generated_affine_residual_group_exact_targets.rs` owns the persisted
  target order, current target premises, typed equality-refinement targets,
  immutable target dispositions, and exact successor preparation. Its
  `prepare_successor(..., Option<GeneratedAffineResidualGroupRetainedReadyExactTarget>)`
  can either preserve every target or consume exactly one authenticated Ready
  target.
- `src/generated_affine_residual_group_exact_session.rs` is the sole owner of
  the database and exact target-state allocation. It seals them into a staged
  transaction, exposes an authenticated joint view, and returns the owning
  `GeneratedAffineResidualGroupExactSessionRecenterOutcome::{NoTarget,
  RequiresAffineEqualityRefinement, Ready}`.
- `src/generated_affine_residual_group_exact_recenter_kernel.rs` supplies the
  authority-free, arbitrary-precision recentering arithmetic through
  `ExactCenteredShift`, `ExactRecenteredTerm`, and `ExactRecenteredRow`.
- Current Ready outcomes retain the exact recentered row, the retained Ready
  target, and the original consume-once transaction. Target premises and
  translated row guards remain separate. No outcome publishes a rule or
  consumes a target.
- The current lineage also implements typed `commit_no_target` and
  `commit_and_suspend_affine_equality_refinement` transitions. NoTarget
  consumes the running session and returns the sole continuation owner only
  after commit. Equality consumes the running session into a sealed
  `GeneratedAffineResidualGroupExactSessionSuspendedForRefinedEpoch` that
  retains the committed session, successor-bound unresolved equality target,
  and exact private terminal event while exposing no resume, staging, or
  session-extraction method.
- Staged database rows retain an opaque production or synthetic source recipe
  and shared exact dependent-reduction or new-pivot evidence. The private
  transaction path prepares an owning database commit that can be aborted on
  recoverable outer-preflight failure. After all event and target replacements
  are admitted, an allocation-free checked fail-stop boundary precedes the
  infallible database/owner commit tail. Equality successor state and rebound
  target are minted as one capability-gated pair over the same exact
  allocation.
- The session owns a private append-only event ledger for every consumed
  source. `Dependent`, `NoTarget`, and mandatory affine-equality events retain
  their source/evidence allocations. The compact publication event instead
  stores only centered relation terms, target locator/offset, canonical loci,
  final relative cases, and one-byte leaf tags; it drops derivation row
  translation, row guards, derivation statistics, source recipe, and pivot
  evidence.
- `replay()` creates a fresh shadow session for `Dependent`, `NoTarget`, and
  affine-equality events. Publication-event audit replay is not implemented;
  normal future application will read the compact event directly.
- `src/generated_affine_residual_group_ready_publication.rs` authenticates the
  sealed Ready/session/target geometry without extracting the transaction. For
  an authenticated selector-independent compact affine map it locates the
  unique unit zero-shift pivot, verifies selector and row geometry, builds
  source and RHS keys from the exact selected anchor, proves strict physical-
  key descent inside the source chamber, and retains finite inactive-orthant
  hazard intervals as Symbolica `Integer` data. Its `ReadyForConditions` result
  is an unpublished, target-preserving typestate. Condition planning,
  materialization, relative partitioning, compact preparation, and the internal
  target-consuming application-event commit are implemented downstream.
- `src/generated_affine_residual_group_exact_when_bad_conditions.rs` consumes
  that typestate only into a recoverable, non-Clone condition plan. It
  authenticates identity or compact-affine target geometry and retains a
  deterministic schedule of target premises, recentered row guards, the pivot
  and descending RHS coefficients, plus fixed-width lazy hazard locators. It
  does not itself materialize a condition, consumes no target, and publishes
  no rule.
- `src/generated_affine_residual_group_exact_when_bad_materialization.rs`
  consumes that exact plan into an owner-bound, replayable, non-publishing
  mapped transcript. It maps sources in schedule order, retaining the full
  schedule for a partition-ready result or the decisive prefix for an
  identically-bad result. It keeps both the pre-normalization and normalized
  mapped-denominator identity projections, admits arbitrary-width hazard
  ranges before expansion, and specializes their exact values into ordered
  boundary events with Symbolica-backed numerator classification. It still
  consumes no target and publishes no rule.
- `src/generated_affine_residual_group_exact_when_bad_partition.rs` consumes
  the mapped owner into a recoverable, move-only, non-publishing partition
  compilation. It assembles the arbitrary-width OR-of-AND bad formula,
  interns first-seen loci with exact Symbolica equality and associate proofs,
  preserves every source/formula occurrence, and builds a replayable relative
  applicable/exceptional partition. Resource, authentication, allocation, and
  panic failures return the original owner. Both `ReadyForPublication` and
  `IdenticallyBad` consume zero targets and publish zero rules.
- `src/generated_affine_residual_group_exact_publication.rs` distills
  `ReadyForPublication` into move-only commit state plus canonical loci, final
  relative cases, and a one-byte applicable/domain/leak tag per leaf. The
  exact-session commit advances the database, consumes one selected target,
  and stores those values with centered relation terms and target
  locator/offset in a compact event. One shallow owner now exposes event-bound
  rule/residual leaves and complete zero-copy domains (parent premises plus
  resolved relative predicates). The later epoch owner supplies algebra-free
  retry scheduling; this event alone is not mathematical residual ingress, a
  provider result, or a reduction.
- `src/generated_affine_residual_group_exact_publication_handoff.rs` consumes
  committed receipts into a bounded canonical `(job, lane, event, leaf)`
  handoff wave. It retains one event handle per slot, one atomic byte per leaf,
  and a hard ceiling on non-Clone borrowed tickets. Acknowledgement records
  handoff acceptance only; the module does not apply a rule or prove discharge,
  coverage, closure, or a terminal.
- `src/generated_affine_residual_group_exact_publication_epoch_owner.rs`
  consumes only a fully acknowledged, quiescent handoff and algebra-free moves
  its slots into one closure-epoch scheduling owner. It keeps the single event
  handle per slot, compact flat applicable/exceptional indices, and one atomic
  byte per exceptional source. Bounded retry-only exceptional leases restore
  pending state on normal drop or panic; a barrier-only recovery operation
  handles deliberately forgotten leases. Its memory limits enumerate the
  transferred event payload, shallow owner buffers, compilation peak, and live
  lease bytes only. It is not applicable-provider admission, a result owner,
  stable `CampaignWorkKey` staging, fresh narrowed-domain database/reducer
  ingress, same-database rejected-candidate continuation, or closure.
- `src/parametric_coefficient.rs` supplies a source-neutral Symbolica-backed
  physical-parameter identity projector. It projects through
  `RationalPolynomial::to_polynomial`, transports exact index-polynomial loci
  with Symbolica variable unification, and classifies coefficients as always
  identity-zero, never identity-zero, or conditional without assuming a fixed
  number of indices or parameters.

Focused tests cover exact Ready translation, NoTarget beyond `i64`, a 4,096-bit
coordinate, post-top-reduction leader selection, transaction return after
stale/foreign/resource failure, equality return before translation, atomic
dependent commit, sealed sibling-successor pairing, exact/one-below resource
boundaries, and the private production-source recipe's Arc lifetime and replay.

The typed transition slice passed licensed, GMP-enabled four-way `cargo-nextest`
runs `f8fefe69-c966-48eb-ada8-9bac85f24158` (sealed equality, 1/1),
`e06c10c3-2f18-48ca-8eec-51a229972d82` (source surface plus compositional
production recipe, 2/2), and `4e1ef6e4-749e-4f2e-8f00-869059b61f20`
(remaining exact database/session/targets/recenter-kernel regression, 44/44).
`cargo check --tests -j 4`, `cargo fmt --all -- --check`, and
`git diff --check` also passed.

The event-ledger slice adds focused tests for private
event/source/evidence `Arc` identity and lifetime, retained production-source
replay after staging owners drop, cumulative and one-below event/replay owner
limits, chronological fresh-shadow re-execution of dependent and pivot
transitions, exact NoTarget/equality offsets, and sealed suspension replay.
The production-source resource test additionally drops every external source
pipeline handle and proves that the admitted recipe owns the physical row,
re-elimination, bound outcomes, elimination, and source-local
authority/premises/ordering/schedule graph. A separate anchor fixture proves
that only exact frame-authority pointer identity suppresses that authority's
otherwise unique allocation.

The frozen event-ledger milestone passed licensed, GMP-enabled four-way
`cargo-nextest` runs `34021f1d-7458-4700-9ec9-4155cd338c39` (all 16 exact
session tests, including the sealed equality replay, 16/16),
`75cec2fb-6846-4a1b-8df5-029b1331e717` (exact database, physical-row, and
recenter-kernel tests, 42/42), and `ff9310fa-fb3b-42e2-b79f-531fc93708ad`
(the complete retained source-parent graph gate, 62/62). `cargo check
--all-targets -j 4`, `cargo fmt --all -- --check`, and `git diff --check`
also passed. Neither FORM nor Symbolica's `no_gmp` feature was used.

Production-source coverage is deliberately compositional: genuine physical-row
ingress proves retained Arc identity, lifetime, and row replay, while the full
equality transition test uses its sealed synthetic test adapter. The current
physical-row fixture skips equality-premise source cases, so an end-to-end
production equality row remains a future refined-epoch gate.

The scoped exact Ready checkpoint passed independent licensed-GMP Nextest
gates with explicit `--lib -j4`: run
`a06d5558-e404-4048-a2e9-5407277a95d6` passed all 11 tests in the independent
Ready/publication validation module, and
`f74b89eb-1e59-4628-91d7-82af1f11b893` passed the two internal Ready units plus
the physical-key comparison witness. The latter shares the authoritative
comparison implementation used by `Ord`; successful descent transcripts keep
only the first decisive component rather than retaining every RHS key. A fast
test-only `L=6`, `K=21` family validates all 36 generic ordinary-IBP rows and
stable regeneration. A genuine all-inactive arity-21 Ready probe was blocked
earlier by eager Boolean-cover split 65,537 exceeding the 65,536 cap;
the cap was not raised.  A later complete-MTBDD experiment avoided that
explicit partition but retained 49 atoms and 268,427 rooted nodes before its
cursor could return the first residual.

The replacement source owner, sealed fresh-normalization seam, and bounded
direct formula-residual cursor are implemented at pushed checkpoint
`c593865`. The direct cursor searches the authenticated normalized formulas
with one three-valued assignment table and one resumable DFS frontier; it
constructs no V4 partition or V5 MTBDD and invokes no residual Boolean/DPLL
owner. Its focused parallel GMP audit passed 9/9 tests. The MTBDD remains an
optional compact-case backend under a separately measured construction budget,
not the primary arity-21 entry path.

That pushed checkpoint adds a safe sealed replay token and one-pass
candidate-to-normalized-source ingress, performing `N`
construction authentications rather than the legacy `2N`. Focused run
`b2ba7679-e7c8-4e64-ba25-c451024843bf` passed 6/6 tests and independent
affected-suite run `db2a98a5-d473-4cdc-b2b7-fe2f444357e8` passed 44/44.
Primary honest all-36 `L=6`, `K=21` run
`37d85ddb-c356-4c79-a6f4-d428828db039` passed 1/1 in 58.109 seconds with 36
construction authentications. Candidate-to-source construction took 17.4507
seconds, cursor initialization 16.756 microseconds, and first-residual search
832.37 microseconds. The census remained 49 loci, 36 attempts, 15 Certified,
21 Unsupported, 30 decisions, 19 free loci, and a 1,841-byte peak cursor; all
524,288 completions were checked. Independent K21 run
`e00cdbea-6312-4fb3-9856-0c2f3bf2ef25` also passed in 56.359 seconds.

The old two-stage run `e7378e6e-5df5-47c3-8fe9-686bbaa8ef30` took 72.935
seconds, spent 17.29 + 16.21 seconds in its two construction phases, and
performed 72 construction authentications. The new fixture's explicit source
and path stress-validation replays took 18.51 and 17.57 seconds; these are
deliberate authentication checks, not production direct-search cost.

The same pushed checkpoint advances the authority to normalized-source V2. It
binds one explicit `IntegralOrderingPolicy` into every source, even an
empty-attempt source, and authenticates every present candidate policy.
Owner-focused run
`8ad499a3-339e-4e0b-a04f-ccf754406516` passed 21/21 tests, formula/residual
run `6a5267d1-fe75-4854-8b98-9a03b1bb2370` passed 14/14, and independent
audit/validation run `430af297-b806-431e-a169-bd0f19a9f9c8` passed 30/30.
The policy-bound all-36 `L=6`, `K=21` run
`88a73ec1-52c2-4771-8a21-75e1b2a848b6` passed 1/1 with 36 construction
authentications, the unchanged 15 Certified/21 Unsupported semantics, and a
1.405-millisecond first-residual search.

This checkpoint completes the generic Direct singleton authority hand-off.
The terminal stable-value identity is allocation-independent across the whole
authenticated source/path/terminal chain; it emits the authenticated row span
once and represents subsequent occurrences with typed identity references.
Direct authority carries that identity through generated ordering V3,
physical frame V2, solve-plan V2, and source-profiled V2 target, database, and
session owners without fabricating V4/V5, Boolean/DPLL, integer-system, or
legacy inventory certificates. Mathematical stable-value identity remains
distinct from proof ownership: replay separately requires the exact retained
terminal, authority, frame, plan, and catalog `Arc` allocations. The old exact
relation compiler remains legacy-only; it is not the Direct production route.

Freshly generated full-cylinder and constrained Direct sources now pass through
chronological row staging and recentering to the existing unpublished
`ReadyForConditions` boundary. V2 accepts authenticated selector-independent
compact affine maps, proves exact descent inside the source chamber, retains
inactive-orthant hazards, and provides owner-allocation-sensitive replay. The
natural constrained production regression has six RHS terms and replays all
six descent witnesses; an additional test preserves the comparison under a
4096-bit common free-coordinate translation. Independent licensed default-GMP
run `b60b4fbd-f7b9-4656-ade0-6a476a7b7805` passed 18/18 focused tests with four
workers, and `cargo check --tests -j4` passed. The condition-plan foundation
subsequently passed 6/6 focused tests with four Nextest workers; the independent
parameter-identity projector passed 6/6 focused tests with four Rust test
threads. Independent combined run
`f6c4a9e7-fcc1-4c48-ae3c-5f2c0d781e42` passed 22/22 tests with four Nextest
workers, including affected Ready/session regressions. Allocation-independent
arbitrary-width OR-of-AND routing subsequently passed 12/12 focused tests. The
source-neutral affine-boundary kernel passed 8/8 focused tests for exact
arbitrary-width values, compact/identity maps, zero/divisible/nondivisible
numerators, malformed inputs, panic recovery, and exact/one-below limits. A
licensed default-GMP parallel library run at that pre-partition checkpoint,
including both kernels, passed 1091/1091. The owner-bound materializer then
passed 16/16 focused tests with
default GMP and four Rust test threads. Its production-derived sector-011
acceptance owner has seven mapped sources, four exact hazard ranges, and five
ordered boundary events: one suppressed by its numerator and four retained.
The suite covers exact/one-below owner and boundary limits, global retained and
compilation-peak accounting, retry ownership, replay, and foreign-owner
rejection. The relative `WhenBad` partition seam for the assembled current-
lineage arbitrary-width OR-of-AND formula is now implemented without
reconstructing V4, V5, the live-leaf queue, or Boolean/DPLL certificates. Its
focused licensed default-GMP gate passed 8/8 with four Rust test threads. The
subsequent canonical-locus owner gate passed 19/19, with an independent 20/20
superset. Authoritative licensed default-GMP Nextest run
`e9004c32-5a51-4705-a2f9-e39bcac40c49` then passed 1,651/1,651 tests with four
workers (52 slow), with 5 additional configured cases skipped; the following
doctest phase also passed.
No Direct input has
produced a published guarded rule or reduced a physical topology, and the
successful `K=21` cursor fixture still has not reached Ready or established
six-loop support.

The target-consuming compact application-event commit, its event-owned
zero-copy rule/residual projections, the exactly-once acceptance handoff, and
the algebra-free epoch owner with bounded retry-only exceptional leases are
now implemented in the worktree. This is not a closed rule publication system:
there is no admitted applicable-provider result path, stable-key result
staging/charge transfer, event-derived fresh narrowed-domain source ingress,
rejected-candidate continuation, provider application, publication-event audit
replay, or subsector feedback yet. The mature
`GeneratedResidualAffine...` implementation is an oracle, not production
authority for these missing pieces. RustRed's stated capability goal,
arbitrary one-loop pentagon reduction, and the high-throughput two- through
six-loop vacuum milestones therefore remain pending.

One of the implemented partition seam's two explicit high-loop performance
blockers is now closed. The outer bounded proof seals a non-cloneable,
schema/context-authenticated canonical-locus table; the trusted nested compiler
validates it linearly and skips the duplicate pairwise Symbolica-associate
scan, while the raw constructor remains a defensive/test path. A fresh compile
or complete proof replay still performs one outer exact/associate proof.
Symbolica's public monic `K[n]` normalization can later provide an indexed key
for that remaining scan, but currently exposes no fallible workspace census.
The same API gap affects rational-polynomial and projected `K[n]` division.
Consequently the resource-bounded arbitrary core currently performs complete
exact splitting without divisibility-based pruning; its public V1 compatibility
path is unchanged. A future censused Symbolica division seam must feed a bounded
ordinal-pair index before this optimization is restored.

The former `src/exact.rs` blocker is complete: exact scalar and matrix algebra
now crosses Symbolica's public GMP `Rational` and `Matrix<Q>` APIs. Continued
Phase B/C work must keep applying the same Symbolica-first rule. See the
[`Symbolica exact-linear-algebra API inventory`](symbolica_exact_linear_algebra_api_inventory.md)
and the
[`Symbolica-first algebra migration audit`](symbolica_first_algebra_migration_audit_2026-08-24.md).
The newer
[`Symbolica-only production algebra compliance roadmap`](symbolica_only_algebra_compliance_roadmap_2026-08-27.md)
promotes the remaining reachable handwritten parametric/concrete elimination,
generic Feynman-polynomial and family-product kernels, case transformations,
integer-lattice primitives, and the later tensor path to a six-loop P0 gate.
This is migration debt, not a claim that the current exact-group native reducer
or publication-epoch owner is incomplete in the narrower behavior already
tested. Integer affine parameterization remains an explicit public-API gap:
Symbolica exposes neither SNF/HNF nor a complete integer kernel basis, so the
topology-neutral semantic controller may remain while available arithmetic
primitives migrate.
Native dense and sparse solves must replace the older custom
`exact_sparse_elimination` wherever the public API applies. The checked
`SparseRowReducer` adapter now establishes the required sentinel, ordered-`L`,
work-limit, and panic contracts and is the live exact-database transcript
authority. The database owns the complete easiest-first physical-key catalog
and clone-on-stage Full-L reducer; Symbolica decides ordered reductions and
disposition, while guarded Rust replay preserves provenance and validates the
returned factors, divisor, normalized row, and outcome. The complete historical
U/L/pivot prefix and every independent successor's appended normalized U row
are authenticated coefficient-for-coefficient before the move-owned successor
commits. Independent regenerated-residual checks remain mandatory. Its checked
field now owns an `Arc` coefficient context and shares a
`Send + Sync`, serialized per-stage ledger controller across clones. Stage
cleanup covers success, typed checked-field abort, and unrelated unwind panic,
with focused ownership, concurrency, inactive-access, and retry tests. The
retained adapter now owns one admitted context `Arc` and performs live clone-on-
stage column insertion and candidate submission without replaying historical
input rows. The old exact-database rebuilding glue/use is compiled only as a
`cfg(test)` oracle; the generic legacy adapter remains compiled outside the live
database path.
Licensed default-GMP four-thread tests pass 15/15 retained-adapter, 18/18
complete sparse-adapter, and 41/41 exact-database cases. Parallelism is across
independently controlled campaign case lanes/shards in the planned executor,
not inside one ordered reducer. RustRed must not build a second CAS or matrix
layer.

## 2. Normative LiteRed semantics

### 2.1 Three distinct state layers

LiteRed keeps three semantically separate layers. RustRed must preserve this
separation even though it makes their successful transition atomic:

1. **Algebra database.** `Solvej` repeatedly reduces only the current hardest
   known integral. It normalizes and installs the first unknown hardest pivot
   before target matching or `WhenBad`
   (`vendor/LiteRed2/Source/LiteRed2026.m:2164-2195`). A candidate later
   rejected by `WhenBad` remains an algebraic pivot and may reduce subsequent
   source rows.
2. **Ordered target state.** `SolvejSector` scans the persisted `cases` order
   and selects at most the first matching case after recentering
   (`:2430-2435`, `:2484-2486`). A certified rule consumes the selected coarse
   target. A rejected candidate does not. RustRed's equality-refinement
   typestate is a safe extension: it must defer the group into a refined epoch,
   not skip the first target.
3. **Published coverage and residual work.** Applicable guarded rules and bad
   subdomains are not the algebra database. A mixed accepted rule removes its
   coarse case, publishes the good subdomain, and places the exceptional
   subdomain into later work (`:2488-2500`, `:2519-2523`).

Append-only row/candidate events belong to the third owner layer as replay and
diagnostic authority. They do not substitute for either database pivots or
target dispositions.

### 2.2 Recentering and first-target order

For the coordinates symbolic in the current affine group, LiteRed applies

```text
n_i -> 2 n_i - r_i
```

where `r` is the returned pivot coordinate. Fixed coordinates are unchanged.
The current exact kernel expresses the same operation in local geometry as

```text
t = r - A r_F
delta_F = -r_F
q = s - r
```

and must remain the only production arithmetic path. After recentering, scan
unresolved targets strictly by persisted solve ordinal. Never use hash/set
iteration, equal-offset tie breaking, or a later Ready target when the first
matching target requires equality refinement.

### 2.3 Exact LiteRed bad formula

Let the recentered right-hand side, after equal integrals have been collected
and exactly-zero coefficients removed, be

```text
R(n) = sum_t b_t(n, lambda) J(a_t(n)).
```

Here `n` are integer indices, `lambda` are algebraically independent
parameters, `c_i in {0,1}` is the target-sector corner, and
`Z = { i | c_i = 0 }` is the set of inactive coordinates.
`CollectjList` performs the coefficient collection and cancellation
(`vendor/LiteRed2/Source/LiteRed2026.m:4132-4134`, `:4171-4202`).

For every factor `f` appearing in a coefficient denominator, LiteRed asks
whether the factor is identically zero as a polynomial in the independent
parameters:

```text
D_f(n) = AND_alpha [ coefficient_{lambda^alpha}(f(n, lambda)) = 0 ]
D(n)   = OR_f D_f(n).
```

With no parameters, LiteRed inserts a dummy parameter so the same operation
is defined. It does not automatically split every physical parameter locus
such as `d - 4 = 0`; the test is polynomial identity in the declared
parameters (`vendor/LiteRed2/Source/LiteRed2026.m:2565-2567`).
For example, `n+d` is never identically zero as a polynomial in parameter `d`
because its `d` coefficient is one; treating `n+d=0` as one pointwise
`Q(d)[n]` predicate would be an unsound over-partition. The Symbolica-native
implementation must project the denominator into its parameter-coefficient
vector (public `RationalPolynomial::to_polynomial(base_variables, true)` after
the authenticated exponent/variable conversion) and form one conjunction of
zero predicates for those index-polynomial coefficients. Factoring is useful
for smaller clauses but is not required for correctness in the integral
domain.

Run this projection only on final Ready coefficients after exact integral-term
collection and recentering, so cancelled terms cannot leave spurious bad
clauses. For the implemented nonidentity compact-affine pullback, use the public
Symbolica-backed `ResidualAffineCoefficientComposition::Available`: its
normalized `value` supplies the leak numerator, while its separately retained
pre-normalization `mapped_denominator` supplies the substitution-domain
condition. `ZeroMappedDenominator` is an all-domain terminal
`IdenticallyBad` candidate, not `Unsupported` or an operational failure. Never
project an earlier raw coefficient or reapply Ready's diagnostic translation.

For every RHS term, the raw inactive-coordinate leak locus is

```text
L_t_raw(n) = OR_{i in Z} [ a_{t,i}(n) >= 1 ]
L_t(n)     = SmartReduce(L_t_raw(n)).
```

Only inactive coordinates are leaks. An active coordinate reaching zero is a
valid pinch into a subsector. If any `L_t` is literal `True`, the whole
candidate is bad. Otherwise LiteRed decomposes `L_t` into alternatives `x`
and retains `x` when

```text
Expand(Numerator[b_t] /. ToRules[x]) =!= 0.
```

The leak part is the disjunction of retained alternatives:

```text
N(n) = OR_{retained (t,x)} x.
B(n) = SmartReduce(D(n) OR N(n)).
```

If reduction introduces any noninteger power, normally a radical, LiteRed
fails closed and replaces `B` with `True`
(`vendor/LiteRed2/Source/LiteRed2026.m:2565-2569`). `SmartReduce` performs
integer reduction under the sector orthant and conservatively returns `True`
for unrecognized residual structure (`:2573-2581`).

The published and exceptional domains of a target with premise `C` are

```text
applicable  = C AND NOT B
exceptional = C AND B.
```

LiteRed marks all of an alternative `x` bad when the coefficient numerator is
not identically zero after `ToRules[x]`; it does not generally emit
`x AND numerator != 0`. RustRed may use its existing explicit numerator-gate
partition to certify a strictly larger good locus, but that is a
Symbolica-native strengthening. It must be represented and replayed as a
proof, never assumed as literal LiteRed behavior.

### 2.4 Successful and failed candidate semantics

If `B =!= True`, LiteRed publishes the guarded rule, records `C AND B`, and
deletes the selected target case (`:2492-2500`). Thus a mixed partition still
consumes the original coarse target; its exceptional leaves are separate work.

If `B === True`, LiteRed publishes no rule, leaves the target case unresolved,
and excludes the exact candidate from being returned again. It does not remove
the already installed pivot from the algebra database (`:2501-2505`). A
consume-once committed pivot/event supplies RustRed's equivalent exclusion.

These outcomes also define two different future execution paths. Every
accepted exceptional leaf from a mixed publication starts a **fresh generic
IBP derivation epoch** over the narrowed domain `C AND leaf`. That epoch gets a
fresh case-lane database/reducer and regenerates or replays the generic family
IBP/LI sources in canonical order. Sharing the immutable source catalog is
allowed; continuing mutation of the database that produced the publication is
not. This is RustRed's structured counterpart of LiteRed2's clean/regenerate
semantics. The successor must also carry a monotone continuation witness: the
exact exceptional domain becomes later unresolved work, and a candidate proved
bad everywhere on that domain is excluded before later generic rows continue.
Simply restarting the same candidate order without that domain/continuation
state could rediscover the same partition forever. The implemented algebra-
free epoch owner preserves the narrowed domain and compact affine source
geometry behind a bounded retry lease. It does not construct or admit a fresh
database/reducer lane, regenerate generic IBP/LI input, carry the monotone
candidate-exclusion continuation witness, stage a result, or perform re-entry.

`IdenticallyBad` does not create such a leaf or epoch. Its pivot remains in the
same live database, the selected target remains unresolved, and later source
rows continue serially in that database and may be reduced by the retained
pivot. The consumed source/candidate is excluded from repetition. Rejected-
candidate continuation is still pending and must remain a separate path from
the fresh-epoch exceptional mathematical-ingress path.

No outcome in this seam infers a master integral.

## 3. Current ownership boundary

The only production input to exact `WhenBad` is the current move-only Ready
typestate from
`GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready`. It binds:

- the staged database transition and source cursor;
- the selected unresolved target and its affine geometry/premises;
- the exact recentered terms, shifts, and translated row guards.

The concrete premise authorities are
`GeneratedAffineResidualCasePremisesCertificate` and
`GeneratedAffineResidualCaseEqualityRefinementCertificate`
(`src/generated_affine_residual_case_premises.rs:385-535`). The retained
target capabilities are
`GeneratedAffineResidualGroupRetainedReadyExactTarget` and
`GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget`
(`src/generated_affine_residual_group_exact_targets.rs:1753-1855`).

Private constructors and non-`Clone` ownership prevent accidental cross-owner
mixing. Do not add public constructors from loose ordinals/locators, but also do
not add hashes, fingerprints, nonces, or binding tokens to duplicate the same
in-memory guarantee.

The exact compiler consumes Ready and returns a terminal object that continues
to own it. Dropping Ready or any later terminal commits nothing. Every
preparation error returns the same owning typestate unchanged.

Preparation borrows Ready while admitting resources, then moves it into the
result. A failed operational preparation returns Ready. Do not require
`catch_unwind` on callback-free private Rust code; reserve panic conversion for
actual native/caller callback boundaries. The live commit similarly prepares
all fallible successor storage before moving the owner into its infallible
mutation tail.

## 4. Explicitly forbidden old-authority reuse

The following older `GeneratedResidualAffine...` objects must not be adapted,
wrapped, or reconstructed by copying ordinals:

- `GeneratedResidualAffineWhenBadBinding`,
  `AuthenticatedGeneratedResidualAffineWhenBadInput`,
  `GeneratedResidualAffineWhenBadCertificate`, and
  `GeneratedResidualAffineWhenBadCompilation`
  (`src/generated_residual_affine_when_bad_compilation.rs:255-326`,
  `:1151-1227`, `:5391-6200`, `:6803-7197`);
- `WhenBadCertificate`, `WhenBadCandidateBinding`, `BoundaryHazardRange`, and
  the old `IndexShift` descent proof in `src/when_bad.rs`;
- old descent and pullback-table certificates in
  `src/generated_residual_affine_when_bad_descent.rs` and
  `src/generated_residual_affine_when_bad_pullback_gate.rs`;
- `GeneratedResidualAffineSequentialTargetState`, old group effective
  coverage, and its rule/residual locators
  (`src/generated_residual_affine_group_effective_coverage.rs:321-706`);
- old sector effective-coverage owner and residual-queue authority slots;
- `ConditionalConcreteAuthority::GeneratedAffine`, old conditional-rule
  specialization, and old generated provider publication.

Those types authenticate the old matcher/`Arc<ParametricRelation>` graph and
carry `IndexShift`, `i64` boundary coordinates, or old owner locators. They
cannot prove ownership of the new session transaction. They remain semantic,
differential, and resource-accounting oracles only.

## 5. Source-neutral kernels to reuse

Reuse is restricted to mathematics with no old authority payload:

1. The truth-routing idea in `src/direct_bad_formula.rs`, after generalizing
   `DirectBadFormulaClause` from its current one- or two-atom shape to a
   resource-bounded slice in a shared atom arena. A coefficient-denominator
   identity clause can contain arbitrarily many parameter-coefficient zero
   atoms, so the existing fixed-width clause is not a correct reusable core.
2. The relative target-domain partition core and
   `AffineWhenBadRelativePartitionCertificate` from
   `src/generated_residual_affine_when_bad.rs:382-518,863-1638`. A new exact
   outer owner consumes the sealed typed input and retains only the partition
   and mathematical provenance needed downstream.
3. Canonical Symbolica polynomial validation, associate detection, and
   deterministic first-seen deduplication algorithms from
   `src/generated_residual_affine_condition_accumulator.rs`. Its existing
   input/certificate and `Option<&IndexShift>` provenance are not reusable.
   The current bounded implementation projects each locus to `K[n]`, preserves
   deterministic first-seen order, and proves equality/association with exact
   Symbolica polynomial operations. The canonical-locus owner is now
   implemented, so the inner compiler borrows its result without repeating the
   pairwise scan. A monic-keyed hash index remains
   deferred until Symbolica exposes a fallible normalization API with a native
   workspace census: public `make_monic` supplies the mathematics but not the
   resource authority required by this path. No uncensused normalization or
   Rust-side fallback algebra is admissible.
4. Compact affine polynomial-composition algorithms used by
   `src/generated_residual_affine_when_bad_pullback_gate.rs`. Its old Ready
   binding, event table, boundary values, and certificate are not reusable.

The old `finite_boundary_hazard_range` and
`prove_uniform_same_sector_descent` (`src/when_bad.rs:3326-3654`) are
algorithms to audit/port to `Integer`, not functions or proof types to call.
Only its `InactiveSectorActivation` interval belongs to parametric `WhenBad`.
The old helper's `ConcreteIndexOverflow` intervals at `i64::MIN/MAX` are
artifacts of a concrete representation, not mathematical target bad loci. If
a bounded concrete application representation still needs such checks, put
them in the sealed application layer and never feed them into rule derivation,
target disposition, or the parametric bad formula.

## 6. Implementation phases and APIs

### Phase A: validated typed non-publishing commits and event ledger —
implemented foundation

The typed NoTarget commit, consuming equality suspension, private append-only
chronological event collection, cumulative resource accounting, and
fresh-shadow audit described in the checkpoint are the validated foundation of
this phase. The current private event disposition also includes a compact
`Publication` event. Publication-event audit replay and the remaining terminal
variants are still pending:

```rust,ignore
enum GeneratedAffineResidualGroupExactRunDisposition {
    Running,
    RequiresAffineEqualityRefinement,
}

enum GeneratedAffineResidualGroupExactSessionEvent {
    Dependent { /* sealed source transition */ },
    NoTarget { /* sealed committed pivot */ },
    RequiresAffineEqualityRefinement {
        /* target locator + retained refinement certificate */
    },
    WhenBadIdenticallyBad { /* future sealed candidate */ },
    WhenBadUnsupported { /* future typed representation reason */ },
    Publication { /* compact application payload */ },
}
```

Names may follow existing conventions, but fields and constructors remain
private. Add consuming APIs whose failure retains the precise input type:

```rust,ignore
fn commit_no_target(
    self,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    outcome: GeneratedAffineResidualGroupExactSessionRecenterNoTarget,
) -> Result<
    GeneratedAffineResidualGroupExactSessionCommittedNoTarget,
    GeneratedAffineResidualGroupExactSessionCommitNoTargetFailure,
>;

fn commit_and_suspend_affine_equality_refinement(
    self,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    outcome:
        GeneratedAffineResidualGroupExactSessionRecenterRequiresAffineEqualityRefinement,
) -> Result<
    GeneratedAffineResidualGroupExactSessionSuspendedForRefinedEpoch,
    GeneratedAffineResidualGroupExactSessionSuspendRefinedEpochFailure,
>;
```

All concrete types remain crate-private.

`NoTarget` commits the pivot/cursor and advances target-state version with
`prepare_successor(..., None)`. It consumes no target and emits no rule. The
running session is consumed across this boundary: preflight failure returns
the original `(session, outcome)`, while success returns a typed committed
owner whose `into_session` is the only continuation seam. Thus even an
unreachable post-database invariant failure cannot leave a callable session.

Equality also commits the pivot/cursor without target consumption, but records
the refinement obligation and changes the run disposition so no further row
can be staged in this group. A fresh quotient/refined target epoch must start
with a fresh database; generic-field pivots must not be imported blindly into
the equality quotient. Equality is neither `IdenticallyBad`, a normal
rejection, nor a solved target.

The existing unconsumed kernel and database now use owning preparation plus a
preallocated event/target replacement and infallible final tail. Preserve that
seam and extend it to prepared rule and residual replacements for later
`WhenBad` outcomes. The suspended equality owner is itself the immediate
refinement authority; its chronological event is already recorded/replayed
without making the committed session resumable.

### Phase B: exact target geometry, descent, and hazards

The topology-neutral current-lineage modules are:

```text
generated_affine_residual_group_ready_publication.rs
generated_affine_residual_group_exact_when_bad_conditions.rs
generated_affine_residual_group_exact_when_bad_materialization.rs
generated_affine_residual_group_exact_when_bad_partition.rs
```

The exact target-geometry view is borrow-only and constructed solely from the
retained Ready target/session. It exposes target constants, corner, free
positions, compact affine map, premises, and ordering inputs without exposing
the transaction or allowing target selection.

Port signed same-sector descent to exact physical-key comparison. Add an exact
physical-frame helper that combines the target anchor offset with an
`ExactCenteredShift` and yields a
`GeneratedAffineResidualGroupPhysicalKey`/exact target-local key. Never call
`ExactCenteredShift::try_to_index_shift`. The exact lattice shift and ordering
foundation already exist in
`src/generated_affine_residual_group_physical_key.rs:384-605`; the new helper
must preserve that ordering rather than duplicate it.

Boundary endpoints, boundary values, interval counts, activation coordinates,
target constants, key aggregates, and descent witnesses use
`symbolica::domains::integer::Integer`. No generated value may round-trip
through `i64`, `i128`, `usize`, or `IndexShift`. A `usize` conversion is
permitted only after proving a nonnegative materialization count fits both
`usize` and its configured limit.

This exact boundary compiler enumerates only genuine inactive-coordinate
activation values. Active-coordinate pinches are valid subsector terms, and
`i64` concrete-overflow boundaries are application-representation checks, not
`WhenBad` hazards.

Represent every inactive activation interval lazily with exact `Integer`
fields. For an inactive target coordinate and centered RHS shift `q_i > 0`,
the interval is `first = 1 - q_i`, `last = 0`, `count = q_i`. An inactive
constant coordinate that is activated is an unconditional bad witness and
must be rejected before numerator gates. A separate bounded materializer may
expand a range only after comparing its exact count with the configured limit
and proving that count fits `usize`; a `2^4096` interval must remain one lazy
range rather than attempting `2^4096` allocations.

Add an exact-local physical-key path beside the legacy `IndexShift` input:
prospectively census `anchor_offset + ExactCenteredShift`, execute canonical
GMP addition, build both RHS and target keys through the existing physical
frame, and compare with the same private field-order routine used by `Ord`.
The descent witness records the first decisive key component so replay cannot
drift from the actual database order.

For nonidentity target maps, expose the already-computed exact affine constants
from `RecenterReady::target_offset` through a narrow borrowed view and combine
them with the authenticated free positions and compact matrix in
`ResidualAffineCompactMapView`. `execute_target_offset` has already computed
`p-A p_F`; condition compilation must not recompute those constants from the
physical anchor. Independent cylinders skip this composition because their
Ready coefficients and row guards are already centered.

### Phase C: owning exact `WhenBad` compiler

The implemented Phase C slices are recoverable, owner-bound, non-publishing
transitions. The materializer owns mapped guards and
coefficients, dual denominator projections, exact specialized boundary events,
replay evidence, and complete resource statistics. The next owner consumes
that move-only payload only after successful preflight, assembles and routes
the current-lineage arbitrary-width OR-of-AND bad formula, and returns either
`GeneratedAffineResidualGroupExactWhenBadPartitionCompilation::ReadyForPublication`
or its proof-bearing `IdenticallyBad` terminal. These names describe a sealed
nonpublishing partition result, not a certified or published rule: both consume
no target and publish no rule. The following compact preparation now consumes
`ReadyForPublication` and returns one non-`Clone` `PreparedPublication` with
move-only commit state, canonical loci, final relative cases, and a one-byte
applicable/domain/leak tag per leaf. Operational failure returns the exact
input owner. Derivation transcripts are discarded after this application
payload has been sealed.

Do not add another `Certified` wrapper between preparation and commit. The
partition compiler has already decided whether the candidate has an applicable
leaf, and `PreparedPublication` already owns everything the commit needs.
`IdenticallyBad` remains the nonpublishing partition terminal. A deterministic
unsupported symbolic representation remains a typed result at the phase that
detects it; it does not justify a new manifest, transcript, or replay layer.

A successfully reproduced LiteRed fail-closed radical/noninteger-power result
has `B=True` and is therefore `IdenticallyBad`. An all-exceptional partition is
also `IdenticallyBad`, not a zero-leaf publication candidate. Operational
resource or allocation failures return the move-only input unchanged. A panic
in callback-free private Rust code is a programmer bug and must not be
relabelled as a retryable validation result.

### Phase D: atomic target disposition and compact publication event

The hot path accepts only the move-bound `PreparedPublication`. It performs one
freshness check against the consumed live session/database/target state, admits
all replacement storage, and then enters an allocation-free infallible move
tail. Algebraic replay remains an explicit audit/testing operation, not a
prerequisite repeated by every internal transition.

The implemented commit stores only:

- a chronological committed-event log;
- centered relation terms and the selected target locator/offset;
- canonical loci, final relative cases, and ordered one-byte leaf tags retained
  once by the committed event;
- equality-refinement records.

The publication event deliberately drops derivation row translation, row
guards, derivation statistics, source recipe, and pivot evidence. Those values
are neither needed to specialize the centered relation nor retained merely for
provenance. Optional audit replay may reconstruct derivation state later.

The present replacement `Vec<Arc<Event>>` and full target-disposition copy are
correctness scaffolding, not the six-loop storage design: repeated commits make
the former quadratic in event count and the latter proportional to
`events * targets`. Replace them with a chunked/persistent event log and shared
or paged copy-on-write target dispositions before scaling the foundry. The
publication seam must not add separate deep rule/residual vectors on top of
those costs.

The consuming publication commit API is implemented. It takes
`PreparedPublication` by value, checks that the selected live target/database
head is still current, prepares successor storage, advances the database,
consumes exactly one selected target, and moves one compact application event
into place. `IdenticallyBad` never enters this API: it keeps the target
unresolved and publishes no rule.

Rule and residual projections are now shallow views into one committed event.
`PublicationReceipt` retains one shallow event `Arc`, so work can survive later
mutable session epochs without cloning the centered row or relative partition.
The view exposes the immutable parent target premises, affine geometry,
target offset, pivot-term ordinal, loci, and relative cases. Each in-range
lookup is a total/exclusive applicable-versus-exceptional classification; it is
deliberately repeatable and does not claim exactly-once scheduling. After the
native-reducer migration, the owning queue is compiled as a sealed batch wave
from explicit `(outer job, exact session, PublicationReceipt)` inputs. It sorts
owners by `(outer job, exact session, event ordinal)`, consumes exactly one
event handle per publication slot, rejects duplicate event keys, and stores
only one byte of handoff state per leaf—never a per-leaf `Arc` or copied
row/partition. No insertion is allowed after compilation; newly discovered
work forms the next deterministic wave. A non-Clone ticket addresses
`(publication slot, leaf ordinal)`, and acknowledgement means only that the
designated consumer accepted the handoff—never rule application, exceptional
discharge, coverage, closure, terminal status, zero, or master status. Provider
admission/results and mathematical residual-source ingress remain pending. A
following algebra-free owner now consumes only a fully acknowledged wave,
reuses the one event handle per slot, and replaces the obsolete per-leaf
handoff bytes with compact applicable/exceptional flat indices plus one atomic
byte per exceptional source. Its exceptional leases are retry-only scheduling
handles: drop or unwind restores pending state, and quiescent recovery handles
a deliberately forgotten lease. Lease resolution is not a committed result,
application, discharge, or durable progress transition.

The production coordinator must make this handoff follow conjunctive
core-and-memory admission on a roughly 100-core/1-TiB EPYC target. `--n-cores`
is only the invocation-wide compute ceiling. The operator sets
`M_operational = --max-memory` with
`M_operational < M_physical`; the difference reserves OS, checkpoint,
allocator, and opaque Symbolica headroom. Before constructing its one pool, the
coordinator will calculate an effective width
`1 <= E <= --n-cores` whose fixed baseline includes the coordinator plus every
possible worker's stack, TLS, and warmed Symbolica Workspace reserve. Per-task
admission then adds the retained reducer, Symbolica/algebra scratch not already
covered by that baseline, and a bounded result cap. Only the deterministic
admitted subset is hydrated; the rest of the logical frontier remains compact
plan metadata, and idle cores are correct when RAM admits no additional owner.

Publication events and later committed-descendant material remain shallow,
shared owners. Tickets carry indices, not copied rows, partitions, or flattened
transitive descendant rule sets, so storage must never grow as
`O(workers * transitive payload)`. The implemented handoff bounds the number
and therefore the fixed-size retained envelope of live borrowed tickets. It
also reports and gates the retained event allocations transferred into the
wave. Shared exact-session authority, family plans, and catalogs remain a
campaign-baseline/deduplicated-owner charge; they are not multiplied by the
number of slots merely because several events reference them.

Memory accounting follows unique live allocations. A shared or transferred
allocation is charged once even if it changes ledger category or has several
borrowed handles; that charge moves atomically with ownership. Distinct old
and new reducer/session states both count while they coexist during a
transition. Each independently schedulable `(CampaignJobKey, CaseLaneKey)`
retains one serial Symbolica reducer; workers may carry the lane between waves
but never fork its live mutation merely because more cores are available.

These handoff and epoch-owner limits are component gates, not a standalone RSS
formula. The campaign coordinator must conjunctively sum its shared baseline,
deduplicated resident event charge, incremental handoff/owner compilation
peak, admitted worker scratch, and buffered-result/downstream-owner envelopes
before hydrating a wave. `--n-cores` remains only the invocation-wide ceiling:
on the roughly 100-core/1-TiB target the coordinator must choose a RAM-admitted
effective width before pool construction and leave cores idle whenever the
operational envelope cannot safely hydrate another owner.

The production coordinator must still put independently admitted permits on
worker result buffers and downstream exceptional-source owners. Those results
must be durably staged or merged and charged temporaries dropped before their
permits are released. Newly discovered exceptional or dependency work is not
hydrated by the current workers: it enters the next sealed, admitted wave.
The stable `CampaignWorkKey` used to index that result and the atomic transfer
of its memory charge from in-flight result to resident successor or durable
staged-result descriptor are both future coordinator work. Neither the current
low-level executor, handoff wave, nor algebra-free epoch owner implements that
result table or charge transfer, so this document does not claim RAM-bounded
provider execution or re-entry yet.

### Phase E: provider integration, persistence, and optional audit

The normal provider path reads the committed event directly and performs no
derivation replay. An explicit offline audit may independently reconstruct:

1. the exact physical source row and hardest-only staging;
2. the session/database/target predecessor state;
3. first unresolved target selection;
4. exact recentering and separation of premises from row guards;
5. condition, descent, pullback, direct-formula, and partition transcripts;
6. the committed transition identity and target disposition;
7. published applicable and exceptional locators/counts.

`PreparedPublication` remains an internal live-session owner and is never the
durable output. A sector may emit a durable rule shard only after its committed
events, exceptional queue, solved-subsector feedback, and declared integer
domain reach a proved coverage fixed point with a finite explicitly enumerated
terminal-key set (or finite products). Every terminal is user-selected or
independently certified zero/factorized; a positive-dimensional or symbolic
residual domain cannot be declared terminal. An
unsupported, resource-limited, interrupted, or merely free-column state is a
resumable incomplete workspace, not a master and not a loadable closed shard.

Before any durable format, the planned `CampaignPlan` is a topology-neutral,
non-authoritative planning value: it will record requested roots, exact job
identities, dependencies, and deterministic ready-job antichains, but will
contain no rules and cannot be opened as `Closed`. Its first implementation
slice will use exact family-representation identity and identity ingress only,
deduplicate a shared proper-subsector child, reject non-descending edges, and
return the same ready antichain independent of root insertion order. Verified
routing and cross-family transports extend this model later without changing
that boundary.

Multiple user starting topologies are compiled as one campaign DAG rather than
one flattened transaction. The topology-neutral `CampaignJobKey` is
`(convention, family, sector, ordering, coefficient specialization, domain,
terminal policy)`. Each canonical job owns one immutable closed shard;
verified ingress maps preserve every root, and strict dependency edges share
proper subsectors, factorizations, and rank-decreasing cross-family transports.
The bundle's canonical family ID is constructed only after verified routing,
denominator-order, and parameter canonicalization and excludes user/root names
and momentum-label aliases; the current label-sensitive family fingerprint
remains a representation/session identity and is not the cross-root dedup key.
Verified same-rank routing/family equivalences are collapsed before DAG
construction and retained as ingress aliases, never dependency edges. Shards
are written independently and the lightweight campaign manifest is installed
last, so extending a campaign can reuse already closed jobs without rewriting
unrelated shards.

Campaign merge is deterministic and transactional. Equal job keys with equal
payloads deduplicate; equal keys with different payloads conflict. Reusing a
root ID with different ingress conflicts. A shared child is stored once with
multiple incoming edges, while incompatible conventions or coefficient
contexts remain distinct unless an exact transport is verified. Same-rank
equivalences are ingress aliases, not dependency edges; a cycle, non-descending
edge, or incomplete shard rejects the proposed merge without changing the last
complete manifest.

Durable artifacts are an actual trust boundary. Every closed shard retains a
compact sparse source-combination/residual witness sufficient to check each
rule exactly against freshly regenerated generic IBPs; this is compiled after
closure and does not require the compact live event to retain its discarded
source recipe or pivot evidence. A rule composed with solved children also
retains and recursively replays every child's source witness plus the strict
subsector/transport path, or an equivalent flattened exact source combination.
It need not reproduce every internal
historical stage, and internal artifact formats may be replaced freely during
development.

Finalizing `Closed`, an explicit `verify --exact`, and the first trust of an
external artifact perform the compact exact admission: reconstruct every family
and coefficient context; replay ingress and dependency maps; regenerate the
generic IBP/LI sources and prove every rule residual zero in Symbolica; prove
recursive child-source witness closure and strict RHS/dependency descent; prove
complete routing of every declared
domain to rules or a finite selected/certified terminal set (including finite
products); and reject every cycle or unresolved route. Success may write a
local verification receipt bound to the disposable artifact checksum and exact
RustRed/Symbolica revisions.

An ordinary load of a locally finalized artifact performs only lightweight
schema/revision, convention, format-local checksum, and DAG structural checks,
and may reuse that exact-verification receipt. Any payload, convention, or
relevant implementation-revision change invalidates the receipt. Full source
regeneration is not repeated on every local load. Complete chronological
derivation transcripts, content addressing, canonical cross-revision byte
serialization, and signatures remain optional; a derivation transcript is not
an admission requirement. One-worker and multi-worker evaluation must have the
same mathematical semantics.

## 7. Condition provenance and ordering

The exact condition accumulator receives sources in this stable order:

1. authenticated target premises, in their persisted certificate order;
2. exact recentered row guards, in row order;
3. coefficient denominator conditions, pivot first via the retained
   `pivot_term_ordinal`, then RHS centered-term/descent order and deterministic
   factor order;
4. boundary and numerator-gate loci, retaining term/coordinate/event
   provenance.

Canonical associate detection and deduplication may merge equal predicates,
but the certificate records every contributing source. Equality predicates
must not be laundered into nonzero premises. Target premises define the target
domain and remain distinct from candidate-specific guard failures until the
partition compiler. Candidate guard complements become exceptional work.

Locus interning is separate from direct-formula clause membership. A projected
denominator contributes one arbitrary-width bad clause
`c_0=0 AND ... AND c_k=0`, with denominator-term and parameter-monomial
provenance on every interned locus. It does not contribute `k+1` independent
candidate-required guards: that would turn LiteRed's conjunction into an
incorrect disjunction. Row guards retain their own one-atom clause semantics.

An identically-zero pivot/coefficient denominator makes the candidate
`IdenticallyBad`. Unknown canonicalization or unsupported symbolic structure
fails closed to typed `Unsupported` only when it is a deterministic
representation result; an exception or exhausted limit is operational.

## 8. Transition matrix

| Typed outcome | Algebra database and cursor | Target state | Events/residuals | Published rule |
|---|---|---|---|---|
| `Dependent` | Advance source; no new pivot | Dispositions unchanged; state advances to the prepared successor | Dependent event | None |
| `NoTarget` | Commit pivot; advance source | All unresolved/consumed dispositions preserved | NoTarget event | None |
| `RequiresAffineEqualityRefinement` | Commit pivot; advance source, then stop group | Selected target remains unsolved; refined epoch required | Mandatory refinement event | None |
| Prepared publication, `B=False` | Commit pivot; advance source | Consume exactly selected Ready target | Compact publication event with a shallow owning handle | Zero-copy applicable-rule view; provider pending |
| Prepared publication, mixed `B` | Commit pivot; advance source | Consume exactly selected Ready target | Compact event, frozen exactly-once acceptance handoff, and algebra-free epoch owner with bounded `Pending -> Issued -> Staged` exceptional-source ownership; an authority-bound compact result batch stages admitted in-memory outputs fail-closed, while fresh mathematical ingress and durable/general result publication remain pending | Zero-copy applicable-rule views; provider admission/results pending |
| `IdenticallyBad` | Commit pivot; advance source | Selected target remains unresolved | Rejected-candidate event; no duplicate residual | None |
| Deterministic unsupported representation | Commit pivot; advance source | Selected target remains unresolved | Typed reason/requeue only; no duplicate exceptional residual leaf | None |
| Stale live state, allocation, arithmetic, or limit failure | Commit nothing | Unchanged | None | None |

A rejected pivot is committed once and is not offered to a second target. A
later source row is reduced by it and may solve the same unresolved target.
The persisted first matching target remains final for the current candidate.
The atomic database/target/event columns of the prepared-publication rows,
repeatable shallow rule/residual inspection views, exactly-once acceptance
handoff, and bounded algebra-free exceptional-source lease owner are
implemented. The compact exceptional path has authority-bound stable-key
in-memory result staging and atomic worker-to-resident charge transfer.
Applicable-provider admission/results, durable/general result publication,
fresh narrowed-domain mathematical ingress, same-database rejected-candidate
continuation, rule application, and closure remain future work.

## 9. Atomic owner transition

Before any mutation, prepare and admit all of:

- the staged database successor/replacement owned by the transaction;
- target-state successor, consuming the selected Ready target only for a
  prepared publication;
- run-disposition successor;
- complete event-log append/replacement for the currently selected storage
  backend;
- prospective compact event storage into which the already-owned application
  payload will move;
- aggregate statistics and every new retained/peak capacity.

Those database, target, event, and statistics preparations are implemented for
the compact publication event. Shallow event/domain projections are also
implemented. The algebra-free epoch owner adds compact retry scheduling without
duplicating the deep payload; admitted provider results and fresh mathematical
residual ingress remain to be added.

Only after all preparations succeed may the move-only commit tail:

1. commit the staged database row;
2. swap in the prebuilt target state;
3. swap in the prebuilt event log, scheduling state, run disposition, and
   statistics;
4. drop predecessor owners.

The tail performs no allocation, Symbolica operation, GMP arithmetic,
formatting, hashing, replay, or other fallible work. It must be panic-free in
release behavior. A preflight error returns the exact owning terminal. A
post-database invariant failure is a fatal internal-consistency error and must
be made unreachable through complete preflight; it cannot honestly return a
retry token after possible mutation.

## 10. Resource contract

Every phase is admitted before allocation or expensive native work. Resource
statistics and limits must cover:

- already-live staged transaction/database ownership, using
  `max(staged_live_prospective_retained_bytes,
  staged_live_observed_retained_bytes)`;
- current target-state and catalog combined ownership;
- the centered relation terms, target locator/offset, loci, cases, and one-byte
  leaf tags retained by the compact publication event;
- exact target constants, centered shifts, physical keys, descent comparisons,
  boundary endpoints/counts/values, and their GMP magnitude bits;
- Symbolica polynomial normalization, factor inspection, composition, and
  native temporary bytes;
- each retained child-certificate prefix plus current child scratch;
- complete WhenBad owner retained bytes;
- target predecessor/successor/catalog overlap;
- event/rule/residual/refinement replacement capacities and control blocks;
- owner-wide combined-live and native-scratch peaks.

The implemented Ready geometry/descent/hazard checkpoint reports an
incremental subphase census. Compact publication accounts for the retained
application event after dropping derivation-only row translation, guards,
statistics, source recipe, and pivot evidence. The implemented views allocate
no storage. Future queue accounting must cover only its shallow owner slots and
leaf-state bytes and must not charge or copy the event payload again.

Project every child compiler from the remaining aggregate budget. Never reset
a child to default limits. Before materializing boundary events, preflight the
complete exact cardinality, the `usize` conversion, value bit envelopes,
vector capacity, retained bytes, and native temporary work. Check prospective
minimum capacity before allocation and observed capacity afterward.

Do not double-charge shared `Arc` payload graphs, but do charge every new
control block, pointer slot, vector capacity, and uniquely owned transcript.
The target-state predecessor + successor + shared-catalog peak already has a
model in `GeneratedAffineResidualGroupExactTargetState::prepare_successor`
(`src/generated_affine_residual_group_exact_targets.rs:1210-1328`).

Every reported resource dimension requires an exact-limit success test and a
one-below transactional rejection test. Failures return the same owning
typestate with database, cursor, targets, events, capacities, and statistics
unchanged.

## 11. Ownership and boundary validation

- Public/debug views expose locators, counts, predicate classes, and resource
  census needed by their caller. Exact relation data remains owned by the
  committed event and is borrowed through application views.
- Move ownership prevents mixing routes, rules, residuals, and predecessor
  state from different in-memory candidates; do not add runtime identity tokens
  where the type system already enforces this.
- The live commit performs the one freshness check needed to reject a stale
  database head or already-consumed target.
- Durable loading validates external bytes before constructing in-memory
  owners. Optional audit replay and serialization round-trip tests live at that
  boundary, not in every forward transition.

## 12. Required implementation gates

### Phase A gates

- NoTarget commits exactly once, advances both versions, preserves every
  target, emits no rule, and drop remains inert.
- Equality commits exactly once, retains its locator/refinement proof, emits a
  mandatory refinement event, and prevents further same-epoch staging.
- Stale live-state, one-below, and allocation paths return the exact input
  outcome and mutate nothing.
- The normal typed API has no transaction extractor or untyped production
  commit bypass.

### Exact `WhenBad` gates

- Differential small-coordinate mathematical transcripts against the old
  lineage, used strictly as an oracle after erasing its authority.
- Exact shifts, target keys, descent, and boundary values beyond `i64`,
  including a `2^4096` case, with no downcast path.
- Cover zero denominator and exact parameter-identity semantics: `n+d` is never
  identity-bad, `n(n+d)` is bad exactly at `n=0`, and multi-parameter/full
  coefficient vectors, the zero-parameter dummy equivalent, clauses wider
  than two atoms, and one-below atom-arena budgets. Also cover
  active-coordinate pinch, inactive-coordinate activation, numerator
  vanishing/nonvanishing on a boundary, and multiple simultaneous hazards.
  Test LiteRed radical/noninteger-power fail-closed behavior separately as
  `IdenticallyBad`; a deterministic RustRed representation limit
  is typed `Unsupported`, while limit exhaustion or panic is operational and
  returns the owner unchanged.
- Empty collected RHS as a valid all-domain zero rule, not a malformed row or
  an inferred master.
- `CollectjList`-equivalent canonical collection: duplicate/equivalent RHS
  integrals combine before classification, including exact coefficient
  cancellation that removes a would-be denominator or leak hazard.
- Stable target-premise/row-guard/denominator ordering, associate deduplication,
  inherited truths, and no invalid converse implications.
- All-applicable, all-exceptional, and mixed applicable/exceptional partitions.
- Exact and one-below limits for every GMP, Symbolica, retained, scratch,
  cardinality, comparison, and publication resource.
- A topology-neutral arity-21, many-RHS, multiple-hazard Ready/condition stress
  row, without a topology name, loop-count dispatch, or injected recurrence
  coefficient.
- A generic six-loop family-generation gate proving `L^2=36` IBP source rows
  per seed, followed by a session/scheduler batch gate that consumes all 36
  sources. One arity-21 row is not a
  substitute for this distinct source-batch test.

### State/publication gates

The current milestone covers compact-event commitment, one target consumption,
freshness/resource failure, retained-payload accounting, event-bound zero-copy
rule/residual domains, exactly-once acceptance handoff, and algebra-free
applicable/exceptional indexing with bounded retry-only exceptional leases.
Applicable-provider admission/results, mathematical re-entry/continuation,
specialization, closure, and durable-audit gates remain pending where they
depend on those future layers.

- Full transition-table test covering Dependent, NoTarget, equality, prepared
  publication, IdenticallyBad, unsupported representations, and every
  operational-failure class.
- Rejected pivot remains in the database and reduces a later row that can solve
  the same target.
- Rejection does not try a second target; publication consumes only the first
  persisted matching Ready target.
- Mixed publication commits every applicable leaf and queues every
  exceptional leaf while consuming the coarse target exactly once.
- Queue construction rejects duplicate `(outer job, exact session, event)`
  keys but accepts identical local event ordinals from distinct sessions;
  exact and every-positive-one-below limits return all input receipts intact.
- Forward/reverse receipt order and forward/reverse acknowledgement order
  produce the same canonical handoff stream and state statistics. Duplicate,
  foreign, or unissued acknowledgement is rejected, and acknowledgement alone
  never changes a leaf's applicable/exceptional classification or proves
  discharge, coverage, closure, terminal status, or master status.
- Epoch-owner compilation accepts only a fully acknowledged quiescent handoff,
  retains one event handle per slot, and replaces handoff state with compact
  applicable/exceptional flat indices plus one atomic byte per exceptional
  source. Retry leases respect their live-count/live-byte ceilings, restore
  pending state on normal drop and unwind, and support explicit barrier-only
  recovery of a deliberately forgotten lease. Exact and every-positive-one-
  below component limits reject transactionally; none of these transitions is
  provider output, mathematical re-entry, application, discharge, or closure.
- A synthetic width-100/approximately-1-TiB coordinator gate computes `E`
  before pool construction, hydrates only the deterministic admitted
  `CampaignWorkKey` subset, charges shared/transferred allocations once and
  distinct old/new states throughout overlap, reserves every possible
  worker's Symbolica TLS/Workspace plus admitted scratch and bounded results,
  and deliberately leaves cores idle when memory is limiting.
- An optional non-CI soak on a real approximately-100-core EPYC/1-TiB host
  records `M_physical`, `M_operational`, effective `E`, warm-worker reserve,
  peak RSS, staged-result high-water mark, and idle-core time while matching
  the serial semantic hashes. The synthetic gate remains mandatory when that
  hardware is unavailable; a six-loop scalability claim requires the real
  named-host evidence.
- Applicable specialization reproduces the complete exact recentered
  relation coefficient-for-coefficient and shift-for-shift. Exceptional
  specialization is refused by the rule handle and routed to the matching
  residual authority.
- Failure injection before each prepared replacement leaves database, cursor,
  targets, run disposition, events, rules, residuals, capacities, and stats
  byte-for-byte unchanged.
- Live-state freshness and concurrency tests, plus tamper/serialization/audit
  tests only for durable artifacts.

All Rust test gates use licensed, GMP-enabled Symbolica and run in parallel.
No test or build enables `no_gmp`; no test invokes FORM. Concrete one-loop and
later vacuum/scattering topologies may validate the finished generic pipeline,
including numerator/denominator cancellation closure, but cannot replace the
topology-neutral unit, property, closure, and boundary tests above.
Artifact tests additionally compare deterministic single-worker and
multi-worker semantics while the surrounding test suite remains sharded and
parallel. Byte-for-byte equality is required only after RustRed deliberately
defines canonical serialization.

### Multi-start campaign-bundle gates

The scheduler boundary is specified in the
[parallel campaign-foundry design](parallel_campaign_foundry_design_2026-08-26.md).
One independently schedulable affine case lane owns one ordered retained
Symbolica reducer and mutates it serially. Separate lanes, sectors, families,
frozen-epoch exceptional case proposals, fixed modular samples, and immutable
verification blocks may run concurrently with case-lane-local checked-field
controllers. No additional hash, nonce, or ancestry layer is required at that
boundary: stable value keys, one workspace revision, move-only lane ownership,
and validation
at durable/global mutation boundaries are enough.

- A `CampaignPlan` with two parents sharing one exact proper-subsector job stores
  one child, exposes the deterministic ready antichain, and remains visibly
  distinct from any durable `Closed` bundle.
- Two routing/permutation-equivalent roots produce one canonical shard and two
  verified ingress maps; reversing root order or worker count preserves bundle
  semantics.
- Two inequivalent parents sharing a proper subsector or factorized component
  retain one child shard and two strict dependency edges, without flattening
  incompatible index or coefficient contexts.
- Parameter spellings deduplicate only through an explicit typed campaign ABI;
  incompatible metric, propagator-sign, unit-mass, ordering, or domain
  conventions remain separate or require a verified transport.
- An uncovered, unsupported, resource-limited, interrupted, or unresolved
  exceptional leaf prevents a `Closed` bundle and is never renamed a master.
  An exceptional leaf discharged by a strictly descending closed child or
  finite selected/certified terminal key is valid closed-shard content.
- Adding a root reuses unaffected shards; changing one child invalidates only
  reachable ancestors; failure before final manifest installation preserves
  the previous complete bundle.
- Equal job keys plus equal payloads merge idempotently; equal keys plus unequal
  payloads and equal root IDs plus unequal ingress maps are typed conflicts.
- Finalization, explicit `verify --exact`, or first trust of each unique
  external shard reconstructs its family/context and verifies ingress maps,
  strict rule/dependency descent, domain coverage, finite selected/certified
  terminals, and exact zero residuals against regenerated generic IBP/LI
  sources. A later ordinary local load may reuse the revision- and checksum-
  bound receipt after lightweight structural checks. Detailed source replay
  remains an optional audit.
- Same-rank routing/family equivalences become aliases before DAG construction;
  a cross-family dependency without a strict well-founded rank decrease, or
  any dependency cycle, is rejected.

### Physical six-loop derivation gate

The synthetic all-36 `K=21` source/frontier fixture remains a unit and stress
test. It does not satisfy the physical gate. Before optimized concrete
application is prioritized, the benchmark topology manifest is frozen before
execution. It uses actual GammaLoop/BPHZ roots when available; the inaugural
fallback corpus must include a QCD-valid connected 1PI quartic `K5` root
(10 physical lines, 11 ISPs) and a cubic 10-vertex/15-line representative such
as Petersen or a lower-symmetry graph (6 ISPs), with multiple non-factorizing
reachable sectors. Each root must construct its 21-coordinate family, process
all 36 sources, traverse shared lower dependencies, close every exceptional
route onto that finite certified or selected terminal set, and emit a
deterministic multi-start-ready shard DAG. No reachable `Unsupported`,
resource, timeout, uncovered frontier, or unresolved exceptional route is
permitted. Every emitted rule is checked by an exact residual against freshly
regenerated generic IBPs.

The benchmark records named hardware, release/GMP configuration, wall and CPU
time, peak RSS, rule/event/target/locus/case counts, queue peak, coefficient
growth, dependency and deduplication counts, artifact bytes, and 1/2/4-worker
scaling. A declared resource envelope is part of the acceptance result;
it is numerical, recorded in the frozen manifest before execution, and cannot
be relaxed post hoc. Exceeding it is a typed failed gate, never a master-
discovery heuristic. The provisional dedicated-host target is at most 48 GiB
peak RSS, 24 hours wall time per root, and 48 hours for a three-root bundle.
When the ready-job antichain exposes at least four independent jobs, four
workers must achieve at least 2.5x speedup over one; otherwise the manifest must
predeclare the measured critical-path exception. After the inaugural roots
close, a small GammaLoop/BPHZ-derived multi-root corpus must prove shared-shard
reuse and equivalent single-/multi-worker semantics.

## 13. Completion criteria and integration order

Implement in this order:

1. typed NoTarget commit and sealed equality suspension — completed;
2. extend the common fully prepared tail with the minimal chronological event
   ledger and owner-wide replacement preparation — completed for Dependent,
   NoTarget, and equality-refinement dispositions;
3. establish the replayable shared normalized-source owner, sealed fresh
   normalization, and bounded direct normalized-formula target-frontier search
   without materializing V4, V5, or the Boolean/DPLL owners — completed at
   pushed checkpoint `c593865`; licensed run
   `e7378e6e-5df5-47c3-8fe9-686bbaa8ef30` passed 10/10 including direct
   all-36 `K=21` residual search, but no Ready/reduction result;
4. one-pass candidate-to-normalized-source construction ahead of V4 with a
   safe sealed replay token — completed at pushed checkpoint `c593865`;
   focused run `b2ba7679-e7c8-4e64-ba25-c451024843bf` passed 6/6,
   independent affected run `db2a98a5-d473-4cdc-b2b7-fe2f444357e8` passed
   44/44, and primary K21 run `37d85ddb-c356-4c79-a6f4-d428828db039`
   passed 1/1 with 36 rather than 72 construction authentications;
5. bind one explicit `IntegralOrderingPolicy` into the normalized source,
   including the empty-attempt case, and authenticate all present candidate
   policies — completed in normalized-source V2 at pushed checkpoint
   `c593865`; focused runs
   `8ad499a3-339e-4e0b-a04f-ccf754406516` (21/21) and
   `6a5267d1-fe75-4854-8b98-9a03b1bb2370` (14/14), independent run
   `430af297-b806-431e-a169-bd0f19a9f9c8` (30/30), and policy-bound K21 run
   `88a73ec1-52c2-4771-8a21-75e1b2a848b6` (1/1) passed;
6. add the generic direct-backed singleton affine adapter — completed through
   source-profiled exact-session staging, chronological replay, recentering,
   and the existing unpublished `ReadyForConditions` boundary for authenticated
   selector-independent compact affine maps. The terminal stable-value identity emits the row span
   once through typed references; ordering V3, physical frame V2, solve-plan
   V2, database V2, catalog/state V2, and session/event V2 retain it without a
   fake inventory, while exact terminal/authority/frame/plan/catalog `Arc`
   ancestry remains a separate replay condition. Constrained production rows
   now enter Ready without sampling away their compact geometry. Focused and
   independent licensed default-GMP tests are green. The owner-bound
   identity/compact condition schedule and Symbolica physical-parameter
   identity projection are also implemented, as are source-neutral
   arbitrary-width formula routing and exact affine-boundary
   mapping/divisibility. The owner-bound non-publishing materializer now maps
   the scheduled conditions and coefficients, retains both denominator
   projections, and owns exact specialized boundary events. The following
   move-only owner now builds and replays the relative `WhenBad` partition of
   that current-lineage arbitrary-width OR-of-AND formula. One consuming
   preparation now distills move-only commit state, loci, cases, and one-byte
   guarded/exceptional tags. The atomic exact-session transition now advances
   the database, consumes one selected target, and stores the compact
   application event. A shallow event owner now exposes zero-copy rule/residual
   leaves and complete event-bound domains. This has not queued or applied a
   rule, completed publication audit replay, reduced an integral, or reached
   six-loop topology support;
   retain the MTBDD only as a
   compact-case/repeated-query backend under its own measured construction
   budget;
7. exact-`Integer` geometry, descent, boundary, condition, and pullback cores —
   selector-independent compact-affine geometry, fixed-chamber descent/lazy
   hazards, an owner-bound transform/source schedule, and physical-parameter
   identity projection are implemented; arbitrary-width direct formula
   construction/routing and source-neutral exact affine-boundary divisibility
   are implemented as reusable kernels. Owner-bound coefficient/guard mapping,
   dual denominator projection, exact boundary-event specialization, and the
   owner-bound relative partition of the current-lineage arbitrary-width
   OR-of-AND formula are also implemented. Move-bound compact route preparation
   and the atomic compact application-event commit are implemented;
8. **Completed through live retained native state:** the historical temporary
   per-stage rebuilding bridge first replaced handwritten row decisions with a
   public Symbolica `SparseRowReducer`/`LuLMode::Full`; that bridge is now
   superseded in production and remains only as a `cfg(test)` differential
   oracle. The live database owns the complete easiest-first physical-key
   catalog, admitted context, Full-L reducer, and sentinel. A stage clones the
   native state, inserts only newly discovered columns, submits one candidate,
   and commits only an independent move-owned reducer/catalog successor. The
   unused sentinel preserves dependent transcripts at full physical rank;
   nonmonotone chronological `L` row indices retain physical traversal order;
   and the prospective `U/L` envelope is admitted before native entry.
   Symbolica is authoritative for factors, normalization, pivots, and
   disposition. RustRed authenticates the complete historical U/L/pivot prefix
   and the independent trial's appended normalized U row coefficient-for-
   coefficient while retaining guards, provenance, transactional failure, and
   resource admission. Licensed default-GMP four-thread suites pass 15/15
   retained-adapter, 18/18 complete sparse-adapter, and 41/41 exact-database
   tests. The checked field's shared controller still serializes one fresh
   ledger per stage and cleans it after success, typed abort, or unwind panic.
   Every stage still deep-clones the full native reducer, forward elimination
   remains serial, and opaque native heap/scratch bytes are not byte-censused.
   Export and profile those costs rather than presenting this as physical-
   topology reduction, Vakint reproduction, or six-loop scaling. Run
   independently controlled shard/case reducers in parallel rather than
   claiming intra-reducer parallel forward elimination;
9. implement the non-durable topology-neutral `CampaignPlan` slice with exact
   representation-level deduplication, identity ingress, one shared proper-
   subsector child, cycle/non-descent rejection, and a deterministic ready-job
   antichain; build on the algebra-free exceptional lease owner with RAM-
   admitted results and stable-key charge transfer, sealed mathematical source
   ingress into fresh narrowed-domain generic IBP epochs, same-database
   rejected-candidate continuation, and solved-subsector feedback; iterate
   those queues to a proved coverage fixed point
   with exact regenerated-IBP residuals and a finite enumerated selected/
   certified terminal-key set; only then construct an immutable closed family/
   sector shard;
10. replace the quadratic event/target replacement storage, add unit-mass
    specialization and required Symbolica-backed acceleration, and preserve
    resumable incomplete workspaces separately from closed shards;
11. extend the earlier `CampaignPlan` with verified routing, canonical job
    identity, and shared subsector, factorization, and cross-family dependencies;
    then compile closed shards into deterministic multi-start campaign bundles
    with verified ingress maps;
12. derive the complete Vakint one- through four-loop replacement-system
    corpus without FORM or copied recurrences, using a minimal generic
    application seam only for exact external-oracle comparisons;
13. pass representative five-loop and then physical nontrivial six-loop
    derivation-only closure, resource, and parallel-scaling gates before
    optimizing concrete application;
14. implement the high-throughput provider/application runtime plus optional
    publication-event audit replay and continue topology-based validation from
    one loop upward. Full derivation replay remains an optional audit.

A phase is complete only when its success disposition, retry ownership,
resource envelope, semantic/ownership tests, exact/one-below tests, and parallel
GMP test gate all pass. Durable/import boundaries additionally require load and
optional audit tests. No phase may bridge a missing proof by
constructing an old-lineage certificate, hard-coding a topology, downcasting
generated arithmetic, invoking FORM, or treating a failed candidate as a
master integral.
