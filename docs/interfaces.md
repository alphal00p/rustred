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
- `scalar_numerator` starts after tensor projection and lowers an already
  scalarized polynomial vacuum numerator through a sealed artifact's affine
  basis, returning exact shifted keys, spectator coefficients, and explicit
  common-mass powers; it does not perform Lorentz projection;
- `foundry::anchored` owns concrete-index rule derivation, while
  `foundry::parametric` owns fixed-sector `K(n)` elimination, uniform descent,
  exact symbolic and concrete-specialization replay, and guards/provenance;
  both expose
  a requested-pivot variant backed by deterministic Symbolica RREF;
  `foundry::dependency` owns exact target-sector partition work admission and
  compact, resumable proper-subsector obligation descriptors;
- `foundry::artifact` owns the versioned immutable closed-artifact value and
  currently admits the freshly generated canonical one-loop and equal-mass
  two-loop unit-mass vacuum partitions, with deterministic bounded durable
  encoding and one-time authenticated load/replay;
- `reduction` owns the topology-independent deterministic memoizing applier,
  exact typed-master decompositions, resource limits, termination checks, and
  common-mass restoration; and
- `campaign` owns resource profiles, execution-width preflight, and bounded
  ordered parallel execution.

The core therefore has two genuine durable closing artifacts and a reusable
scalar-IBP reducer. It still has no master substitution, generic tensor
kinematics, higher-even-rank projector, or closed three- or higher-loop family.
The Rust API may change directly as these owners and callers are extended;
obsolete facades are not retained for compatibility.

## Shared application API

`rustred-app` exposes transport-neutral derivation, campaign, artifact, and
reduction operations, including the two bounded foundry diagnostics:

```rust
derive(DeriveRequest) -> Result<DeriveResult, AppError>
campaign_plan(CampaignPlanRequest) -> Result<CampaignPlanResult, AppError>
campaign_preflight(CampaignPreflightRequest)
    -> Result<CampaignPreflightResult, AppError>
foundry_campaign_run(FoundryCampaignRunRequest)
    -> Result<FoundryCampaignRunResult, AppError>
foundry_wave_campaign_run(FoundryWaveCampaignRunRequest)
    -> Result<FoundryWaveCampaignRunResult, AppError>
closing_artifact_generate(ClosingArtifactGenerateRequest)
    -> Result<ClosingArtifactGenerateResult, AppError>
closing_artifact_inspect(ClosingArtifactInspectRequest)
    -> Result<ClosingArtifactInspectResult, AppError>
closing_artifact_reduce(ClosingArtifactReduceRequest)
    -> Result<ClosingArtifactReduceResult, AppError>
```

The strict foundry-config V2 schema is a sum type with two construction paths.
`mode = "autonomous"` accepts no caller-authored proof order, proposal order,
probe portfolio, domain queue, or itinerary; the selected application entry
point and RustRed preset derive that deterministic program. `mode =
"external-hints-only"` requires a typed `[hints]` object and may choose only
the supported non-authoritative itinerary, proof/proposal order, probe, and
resource inputs. Unknown fields are rejected, and neither shape can represent
an imported rule, recurrence RHS, coefficient, source row, or support. The
report-only provenance ID is derived from the successful construction path,
never accepted as a free label. RustRed derives and replays all identities
itself. Single-sector results remain diagnostic-only. A complete full-wave
result crosses the separate installation boundary, deterministically encodes
and cold-reloads the artifact, and owns its canonical durable bytes; an
incomplete result owns no artifact bytes. Full-wave diagnostics retain a
detached report for every sibling that blocks atomic wave publication,
including its typed stop, exact residual-box census, the caller-bounded box
coordinates for that sibling, and an explicit truncation bit.

`derive` parses and lowers one family and emits selected raw parametric
ordinary and/or LI relations. A concrete target in the input is validated and
reported, not reduced. `campaign_plan` authenticates and interns only supplied
roots; it does not discover dependencies or prove closure.
`campaign_preflight` computes a topology-neutral memory-limited execution
width and does not start workers. Closing-artifact generation accepts the
semantic `unit-mass-vacuum-k1` and `unit-mass-vacuum-k3` family selectors and
owns deterministic durable bytes. Inspection and reduction require those
bytes and decode/authenticate them exactly once; they never substitute a
hidden preset. The `K = 3` loader cold-regenerates its tagged derivation and
requires byte-exact equality at that one untrusted boundary. Reduction returns
an ordered exact decomposition keyed by typed master power vectors plus
common-mass-squared homogeneity powers.

Rust callers that accept untrusted artifact bytes may use
`ClosedArtifact::decode_durable_with_limits` with `ArtifactLoadLimits`. Its
public `cover_replay: ArtifactCoverReplayLimits` member independently bounds
arity, requested boxes and coordinate cells, uncovered boxes and coordinate
cells, and exact-cover split operations. K6 decoding also applies the existing
translated-source and rule-cell limits before retaining cell-plan payloads.
These checks belong only to cold loading; a successfully sealed artifact does
not repeat them during memoized reduction.

Each result owns a canonical, newline-terminated TOML document accessible
through `to_toml()` (and, where appropriate, `into_toml()`). The generation
result additionally owns its durable `Vec<u8>`. Application errors retain
typed input, schema, resource, lowering, derivation, execution, license,
serialization, output-limit, and internal categories.

## Command line

The binary is `rustred`, supplied by `rustred-app`:

```text
rustred derive [OPTIONS]
rustred campaign plan [OPTIONS]
rustred campaign preflight [OPTIONS]
rustred campaign run --config <PATH|-> --output <PATH|-> [OPTIONS]
rustred campaign run-waves --config <PATH|-> --output <PATH|-> --n-cores <N> [OPTIONS]
rustred campaign generate \
  --family <unit-mass-vacuum-k1|unit-mass-vacuum-k3> [OPTIONS]
rustred campaign inspect --artifact <PATH|-> [OPTIONS]
rustred campaign reduce --artifact <PATH|-> --powers <N,...> [OPTIONS]
```

`derive` accepts `--input-format auto|toml|symbolica`,
`--relations all|ordinary|li`, and a positive `--n-cores`. Campaign planning
accepts an optional root identifier. Campaign preflight requires a resource
profile and an explicit positive memory limit. Input and output default to
standard streams; file output requires `--force` to replace an existing file
and is committed atomically.

`campaign run` executes one bounded single-sector K6 diagnostic campaign.
`campaign run-waves` executes the bounded full-rank atomic-wave itinerary and
requires an explicit positive sibling-worker count. Both consume the strict
V2 campaign configuration, produce diagnostic reports, and distinguish
autonomous requests from external search hints structurally. A completely
published `run-waves` result additionally exposes canonical artifact bytes only
after exact installation, deterministic encoding, and one cold reload; an
incomplete result exposes none. Neither command accepts or imports recurrence
algebra.

`campaign generate` writes binary durable bytes. Inspection and reduction read
those bytes from a file or standard input and emit canonical TOML. Invalid
bytes are rejected before output begins. The three-loop `K = 6` selector is not
available yet.

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
generated = rustred.generate_closing_artifact(
    family=rustred.ClosingFamily.UNIT_MASS_VACUUM_K1,
)
inspection = rustred.inspect_closing_artifact(generated.artifact)
reduction = rustred.reduce_with_closing_artifact(generated.artifact, [3])
```

The result classes expose `schema`, `status`, and `to_toml()`. Generation also
exposes immutable `artifact: bytes`; reduction exposes typed exact master
terms. Public exception classes mirror the application error categories.
`rustred._rustred` is a private extension implementation detail; top-level
`_rustred` is not the user API.

Long-running calls release the GIL and pass through one process coordinator.
If an internal panic crosses that boundary, the coordinator is poisoned and
later requests fail instead of reusing uncertain native state. A coordinator
created before `fork()` is likewise rejected in the child.

## Vakint tensor boundary

Vakint/GammaLoop development lives in its separate repository on the
`vakint_rustred` feature branch, rebased onto `feynkit`. Tensor-bearing RustRed
lanes explicitly select `TensorReductionMethod::FeynKit`; RustRed itself does
not provide or extend the Stage 1 tensor reducer. Existing FORM-backed Vakint
methods and syntax remain backward-compatible alternatives and oracle lanes.

## Active Stage 1 Vakint backend

The new additive interface provides a scalar evaluation backend, separate
from tensor mode selection:

```rust
EvaluationMethod::RustRed(RustRedEvaluationOptions::default())
EvaluationOrder::rustred_only()
```

`RustRedEvaluationOptions` controls optional master substitution, enabled by
default. The backend currently supports the registered one-loop tadpole,
two-loop sunset, and pinch. It consumes the topology match and simultaneous
routing witness already produced by Vakint, loads the corresponding shipped
immutable artifact once, applies guarded rules through RustRed, and returns
exact coefficients of typed master keys mapped to Vakint's existing MATAD
master basis. It reuses Vakint's pure-Rust master values when substitution is
requested, reports no FORM dependency, invokes no FORM scalar reduction, and
never falls back internally. Unsupported graph classes remain unsupported so
mixed orders can continue safely to their next configured method.

Tensor-bearing inputs use FeynKit's FORM-less tensor prepass before this
FORM-less scalar tail. The complete lane is tested with an invalid FORM path;
AlphaLoop/MATAD comparisons run separately with the pinned FORM 5 oracle.

Production `K = 1` and `K = 3` artifacts are generated once by RustRed, checked
into and shipped with Vakint, validated once when lazily loaded, and reused for
ordinary evaluation. The same ownership applies to the pending `K = 6`
artifact. Vakint does not regenerate them or maintain topology-authored
recurrence code.

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
- backward compatibility for its public API conventions, defaults, and all
  existing FORM-backed modes and legacy integral notation, while accepting any
  newer notation additively, but not for obsolete RustRed artifact schemas.

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

## Stage 1 fine-grained surfaces

The Rust application, CLI, and Python package now expose durable generation,
inspection/replay, and guarded memoized reduction for the closed `K = 1` and
`K = 3` families. The same service boundaries will extend to the remaining
work:

- family construction and raw IBP/LI generation;
- closure campaigns for `K = 6`;
- a durable artifact and reduction to stable master keys for that family;
- the additional symmetry/factorization and lower-artifact routing it requires;
  and
- supplied master-value validation and substitution.

These interfaces remain useful for arbitrary non-vacuum families. The Vakint
vacuum artifact library is an optimized deployment of the generic services,
not the limit of RustRed's family model. Tensor API expansion is not part of
these Stage 1 surfaces.
