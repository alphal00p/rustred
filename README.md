# RustRed

RustRed is a pre-alpha, pure-Rust and Symbolica-native project for deriving and
applying parametric integration-by-parts identities. Its active Stage 1 target
is a generic, deterministic rule foundry that closes every single-scale vacuum
family represented in Vakint through three loops, publishes the resulting
one-off artifacts, and applies them through a FORM-free scalar Vakint backend.
Loop count and topology are input data, never production dispatch keys.

## Current capability

The currently evidenced core can:

- compile compact, structured-text, and caller-owned Symbolica Atom family
  descriptions, authenticating every form at ingress;
- build exact topology-neutral affine integral families;
- generate the complete ordinary parametric IBP and LI source rows;
- exactly replay physical family rows/common-scale claims, retain auxiliary
  roles, and structurally validate caller-attested presentation metadata;
- project scalar, odd-rank, and rank-two vacuum tensors and, through a
  separate post-projection service, lower polynomial loop--loop scalar
  numerators onto shifted family keys with explicit common-mass powers;
- derive both a guarded rule at one concrete anchor and a genuine fixed-sector
  parametric recurrence over `K(n)`, accepting the latter only after exact
  symbolic replay, uniform descent proofs, retained nonzero guards, and
  independent anchored agreement, with an optional requested-pivot path that
  performs deterministic Symbolica RREF over physical pivots, keeps
  chronological provenance columns as free source weights, and retains every
  reachable pivot guard;
- refine sector-monotone RHS shifts into lazy exact fixed-target-sector cells
  and stream preflighted O(1), rule-bound proper-subsector obligation
  descriptors with stable process-local resume and on-demand domain
  materialization;
- verify explicit affine symmetry maps;
- analyze requested zero sectors using generic Symanzik/rank evidence; and
- provide deterministic core-owned campaign execution and memory-preflight
  primitives, with roots-only composition in the application layer;
- freshly generate, exactly replay, and seal the complete canonical one-loop
  `K = 1` and equal-mass two-loop sunset `K = 3` vacuum partitions over
  `Q(d)`, including explicit masters, scaleless zero terminals, exact `S3`
  routing, pinched-face factorization, exceptional numerator-corner cells,
  and checked common-mass homogeneity; and
- apply a sealed artifact with the topology-independent `reduction::Reducer`,
  using deterministic first-applicable rule selection, concrete strict-descent
  checks, an explicit work stack, memoization with retained-payload limits,
  like-master collection, typed uncovered/cycle failures, and optional common-
  mass restoration; and
- deterministically encode both sealed artifacts, authenticate and replay them
  once when loading untrusted bytes, and expose durable generation,
  inspection, and exact reduction through the Rust application API, the
  `campaign` CLI, and `import rustred` Python API; and
- drive Vakint's opt-in `EvaluationMethod::RustRed` scalar backend through the
  shipped `K = 1` and `K = 3` artifacts, reusing Vakint's existing matcher and
  routing witness, returning exact MATAD-basis master coefficients, restoring
  a symbolic or exact common mass, and optionally applying Vakint's pure-Rust
  master values without invoking or falling back to FORM.

It does **not** yet close the three-loop `K = 6` family or support
generic/higher-even-rank tensor reduction. RustRed itself deliberately does not
own evaluated master values; the Vakint adapter can substitute Vakint's
existing values after reduction. Structural source counts—at any loop
count—remain insufficient closure evidence.

## Active development stage

Stage 1 produces sector-complete unit-mass artifacts for the one-loop
`K = 1`, two-loop `K = 3`, and three-loop `K = 6` vacuum families. Together
they must cover Vakint's eight registered graph classes through three loops:
the tadpole, the sunset and its pinch, and the K4/Mercedes parent with four
inequivalent contractions. Their complete ordinary-source counts are 1, 4,
and 9 respectively.

The `K = 1` and `K = 3` families are installed as mathematically closed,
deterministically encoded artifacts and consumed by the generic recursive
reducer through Rust, CLI, and Python surfaces. They are also shipped with and
consumed by Vakint's FORM-free scalar backend. Three-loop `K = 6` closure and
extension of that backend across the five registered three-loop graph classes
are the remaining active Stage 1 work. A test-only K6 pressure fixture already
pins its exact family, nine sources, order-24 `S4` sector partition, the five
revision-stamped Vakint class/routing snapshots, and certified `K3 x K1` plus
both inequivalent `K1 x K1 x K1` factorization sectors. It also derives the
first exact top-sector rule cell and the two inequivalent positive dotted-edge
cells on the canonical five-line residual face. Each cell is projected from
all nine sources and retains exact residual replay, guards, bounded application
proof, provenance, and strict descent. On the irreducible four-line face, an
exact target-aligned translation supplies a guarded canonical-dot multi-excess
cell, while the untranslated span supplies the canonical mixed
numerator/dot cell, including its isolated mixed corner. Exact fixed-corner
projections additionally lower the isolated pure-dot orbit to the scalar
corner and supply a strict-descent recurrence for the opposite two-dot orbit
from the complete nine-row one-dot translated source layer, with exact RREF
selecting five rows. A topology-neutral bounded same-sector search now grows
complete L1 translation diamonds deterministically; its first successful
depth-two cone contains 28 translations and all 252 translated ordinary rows.
Exact targeted RREF selects 16 of those rows and supplies a guarded two-term,
strictly descending recurrence for the inequivalent adjacent two-dot orbit,
while complete provenance and projection replay retain the full search span.
The resulting fixed-corner cells route all three dotted orbits under the exact
`S4` action. Their descendants, deeper dot/numerator faces, and the rest of the
fixed point remain open.

Live matcher comparison remains an integration gate, and no artifact is
published before the complete rule fixed point closes.

Tensor reduction is explicitly outside Stage 1. Vakint retains its existing
FORM tensor prepass, while the new RustRed evaluation backend is FORM-free
from scalar IBP application through master substitution. Existing experimental
RustRed rank-two tensor code remains frozen. Four- through six-loop closure,
high-loop performance work, and new tensor technology are deferred until
explicit new guidance. See [`GOAL.md`](GOAL.md) for the authoritative gates.

The codebase has no RustRed backward-compatibility promise during deep
development. Obsolete prototype solvers, schemas, compatibility facades,
authored recurrences, and milestone-log architecture have been deleted rather
than migrated.

## Workspace

The repository root is a virtual Cargo workspace with three packages:

- `crates/rustred-core` is package and library `rustred`; it owns exact
  algebra, families, normalized input, identities, sectors, tensor and foundry
  services, sealed artifacts, deterministic reduction, and generic campaign
  primitives.
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

## Durable closing artifacts

The semantic generation selectors are `unit-mass-vacuum-k1` for the canonical
one-loop family and `unit-mass-vacuum-k3` for the equal-mass two-loop sunset.
They are family selectors, not Vakint topology names. Generate deterministic
binary bytes, then inspect or apply those exact bytes:

```bash
rustred campaign generate \
  --family unit-mass-vacuum-k1 \
  --output one_loop.rr

rustred campaign inspect \
  --artifact one_loop.rr \
  --output one_loop.inspect.toml

rustred campaign reduce \
  --artifact one_loop.rr \
  --powers 3 \
  --output one_loop.I3.toml

rustred campaign generate \
  --family unit-mass-vacuum-k3 \
  --output two_loop_sunset.rr

rustred campaign reduce \
  --artifact two_loop_sunset.rr \
  --powers 2,2,1 \
  --output two_loop_sunset.I221.toml
```

Artifact input and output also support standard streams. For example, this
inspects freshly generated bytes without a file:

```bash
rustred campaign generate --family unit-mass-vacuum-k1 \
  | rustred campaign inspect --artifact -
```

For target `I(3)`, reduction returns the typed master key `[1]`, exact
unit-mass coefficient
`(-6*rustred::{}::d+8+rustred::{}::d^2)*1/8`, and
`common_mass_squared_power = "-2"`; hence the complete coefficient is the
reported exact coefficient times `(mass_squared)^(-2)`. The power is a decimal
string in TOML because the generic homogeneity API uses a signed 128-bit
exponent while TOML integers are restricted to signed 64-bit values.

The Python surface uses immutable `bytes` directly:

```python
import rustred

generated = rustred.generate_closing_artifact(
    family=rustred.ClosingFamily.UNIT_MASS_VACUUM_K1,
)
artifact = generated.artifact
inspection = rustred.inspect_closing_artifact(artifact)
reduction = rustred.reduce_with_closing_artifact(artifact, [3])

term = reduction.terms[0]
assert term.master_powers == [1]
assert term.common_mass_squared_power == -2
```

Loading authenticates and exactly replays untrusted artifact bytes once;
recursive application then uses the sealed owner without repeating cold-load
authentication in the hot path. The `K = 6` artifact remains Stage 1 work.

## Two-loop parametric-IBP examples

The [`examples/`](examples/) tree contains complete, runnable versions of the
same closing-artifact calculation through the Rust library, CLI, and Python
APIs. The registered `K = 3` family is the unit-mass presentation of

```text
D1 = k1^2 - 1
D2 = k2^2 - 1
D3 = (k1 + k2)^2 - 1.
```

RustRed generates all four ordinary sources, derives five guarded rule cells,
proves exact `S3` routing and four scaleless zero sectors, and factorizes the
two-line face through the immutable `K = 1` artifact. The two explicit masters
are `I(1,1,1)` and `I(0,1,1)`. An arbitrary common `m^2` is restored after
unit-mass reduction by the reported homogeneity power.

The Rust example calls the public `rustred` crate directly:

```bash
cargo run --locked -p rustred-app --example two-loop-single-mass-vacuum
```

Its defining output is:

```text
algorithm = rustred.generated.two-loop-unit-mass-sunset.v1
ordinary_sources = 4
closing_rule_cells = 5
source = ordinary-ibp:0:0
source = ordinary-ibp:0:1
source = ordinary-ibp:1:0
source = ordinary-ibp:1:1
target = [2, 2, 1]
master [0, 1, 1]: ... mass_squared_power = -3
master [1, 1, 1]: ... mass_squared_power = -2
```

The CLI runner generates durable bytes, authenticates them, and reduces
`I(2,2,1)`:

```bash
sh examples/cli/run.sh
```

Its inspection TOML reports:

```toml
[artifact]
algorithm_id = "rustred.generated.two-loop-unit-mass-sunset.v1"
arity = 3

[validation]
source_rows = 4
guarded_rules = 5
master_terminals = 2
zero_sector_terminals = 4
```

[`examples/python/two_loop_single_mass_vacuum.py`](examples/python/two_loop_single_mass_vacuum.py)
uses the public package name `import rustred`, checks the complete five-cell
artifact, and verifies both exact master keys and mass powers:

```bash
uv venv .venv
. .venv/bin/activate
maturin develop --features extension-module
python examples/python/two_loop_single_mass_vacuum.py
```

## Vakint integration

Vakint development occurs in the independent GammaLoop repository on branch
`vakint_rustred`. The opt-in scalar API boundary
`EvaluationMethod::RustRed(RustRedEvaluationOptions)` and
`EvaluationOrder::rustred_only()` now support the one-loop tadpole, two-loop
sunset, and its pinch. The backend consumes Vakint's existing topology match
and simultaneous routing witness, applies shipped RustRed artifacts, returns
exact coefficients in Vakint's existing MATAD master basis, and optionally
reuses its pure-Rust master evaluations. It does not rematch graphs, regenerate
artifacts, invoke FORM, or fall back to another scalar reducer.

Vakint's defaults and backward compatibility remain unchanged. Tensor-bearing
inputs continue through the existing FORM tensor prepass before the FORM-free
RustRed scalar tail, so that whole tensor-bearing chain is not described as
FORM-free. Dedicated invalid-FORM-path tests exercise nontrivial one- and
two-loop scalar numerators and prove that the scalar backend itself has no FORM
dependency. Broad raw-master and substituted-result tests agree with MATAD. The
previously implemented optional, bounded `TensorReductionMode::RustRed`
experiment remains frozen and is not an active Stage 1 dependency.

Production `K = 1` and `K = 3` artifacts are generated once, checked into and
shipped with Vakint, and loaded rather than rediscovered during evaluation.
They are validated once at lazy load and reused thereafter. RustRed owns
guarded rule application and typed master keys; Vakint owns topology matching,
canonical routing, steering, normalization, presentation, and its existing
master values.

## Documentation

[`GOAL.md`](GOAL.md) is the authoritative objective and execution roadmap.
Stable design documents are:

- [architecture and ownership](docs/architecture.md);
- [Symbolica and exact algebra](docs/algebra.md);
- [frozen tensor boundary and Vakint sequencing](docs/tensor.md);
- [closing-rule foundry target](docs/foundry.md);
- [application, Python, and Vakint interfaces](docs/interfaces.md);
- [validation and oracle ladder](docs/validation.md);
- [LiteRed2 semantic reference](docs/references/litered2.md); and
- [current CLI contract](docs/CLI.md).

Local LiteRed2, GammaLoop/Vakint, FORM, and other reference checkouts live only
under ignored `FOR_REFERENCE_ONLY_DO_NOT_PUSH/`. They must never enter RustRed
history. GammaLoop inside that tree is a separate Git repository.

RustRed is licensed under the [MIT License](LICENSE).
