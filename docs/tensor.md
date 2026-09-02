# Frozen RustRed tensor boundary and FeynKit integration

[`GOAL.md`](../GOAL.md) is authoritative. RustRed tensor-reducer development is
frozen during Stage 1. A collaborator supplies the FORM-less tensor reducer in
GammaLoop's `feynkit` branch, and the Vakint branch `vakint_rustred` is based on
that branch.

## Active Stage 1 evaluation path

For a tensor-bearing Vakint input, the accepted path is:

```text
Vakint parse and topology match
  -> Vakint simultaneous canonical routing witness
  -> FeynKit tensor reduction
  -> RustRed scalar numerator lowering
  -> shipped RustRed parametric-IBP artifact application
  -> Vakint pure-Rust master substitution and presentation
```

The complete path must work with an invalid FORM executable. AlphaLoop,
MATAD, FMFT, and the historical FORM tensor method remain independent,
backward-compatible modes and offline comparison oracles; they are never a
fallback from `EvaluationMethod::RustRed`.

The RustRed scalar adapter consumes the topology and simultaneous routing
witness already selected by Vakint. It must not rematch graphs, dispatch on
topology names, duplicate Vakint's registry, regenerate artifacts during
evaluation, or reinterpret FeynKit's tensor result through a second tensor
projector.

## Ownership

Vakint owns parsing, graph/topology matching, canonical routing, backend
selection, normalization, master presentation, and user-facing backward
compatibility. FeynKit owns tensor reduction. RustRed owns exact scalar
numerator lowering, immutable artifact validation/application, guarded descent,
typed terminal keys, memoization, and common-mass homogeneity restoration.

Production RustRed and the FeynKit-plus-RustRed acceptance lane use Rust and
Symbolica only. FORM may be executed solely by separately selected legacy
oracle modes.

## Frozen RustRed experiment

The repository may retain already existing bounded RustRed tensor code as
non-production experimental capability, but Stage 1 does not extend, expose,
or integrate it. The obsolete GammaLoop commits that introduced a
`TensorReductionMode::RustRed` path were deliberately dropped when
`vakint_rustred` was rebased onto `feynkit`.

This freeze does not weaken the generic scalar design. RustRed families,
integral keys, scalar-numerator lowering, artifacts, and reducers remain
topology-neutral and usable independently of Vakint or FeynKit.

## Validation

The Vakint acceptance matrix must:

- explicitly select `TensorReductionMethod::FeynKit` for tensor-bearing
  RustRed lanes instead of relying on a default;
- use an invalid FORM path for the complete FeynKit-plus-RustRed lane;
- reuse the existing AlphaLoop/MATAD comparison harness and compare the same
  inputs, masses, normalization, epsilon order, and numerical precision;
- cover scalar and tensor-bearing representatives of every supported matcher
  class through three loops once the K6 artifact is available;
- preserve existing Vakint defaults, public API conventions, and FORM-backed
  modes; and
- distinguish failures in tensor preprocessing, topology/routing, RustRed
  scalar reduction, terminal substitution, and final numerical comparison.

After parity is green, optimized benchmarks separate cold artifact
load/validation from warm scalar reduction and profile equivalent RustRed,
AlphaLoop, and MATAD workloads. Debug timings are not performance evidence.

## Deferred work

Stage 2 may revisit advanced rank-generic tensor technology only after explicit
user guidance. No Stage 1 work should duplicate FeynKit, grow the frozen
RustRed tensor experiment, or allow tensor concerns to delay autonomous K6
parametric-IBP closure.
