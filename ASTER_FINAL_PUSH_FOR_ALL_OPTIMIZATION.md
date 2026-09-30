# Final optimization push: generated rules and the complete S5 merge path

Status: authorized on 2026-09-30; implementation follows this plan.
Branch: `fable_5_1_parallel`. Starting committed milestone: `297be07f`.
Continuous evidence and task ownership: `CODEX_PROGRESS.md`.

Latest clarification: the user removes the delivery deadline. Take the time
needed for clean implementations, thorough organization and smart delegation.
These two tracks are intended as durable foundations, not rushed experimental
patches. The thirty-minute exploratory-pilot bound below is a measurement and
resource-control rule, not a deadline for finishing implementation.

## Goal

Deliver a clean, committed and pushed optimized build for a future five-loop
campaign by completing **two workstreams only**:

1. Fully exploit the measured improvement from better generated-rule selection,
   including implementing the broader persisted, programmable integral-order
   comparator that is currently design-only.
2. Complete the unfinished S5 architecture: parallel merge preparation, typed
   records, bulk dependency updates and efficient immutable-index publication,
   without introducing substantial redundant work through changed publication
   or containment choices.

Test both implementations independently and together. Demonstrate useful gains
on matched, fully checked pilots before recommending the resulting campaign
setup. Useful activity around twenty or more physical cores is a performance
target, not a reason to manufacture work or an unsupported utilization promise.
Saturating two hundred cores is not required.

The user explicitly postpones launching a campaign. **Do not stop, restart,
resume or mutate LC2 or launch another production campaign.** A prepared future
command is a deliverable, not permission to execute it. Existing campaigns need
not be compatible with the new code, schemas or APIs; preserve their frozen
executables and files so the owner retains control.

## Scope and starting evidence

- Preserve the frozen mathematical request: all 116 required starting queries,
  all 67 original auxiliary helpers, and all reachable descendant obligations.
  Do not narrow bounds, drop difficult sectors or relabel uncovered work as a
  terminal to produce a better benchmark.
- Algorithms and ordering descriptors remain topology- and loop-count-generic.
  Family-specific heuristic data belong in inputs, not engine dispatch.
- The current Source A API changes finite source-row and sector visitation.
  Two isolated four-loop pairs reduced generation+walk+cold time by 17.45% and
  18.73%, and walk+cold time by 36.51% and 39.23%. The small five-loop transfer
  was positive but limited and concurrent. These gains do not alter already
  generated saved rules.
- The current Epoch settings did not outperform Ready. Larger worker budgets
  and oldest-ready publication sometimes increased work drastically. Reuse
  those negative results: do not repeat blind width, window or ordering sweeps.
- The original S5 is incomplete: P2 bucket planning and P3 are serial, records
  are JSON, and tracker updates are per-edge. Ledger6 and hash-feed batching
  alone are not completion of S5. S3 durable checkpoints and S4/S6 immutable
  lookup/rolling infrastructure are foundations to retain and test.
- Source certificates that reached proof/input limits remain unpassed.
  Successful scoped cold reinspection is not a substitute claim of unrestricted
  source-artifact certification. Neither is this delivery full five-loop closure.

## Governance and delegation

- Root orchestrates integration, shared-resource allocation, profiling, final
  verification and release. Separate agents own the two implementation lanes;
  another independent agent audits mathematics, code and measurement claims.
  An implementation author does not approve their own implementation.
- Begin with a bounded source/API review, then agree file ownership before
  edits. Serialize shared codec, application, CLI and Python edits. Independent
  core ordering and epoch merge slices can proceed concurrently.
- Use Symbolica for algebra. Before any algebraic operation is added, inspect
  the pinned public API, current available public API and existing RustRed use.
  No custom polynomial, factorization, interpolation or reconstruction kernel.
  Ordering integer keys and graph bookkeeping are not new coefficient algebra.
- Use workspace-local ignored `TMP/`, existing build/heavy-job reservations and
  disjoint physical CPU sets. Never escalate, tune host-wide settings, use LC2's
  cores or serialize the license into a repository artifact.
- Record each material result, decision, failed test and negative measurement
  in `CODEX_PROGRESS.md`, with exact revision, commands, scope, evidence paths,
  responsible agent, next step and measured/inferred distinction.
- Commit and push audited milestones using `user.name=ValentinHirschi` and
  `user.email=valentin.hirschi@gmail.com`, retaining the existing coauthorship
  convention. Preserve unrelated user work and exclude campaign/reference data.

## Runtime flexibility and compilation architecture

- Design configurable mechanisms rather than compiled-in experiments. Runtime
  descriptors should select order programs, source/sector priorities, numeric
  weights, preparation policy, worker budgets and supported batch/index controls.
  Persist every setting that changes semantics, replay or scheduling decisions;
  reject unsupported combinations before work starts.
- New values or combinations of existing generic features must not require a
  Rust rebuild. Separate a configuration interpreter/compiled plan from the
  elimination and merge engines. Experimental Python steering constructs data,
  not ad hoc solver implementations. New primitive capabilities may legitimately
  require compilation; document that boundary instead of claiming arbitrary
  executable callbacks are a portable runtime language.
- Review the actual Cargo dependency graph, existing monomorphization and build
  receipts before choosing new module/crate boundaries. Isolate stable order
  data/evaluation and merge-plan types from changing recipe/adaptor logic where
  this removes real dependencies. Avoid broad generics that instantiate GPLU or
  the whole walker separately for every strategy.
- Use cohesive modules and, when supported by the dependency review, a small
  independent crate. Do not split crates just to claim better caching: Rust
  module separation alone does not guarantee crate-level incremental reuse.
  Keep hot paths borrowed and compiled once; avoid an allocation-heavy or
  dynamically dispatched CAS layer merely to reduce rebuild time.
- Record affected-package checks, focused-test/build costs and cache reuse for
  representative edits. Use isolated, reusable Cargo targets and incremental
  development checks; reserve expensive optimized/LTO builds for validated
  framework milestones and final matched measurements. Never time a development
  build against a production baseline or race builds in one mutable target.
- Maintain a stable experiment schema and small steering examples so later
  topology, pivot, source-order and scheduler experiments reuse one frozen
  optimized binary. Keep algorithm configuration generic and future-extensible
  without topology or fixed-loop-count limits.

## Track O — generated-rule selection and integral ordering

### O1. Implement a persisted mathematical order, not an opaque comparator

- Review `docs/research/runtime_pivot_strategy_design_2026-09-29.md` against
  actual concrete, symbolic-shift, preconditioning, modular-discovery, exact
  lifting, descent, replay and artifact-loading consumers.
- Implement one canonical validated runtime descriptor and compiled evaluator.
  Expose meaningful sector/support priorities, weighted within-sector degree
  priorities and explicit denominator/numerator coordinate tie ordering.
- Admit only a mathematically justified well-founded class. A starting design
  uses pinch-monotone finite support order followed by nonnegative weighted
  excess degrees that cover every coordinate, and ties within finite fibres.
  The mathematical audit must establish concrete/shift agreement and precisely
  identify any cut or boundary restrictions before this design becomes code.
- Persist semantic descriptor bytes, not a user label, process-local handle or
  arbitrary comparison closure. Fail closed on unsupported orders, cuts,
  malformed descriptors, overflow, unknown versions and mismatched replay.
- Keep comparison hot paths allocation-free after compilation where practical.
  Do not inflate every integral with an independently owned copy of its order.
  A Rust builder/callback may construct a validated descriptor or finite plan;
  it cannot bypass order admissibility or replace persisted mathematical meaning.
- Version evolving schemas clearly. No legacy migration project. New artifacts
  must carry their actual order through generation, load, application and proof.
- When reusing a rule and its order proof in another coordinate frame, transport
  the complete descriptor and prove commutation; identical labels do not prove
  equivariance. Do not impose this requirement on independently valid owners:
  the existing reducer permits distinct local orders, keeps same-support Apply
  steps within one owner and reroutes only after strict support-count decrease.
  Preserve that well-founded composition and test mixed-order routing plus
  refusal of illegal same-support rerouting. The end-to-end design must support
  real routed controls, not only isolated unrouted generators.

### O2. Public controls and mechanics-informed strategies

- Keep finite source scheduling and mathematical integral ordering separate.
  Extend Rust, CLI and `import rustred` surfaces consistently, with runtime
  JSON/data descriptors so selecting weights, priorities and named recipes does
  not require rebuilding the engine.
- Retain the successful sparse/coefficient-aware source recipes. Derive a small
  portfolio from structural information: shared pinches and routed owners,
  support incidence, likely degree descent, source sparsity and guard/branch cost.
  No topology-name special cases or unbounded search framework.
- Prefer static, reproducible, lawful mathematical order per generated owner.
  Search over candidate orders outside the proof engine; never mutate the
  meaning of an accepted artifact according to live timing feedback.
- Use the same optimized executable for experimental choices. Regenerate the
  affected owner programs when an algebraic order changes; remap/load/replay
  them consistently rather than pretending a walk flag can rewrite saved rules.

### O3. Validation and measured selection

- Test admissibility, totality, transitivity, strict-pinch descent, finite-fibre
  conditions, default/reference equivalence, concrete/shift agreement, boundary
  rejection, malformed input, overflow, codec identity and wrong-order refusal.
- Exercise exact lifted-rule descent and regenerated-source checks, exceptional
  guards, finite residuals, load/application, resume and worker determinism.
  Finite test cubes supplement rather than replace the well-foundedness argument.
- Reproduce the existing source-order gain on the full combined four-loop
  control, then measure the new comparator portfolio. Include generation,
  loaded payload size, rule/terminal counts, native walk and independent cold
  verification separately and together.
- Use finite and hot five-loop controls and a bounded broader-owner pilot to
  assess transfer. Keep reduced diagnostic scope explicit. Do not declare a
  production-wide gain or full closure from one owner or a sampled subset.

## Track S — complete S5 while preserving useful selected work

### S1. Typed preparation and a semantics reference

- Map the real P1/P2/P3/P4 responsibilities, measured costs and dependency
  boundaries. Establish a reference trace for each selected fixed-cut control.
- Replace hot-path unstructured record construction/inspection with typed
  internal records. Serialize at the durable boundary, using the project's
  uniform binary conventions and Symbolica state-aware encoding for any atoms.
  Keep optional human-readable diagnostics outside the hot representation.
- Carry exact source/event identities, guard obligations, anchor pins, accepted
  decisions and publication versions through these records and checkpoints.

### S2. Parallel deterministic merge preparation

- Partition independent P2 bucket work against immutable snapshots. Each task
  returns a typed plan: containment/alias choices, antichain changes, retirement
  and transfer proposals, edge batches and capacity requirements.
- Establish which decisions are bucket-local and which require cross-bucket or
  global reconciliation. Do not assume independence merely because storage is
  bucketed. Resolve global choices in the same explicit canonical order.
- Keep P1 validation, reservation/preflight and authoritative P3 state mutation
  controlled. Parallelize safe preparation, not uncoordinated shared mutation.
  A failed preparation must not partially publish a cut.
- Preserve reference publication/work-selection policy while measuring this
  change. The initial speed comparison must not obtain different workload just
  by skipping an inconvenient earlier result. Retain complete dependency edges.

### S3. Bulk publication and bounded index layers

- Add bulk tracker/edge operations that preserve duplicate handling, reverse
  dependencies, recursive-closure propagation, cycles and quarantine semantics.
  Batch validation, reservation, serialization and hash feeds coherently; do not
  call per-edge machinery in a loop and describe that as completed bulk work.
- Implement P4 immutable lookup publication/delta layers and bounded compaction
  where required by the completed merge design. Preserve exact positive checks
  and correct stale-negative handling; enforce bounded retained snapshots.
  Preserve the current chosen container: admissible exact hits include retired
  IDs, the dominant orthant keeps its existing rank/tie policy, and general
  containment chooses the minimum currently live ID. A layered physical index
  may change truthful candidate/test telemetry under the new Epoch semantics;
  do not claim those physical counters equal the old layout. For a fixed new
  layout they must remain deterministic across helper budgets. Selected IDs,
  dependency edges, logical work and authority must not change merely because
  storage is shared. Do not adopt the historical design's retired-tier first-hit
  shortcut, which would change selected work.
- Share the configured worker budget between inspectors and merge preparation;
  avoid oversubscribed nested Rayon/Symbolica pools and large per-worker clones.
  Start with controlled task granularity and transparent scheduling rather than
  an unrelated global scheduler rewrite.

### S4. Operational correctness and instrumentation

- Exercise interruption, RAM stop, allocation/preflight failure, worker error,
  quarantine during an outstanding proposal, G2 anchors blocked after inspection,
  accepted dependency replay and cold checkpoint restoration.
- Persist the new semantics with an explicit version. Fresh campaigns are fine;
  incompatible restoration must fail clearly rather than reinterpret old data.
- Report useful inspector and merge-worker activity separately; record task
  backlog, phase time, queue/domain work, memory and checkpoint time. Keep the
  current dashboard's metrics and rendering semantics, extending its generic
  data stream only as needed for the new phase counters.
- Independently audit exact plan/record/edge/counter equality under reversed
  helper completion and helper budgets zero/one/two/seven. Include dominant
  single-bucket skew, equivalent/nested images, canonical error order, every
  preflight failure, duplicate/disabled/cyclic dependency updates and held-reader
  compaction. Cooperative output/RAM limits discard the unpublished cut; they
  never truncate its mathematical obligations or create partial authority.

## Integration, pilots and acceptance

1. Freeze current measured baseline binaries and inputs. Never overwrite LC2
   or the prior `1b33ad29` executable to claim a matched comparison.
2. Run focused implementation tests before expensive builds. Consolidate code
   freezes sensibly; distinguish a necessary framework rebuild from repeated
   input-only experiments. Do not overlap heavy work on production cores.
3. Cover FG, BMW, H, X, combined four-loop, finite and hot five-loop controls.
   Completed claims require independent full reinspection of the actual scope.
4. Compare reference versus S5 on identical generated programs, query bytes,
   worker budget, CPU placement and timing boundaries. Compare ordering changes
   separately with their regeneration cost. Finally test the combined candidate.
5. Each exploratory pilot is at most thirty minutes including preparation,
   restore and safe drain. Report censored runs honestly; production has no new
   arbitrary runtime cap. Longer production work is not authorized here.
6. Record wall and CPU time, peak RSS, scheduled/native domains, pending work,
   required-query closure, frontiers, rules/terminals, bytes and phase costs.
   Busy cores alone are not success. Check whether a speedup is purchased with
   enough redundant work or memory to undermine the full campaign.
7. Reproduce deployment claims in two matched pairs. The existing 1.5x gate
   remains required to call Epoch a qualified faster replacement. The ordering
   gain already has evidence, but a new comparator or integrated binary needs
   its own checks. Require combined four-loop non-regression and useful five-loop
   transfer before recommending the final setup.
8. Profile representative widths targeting twenty-plus useful physical cores
   once low-width correctness/work identity pass. No speculative two-hundred-
   core launch. If work changes dramatically, diagnose that before increasing
   width. Report an unmet utilization target rather than hiding it.
9. If a slice fails, isolate the mathematical/engineering/performance cause,
   correct and retest. Do not relabel the still-incomplete S5 architecture as
   finished merely because an earlier Epoch setting was slow. Escalate a genuine
   design impasse with concrete evidence instead of silently dropping a track.

## Deliverables and completion

- Implemented, audited runtime integral-order API plus tested generic strategy
  choices, with corresponding generation/load/application examples.
- Completed, audited S5 merge path with deterministic preparation/publication,
  typed records, bulk dependencies and bounded immutable index publication.
- A consolidated optimized build, independent correctness results and matched
  pilot report identifying measured gains, negative results and remaining risks.
- Updated `CODEX_PROGRESS.md`, this plan's completion states, relevant stable
  docs, and exact build/preparation/launch/rollback instructions for the future
  campaign. Explicitly identify whether new saved owners must be generated.
- Clean task-owned commits and pushes on `fable_5_1_parallel`, preserving other
  collaborators' work. No campaign is started by this work.

The new tool-managed goal is complete only after both tracks are implemented,
tested, independently audited and the combined delivery has demonstrated pilot
gains with an honest final campaign configuration. Full five-loop mathematical
closure, terminal minimization, numerical masters, Vakint and unrelated research
remain outside this optimization delivery.
