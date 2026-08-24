# RustRed

RustRed is a pure-Rust implementation of the LiteRed approach to parametric
integration-by-parts (IBP) identities for Feynman integrals. Symbolica supplies
the exact symbolic algebra.

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
| Preserve symbolic nonzero conditions and replay evidence | Available in the library |
| Process a concrete numerator through tensor projection and scalar lowering | Available through the direct library path |
| Automatically reduce every arbitrary integral to masters | **Not yet complete** |
| Reproduce full LiteRed sector solving, rule publication, and exceptional-locus closure | **In progress** |

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
  sealed NoTarget, affine-equality-refinement, or Ready outcomes without
  publishing a rule or mutating solver state;
- one-loop scalar and tensor comparisons against frozen Vakint-derived oracles;
- concrete two-loop sunset and three-loop tetrahedron scalar/tensor fixtures;
- elementary factorized four- and five-loop fixtures which exercise the same
  loop-count-neutral library stack; and
- five numerator-cancellation closure pairs in
  [`tests/symbolica_target_numerator.rs`](tests/symbolica_target_numerator.rs):
  scalar denominator cancellation, squared cancellation, rank-two and
  rank-four tensor cancellation, and metric-contraction cancellation.

Those five closure pairs pass through compact numerator parsing, the generic
tensor projector, and scalar lowering. They currently use the direct library
path; `rustred derive` only retains their numerator metadata.

The multi-loop fixtures demonstrate concrete validated computations. They are
not proofs of complete symbolic coverage for all integer powers or all
topologies at those loop counts. Legacy authored finite-oracle modules are
feature-gated validation material, not production topology dispatch.

## Current blockers and roadmap

The transactional exact pivot database, hardest-first top reduction, and
session-owned exact recentering stages are implemented. The immediate remaining
solver work is the generic LiteRed-style continuation:

1. refine NoTarget and affine-equality outcomes into typed state transitions;
2. compile and close `WhenBad` exceptional branches;
3. feed solved subsectors into supersectors and iterate residual cases; and
4. publish replayable guarded rules and a complete reduction result.

After that generic path is joined end to end, the next non-vacuum validation
rung is an external scalar pentagon family. Once scalar reduction is certified,
the same family will exercise the external-momentum tensor projector. This
ordering keeps tensor validation from hiding unresolved scalar-sector gaps.

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
- [Full LiteRed scope specification](docs/research/litered_full_scope_spec.md)
- [LiteRed2 algorithm report](docs/research/litered2_algorithm_report.md)
- [Generic IBP parity audit](docs/research/generic_ibp_litered_parity_audit_2026-08-13.md)
- [Symbolica Rust API reference](docs/research/symbolica_rust_api_for_litered.md)
- [Vakint/alphaLoop tensor and IBP audit](docs/research/vakint_alphaloop_tensor_ibp_audit.md)
- [One- and two-loop validation audit](docs/research/one_two_loop_vacuum_validation_and_legacy_quarantine_2026-08-20.md)
- [Exact-group database design](docs/research/litered_solvej_exact_group_database.md)
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
