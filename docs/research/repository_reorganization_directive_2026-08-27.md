# RustRed repository reorganization directive

Date: 2026-08-27

Status: active mandatory architecture gate. The multi-agent inventory,
independent design audit, bounded application/CLI extraction,
transport-neutral boundary refactor, and development Python adapter are
complete. The physical legacy-oracle package extraction is also complete;
the three orphan-source decisions are complete; deeper topology-neutral core,
test-support, and documentation migrations remain pending.

## Decision

Feature growth stops after the in-flight transient mapped-`NonZero` core is
validated, documented, committed, and pushed. Before committed-resident
equality refinement, child-session regeneration, or further loop milestones,
RustRed must undergo a deep structural reorganization.

The long-term mathematical objective is unchanged: a generic, pure-Rust,
Symbolica-native LiteRed-like engine with no FORM and no topology-authored
recurrences. This gate changes the route to that objective. A production
engine whose boundaries cannot be understood or tested independently is not a
suitable base for five- and six-loop campaigns.

## Audited package decision and current progress

The reconciled target is a deliberately small four-package workspace:

```text
rustred-python ------> rustred-app ------> rustred core
                            |
                            +-- rustred CLI binary

rustred-legacy-oracles ------------------> rustred core
```

The root `rustred` package remains the topology-neutral mathematical core.
`rustred-app` now owns the transport-neutral typed requests/results, semantic
services, resource limits, stable errors/options, canonical TOML
serialization, and the CLI binary. Keeping the binary in this package avoids a transport-only
microcrate. The application/CLI boundary is complete: OS arguments, paths,
stdin/stdout, overwrite policy, exit codes, help, and terminal diagnostics are
confined to the adapter. Public calls document their panic-safety contract.
Panic containment belongs at the outer coordinator/FFI boundary and must poison
further work rather than claim that an invariant failure is safely recoverable.
`rustred-python` now implements that boundary. The publish-disabled
`rustred-legacy-oracles` package now owns all 35 compiled authored modules, 34
dedicated integration tests, and four diagnostic examples. It is excluded
from the default workspace members and depends only on the core with default
features disabled plus the narrow hidden `legacy-oracle-support` facade. The
former `legacy-authored-oracles` feature and root re-exports no longer exist.
Test support remains adjacent to the code it validates unless a later measured
dependency boundary justifies another package.

The first migration was intentionally mechanical; the subsequent boundary
milestone moved normalization, lowering, derivation/output, and campaign
services under `application`, split application and CLI errors, and removed the
app's direct Symbolica dependency through a narrow core facade. A direct
contract suite checks API/CLI canonical-byte parity. The subsequent dedicated
Python package uses exactly that application boundary, adds a process-wide
poison-on-panic coordinator, and preserves canonical bytes without a second
semantic path. No solver algorithm, topology dispatch, or authored recurrence
was added in either frontend phase.

### Resolved orphan sources

The Phase 0 reachability audit found three tracked source files that had never
been declared in `lib.rs`, compiled by Cargo, referenced by a caller, or
covered by an executable test. `five_loop_d4.rs` was a truncated authored
five-loop banana shell with missing helpers and builder stages;
`four_loop_next_conditions.rs` was only a fixed-topology schema/error skeleton
without an inventory builder or replay implementation. Neither was a usable
oracle, so both were deleted rather than moved into
`rustred-legacy-oracles`. Their unauthenticated historical claims remain
recoverable from Git and are not capability evidence.

`exact_sparse_provenance.rs` was likewise deleted rather than wired. Its
optional flattened weights duplicated the recursive provenance already
retained and replayed by `ExactSparseElimination`, and it introduced a second
handwritten forward-elimination, sparse-matrix, arithmetic-metering, checksum,
and full-replay layer on the older concrete coefficient path. If explicit
source weights or left-kernel roots become necessary, they must be built at
the live Symbolica transcript boundary from the retained `L`, `U`, pivot,
dependent-row, and normalization data. No compatibility shim, tombstone, or
archive package replaces any of these files.

## Problems to audit

The core repository still has a very large flat `src/` namespace. After the
completed application/CLI separation, it still mixes:

- topology- and loop-neutral algebra, family, IBP, sector, and rule kernels;
- exact-session and exceptional-closure campaign orchestration;
- concrete one- through five-loop validation/oracle code, now isolated in the
  publish-disabled `rustred-legacy-oracles` package;
- differential bridges and test-only campaign machinery; and
- a long research-document history whose current authority is not always
  apparent from its filename.

The audit must determine actual reachability and ownership. A loop-count name
is a strong signal that a file does not belong in the generic production
engine, but it is not by itself proof that the file is unused or safe to
delete. Conversely, feature-gating a stale authored recurrence does not make
it part of the desired RustRed architecture.

## Required parallel research lanes

Use multiple independent agents and reconcile their results before moving
files:

1. **Production dependency graph.** Classify every Rust module by public API,
   inbound/outbound dependencies, feature gates, binary/library reachability,
   and topology/loop neutrality.
2. **Test and oracle inventory.** Identify concrete topology fixtures,
   differential bridges, acceptance campaigns, benchmarks, and authored
   recurrence oracles. Decide which belong under integration tests, test-only
   support crates, archived external fixtures, or deletion.
3. **Workspace-boundary design.** Compare a disciplined module tree with a
   small Cargo workspace. Candidate units must have acyclic dependency
   direction, explicit public surfaces, and measurable compile/test costs;
   subcrates are not an end in themselves.
4. **Documentation authority audit.** Mark each document as current normative
   design, current implementation note, acceptance evidence, superseded
   history, or stale. Consolidate current guidance and delete stale material;
   do not create an ever-growing archive merely to avoid decisions.
5. **API and naming audit.** Find duplicated versioned types, obsolete V1/V2
   bridges, overlong generated-affine names, hidden topology assumptions, and
   internal APIs exposed only because of the flat tree.
6. **Frontend and packaging audit.** Extract the transport-neutral operations
   currently trapped in CLI-private modules; design one typed application API
   used by both the CLI and a dedicated PyO3 package, including deterministic
   serialization, error parity, Python packaging, GIL release, Symbolica
   license/thread constraints, and `n_cores` behavior.
7. **Migration and validation audit.** Design mechanical phases with a test
   baseline, import-boundary checks, no semantic rewrite mixed into file moves,
   and a rollback-sized commit for each phase.

At least one agent must independently challenge the proposed target structure
and deletion list. Research agents may inspect and report, but the root agent
owns the final dependency interpretation and migration decisions.

## Target-structure requirements

The audited design, whatever exact module/subcrate split it selects, must
enforce these properties:

- The production derivation/reduction engine is topology- and loop-neutral.
  It accepts families, sectors, expressions, and policies as data; it never
  dispatches on names such as one-loop, sunset, three-loop, or vacuum family.
- Concrete loop-count topologies live only in tests, examples, benchmarks, or
  explicitly historical oracle support. Authored recurrences cannot be linked
  into the default production library.
- Symbolica algebra adapters are centralized and reusable. Semantic layers may
  own LiteRed ordering, proof, provenance, scheduling, and resource policies,
  but not duplicate CAS operations.
- Tensor parsing/reduction, parametric IBP/LI generation, sector geometry,
  exact solving/closure, campaign execution, persistence/publication, and CLI
  I/O have explicit dependency directions and independently testable APIs.
- CLI and Python are adapters over one owned, typed Rust application layer.
  PyO3 types, Python callbacks, path/stdin handling, exit codes, and GIL state
  cannot enter the mathematical core. Python must expose the same semantic
  operations and `n_cores` control, and its canonical TOML must be
  byte-identical to the shared serializer used by the CLI.
- Test campaigns do not inflate or obscure the production namespace. Shared
  fixtures have a deliberate test-support home and cannot be imported by
  production modules.
- Current documentation has a small discoverable index and one authoritative
  statement per active design. Superseded or false documents and sources are
  removed once their remaining evidence has been incorporated where needed.
- No backward-compatibility layer is required during this pre-release phase.
  Compatibility shims must justify a present validation or migration need and
  carry a deletion point.

## Deletion policy

Deletion is expected, but it must be evidence-based and recoverable through
Git history. A source or document may be removed when the audited map proves
that it is unreachable from the desired default product, duplicates current
coverage, encodes an authored/topology-specific production path, or states a
superseded design whose surviving facts have been consolidated elsewhere.

Before deletion, record:

- current build/test/feature reachability;
- any unique oracle result, fixture, or rationale worth retaining;
- the replacement test or document, if one is needed; and
- the exact migration phase that removes it.

Do not preserve stale files inside an `archive/` directory by default. Git is
the archive. Vendored upstream sources such as LiteRed2 and Symbolica have a
separate provenance role and are not treated as RustRed-owned stale code.

## Migration gates

1. **Complete:** freeze and push the mapped-`NonZero` checkpoint.
2. **Complete:** capture default-GMP build, focused, and parallel test baselines.
3. **Complete:** produce the classified file/dependency/document inventory.
4. **Complete:** publish and reconcile a target tree, package dependency diagram, move
   map, deletion list, and risk/test matrix.
5. **Complete for the package decision:** obtain an independent adversarial
   audit and reconcile its structural blockers.
6. **In progress:** execute mechanical moves and visibility tightening in
   small commits, with parallel tests after each phase and milestone pushes.
   The `rustred-app` extraction and transport-boundary phases are complete;
   the physical legacy-oracle extraction and orphan-source deletion are
   complete; deeper core/test separation remains.
7. **Complete for development use:** add the PyO3 package only after the shared
   application boundary exists; prove CLI/application/Python parity, licensed
   parallel execution, safe Python-thread coordination, and wheel/sdist
   installation without enabling Symbolica's `no_gmp` feature. Public package
   distribution remains gated on third-party redistribution review and a
   reproducible manylinux build.
8. Delete reconciled stale code/docs and re-run default plus applicable legacy
   oracle gates separately.
9. Resume feature work only after the generic engine and test campaigns are
   visibly separated and the README/design index match the actual tree.

This gate is architectural work, not evidence of a new reduction capability.
