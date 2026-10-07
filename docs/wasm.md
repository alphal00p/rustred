# WebAssembly and HEPKit

RustRed's browser target is **Pyodide (`wasm32-unknown-emscripten`)**.
This is not a JavaScript rewrite, remote reduction service, or separate CAS.
The same Rust solver and exact authority pipeline use Symbolica's existing
`wasm` feature, with its Malachite integer and Astro floating-point backends.
Bare `wasm32-unknown-unknown`, WASI, and a browser CLI are not claimed supported.

## Build selection

Native defaults retain GMP/MPFR and reconstruction. Embedded hosts must select
their arithmetic backend once for their shared Symbolica kernel:

```toml
rustred-feynkit = { git = "https://github.com/alphal00p/rustred", branch = "main", default-features = false, features = ["wasm", "campaign-api"] }
```

For a Rust-only application check, with the matching Rust target installed:

```bash
cargo check -p rustred-app --no-default-features --features wasm \
  --target wasm32-unknown-emscripten
```

Build a complete Symbolica Community wheel using that repository's Pyodide
build tooling and `wasm` feature. Never load a second Symbolica-owning RustRed
extension beside HEPKit. Standalone `import rustred` builds use the same Pyodide
cross-toolchain, disabled defaults, and `wasm,extension-module` features.
No reconstruction implementation was added: that feature still depends on the
selected Symbolica revision's existing reconstruction API.

For faster functional iterations, Community's build script also accepts
`WASM_RUST_PROFILE=dev`. This retains debug assertions, disables debug symbols
by default to keep the wheel manageable, and uses an unoptimized final link.
Use release builds for performance measurements; dev timings are not comparable.

## Execution contract

`hep.rustred.execution_capabilities()` (or `rustred.execution_capabilities()`)
reports the execution model. On WebAssembly:

- Exactly one worker is supported. Larger requests fail explicitly.
- Calls execute inline on the interpreter thread; no OS threads are spawned.
- A `start_*` call completes its calculation before returning a completed or
  failed session. Bounded recorded events and the result remain accessible.
- There is no live event polling or in-flight cancellation by the same
  interpreter. A notebook must require an explicit Generate action and explain
  this limitation. Running Pyodide in a Web Worker keeps the browser UI separate.
- Native background coordination, panic poisoning, and worker scheduling remain
  unchanged. The inline executor likewise rejects recursive entry and poisons
  subsequent work after a caught panic.

The reusable byte-oriented generation, candidate inspection, certification and
rule-application APIs do not require a host filesystem. File operations use the
Pyodide virtual filesystem; download artifacts explicitly to retain them.

## Artifact portability and bounds

Artifacts retain the existing fixed-width little-endian `u64` length framing
and Symbolica-owned atom/state encoding. RustRed framing checks lengths
against the reader's addressable range. The wire format does not encode the host's
`usize` width, and existing 64-bit artifacts are not rewritten to a new schema.
Matching Symbolica serialization formats are still required.

Browser memory limits remain real: WebAssembly32 cannot address native-sized
multi-gigabyte campaigns. A generous native limit does not grant browser RAM.

There is one narrow portable-input limitation: the strict retained
allocation census for affine denominator compilation cannot inspect hidden
Malachite allocation capacity for coefficients larger than 128 bits. That
boundary returns an explicit unsupported-operation error rather than reporting
a false memory bound. Arbitrary-precision solver coefficients still use
Symbolica; this is not a general 128-bit arithmetic restriction.

## Regression checks

The Python adapter's `tests/wasm_smoke.py` supplies `run_smoke(api)` for an
installed standalone or HEPKit module. It generates and certifies a tadpole,
reopens binary artifacts, and reduces a dotted integral with mass restoration.
Community's Pyodide gate additionally exercises HEPKit DOT input and the full
three-loop unit-mass vacuum example, including exact reductions, numerator and
pinch checks, and a nonminimal terminal basis. Compilation alone is not a
runtime acceptance result.

The HEPKit four-loop notebook may use the same serial APIs, but completing
four-loop production workloads within browser memory/time limits is a separate
performance exercise. Vakint and FORM-based backends remain native-only here.
