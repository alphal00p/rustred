# RustRed

RustRed is a pre-alpha, pure-Rust and Symbolica-native project for deriving and
eventually applying parametric integration-by-parts identities. Its scientific
pressure target is a generic, deterministic rule foundry capable of closing
single-scale vacuum families through six loops. Loop count and topology are
input data, never production dispatch keys.

## Current capability

The currently evidenced core can:

- compile compact, structured-text, and caller-owned Symbolica Atom family
  descriptions, authenticating every form at ingress;
- build exact topology-neutral affine integral families;
- generate the complete ordinary parametric IBP and LI source rows;
- exactly replay physical family rows/common-scale claims, retain auxiliary
  roles, and structurally validate caller-attested presentation metadata;
- project scalar, odd-rank, and rank-two vacuum tensors and separately lower
  explicit scalar products onto family integral keys;
- derive both a guarded rule at one concrete anchor and a genuine fixed-sector
  parametric recurrence over `K(n)`, accepting the latter only after exact
  symbolic replay, uniform descent proofs, retained nonzero guards, and
  independent anchored agreement, with an optional requested-pivot path that
  performs deterministic Symbolica RREF and retains every reachable pivot
  guard;
- refine sector-monotone RHS shifts into lazy exact fixed-target-sector cells
  and stream preflighted O(1), rule-bound proper-subsector obligation
  descriptors with stable process-local resume and on-demand domain
  materialization;
- verify explicit affine symmetry maps;
- analyze requested zero sectors using generic Symanzik/rank evidence; and
- provide deterministic core-owned campaign execution and memory-preflight
  primitives, with roots-only composition in the application layer.

It does **not** yet refine coefficient/guard applicability on dependency cells,
feed back or close proper subsectors or whole families, publish reusable rule
artifacts, apply IBP reductions, substitute masters, or support
generic/higher-even-rank tensor reduction. The current API also accepts a
caller-supplied source span rather than certifying a fresh complete source set.
Structural source counts—even the six-loop count—are not closure evidence.

The codebase has no RustRed backward-compatibility promise during deep
development. Obsolete prototype solvers, schemas, compatibility facades,
authored recurrences, and milestone-log architecture have been deleted rather
than migrated.

## Workspace

The repository root is a virtual Cargo workspace with three packages:

- `crates/rustred-core` is package and library `rustred`; it owns exact
  algebra, families, normalized input, identities, sectors, tensor and foundry
  services, and generic campaign primitives.
- `crates/rustred-app` owns shared application operations and the `rustred`
  CLI. Transport schemas and presentation stay here rather than in the
  mathematical core.
- `crates/rustred-python` is a thin PyO3 adapter over `rustred-app`. Python
  users write `import rustred`; `rustred._rustred` is a private extension
  detail, and top-level `import _rustred` is intentionally unsupported.

The exact registry-shaped Symbolica 2.2.0 dependency is patched to
`vendor/symbolica` and built with GMP. Symbolica is the sole production CAS.
RustRed never invokes FORM, Mathematica, SymPy, or authored recurrence tables.

## Development

Use the pinned Nix environment. Licensed or multicore Symbolica operations
need `SYMBOLICA_LICENSE` set before the first Symbolica object or worker pool is
created.

```bash
nix develop --command cargo fmt --all -- --check
SYMBOLICA_LICENSE=... nix develop --command cargo check --workspace --all-targets
SYMBOLICA_LICENSE=... nix develop --command cargo test --workspace --all-targets
```

Inspect the current CLI contract with:

```bash
nix develop --command cargo run -p rustred-app --bin rustred -- --help
```

The root `pyproject.toml` builds the Python distribution. The public smoke test
is:

```bash
uv venv .venv
source .venv/bin/activate
maturin develop --features extension-module
python -c 'import rustred'
```

## Two-loop parametric-IBP examples

The [`examples/`](examples/) tree contains complete, runnable versions of the
same calculation through the Rust library, CLI, and Python APIs. They define
the equal-mass two-loop vacuum (sunset) family

```text
D1 = k1^2 - m2
D2 = k2^2 - m2
D3 = (k1 + k2)^2 - m2
```

with symbolic dimension `d`, no external momenta, and common squared mass
`m2`. Because `L = 2` and `E = 0`, RustRed must generate exactly
`L * (L + E) = 4` ordinary parametric IBP sources and no LI sources. Their
stable IDs are:

```text
ordinary-ibp:0:0
ordinary-ibp:0:1
ordinary-ibp:1:0
ordinary-ibp:1:1
```

These are the complete universal source identities for this family, with
equation convention `sum(coefficient * I(n + shift)) = 0`. They are not yet a
closed sector-reduction table. For the scale-free presentation used by the
single-scale vacuum program, `m2` may subsequently be specialized to `1`.

The commands below are for a Git checkout. Run them from the repository root
inside `nix develop` (or an equivalent environment).

### Rust library

[`examples/rust/two_loop_single_mass_vacuum.rs`](examples/rust/two_loop_single_mass_vacuum.rs)
uses the public `rustred` crate directly: it compiles and lowers the family,
prepares all ordinary rows, completes the batch, checks the four stable IDs,
and renders the equations.

```bash
cargo run --locked -p rustred-app --example two-loop-single-mass-vacuum
```

After any Cargo build messages, the expected program output is:

```text
# sum(coefficient * I(n + shift)) = 0
ordinary-ibp:0:0: (-n2) * I(n0-1,n1,n2+1) + (n2) * I(n0,n1-1,n2+1) + (d-2*n0-n2) * I(n0,n1,n2) + (-m2*n2) * I(n0,n1,n2+1) + (-2*m2*n0) * I(n0+1,n1,n2) = 0
ordinary-ibp:0:1: (-n2) * I(n0-1,n1,n2+1) + (n1) * I(n0-1,n1+1,n2) + (n2) * I(n0,n1-1,n2+1) + (n1-n2) * I(n0,n1,n2) + (-m2*n2) * I(n0,n1,n2+1) + (-n1) * I(n0,n1+1,n2-1) + (m2*n1) * I(n0,n1+1,n2) = 0
ordinary-ibp:1:0: (n2) * I(n0-1,n1,n2+1) + (-n2) * I(n0,n1-1,n2+1) + (n0-n2) * I(n0,n1,n2) + (-m2*n2) * I(n0,n1,n2+1) + (n0) * I(n0+1,n1-1,n2) + (-n0) * I(n0+1,n1,n2-1) + (m2*n0) * I(n0+1,n1,n2) = 0
ordinary-ibp:1:1: (n2) * I(n0-1,n1,n2+1) + (-n2) * I(n0,n1-1,n2+1) + (d-2*n1-n2) * I(n0,n1,n2) + (-m2*n2) * I(n0,n1,n2+1) + (-2*m2*n1) * I(n0,n1+1,n2) = 0
```

### CLI

[`examples/cli/two_loop_single_mass_vacuum.symbolica`](examples/cli/two_loop_single_mass_vacuum.symbolica)
is the family input. The portable runner resolves the checkout independently
of the caller's working directory and requests ordinary rows explicitly:

```bash
sh examples/cli/run.sh
```

The CLI prints a canonical `rustred.derive-output.toml.v1` document. It is
verbose because it includes exact family/context fingerprints and every sparse
term; the defining expected fields are:

```toml
schema = "rustred.derive-output.toml.v1"
status = "ok"
relation_selection = "ordinary"

[family]
name = "equal_mass_sunset"
loop_momenta = ["k1", "k2"]
external_momenta = []
denominator_count = 3
index_symbols = ["n0", "n1", "n2"]

[relation_counts]
generated_ordinary = 4
generated_li = 0
emitted_ordinary = 4
emitted_li = 0
emitted_total = 4
```

The four `[[relations]]` records have the stable IDs listed above, in that
order. The CLI integration test pins this input and those counts/IDs.

### Python

[`examples/python/two_loop_single_mass_vacuum.py`](examples/python/two_loop_single_mass_vacuum.py)
uses the public package name `import rustred`, embeds the same family, checks
the output schema, counts, and row IDs, then prints the canonical TOML.

```bash
uv venv .venv
. .venv/bin/activate
maturin develop --features extension-module
python examples/python/two_loop_single_mass_vacuum.py
```

Its expected output is byte-identical to the CLI's canonical TOML for the same
build. Any schema, count, or stable-ID mismatch makes the script fail before
printing.

## Vakint integration

Vakint development occurs in the independent GammaLoop repository on branch
`vakint_rustred`. Its additive `RustRed` tensor mode now reuses Vakint's
matcher/canonical routing and calls RustRed's key-aware projector for
registered common-mass vacuum families across loop counts through rank two.
Pinched families receive auxiliary-ISP completion, and explicit multi-loop
routings must replay exactly through the matcher's complete simultaneous
basis witness. Scalar numerators pass through, odd ranks vanish, exact
symbolic or numeric masses and integer powers are retained, and both Vakint
output notations are covered. Existing behavior still defaults to FORM;
unsupported RustRed inputs fail with typed errors and never invoke or fall
back to FORM.

The RustRed crate now owns the first scalar/odd/rank-two single-scale-vacuum
service; Vakint already reuses its projection boundary, while CLI and Python
tensor adapters remain to be added.
Vakint remains responsible for topology matching, canonical routing, steering,
and presentation. Existing FORM-backed Vakint paths and their compatibility
tests remain reference oracles; the RustRed mode does not invoke or fall back
to them.

## Documentation

[`GOAL.md`](GOAL.md) is the authoritative objective and execution roadmap.
Stable design documents are:

- [architecture and ownership](docs/architecture.md);
- [Symbolica and exact algebra](docs/algebra.md);
- [tensor reduction and Vakint integration](docs/tensor.md);
- [closing-rule foundry target](docs/foundry.md);
- [application, Python, and Vakint interfaces](docs/interfaces.md);
- [validation and oracle ladder](docs/validation.md);
- [LiteRed2 semantic reference](docs/references/litered2.md); and
- [current CLI contract](docs/CLI.md).

Local LiteRed2, GammaLoop/Vakint, FORM, and other reference checkouts live only
under ignored `FOR_REFERENCE_ONLY_DO_NOT_PUSH/`. They must never enter RustRed
history. GammaLoop inside that tree is a separate Git repository.

RustRed is licensed under the [MIT License](LICENSE).
