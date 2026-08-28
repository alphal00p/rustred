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

The verbatim preamble records the original directive. A later user
clarification supersedes only its strictly serial Vakint sequencing: after the
mandatory Phase 0 restructuring and the first usable, genuinely closed
lower-loop shards, the FORM-less Vakint validation lane starts concurrently
and does not wait for six-loop efficiency or closure. The six-loop foundry
remains the primary mathematical objective, and production/default execution
remains pure Rust + Symbolica.

- **Status:** active, long-horizon engineering and research goal.
- **Owner:** the primary Codex agent (`/root`), acting primarily as architect,
  orchestrator, integrator, and final verifier.
- **Delegation:** research, bounded implementation slices, performance work,
  and adversarial audits are assigned to subagents whenever they can proceed
  independently. The primary agent owns reconciliation, dependency decisions,
  integration, and capability claims.
- **Baseline:** repository commit `e4d073b` (`Complete transport-neutral
  application boundary`).
- **Cross-repository boundary:** after Phase 0 and the first genuinely closed
  lower-loop RustRed artifacts, initial Vakint/GammaLoop integration proceeds
  in parallel with the higher-loop foundry work. It is developed, committed,
  and pushed on a dedicated `vakint_rustred` feature branch created in the
  GammaLoop repository, never folded into a RustRed commit. During active local
  co-development, Vakint's Cargo manifest may use a relative path dependency
  to this RustRed checkout so uncommitted RustRed changes are exercised
  immediately. Before a reproducible GammaLoop milestone is committed/pushed
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
  mandatory heavy structural refactor before resuming the exact solver
  critical path. Structural work is an enabler, not evidence of mathematical
  closure, but a clear production architecture is a prerequisite for a
  maintainable and scalable six-loop implementation.

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

The lower-loop Vakint mode is an early parallel validation track once closed
RustRed shards exist. Its complete six-loop deployment, GammaLoop integration,
AMFlow master-data production, and final physics computation remain downstream
gates. Parallel adapter work must not displace the first offline one- through
six-loop vacuum artifact-library goal or be presented as its completion.

Vakint's integrated steering role does not replace RustRed's own interfaces.
The RustRed CLI and Python API remain first-class, supported, fine-grained
control surfaces over the shared application/core path. They expose family
construction, raw parametric IBP/LI generation, closure campaigns, artifact
inspection and exact verification, and—when implemented—the individual
tensor, scalar-reduction, rule-application, and master-substitution services.
These interfaces accept fully generic families with external kinematics,
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
  alphaLoop, MATAD, and/or FMFT paths with the supplied FORM5 executable under
  `FOR_REFERENCE_ONLY_DO_NOT_PUSH/form5`. This exception exists solely to
  produce or compare authoritative lower-loop reference results. It is never a
  RustRed dependency, never a fallback of the new mode, never copied into
  production logic, and never part of the default FORM-less test/runtime path.
  Oracle executable/version, inputs, conventions, and frozen outputs are
  authenticated explicitly.
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
  guards, provenance, replay, resource admission, scheduling, transactions,
  artifacts, and structural tensor semantics. Those responsibilities do not
  authorize a second algebra engine.

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
  policies as data. Loop/topology-named recurrences live only in tests,
  examples, benchmarks, or an explicitly quarantined legacy-oracle package.
- No eager `2^K` sector/orthant enumeration at high loop order, no brute-force
  `GL(L,Z)`/bounded `3^(L^2)` symmetry discovery, no eager global exact
  rational-function elimination, and no authored recurrence dispatch.
- Graph automorphisms, routing equivalences, and polynomial signatures may
  propose symmetries. Only the generic exact affine/momentum-map verifier may
  certify them.
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

## Required architecture

The workspace converges on four deliberately bounded packages:

```text
rustred-python ------> rustred-app ------> rustred core ------> Symbolica[gmp]
                            |
                            +-- rustred CLI binary

rustred-legacy-oracles ------------------> rustred core

Vakint `RustRed` mode -------------------> rustred core
```

The root `rustred` crate is the topology-neutral mathematical core and, in the
parallel Vakint workstream as well as the complete production phase, the
implementation home for tensor reduction, scalar lowering, guarded rule
application, and typed master substitution.
`rustred-app` is the typed transport-neutral composition layer and CLI host.
`rustred-python` is a thin PyO3/maturin adapter over `rustred-app`, with no
algebra or independent schema. `rustred-legacy-oracles` is publish-disabled,
default-unlinked, and contains only authored historical validation logic.
Vakint is the concurrently developed lower-loop and ultimately complete
user-facing steering layer over the relevant RustRed core services; it must
not duplicate their algebra or silently fall back to FORM. The CLI and Python
package remain parallel first-class interfaces for fine-grained generic work
rather than compatibility shells around Vakint.

The four-package layout is a disciplined baseline, not a prohibition on
additional subcrates. Introduce another crate when it creates a demonstrable,
acyclic ownership or dependency boundary, materially improves independent
testing/compilation, and avoids a new dumping ground. Do not create transport-
only or one-file microcrates merely to make the tree look modular.

Within the core, dependency direction must be explicit and acyclic across
these conceptual areas:

```text
Symbolica algebra adapters
        ↓
family/input and authenticated coefficient contexts
        ↓
IBP/LI generation, sector geometry, zero/symmetry/factorization proofs
        ↓
exact solving, guards/WhenBad, exceptional closure, subsector feedback
        ↓
artifact verification/publication and deterministic campaign execution
        ↓
tensor/scalar/master application services behind a typed RustRed core API
        ↓
rustred-app adapters and, once lower-loop shards close, the parallel Vakint
`RustRed` workstream
```

Private implementation names and files should describe mathematical roles,
not historical implementation chronology. Code is factored to the highest
professional standard practical for this project: cohesive modules, narrow
interfaces, explicit owners, acyclic dependencies, minimal visibility, and no
duplicated authority. Giant inline test campaigns move beside integration
fixtures. Public re-exports are narrowed. Semantic solver changes are not
mixed into mechanical file moves. Git is the archive; stale code and
documentation are deleted after their unique evidence is retained.

Legacy paths are transitional evidence, not permanent architecture. Remove
obsolete compatibility layers, V1/V2 bridges, handwritten production algebra,
loop-authored reducers, dead modules, and stale tests/docs once their unique
fixtures or differential evidence have moved to the appropriate current
boundary. Retain something in `rustred-legacy-oracles` only when it has a
specific continuing oracle purpose, cannot yet be replaced by a smaller data
fixture, is unreachable from default production, and has an explicit retention
or deletion decision. “Legacy” is never an acceptable production dependency.

### Trust-boundary and compatibility discipline

Authentication is proportional to the boundary. RustRed validates untrusted
user input, deserialized durable artifacts, cross-process or cross-repository
handoffs, live mutation/transaction commits, and final artifact installation.
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
working while the new FORM-less path is introduced.

## Execution roadmap

### Phase 0 — mandatory heavy refactor and authority cleanup

This phase is the important first implementation step. It executes before new
solver feature growth and preserves a green licensed-GMP baseline after each
rollback-sized tranche. Its purpose is to turn the current large, historically
layered codebase into a professional base whose production call graph,
ownership, CAS authority, test boundaries, and future Vakint-facing services
can be understood and optimized independently.

Each coherent package extraction, module-tree move, visibility tightening,
legacy removal, and documentation consolidation is committed and pushed after
its focused and workspace gates pass. Mechanical moves remain independently
reviewable from semantic changes.

1. Treat the completed `rustred-app` transport-neutral boundary as the fixed
   starting point.
2. Add `rustred-python` over `rustred-app` only: owned requests under the GIL,
   GIL release for work, one process-wide coordinator before Symbolica
   initialization, poison-on-panic behavior, canonical CLI/app/Python byte
   parity, `n_cores = 1,2,4`, and clean wheel/sdist tests.
   Preserve the shared API's generic family/kinematics model and fine-grained
   operation boundaries; neither frontend may become vacuum-only or merely a
   launcher for Vakint.
3. **Complete:** create publish-disabled `rustred-legacy-oracles`, move all 35
   compiled authored loop/topology modules, the six-module concrete
   vacuum-family/IBP/reduction oracle engine, 35 dedicated integration tests,
   and four diagnostic examples, and remove the former root feature and
   concrete-engine surfaces. The package is not a default workspace member
   and depends one-way on the core's narrow hidden support facade; the default
   production graph does not link it.
4. **Complete:** resolve every unwired source explicitly as wire, move, or
   delete with reachability evidence. The three never-compiled orphan drafts
   (`five_loop_d4`, `four_loop_next_conditions`, and
   `exact_sparse_provenance`) were deleted: the authored shells were
   incomplete and had no executable oracle, while the provenance draft
   duplicated the live exact/Symbolica transcript path with a second
   handwritten algebra and replay layer. Git retains their historical text;
   no compatibility shim or archive crate was added.
5. **In progress:** refactor the 450k-line flat core into clear
   topology-neutral algebra, family, identity, sector, solver/closure,
   artifact, campaign, tensor, and application boundaries; move large test
   campaigns out of production modules and reduce visibility. The first
   nested core boundary is complete: deterministic campaign planning,
   resources, width selection, work identity, and admission live behind the
   selective `rustred::campaign` API with private child modules and no root
   compatibility aliases. The unused raw exact-relation compiler has also
   been deleted after its unique GMP work, sealed-census, and exact buffer
   boundary tests were transplanted to the live recenter kernel; Git retains
   the obsolete i64 differential rather than a `cfg(test)` compatibility
   authority. The exact-session foundation and transaction-core tranches are
   also complete: physical keys, immutable solve plans, sealed physical rows,
   the exact recenter kernel, transactional database, target catalog,
   transactional session owner/state machine, and native sparse telemetry now
   live as private children of
   `solver::exact_session`. External production consumers reach only a
   selective crate-private facade, shared fixtures live behind a test-only
   support facade, and no old-path aliases remain. Ready/WhenBad
   materialization, publication/epoch scheduling, their remaining ownership
   cycles, and the other mathematical clusters still need reorganization.
6. Consolidate the research corpus into a small authoritative index for scope,
   architecture, solver, campaigns, interfaces, references, status, and
   acceptance. Delete reconciled stale documents rather than growing an
   in-tree archive.

Phase 0 is complete only when package/module ownership and dependency direction
are explicit, no authored recurrence is reachable from the default product,
tests/fixtures cannot be imported by production, outstanding Symbolica
migrations have named boundaries, and the README/design index describe the
actual tree. This phase does not claim to have changed solver semantics.

### Phase 1 — Symbolica-native foundry authority and one exact exceptional child

First clean the algebra authority on the production foundry call path, then
resume the current exact solver at its narrowest missing production seam:

1. Remove every production-reachable handwritten determinant, matrix product,
   polynomial kernel, exact/parametric Gaussian engine, and finite-field
   implementation for which Symbolica has a public operation. Prioritize the
   family/zero/symmetry foundations and all eliminators reachable from the
   direct foundry. Preserve RustRed ordering, guards, provenance, and resource
   control, and keep old algebra only as a temporary differential test with a
   deletion point.
2. Migrate the tensor/application algebra needed by the Phase 4 lower-loop
   Vakint validation track when that track starts. Defer only the remaining
   six-loop/high-throughput application algebra until its measured production
   gate; it does not block the first derivation-only exceptional child.
3. Consume the committed exceptional `CampaignResident` before constructing
   any child exact session.
4. Bind its ordered `EqualZero` premises through a private source view to the
   literal-unit affine-refinement compiler.
5. Invoke the currently private mapped-`NonZero` worker for every surviving
   inherited condition and commit a typed refined-source successor while
   preserving recoverable rollback.
6. Only then construct fresh child authority, frame, solve plan, catalog,
   retained Symbolica reducer, and exact session.
7. Regenerate all inherited generic IBP/LI rows under the child affine map,
   attach mapped guards/base assumptions, submit them chronologically to the
   empty child database, and re-enter exact recentering and `WhenBad`.
8. Prove parent/child substitution and exact residual replay, resource exact/
   one-below behavior, stale/foreign identity rejection, and unwind rollback
   on natural one- and two-loop constrained branches.

No parent pivot/database state may leak into the child. An empty diagnostic,
unsupported quotient, or successful map alone is not a closed branch.

### Phase 2 — generic sector and family fixed-point closure

1. Replace eager family-wide sector enumeration with a lazy, target-driven
   dependency DAG so no high-loop path materializes `2^K` sectors or orthants.
2. Add graph-automorphism/routing-equivalence candidate ingress to the generic
   exact symmetry verifier; bounded matrix enumeration remains only a
   small-family oracle.
3. Connect applicable-rule and exceptional-residual publication to the exact
   native-session lineage with atomic chronological replay.
4. Add recursive exceptional scheduling, same-database `IdenticallyBad`
   continuation, authenticated ancestry, and immutable solved-subsector
   feedback.
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
   staging, and opaque headroom. No nested pools or per-job process forks.
6. Use deterministic finite-field discovery and reconstruction only as a
   proposal accelerator; freeze sample schedules and verify every result
   exactly over the authenticated Symbolica coefficient domain.

### Phase 4 — evidence ladder through four-loop oracles

1. Produce genuinely coverage-closed one-, two-, and three-loop generic
   shards, including tensor/scalar input closure and cancellation metamorphic
   tests.
2. Translate pinned LiteRed/LiteRed2 examples into data fixtures and progress
   them from input/identity parity through sector/symmetry, guarded-rule, and
   target-reduction parity without topology-specific production logic.
3. Once the first closed lower-loop shards exist, create the dedicated
   GammaLoop feature branch and develop Vakint's FORM-less `RustRed` mode in
   parallel with Phases 3 and 5. Split this work across independent subagent
   lanes for source/oracle audit, topology matching, RustRed adapter and rule
   application, tensor/master handling, performance, and adversarial review.
4. Reuse Vakint's existing topology matching and canonicalization engine as the
   steering authority. Fix defects found by the existing end-to-end corpus,
   add exact convention/routing witnesses at the RustRed boundary, and extend
   candidate/registry support generically through six loops. An exhaustive
   acceptance gate must prove that every entry in the frozen one- through
   six-loop vacuum manifest is accepted, canonicalized, and routed by the
   reused matcher. Defects are fixed in that matcher rather than bypassed or
   duplicated inside RustRed; topology-name dispatch is forbidden.
5. Implement a bounded but real RustRed-core/Vakint path for lower-loop native
   tensor reduction, scalar lowering/cancellation, guarded IBP application,
   stable master keys, and supplied master substitution. The new path itself
   remains pure Rust + Symbolica and uses independently generated RustRed
   artifacts.
6. Run Vakint's existing one- through four-loop end-to-end tests through the
   pinned alphaLoop, MATAD, and/or FMFT backends using the reference FORM5
   executable only in the segregated oracle job. Compare final expressions
   over unsubstituted masters—and master-substituted results where authoritative
   data exists—after an explicit convention map. Never copy the oracle's
   authored recurrence tables into RustRed rules.
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
begins. While iterating locally, `crates/vakint/Cargo.toml` may point to the
local RustRed checkout for immediate cross-repository feedback. Before each
reproducible pushed milestone, CI run, frozen oracle comparison, or published
artifact claim, it points to the RustRed GitHub repository at the exact
validated RustRed commit used by that milestone. Machine-specific absolute
paths are never committed. RustRed changes remain in the RustRed repository.
Before every commit, both repositories' scopes are checked independently so no
path under `FOR_REFERENCE_ONLY_DO_NOT_PUSH/` enters RustRed history.

The combined Cargo graph must resolve one compatible pinned Symbolica package
and feature set, with GMP enabled and `no_gmp` absent. A duplicated or
revision-incompatible Symbolica dependency is rejected before exposing a
zero-copy `Atom` boundary between Vakint and RustRed.

None of the Vakint/RustRed implementation layers may invoke FORM, Mathematica,
SymPy, or copied authored recurrences. Only the segregated existing-backend
oracle job described above may execute FORM5.

## Current evidence baseline

As of the assigned baseline, the following are real but partial:

- topology-neutral family lowering and raw parametric IBP/LI generation;
- the synthetic `L=6`, `K=21`, 36-source generation/stress fixture;
- automatic ISP completion and several zero/symmetry/tensor foundations;
- a retained Symbolica `SparseRowReducer` exact database with authenticated
  recentering, condition planning/materialization/partition, compact events,
  handoff/epoch ownership, and same-database rejection machinery;
- static multi-root planning and preliminary memory-width admission; and
- the transport-neutral `rustred-app`/CLI boundary.

The strongest end-to-end reductions are narrow one-loop cases; one
generic-source-derived but narrowly wired two-loop sunset target; and an
equal-mass three-loop tetrahedron fixture with generated rows, discovered
`S4` symmetry, demand-time concrete quotient reduction, and five explicitly
selected masters. They are validation evidence, not closed reusable family
artifacts.

The following claims are explicitly false at this baseline:

- the public application path does not derive or publish closed rules;
- the synthetic six-loop fixture is not a physical six-loop family or a
  reduction result;
- no physical six-loop source has reached `Ready`, published a guarded rule,
  or closed a sector;
- the current family inventory is unsuitable for `K=21` because it eagerly
  enumerates sectors and its default cap is below `2^21`;
- the private mapped-`NonZero` worker has no production caller;
- fresh exceptional child sessions do not yet contain regenerated refined
  rows;
- public family/fixed-point routes still reach handwritten eliminators;
- production symmetry discovery is not scalable to six loops;
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
declared Vakint reference-oracle job may execute pinned FORM5 only to run the
existing alphaLoop/MATAD/FMFT comparison paths; the new mode remains FORM-less.
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
- `docs/research/rustred_scope_and_acceptance.md` and
  `docs/research/litered_full_scope_spec.md` for mathematical scope;
- `docs/research/repository_reorganization_directive_2026-08-27.md` and
  `docs/research/python_api_directive_2026-08-27.md` for the initial
  architecture gate;
- `docs/research/exact_session_when_bad_port_plan_2026-08-24.md` and the
  exceptional/refinement notes for the live solver continuation;
- `docs/research/six_loop_single_scale_vacuum_priority_2026-08-24.md` and
  `docs/research/parallel_campaign_foundry_design_2026-08-26.md` for the first
  deployment and scaling contract;
- `docs/research/litered2_algorithm_report.md` and
  `docs/research/litered_examples_acceptance_matrix.md` for LiteRed semantics
  and oracle progression;
- `docs/research/symbolica_rust_api_for_litered.md`,
  `docs/research/symbolica_exact_linear_algebra_api_inventory.md`,
  `docs/research/symbolica_first_algebra_migration_audit_2026-08-24.md`, and
  `docs/research/symbolica_only_algebra_compliance_roadmap_2026-08-27.md` for
  the CAS boundary; and
- `docs/research/vakint_alphaloop_tensor_ibp_audit.md` and
  `docs/research/gammaloop_six_loop_boundary_audit_2026-08-24.md` for the
  parallel lower-loop oracle/application boundary and complete deployment.

Historical notes remain evidence only when they conflict with a newer
governing document or the live implementation. In particular, loop-authored
recurrence notes, eager MTBDD/sector strategies, handwritten algebra plans,
and obsolete “not implemented” checkpoints cannot override this goal.
