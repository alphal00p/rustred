# RustRed Python API directive

Date: 2026-08-27

Status: mandatory product requirement. The transport-neutral application
boundary is complete; the dedicated Python package and outer coordinator remain
to be implemented through the repository reorganization.

## Contract

RustRed will provide a PyO3 package exposing the same semantic operations as
the CLI. There will be one implementation path:

```text
CLI ---------+
             +--> typed Rust application API --> topology-neutral RustRed core --> Symbolica[gmp]
PyO3 -------+
```

The shared application layer owns request validation, input lowering,
derivation/campaign services, deterministic result DTOs, output-size limits,
typed errors, and canonical serialization. The CLI alone owns OS argument
parsing, paths, stdin/stdout, overwrite policy, help text, exit codes, and
terminal presentation. The Python crate alone owns Python argument/result
objects and exception conversion. Neither frontend may contain algebra,
reduction logic, or an independently evolving output schema.

Initial application operations are the CLI-equivalent forms of:

- `derive`, including compact Symbolica, hybrid TOML, and explicit TOML input,
  relation selection, and positive `n_cores`;
- multi-root campaign planning; and
- campaign resource preflight.

Future CLI operations must enter the shared application API before Python
parity is added. Python result objects may offer ergonomic projections, but
their `to_toml()` output must call the same serializer and be byte-identical to
CLI stdout for an equivalent request.

## Workspace and package boundary

The reorganization audit selected the following dependency direction:

```text
rustred CLI binary --+
                    +--> rustred-app --> rustred core --> vendored Symbolica with GMP
rustred-python -----+
```

The CLI binary lives inside `rustred-app`; a separate transport-only CLI crate
would add no useful isolation. The app boundary is now transport-neutral:
semantic services, public errors/options, resource limits, and canonical
serialization live under `application`, while CLI concerns remain in the
adapter. Direct tests prove API/CLI canonical-byte parity, and `rustred-app`
depends on Symbolica only transitively through the core. The dedicated
`rustred-python` package remains to be implemented with poison-on-panic
containment at its outer coordinator/FFI boundary.

The binding should be a dedicated `cdylib`/`rlib` workspace package using
PyO3 and maturin, with Python >= 3.11 as the initial supported floor. PyO3,
Python objects, and packaging dependencies cannot leak into the app or core
crates. Symbolica's Python feature is not needed: RustRed uses Symbolica's Rust
API directly. The final feature graph must prove that GMP is enabled and
`no_gmp` is absent.

## Concurrency and Symbolica safety

Python inputs are converted to owned Rust requests while holding the GIL.
Long-running Rust work then releases the GIL, and no Python-owned value,
callback, or GIL token may reach RustRed's worker threads. `n_cores` controls
the same private deterministic Rust worker pool used by the CLI.

The vendored Symbolica 2.2.0 unlicensed manager is tied to the first calling
thread and can abort the process if a later call arrives on another thread.
Therefore the initial Python runtime must route top-level requests through one
process-wide coordinator thread. This also prevents concurrent Python callers
from multiplying private pools and violating CPU/RAM admission. A licensed
request may still use its requested Rust worker width beneath that coordinator.
Rust panics must be caught at the outer coordinator/FFI boundary and translated
to a typed internal Python failure. The coordinator must then be poisoned and
reject later requests; an invariant panic or partially mutated Symbolica/global
state must not be presented as safely recoverable. No binding can catch a
native process abort, so the thread contract must still be respected
proactively.

The module must initially declare that it uses the GIL for module/object state.
Free-threaded CPython support is a separate future audit. Cooperative
cancellation is also a separate core capability; releasing the GIL alone does
not make an in-flight Symbolica calculation cancellable.

## Error and parity requirements

The application boundary replaces CLI-string-only failures with stable typed
categories for input/schema/limits, lowering, derivation, execution/license,
serialization/output limits, and internal invariants. The CLI maps these to
messages and exit codes. PyO3 maps them to a compact RustRed exception
hierarchy after the GIL is reacquired.

Acceptance requires:

- application tests for each operation and exact canonical TOML bytes;
- byte-for-byte CLI/application/Python parity across every accepted input mode
  and relation filter;
- identical results for licensed `n_cores = 1, 2, 4`;
- Python-thread tests proving GIL release and coordinator serialization;
- malformed-input, resource, license, and serialization error parity;
- fresh-subprocess tests for license modes and abort-prone thread-affinity
  boundaries, since Symbolica license initialization is process-global and
  one-shot;
- `maturin build --release --locked`, clean-environment wheel installation,
  and sdist rebuild including all path dependencies;
- an audit of GMP/MPFR/MPC linkage before claiming portable manylinux wheels;
  and
- ordinary tests run in parallel, with only genuinely process-global license
  probes isolated.

This directive records a required frontend, not a new reduction capability.
