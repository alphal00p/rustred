# AMFlow / RustRed runtime-arity handoff

Status: implemented, independently reviewed and pushed to RustRed `main` at
[`33fd03ec5ee655d1aec989dad3c37f93c0539478`](https://github.com/alphal00p/rustred/commit/33fd03ec5ee655d1aec989dad3c37f93c0539478),
on reviewed PR2 merge `3d0b08fb`.
All21 focused optimized native tests and the FeynKit Rust check pass.
The cross-task notification outcome is recorded below separately.

## What is limited

`rustred::solver::SectorSolver<const N: usize>` is not intrinsically limited to
twelve scalar-product coordinates. Two independently compiled front doors
were identified as selecting only `N = 1, …, 12`:

- RustRed's `solver::bridge` dynamic solve and certificate dispatch (fixed in
  the pushed revision above).
- AMFlow's `src/native.rs`, which uses the generic solver directly rather than
  passing through that bridge for its factorized-coefficient path.

The count includes the completed family's auxiliary/ISP coordinates, not just
physical propagators or the number of loops. Exported vector-key rules do not
inherit the twelve-slot input-dispatch restriction.

The optional legacy packed coordinate-priority permutation has a separate
34-coordinate bound (`34!` fits in `u128`; `35!` does not). The default integral
comparator is not subject to that permutation encoding limit. Compact powers
remain bounded to `-64..=63`, independently of arity.

## Delivery design

The patch exposes checked const-generic bridge entry points returning the same
dynamic solution types: `solve_parametric_for::<N>`, `solve_laporta_for::<N>` and
`certify_laporta_for::<N>`. They validate that `N` matches the complete family
before entering its existing solver. This lets Rust callers select their own
`N` without editing the engine. `rustred::compiled_runtime_arities()` reports
the finite runtime registry, defaulting to `1..=16`. Set, for example,
`RUSTRED_RUNTIME_ARITIES=1,2,13,14,15,21` **at build time** to change it; Cargo
tracks the setting and rebuilds the affected crate. This is not a runtime flag.
Do not advertise this as an unlimited precompiled Python binary. Preserve
the exact solver, guards, cuts, preferred-master and certificate semantics.

AMFlow's direct factorized path can replace its local match with the shared
macro while preserving its own generic `solve` implementation and every option:

```rust
rustred::dispatch_arity!(
    family.family.denominator_count(),
    solve(family, original, dimension, targets, options, context, cuts),
    n => Err(Error::Unsupported(format!(
        "native host was compiled for {:?}; received {n}",
        rustred::compiled_runtime_arities()
    )))
)
```

Alternatively insert an explicit literal list after the arity expression,
such as `[1, 2, 13, 14, 15, 21]`, to choose AMFlow's monomorphizations
independently. Neither dispatcher changes ordering, the coefficient backend,
cuts, or resource budgets. Other APIs retain their own documented
representation/capability limits: this is a bridge/direct-host fix, not a claim
that every saved-campaign front door now has arbitrary compiled arity.

HEPKit's `IBPFamily.compiled_runtime_arities()` exposes the same capability list
for its native host. For a complete family with independent external momenta,
the coordinate count is `L*(L+1)/2 + L*E`; auxiliary coordinates contribute to
the count even when their powers vanish in a requested integral.

Validated nonzero exact tadpole recurrences in complete families with13,14,15
and17 coordinates, including auxiliary linear scalar products. Tests check exact
coefficients and regenerated-source certificates;17 uses the generic entry point
outside the default runtime registry. Runtime/generic parity, invalid-arity
rejection, cuts, preferred-master guards and existing bridge behavior also pass.
Such tests establish arity plumbing, not physical Higgs-plus-jet or AMFlow/DiffExp
parity. The downstream task should run its actual blocked workload as well.

The21 tests comprise seven `bridge_arity`, two `arity_dispatch`, and twelve
existing `feynkit_bridge` tests. Command, from the isolated final source:

```sh
cargo test --release --locked --offline -j8 -p rustred \
  --test bridge_arity --test arity_dispatch --test feynkit_bridge \
  -- --test-threads=8
cargo check --release --locked --offline -j8 -p rustred-feynkit
```

Local receipts: `TMP/arity-final-integration-20261004/TMP/arity-gate-v1/`.
The compiled host typecheck passed; the newly added Python14-slot test has not
yet run against a rebuilt community extension. This distinction does not block
using the tested Rust API from AMFlow.

## Incoming PR2 API migration

The independently reviewed PR adds cut and preferred-master arguments:

```rust
let cuts = rustred::sector::CutConstraint::none(family.denominator_count())?;
// Ordinary reductions pass the family's no-cut constraint and an empty
// preferred-master slice.
solve_parametric(&family, &cuts, &sector, &fixed, options);
solve_laporta(&family, &cuts, &targets, &[], options);
```

`DynamicSolveOptions` gains `until_stable`, defaulting to `false`; use
`..Default::default()` in an explicit options literal. Stability is a bounded
search heuristic, not a proof of minimal masters or unrestricted closure.
AMFlow's nonfactorized bridge call must migrate even if its usual direct native
path is unchanged. Its cut-enabled direct path should retain its own validated
semantics when adopting the shared arity dispatcher.

## Coordination evidence

Root read the app thread and the accessible AMFlow sources. Cross-task sending
currently fails with the app tool's “no longer available through dynamic tools”
message, and the indicated replacement messaging MCP is not exposed in this
session. No coordination message has been delivered through that channel yet.
Root retried the supported send tool after the successful push; it returned
the same unavailable-tool error. The user was given the exact revision and a
copy-pasteable relay message. Notification must not be reported as delivered.
No AMFlow source has been edited by this task. A later read-only check at
22:16 UTC found its adapter already using `rustred::dispatch_arity!` and the
capability getter, via its local-path RustRed dependency. This is evidence of
downstream source adoption, not successful downstream compilation or physics
validation, and does not mean the failed message was delivered. The receiving
task should validate its PR2 argument migration and actual workload. Pin the tested
Git revision (or use a clean checkout of it); this working tree also contains
ongoing, uncommitted native-session work unrelated to the arity fix.
