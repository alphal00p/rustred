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
- `rustred.family_candidates(source, ...)`
- `rustred.certify_candidates(bundle, ...)`
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
strict descent, and complete coverage inside it. Generation report schema v4
records `root_sector`, in-domain `zero_sectors`, and `global_zero_sectors`:
global zero proofs can be needed to replay translated sources even outside
the reduction domain. Inspection schema v4 reports `root_power_lower`,
`root_power_upper`, and `in_scope_zero_sectors`. Duplicate, negative, boolean,
or out-of-range indices are rejected. Zero-only scopes are not yet supported
and fail without publishing an artifact.

With a Project saved as `family.toml`, the equivalent CLI is:

```console
rustred family-close --input family.toml --output family.rr --n-cores 1
rustred campaign inspect --artifact family.rr
rustred campaign reduce --artifact family.rr --powers 2,2,1
```

## Save formulas before independent certification

`family_candidates` prepares and solves an explicitly supplied family but skips
the subsequent source replay and global closure proof. Its immutable result is
`CandidateBundleResult`, with `.bundle: bytes`, `.status ==
"uncertified-candidates"`, and a separate observational `to_toml()` timing
report. The bundle uses native Symbolica Atom binary serialization with a
shared state context, not expression strings or TOML. It is not a closing artifact and cannot be used by
`inspect_closing_artifact` or `reduce_with_closing_artifact`.

```python
from pathlib import Path
import rustred

source = "I(loops(k),externals(),dimension(d),prop(P,k^2-1,1))"
candidates = rustred.family_candidates(source, n_cores=1)
Path("tadpole.rrcandidate").write_bytes(candidates.bundle)
print(candidates.to_toml())  # preparation, solve and bundle-writing microseconds

# Can run in a fresh Python process; this does not repeat the search.
certified = rustred.certify_candidates(Path("tadpole.rrcandidate").read_bytes())
assert certified.status == "generated-durable"
print(certified.to_toml())  # reconstruction and exact certification timings
print(rustred.reduce_with_closing_artifact(certified.artifact, [3]).to_toml())
```

Load only generated bundles from a trusted source: Symbolica's native state
and Atom readers are not hardened parsers for hostile bytes. RustRed checks
framing, structural limits and coefficient contexts; independent certification
then checks mathematical replay and closure. Native dumps currently require
a 64-bit host. Their bytes may vary with prior Symbolica registrations, while
decoded coefficients and reductions must agree exactly.

Select `exact_backend="sparse-factorized"` to use Symbolica's native factorized
denominator field during symbolic target materialization and the shared exact
lift of numerical cases. Coefficients
return to the ordinary representation before rule extraction and bundle
encoding. The equivalent CLI option is `family-candidates --exact-backend
sparse-factorized`. This opt-in generation field is separate from the
candidate reduction cache; `"sparse"` remains the default.

The experimental `exact_backend="sparse-target-factorized"` option combines
the native factorized field with target-block symbolic elimination and a
one-shot full-identity reconstruction. Fully fixed numerical cases still use
the shared multi-target factorized lift. The matching CLI value is
`sparse-target-factorized`; there is no topology-specific dispatch or automatic
selection. A source prefix dependent in the harder/target block produces a
typed error, not a silent fallback. Candidate output still requires separate
certification.

Select `exact_backend="semi-numerical"` for Symbolica's rational-function
reconstruction, or `family-candidates --exact-backend semi-numerical` at the
CLI. These opt-ins retain the same source search, numerical-case discovery and
guards. Reconstruction retains ordinary exact replay, including its numerical
tail. Reconstruction has explicit bounds (degree 128, 200,000 probes,
four attempts, eight primes), and failure is an error, not an automatic exact
fallback. The report records the selected backend; backend selection neither
changes the bundle schema nor certifies closure. A small case that finds only
direct rules may not invoke the selected exact materialization backend.

The result of successful certification is the usual
`ClosingArtifactGenerationResult`. A modified, incomplete or inadmissible
bundle raises an exception; it cannot grant itself closure authority.
`numerical_depth=2` is the default signed-L1 search depth around fully fixed
cases. Use `numerical_depth=0` to search only their initial seeds and potentially
retain more finite residuals. Symbolic search and exact rule checks are unchanged;
this never proves residual independence or family closure. The value is saved
in the native bundle's versioned generation policy and appears in `to_toml()`.
It is independent of worker count and arithmetic backend, and rejects negative,
boolean, noninteger and larger-than-u32 values before generation.

Generation accepts `input_format`, `n_cores`, `exact_backend`,
`numerical_depth`, `permutation`, `checkpoint_dir`, `resume`, `checkpoint_max_bytes`, and
`nonpositive_indices`. Certification keeps the existing unit-mass vacuum
publication admission and accepts these optional caller resource limits:

```python
certified = rustred.certify_candidates(
    candidates.bundle,
    max_domain_bound_endpoint_cells=65536,
    max_predicate_consistency_work=67108864,
    max_predicate_atoms=64,
)
```

`max_negative_index_degree` requests the still-unsupported numerator-only
contract, which leaves positive propagator dots unbounded. Values through 30
fail closed; larger values are input errors. RustRed never silently substitutes
unrestricted certification or a bound that also counts dots.

The Rust application API now separately supports total excess
`sum(max(n_i-1,0) + max(-n_i,0)) <= D` at entry, including bounded native
scope and cold reproof. Its checked descendant degrees can exceed `D`.
That option is not yet exposed as a Python keyword or CLI flag; frontend parity
remains required follow-up. Existing Python inspection/reduction can load
bounded bytes through the shared verified loader. See the
[scope contract](../../docs/research/rank_bounded_certification.md).

Those same three limits are accepted by `family_close`,
`inspect_closing_artifact`, and `reduce_with_closing_artifact`; reapply chosen
budgets on a later cold load. Defaults are unchanged, zero is restrictive, and
the supported predicate-atom ceiling is 256. Limits are not serialized as
artifact authority. Candidate generation does not accept proof-budget options.

Equivalent CLI commands are `family-candidates` and `certify-candidates`; both
support `--report-output` for phase timings separate from their data output.
Neither candidate generation nor a successful finite-target experiment is a
proof of complete family coverage.

Optional `checkpoint_dir=Path("saved-sectors")` saves each completed sector as an
ordinary native uncertified candidate shard. The directory must initially be
new or empty and is exclusively locked for the request. To continue later, use
the same source text, input format, root, order, arithmetic backend and search
depth with `resume=True`; worker count may change. Completed shards are reused,
not solved again. Structural metadata is checked before native State import;
the final assembly imports each shard once and checks its exact family/context.
Invalid committed shards fail closed without replacement or automatic reruns.

The CLI equivalents are `--checkpoint-dir`, `--resume` and
`--checkpoint-max-bytes`. Resume or an explicit budget requires a directory.
`checkpoint_max_bytes` is a positive platform-sized integer (not bool), default
1 GiB. It bounds logical payload lengths of the manifest, retained/staging files
and pending writes, not peak RAM or filesystem overhead. Existing per-shard and
final bundle limits still apply; final assembly retains a global dictionary and
output buffer. Keep source files, final bundles and reports **outside** the
dedicated checkpoint directory. The CLI rejects conflicting paths even with
`--force`; Python returns final bytes for the caller to write elsewhere.
Checkpoints remain trusted-local generated data, not proof or closure authority.
When enabled, the report adds a `[checkpoint]` table with reused/newly solved
sector counts, charged disk bytes, resume-validation and assembly timings.
`solve_us` measures current new solves plus checkpoint writes, not historical
solving; `bundle_encoding_us` includes final assembly. Without checkpoints the
existing report has no additional table.

## Preset generation and artifact consumption

`generate_closing_artifact()` currently accepts the semantic family selectors
`rustred.ClosingFamily.UNIT_MASS_VACUUM_K1` and
`rustred.ClosingFamily.UNIT_MASS_VACUUM_K3`. Its result also exposes the
immutable native encoding as `.artifact: bytes`. Inspection and
reduction consume those exact bytes rather than substituting a hidden preset.
For `K = 3`, the cold validation boundary regenerates the registered
derivation once and compares the complete program structure and every exact
coefficient, including ordered variable maps, after native state remapping.
Ambient Symbolica registrations may change dump bytes without changing these
semantics. The hot reducer does not regenerate or reauthenticate the sealed
owner. Reduction terms expose typed
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
its V6 native Atom/State bytes through the shared Rust codec; they do not select
or generate a hidden K6 preset. For these original-domain source-port artifacts,
cold loading regenerates ordinary IBP rows and replays the saved exact
combinations without repeating source search. Native payloads must come from
a trusted source: independent mathematical validation does not make Symbolica's
native binary reader a hardened hostile-input parser. Recursive application
remains in RustRed's existing memoized reducer. The complete recorded 83-test through-three-loop Vakint
selection passes. The four-loop public numerical references and pinch checks
also pass, as recorded in the [numerical acceptance checkpoint](../../docs/checkpoints/2026-09-19.md#completed-public-numerical-acceptance).
Optional bounded four-loop artifact certification remains unfinished; see the
[bounded certification audit](../../docs/research/rank30_certification_audit_2026-09-17.md).

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
