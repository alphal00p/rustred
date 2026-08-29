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
  exact symbolic replay, guards/provenance, and anchored agreement; and
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

## Current Vakint seam

Vakint/GammaLoop development lives in its separate repository on the
`vakint_rustred` feature branch. The pushed additive boundary currently
provides:

```rust
vakint
    .tensor_reducer(&settings)
    .mode(TensorReductionMode::RustRed(RustRedOptions::new()))
    .reduce(input)
```

`TensorReductionMode::Form` is the default. The existing
`Vakint::tensor_reduce`, `VakintSettings`, evaluation methods, and FORM-backed
behavior remain unchanged. The builder's default path is tested to produce
exactly the same result as the existing method.

The `RustRed` selection now reaches the [bounded RustRed tensor
service](tensor.md) for one-loop, one-propagator single-scale vacuum inputs
through rank two. Vakint performs its existing topology match and simultaneous
numerator routing, passes the exact matched integral power into RustRed, and
maps exact `d = 4 - 2 epsilon` coefficients and tensor heads back to either
Vakint output notation. Other families or unsupported ranks return precise
typed errors. It does not invoke or fall back to FORM.

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
- backend choice, orchestration, normalization, and result presentation; and
- backward compatibility for all existing modes.

RustRed will own the reusable mathematical services:

- authentication of a matched family presentation, including physical versus
  auxiliary denominators, routing, shifts, and common-scale evidence;
- Lorentz tensor projection and family-aware scalar-product lowering;
- guarded artifact lookup and IBP rule application;
- stable master keys and typed supplied-master substitution; and
- exact failures for unsupported domains, missing artifacts, undecidable
  guards, cycles, or resource exhaustion.

RustRed does not rematch a topology that Vakint has already matched, and
Vakint does not duplicate tensor projectors or the rule engine. Defects in
Vakint matching are fixed and tested in that matcher rather than bypassed by a
second topology table.

The first tensor API exposes separate projection, scalar lowering, and
composed operations. `Auto` selects an optimized vacuum lane only from a
sealed RustRed proof of single-scale, no-external-denominator-shift semantics.
External spectator vectors in the numerator do not invalidate that proof. A
fully generic external-kinematics lane is present as an explicit typed
unsupported boundary until its algorithm is implemented.

## Planned fine-grained surfaces

As implementations become real and validated, the Rust application, CLI, and
Python package will expose the same individual services used by Vakint:

- family construction and raw IBP/LI generation;
- closure campaigns and artifact inspection/replay;
- tensor projection and scalar lowering;
- guarded reduction to stable master keys; and
- supplied master-value validation and substitution.

These interfaces remain useful for arbitrary non-vacuum families. The planned
Vakint vacuum artifact library will be an optimized deployment of the generic
services, not the limit of RustRed's family model.
