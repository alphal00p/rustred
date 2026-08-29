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

## Assignment and current frontier

This is the authoritative active goal assigned to the primary Codex agent
(`/root`). The primary agent acts mainly as architect, orchestrator, integrator,
and final verifier. Independent research, implementation slices, performance
work, and adversarial audits are delegated to subagents whenever useful.

Later user directives supersede the preamble only on sequencing. The six-loop
closing-rule foundry remains the primary scientific objective, but the
orthogonal Vakint integration now proceeds in parallel. Tensor reduction can be
implemented and tested before closed IBP artifacts exist; guarded IBP
application begins when the first genuinely closed lower-loop artifact exists.

The mandatory clean reset is complete:

- the root is already a virtual Cargo workspace with no root `src`;
- the only first-party workspace packages are `rustred`, `rustred-app`, and
  `rustred-python`;
- the former solver/session/publication/re-entry and authored-recurrence
  prototype has been deleted rather than preserved;
- the core is organized under `algebra`, `family`, `input`, `identity`,
  `sector`, `tensor`, `foundry`, and `campaign`; the completed Track-A spine's
  source/facade/allocation audit passed, the new capability slices are audited
  at each milestone, and the dated research corpus has been consolidated into
  the stable documents linked below and deleted; and
- the live core currently compiles families, generates generic ordinary
  parametric IBP and LI rows, verifies affine symmetry candidates, performs
  on-demand zero-sector analysis, authenticates physical/auxiliary family
  presentations, performs bounded scalar/odd/rank-two vacuum tensor projection
  and affine scalar-product lowering, derives one guarded strictly descending
  rule at a concrete anchor with Symbolica's sparse reducer and exact source
  replay, and provides deterministic core-owned campaign primitives with
  application-owned roots-only composition. It does **not** yet generalize
  anchored rules, solve or close families, publish rule artifacts, apply IBPs,
  substitute masters, or support generic/higher-even-rank tensor reduction.

In the independent GammaLoop repository, branch `vakint_rustred` now has the
first real backward-compatible `TensorReductionMode::RustRed` adapter. It
reuses Vakint's topology matcher and simultaneous canonical routing, then
calls RustRed's key-aware projector for the bounded one-loop,
one-propagator, single-scale-vacuum slice through rank two. Existing Vakint
behavior remains unchanged and defaults to its FORM backend. Unsupported
families and ranks return typed errors; the RustRed mode never invokes or
falls back to FORM. This is tensor-projection capability, not yet generic
tensor reduction, scalar IBP application, or master substitution.

Commit and push coherent passing milestones frequently. Every Git operation
uses:

```text
user.name=ValentinHirschi
user.email=valentin.hirschi@gmail.com
```

## Assigned objective

Build RustRed into a production-grade, topology- and loop-count-independent
offline rule foundry, written entirely in Rust and using GMP-enabled Symbolica
as its sole CAS. It must derive guarded, strictly descending, exactly
replay-certified, coverage-closed parametric IBP/LI replacement systems and
persist them as deterministic reusable artifacts.

The first pressure target is practical closure of physical six-loop,
single-scale massive-vacuum families after a proved `m_uv^2 = 1`
specialization. A complete six-loop vacuum family has 21 scalar-product
coordinates and 36 ordinary parametric IBP source rows. Producing those rows is
only structural evidence: the goal is to discharge every reachable generic and
exceptional domain onto strictly lower closed dependencies or a finite,
explicitly selected terminal set.

All production algorithms remain generic in topology and loop count. Dedicated
fast lanes may be selected only from authenticated semantic properties such as
vacuum kinematics, common nonzero scale, proved unit-mass homogeneity,
coefficient field, graph structure, and routing witnesses. Dispatch on a
topology name, family label, authored table, or literal `loop_count == 6` is
forbidden.

The one-off foundry deliverable is a versioned and independently verifiable
library of closing parametric-IBP artifacts covering the declared canonical
single-scale vacuum topology domain through six loops. Completeness is defined
by a frozen GammaLoop/Vakint graph manifest after documented routing,
canonicalization, fusion, and factorization equivalences. The generated
artifacts are saved and shipped with Vakint; ordinary integral evaluation must
not rediscover them.

Vakint will be the user-facing steering layer for the full evaluation chain,
but the reusable mathematics belongs to the RustRed crate. Vakint owns topology
matching, routing, orchestration, configuration, and presentation. RustRed owns
tensor reduction, family-aware scalar lowering, compiled guarded-rule
application, stable master keys, and typed master substitution. Numerical
master data is expected from AMFlow. Through GammaLoop's BPHZ/R-operation
boundary, this is intended to contribute to the six-loop QCD beta-function
programme.

Vakint does not replace RustRed's own interfaces. The Rust CLI and Python API
remain first-class fine-grained surfaces for generic families, including
non-vacuum topologies. Python users write `import rustred`; the native
`rustred._rustred` module is private and top-level `import _rustred` is not a
supported interface.

## Definition of closure

A root is `Closed` only when one immutable artifact establishes all of the
following:

1. The exact family, coefficient/index contexts, kinematics, metric and
   propagator conventions, routing, cuts, power shifts, ordering, and source
   identity set are bound to one canonical identity.
2. Every required ordinary parametric IBP and separate LI identity is generated
   generically. For `L` loops and `E` external momenta,
   `K = L(L+1)/2 + LE`, the ordinary-row count is `L(L+E)`, and the LI count is
   `E(E-1)/2`.
3. Every published rule carries its integer-domain and nonzero-polynomial
   guards, a strict well-founded descent witness, source provenance, and an
   exact zero residual against freshly regenerated source identities.
4. Zero, symmetry, cross-family maps, factorization, product structure, and
   proper-subsector dependencies are proof-bearing.
5. Every exceptional equality/nonzero branch recursively reaches a descending
   rule, an already closed dependency, an explicitly enumerated selected
   master, or an independently certified finite zero/product/factorized
   terminal. A symbolic residual is never a terminal.
6. Solved dependencies feed back immutably and the reachable dependency graph
   reaches a deterministic fixed point.
7. No reachable leaf is uncovered, unsupported, resource-limited, interrupted,
   timed out, search-exhausted, or unresolved.
8. The artifact can be validated once at an untrusted durable or cross-process
   boundary and then represented by a sealed trusted owner. Incomplete
   resumable workspace state is never confused with an installed closed
   artifact.

Failure to find a rule is neither a zero proof nor a master proof. Finite-field
or numerical samples propose candidates only; exact regenerated-source replay
is mandatory.

## Engineering invariants

### Clean ownership first

The first milestone is a professional, aggressively cleaned codebase. Delete
obsolete RustRed APIs, schemas, tests, and compatibility layers directly; no
RustRed backward compatibility is required during deep development. Preserve
Vakint backward compatibility.

The live workspace remains:

```text
rustred (crates/rustred-core)
    ^
rustred-app (shared application services and CLI)
    ^
rustred-python (thin PyO3 adapter)
```

Core responsibilities use cohesive semantic modules. Common filename prefixes,
chronological stage names, and vague `generated`, `residual`, `runtime`,
`legacy`, or `misc` buckets are not architecture. Split large implementations
along real value, algorithm, admission, error, and test boundaries. Add a
subcrate only for a genuine dependency or independent build/test boundary.

Authentication is proportional to trust. Validate untrusted user input,
cross-process/repository handoffs, durable artifacts, and final installation.
Inside a sealed ownership boundary, do not accumulate fingerprint comparisons,
schema round-trips, deep snapshot clones, or full self-replay merely to call a
private function.

### Symbolica owns CAS work

Production RustRed and Vakint's RustRed mode are pure Rust plus GMP-enabled
Symbolica. They never link to or execute FORM, Mathematica, SymPy, or another
CAS. Before implementing any algebraic primitive:

1. pin the exact Symbolica revision and features;
2. search public exports, Rustdoc, source, examples, tests, and existing
   compositions exhaustively;
3. prefer a checked composition of native operations; and
4. compile and run focused licensed probes over edge cases.

RustRed may own authentication, shapes, physics meaning, ordering, resource
admission, guards, provenance, panic containment, and exact replay. It must not
grow a second determinant, row reducer, polynomial engine, graph-isomorphism
engine, or other CAS implementation. A genuine public-API gap produces a typed
unsupported boundary and a recorded upstream requirement.

Symbolica's intrinsic graph generation/canonization/isomorphism facilities are
the future symmetry candidate authority. RustRed owns the physics-colored graph
encoding and exact momentum/routing replay. GammaLoop `feyngen` and LiteRed2
are read-only design evidence, never production dependencies.

### Tensor service belongs to RustRed

Introduce tensor reduction only with a real core service and caller. The API
has `Auto`, an optimized single-scale vacuum lane, and a fully generic lane.
The vacuum lane is implemented first; the generic lane initially returns a
typed unsupported result rather than pretending support.

Vacuum admission is minted by RustRed from an authenticated family
presentation retaining physical-denominator versus auxiliary-ISP roles,
momentum shifts, and common-scale evidence. It is never a Vakint boolean or
topology label. Numerator-only external spectator vectors remain admissible;
external shifts in physical denominators do not.

Use the efficient global isotropic projector, not independent per-loop
averages. Pairing Gram entries depend on alternating-cycle partitions, and the
solve must operate on permutation/orbit quotients rather than invert the full
`(2r-1)!!` pairing matrix. Symbolica owns coefficient extraction, tensor
canonization, rational simplification, and matrix operations. Caller-supplied
Symbolica heads keep the service reusable outside Vakint.

The first custom-head sentinel is the one-loop reduction
`dot(k,p) k(mu)/(k^2+m^2)` to the covariant result proportional to
`p(mu)/d * [I(0)-m^2 I(1)]`, with its `d != 0` guard. The first vertical Vakint
sentinel reuses its checked one-loop tensor input and compares projected form,
raw master coefficient, and final series against existing expectations.

### Parallelism is a measured foundry design

Six-loop practicality requires deliberate multicore and memory design, not a
Rayon call around every row. Use one deterministic coordinator and one
invocation-wide bounded pool. `--n-cores` is a ceiling. Share immutable
family/source state; isolate bounded lane-local reducers and Symbolica
contexts; do not clone complete symbolic state per worker or fork a process per
task.

Admit width from RAM before constructing workers. Charge coordinator and
worker Symbolica TLS, resident immutable data, committed/trial/successor
reducer overlap, GMP/native scratch, result staging, buffers, and calibrated
headroom. Use coarse work units, compact references or bounded framed chunks,
and deterministic sorted wave/barrier merges. Thread, process, or hybrid
choices require measured RAM, CAS-scratch, and communication benchmarks.

Deterministic finite-field discovery and reconstruction may accelerate
candidate search, but sample schedules are frozen and every accepted result is
verified exactly in the authenticated Symbolica coefficient domain.

## Parallel execution tracks

### Track A — refactor milestone closed

The source/facade liveness pass, semantic file splits, legacy deletion,
documentation consolidation, bounded-allocation audit, strict Rustdoc, full
Rust/Python tests, packaging gates, and independent adversarial audit have
passed. The resulting pushed milestone is the clean baseline from which the
new tensor and foundry owners are introduced; Track A is not scientific
closure evidence.

### Track B — RustRed tensor foundation and Vakint integration

1. Maintain the now-live authenticated family-presentation contract needed to
   distinguish physical denominators, auxiliary ISPs, routing, shifts, and
   common scale.
2. Extend the now-live scalar/odd/rank-two vacuum tensor slice to the full
   projector/orbit kernel, while retaining family-aware scalar lowering and
   the typed generic stub.
3. Maintain and widen the now-live Vakint adapter on `vakint_rustred`, while
   preserving every existing input/backend/default result and keeping all
   tensor mathematics in RustRed.
4. Extend the now-passing one-loop frozen-result and FORM-oracle comparison to
   identical higher-rank and higher-loop inputs; later add guarded rule
   application and master substitution as real artifacts become available.

For local co-development, GammaLoop's workspace may temporarily depend on
`../../crates/rustred-core`. Every pushed GammaLoop milestone pins the
exact validated RustRed Git revision instead. GammaLoop's Symbolica source and
features must resolve to the same exact CAS identity.

### Track C — fresh closing-rule foundry

1. Extend the now-live concrete-anchor exact-solving boundary around freshly
   generated rows and Symbolica's native sparse reducer toward parametric
   discovery.
2. Generalize the now-live guarded, strictly descending, exactly source-replayed
   lower-loop sentinel without weakening its concrete-anchor evidence.
3. Add exceptional-domain refinement only after that generic path is real;
   retain LiteRed2's translate-before-substitute and residual recentering
   semantics without reviving its mutable architecture.
4. Build a lazy target-driven sector/dependency fixed point with proof-bearing
   zero, symmetry, factorization, product, mapping, and proper-subsector
   providers.
5. Add deterministic resumable parallel campaigns and immutable artifact
   publication only when their real callers define the contracts.
6. Close and independently validate one-, two-, three-, and four-loop domains,
   then optimize and exhaust the frozen six-loop vacuum manifest.

Tracks B and C now advance in parallel from Track A's structural gate. Vakint
validation is a lower-loop north star and must not divert foundry performance
work from six-loop closure.

## Validation and oracle policy

Evidence is reported at its actual level:

- source-count or topology-acceptance evidence is structural only;
- generated identity equality is raw-row parity only;
- a descending replayed rule is not family closure;
- tensor equality, scalar reduction, master coefficients, and evaluated series
  are distinct gates; and
- physical six-loop closure requires every frozen-manifest entry to install a
  verified closed artifact.

LiteRed/LiteRed2 and Vakint fixtures provide independent semantics and lower-
loop expectations. Existing alphaLoop, MATAD, and FMFT paths and tests may
execute a pinned FORM as compatibility/oracle coverage. The local reference
tree or a Nix-store executable may supply that oracle. RustRed, Vakint's
RustRed mode, and tests of that mode never invoke FORM and never fall back to
it; compatibility coverage should be segregated in CI. Oracle-authored
recurrence tables are not copied into RustRed rules.

Vakint's existing topology matcher and canonical routing engine remain the
steering authority through six loops. Fix defects in that engine and extend it
generically; do not duplicate or bypass it inside RustRed. The acceptance gate
must cover every topology in the frozen one-through-six-loop vacuum manifest.

## Repository and release discipline

- `FOR_REFERENCE_ONLY_DO_NOT_PUSH` is ignored and never enters RustRed
  history. GammaLoop inside it is a separate repository with its own branch
  and commits.
- Never escalate commands; use in-scope alternatives when a sandbox or tool is
  unavailable.
- RustRed has no pre-release API/format compatibility promise. Vakint retains
  backward compatibility.
- Use rollback-sized commits, the exact Git identity above, and push relevant
  milestones frequently.
- Do not authenticate internally generated artifacts repeatedly. Do validate
  final durable artifacts and provide an independent exact audit path.
- Do not claim the overall goal complete until the six-loop artifact manifest
  is coverage-closed and the FORM-less Vakint chain is operational and
  validated. Intermediate milestones must state their limitations explicitly.

## Stable project documentation

- [Architecture](docs/architecture.md)
- [Algebra and Symbolica boundary](docs/algebra.md)
- [Tensor reduction and Vakint integration](docs/tensor.md)
- [Closing-rule foundry design](docs/foundry.md)
- [Application, Python, and Vakint interfaces](docs/interfaces.md)
- [Validation and oracle ladder](docs/validation.md)
- [LiteRed2 semantic reference](docs/references/litered2.md)
- [Current CLI contract](docs/CLI.md)
