# RustRed Python adapter

This package is the thin PyO3 frontend to `rustred-app`. It owns Python value
conversion, exception mapping, GIL release, and the process-wide coordinator;
Python representation/range checks and the early shared ingress guard stay in
the adapter, while semantic validation, shared resource policy, algebra, and
canonical TOML serialization remain in the Rust application layer.

Python 3.11 or newer is required. The extension uses Python's stable ABI with
a Python 3.11 floor. RustRed still uses Symbolica's Rust API with GMP; it does
not enable Symbolica's Python feature.

The public module is imported directly:

```python
import rustred
```

`rustred._rustred` is a private native extension detail. Top-level
`import _rustred` is intentionally unsupported.

The initial operations are:

- `rustred.derive(...)`
- `rustred.campaign_plan(...)`
- `rustred.campaign_preflight(...)`
- `rustred.family_close(source, ...)`
- `rustred.generate_closing_artifact(...)`
- `rustred.inspect_closing_artifact(artifact_bytes)`
- `rustred.reduce_with_closing_artifact(artifact_bytes, target_powers, ...)`

Each result's `to_toml()` method returns the exact newline-terminated TOML
produced by `rustred-app`. The `family_close` generation report includes
observational wall times; its durable artifact bytes, not those times, are the
semantic output.

## Closing a caller-supplied family

`family_close` accepts the same Project TOML or Symbolica family syntax as
`derive`. It never selects a solver by family name. The complete sector census
of the requested domain is solved and checked before a result is returned; incomplete closure raises
an exception instead of returning a partial artifact. Input must currently be
an unshifted vacuum with 1 through 16 denominator coordinates, dimension `d`,
no other scalar parameters, and literal constant term `-1` in every denominator.
The shared core checks this unit-mass scope before search. Normalize the common
mass yourself; symbolic `m` does not mean unit mass.

```python
import rustred

source = """I(
    name(my_sunset), loops(p,q), externals(), dimension(d),
    prop(A,p^2-1,1), prop(B,q^2-1,1), prop(C,(p-q)^2-1,1)
)"""
generated = rustred.family_close(source, n_cores=1)
assert isinstance(generated.artifact, bytes)
inspection = rustred.inspect_closing_artifact(generated.artifact)
reduction = rustred.reduce_with_closing_artifact(generated.artifact, [2, 2, 1])
print(generated.to_toml())  # Full-census counts and per-phase wall times.
print(reduction.to_toml())  # Exact coefficients of declared master keys.
```

For a one-loop example, use
`I(name(my_tadpole),loops(k),externals(),dimension(d),prop(P,k^2-1,1))`
and reduction powers `[3]`; the resulting master has powers `[1]` and the
separate common-mass-squared factor has exponent `-2`.

`input_format="toml"` selects an explicit Project input; `"auto"` is the
default. `permutation=[2,1,0]` optionally changes coordinate priority coherently
in every sector. It must contain every coordinate exactly once. `n_cores`
bounds the existing solver worker pool. The Python adapter releases the GIL
and submits owned requests to its existing process coordinator; all algebra,
coverage proofs, artifact generation and application remain in RustRed.

By default the domain is unrestricted; zero powers in the sample target never
restrict it. `nonpositive_indices=[2]` instead explicitly declares that input
coordinate 2 must have integer power at most zero, with no lower bound. Every
other coordinate remains unrestricted. Indices refer to the original input
order, not `permutation`. For example, the sunset's factorized pinch can be
closed and reduced independently:

```python
pinch = rustred.family_close(source, nonpositive_indices=[2], n_cores=1)
print(rustred.reduce_with_closing_artifact(pinch.artifact, [2, 2, -1]).to_toml())
# Reducing [1, 1, 1] with this artifact raises RustRedInputError: outside its domain.
```

The domain survives cold loading. Publication still requires exact replay,
strict descent, and complete coverage inside it. Generation report schema v2
records `root_sector`, in-domain `zero_sectors`, and `global_zero_sectors`:
global zero proofs can be needed to replay translated sources even outside
the reduction domain. Inspection schema v2 reports `root_power_lower`,
`root_power_upper`, and `in_scope_zero_sectors`. Duplicate, negative, boolean,
or out-of-range indices are rejected. Zero-only scopes are not yet supported
and fail without publishing an artifact.

With a Project saved as `family.toml`, the equivalent CLI is:

```console
rustred family-close --input family.toml --output family.rr --n-cores 1
rustred campaign inspect --artifact family.rr
rustred campaign reduce --artifact family.rr --powers 2,2,1
```

## Preset generation and artifact consumption

`generate_closing_artifact()` currently accepts the semantic family selectors
`rustred.ClosingFamily.UNIT_MASS_VACUUM_K1` and
`rustred.ClosingFamily.UNIT_MASS_VACUUM_K3`. Its result also exposes the
deterministic immutable encoding as `.artifact: bytes`. Inspection and
reduction consume those exact bytes rather than substituting a hidden preset.
For `K = 3`, the untrusted-load boundary cold-regenerates the registered
derivation once and byte-compares it before returning a sealed owner; the hot
reducer does not regenerate or reauthenticate it. Reduction terms expose typed
master power vectors, exact unit-mass coefficients, and the signed power of
the common mass squared that restores dimensional homogeneity.

```python
import rustred

generated = rustred.generate_closing_artifact(
    family=rustred.ClosingFamily.UNIT_MASS_VACUUM_K1,
)
assert isinstance(generated.artifact, bytes)

inspection = rustred.inspect_closing_artifact(generated.artifact)
reduction = rustred.reduce_with_closing_artifact(generated.artifact, [3])
term = reduction.terms[0]
assert term.master_powers == [1]
assert term.common_mass_squared_power == -2
```

The matching file-based CLI sequence is:

```bash
rustred campaign generate --family unit-mass-vacuum-k1 --output one_loop.rr
rustred campaign inspect --artifact one_loop.rr --output one_loop.inspect.toml
rustred campaign reduce --artifact one_loop.rr --powers 3
```

`--output -` writes artifact bytes or TOML, as appropriate, to standard
output; `--artifact -` reads durable artifact bytes from standard input. The
matching two-loop selector is `unit-mass-vacuum-k3`; its powers have arity
three. The `K = 1` and `K = 3` artifacts are closed today. The three-loop
`K = 6` artifact can be generated with the Rust `spired-generate-k6`
example or by supplying its unit-mass family to `family_close`; no additional
Python family selector is needed. Existing K6 examples are retained.
The generic Python inspection and reduction functions consume
its real V5 bytes through the shared Rust codec; they do not select or generate
a hidden K6 preset. For these original-domain source-port artifacts, untrusted
loading regenerates ordinary IBP rows and replays the saved exact combinations,
not the source search. Recursive application remains in RustRed's existing
memoized reducer. The complete recorded 83-test through-three-loop Vakint
selection passes; four-loop acceptance remains a separate open gate.

Linux wheels built in the Nix development shell are development artifacts.
Portable manylinux publication remains gated on a separate audited build and
repair pipeline for the platform C runtime and GMP-backed native linkage.

## Release and test gates

Do not publish the current sdist. A rebuildable sdist must contain the
Symbolica, graphica, and numerica path sources, while Symbolica's bundled
`License.md` forbids copying or distribution without express permission.
Obtain and record redistribution permission before publishing either source
archives or wheels containing the linked Symbolica implementation.

The Nix-built wheel is only a development/test artifact: it carries local Nix
runpaths and a generic `linux_x86_64` tag. A release wheel requires a dedicated
manylinux build, `auditwheel` inspection/repair, clean-container installation,
and a fresh license/linkage review. Do not relabel the Nix artifact as
manylinux.

Ordinary Rust tests run with this crate's default features. Use `cargo check
-p rustred-python --features extension-module` to compile-check the extension
configuration and use maturin for the real extension build. Do not run Rust
test binaries with `extension-module` or `--all-features`: extension modules
intentionally leave CPython symbols for the interpreter to resolve and such
test executables do not link as Python extensions.

Schema, input-limit, lowering, and license failures have public Python/CLI
parity fixtures. Serialization and output-limit exception selection is covered
by the exhaustive internal `AppErrorKind` mapping test because there is no
small, safe public request that forces the 256 MiB output boundary. That is an
internal mapping test, not claimed as injected end-to-end serialization
evidence.

To prove that an sdist is self-contained, build it and then ask `uv` to create
a wheel in an isolated build environment using only that archive:

```bash
maturin sdist --out dist
uv build --wheel --out-dir dist/rebuilt --python python dist/rustred-*.tar.gz
python crates/rustred-python/tests/assert_distribution_contents.py \
  dist/rustred-*.tar.gz dist/rebuilt/*.whl
```
