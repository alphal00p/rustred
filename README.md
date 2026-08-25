# RustRed

RustRed is a pure-Rust implementation of the LiteRed approach to parametric
integration-by-parts (IBP) identities for Feynman integrals. Symbolica already
supplies the main exact coefficient algebra and is the required target for all
remaining production algebra; the status and roadmap below identify the
pre-existing custom layers that still have to be migrated.

RustRed does not invoke FORM or a Mathematica kernel. The checked-in build uses
Symbolica's licensed GMP backend and deliberately does not enable `no_gmp`.

## Status

RustRed is an active implementation project, not yet a drop-in replacement for
all of LiteRed.

| Capability | Status |
|---|---|
| Parse compact integral-family descriptions | Available |
| Infer scalar parameters from family-defining expressions | Available |
| Derive generic ordinary parametric IBPs | Available through `rustred derive` and the library |
| Derive generic Lorentz-invariance identities | Available through `rustred derive` and the library |
| Preserve symbolic nonzero conditions and proof-component replay evidence | Available in the library |
| Chronologically replay committed generated-affine exact-session transitions | Available for the current exact-session slice |
| Process a concrete numerator through tensor projection and scalar lowering | Available through the direct library path |
| Automatically reduce every arbitrary integral to masters | **Not yet complete** |
| Reproduce full LiteRed sector solving, `WhenBad` closure, and rule publication | **In progress** |

The important distinction is that `derive` constructs universal, fully
parametric identities for the declared family. It does not choose masters or
claim that a concrete target has been reduced.

## What `derive` generates

For a complete affine denominator basis with `L` loop momenta and `E` external
momenta, the generic generator constructs:

- all `L(L+E)` ordinary total-derivative IBP identities;
- all `E(E-1)/2` Lorentz-invariance identities;
- coefficients over the authenticated Symbolica field `K(n)`;
- integer shift vectors for every integral term; and
- exceptional nonzero conditions with typed provenance.

There is no topology name, vacuum-only branch, or loop-count dispatch in this
derivation path. Concrete topologies are fixtures used to validate the generic
algorithms.

## Quick start

Rust 1.89 or newer is required. Clone the Symbolica source together with the
repository:

```bash
git clone --recurse-submodules https://github.com/alphal00p/rustred.git
cd rustred
```

The supplied Nix development shell includes Rust, GCC, `m4`, `pkg-config`,
Perl, and `cargo-nextest`:

```bash
nix develop
```

Export a valid Symbolica license, then build the GMP-enabled CLI:

```bash
export SYMBOLICA_LICENSE='your-symbolica-license'
export SYMBOLICA_HIDE_BANNER=1
cargo build --release --bin rustred
```

`SYMBOLICA_HIDE_BANNER` only keeps machine-readable CLI output clean; it does
not configure or replace the license.

RustRed itself is distributed under the [MIT license](LICENSE). Symbolica is a
separate dependency with its own licensing terms.

## Tested CLI derivation

[`examples/cli/one_loop.symbolica`](examples/cli/one_loop.symbolica) contains:

```text
I(
  name(tadpole),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1),
  numerator(sp(k,k))
)
```

Run the literal checked example from the repository root:

```bash
cargo run --quiet --bin rustred -- derive \
  --input examples/cli/one_loop.symbolica \
  --input-format symbolica \
  --relations ordinary
```

The tested output contains these fields:

```toml
schema = "rustred.derive-output.toml.v1"
status = "ok"
relation_selection = "ordinary"
target_disposition = "not_processed_by_derive"

[relation_counts]
generated_ordinary = 1
generated_li = 0
emitted_ordinary = 1
emitted_li = 0
emitted_total = 1

[[relations]]
stable_id = "ordinary-ibp:0:0"
```

That result proves that RustRed parsed the family, inferred `d` and `m2`, and
derived the one ordinary parametric identity expected for `L=1, E=0`. The
emitted relation has terms at shifts `[0]` and `[1]`, with coefficients that
remain symbolic in `d`, `m2`, and the abstract index `n0`.

In ordinary mathematical notation, the emitted identity is

```text
0 = (d - 2 n0) I(n0) - 2 m2 n0 I(n0 + 1)
```

It also proves a deliberate boundary: the powers and `sp(k,k)` numerator are
retained in `[target]`, but their disposition is
`not_processed_by_derive`. The CLI has derived an IBP; it has not claimed a
complete reduction of that target.

## Input modes

The CLI accepts three compact modes which normalize into the same authenticated
family representation:

- raw Symbolica `I(...)`, as above;
- hybrid TOML containing one compact `integral = """I(...)"""` plus metadata;
- fully explicit compact TOML for generated configurations.

Parameter declarations are optional when they can be inferred. They remain
useful as an explicit allowlist or for symbols used only by a retained target
numerator. Grammar, examples, output schema, exit codes, and atomic-I/O rules
are documented in [`docs/CLI.md`](docs/CLI.md).

## Direct library path

The Rust library exposes more of the work-in-progress pipeline than the CLI:

```text
family declaration
  -> affine basis authentication / optional ISP completion
  -> parametric IBP and LI generation
  -> sector, zero, symmetry, and exact-elimination certificates
  -> tensor projection and scalar-product lowering
  -> guarded application of whatever reduction coverage was proved
```

[`examples/generic_symbolica_tensor_ibp.rs`](examples/generic_symbolica_tensor_ibp.rs)
shows this direct path. Its APIs are still evolving and should not be confused
with a finished arbitrary-family reducer.

## Tested milestones

The repository currently includes tests for:

- generic ordinary IBP and LI generation with exact replay;
- raw Symbolica, hybrid TOML, and explicit TOML input normalization;
- parameter inference without a required `parameters(...)` clause;
- affine denominator-basis completion for independent short lists;
- zero-sector, symmetry, guarded specialization, and sparse-elimination proof
  components;
- an exact-GMP, session-owned `Solvej` recentering transaction: authenticated
  post-top-reduction leaders are matched against persisted targets and return
  sealed NoTarget, affine-equality-refinement, or Ready outcomes;
- typed NoTarget commit and affine-equality suspension: NoTarget commits the
  algebraic pivot while preserving every target, whereas an equality-bearing
  first target commits the pivot and seals the old solve epoch for refinement;
- an opaque, session-owned recipe for every committed production or synthetic
  source, with the exact dependent-reduction or new-pivot evidence retained by
  shared ownership rather than reconstructed from public metadata; production
  recipes admit the complete uniquely retained physical-row/re-elimination
  graph while excluding only pointer-proven shared plan/frame/inventory
  ancestry;
- an owning prepared-database commit whose fallible authentication, resource
  accounting, and allocation finish before its infallible commit tail;
- a private chronological event ledger for the generic generated-affine exact
  session, plus shadow replay which re-stages each opaque source and re-executes
  its dependent, NoTarget, or affine-equality transition before comparing the
  resulting evidence and terminal state;
- the first exact Ready-analysis checkpoint: for an authenticated independent
  affine cylinder it locates the unit pivot, proves every RHS strictly descends
  through the persisted arbitrary-precision physical-key ordering, and retains
  finite inactive-orthant hazard intervals without narrowing to machine
  integers; this checkpoint neither consumes a target nor publishes a rule;
- one-loop scalar and tensor comparisons against frozen Vakint-derived oracles,
  including an independent public-Symbolica rank-six (15 by 15) projector
  reconstruction;
- concrete two-loop sunset and three-loop tetrahedron scalar/tensor fixtures;
- elementary factorized four- and five-loop fixtures which exercise the same
  loop-count-neutral library stack;
- a fast test-only `L=6`, `K=21` coordinate-family gate proving the generic
  generator emits all 36 ordinary IBPs in deterministic row order with stable
  replay manifests; it does not claim sector coverage or reduction;
- a bounded authenticated residual-path cursor over the existing coverage
  MTBDD, with compact replay/resource tests and an ignored all-36 `K=21`
  stress oracle; the latter measures 49 normalized atoms, 268,427 retained
  nodes, and an exact 43-decision first Unsupported path, and is not a Ready or
  reduction result; and
- seven end-to-end numerator-spelling closure pairs in
  [`tests/one_loop_numerator_cancellation_closure.rs`](tests/one_loop_numerator_cancellation_closure.rs):
  scalar and squared denominator cancellation, rank-two/rank-four/rank-six
  tensor cancellation, metric-contraction cancellation, and traced-rank-six
  versus scalar-product spelling.

Those closure pairs independently rebuild generated parametric IBPs on both
sides and compare unreplaced-master output. They pass through compact numerator
parsing, the generic tensor projector, scalar lowering, and guarded rule
application. Projector coefficient powers and Gram algebra use public
Symbolica APIs; FORM is not invoked. They currently use the direct library
path; `rustred derive` only retains their numerator metadata.

The multi-loop fixtures demonstrate concrete validated computations. They are
not proofs of complete symbolic coverage for all integer powers or all
topologies at those loop counts. Legacy authored finite-oracle modules are
feature-gated validation material, not production topology dispatch.

## Current blockers and roadmap

The priority deployment is now a six-loop QCD beta-function campaign after
GammaLoop's general BPHZ R-operation.  RustRed will keep its full generic
LiteRed scope, but near-term work emphasizes single-scale massive vacuum
families with the common mass set to one.  The architecture is deliberately
two-stage: an offline, topology-generic foundry derives and verifies guarded
parametric rules for canonical families/sectors; a separate online runtime
applies the compiled artifacts in batches to the many concrete numerator
integrals produced by GammaLoop.  See the
[six-loop single-scale vacuum priority](docs/research/six_loop_single_scale_vacuum_priority_2026-08-24.md).

The transactional exact pivot database, hardest-first top reduction,
session-owned exact recentering, typed NoTarget commit, sealed affine-equality
suspension, private chronological event ledger, and shadow replay are
implemented for the generic generated-affine exact-session slice. Committed
events own opaque source recipes and exact evidence, and the owning prepared
database transition has an infallible commit tail after all fallible work has
completed.

The B0 algebra blocker has been removed. [`src/exact.rs`](src/exact.rs) now
keeps only a nominal RustRed wrapper around Symbolica's GMP `Rational`; scalar
arithmetic and exact matrix inverse, rank, determinant, multiplication, and
transpose cross the public `Q`/`Matrix<Q>` APIs. The old fixed-width rational,
gcd, Gaussian-elimination, and determinant implementations have been deleted.
Checked row-major conversion, allocation/shape admission, panic containment,
and the independent singularity guard remain RustRed-owned boundaries.

B0 is not the end of the Symbolica-first cleanup. Generated-affine composition,
strict polynomial-associate checks, generic-family matrices, automatic ISP
rank, tensor-projector matrices, and the affine-family/symmetry verifier have
since crossed public Symbolica APIs. The latter now delegates determinants,
transpose, Gram congruence, and denominator-coordinate products through an
authenticated V2 boundary while retaining an independent physics replay.
The next vacuum-critical solver milestone is the topology-neutral
`Ready -> condition/WhenBad partition -> atomic guarded-rule/residual
publication -> replay-certified closure` slice. Exact descent and lazy hazard
geometry are implemented for independent cylinders; the active slice must
also carry those proofs through general compact affine target maps. Graph-
lifted symmetry discovery and the `SparseRowReducer` transcript-equivalence
path follow on the scaling route; unrelated Feynman/non-vacuum algebra
migrations stay required but no longer displace the six-loop vacuum critical
path.

Alongside that staged algebra migration, the remaining generic LiteRed-style
solver work is to:

1. make authenticated normalized coverage the high-loop source authority and
   search only a requested residual frontier directly; use the full MTBDD only
   when a measured construction budget makes it appropriate;
2. compile and close `WhenBad` exceptional branches;
3. atomically publish guarded rules and residual work;
4. feed solved subsectors into supersectors and iterate residual cases; and
5. expose a replay-certified complete reduction result.

This checkpoint is not a complete LiteRed port, complete `WhenBad` or rule
publication, an arbitrary one-loop pentagon reduction, or completion of the
two- through six-loop reduction campaign. Concrete multi-loop families
currently serve only as bounded validation fixtures. Non-vacuum pentagon work
remains in scope but is behind the vacuum rule-foundry and batch-application
critical path.

A genuine all-inactive `K=21` probe first exposed the legacy Boolean-cover cap
at split 65,537 of 65,536. The replacement MTBDD avoids that explicit
`2^K` partition, and its new cursor avoids flattening all terminal paths, but
the real all-36 source still constructs 49 atoms and 268,427 nodes before the
cursor can return its first residual. The cap is not being raised: the next
high-loop gate is a direct, bounded search over the authenticated normalized
formulas, followed by the existing exact Ready analysis. No arity-21 case has
reached Ready yet.

Further LiteRed parity includes broader symmetry discovery, partial fractions
for dependent or overcomplete propagator lists, master inference, persistent
proof serialization, dimension shifts, and differential equations.

## Testing

Run the licensed test suite in parallel with the bounded default of four jobs:

```bash
export SYMBOLICA_LICENSE='your-symbolica-license'
./scripts/test.sh
```

Override concurrency with `RUSTRED_TEST_JOBS`. When `cargo-nextest` is
available, the script runs test binaries concurrently; it otherwise keeps
Cargo's parallel test workers. No test path enables `no_gmp`.

## Documentation

- [CLI contract and input formats](docs/CLI.md)
- [RustRed scope and acceptance criteria](docs/research/rustred_scope_and_acceptance.md)
- [Six-loop single-scale vacuum priority](docs/research/six_loop_single_scale_vacuum_priority_2026-08-24.md)
- [Full LiteRed scope specification](docs/research/litered_full_scope_spec.md)
- [LiteRed2 algorithm report](docs/research/litered2_algorithm_report.md)
- [Generic IBP parity audit](docs/research/generic_ibp_litered_parity_audit_2026-08-13.md)
- [Symbolica Rust API reference](docs/research/symbolica_rust_api_for_litered.md)
- [Symbolica exact linear-algebra API inventory](docs/research/symbolica_exact_linear_algebra_api_inventory.md)
- [Symbolica-first algebra migration audit](docs/research/symbolica_first_algebra_migration_audit_2026-08-24.md)
- [Symbolica upstream correctness and embedding-gap audit](docs/research/symbolica_upstream_gap_audit_2026-08-25.md)
- [Vakint/alphaLoop tensor and IBP audit](docs/research/vakint_alphaloop_tensor_ibp_audit.md)
- [One- and two-loop validation audit](docs/research/one_two_loop_vacuum_validation_and_legacy_quarantine_2026-08-20.md)
- [Exact-group database design](docs/research/litered_solvej_exact_group_database.md)
- [Exact-session `WhenBad` and publication port plan](docs/research/exact_session_when_bad_port_plan_2026-08-24.md)
- [Residual recentering design](docs/research/litered_solvej_residual_recentering_2026-08-13.md)
- [Latest persistent quotient checkpoint](docs/research/persistent_cylindrical_numeric_quotient_checkpoint_2026-08-20.md)

## Contributing

Contributions should keep production algorithms topology-independent, retain
exact replay evidence, and add concrete topologies only as validation inputs.
Please run formatting, checks, and the relevant parallel tests before opening a
change.

## License

RustRed is available under the [MIT License](LICENSE), copyright RustRed
contributors.
