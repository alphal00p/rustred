# RustRed restart handoff

Last updated: 2026-08-27

This file is the restart authority for the next Codex task. Read it before
changing code. It records what was actually implemented and tested, what is
still only infrastructure, and the agreed route to the full RustRed goal.

## Repository state at handoff

- Worktree: `/shared/localunitaritythree/LiteRed`
- Branch: `main`
- Remote: `origin` -> `https://github.com/alphal00p/rustred.git`
- Latest solver checkpoint: `ed3cc0a84bac7267f05e2f197ab3c6e36677b080`
  (`Map refined nonzero predicates through Symbolica`)
- Refactor checkpoint: `7ff5f6623f5d6ab147607a46ee4dbbb9f0a2b3e3`
  (`Extract shared RustRed application package`)
- The commit containing this file is the final restart checkpoint. Verify that
  local `HEAD` equals `origin/main` and that `git status --short` is empty
  before resuming.

The two checkpoints above were pushed to `origin/main`. No license key is
stored in the repository.

## Goal and non-negotiable scope

RustRed is to provide the mathematical scope of LiteRed in pure Rust, using
Symbolica as the computer algebra system. The conceptual baseline is
**rnlg/LiteRed2**, vendored at `vendor/LiteRed2`; it is not LiteRed1 and is not
a source-level or bug-for-bug port. LiteRed2 is guidance for the generic
algorithms and acceptance surface. RustRed should be better structured,
generic, and optimized for production campaigns.

The final system must:

- derive complete, fully parametric IBP and Lorentz-invariance relations from
  a user-provided integral family;
- discover guarded parametric replacement rules and prove that they close on a
  reduced master set, rather than loading topology-authored recurrences;
- reduce concrete scalar and tensor-numerator integrals with those generated
  rules;
- support arbitrary loop counts, external momenta, masses, scalar parameters,
  propagator powers, and irreducible scalar products subject to resources;
- offer a Rust library, human-oriented CLI, and PyO3 API over one semantic
  implementation;
- prepare and combine artifacts for several starting topologies in one
  campaign; and
- prioritize fast single-scale massive-vacuum campaigns through six loops for
  the intended QCD beta-function application after GammaLoop's BPHZ
  R-operation.

Concrete topologies and fixed powers are validation inputs and oracle fixtures
only. Production derivation and reduction must never branch on names such as
`tadpole`, `sunset`, `three_loop`, `vacuum`, or a loop-count-specific family.
Loop count is ordinary input data, not a dispatch key.

The following constraints are standing instructions:

- Do not invoke FORM or a Mathematica kernel. Vakint FORM sources may be read
  only to understand tensor reduction and to extract frozen acceptance oracles.
- Do not build a second CAS. Delegate algebra, including exact polynomial,
  rational-function, substitution, factorization, and matrix operations, to
  Symbolica's public Rust API. Search all of that API before adding algebraic
  code in RustRed.
- Use vendored Symbolica with its `gmp` feature. Never enable `no_gmp`.
- The user supplies `SYMBOLICA_LICENSE` in the process environment. Do not
  commit the value, print it in logs, or place it in documentation.
- Tests should run in parallel. `--n-cores` is the user-visible concurrency
  ceiling; do not confuse it with the test runner's thread count.
- Six-loop execution must be RAM-aware. On a roughly 100-core, 1-TiB EPYC
  machine, do not fork every available task or let nested pools multiply the
  requested width. Preserve headroom for Symbolica-native scratch and the OS.
- Use several agents for research, implementation audit, and validation. The
  root agent owns the final interpretation and must reconcile disagreements.
- Do not escalate commands. If a command is sandbox-blocked, use a safe
  alternative. Commit and push each real milestone.
- Backward compatibility is not required during this pre-release refactor.

## Intended mathematical workflow

The high-loop vacuum workload naturally separates into two stages:

1. **Generic preparation:** for every family reachable from one or several
   roots, generate ordinary IBP/LI identities, discover sectors and
   symmetries, solve all required parametric cases with explicit exceptional
   loci, prove closure, and publish deterministic reusable rules.
2. **Concrete application:** tensor-reduce and scalarize each concrete graph,
   normalize algebraically equivalent numerator/denominator spellings, then
   apply the prepared rules efficiently until only masters remain.

Vakint is an authoritative concrete oracle through its current supported loop
orders, but its hard-coded FORM recurrence tables are not an implementation
template. RustRed must independently derive equivalent rules and results, then
go beyond Vakint to five and six loops.

An important closure invariant is spelling independence. For example, an input
with numerator `q_i^2-m^2` cancelling a denominator must agree with the same
integral supplied after explicit cancellation. Keep numerator-cancellation
equivalence in every future acceptance campaign.

## What is implemented now

### Generic family input and raw relation generation

The topology-neutral core can parse and lower:

- one compact Symbolica `I(...)` expression;
- hybrid TOML containing an `I(...)` expression plus optional metadata; and
- explicit compact TOML family/kinematics/target data.

It infers family scalar parameters such as masses from the expressions when
they are not explicitly declared. It constructs affine integral families and
generates the raw, fully parametric ordinary IBP relations and, when external
momenta exist, Lorentz-invariance relations. The relation coefficients and
canonical expression strings use Symbolica. Concrete target powers and
numerators do not specialize the universal generated rows.

This front end is structurally generic in loop count, external count, masses,
and denominator count. That does **not** mean the downstream closed reduction
algorithm is complete at arbitrary size.

### Tensor and numerator infrastructure

There is native Rust/Symbolica infrastructure for parsing tensor numerators,
constructing covariant vacuum tensor projectors, contracting metric/vector
structures, and lowering the result to scalar products suitable for family
coordinates. One-loop scalar and tensor cases have frozen Vakint-derived
oracle coverage, including a public-Symbolica rank-six projector check. There
are also two-loop tensor-oriented fixtures.

This remains a library validation path, not a complete CLI pipeline that takes
an arbitrary tensor integral through generated-rule closure to masters. Do not
claim arbitrary one-loop pentagons or general multiloop tensor reductions are
complete yet.

### Campaign and resource infrastructure

`rustred campaign plan` accepts several root topologies, authenticates and
deduplicates families and declared-power sector jobs, and emits a deterministic
**roots-only** plan. Dependency discovery, derivation execution, closure, and
artifact publication are explicitly marked not started in that output.

`rustred campaign preflight` reads a physical resource profile and computes a
RAM-aware effective width before a worker pool is constructed. Low-level core
types enforce move-only admission, bounded worker counts, retained/transient
memory accounting, and stable work ordering. The production multi-root
frontier coordinator is not implemented.

### Guarded parametric solving substrate

The repository contains a large topology-neutral exact-solving and guarded
coverage substrate: ordering, sector geometry, generated row systems,
symbolic sparse elimination, conditional loci, `WhenBad` partitions,
symmetry/source authority, replayable sessions, publication handoff/epoch
owners, and RAM/resource contracts.

The latest solver checkpoint, `ed3cc0a`, added a private source-neutral worker
in `src/generated_affine_residual_case_mapped_nonzero.rs`. It:

- replays a refined unit-affine child coordinate plan;
- preflights inherited and exceptional `NonZero` mapping work;
- maps predicates by simultaneous Symbolica substitution;
- diagnoses the first predicate mapped identically to zero without turning the
  diagnostic into source authority;
- discharges nonzero integer constants; and
- canonicalizes base-field loci separately from index-dependent loci.

It also tightened Symbolica/GMP retained-polynomial and transient-work resource
accounting. The worker remains private and is not yet bound to the committed
exceptional resident. The immediate mathematical continuation is to bind it,
regenerate the child rows in the refined coordinates, construct a fresh child
exact session, and recursively prove closure.

### Completed CLI/application boundary

The current Cargo workspace has exactly two packages:

```text
rustred-app  --->  rustred core  --->  vendored Symbolica[gmp]
     |
     +-- `rustred` CLI binary
```

`rustred-app` depends directly only on `rustred`, Serde, and TOML. Its former
direct Symbolica edge was removed through the narrow core-owned
`symbolica_runtime` representation facade; the core remains the sole package
which depends directly on vendored Symbolica with GMP.

The root `rustred` package is still the mathematical core. Commit `7ff5f66`
moved the CLI modules and all three CLI integration suites into
`crates/rustred-app`. It introduced owned requests/results and public functions
for:

- `derive`;
- `campaign_plan`; and
- `campaign_preflight`.

Results expose schema, status, and canonical newline-terminated TOML. The app
and CLI share the 16-MiB ingress and 256-MiB output limits. Workspace package
versioning is centralized. Direct integration tests prove byte-for-byte
API/CLI output parity for all three operations. `InputFormat` and
`RelationSelection` implement typed string parsing.

The transport boundary is now complete. `crates/rustred-app/src/application`
owns requests/results, neutral options, stable application error kinds, input
normalization, lowering orchestration, derivation, campaign planning and
preflight, resource limits, producer metadata, and canonical serialization.
`crates/rustred-app/src/cli` contains only OS argument parsing, path/stdin/stdout
handling, overwrite policy, exit-code/category mapping, help, and terminal
diagnostics. `ArgError` is private to that adapter.

Public application calls document that expected failures return `AppError` and
that invariant panics are intentionally not caught. The future PyO3 coordinator
must catch only at its outer boundary, poison itself, and reject later work;
that coordinator and the Python package are not implemented yet.

## What is not implemented

The following are important negative claims:

- There is no complete generic coverage-closed replacement-rule artifact yet.
- RustRed cannot yet reduce every arbitrary integral to masters.
- It cannot yet completely reduce an arbitrary one-loop pentagon with general
  external kinematics and tensor numerator.
- It has not independently reproduced the full Vakint one- through four-loop
  replacement systems.
- It has not completed physical five- or six-loop vacuum derivation or a
  meaningful six-loop scalability gate.
- Multi-root input planning exists, but multi-root dependency discovery,
  derivation scheduling, checkpointing, closure, and publication do not.
- The PyO3 package does not exist.
- The proposed `rustred-legacy-oracles` package does not exist.
- The flat core solver hierarchy and the large documentation history have not
  yet undergone the deep move/delete phase.
- Published LiteRed/LiteRed2 notebook examples are inventoried, but no full
  translated notebook workflow passes. The user may later supply evaluated
  notebooks with outputs for authoritative targets.

Loop-named modules and tests already in the tree are historical pipelines,
frozen oracles, partial boundary checks, or feature-gated authored recurrence
material. Their presence is not proof of generic support at that loop count.
No new production work should depend on authored special recurrences.

## Reorganization decision

The reconciled target is deliberately small:

```text
rustred-python ------> rustred-app ------> rustred core
                            |
                            +-- `rustred` CLI binary

rustred-legacy-oracles ------------------> rustred core
```

The current app/core edge now matches this target: `rustred-app` has no direct
Symbolica dependency. Version metadata, canonical Atom rendering, packed-Atom
census, and Symbolica-integer census cross a narrow core-owned facade without
re-exporting Symbolica wholesale or changing the established resource bounds.

- Keep the root package as the topology-neutral core during the migration; do
  not move roughly 400k lines merely to satisfy a cosmetic `crates/` layout.
- Keep the CLI binary inside `rustred-app`; a transport-only CLI microcrate adds
  no useful boundary.
- `rustred-python` will be a dedicated PyO3/maturin package depending only on
  `rustred-app`.
- `rustred-legacy-oracles` will be `publish = false`, hold authored
  topology/loop material and frozen oracle support, and depend only on core.
- Do not create algebra, solver, tensor, campaign, or test-support microcrates
  until an acyclic measured dependency boundary justifies one. First impose a
  clear internal module hierarchy and reduce visibility.

The pre-extraction audit reported 151 default production modules (about
397,364 lines), including 93 solver/closure files (about 272,662 lines), plus
35 feature-only authored loop modules (about 43,749 lines). It also identified
three source files with no production wiring:

- `src/exact_sparse_provenance.rs`
- `src/five_loop_d4.rs`
- `src/four_loop_next_conditions.rs`

Do not silently retain or delete those files. Decide whether each should be
wired and tested, moved to legacy-oracle support, or removed with Git history
as the archive.

Before this handoff was added, the repository had 80 RustRed-owned Markdown
files totaling roughly 42,300 lines; `HANDOFF.md` is the 81st. The
documentation audit proposed a final discoverable set of about 12–13 current
pages and identified roughly 20 stale/superseded documents for
evidence-preserving consolidation and deletion. Do not create a `docs/archive`
directory; Git is the archive.

## Validation evidence

All licensed commands used the user's `SYMBOLICA_LICENSE` only in the process
environment, used Symbolica's default GMP-backed configuration, and did not
enable `no_gmp`.

### Solver checkpoint

- Focused mapped-`NonZero` module: 10/10 tests passed with four test threads.
- `cargo check --tests -j8`: passed before the checkpoint was pushed.
- Formatting and diff hygiene: passed.
- Pushed commit: `ed3cc0a84bac7267f05e2f197ab3c6e36677b080`.

A full default-GMP run was attempted before the restart request:

- command: `cargo nextest run -j8 --no-fail-fast`
- run ID: `8945cbed-1cf4-4bd3-93d6-63006b3e638a`
- observed before deliberate SIGINT: 1,912 passed, 44 slow, 5 skipped;
- the sole in-flight test was
  `generated_affine_residual_group_exact_session::tests::equality_target_commits_only_into_a_sealed_refined_epoch_suspension`;
- that test was CPU-bound at about 99% and had run for about 1,378 seconds when
  interrupted;
- Nextest reported one failed test because of SIGINT. This was an incomplete
  gate, not a recorded semantic assertion failure.

Do not report the full suite as passing. Investigate or profile that slow test
before repeatedly spending another half hour on it.

### Initial application extraction checkpoint (historical)

- `cargo metadata --no-deps --format-version 1`: passed and reported exactly
  `rustred` and `rustred-app` as workspace members.
- `cargo check --workspace --all-targets -j8`: passed.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed.
- Parallel app gate:
  `cargo nextest run -p rustred-app -j8 --no-fail-fast`
- run ID: `3ecbae9f-2d5f-474a-b305-0cab1333f2ea`
- result: 35/35 passed, 0 skipped, across six binaries.
- Those 35 include 8 app unit tests, 25 moved CLI integration tests, and 2 new
  direct application contract tests. The contract test compares canonical
  bytes with CLI stdout for derive, campaign plan, and campaign preflight.
- Two independent read-only agents audited package direction, feature graph,
  topology neutrality, moved fixture paths, CLI behavior, and test results.
  Neither found a blocker to committing that initial seam. One identified the
  CLI coupling and panic-boundary work completed by the next checkpoint.

That 35-test parallel app gate was a **licensed** gate. Do not reinterpret a
parallel unlicensed run as a supported mode: concurrent Symbolica-using child
processes can terminate by signal. The focused campaign-preflight suite is the
license-free application gate; license-mode and abort-prone affinity probes
belong in fresh subprocesses.

The full 1,900+ test suite was not rerun after the mechanical app extraction;
the whole-workspace all-target build and focused parallel app suite are the
restart checkpoint.

### Completed transport-neutral application boundary

- `cargo check --workspace --all-targets -j8 --locked`: passed after the
  refactor.
- Licensed parallel app gate:
  `cargo nextest run -p rustred-app -j8 --no-fail-fast`
- run ID: `3fdaad79-3908-42e8-b7b1-9c6a30355da6`
- result: 39/39 passed, 0 skipped, across six binaries.
- The original 35 tests remain represented. Four focused boundary tests cover
  typed application error classification, transport-neutral memory parsing,
  and CLI exit mapping.
- The direct application contract still proves byte-identical canonical output
  against CLI stdout for derive, campaign plan, and campaign preflight, with
  derive parity at `n_cores = 1, 2, 3, 4` on this validation host.
- `rustred-app` now depends directly only on `rustred`, Serde, and TOML. Its
  source contains no direct `symbolica::` path or import, and application
  modules do not depend on `crate::cli`.
- Fresh metadata still reports exactly two workspace packages. The resolved
  feature graph has Symbolica `gmp` enabled, with `no_gmp` and PyO3 absent.
- Three fresh independent read-only audits covered architecture, acceptance,
  tests/documentation, and validation. Their findings were reconciled; the
  final worktree has no reported milestone blocker.

This remains a **licensed** parallel gate. The incomplete full-suite result
recorded above has not been reinterpreted as a pass; the boundary milestone is
gated by the whole-workspace all-target build and the focused application
suite.

## Commands for the next task

Use the existing Nix development environment. Set the license only in the
process environment; substitute the user's supplied value locally:

```bash
export SYMBOLICA_LICENSE='<user-supplied-license>'
export SYMBOLICA_HIDE_BANNER=1

nix develop --accept-flake-config --command cargo metadata --no-deps --format-version 1
nix develop --accept-flake-config --command cargo check --workspace --all-targets -j8
nix develop --accept-flake-config --command cargo nextest run -p rustred-app -j8 --no-fail-fast
nix develop --accept-flake-config --command cargo fmt --all -- --check
git diff --check
```

The root workspace must leave the root package implicit:

```toml
[workspace]
members = ["crates/rustred-app"]
default-members = [".", "crates/rustred-app"]
```

Listing `.` explicitly in `members` caused Cargo to treat descendant vendored
paths as explicit-member prefixes and collide with Symbolica's nested
workspace. Keep `vendor/symbolica` excluded and re-run `cargo metadata` after
every workspace-package change.

## Exact next sequence

Keep each item rollback-sized and push after every completed item.

1. **Complete: finish the app boundary.** Semantic input/lowering/output/campaign
   services out of `cli::*`; define transport-neutral option and error types;
   keep argument parsing, stdin/path handling, overwrite policy, exit codes,
   help, and terminal diagnostics in the CLI adapter. Do not blanket-catch
   invariant panics and then reuse potentially mutated application/Symbolica
   state. The original 35-test gate remains represented, exact canonical bytes
   are preserved, and four focused boundary tests were added.
2. **Add `rustred-python`.** Use PyO3 and maturin over `rustred-app` only. Convert
   Python values to owned Rust requests under the GIL, release the GIL for
   work, and route top-level calls through one process-wide coordinator thread
   before any Symbolica initialization. This is required because the vendored
   unlicensed Symbolica manager can be first-thread-affine and because
   concurrent callers must not multiply private pools. Catch Rust panics only
   at the outer coordinator/FFI boundary, translate the current call to a typed
   Python failure, poison the coordinator, and reject later work. Prove
   CLI/app/Python byte parity, `n_cores = 1,2,4`, malformed/resource/license
   error parity, clean wheel installation, and GMP linkage. Exercise license
   modes and abort-prone thread-affinity cases in fresh subprocesses because
   Symbolica initialization is process-global and one-shot. Do not enable
   Symbolica's Python feature.
3. **Extract legacy authored oracles.** Create the publish-disabled
   `rustred-legacy-oracles` package, move loop/topology-authored recurrences and
   their dedicated tests there, and prove the default core no longer links
   them. Keep concrete fixtures in tests/examples.
4. **Resolve the three unwired source files** listed above, with explicit
   evidence for wire/move/delete decisions.
5. **Refactor the core hierarchy.** Group Symbolica algebra boundaries,
   family/input, tensor reduction, IBP/LI generation, sector geometry,
   exact-solving/closure, campaign execution, and publication under clear
   topology-neutral modules. Move giant inline test campaigns adjacent to
   integration fixtures and reduce public re-exports. Do not combine semantic
   solver rewrites with file moves.
6. **Consolidate documentation.** Establish a small indexed set for scope,
   architecture, solver, campaigns, CLI, Python, LiteRed2/Symbolica/Vakint
   references, status, and acceptance matrices. Delete reconciled stale files
   rather than archiving them in-tree.
7. **Resume the solver critical path.** Bind the mapped-`NonZero` worker to the
   committed equality resident, regenerate child rows, create the fresh child
   exact session, recurse through exceptional cases, feed solved subsectors
   back, prove fixed-point closure, and publish deterministic parametric rules.
8. **Scale in evidence order.** Complete one-loop scalar/tensor end-to-end
   Vakint parity and numerator-cancellation closure; then two-, three-, and
   four-loop Vakint reproduction; then derivation-only five-/six-loop vacuum
   gates; only after that optimize concrete high-volume rule application.

If the restart must be even shorter, complete item 1, push it, and leave item 2
for the next milestone. Do not skip item 1 and bind PyO3 directly to CLI
internals.

## Files to read first

1. `HANDOFF.md`
2. `README.md`
3. `docs/research/rustred_scope_and_acceptance.md`
4. `docs/research/repository_reorganization_directive_2026-08-27.md`
5. `docs/research/python_api_directive_2026-08-27.md`
6. `docs/research/six_loop_single_scale_vacuum_priority_2026-08-24.md`
7. `docs/research/parallel_campaign_foundry_design_2026-08-26.md`
8. `docs/research/litered2_algorithm_report.md`
9. `docs/research/symbolica_rust_api_for_litered.md`
10. `docs/research/symbolica_api_report.md`
11. `docs/research/vakint_alphaloop_tensor_ibp_audit.md`

Code entry points for the immediate refactor are:

- `crates/rustred-app/src/api.rs`
- `crates/rustred-app/src/lib.rs`
- `crates/rustred-app/src/cli/mod.rs`
- `crates/rustred-app/src/cli/error.rs`
- `crates/rustred-app/src/cli/args.rs`
- `crates/rustred-app/tests/application_api.rs`

Code entry points for the later solver continuation are:

- `src/generated_affine_residual_case_mapped_nonzero.rs`
- `src/generated_affine_residual_case_bound_unit_equality_refinement.rs`
- `src/generated_affine_residual_group_exact_publication_epoch_owner.rs`
- `src/generated_affine_residual_group_exact_relation.rs`
- `src/generated_affine_residual_group_exact_session.rs`

Authoritative upstream/reference source trees are:

- `vendor/LiteRed2/Source/LiteRed2026.m`
- `vendor/symbolica`
- `vendor/gammaloop/crates/vakint`

## Final caution

The repository contains sophisticated and heavily tested components, but it is
not yet a complete LiteRed-like reducer. Keep capability claims tied to an
actual end-to-end acceptance test. A raw generated IBP row, a topology-specific
legacy recurrence, or a partial exact-session transition is not by itself a
closed parametric reduction system.
