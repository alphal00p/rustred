# RustRed

RustRed is a pre-alpha, pure-Rust and Symbolica-native project for deriving,
closing, validating, and applying parametric integration-by-parts identities.
Its scientific pressure target is a generic, parallel rule foundry capable of
closing single-scale vacuum families through six loops. Loop count and topology
remain input data rather than production dispatch keys.

The project is in a deliberate repository and API reset. There is no RustRed
backward-compatibility promise during this phase. Historical authored
recurrences, broad integration suites, compatibility facades, and milestone
logs are being deleted rather than migrated. Git is their archive.

## Current workspace

- `rustred` is temporarily the repository-root mathematical crate while its
  live code is pruned into owned domains. It will move to
  `crates/rustred-core` before Phase 0 completes.
- `crates/rustred-app` owns the shared application boundary and the `rustred`
  CLI.
- `crates/rustred-python` is a thin PyO3 adapter. Python users write
  `import rustred`; `rustred._rustred` is a private native extension detail.
  Long-running calls release the GIL and serialize through one process-wide
  coordinator. If a contained native panic reaches that boundary, the
  coordinator is permanently poisoned and later requests are rejected rather
  than reusing potentially inconsistent process state.
- `vendor/symbolica` is the sole production CAS dependency and is built with
  GMP support. FORM, Mathematica, SymPy, and authored recurrence tables are
  forbidden from RustRed production and ordinary tests.

The root mathematical package is still in the temporary in-place pruning
stage; ownership cleanup is active and the planned relocation has not yet
completed. The currently evidenced application operations are raw family/IBP
derivation, roots-only campaign planning, and campaign resource preflight.
Closed reusable family artifacts, general reduction, master substitution, a
complete one-through-four-loop Vakint comparison corpus, and physical six-loop
closure are not yet available.

## Development

Enter the pinned Nix development environment and provide a valid Symbolica
license through the `SYMBOLICA_LICENSE` environment variable when executing
licensed operations:

```bash
nix develop
cargo fmt --all --check
cargo check --locked
```

Inspect the current CLI contract with:

```bash
cargo run -p rustred-app --bin rustred -- --help
```

The Python distribution is built from the root `pyproject.toml`; its public
import smoke test is:

```bash
uv venv .venv
source .venv/bin/activate
maturin develop --features extension-module
python -c 'import rustred'
```

## Authority and scope

[`GOAL.md`](GOAL.md) is the authoritative long-horizon objective and execution
roadmap. The current destructive reset is specified by the
[clean-repository architecture plan](docs/research/repository_clean_architecture_plan_2026-08-28.md).
LiteRed2 is a read-only algorithmic reference, not a code or architecture
dependency; its upstream project is available at
[rnlg/LiteRed2](https://github.com/rnlg/LiteRed2).

Local reference checkouts live only under the ignored
`FOR_REFERENCE_ONLY_DO_NOT_PUSH/` directory. They must never enter RustRed
history. Vakint/GammaLoop development occurs in its own repository and branch.

RustRed is licensed under the [MIT License](LICENSE).
