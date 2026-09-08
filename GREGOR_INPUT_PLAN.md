# GOAL

Implement Gregor Kälin's SpIReD-inspired, case-directed strategy as RustRed's
primary parametric-IBP closure engine. Prioritize a dedicated high-performance
single-scale/no-scale vacuum lane capable of closing K6, while preserving a
clean topology-generic architecture. Complete the existing Stage 1 goal—K6
generation and FORM-less Vakint scalar reduction through three loops—then
proceed into Stage 2 studies and closure attempts for K10, K15, and K21.

RustRed must remain pure Rust plus Symbolica. It must produce exact, replayable,
strictly descending closing artifacts; use rational-polynomial reconstruction
only when Symbolica exposes it; retain Janet/Ore as an optional complementary
strategy; and aim to come within 3× of Gregor's Section 4.5 timings on
faithfully reproducible benchmark cases. Once Symbolica reconstruction becomes
available, the objective becomes outperforming those timings.

## Authoritative directive

The following input is included verbatim:

```text
a) Yes the PDF is confidential and should not be added to the repo, but it's also not hyper-sensitive material either, so all I ask you is just to not push it to the repo but it's ok for you to keep it in context, and also to implement these ideas in RustRed. The author of the notes Gregor will also be an author of RustRed.

b) I want you to work on implementing this strategy already, but avoiding the rational polynomial reconstruction route for now. It should be possible to have this added on later, so plan for this in the architecture of your implementation, but do not implement these rational polynomial reconstruction algorithms by yourself now, since they are planned to be added in Symbolica itself soon by its author.

c) Implement Gregor's ideas in a meaningful way within what you already have, Janet/Ore may still prove useful or complementary at some point, so don't deprecate it, but if not useful for K6 make sure it's very presence is not hurting performance (i.e. or even not used in that particular case too it's fine).

d) Keep in mind that performance is paramount, so often run end-to-end cases and audit/profile the run to find optimisations, all the while keeping the very generic implementation of RustRed as flexible, clear and simple as it can be.
```

The working notes are available locally at `./notes-spired.pdf`. That file must
remain untracked and must never be committed or pushed.

## Execution bootstrap and governance

- Before changing implementation code, write this complete plan to
  `GREGOR_INPUT_PLAN.md`, update `GOAL.md` so both Stage 1 and Stage 2 reflect
  Gregor's input, and create a tool-managed goal assigning the full objective
  to `/root`.
- Add `/notes-spired.pdf` to `.gitignore` without deleting the local file.
  Preserve unrelated and existing untracked work.
- Update package authorship to include
  `Valentin Hirschi <valentin.hirschi@gmail.com>` and `Gregor Kälin`.
- Triple-check Symbolica's pinned and current public APIs before implementing
  any algebraic primitive. Implement locally only narrowly scoped operations
  demonstrably absent from Symbolica.
- Every milestone receives an implementation pass and an independent
  adversarial audit by separate subagents. The primary agent remains
  orchestrator, integrator, profiler, and final verifier.
- Commit and push coherent milestones using `user.name=ValentinHirschi` and
  `user.email=valentin.hirschi@gmail.com`. Never push reference-only material.
- Keep RustRed free to break its own evolving schemas. Preserve Vakint's
  existing public API conventions, defaults, and FORM-backed modes.

## Core implementation

### SpIReD completion engine

Introduce a cohesive `completion::spired` subsystem that replaces the
expensive search front end while retaining RustRed's existing exact authority
pipeline.

For each uncovered case:

1. Freeze its sector, target, ordering, and immutable lower-owner snapshot.
2. Enumerate translated ordinary IBP sources fairly in signed-L1 depth shells.
   Heuristics may reorder within a shell but cannot starve a source
   indefinitely.
3. Evaluate shifted coefficients modularly as `c(n+s)` without constructing
   exact shifted polynomials.
4. Stream each row once into two incremental Symbolica sparse reducers:
   forbidden columns only, and forbidden columns plus a stable logical target
   column.
5. Detect a candidate when adding the target changes the relevant rank or
   pivot condition.
6. Retain direct GPLU/L-pattern dependency edges and walk them backwards only
   after a hit. Never store expanded transitive row combinations throughout
   the search.
7. Rebuild a compact exact frame from the winning source trace.
8. Feed that frame through the existing exact lifting, regenerated-source
   replay, guard extraction, strict-descent proof, owner publication, and
   artifact machinery.
9. Recurse over exceptional guard-zero cases until no positive-dimensional
   uncovered domain remains. Only fully fixed finite leaves may become
   explicit nonminimal terminals.

A modular hit remains discovery evidence only. It cannot directly create a
rule, terminal, owner, artifact, or closure claim.

### Case geometry

- Implement coordinate faces first because they cover the immediate
  vacuum/K6 pressure target and fit the existing rectangular owner-cover model.
- Define the subsystem boundary around a future exact affine-integer case
  representation `A n = b`, including a canonical chart `n = n₀ + B t`, so
  adding affine support later does not change GPLU, exact lifting, artifact, or
  reducer interfaces.
- If a coupled affine exceptional condition appears before affine support
  exists, return a typed incomplete or unsupported-case result and prevent
  publication. Never approximate it by sampled points or rectangular rays.
- If such a condition actually blocks K6, implement the exact affine service
  at that point after another Symbolica API audit. Otherwise defer its
  implementation and exhaustive testing to the generic Stage 2 lane.
- Non-affine exceptional factors likewise fail closed and are reported
  explicitly.

### Exact materialization and future reconstruction

Introduce a narrow internal `TargetRuleMaterializer` seam:

- `PrunedExactMaterializer` is the only implementation initially. It uses
  Symbolica exact rational-polynomial arithmetic on the compact dependency
  trace.
- A future `SymbolicaReconstructionMaterializer` may be added only after
  Symbolica provides the required sparse multivariate rational-function
  reconstruction API.
- Do not implement interpolation, CRT orchestration, FireFly-style
  reconstruction, or an independent rational reconstruction framework inside
  RustRed.
- Both materializers must return the same exact-circuit type and pass the same
  replay, guard, descent, and publication gates.

### Generic and vacuum execution lanes

Expose:

- `execution_lane = auto`
- `execution_lane = generic`
- `execution_lane = single-scale-vacuum`

`auto` must authenticate the mathematical family and select the vacuum lane
only when its requirements are proved. Explicitly requesting an incompatible
vacuum lane produces a typed error. `auto` is the default; the explicit lanes
are authenticated diagnostic and benchmarking overrides.

The vacuum lane may specialize aggressively by:

- normalizing the sole mass scale to one and restoring it by homogeneity;
- exploiting the absence of external shifts;
- sharing compact structural IBP rows across probes and workers;
- using Symbolica graph canonicalization and automorphism information to
  quotient equivalent sectors, targets, and orderings;
- using compact shift and column encodings;
- precomputing structural incidence once per family;
- racing a bounded symmetry-reduced ordering portfolio;
- avoiding exact expression construction until a winning support is known.

It must not dispatch on topology names or hard-code K6 relations. Every
resulting rule passes the same generic exact authority pipeline. Overlapping
vacuum fixtures must be solvable through both lanes with equivalent exact
reductions.

### Janet/Ore coexistence

Expose completion strategies:

- `spired`
- `janet-ore`
- `hybrid`

`spired` is the default for new campaigns. In this mode, no Janet arena,
queue, coefficient DAG, or completion state may be constructed. Tests and
profiles must demonstrate that Janet/Ore contributes no measurable work or
memory to a pure SpIReD K6 run.

`hybrid` may invoke Janet/Ore only through an explicit bounded policy,
initially for complement analysis, candidate-source proposals, small-case
differential checks, or fallback diagnostics. Janet queue exhaustion alone
never establishes family closure.

### Parallelism and strategy control

- Share immutable family structure, exact source definitions, graph data, and
  owner snapshots between workers.
- Keep only modular residues and sparse reducer state probe-local; avoid
  cloning Symbolica expressions per thread.
- Workers solve immutable cases or ordering probes and return proposals.
  Canonical exact publication remains deterministic and serialized at the
  owner-ledger boundary.
- Cap all RustRed, Symbolica, Rayon, BLAS, and nested compute pools under the
  configured worker budget.
- Start with a deterministic cost-guided ordering portfolio using fill, pivot
  progress, dependency-trace size, guard complexity, and predicted branch
  count.
- Permit a small bounded post-hit window to compare alternative pivots rather
  than accepting the first pathological rule.
- If profiling shows the 3× target is missed primarily because ordering
  choices cause fill or branching outliers, add a bounded MCTS or bandit
  ordering search with explicit early-abort thresholds. It must optimize
  generic structural metrics, not recognize benchmark names.

## Interfaces and artifacts

Extend the Rust library, `campaign` CLI, and public `import rustred` Python API
consistently with:

- `strategy`;
- `execution_lane`;
- worker and deterministic-probe controls;
- SpIReD progress and profiling output;
- generation, inspection, cold loading, and rule application;
- typed diagnostics for budget exhaustion, unsupported exceptional geometry,
  modular misses, exact-lift failures, and incomplete closure.

Keep `_rustred` private as the native extension implementation detail.

Bump the evolving campaign and artifact schemas rather than retaining RustRed
compatibility shims. Artifact envelopes retain strategy and execution-lane
reproducibility metadata, while only mathematical inputs, authenticated
ordinary-source provenance, case or guard ownership, exact rule payload,
terminals, homogeneity information, and deterministic content identity carry
reduction authority. External hints and transient search diagnostics remain
outside the authoritative artifact payload.

Update the Rust, CLI, and Python examples so K1 and K3 can be generated and
applied through `spired` without unnecessary explicit `parameter(...)`
declarations. Add a Python-driven K6 campaign example once K6 is genuinely
closed; it must remain autonomous and contain no FORM-derived rules or hidden
oracle hints.

## Stage sequencing

### Stage 1

1. Land the documentation, goal assignment, authorship, and local-notes ignore
   rule.
2. Implement incremental modular GPLU, dependency tracing, coordinate-case
   recursion, and the exact-materialization seam.
3. Reproduce the existing K1 and K3 closing artifacts through the new SpIReD
   lane.
4. Generate a cold-reloadable, zero-uncovered K6 artifact covering the complete
   frozen census of all five three-loop single-scale vacuum graph classes.
5. Ship K1, K3, and K6 artifacts with Vakint's `vakint_rustred` branch.
6. Complete Vakint's opt-in RustRed scalar backend using its existing topology
   match and routing witness. Do not rematch by graph or dispatch on topology
   names.
7. Use the FeynKit tensor prepass and RustRed scalar tail for the FORM-less
   acceptance lane. Do not develop another tensor reducer here.
8. Reuse Vakint's existing master evaluation machinery. A finite nonminimal
   RustRed terminal basis is acceptable if numerical parity is established.
9. Extend the existing comparative harness analogously to
   AlphaLoop-versus-MATAD and pass every applicable single-scale acceptance
   test through three loops with an invalid FORM path.
10. Profile and optimize scalar application as well as artifact generation
    before closing Stage 1.

### Stage 2

After Stage 1 acceptance:

- exercise the same generic architecture on complete K10, K15, and K21
  single-scale vacuum manifests;
- use the optimized vacuum lane to pursue four-, five-, and six-loop closure;
- implement full affine-integer cases if not already forced by K6;
- integrate the collaborator's tensor technology when it becomes available,
  without independently recreating it;
- retain finite nonminimal universal terminal bases where this improves
  scaling, provided closure is exact and the basis remains practical for
  high-precision evaluation;
- add Symbolica's rational-polynomial reconstruction backend when its public
  API becomes available, then reprofile all established campaigns.

No high-loop artifact may be claimed closed from bounded reachability, Janet
exhaustion, modular evidence, or sampled coverage.

## Benchmark and profiling objective

Gregor's Section 4.5 timings are a formal performance objective. For each
benchmark that can be reconstructed faithfully, RustRed should initially run
within 3× of the reported SpIReD timing without rational-polynomial
reconstruction and should ultimately beat it after Symbolica reconstruction is
integrated.

The shorthand rows in the current notes do not yet contain enough information
to qualify as reproducible fixtures. A row becomes eligible only when its
actual family, requested sectors or cases, relation sources, ordering or
strategy, terminal policy, expected coverage, worker count, hardware, and
timed boundary can be represented unambiguously. Domain-specific benchmarks
that cannot be reproduced here are excluded rather than replaced with
superficially similar K3 or K6 examples.

For eligible cases:

- use `--release --locked`; exclude compilation and dependency download;
- compare equivalent symbolic closing-rule workloads, not FIRE-style
  per-integral tables;
- prefer paired runs on the same host, same physical cores, memory limit, and
  frozen strategy;
- cap the primary comparison at six physical compute workers and also report
  serial timing;
- require the upper 95% confidence bound of the paired median
  RustRed-to-SpIReD ratio to be at most 3;
- separately report autonomous strategy search and a fixed winning-strategy
  comparison;
- prohibit FORM, MATAD, or AlphaLoop hints, persistent pivot traces, prior
  artifacts, fixture-specific dispatch, or terminal inflation.

Measure both:

1. matched solver-core time from prepared sources to exact guard-complete
   rules;
2. full logical-cold RustRed time from process launch and family input to a
   written artifact, followed by fresh-process validation and a canary
   reduction.

Record initialization, modular discovery, exact materialization, replay, guard
refinement, cover compilation, publication, and cold verification separately.
Also record CPU time, peak RSS, generated rows, sparse input/U/L nonzeros, fill
ratio, dependency-trace size, artifact size, branch count, and terminal count.

Until a Section 4.5 fixture is fully specified, the 3× comparison remains an
active objective rather than a Stage 1 blocker. K1, K3, and K6 remain mandatory
release-profile regression workloads but must not be presented as substitutes
for Gregor's benchmark families.

## Validation and acceptance

Required focused tests include:

- streamed Symbolica reduction versus batch rank or reduction;
- dynamic column insertion and stable target identity;
- shifted modular evaluation versus exact translated sources;
- unlucky-prime or sample retry without false authority;
- dependency-trace extraction and compact exact lift;
- guard branching, coordinate-face intersections, and fail-closed affine or
  non-affine cases;
- strict descent, regenerated-source replay, explicit terminals, and zero
  positive-dimensional complement;
- deterministic artifacts and reductions across supported worker counts;
- identical generic- and vacuum-lane results on shared fixtures;
- no Janet/Ore construction or work in pure `spired` mode;
- mass restoration by dimensional homogeneity;
- Rust, CLI, and Python surface parity;
- cold artifact validation and memoized master-only application;
- Vakint raw master-coefficient and numerical acceptance comparisons through
  three loops;
- unchanged Vakint defaults and existing backend behavior;
- end-to-end FORM-less Vakint tests with an invalid FORM path.

Run release K1/K3 tests after each search-engine slice and a bounded K6
campaign after every material change affecting discovery, ordering, exact
lifting, guards, or coverage. Run a full K6 closure attempt at each stable
milestone.

Every milestone ends with:

1. focused tests and release profiling;
2. an independent implementation audit;
3. an independent mathematical or closure audit;
4. documentation and benchmark updates;
5. a clean commit and push to the appropriate RustRed or `vakint_rustred`
   branch.
