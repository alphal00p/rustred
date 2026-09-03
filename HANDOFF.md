# RustRed session handoff — 2026-09-03

## 1. Authority, purpose, and evidence discipline

This file is a session snapshot for the next implementation session. It is
not a replacement for [`GOAL.md`](GOAL.md): `GOAL.md` is authoritative when
the two differ. The active product objective is Stage 1—closing and shipping
the one-, two-, and three-loop single-scale vacuum parametric-IBP families and
validating Vakint's FORM-less RustRed scalar backend through three loops—then
continuing directly into the already-authorized Stage 2 high-loop program.

The tool-managed goal is still active and contains an older final clause saying
to stop after Stage 1. That goal cannot be edited in place. When Stage 1 is
actually complete, mark that managed goal complete, create a new managed goal
for Stage 2 from the authoritative wording in `GOAL.md`, and continue. Do not
mark the present goal complete merely because this session stopped.

The central evidence rule is strict:

- A bounded search hit, modular relation, owner-cover shrink, finite sample,
  or resource stop is not closure.
- Janet queue exhaustion alone is not family closure if the monomial
  complement remains infinite.
- A K6 artifact may be called closing only after the complete exact queue is
  exhausted, the exact complement is finite and fully enumerated, every
  terminal is explicit, all rows and zero-obligation witnesses cold-replay,
  and the serialized artifact cold-loads and reproduces the same authority.
- Minimal masters are desirable but not required. A finite, universal,
  manageable nonminimal terminal basis is acceptable; completeness is not
  negotiable.

The latest user request stopped implementation because of quota and requested
this extensive handoff plus a simple handoff. All partially written code from
the just-opened next slice was deliberately removed. The RustRed tree was
returned to the clean, fully tested pushed milestone before these two handoff
documents were added.

## 2. Repository and Git snapshot

RustRed repository:

- Path: `/common/dev/rustred`
- Branch: `main`
- Remote: `origin`, `git@github.com:alphal00p/rustred.git`
- Last code milestone before this handoff: `759ab1c` — `Build persistent
  exact-lazy Janet foundations`
- Its predecessor: `b4c7ba5` — `Build exact-lazy Janet coefficient
  foundations`
- `759ab1c` is already pushed to `origin/main`.
- The full workspace test suite for `759ab1c` passed after the push.
- The only intended changes after `759ab1c` are this file and
  `HANDOFF_simple.md`; verify that assertion before committing.

Every Git operation must be run with the requested identity, including status,
commit, and push commands:

```bash
git -c user.name=ValentinHirschi \
    -c user.email=valentin.hirschi@gmail.com <operation>
```

Never stage or commit anything below `FOR_REFERENCE_ONLY_DO_NOT_PUSH` in the
RustRed repository. Do not use destructive resets/checkouts on user work.

The GammaLoop/Vakint reference worktree currently reports:

- Path: `/common/dev/rustred/FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop`
- Branch: `vakint_rustred`
- Status at handoff time: `ahead 34, behind 13` relative to
  `origin/vakint_rustred`
- It has uncommitted modifications in `Cargo.lock`, `crates/vakint`, two
  shipped RustRed artifacts, FeynKit tensor files, and Vakint tests.
- Those changes were not altered, cleaned, committed, rebased, or pushed in
  this final handoff step. Audit ownership and intent before touching them.
- The feature branch is intended to be based on GammaLoop's `feynkit` branch,
  retain Vakint backward compatibility, and use a local-path RustRed dependency
  while developing. Pushed milestones must pin the matching RustRed Git
  revision.

## 3. Environment and reproducible commands

Use the Nix development shell for builds:

```bash
nix develop --command cargo <args>
```

Licensed Symbolica work needs the current operator-provided
`SYMBOLICA_LICENSE`. The old value in the verbatim historical preamble of
`GOAL.md` is expired. Export the newer value supplied in the conversation or
ask the operator for a refreshed value; do not silently run licensed scaling
experiments with the old one. The literal is intentionally not duplicated in
repository documentation.

Core verification commands used for the last milestone were:

```bash
export SYMBOLICA_LICENSE='<current operator-provided value>'
nix develop --command cargo fmt --all -- --check
git -c user.name=ValentinHirschi \
    -c user.email=valentin.hirschi@gmail.com diff --check
nix develop --command cargo check --workspace --all-targets
nix develop --command cargo test -p rustred exact_lazy --lib -- --test-threads=1
nix develop --command cargo test -p rustred \
    foundry::completion::involutive --lib -- --test-threads=1
nix develop --command cargo test --workspace --all-targets -- --test-threads=1
```

Last verified results:

- Exact-lazy focused suite: 72 passed, 0 failed.
- Complete involutive subsystem: 196 passed, 0 failed.
- RustRed core full suite: 1,237 passed, 24 explicitly ignored offline/release
  diagnostics, 0 failed; it took about 1,145 seconds in the dev test profile.
- All remaining application, CLI, examples, integration, and Python test
  binaries in the workspace command also passed.
- `cargo check --workspace --all-targets`, formatting, and diff checks passed.
- Only deprecation warnings from vendored Symbolica's SIMD dependency were
  observed; the code milestone itself introduced no compiler warning.

Do all performance measurements and K6 campaigns in a release build. Debug
timings are not evidence. Compilation time must be reported separately from
campaign wall time, but the CLI and Python APIs should be kept current so
experiments do not require editing/recompiling a Rust harness for every run.

## 4. User constraints that must remain active

- Implement CAS work in Rust using Symbolica. Before adding any algebraic
  primitive, inspect Symbolica's public API and use its optimized operation if
  it exists. Do not implement a second CAS inside RustRed.
- Never use Mathematica, SymPy, or FORM in RustRed or in Vakint's RustRed
  scalar reduction backend.
- Existing FORM-based Vakint methods remain supported and may be used as
  authoritative development oracles. FORM5 is available in the reference
  tree for those comparisons.
- Stage 1 tensor handling uses the collaborator-supplied FeynKit tensor
  prepass. Do not resume work on RustRed tensor reduction until the planned
  collaborator technology is available.
- RustRed needs no backward compatibility during deep development. Remove or
  reshape obsolete RustRed APIs and artifact schemas when needed. Vakint's
  public API and pre-existing evaluation methods must remain backward
  compatible. Vakint does not need compatibility with old experimental
  RustRed artifact schemas.
- The Python package must be imported as `import rustred`; `_rustred` may remain
  only the internal extension module.
- Keep the Rust library, CLI, and Python APIs useful for fine-grained generic
  tasks, including non-vacuum topologies. Algorithms must stay generic in
  topology, loop count, and rank even when an automatically selected lane is
  optimized for unit-mass vacuum families.
- `parameter(...)` declarations are optional metadata and must be inferred
  where possible. Examples should show the smallest necessary inputs.
- Avoid repeated expensive artifact authentication in hot reduction paths.
  Validate once at the untrusted load boundary, then use immutable typed
  authority internally.
- Parallel execution must ultimately be deterministic and RAM-conscious.
  Do not clone the full symbolic state per worker. Immutable shared epochs,
  bounded probe-local work, deterministic result ordering, and explicit
  memory telemetry are the intended basis. The current exact-lazy completion
  mutation path is deliberately serial; only immutable probe evaluation is a
  candidate for parallelism until correctness is sealed.
- Delegate independent implementation, research, and adversarial audits at
  each meaningful milestone. Keep at least one auditor separate from the
  implementation owner.
- Commit and push clean intermediate milestones frequently with the required
  identity.

## 5. What RustRed can currently do

The repository cleanup/refactor is complete. It is a virtual Cargo workspace
with:

- `rustred` / `crates/rustred-core`: topology-neutral mathematical engine;
- `rustred-app`: application and CLI layer;
- `rustred-python`: PyO3 package exposed to users as `rustred`.

The core currently provides:

- authenticated generic integral families and exact indexed coefficients;
- automatic ordinary IBP and LI source generation;
- unit-mass/single-scale presentation with exact homogeneity restoration;
- topology-neutral sector geometry, exact zeros, symmetry/canonical routing,
  factorization, and deterministic integral ordering;
- exact source replay, guarded parametric rule generation, immutable artifact
  ownership, and strictly descending memoized application;
- bounded modular source discovery and exact promotion boundaries;
- coefficient-free Janet geometry, indexed Janet division, leading ideals,
  complement partitions, and pure-power finiteness witnesses;
- a tested exact eager Janet/Ore semantic oracle;
- the new exact-lazy coefficient/provenance/guard DAG and persistent epoch
  foundation described below.

Closed production families already exist for:

- `K = 1`: one-loop tadpole family;
- `K = 3`: two-loop equal-mass sunset, including its pinch/factorization
  dependency.

Here `K` is the number of independent inverse-propagator/ISP coordinates in
the family, not the number of loops. For a complete scalar-product basis of
an `L`-loop vacuum family, `K = L(L+1)/2`; therefore 1L, 2L, 3L, 4L, 5L, and
6L correspond to `K = 1, 3, 6, 10, 15, 21` respectively.

The Rust library, `campaign` CLI, and `import rustred` Python API can generate,
inspect, serialize/load, and apply the current one- and two-loop closing
artifacts. The repository examples are organized under `examples/rust`,
`examples/cli`, and `examples/python`, with the two-loop single-mass vacuum
workflow documented in the README.

## 6. Stage 1 status

### Complete or substantially complete

- Clean repository/module refactor and removal of legacy compatibility burden.
- Generic ordinary-source generation and exact replay.
- Immutable versioned artifact infrastructure and deterministic reduction
  primitives.
- Complete one-loop and two-loop unit-mass artifacts.
- Mass restoration by dimensional homogeneity, including the coefficient
  factor `(m^2)^(sum(master powers) - sum(target powers))`.
- Rust/CLI/Python generation, inspection, and application surfaces for the
  existing artifacts.
- A Vakint RustRed scalar-backend prototype that previously reduced the
  registered 1L tadpole, 2L sunset, and pinch with an invalid FORM path and
  mapped terminals to Vakint's existing pure-Rust master evaluation.
- Rebase work toward the FeynKit branch and parity-harness integration exists
  in the dirty GammaLoop worktree, but no clean post-rebase parity claim is
  made here.

### Still open

- Produce a complete, exact, cold-reloadable `K = 6` artifact covering all
  five registered three-loop graph classes.
- Wire that artifact into Vakint and pass the complete analogous acceptance
  matrix, not merely a handful of bespoke tests.
- Compare exact raw master coefficients where bases coincide; otherwise use
  numerical parity against MATAD/AlphaLoop through three loops. A valid
  nonminimal RustRed terminal basis must not be rejected solely because it is
  not MATAD's preferred symbolic basis.
- Exercise scalar RustRed with an invalid FORM path.
- Exercise tensor-bearing cases with the FeynKit FORM-less tensor prepass and
  the RustRed FORM-less scalar tail.
- Preserve Vakint defaults and all old methods.

Stage 1 is not complete until these three-loop closure and Vakint acceptance
gates pass.

## 7. Exact status of K6 and Janet/Ore

No complete closing K6 artifact has yet been generated, with or without
FORM-derived hints.

The most recent exact owner-cover baselines are:

| Lane | Replayed semantic owners | Guard-total owners | Exact unbounded complement |
|---|---:|---:|---:|
| Rank-three path routing | 9 | 4 | 10 boxes |
| Rank-three star routing | 22 | 12 | 4 boxes |
| Maximal S4a degree-1/2 study | 24 inputs total; one guard-total owner in the cited sweep | 1 | 3 five-dimensional boxes |

These counts are not the result of an exhausted full Janet/Ore completion
queue. They are exact owner-cover diagnostics from finite semantic-source
studies. Do not describe “10”, “4”, or “3” as the final number of Janet rays.

The older eager exact Janet release campaigns were stopped by implementation
resources after roughly 60–110 basis rows, before queue exhaustion. Exact
coefficients expanded to tens of millions of terms and drove time/RSS beyond
the useful envelope. Consequently there is no defensible final count of rays
remaining after a completed eager Janet run.

What “escaped Janet/Ore” therefore means at present:

1. It is not a proof that Janet/Ore is mathematically insufficient.
2. The eager representation materialized every rational coefficient and
   repeatedly copied/reduced large rows; coefficient swell stopped execution
   before the algorithm answered the question.
3. The owner-cover experiments also show that the first useful move in some
   uncovered directions is not exposed by the shallow ordinary-source window.
   It requires affine loop-routing/factorization relations that expand or move
   inactive numerator coordinates, after which ordinary IBP descent becomes
   visible.
4. Variable/coordinate ordering materially changes the intermediate basis and
   coefficient swell. Ordering portfolios and eventually bounded/MCTS-like
   ordering search remain promising, but an ordering screen may change
   priority only; it cannot omit mandatory completion obligations.

The immediate strategy is not to declare the surviving boxes extra masters:
they are infinite families, not a finite terminal set. The strategy is to run
the mathematically complete Janet/Ore queue with coefficients represented as
exact lazy circuits, share immutable basis rows, use modular nonzero proofs
with exact fallback, add the affine/factorized seed relations through audited
generic generators, and only then inspect the exact complement. If the final
complement is finite but larger than MATAD's master basis, ship that finite
nonminimal basis and evaluate/map it numerically.

## 8. The pushed exact-lazy milestone (`759ab1c`)

The performance rescue is implemented through the following foundation:

- Hash-consed exact coefficient DAG over authenticated Symbolica-backed leaf
  coefficients and Ore translations.
- Separate lazy DAGs for physical coefficients, source derivation/provenance,
  and typed localization guards.
- Atomic transactions with rollback of live arena floors and monotone charging
  of attempted work.
- Per-transaction commit receipts, so an aborted wrapper made entirely from
  pre-existing roots cannot masquerade as committed authority.
- Deterministic modular nonzero probes and batched exact Symbolica fallback for
  sampled zeros. Support never changes on the basis of a sampled zero.
- Complete exact-lazy Janet cancellation and normal form against an immutable
  frozen exact epoch.
- Distinct typed unrestricted and self-excluding normal forms.
- Guarded monic normalization that retains denominator-definedness and adds
  the numerator-nonzero domain required for inversion.
- Cold lowering that materializes and exactly replays physical rows,
  provenance, and guards against regenerated sources.
- Principal-open guard comparison using Symbolica square-free factorization,
  polynomial GCD, and exact division. The direction is one-way: the
  authenticated lazy domain must imply every replay-required condition; the
  lazy witness remains authoritative.
- Conservative pre-native resource admission for guard algebra. An audit found
  that GCD/exact-quotient coefficient height can exceed either expanded input;
  a checked mixed-radix Kronecker-to-univariate Mignotte factor-height bound
  and taller-GCD/taller-quotient regressions now cover this.
- Shared coefficient-free Janet geometry for eager and lazy representations.
- Persistent immutable exact-lazy Janet division and complete epochs with
  `Arc`-shared rows, opaque instance identity distinct from revision depth,
  sibling isolation, a complete Janet queue, exact complement, and pure-power
  coverage.
- One non-cloneable completion ledger binds owner, action, limits, support
  probes/fallbacks, and involutive selector/index work across the campaign.

Important current limitations of this milestone:

- Exact-lazy normal form accepts the frozen ingress epoch, not yet the
  persistent lazy division epoch in production.
- Persistent epoch raw addition/replacement constructors are test-only.
- There is no production basis-admission token.
- There is no exact-lazy prolongation operation.
- There is no exact-lazy synchronous autoreduction or equal-head collision
  resolution.
- There is no exact-lazy completion driver or queue-exhaustion certificate.
- Cold lowering currently materializes one whole requested envelope and keeps
  its cache/output live until the batch ends; K6 publication still needs
  deterministic streaming/chunking and safe cache eviction.
- Therefore the new engine cannot yet launch the decisive full K6 completion
  run. This is the precise reason another complete K6 attempt was not run in
  the final session.

## 9. Exact restart point: next implementation slice

Three agents independently designed and audited the next seams. They began
implementation after `759ab1c`, but the user stopped the session immediately.
Those partial changes compiled with two visibility warnings but had no tests
and were intentionally reverted. Restart from the clean `759ab1c` APIs; do not
assume a provider or prolongation module exists.

### 9.1 Sealed persistent divisor provider

Add a private `exact_lazy/divisor_provider.rs` with a sealed, statically
dispatched trait implemented only by the frozen ingress epoch and persistent
lazy division epoch. It should expose:

- borrowed coefficient-free Janet geometry;
- owner/epoch identity;
- full environment and campaign-ledger authentication;
- an opaque borrow-only divisor view containing canonical ordinal, leader, and
  `&ExactLazyConsequence`;
- bounded divisor scratch through the same ledger.

Generalize cancellation and complete normal form over this provider. Before
any accounting or mutation, verify provider/session/order/context/limits,
provider ledger versus cursor ledger, exact epoch identity, exclusion bounds,
subject owner, and scratch identity. For a selected divisor, independently
reselect its live leader and require the structural-one coefficient. Never
expose a raw `Arc` or an insertion constructor from this trait.

Focused tests should compare frozen and persistent normal forms for 1L and all
four generated K3 ordinary sources, over active and inactive actions; sweep a
bounded target box with every exclusion; exercise same-depth sibling epochs
and sibling ledgers; and prove predecessor `Arc` identity remains unchanged.

### 9.2 Exact-lazy pure Janet prolongation

Add `exact_lazy/prolongation.rs`. Accept only a complete sealed lazy epoch and
one of its authenticated `JanetProlongation` values. Return a move-only
`ExactLazyProlongationSubject` carrying owner, action, epoch, ledger,
obligation origin, and committed consequence. It is full-normal-form input,
not queue-discharge or basis-admission authority.

For a unit Ore shift `delta`, translate atomically:

```text
(shift, coefficient) -> (shift + delta, sigma_delta(coefficient))
```

Translate the whole source derivation and typed guard lineage by the same
operator. Do not add a multiplier guard: the multiplier is structural one.
Preflight every shift, physical row, derivation, guard, and transaction limit.
Charge one completion iteration only after all foreign/stale authority checks.

Do not re-run support classification for a pure prolongation. Forward Ore
translation is an injective automorphism of the rational-function coefficient
field, so exact nonzero support can be transported by a private proof whose
constructor receives the live authenticated source term and internally creates
the translated root. A whole-row private seal must prove every source term was
translated exactly once. Independently reselect the output leader and require
it to equal the obligation's target/key with structural-one coefficient.

Test exact equality with the eager prolongation oracle on a synthetic 2D
fixture, 1L, and every K3 obligation; test translated denominator and
numerator guards; set classification allowance to zero and prove the
prolongation still succeeds without changing classification census; and cover
all stale/foreign/one-below transaction cases.

### 9.3 Obligation-bound admission and immutable autoreduction

Add a private `exact_lazy/admission.rs` and then
`exact_lazy/autoreduction.rs`. Required move-only authority types include:

- full-NF insertion admission;
- self-excluded replacement admission;
- unchanged shared-row admission;
- full/queue zero witness;
- autoreduction/equal-head zero witness;
- resolved autoreduction batch;
- stable-division seal.

Two constraints found by the independent audit are mandatory:

1. A self-excluded NF may become replacement authority only when it was
   created from the actual epoch row at the excluded ordinal. Bind epoch,
   ordinal, original row/`Arc` identity, and ledger. An arbitrary caller-owned
   subject plus `Some(ordinal)` is calculation/test functionality only.
2. Raw `ExactLazyJanetDivisionEpoch::try_seal` must not remain an unrestricted
   production seam. Only trusted complete frozen ingress or an opaque stable
   autoreduction pass may seal a complete epoch. Otherwise insertion could
   skip mandatory synchronous autoreduction.

Preparation must consume typed NF authority and internally distinguish zero
from nonzero. Nonzero output is guarded-normalized and checked monic; zero
output gets a purpose-specific witness. Full and self-excluding modes must not
interchange.

Autoreduction must be synchronous:

1. Every row reduces against the same immutable division snapshot while
   excluding itself.
2. An unchanged row is shared without copying its coefficient DAG.
3. Changed nonzero rows are normalized and replacement-bound.
4. Exact zeros retain bounded replayable evidence.
5. No successor is published until the entire pass and equal-head resolution
   succeed.
6. If any row changed, build a division-only successor and repeat.
7. If no row changed, mint the stable-division token and only then seal
   complement/queue geometry.

Synchronous remainders can collide at equal leaders. Resolve them
deterministically, preferring an unchanged shared predecessor row then stable
original ordinal; cancel equal monic heads with structural multiplier `-1`,
classify the lower row exactly, normalize if nonzero, and require strict leader
decrease. Preserve guards/provenance and reject incomplete/duplicate batch
coverage.

### 9.4 Exact-lazy completion and closure certificates

Once the preceding slice is audited, implement the one-shot completion driver:

- construct/rank every mandatory Janet nonmultiplicative prolongation;
- blind-domain information may adjust priority but never omit an obligation;
- prolong, full-normal-form reduce, and produce an obligation-bound zero
  witness or insertion admission;
- insertion invalidates the entire old queue and its zero witnesses;
- synchronously autoreduce and rebuild a new immutable epoch;
- repeat until the complete current queue is exhausted;
- bind the queue-exhaustion seal to exact epoch/owner/action/order/ledger and
  the complete current zero-witness set;
- independently require a finite exact complement and enumerate every point
  as an explicit terminal candidate;
- return a typed `QueueExhaustedNonFinite` report with missing axes/unbounded
  boxes if the complement is infinite. Never turn it into an artifact.

Resource accounting must remain cumulative across retries and failures.
Add bounded counters for completion iterations, queue rebuilds, autoreduction
passes/outcomes, shared/materialized rows, equal-head work, zero witnesses,
and retained transcript bytes. Failed transactions roll back live roots but do
not refund attempted work.

### 9.5 Streaming cold publication

After exact closure only:

- cold-lower final basis rows and all final queue-zero witnesses;
- reuse one authenticated ordinary-source lift;
- materialize in deterministic bounded chunks;
- track root liveness so exact-materialization cache entries can be evicted
  safely;
- verify support, monicity, leader, provenance, localization, geometry,
  terminal enumeration, and exact replay;
- serialize, reload cold, and reproduce the same reducer authority;
- record peak RSS and cumulative lowering work.

A hot completion that closes but recreates the old peak-RSS cliff during cold
publication is not a usable K6 result.

## 10. Decisive K6 experiment sequence

Do not wait until every optimization is complete before testing the actual
target. After each coherent audited slice:

1. Differential-test synthetic, 1L, and K3 fixtures against the eager exact
   path.
2. Run short release K6 prefixes for the natural path/star orders and the best
   audited S4a/custom orderings.
3. Record basis rows, queue length, exact-support classifications, modular
   nonzero fraction, exact fallback fraction, DAG live/cumulative nodes,
   shared versus materialized rows, coefficient cold sizes, wall time, and peak
   RSS.
4. Reject regressions or likely blow-ups early with typed resource outcomes;
   do not reinterpret them as algebraic failure.
5. When prefixes fit the envelope, run the complete release campaign serially
   first for a simple authoritative transcript. Then compare supported worker
   counts deterministically where immutable probe parallelism applies.
6. Run both autonomous/hintless and oracle-informed seed portfolios. The latter
   may use AlphaLoop/MATAD only to choose a generic RustRed input itinerary; it
   must regenerate all ordinary relations itself and must never import FORM
   rules or coefficients into the artifact.
7. Cold publish/reload only if queue exhaustion and finite complement both
   succeed.

If the run exhausts the queue but leaves an infinite complement, preserve the
exact missing boxes and diagnose the absent leading directions. Add generic
affine-routing/factorization source generators or new seed families as input,
not hand-authored topology-name rules. If the complement is finite, accept a
nonminimal master set and proceed to numerical evaluation/mapping.

## 11. Vakint integration after K6 closure

Vakint should ship the immutable K1, K3, and K6 artifact bytes. Ordinary users
must never regenerate them at evaluation time. The adapter must:

- expose opt-in `EvaluationMethod::RustRed(RustRedEvaluationOptions)` and
  `EvaluationOrder::rustred_only()` without changing defaults;
- consume Vakint's existing topology match and simultaneous routing witness;
  do not rematch graphs and do not dispatch on topology names;
- use FeynKit for the FORM-less tensor prepass in Stage 1;
- pass scalar integral keys to RustRed's efficient memoized parametric-rule
  applier;
- receive exact coefficients of typed terminal/master keys;
- map known terminals to the existing MATAD basis where exact mapping exists;
- otherwise use the accepted per-lane numerical-parity policy for a finite
  nonminimal terminal basis;
- reuse only Vakint's pure-Rust master substitution/evaluation machinery;
- expose master-substitution control, default enabled;
- report no FORM dependency and never invoke or fall back to FORM.

Extend the existing Vakint comparison harness analogously to
AlphaLoop-versus-MATAD. Do not create a tiny separate RustRed-only harness.
Run every applicable single-scale 1L–3L analytic, freeform, numerator, routing,
and method-comparison case. Validate exact raw coefficients when bases agree
and final numerical/Laurent parity when they do not. Include invalid-FORM-path
tests and unchanged-default/backward-compatibility tests. Profile scalar
application after correctness; compare with AlphaLoop/MATAD while separating
one-time artifact loading from hot reductions.

## 12. Research and documentation map

Read the directly relevant notes completely before modifying the corresponding
boundary:

- `docs/research/k6_exact_lazy_persistent_completion_design_2026-09-03.md`:
  current exact-lazy architecture and acceptance gates.
- `docs/research/k6_exact_lazy_frozen_cancellation_design_2026-09-03.md` and
  `..._audit_2026-09-03.md`: cancellation/normal-form proof boundary.
- `docs/research/k6_exact_lazy_support_certificate_design_2026-09-03.md` and
  `..._audit_2026-09-03.md`: exact support authority.
- `docs/research/k6_janet_ore_release_study_2026-09-02.md`: eager K6 resource
  evidence and what was/not proven.
- `docs/research/k6_janet_incremental_epoch_audit_2026-09-02.md`: immutable
  epoch and copy-on-write findings.
- `docs/research/k6_janet_modular_fraction_free_design_2026-09-02.md`: modular
  scheduling/fraction-free alternatives.
- `docs/research/janet_ore_integration_seam_2026.md` and
  `blind_domain_janet_closure_2026.md`: Janet/Ore integration semantics.
- `docs/research/parametric_ibp_literature_2026.md`,
  `parametric_ibp_breakthrough.md`, and
  `parametric_ibp_breakthrough_audit.md`: literature and candidate methods.
- `docs/research/factorized_product_angular_owner_2026.md` and
  `sector_local_coordinate_chart_2026.md`: the missing affine/factorized
  routing direction.
- `docs/research/six_loop_candidate_shootout_2026.md`,
  `six_loop_scaling_audit_2026.md`, and
  `six_loop_execution_runbook_2026.md`: later scaling studies.
- `docs/research/vakint_k6_oracle.md`: permitted oracle use and separation from
  autonomous proof generation.

Use LiteRed2 only for algorithmic inspiration and behavioral comparison; it
is reference-only Mathematica code and must not shape the Rust architecture or
enter commits. Use Symbolica's graph isomorphism/canonicalization primitives
rather than copying FeynGen-specific logic. Review the public Symbolica source
under the reference tree before implementing algebra or graph operations.

## 13. Known P2 follow-ups (not blockers for the next slice)

- `epoch.rs` rescans a genuinely new row when selecting its leader after full
  liveness validation; cache the authenticated leader only if profiling shows
  this matters.
- A private raw-budget divisor-query helper exists for tests. Production
  completion must route all queries through the campaign ledger.
- Extend geometry differential coverage from leader-only queries to a bounded
  target box with every exclusion choice.
- `GuardedLeaderInverse::inverse()` is slightly broader internally than ideal;
  future hardening may move scaling entirely behind arena-owned operations.
- Narrow ledger budget borrows currently rely on the enclosing operation
  having performed the full environment check. Current call sites do; a
  validated borrow token could encode it.
- Cold lowering collapses some typed chart-lift errors to `InvalidProof`.
- Source chart lifting is repeated per lowering attempt rather than shared
  across a complete publication batch.
- Append-only lazy DAG history and multiplier-guard lineage should be measured
  on K6 prefixes before implementing garbage collection or compaction.

Do not let these P2s distract from reaching the first runnable exact-lazy
completion loop unless a measured prefix promotes one to P1.

## 14. Immediate resume checklist

1. Read `GOAL.md`, this file, `HANDOFF_simple.md`, and the exact-lazy persistent
   design note completely.
2. Verify RustRed `main` is clean and matches pushed `origin/main`.
3. Confirm the current Symbolica license and run the focused 72-test exact-lazy
   gate.
4. Spawn separate implementation owners for sealed divisor access and pure
   prolongation, plus an independent auditor.
5. Land the provider first or coordinate the tiny shared `epoch.rs`/`mod.rs`
   seams explicitly; do not allow concurrent blind edits to the same files.
6. Audit and push that milestone.
7. Implement obligation-bound admission, synchronous autoreduction, and
   equal-head resolution with another independent audit.
8. Implement completion/queue-exhaustion/finite-complement certificates.
9. Run release K6 prefixes immediately, then the full campaign once the
   measured envelope is credible.
10. On exact closure, stream cold publication, ship K6 into Vakint, and finish
    the complete through-3L parity matrix.

The next session should lead every status report with the evidence boundary:
K6 is not closed yet; the exact-lazy representation and persistent geometry
foundation are complete and fully green; the production provider,
prolongation, admission, autoreduction, completion, and streaming publication
layers are the remaining path to the first decisive run.
