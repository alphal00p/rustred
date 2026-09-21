# GOAL

Current execution priority is the September 21 shared five-loop campaign in
[GOAL.md](GOAL.md) and [its implementation plan](docs/five_loop_rank_campaign.md):
finish a nonminimal R=10 solve across the complete input census using shared
subtopology rules and parallel dependency work, at most 50 cores/500 GB, aiming
for completion within 15 hours. Monitor and optimize instead of imposing
30-minute deadlines. Independent certification is deferred. The sequential
gates are (a) complete bounded solve, (b) at least R=10/ideally R=20, (c) terminal
minimization, (d) numerical master catalog, (e) five-loop Vakint integration.
The original design below remains background where it does not conflict with
that current directive. Symbolica's native reconstruction API is now available
and already used; the original prohibition concerned implementing our own CAS.

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
   after a hit. Preserve the resulting dependency-topological row order as
   well as a separately canonicalized support identity; never replace the
   successful triangular schedule with a lexicographically sorted row order.
   Never store expanded transitive row combinations throughout the search.
7. Rebuild a compact exact frame from the winning source trace, presenting
   rows to the symbolic reducer in that dependency-topological order.
8. Feed that frame through the existing exact lifting, regenerated-source
   replay, guard extraction, strict-descent proof, owner publication, and
   artifact machinery.
9. Recurse over exceptional guard-zero cases until no positive-dimensional
   uncovered domain remains. Only fully fixed finite leaves may become
   explicit nonminimal terminals.

The systematic walk in item 2 is a walk over **source translations for one
symbolic case**, not a claim that a finite corner/depth-one walk over target
integrals is exhaustive. Target nominations are an optimization. The
coordinate lane should normally use the zero-shift logical target `I(n)` on
the case domain. If a seeded system exposes a pivot `I(n+a)` which satisfies
the same case, then `a` lies in the case's integer tangent lattice and shifting
the complete finite source witness by `-a` recenters that pivot to `I(n)`.
Fair source translations therefore provide the completeness mechanism;
walking a portfolio of concrete target powers does not. Nonzero target
nominations remain a fill/order optimization only. A bounded target portfolio
that finds no owner remains incomplete while the fair source shell can still
grow. The outer
convergence state is the canonical worklist of **equality-only** cases produced
by guard-zero refinement, with the exact owner compiler serving as the
separate final closure authority. Discovery cases and published owner domains
are intentionally different objects: discovery substitutes only the equations
in `C`, whereas an owner's applicability domain also retains the rule's
nonzero conditions.

For the linear case regime described in Gregor's notes, the termination
measure is the rank of the equality lattice, not the number of target points
already tried. Once a rule has been admitted on `C & g1 != 0 & ... & gr !=
0`, discovery enqueues the overlapping equality-only cases `C & g1 = 0`, ...,
`C & gr = 0`. They are canonicalized in integer RREF or HNF form, scheduled
most-generic-first, and removed when a pending weaker case subsumes them.
Only the later owner/coverage compiler turns the complement into disjoint
first-zero domains such as `C & g1 = 0` and `C & g1 != 0 & g2 = 0`. Keeping
those nonzero prefixes out of discovery is what permits Gregor's generic-case
subsumption and fixed-coordinate grouping.

An inconsistent child is dropped. A zero equation already implied by `C`
means that this candidate does not own any part of the case and the fair
source search must resume. Every other linear zero equation increases the
saturated equality rank by at least one. Consequently, if every nonterminal
case has a finite translated IBP witness and all exceptional loci are
affine-linear, recursion reaches a fully fixed finite leaf after at most `K`
independent refinements along any branch. Such a leaf may be retained as an
explicit nonminimal terminal under the configured policy; it is not inferred
to be an irreducible master by a failed bounded search.

Inactive-line activation faces participate in the same measure. The exact
combined coefficient is restricted to each finite activating slice. A slice
on which it vanishes needs no child; a surviving slice is an explicit
lower-dimensional case. This is the precise systematic replacement for a
coefficient-blind supersector prohibition.

For an exact affine case `A n = b`, this recentering argument requires a
saturated integer chart `n = n0 + B t`, all ambient source translations, and
translation before chart substitution. Every eligible pivot displacement is
then represented in the integer tangent lattice generated by `B`, so a fair
source schedule eventually exposes the recentered form of any finite witness.
Alternative target nominations may improve fill or find a good rule sooner,
but are not the completeness mechanism. Eventual discovery requires a finite-
support exact witness in the localized translated-IBP module for every
unresolved positive-dimensional case, with an admissible strict orientation
and exact guard/boundary authority. Finite Ore-module rank can motivate that
expectation but does not establish all of those conditions by itself, and a
finite numerical master count is not proof of the stronger uniform statement.
A bounded miss therefore pauses the source walk and never creates a master.
Terminal eligibility depends on the exact integer sector slice being finite,
not merely on `rank(A) = K`: bounds can also make a lower-rank affine slice
finite.

Finite-field specialization adds a second fairness obligation. A valid exact
pivot can disappear at one unlucky prime or sample point, so increasing source
depth forever in only that reducer is not a complete discovery schedule. The
production driver must interleave a deterministic, unbounded sequence of
independent admissible probes with increasing signed-L1 source depth (a
diagonal schedule is sufficient in principle), while racing a small fast
portfolio first in practice. For any finite exact witness whose relevant
minor is a nonzero rational polynomial, that schedule must eventually reach
both its finite source shell and a specialization where the minor survives.
Modular hits still receive no authority; exact compact replay makes unlucky
rank gains harmless, and a bounded probe/depth rectangle remains a resumable
incomplete result.

This makes the implementation an exact semi-decision procedure under a clear
algebraic hypothesis. Translated ordinary IBPs generate a left ideal in the
rational double-shift algebra. If its localization on every unresolved
positive-dimensional case has a finite-support relation whose leading term is
the canonical target and whose other terms satisfy RustRed's strict order and
boundary rules, fair source/probe enumeration finds it. Finite master count
and a finite standard-monomial staircase are strong evidence for this
zero-dimensional behavior, but they do not replace the exact per-case witness
and guard proof. Full-rank finite leaves need no irreducibility claim: they may
be recorded explicitly as a finite nonminimal terminal basis.

A modular hit remains discovery evidence only. It cannot directly create a
rule, terminal, owner, artifact, or closure claim.

Nor may the first exactly replayed hit end a case search merely because it is
algebraically valid. If its guards or boundary terms cannot own the requested
case, retain its diagnostic or branch proposal and continue the fair source
walk or bounded post-hit candidate window. Only an admitted owner, an
explicitly certified finite terminal, or a typed resource pause may retire the
current search invocation; an inconvenient pivot must not hide a later clean
one.

Source-exclusion branches alone are not a complete post-hit window: after a
rejected support `S` they explore systems omitting at least one row of `S`, but
a better relation may retain all of `S` and add later rows, or use another
linear combination of the same row span. The production runner must therefore
also continue the original all-row stream and expose additional target
dependencies (or an equivalent bounded nullspace/back-substitution portfolio).
Exclusion remains useful for cheap alternate supports, but exhausting it is
not evidence that the case has no admissible pivot.

Continuing the modular stream is not sufficient by itself.  A compact exact
rerun that merely asks for the first target pivot will rediscover the earlier
rule even when a later dependent row nominated a better circuit.  The exact
materialization seam must therefore support a root-constrained variant.  One
Symbolica-backed construction is to reduce the selected rows over the
forbidden columns augmented by source-provenance columns, require the newly
streamed root row to participate, read the resulting exact source
combination, and evaluate its target coefficient separately.  Only a
nonzero exact target coefficient may be normalized into a candidate circuit.
The complete relation is then regenerated and replayed through the ordinary
authority pipeline.  The modular `L` pattern chooses a small candidate frame
and chronology; it never supplies an exact coefficient.

Gregor's observation that translation-related fixed-coordinate cases can be
seeded together is an amortization layer over this same logic.  A shared
stream may watch several stable logical target columns and continue after
one of them hits, but each case still has its own exact specialization,
guards, boundary checks, terminal policy, and owner publication.  Batched
case solving is therefore a K6 performance objective after the scalar
single-case chronology is authoritative, not an alternative closure proof.

Observed uncovered integer tuples are only anchors from which the driver may
extract the weakest exact affine equality pattern supported by the live
complement. They are never appended as ersatz IBP generators. Solving every
point separately turns an unbounded ray into infinitely many full-rank tasks,
while adding a "topology" at another integer power does not enlarge the module
generated by the ordinary translated relations. New relation sources may be
added only when they are independently derived and exactly replayable (for
example authenticated symmetries or syzygies). The default convergence move is
instead `uncovered anchor -> maximal symbolic equality case -> one guarded
rule -> exact lower-rank equality children`.

The first K6 execution target is a derived, not hard-coded, bulk-to-boundary
walk: solve the path/star interior with three inactive powers symbolic, enqueue
the coefficient-checked `z=0` activation or guard face, then its
two-coordinate face and remaining one-coordinate ray, and finally discharge
only the exact finite leaf through authenticated factorization or the explicit
terminal policy. If the generated guards are coupled linear forms, that run
triggers the exact affine chart rather than replacing them by sampled
coordinate points.

Column admissibility must also follow Gregor's coefficient-aware boundary
test. A shift which activates an inactive line only on a lower-dimensional
sector boundary is not globally forbidden merely because that boundary
exists. The candidate is valid across the boundary when the exact coefficient
of that term vanishes after restriction to every activating face. Otherwise
the guard-free interior may still be owned and the activating faces become
explicit case obligations. Modular discovery may propose this structure, but
exact Symbolica substitution on every boundary face is required before it can
affect ownership. Treating `InactiveLineActivation` as an unconditional
whole-case forbidden column is a conservative diagnostic fallback, not the
production SpIReD case algorithm.

The depth-dependent inactive-axis bulk used while streaming rows is only a
temporary discovery envelope. It may shrink as a larger source shell admits
larger positive shifts, but those provisional faces must not be published as
mathematical exceptional cases. After a modular hit has been lifted exactly,
RustRed recomputes the weakest safe bulk from the compact winning circuit's
actual retained shifts, replays the rule there, and enqueues only the finite
activation faces of that final circuit. Otherwise search depth would leak into
artifact geometry and create spurious K6 branches.

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

### Sector-source preconditioning and grouped cases

Gregor's search starts from a sector- and ordering-specific optimized basis of
the ordinary IBP/LI rows rather than repeatedly seeding the raw derivative
rows. Add an optional, exactly replayable preconditioner which uses
Symbolica's sparse exact row reduction once per immutable sector/order,
removes dependent rows, clears coefficient denominators without unitarizing
pivots, and retains the transformation back to the generated ordinary-source
authority. Benchmark this against the raw basis: enable preconditioning only
when its reduced translated-row count and downstream fill justify its exact
setup and retained payload. It is a performance transformation, never a new
relation source or closure argument. A fraction-field row transformation may
lose rank after specializing an exceptional index case. Unless its change of
basis is proved unimodular or otherwise specialization-safe on that case, the
optimized rows therefore run as a fast front porch and the fair raw ordinary-
source stream remains the eventual-discovery fallback.

After correctness of the equality-case driver, group coordinate cases that
differ only by integer values of fixed coordinates when translations connect
their charts. One fair translated-source stream may then nominate pivots for
several members, as described in the notes, but every emitted rule and guard
branch is still specialized, replayed, and owned against its individual exact
case. This is especially important for the many full-rank numerical leaves;
it must not turn a bounded group miss into a terminal claim.

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
