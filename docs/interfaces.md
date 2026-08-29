# Interfaces

This document separates the currently available interfaces from planned
services. RustRed is pre-alpha and does not promise backward compatibility;
Vakint's existing user-facing behavior does.

## Current workspace

The repository is a virtual Cargo workspace with three packages:

| Package | Responsibility |
|---|---|
| `rustred` (`crates/rustred-core`) | Topology-neutral mathematical values and services backed by Symbolica |
| `rustred-app` | Shared application composition, canonical TOML results, and the `rustred` CLI |
| `rustred-python` | Thin PyO3 adapter exposing the public Python package `rustred` |

There is no root Rust package or root `src/` tree. Symbolica 2.2.0 with GMP is
the sole production CAS. RustRed has no FORM, Mathematica, or SymPy runtime
path.

## Rust core

The public `rustred` facade is organized by mathematical owner:

- `algebra` provides authenticated base and index-extended Symbolica
  coefficient contexts and checked exact operations;
- `family` owns complete affine integral families, integral keys, automatic
  ISP completion, kinematics, domain conditions, Symanzik polynomials, and the
  authenticated physical/auxiliary presentation used for optimized-lane
  evidence;
- `input` compiles compact, TOML-derived, text-Symbolica, or caller-owned Atom
  descriptions into one normalized project, authenticates them at ingress,
  and lowers the result to a family;
- `identity` owns sparse parametric relations and the topology-neutral
  `ParametricIbpGenerator`, including ordinary and LI source batches;
- `sector` owns masks, restrictions, ordering, exact caller-supplied symmetry
  verification/transport, and on-demand zero-sector analysis;
- `tensor` owns validated caller Symbolica heads, sealed-evidence lane
  selection, bounded key-aware Lorentz projection, and affine scalar-product
  lowering onto typed integral keys;
- `foundry::anchored` owns concrete-index rule derivation, while
  `foundry::parametric` owns fixed-sector `K(n)` elimination, uniform descent,
  exact symbolic replay, guards/provenance, and anchored agreement; both expose
  a requested-pivot variant backed by deterministic Symbolica RREF;
  `foundry::dependency` owns exact target-sector partition work admission and
  compact, resumable proper-subsector obligation descriptors; and
- `campaign` owns resource profiles, execution-width preflight, and bounded
  ordered parallel execution.

These first foundry and tensor slices do not yet constitute a closing foundry
or scalar-IBP reducer. There is currently no public durable `artifact` owner,
closed-rule publisher, rule application, master substitution, generic tensor
kinematics, or higher-even-rank projector. The Rust API may change directly as
those real owners and callers are introduced; obsolete facades are not
retained for compatibility.

## Shared application API

`rustred-app` currently exposes three transport-neutral operations:

```rust
derive(DeriveRequest) -> Result<DeriveResult, AppError>
campaign_plan(CampaignPlanRequest) -> Result<CampaignPlanResult, AppError>
campaign_preflight(CampaignPreflightRequest)
    -> Result<CampaignPreflightResult, AppError>
```

`derive` parses and lowers one family and emits selected raw parametric
ordinary and/or LI relations. A concrete target in the input is validated and
reported, not reduced. `campaign_plan` authenticates and interns only supplied
roots; it does not discover dependencies or prove closure.
`campaign_preflight` computes a topology-neutral memory-limited execution
width and does not start workers.

Each result owns a canonical, newline-terminated TOML document accessible
through `to_toml()` or `into_toml()`. Application errors retain typed input,
schema, resource, lowering, derivation, execution, license, serialization,
output-limit, and internal categories.

## Command line

The binary is `rustred`, supplied by `rustred-app`:

```text
rustred derive [OPTIONS]
rustred campaign plan [OPTIONS]
rustred campaign preflight [OPTIONS]
```

`derive` accepts `--input-format auto|toml|symbolica`,
`--relations all|ordinary|li`, and a positive `--n-cores`. Campaign planning
accepts an optional root identifier. Campaign preflight requires a resource
profile and an explicit positive memory limit. Input and output default to
standard streams; file output requires `--force` to replace an existing file
and is committed atomically.

The CLI calls the same application functions as Python. It is not a separate
solver implementation.

## Python

Users always import the public package:

```python
import rustred

relations = rustred.derive(source, relations="all", n_cores=4)
plan = rustred.campaign_plan(source)
width = rustred.campaign_preflight(
    profile,
    n_cores=4,
    max_memory_bytes=16 * 1024**3,
)
```

The result classes expose `schema`, `status`, and `to_toml()`. Public exception
classes mirror the application error categories. `rustred._rustred` is a
private extension implementation detail; top-level `_rustred` is not the user
API.

Long-running calls release the GIL and pass through one process coordinator.
If an internal panic crosses that boundary, the coordinator is poisoned and
later requests fail instead of reusing uncertain native state. A coordinator
created before `fork()` is likewise rejected in the child.

## Existing frozen Vakint tensor seam

Vakint/GammaLoop development lives in its separate repository on the
`vakint_rustred` feature branch. An existing experimental boundary provides:

```rust
vakint
    .tensor_reducer(&settings)
    .mode(TensorReductionMode::RustRed(RustRedOptions::new()))
    .reduce(input)
```

It reaches the bounded RustRed scalar/odd/rank-two projector for authenticated
common-mass vacuum presentations. `TensorReductionMode::Form` remains the
default. This experiment is frozen during Stage 1: it is not extended, made
rank-generic, or used as the Stage 1 tensor path. Vakint retains its existing
FORM tensor preprocessing and behavior.

## Active Stage 1 Vakint seam

The new additive interface is a scalar evaluation backend, separate from
tensor mode selection:

```rust
EvaluationMethod::RustRed(RustRedEvaluationOptions::default())
EvaluationOrder::rustred_only()
```

`RustRedEvaluationOptions` controls optional master substitution, enabled by
default. The backend consumes the topology match and simultaneous routing
witness already produced by Vakint, loads the corresponding shipped immutable
artifact, applies guarded rules through RustRed, and returns exact coefficients
of typed master keys mapped to Vakint's existing MATAD master basis. It reuses
Vakint's pure-Rust master values when substitution is requested.

The scalar backend reports no FORM dependency, never invokes FORM, and never
falls back to AlphaLoop, MATAD, FMFT, or another reducer. For tensor-bearing
inputs, Vakint deliberately runs its unchanged FORM tensor prepass before this
FORM-free scalar tail. Scalar or already tensor-reduced tests use an invalid
FORM path to prove the backend's isolation. Existing evaluation orders,
defaults, results, and public behavior remain backward compatible.

Artifacts are generated once by RustRed, checked into and shipped with Vakint,
validated once when loaded, and reused for ordinary evaluation. Vakint must not
regenerate them or maintain topology-authored recurrence code.

Milestone commits in GammaLoop pin RustRed to an exact Git revision and resolve
RustRed and Vakint against one exact Symbolica-family revision. A relative
local path may be used while co-developing uncommitted changes, but a pushed,
reproducible milestone uses an exact Git revision. Reference checkouts under
`FOR_REFERENCE_ONLY_DO_NOT_PUSH/` never enter RustRed history.

## Ownership of the completed Vakint path

Vakint remains the user-facing steering and presentation layer. It owns:

- its existing topology registry and
  `Topologies::match_topologies_to_user_input` matcher;
- canonical graph/routing selection and simultaneous numerator routing maps;
- conversion between Vakint terms and typed RustRed requests;
- backend choice, orchestration, normalization, existing master values, and
  result presentation; and
- backward compatibility for all existing modes.

RustRed owns or will own the reusable mathematical services:

- authentication of a matched family presentation, including physical versus
  auxiliary denominators, routing, shifts, and common-scale evidence;
- guarded artifact lookup and deterministic memoized IBP application;
- stable master keys, common-mass restoration, and typed supplied-master
  substitution; and
- exact failures for unsupported domains, missing artifacts, undecidable
  guards, cycles, or resource exhaustion.

RustRed does not rematch a topology that Vakint has already matched, and
Vakint does not duplicate the rule engine. Defects in Vakint matching are
fixed and tested in that matcher rather than bypassed by a second topology
table.

The existing tensor API remains available at its evidenced bounded frontier,
but extension or replacement of tensor reduction belongs to Stage 2.

## Planned Stage 1 fine-grained surfaces

The Rust application, CLI, and Python package will expose the same individual
artifact services used by Vakint:

- family construction and raw IBP/LI generation;
- closure campaigns and artifact inspection/replay;
- guarded, memoized reduction to stable master keys with common-mass
  restoration; and
- supplied master-value validation and substitution.

These interfaces remain useful for arbitrary non-vacuum families. The Vakint
vacuum artifact library is an optimized deployment of the generic services,
not the limit of RustRed's family model. Tensor API expansion is not part of
these Stage 1 surfaces.
