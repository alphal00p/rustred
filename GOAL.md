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

## Authority and staged assignment

The preamble above records the long-term scientific motivation verbatim. The
latest approved plan in this section supersedes its sequencing and defines the
only active assignment for the primary Codex agent (`/root`). The primary
agent acts mainly as architect, orchestrator, integrator, and final verifier;
independent research, implementation slices, and adversarial audits are
delegated whenever useful.

Development is divided into two hard-gated stages:

- **Stage 1 is active:** close and publish single-scale vacuum parametric-IBP
  artifacts through three loops, implement their FORM-free scalar application
  in Vakint, and reproduce Vakint's end-to-end expectations through three
  loops. Vakint continues to use its existing FORM tensor reduction before the
  new scalar backend when tensor numerators are present.
- **Stage 2 is deferred and must not start without new user guidance:** do not
  enhance tensor reduction, integrate speculative collaborator tensor work,
  pursue four- through six-loop closure, or optimize the foundry for a
  six-loop breakthrough. Those remain long-term goals, not current tasks.

The existing experimental RustRed tensor service and GammaLoop
`TensorReductionMode::RustRed` adapter are frozen. They may remain in their
repositories, but Stage 1 must not extend, redesign, or make them part of the
active acceptance path. Vakint's established `TensorReductionMode::Form`
remains its tensor default and the Stage 1 tensor prepass.

## Current evidence boundary

The clean workspace refactor is complete. The root is a virtual Cargo
workspace containing package/library `rustred`, `rustred-app`, and
`rustred-python`; obsolete prototype solvers, authored recurrences, and legacy
compatibility layers have been removed.

The live core compiles topology-neutral families, generates generic ordinary
IBP and LI source rows, authenticates physical and auxiliary family
presentations, verifies supplied affine symmetries, analyzes requested zero
sectors, and provides deterministic campaign primitives. Its foundry can
derive concrete-anchor and guarded fixed-sector recurrences over Symbolica
`K(n)`, select a requested pivot with deterministic Symbolica RREF, prove
uniform descent, exactly replay source combinations, partition target-sector
domains, and stream proper-subsector obligations.

The core now freshly generates and seals the canonical unit-mass one-loop
partition and equal-mass two-loop sunset as mathematical and durable closing
artifacts. Their tagged binary representations authenticate complete source,
rule-cell, projection, symmetry, factorization, terminal, and homogeneity
semantics once at the untrusted load boundary. The sunset owner derives all
four ordinary sources, closes its generic and exceptional cells, routes exact
`S3` symmetries, and feeds its pinched face into the immutable one-loop
dependency. A topology-independent deterministic memoizing reducer applies
both sealed owners without repeating whole-artifact authentication. The Rust
application API, `campaign` CLI, and public `import rustred` Python package
generate, inspect, load, and apply those actual artifact bytes with typed
errors and deterministic output.

RustRed still does **not** close or publish the three-loop `K = 6` family or
substitute evaluated masters. Vakint does not yet consume either standalone
artifact: its opt-in RustRed scalar method remains a deliberately unavailable
API seam, with no end-to-end RustRed-backed one-loop result claimed yet.

The bounded scalar/odd/rank-two RustRed vacuum projector and its optional
Vakint adapter are existing experimental capability. They do not establish
rank-generic tensor reduction and are frozen for Stage 1.

## Stage 1 objective

Build a production-grade, topology- and loop-count-independent offline rule
foundry, written entirely in Rust and using GMP-enabled Symbolica as its sole
CAS. For the active pressure domain it must derive guarded, strictly
descending, exactly replay-certified, coverage-closed replacement systems and
persist them as deterministic reusable artifacts.

Stage 1 freezes a matcher-derived manifest with all eight Vakint graph classes
through three loops:

| Family artifact | Coordinates `K` | Ordinary sources | Required Vakint coverage |
|---|---:|---:|---|
| one-loop tadpole | 1 | 1 | the one-loop class |
| two-loop sunset | 3 | 4 | the sunset and its pinch |
| three-loop K4/Mercedes | 6 | 9 | the parent and four inequivalent contractions |

One sector-complete unit-mass artifact is produced for each row of this table.
The `K = 6` artifact must prove coverage of all five registered three-loop
classes; topology names are manifest labels and fixtures, never algorithmic
dispatch keys. Pinches, symmetries, factorization, and routing are handled
through generic proved transformations.

All production algorithms remain generic in topology and loop count, including
the Rust library, `campaign` CLI, and public Python package. Python users write
`import rustred`; `rustred._rustred` is private and top-level `import _rustred`
is unsupported. Generic non-vacuum family construction and identity generation
remain first-class even though Stage 1 closure pressure is the vacuum manifest.

### Unit-scale contract

The closing search and shipped tables use a proved common squared mass
`s = m^2 = 1`. For `L` loops and powers `a`,

```text
I(a; s) = s^(L*d/2 - sum(a)) I(a; 1).
```

Consequently the coefficient restoring a unit-scale reduction from target
`a` to master `b` is

```text
c[a -> b](s) = s^(sum(b) - sum(a)) c[a -> b](1).
```

This includes negative auxiliary powers. Specialization requires authenticated
single-scale homogeneity and nonzero scale evidence; dimensional analysis does
not excuse a convention or routing mismatch.

## Definition of closure

RustRed distinguishes mathematical in-process closure from durable production
publication. A root may become the immutable in-process `ClosedArtifact` only
when it establishes items 1–6 below. It may be called a published or production
artifact only after it also establishes item 7:

1. The exact family, coefficient/index contexts, kinematics, metric and
   propagator conventions, routing, cuts, power shifts, ordering, and freshly
   generated source set are bound to one canonical identity.
2. Every required ordinary IBP and LI identity is generated generically. For
   `L` loops and `E` external momenta,
   `K = L(L+1)/2 + LE`, with `L(L+E)` ordinary sources and `E(E-1)/2` LI
   sources.
3. Every rule carries exact integer-domain and nonzero-polynomial guards,
   strict well-founded descent, source provenance, and a zero residual against
   freshly regenerated identities.
4. Zero, symmetry, cross-family maps, factorization, product structure, and
   proper-subsector dependencies are proof-bearing.
5. Every generic and exceptional branch reaches a descending rule, an already
   closed dependency, an explicitly enumerated master, or an independently
   certified zero/product/factorized terminal. A residual or failed search is
   never a terminal.
6. Solved dependencies feed back immutably until the reachable dependency
   graph reaches a deterministic fixed point with no uncovered, unsupported,
   resource-limited, interrupted, or unresolved leaf.
7. The deterministic durable representation is bounded and validated once at
   the untrusted load boundary before conversion to the sealed owner; the
   reduction hot path does not repeat whole-artifact authentication.

Finite-field or numerical samples may propose candidates, but exact
regenerated-source replay is mandatory.

## Stage 1 implementation tracks

### Closing foundry and artifacts

1. Refine exact coefficient and guard applicability on target cells, including
   exceptional equality and nonzero branches.
2. Add translate-before-substitute residual search, immutable lower-sector
   feedback, and a deterministic fixed point with proof-bearing symmetry,
   zero, factorization, product, mapping, and terminal providers.
3. Introduce versioned immutable artifact ownership only with its first closed
   family. Keep incomplete resumable workspaces structurally distinct from
   installable artifacts.
4. Close, replay, publish, and independently audit the `K = 1`, `K = 3`, and
   `K = 6` families in that order.

### Rule application and public APIs

Add a deterministic, memoized, strictly descending artifact applier. It
selects applicable guarded rules, detects malformed artifacts or cycles at the
trusted boundary, returns exact coefficients of typed master keys, and restores
the common scale by homogeneity. It never regenerates the artifact during an
ordinary reduction.

Expose closing-artifact generation, inspection/replay, and reduction through
the Rust library, `campaign` CLI, and `import rustred` Python API. All three
frontends call the same application services and produce the same deterministic
semantics.

### Vakint scalar backend

On GammaLoop branch `vakint_rustred`, add the opt-in scalar evaluation backend
`EvaluationMethod::RustRed(RustRedEvaluationOptions)` and
`EvaluationOrder::rustred_only()` without changing existing defaults or
behavior. The adapter:

- consumes Vakint's existing topology match and simultaneous routing witness;
- never rematches a graph, dispatches on a topology name, or duplicates the
  topology registry;
- applies shipped immutable RustRed artifacts and never regenerates them at
  evaluation time;
- returns exact coefficients in Vakint's existing MATAD master basis and reuses
  its pure-Rust master substitution/evaluation data;
- exposes master substitution control, enabled by default; and
- reports no FORM dependency and never invokes or falls back to FORM for
  scalar IBP reduction or master substitution.

Production artifacts are generated once, checked into and shipped with Vakint,
and loaded once. A local path dependency may be used while co-developing; every
pushed GammaLoop milestone pins the exact validated RustRed Git revision.

“FORM-free RustRed backend” refers precisely to the scalar IBP application and
master-substitution tail. For tensor-bearing inputs, Stage 1 intentionally runs
Vakint's unchanged FORM tensor prepass first. Such a complete tensor-bearing
evaluation is therefore not claimed to be FORM-free. Scalar or already
tensor-reduced inputs can exercise the RustRed backend with an invalid FORM
path to prove that the backend itself has no hidden FORM dependency.

## Stage 1 milestones and acceptance

1. Commit and push the authoritative staged goal and documentation.
2. Close and publish the one-loop artifact with the artifact/reducer spine.
3. Close the two-loop family, cover its pinch, and expose Rust, CLI, and Python
   artifact/application APIs.
4. In parallel with three-loop closure, integrate and validate Vakint RustRed
   scalar reduction through two loops.
5. Close the `K = 6` family and prove coverage of all five registered
   three-loop graph classes.
6. Pass all applicable single-scale Vakint acceptance tests through three
   loops, update documentation, commit and push both repositories, then pause.

Acceptance requires:

- exact regenerated-source replay, strict descent, explicit terminals, and no
  uncovered branch in each installed artifact;
- deterministic artifacts and reductions across supported worker counts;
- guard selection, termination, memoization, symmetry routing, master-only
  output, and non-unit-mass restoration tests;
- exact raw master-coefficient comparison with AlphaLoop/MATAD oracle outputs
  and matching existing Laurent-series expectations;
- scalar RustRed-backend tests with an invalid FORM path;
- tensor-bearing tests using the unchanged FORM tensor prepass followed by the
  FORM-free RustRed scalar tail; and
- unchanged Vakint defaults and backward-compatibility tests.

PySecDec comparisons are optional, non-gating corroboration.

## Stage 2 — deferred, not authorized

Stage 2 preserves the long-term ambition from the historical preamble, but no
Stage 2 implementation or performance campaign may begin until the user
provides the collaborator's tensor-reduction direction, the high-loop IBP
breakthrough, and explicit permission. It includes:

- integrating or replacing tensor reduction and making it generic in rank;
- changing Vakint's tensor preprocessing away from FORM;
- closing four-, five-, and six-loop vacuum manifests;
- high-loop-specific distributed-memory, reconstruction, and extreme
  efficiency work; and
- the eventual six-loop QCD beta-function evaluation chain.

Stage 1 code must not preclude Stage 2, but speculative infrastructure is not a
Stage 1 deliverable.

## Engineering and repository invariants

- Production RustRed and the Vakint RustRed scalar backend use only Rust plus
  GMP-enabled Symbolica. They never execute FORM, Mathematica, SymPy, or
  another CAS. The explicitly retained Vakint FORM tensor prepass is an
  external legacy stage, not a RustRed algebra provider.
- Search Symbolica's public API, Rustdoc, source, examples, and tests before
  implementing any algebraic primitive. RustRed owns physics meaning,
  authentication, guards, ordering, provenance, resource admission, and exact
  replay; it does not grow a second CAS or graph-isomorphism engine.
- Symbolica's intrinsic graph generation/canonization/isomorphism facilities
  are the symmetry-candidate authority. RustRed owns physics-colored encoding
  and exact momentum/routing replay.
- Use semantic module ownership; do not revive chronological, `generated`,
  `residual`, `runtime`, `legacy`, or `misc` buckets. RustRed has no pre-release
  compatibility promise; Vakint retains backward compatibility.
- Validate untrusted inputs and durable artifacts at their boundary. Do not
  accumulate repeated internal authentication ceremonies in the hot path.
- Deterministic parallel work uses one bounded coordinator/pool, shared
  immutable state, RAM-aware admission, stable ordinals, and sorted merges.
  Stage 1 implements only the parallelism justified by three-loop workloads;
  high-loop scaling research belongs to Stage 2.
- `FOR_REFERENCE_ONLY_DO_NOT_PUSH` is ignored and never enters RustRed history.
  GammaLoop inside it is a separate repository and branch.
- Never escalate commands. Use rollback-sized commits, push passing milestones
  frequently, and configure every Git operation with:

  ```text
  user.name=ValentinHirschi
  user.email=valentin.hirschi@gmail.com
  ```

Do not claim Stage 1 complete until all three artifacts cover the frozen
through-three-loop manifest and the Vakint RustRed scalar backend reproduces
the applicable acceptance suite. At that point, pause; do not roll directly
into Stage 2.

## Stable project documentation

- [Architecture](docs/architecture.md)
- [Algebra and Symbolica boundary](docs/algebra.md)
- [Frozen tensor boundary and Vakint sequencing](docs/tensor.md)
- [Closing-rule foundry design](docs/foundry.md)
- [Application, Python, and Vakint interfaces](docs/interfaces.md)
- [Validation and oracle ladder](docs/validation.md)
- [LiteRed2 semantic reference](docs/references/litered2.md)
- [Current CLI contract](docs/CLI.md)
