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
- `rustred.generate_closing_artifact(...)`
- `rustred.inspect_closing_artifact(artifact_bytes)`
- `rustred.reduce_with_closing_artifact(artifact_bytes, target_powers, ...)`

Each result's `to_toml()` method returns the exact canonical,
newline-terminated TOML produced by `rustred-app` and the CLI.

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
`K = 6` artifact remains Stage 1 work.

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
