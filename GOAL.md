# RustRed project goal

## Preamble — user directive (verbatim)

````text
Ok, but did you not read all the markdown listed in the HANDOFF (especially in `./docs/research`)?

the goal is very ambitious, and it is to get inspiration from LiteRed2 (see `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/LiteRed2`) which is a poorly software-designed software in Mathematica (you cannot run it) to generate closing parametric IBPs).

you must implement a version of that, RustRed, in fully rust and maximally using Symbolica (see public API in `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/symbolica`), optimally parallelized and highly efficient, to push to up to 6-loop IBPS (using algorithms fully general in topology and loop counts) suitable in practice for example to build the parametric IBPS for the 6-loop single scale vacuum integral showing up in the 6-loop QCD beta function we aim to compute as a breakthrough eventually using the R-formulat for BPHZ renormalisation as implemented in gammaloop (e.g. see `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop`), so such IBPs can be used in a new reduction mode of vakint (see `./FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop/crates/vakint`) which is *pure* rust and symbolica, with numerical masters that will come from AMFlow.

Anyway this is a highly ambitious project you have to delegate multiple agents to (you're mostly just the orchestrator) with the main goal for now to be able to produce closing  parameteric IBPs for 6-loop single scale (actually no scale effectively, m\_uv can be set to 1) vacuum integrals, which will be used for the rest of the program culminating in the 6-loop QCD beta function (though all rustred algos must remain fully generic, but with dedicated lanes highly optimized for that use-cased dynamically automatically opted into).

Re-read thoroughly the HANDOFF, the existing code, and the research markdown files, and layout the goal clearly in GOAL.md (incl. this message verbatim as a preamble) and assign it to yourself.

Remember you must design a very clear and well structured professional code, highly optimized and using Symbolica for all CAS taskes (triple-check any CAS feature really is not available in Symbolica before implementing your own solution for such CAS task).

To unlock multicore use of Symbolica, you can and should use:

`export SYMBOLICA_LICENSE=`dcec4a5e#6a95649c#7dca8216-8afe-57c8-975e-03eb5e68e4ee
(not sensitive info)

Remember, before implementing in vakint the application of the parametric IBPs for performing the reduction (NEVER USE FORM) you must first focus on being able to find them for 6-loops with RustRed (first goal). Howver you can use vakint as an oracle to compare reductions (up to four-loops which is the max currently supported in Vakint). RustRed only generates the parametric IBPs, then a new tensor reduction and IBP application mode `RustRed` in vakint will use rustred to actually perform the reduction But that new mode and anything you do SHOULD NEVER USE FORM, MATHEMATICA, Sympy or anything like this. Pure RUST + Symbolica implementation here.
Also never escalate commands but find workaround if sanbox is being hit.
````

## Assignment and status

The verbatim preamble records the original directive. Later user
clarifications supersede only its strictly serial Vakint sequencing: as soon
as mandatory Phase 0 restructuring closes, the orthogonal FORM-less Vakint
tensor-reduction lane starts concurrently and does not wait for closed
lower-loop shards, six-loop efficiency, or six-loop closure. Guarded IBP
application in that lane begins as soon as the first usable, genuinely closed
lower-loop shards exist. The six-loop foundry remains the primary mathematical
objective, and production/default execution remains pure Rust + Symbolica.

- **Status:** active, long-horizon engineering and research goal.
- **Owner:** the primary Codex agent (`/root`), acting primarily as architect,
  orchestrator, integrator, and final verifier.
- **Delegation:** research, bounded implementation slices, performance work,
  and adversarial audits are assigned to subagents whenever they can proceed
  independently. The primary agent owns reconciliation, dependency decisions,
  integration, and capability claims.
- **Pre-reset baseline:** repository commit `dda284a` (`Extract committed
  exceptional reentry construction`). A completed production-liveness audit
  subsequently classified the private solver/exact-session/closure/
  publication/re-entry stack and its dependent generated-affine provider
  layers as one dead prototype island. Earlier milestones remain Git evidence,
  not architecture that the reset must preserve.
- **Cross-repository boundary:** immediately after Phase 0, Vakint/GammaLoop
  tensor-backend integration proceeds in parallel with the RustRed foundry;
  guarded rule application joins it when the first genuinely closed lower-loop
  RustRed artifacts exist. It is developed, committed, and pushed on a
  dedicated `vakint_rustred` feature branch created in the GammaLoop
  repository, never folded into a RustRed commit. In the present
  ignored co-development layout, while the core package remains temporarily at
  the RustRed repository root, Vakint may use `rustred = { package =
  "rustred", path = "../../../../" }`; after Phase 0 moves that package, the
  local path becomes `../../../../crates/rustred-core`. Either local form makes
  uncommitted co-development changes visible immediately. Before a
  reproducible GammaLoop milestone is committed/pushed
  or cited as oracle evidence, the dependency is switched to the RustRed
  GitHub repository at the exact validated revision; milestone updates advance
  that pin deliberately. Full six-loop artifact deployment and the production
  GammaLoop boundary remain later gates.
- **Milestone discipline:** make frequent rollback-sized commits and push each
  relevant intermediate milestone after its declared checks pass. Do not let
  unrelated architectural, solver, or cross-repository changes accumulate in
  one opaque commit.
- **Immediate posture:** the first substantive step is to restructure the
  current RustRed code cleanly around the stated end goal. Complete the
  mandatory heavy structural refactor by deleting the audited dead solver
  island and retaining only a small, demonstrably live generic capability
  spine. The production foundry is then rebuilt cleanly rather than resumed.
  Structural work is an enabler, not evidence of mathematical closure, but a
  clear production architecture is a prerequisite for a maintainable and
  scalable six-loop implementation.

## Assigned objective

Build RustRed into a production-grade, topology- and loop-count-independent
offline rule foundry, written in pure Rust and using GMP-enabled Symbolica as
its CAS, that derives guarded, strictly descending, replay-certified,
coverage-closed parametric IBP/LI replacement systems and persists them as
deterministic reusable artifacts.

The first scientific pressure target is to close physical six-loop,
single-scale massive-vacuum families after a verified `m_uv^2 = 1`
specialization. For a complete six-loop vacuum family this means 21
scalar-product coordinates and 36 ordinary parametric IBP sources. It does
**not** mean merely generating those 36 identities: every reachable generic
and exceptional domain must be discharged onto strictly lower closed
dependencies or a finite explicitly enumerated terminal set. Master keys in
that set must be explicitly selected by the user/versioned manifest; zero,
product, and factorized terminals must be independently certified. Every
emitted rule is replayed exactly against freshly regenerated generic
identities.

The six-loop lane must be a dynamically selected specialization of the same
generic algorithms. Selection may depend on proved semantic properties such
as vacuum kinematics, a common nonzero mass, valid unit-mass homogeneity, the
coefficient field `Q(d)`, graph structure, and available routing witnesses. It
must never dispatch on a topology name, a hand-authored family label, or a
condition such as `loop_count == 6`.

The one-off foundry deliverable is a versioned, verified library of closing
parametric IBP artifacts covering the canonical single-scale vacuum-topology
domain through six loops. “All vacuum topologies” is made testable by a frozen
completeness manifest over the declared GammaLoop/Vakint graph domain after
its documented routing, fusion, and canonicalization equivalences; every
manifest entry must map to a closed family artifact. New supported topology
domains extend that manifest and regenerate artifacts without adding
topology-specific solver code. The completed artifacts are saved and shipped
with Vakint rather than rediscovered during ordinary integral evaluation.

The user-facing steering surface for the complete evaluation chain is a
pure-Rust + Symbolica `RustRed` mode in Vakint. Its lower-loop implementation
and validation proceed concurrently with higher-loop foundry work, and it
grows into the complete production surface as the artifact library expands.
Vakint owns orchestration and presentation; the RustRed crate implements the
mathematical services it calls for native tensor reduction,
scalar-product/propagator lowering, compiled guarded IBP application, and typed
master substitution. Master data will come from AMFlow, but loading,
validating, and applying that data in this mode remains FORM-less
RustRed/Vakint functionality. Through GammaLoop's existing BPHZ/R-operation
boundary, this chain is intended to contribute to the six-loop QCD
beta-function programme.

Tensor reduction is a reusable RustRed-core capability, not an algorithm
implemented inside the Vakint adapter. Its initial public domain model and
dispatch boundary expose two topology-neutral lanes. The implemented fast lane
targets families proved to be single-scale vacuum kinematics with no external
momentum shift in any denominator; `Auto` selects it from those semantic
properties, never from topology names. A generic lane over external kinematics
and shifted denominators is present from the first API milestone and initially
returns an explicit typed `Unsupported` result until its implementation is
complete. Vakint translates its matched topology/numerator into this RustRed
request and translates the result back; the same service remains directly
usable by RustRed's Rust, CLI, and Python surfaces.

The RustRed API separates Lorentz projection from family-aware scalar
lowering and also offers their composition. Projection is driven by the typed
numerator, loop/external/spectator bindings, dimension, and caller-supplied
Symbolica heads; topology names, masses, and propagator powers are irrelevant
to that stage. The authenticated family/domain evidence gates the optimized
projection-plus-lowering composition and subsequent integral-index shifts.

That fast-lane proof cannot be a caller-supplied boolean and cannot currently
be reconstructed from `IntegralFamily` alone, because the present family model
does not retain physical-denominator versus auxiliary-ISP roles. Phase 0/early
tensor input design must retain authenticated denominator-role, shift, and
common-scale metadata and let RustRed mint a sealed `VacuumTensorDomain` (or
equivalent) token. `Auto` consumes that RustRed-owned evidence. External
spectator vectors appearing only in a tensor numerator do not disqualify the
fast lane; external shifts in physical denominators do.

The lower-loop Vakint mode is an early parallel validation track: its generic
tensor-reduction backend starts immediately after Phase 0, while its IBP
application tests start once closed RustRed shards exist. Its complete
six-loop deployment, GammaLoop integration, AMFlow master-data production, and
final physics computation remain downstream gates. Parallel adapter work must
not displace the first offline one- through six-loop vacuum artifact-library
goal or be presented as its completion.

Vakint's integrated steering role does not replace RustRed's own interfaces.
The RustRed CLI and Python API remain first-class, supported, fine-grained
control surfaces over the shared application/core path. They currently expose
the baseline derive, roots-only campaign-planning, and preflight operations.
They will expose family construction, raw parametric IBP/LI generation,
closure campaigns, artifact inspection and exact verification, and the
individual tensor, scalar-reduction, rule-application, and master-substitution
services as those application contracts are implemented and evidenced. The
target interfaces accept fully generic families with external kinematics,
masses, cuts, symbolic power shifts, and numerator/ISP coordinates; they are
not restricted to vacuum topologies or to the precomputed Vakint library.

## What “closing parametric IBPs” means

A family/root is **closed** only if its immutable artifact proves all of the
following:

1. At the durable artifact boundary, the family, coefficient/index contexts,
   kinematics, metric and propagator signs, mass convention, routing, cuts,
   power shifts, ordering, and source identity set are bound once to a
   canonical identity/fingerprint. Trusted typed values inside the producing
   process are not repeatedly reauthenticated.
2. All required ordinary parametric IBPs and separate LI identities are
   generated generically. For `L` loops and `E` external momenta the complete
   coordinate count is `K = L(L+1)/2 + LE`, with `L(L+E)` ordinary identities
   and `E(E-1)/2` LI identities.
3. Every published rule has an exact integer-domain guard, all required
   nonzero polynomial guards, a strict well-founded descent proof, source-row
   provenance, and an exact zero residual against freshly regenerated source
   identities.
4. Zero, factorized, symmetric, mapped, and proper-subsector dependencies are
   proof-bearing and use verified transport maps.
5. Every exceptional equality/nonzero branch is recursively processed until
   it reaches a descending rule, an already closed dependency, an explicitly
   enumerated and selected master key, or an independently certified finite
   zero/product/factorized terminal. A symbolic residual domain is never a
   terminal.
6. Solved proper subsectors feed back immutably into their parents and the
   dependency graph reaches a deterministic fixed point.
7. No reachable leaf is `Uncovered`, `Unsupported`, resource-limited,
   interrupted, timed out, search-exhausted, or unresolved.
8. The artifact can be validated once when loaded from an untrusted durable or
   cross-process boundary, without trusting the process that produced it, and
   then represented by a trusted typed owner. Resumable incomplete workspace
   state is never confused with an installed `Closed` artifact.

Failure to find a rule is not a zero proof and is not a master proof. A finite
sample is candidate-discovery evidence only. Numerical agreement is
corroboration only; symbolic regenerated-source replay is mandatory.

## Non-negotiable engineering constraints

### Pure Rust + Symbolica algebra

- RustRed production, the new Vakint `RustRed` mode, and their ordinary/default
  tests must never link to or execute FORM, Mathematica, SymPy, or another CAS.
  Mathematica source and FORM-backed resources are readable reference oracles
  only.
- A separate pinned oracle-validation job may execute the existing Vakint
  alphaLoop, MATAD, and/or FMFT paths with a real FORM >= 4.2.1 executable.
  The currently supplied `FOR_REFERENCE_ONLY_DO_NOT_PUSH/form5` directory is a
  React/Node project, not that executable, so live oracle regeneration is
  presently unavailable while Vakint's actually embedded inline expectations
  remain extractable. This
  exception exists solely to produce or compare authoritative lower-loop
  reference results. FORM is never a RustRed dependency, never a fallback of
  the new mode, never copied into production logic, and never part of the
  default FORM-less test/runtime path. Oracle executable/version, inputs,
  conventions, and frozen outputs are authenticated explicitly.
- Use the public Rust API of GMP-enabled Symbolica for all CAS work: exact
  integers/rationals, expressions, substitutions, expansion, polynomials,
  rational functions, differentiation, GCD/factorization, Groebner reduction,
  finite fields, CRT/rational reconstruction, matrices, sparse row reduction,
  and tensor canonicalization where semantically applicable.
- Never enable Symbolica's `no_gmp` feature. The license is supplied through
  the environment before Symbolica initialization or worker-pool creation and,
  outside the required verbatim preamble above, is not duplicated in project
  documentation or logs.
- RustRed owns physics/domain semantics, typed integral keys, ordering,
  guards, provenance, replay, resource admission, scheduling, durable
  artifacts, and structural tensor semantics. A future foundry may introduce
  narrowly justified state-transition primitives, but the deleted prototype's
  transaction/session design is not an inherited authority. None of these
  responsibilities authorize a second algebra engine.

### Mandatory three-stage CAS check

Before adding any bespoke algebraic operation:

1. Define the exact operation and domain, pin the Symbolica revision/features,
   and search public exports, source, Rustdoc, examples, tests, internal call
   sites, and existing RustRed Symbolica adapters.
2. Attempt a checked composition of public `Atom`, polynomial,
   rational-polynomial, matrix, sparse-reducer, Groebner, finite-field,
   CRT/reconstruction, and pattern APIs as appropriate.
3. Compile and run a minimal licensed-GMP probe covering empty, zero,
   singular, overflow, bad-sample, short-map, resource-boundary, equivalent-
   spelling, and deterministic parallel cases; authenticate the result by
   exact substitution, products, divisibility, or regenerated residuals.

If the required semantic is still absent, record the exact searched APIs,
why composition is insufficient, the precise upstream capability required,
and return a typed unsupported/pause boundary. Do not implement a substitute
CAS primitive. Repeat the audit whenever the Symbolica revision or feature
graph changes.

Known current gaps include public SNF/HNF, integer-kernel/complete affine-
lattice parameterization, module syzygies, complete multivariate rational-
function reconstruction, fallible resource-censused algebra sessions, and a
proved selective tensor-subtree expansion preserving opaque spectators. The
literal-unit affine-equality subset may compose public Symbolica primitives;
general no-unit or simultaneous cases currently stop at the typed
`RequiresIntegerNormalForm` boundary.

### Genericity and evidence

- Production algorithms accept families, sectors, graphs, maps, domains, and
  policies as data. Loop/topology-named inputs may live in tests, benchmarks,
  or external frozen oracle fixtures; authored recurrence implementations do
  not live in any RustRed package.
- Core production modules, type names, branches, and lookup tables never name
  or dispatch on a concrete topology. Explicit topology names belong only to
  test/benchmark inputs, external oracle fixtures, Vakint's user-facing
  registry, or shipped artifact metadata.
- Optimized lanes are permitted only for generic, proved semantic subclasses,
  such as vacuum kinematics, single/no scale after specialization, uniform
  unit mass, coordinate density, or a verified symmetry class. A lane such as
  `single_scale_vacuum` accepts arbitrary qualifying families and loop counts,
  is selected dynamically from authenticated family properties, and shares
  the generic correctness path. It may tune algorithms by measured sizes but
  may not embed a six-loop/topology recurrence or a named-family case.
- No eager `2^K` sector/orthant enumeration at high loop order, no brute-force
  `GL(L,Z)`/bounded `3^(L^2)` symmetry discovery, no eager global exact
  rational-function elimination, and no authored recurrence dispatch.
- Symbolica's intrinsic public `symbolica::graph` implementation is the graph
  authority. Existing-topology symmetry discovery calls `Graph::canonize()`
  and consumes the resulting `CanonicalForm` automorphism generators;
  optional finite topology-domain enumeration uses `GenerationSettings` and
  `Graph::generate`. RustRed supplies physics-aware colors and interprets the
  recovered permutations as routing candidates. Parallel propagator swaps
  and momentum-orientation flips are represented as colored subdivision/flag
  vertices so they are explicit vertex automorphisms rather than hidden edge
  cases. Only the generic exact affine/momentum-map verifier may certify a
  candidate. GammaLoop/feyngen remains reference-only evidence for how a
  caller can encode and orchestrate graph work; it is neither a dependency nor
  an implementation authority.
- Algebraically equivalent numerator spellings, especially explicit
  propagator cancellation versus an uncancelled numerator factor, must reduce
  to identical masters and coefficients.
- The trees under `FOR_REFERENCE_ONLY_DO_NOT_PUSH/` remain untracked reference
  material and must never be staged or pushed to the RustRed repository. The
  parallel authorized Vakint work uses the separate GammaLoop repository
  history on its dedicated feature branch; this does not turn the reference
  tree into RustRed-owned content.
- Never request privileged command escalation. Use an in-scope workaround or
  report a genuine external blocker.

## Required architecture and authoritative repository reset

The 2026-08-28 reset directive supersedes the former root-package location,
four-package baseline, gradual legacy preservation, and legacy-test repair
policy. Its tracked execution specification is
[`docs/research/repository_clean_architecture_plan_2026-08-28.md`](docs/research/repository_clean_architecture_plan_2026-08-28.md).
Phase 0 is now a stop-the-line repository reset: delete obsolete surfaces
first, retain only demonstrably live generic core, and only then begin a fresh
foundry implementation and the parallel Vakint implementation.

The Phase-0 target is a **virtual Cargo workspace** with no root `src/`,
`tests/`, or package. At that gate, the deliberately bounded live packages
are:

```text
rustred-python ------> rustred-app ------> rustred
                            |                 |
                            +-- CLI           +------> Symbolica[gmp]

Vakint `RustRed` mode -------------------> rustred

Cargo package `rustred` will live at crates/rustred-core.
```

`rustred` is the topology-neutral mathematical core and becomes the
implementation home for tensor reduction, scalar lowering, guarded rule
application, and typed master substitution. `rustred-app` is the typed
composition layer and CLI host. `rustred-python` is a thin PyO3/maturin adapter
over `rustred-app`,
with no algebra or independent schema; users always write `import rustred`,
while a native `_rustred` extension may remain a private packaging detail.
Vakint is the concurrently developed lower-loop and ultimately complete
user-facing steering layer over RustRed core services; it must not duplicate
their algebra or silently fall back to FORM. The CLI and Python package remain
parallel first-class interfaces for fine-grained generic work rather than
compatibility shells around Vakint.

The publish-disabled `rustred-legacy-oracles` package, the core bridge/feature
created solely for it, loop-authored standalone tools, obsolete test wrappers,
old broad examples, stale LiteRed2/GammaLoop gitlinks, superseded dated docs,
and compatibility APIs are deleted. Git is their archive. Do not update old
paths, schemas, re-exports, or tests merely to keep them compiling before
deletion. Root integration tests are not mechanically ported: extract only a
small fresh generic contract matrix around retained services. There is no
RustRed backward-compatibility promise during this development reset.

Additional subcrates remain possible only when they create a demonstrable,
acyclic ownership or dependency boundary, materially improve independent
testing/compilation, and avoid a new dumping ground. Do not create transport-
only or one-file microcrates merely to make the tree look modular.

The Phase-0 core is deliberately smaller than the eventual product. The
completed liveness audit found no application/CLI/Python/Vakint production
caller for the existing private `src/solver/**` tree or for its exact-session,
closure, publication, re-entry, and generated-affine/provider dependants.
Those files are deleted as a prototype island; they are not moved, renamed, or
used as the skeleton of a production foundry. Consequently Phase 0 ends with
no production `foundry`, private solver, or closed-artifact publisher.

Within the retained Phase-0 core, dependency direction must be explicit and
acyclic. In this diagram `A -> B` means that A may depend on B:

```text
rustred-app -> input, identity, sector, campaign, tensor, reduction
Vakint RustRed mode -> input, tensor, reduction

reduction -> identity, sector, tensor, family, algebra
input -> tensor, family, algebra
tensor -> family, algebra
identity -> family, algebra
sector -> family, algebra
campaign ->                             # never foundry
family -> algebra
algebra -> Symbolica public Rust API
```

The Phase-0 core directories are `algebra`, `family`, `input`, `identity`,
`sector`, `campaign`, `tensor`, and `reduction`. `campaign` remains
low-level work/resource infrastructure. `reduction` retains only generic
rule-application and master-substitution primitives with real callers; it
does not imply that a closed rule library already exists. The core has no
`application/` directory because `rustred-app` is the composition layer.

After the reset gates pass, a fresh `foundry` and a stable `artifact` domain
are introduced from their required contracts. The new foundry composes the
retained identity, sector, campaign, family, and algebra services and
emits immutable artifact values that reduction can consume. Its internal
algorithm modules are designed from the mathematical closure requirements and
Symbolica APIs, not from the deleted exact-session/closure directory shape.
Artifact models never depend on foundry internals. This is the future
dependency direction, not a promise that any audited prototype implementation
survives Phase 0.

Private implementation names and files should describe mathematical roles,
not historical implementation chronology. Code is factored to the highest
professional standard practical for this project: cohesive modules, narrow
interfaces, explicit owners, acyclic dependencies, minimal visibility, and no
duplicated authority. Only essential generic tests are co-located or rebuilt
at true integration boundaries; historical campaigns are deleted. Public
re-exports are narrowed. The fresh solver is not developed inside mechanical
file moves. Git is the archive; stale code and documentation are deleted after
their unique evidence is retained.

The former flat `parametric_coefficient` migration unit has been replaced by
the acyclic `algebra::indexed` tree, and the former `runtime` wrapper is gone.
Algebra now has one raw Symbolica polynomial owner plus clear base-field and
index-field owners; remaining transitional wrappers are challenged during the
next semantic pruning pass rather than preserved for compatibility. Stored
polynomial exponents use Symbolica's native `u16`; only checked `u32`/`u64`
prospective arithmetic is used where an operation can widen. `u128` is not an
exponent representation. Self-only sparse-solver, associate, residual-affine,
parameter-identity, transcript, census, and compatibility machinery is
deleted. This includes the old partial-specialization/replay stack, aggregate
concrete-specialization authorization/census stack, exact-Integer translation
lane, retained-payload serializers, two-phase division seam, and unused
integer-matrix adapter. Symbolica primitives are wrapped only when a real
invariant or an infallible native trait boundary requires it.

Reducing the flat root is not permission to cram unrelated responsibilities
into a few giant files. Stable domains use parent modules and short role-named
children; large retained implementations are split along value, algorithm,
policy, error, and test boundaries when those boundaries are real. Every
multi-thousand-line survivor is an explicit cohesion audit item during Phase 0,
and its default outcome is a semantic submodule tree rather than a monolith or
a renamed dumping ground.

Legacy paths are not permanent architecture. Remove obsolete compatibility
layers, V1/V2 bridges, handwritten production algebra, loop-authored reducers,
dead modules, and stale tests/docs after extracting only genuinely unique
current evidence into the new stable design/validation documents. Nothing is
retained in a legacy package. “Legacy” is never an acceptable production
dependency.

### Trust-boundary and compatibility discipline

Authentication is proportional to the boundary. RustRed validates untrusted
user input, deserialized durable artifacts, cross-process or cross-repository
handoffs, and final artifact installation.
Once data has crossed such a boundary, sealed constructors, move ownership,
borrowed views, and typestate carry the invariant. Internally generated values
must not accumulate repeated fingerprint comparisons, schema round trips,
full proof replay, or other authentication ceremonies merely to cross private
functions. Exact closure evidence and optional independent artifact audit
remain mandatory, but they are not licenses for redundant hot-path checking.

RustRed is pre-release: its Rust APIs, CLI/Python details, internal schemas,
workspace state, and artifact formats have no backward-compatibility promise
while the architecture is being cleaned. Replace or delete obsolete forms
directly and update fixtures/callers in the same milestone; do not retain
compatibility shims. Vakint is different: its existing user-facing inputs,
steering behavior, serialized data, supported backends, and accepted results
must remain backward compatible. The Vakint `RustRed` mode is additive, and
its regression gates must prove that existing modes and end-to-end tests keep
working while the new FORM-less path is introduced. Initially this means new
opt-in methods/builders with `RustRedOptions`, not a new variant in Vakint's
public exhaustive `EvaluationMethod` enum or a field added to its public
settings layout; either source-breaking change requires a deliberate versioned
API decision and downstream audit.

## Execution roadmap

### Phase 0 — mandatory heavy refactor and authority cleanup

This phase is the immediate stop-the-line implementation step. Historical
exact-session/closure/publication/re-entry milestones remain Git evidence
only. The completed liveness audit found that stack and its dependent
generated-affine/provider layers to be a private prototype island with no
user-facing production caller. Delete it wholesale instead of untangling,
moving, or preserving its sessions, transactions, rollback machinery,
exceptional-child sentinels, schemas, tests, or directory shape.

Execute the rollback-sized tranches in the order specified by the clean
architecture plan:

1. Freeze the reset plan as a documentation-only milestone.
2. Delete `rustred-legacy-oracles` wholesale, then delete its core bridge,
   feature, dependency edges, tests, examples, and documentation promises.
   This happens before moving the core so no legacy path is repaired merely to
   be deleted.
3. Generate a tracked-path liveness ledger and write compact fresh sentinels
   for the retained capability spine before deleting its old test surfaces.
   Re-enumerate every tracked Rust file after each cleanup milestone and
   reconcile it against the ledger; a previous `split` or `retain` judgment is
   never permanent evidence.
   Then delete the entire root `tests/`, `examples/`, `tools/`, and `scripts/`
   trees, plus stale tracked LiteRed2/GammaLoop gitlinks. Do not port or repair
   the old binaries. Retain only the pinned Symbolica gitlink; reference trees
   remain ignored and untracked.
4. Delete `src/solver/**` and the dependent exact-session, closure,
   publication, re-entry, generated-affine/cylindrical/residual, and provider
   orchestration island. Retain only independently live generic
   algebra/family/IBP/sector/campaign/reduction primitives with callers outside
   that island. Then form and prune those values under acyclic `algebra`,
   `family`, `input`, `identity`, `sector`, `campaign`, `tensor`, and
   `reduction` owners while the package is still at the root. Prefixes such
   as `generated_`, `residual_`, and `parametric_` do not define directories.
   Every remaining long chronology/state-named file is challenged explicitly
   on each enumeration and must resolve to a small role-named owner or deletion
   before the new facade is accepted. When several live responsibilities share
   a long semantic prefix, express that prefix once as a cohesive parent module
   and use short role names below it; do not keep a flat family of repeated
   prefixes or create a miscellaneous prefix bucket.
5. The former 750-line public facade has already been reduced to a small
   intentional surface. Complete its liveness audit and narrow it further as
   modules reach their final owners, retaining only the generic API required by
   the spine, app/CLI/Python, and upcoming Vakint boundary.
   Delete unreferenced generations/provider layers, eager sector machinery,
   and compatibility APIs rather than suppressing dead-code warnings. Delete
   a CAS duplicate only after it is dead or a Symbolica API/differential audit
   has transferred authority; leave named live migrations for Phase 1.
6. Convert the root to a virtual workspace only after pruning. Move the now
   structured Cargo package named `rustred` to `crates/rustred-core`, update
   the app dependency to `../rustred-core`, retain the root maturin
   `pyproject.toml`, and leave no root `src/`, `tests/`, `examples/`, `tools/`,
   or `scripts/` tree. Use a registry-shaped Symbolica dependency patched by
   each workspace root so RustRed and GammaLoop can resolve one exact package.
7. Complete the small fresh generic contract suite instead of porting all 103
   root integration binaries. Preserve exact generic family/IBP/LI,
   Symbolica coefficient/row, zero/symmetry, deterministic campaign,
   reduction-primitive, and app/CLI/Python evidence. Phase 0 deliberately has
   no transaction/rollback or exceptional-child sentinel requirement. Use
   only actually embedded Vakint expectations immediately; live backend
   differentials wait for their real external tools and are not mislabeled as
   frozen goldens.
8. Replace the dated research corpus with concise stable architecture,
   algebra, foundry, interface, validation, and LiteRed-reference documents;
   rewrite the README for actual capabilities and delete milestone logs and
   superseded plans.
9. Require a zero-warning licensed workspace check, strict rustdoc, formatting,
   exact focused tests, tree/dependency audits, and independent subagent audits
   before declaring Phase 0 complete. Commit and push every coherent passing
   tranche; do not squash the reset into one opaque milestone.

The currently supplied `FOR_REFERENCE_ONLY_DO_NOT_PUSH/form5` path is a
React/Node project, not a runnable FORM5 installation. Vakint's actually
embedded expectations remain immediately extractable; live FORM-backed
differential regeneration is an opt-in external oracle gate requiring a real
FORM >= 4.2.1 executable. This does not block Phase 0 and never permits FORM in
RustRed or Vakint's RustRed mode.

Phase 0 is complete only when the root is virtual, package/module ownership
and dependency direction are explicit, no legacy package or authored
recurrence remains, root historical tests/docs are gone, production/test code
cannot cross boundaries accidentally, outstanding Symbolica migrations have
named boundaries, compiler and rustdoc warnings are zero, and the stable
README/design set describes the actual tree. The deleted prototype island has
no production replacement at this gate: Phase 0 ends without a foundry,
closed-artifact publisher, or solver-closure claim.

### Phase 1 — fresh Symbolica-native foundry

Build the production foundry anew on the retained generic spine. Git history
and research notes may supply hypotheses and failure cases, but no deleted
session, transaction, publication, or re-entry type is resumed by default:

1. Remove every production-reachable handwritten determinant, matrix product,
   polynomial kernel, exact/parametric Gaussian engine, and finite-field
   implementation for which Symbolica has a public operation. Preserve only
   RustRed's domain ordering, guards, provenance, resource policy, and exact
   verification responsibilities.
2. Migrate the tensor/application algebra needed by the Phase 4 lower-loop
   Vakint validation track when that track starts. Defer only the remaining
   six-loop/high-throughput application algebra until its measured production
   gate; it does not block the first derivation-only foundry slice.
3. Define a small private exact-solving boundary over freshly generated
   topology-neutral rows and Symbolica's retained sparse/matrix operations.
   Its input/output types are derived from the required guarded-rule contract,
   not copied from the deleted exact-session API.
4. Derive one strictly descending guarded lower-loop rule, with source-row
   provenance and exact regenerated-source residual replay, through the real
   app/core path.
5. Introduce exceptional equality/nonzero refinement only after the generic
   rule path is real. Regenerate source rows under the refined affine map and
   discharge one natural constrained lower-loop branch without importing
   parent reducer state or reviving deleted publication/re-entry wrappers.
6. Add deterministic resource-boundary, stale/foreign-input, and exact replay
   tests around the new contracts. State management is added only when a
   demonstrated multi-step need defines its semantics; no transaction or
   rollback abstraction is presumed from history.

An empty diagnostic, unsupported quotient, successful map, or reconstructed
prototype event is not a closed branch.

### Phase 2 — generic sector and family fixed-point closure

1. Replace eager family-wide sector enumeration with a lazy, target-driven
   dependency DAG so no high-loop path materializes `2^K` sectors or orthants.
2. Add graph-automorphism/routing-equivalence candidate ingress to the generic
   exact symmetry verifier by calling Symbolica's intrinsic public graph engine
   directly. Construct a physics-colored `symbolica::graph::Graph`; call
   `Graph::canonize()` for an existing topology and consume the returned
   `CanonicalForm` generators/canonical labeling for automorphisms and
   isomorphism maps. Encode lines, line ports, and topology vertices as
   differently colored vertices in a subdivision/flag graph so parallel-line
   exchange and orientation reversal are explicit vertex permutations. Use
   `GenerationSettings` with `Graph::generate` only for optional finite
   topology-domain enumeration, never as the symmetry search for an already
   supplied graph. RustRed interprets the permutations as routing candidates,
   solves their exact momentum maps through Symbolica matrix APIs, and retains
   the generic affine/momentum-map replay verifier as the certification gate.
   GammaLoop's local `feyngen` implementation is reference-only usage evidence
   for coloring, bucketing, and orchestration—not a dependency, graph
   authority, or substitute for Symbolica's native capabilities. Bounded
   `GL(L,Z)` matrix enumeration is deleted, not retained as an oracle.
3. Define a narrow immutable boundary from the newly built exact solver to
   applicable rules and exceptional residual domains, with exact chronological
   replay against regenerated source rows.
4. Add recursive exceptional scheduling, authenticated ancestry, explicit
   identically-bad continuation semantics, and immutable solved-subsector
   feedback without assuming the deleted database/session design.
5. Add proof-bearing zero, symmetry, factorization, proper-subsector, and
   cross-family providers to the lazy dependency DAG.
6. Iterate until every reachable domain is discharged under the strict
   closure definition above.
7. Persist resumable workspaces separately from immutable, revision-bound,
   checksummed, independently verifiable `Closed` shards and multi-root
   bundles.

### Phase 3 — deterministic parallel rule foundry

1. Use one deterministic coordinator and one invocation-wide bounded local
   pool. `--n-cores` is a total ceiling, not a promise to occupy every core.
2. One case lane owns one checked coefficient field and one retained
   `SparseRowReducer`; ordered forward mutation within that lane remains
   serial.
3. Parallelize independent families, sectors, frozen exceptional-case
   proposals, modular sample ordinals, immutable source-row preparation, and
   exact verification blocks.
4. Merge only at stable sorted wave/barrier boundaries. Root order, worker
   arrival order, and `n_cores = 1,2,4` must preserve semantic artifacts.
5. Admit width from RAM first, accounting for coordinator/worker Symbolica TLS,
   committed + trial + successor reducer overlap, GMP/native scratch, result
   staging, and opaque headroom. Per-lane algebra limits are mathematical,
   resumable outcomes; global memory admission only schedules or delays work.
   The coordinator acquires cores and memory atomically before constructing
   owners or clones and charges unique live allocations, including
   predecessor/successor overlap. No nested pools or per-job process forks.
6. Use deterministic finite-field discovery and reconstruction only as a
   proposal accelerator; freeze sample schedules and verify every result
   exactly over the authenticated Symbolica coefficient domain.
7. Treat the final worker architecture as a measured six-loop design problem,
   not as a consequence of the Phase-0 ordinal-batch API. Share immutable
   family/source data instead of cloning the complete symbolic state per
   worker; isolate only bounded lane-local mutable reducers and Symbolica
   contexts. Use coarse enough admitted work units to amortize scheduling,
   transfer compact references or framed chunks rather than full snapshots,
   and stream bounded results/artifacts through deterministic coordinator
   barriers. Thread, process, and hybrid choices require RAM, CAS-scratch, and
   communication benchmarks; per-task forks and unbounded worker I/O queues
   are forbidden.

### Phase 4 — evidence ladder through four-loop oracles

1. Produce genuinely coverage-closed one-, two-, and three-loop generic
   shards, including tensor/scalar input closure and cancellation metamorphic
   tests.
2. Translate pinned LiteRed/LiteRed2 examples into data fixtures and progress
   them from input/identity parity through sector/symmetry, guarded-rule, and
   target-reduction parity without topology-specific production logic.
3. As soon as Phase 0 closes, create the dedicated GammaLoop feature branch
   and begin Vakint's generic tensor-reduction backend boundary and FORM-less
   `RustRed` tensor variant in parallel with Phases 3 and 5. Extend that lane
   to guarded rule application when the first closed lower-loop shards exist.
   Split this work across independent subagent lanes for source/oracle audit,
   topology matching, RustRed adapter and rule application, tensor/master
   handling, performance, and adversarial review.
4. Reuse Vakint's existing topology matching and canonicalization engine as the
   steering authority. Fix defects found by the existing end-to-end corpus,
   add exact convention/routing witnesses at the RustRed boundary, and extend
   candidate/registry support generically through six loops. An exhaustive
   acceptance gate must prove that every entry in the frozen one- through
   six-loop vacuum manifest is accepted, canonicalized, and routed by the
   reused matcher. Defects are fixed in that matcher rather than bypassed or
   duplicated inside RustRed; topology-name dispatch is forbidden.
5. Begin the Vakint track by introducing a generic, backward-compatible tensor
   reduction backend boundary. The existing FORM implementation remains the
   default/compatibility backend and an authoritative lower-loop oracle, while
   a new `RustRed` backend variant calls the tensor-reduction service actually
   implemented in the RustRed crate. Vakint owns only matching, request/result
   adaptation, steering, and presentation. From its first milestone, the
   RustRed service has an `Auto` dispatch and explicit optimized-vacuum and
   generic lanes: implement and optimize the single-scale vacuum/no-external-
   denominator-shift lane covered by Vakint's current FORM path, and install a
   tested typed stub for the fully generic lane without pretending support.
   Fast-lane admission uses a sealed RustRed proof derived from authenticated
   physical-denominator roles, shifts, and common-scale metadata—not a Vakint
   boolean, family label, or topology name—and permits numerator-only external
   spectator vectors.
   Study the existing FORM algorithm for contraction/projector efficiency and
   use it as segregated reference/oracle evidence, while re-expressing the
   algorithm in clean Rust and Symbolica public APIs and never invoking FORM
   from the new backend. Exercise identical existing Vakint tensor inputs
   through the FORM oracle and RustRed variant and compare canonical
   tensor/scalar outputs before adding IBP application. Then implement a
   bounded but real
   RustRed-core/Vakint path for lower-loop scalar lowering/cancellation,
   guarded IBP application, stable master keys, and supplied master
   substitution. The new path itself remains pure Rust + Symbolica and uses
   independently generated RustRed artifacts. Vakint's existing matcher
   supplies the canonical topology, routing, and loop bindings; RustRed does
   not rematch it. Rebuild the current
   oversized tensor prototypes as `tensor::{model,atom,lowering}` and
   `tensor::projector::{pairing,contraction,orbit,vacuum}`. Preserve the small
   mathematical kernels, use Symbolica-native polynomial/matrix operations,
   keep arbitrary numerator weights as opaque `Atom`s separate from exact IBP
   coefficients, and use orbit-quotient projector solves rather than dense
   pairing-space inversion. The untested legacy tensor SCC is deleted rather
   than moved. Its fresh replacement must specify and test deterministic
   pairings/contraction cycles with Gram entries `d^cycles`, covariant metric
   precontraction, generic affine scalar-product lowering, and caller-supplied
   Symbolica heads. A pre-sentinel with custom non-Vakint heads lowers
   `dot(k,p)*k(mu)` for `D=k^2+m^2` to the complete covariant-keyed result
   `p(mu)/d * [I(0)-m^2 I(1)]` with the `d != 0` guard, and checks deterministic
   rank-four pairing/Gram behavior. The first vertical Vakint sentinel reuses
   Vakint's checked one-loop `(k_mu k_nu + k.p)` analytic test and independently
   checks projected tensor form, raw master coefficient, and final Laurent
   series.
6. When a real FORM >= 4.2.1 executable becomes available, run Vakint's
   existing one- through four-loop end-to-end tests through the pinned
   alphaLoop, MATAD, and/or FMFT backends only in the segregated oracle job.
   Until then, consume only expectations actually embedded in the existing
   tests; live differential-only suites are unavailable. Compare final
   expressions over unsubstituted masters—and master-substituted results where
   authoritative data exists—after an explicit convention map. Never copy the
   oracle's authored recurrence tables into RustRed rules.
7. Freeze a separate versioned four-loop raw oracle corpus where required;
   oracle absence remains explicit and cannot be filled with RustRed's own
   output.
8. Prove restartability, multi-root deduplication, routing/permutation
   equivalence, factorization, dependency invalidation, and deterministic
   bundle verification.

Phases 3–5 are intentionally parallel after their dependency gates. Vakint
validation must not divert the foundry from six-loop efficiency and closure;
conversely, the lower-loop RustRed mode must not wait for the entire six-loop
library when it can already provide end-to-end oracle evidence for newly
closed lower-loop artifacts.

### Phase 5 — complete vacuum artifact library through six loops

1. Close representative generic five-loop, ISP-rich, lower-symmetry,
   duplicate/dependent-denominator, and factorization cases without adding a
   loop-specific decision path.
2. Define and freeze an exhaustive canonical single-scale vacuum-topology
   manifest through six loops under the declared graph normalization, routing,
   fusion, connectivity, and family-equivalence contract. Retain a replayable
   enumeration/completeness witness; a hand-picked topology list is not an
   “all topologies” claim.
3. Freeze the physical six-loop benchmark subset before execution. Prefer
   actual GammaLoop/BPHZ roots when available; otherwise include QCD-valid
   connected 1PI quartic and cubic representatives with non-factorizing
   reachable sectors.
4. For every six-loop manifest root, construct the full 21-coordinate
   unit-mass family, process all 36 ordinary sources, verify
   graph/routing-derived symmetries, traverse shared dependencies, close every
   exceptional route, and emit a deterministic multi-start-ready shard DAG.
   Apply the corresponding generic construction and closure contract to every
   lower-loop manifest entry.
5. Require zero reachable unsupported/uncovered/resource/timeout/interrupted
   leaves and exact regenerated-IBP residuals for every rule.
6. Assemble the closed shards, verified ingress/routing maps, terminal/master
   keys, schema/ABI metadata, and checksums into a versioned distributable rule
   library designed to be saved and shipped with Vakint. Ordinary users load
   this library by authenticated family fingerprint; they do not rerun the
   foundry.
7. Record named hardware, release/GMP configuration, wall and CPU time, peak
   RSS, rules/events/targets/loci/cases, queue peak, coefficient growth,
   dependency/deduplication counts, artifact bytes, and 1/2/4-worker scaling.
   Freeze the resource envelope before the run; do not revise it post hoc to
   convert an incomplete campaign into a pass.
   The provisional physical benchmark gates are at most 48 GiB peak RSS and
   24 hours per root, at most 48 hours for the initial three-root bundle, and
   at least 2.5x four-worker speedup when at least four independent jobs are
   ready. Any critical-path exception must be declared before execution. The
   exhaustive manifest additionally requires a separately predeclared
   aggregate storage/time/RSS budget.

The first credible six-loop milestone is one physical `L=6`, `K=21`,
unit-mass root producing a replayable closed sector shard through the direct
foundry. The first-goal completion gate is stronger: every entry in the frozen
canonical one- through six-loop single-scale vacuum manifest maps to a
completely verified closed bundle, the physical QCD/GammaLoop benchmark subset
passes within its declared resource envelope, and the resulting versioned
artifact library is ready to ship with Vakint.

### Phase 6 — complete production Vakint/artifact integration after Phase 5

The lower-loop Vakint `RustRed` mode begins in Phase 4. Only after the offline
one- through six-loop vacuum-artifact gate may it be promoted to the complete
production evaluation chain and six-loop shipped-library deployment:

- a compiled high-volume guarded-rule application engine in the RustRed crate;
- native tensor reduction, scalar lowering/cancellation, IBP application, and
  typed master substitution services in the RustRed crate;
- Vakint's FORM-less `RustRed` mode as the user-facing steering API over those
  services and the shipped one- through six-loop vacuum artifact library;
- continued first-class CLI/Python access to the same individual RustRed
  services for generic vacuum and non-vacuum families, including on-demand
  parametric IBP generation and closure outside the shipped library;
- a typed GammaLoop boundary replacing Vakint's current
  canonicalize/tensor/evaluate middle while retaining GammaLoop's BPHZ forest
  and postprocessing;
- stable master keys plus validated AMFlow-derived numerical/Laurent master
  data, with Vakint steering and RustRed implementing substitution; and
- the complete six-loop QCD beta-function computation.

All Vakint changes from Phase 4 onward are made in the GammaLoop repository on
the dedicated `vakint_rustred` branch, with rollback-sized commits and
milestone pushes to that branch. The branch is created when this workstream
begins. In the present ignored checkout, `crates/vakint/Cargo.toml` may use
`rustred = { package = "rustred", path = "../../../../crates/rustred-core" }`
for immediate cross-repository feedback. Before each
reproducible pushed milestone, CI run, frozen oracle comparison, or published
artifact claim, it points to the RustRed GitHub repository at the exact
validated RustRed commit used by that milestone. Machine-specific absolute
paths are never committed. RustRed changes remain in the RustRed repository.
Before every commit, both repositories' scopes are checked independently so no
path under `FOR_REFERENCE_ONLY_DO_NOT_PUSH/` enters RustRed history.

The combined Cargo graph must resolve one compatible pinned Symbolica package
and feature set, with GMP enabled and `no_gmp` absent. `rustred-core` declares
the exact registry-shaped Symbolica version; the RustRed workspace patches it
to the pinned vendor checkout, while GammaLoop's workspace patches the same
package names to one jointly validated exact Git revision. GammaLoop's Hakari
workspace-hack must also be regenerated so its direct normal/build Symbolica
and Numerica dependencies use that same revision; a root crates.io patch does
not override direct Git dependencies. Moving `dev` branch selectors anywhere
in tracked manifests are forbidden for a milestone. Manifest/lock scans and
source-qualified `cargo tree -d`/`cargo tree` audits reject duplicated or
revision-incompatible Symbolica-family packages. Until the graph is unified,
the cross-repository API uses owned RustRed domain values and Vakint-owned
conversion rather than exposing `Atom`/`AtomView`. Producer version metadata
comes from Symbolica's public `LicenseManager::get_version()`, not from
scraping the vendored manifest.

None of the Vakint/RustRed implementation layers may invoke FORM, Mathematica,
SymPy, or copied authored recurrences. Only the segregated existing-backend
oracle job described above may execute a real pinned FORM >= 4.2.1.

## Current evidence baseline

As of the assigned baseline, the following retained capabilities are real but
partial:

- topology-neutral family lowering and raw parametric IBP/LI generation;
- the synthetic `L=6`, `K=21`, 36-source generation/stress fixture;
- automatic ISP completion and several zero/symmetry foundations;
- exact Symbolica coefficient/row primitives available to a future clean
  solving boundary;
- static multi-root planning and preliminary memory-width admission; and
- the transport-neutral `rustred-app`/CLI boundary.

The deleted private prototype island had local tests for recentering,
condition partition, exact sessions, exceptional publication/re-entry, and
narrow one- through three-loop reductions. The completed liveness audit found
no production application/CLI/Python/Vakint caller for that stack. Those
results remain historical design and regression evidence in Git, not retained
capabilities, a production foundry, or closed reusable family artifacts.

The following limitations remain explicit at this baseline:

- the public application path does not derive or publish closed rules;
- the synthetic six-loop fixture is not a physical six-loop family or a
  reduction result;
- no physical six-loop source has reached `Ready`, published a guarded rule,
  or closed a sector;
- after the reset there is no production foundry until Phase 1 builds one
  cleanly;
- the deleted tensor prototype SCC is not a retained tensor-reduction
  capability; the tensor boundary is rebuilt from fresh contracts and
  sentinels;
- no complete lazy sector-domain traversal or canonical physical vacuum
  topology manifest suitable for the full `K=21` domain exists yet;
- no live sparse foundry/elimination backend or guarded-rule publisher exists;
- direct `symbolica::graph` symmetry ingress is not yet implemented;
- campaign application is currently roots-only/preflight-oriented; and
- there is no complete independently derived Vakint one- through four-loop
  rule corpus, physical five-/six-loop closure result, persistent closed
  bundle, publishable portable Python distribution, or Vakint `RustRed` mode.

Capability reports, commits, and documentation must remain tied to these
evidence levels. A generated source row, a legacy recurrence, a first
residual, a component typestate transition, or a synthetic stress test must
never be described as six-loop closure.

## Acceptance and verification discipline

Every rollback-sized implementation tranche must include, in proportion to
risk:

- focused component and boundary tests;
- licensed default-GMP parallel tests;
- exact and one-below memory/work limits;
- deterministic `n_cores = 1,2,4` comparisons where concurrency is involved;
- algebraically equivalent input spellings;
- native-Symbolica versus quarantined-old differential tests before deleting
  an algebra path;
- exact regenerated-source replay and deliberate certificate tampering;
- `cargo check`, formatting, and diff hygiene; and
- an independent subagent audit for architectural, CAS-authority, closure, or
  capability-claim changes.

Benchmarks and acceptance fixtures are data, never production dispatch.
Ordinary/default tests must not initialize FORM or Mathematica. The separately
declared Vakint reference-oracle job may execute a pinned real FORM >= 4.2.1
only to run the existing alphaLoop/MATAD/FMFT comparison paths; the new mode
remains FORM-less. The supplied `form5` directory is not such an executable.
Reference-only trees remain untracked. Milestones are committed and pushed
only after their declared gate passes; partial work is reported honestly and
is not relabeled as closure.
Intermediate commits are frequent enough to remain bisectable and
rollback-sized; passing work is pushed rather than left as a long-lived local
stack.

## Governing reading set and authority

This file is the sole current goal and sequencing authority. It was initially
reconciled from the complete historical `HANDOFF.md` prescribed reading set,
the live code, the full research-Markdown inventory, and independent
research/code/reference audits. That handoff was consumed and then removed as
stale legacy documentation; Git history retains it as evidence only. The
principal supporting sources are:

- `README.md` and the live implementation for the actual development frontier;
- `docs/research/litered_full_scope_spec.md` for the durable mathematical
  capability and acceptance scope;
- `docs/research/repository_clean_architecture_plan_2026-08-28.md` for the
  authoritative repository reset, domain DAG, deletion order, and Vakint
  dependency/oracle boundary; older reorganization and Python directives were
  superseded and are retained only in Git history;
- `docs/research/litered_solvej_residual_recentering_2026-08-13.md` for the
  retained residual-case, affine-locus, and `WhenBad` source semantics;
- `docs/research/litered2_algorithm_report.md` and
  `docs/research/litered_examples_acceptance_matrix.md` for LiteRed semantics
  and oracle progression;
- `docs/research/symbolica_rust_api_for_litered.md`,
  `docs/research/symbolica_exact_linear_algebra_api_inventory.md`,
  `docs/research/symbolica_upstream_gap_audit_2026-08-25.md`, and
  `docs/research/symbolica_only_algebra_compliance_roadmap_2026-08-27.md` for
  the CAS boundary; and
- `docs/research/vakint_alphaloop_tensor_ibp_audit.md` and
  `docs/research/gammaloop_six_loop_boundary_audit_2026-08-24.md` for the
  parallel lower-loop oracle/application boundary and complete deployment.

Historical notes remain evidence only when they conflict with a newer
governing document or the live implementation. In particular, loop-authored
recurrence notes, eager MTBDD/sector strategies, handwritten algebra plans,
and obsolete “not implemented” checkpoints cannot override this goal.
