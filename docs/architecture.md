# Architecture

[`GOAL.md`](../GOAL.md) is the project objective and sequencing authority.
This document describes the live RustRed ownership boundaries. It separates
implemented services from planned product domains so that a directory name or
roadmap item is never mistaken for a capability claim.

## Current capability boundary

RustRed currently provides a topology-neutral mathematical spine for:

- compiling compact, structured-text, and caller-owned Symbolica input into a
  normalized project;
- lowering a complete affine scalar-product basis into an authenticated
  integral family;
- automatic ISP completion and construction of `U`, `F`, and
  Lee--Pomeransky/Symanzik data;
- exact ordinary parametric IBP rows and separate Lorentz-invariance rows over
  symbolic integral indices;
- authenticated physical/auxiliary family presentations and sealed semantic
  admission for a common-scale vacuum lane;
- bounded scalar, odd-rank, and global rank-two vacuum tensor projection, with
  separately callable affine scalar-product lowering onto integral keys;
- concrete-anchor and fixed-sector parametric foundry boundaries that derive
  guarded strictly descending rows through Symbolica's sparse reducer; the
  latter eliminates directly over `K(n)` and requires exact symbolic replay,
  uniform descent, and independent anchored agreement, while both boundaries
  can target a requested pivot through guarded deterministic RREF; the
  targeted path can additionally classify a maximal parent-sector box into
  exact fixed-target-sector product cells and expose a preflighted, resumable
  stream of compact rule-bound proper-subsector obligation descriptors;
- sector masks, restrictions, deterministic ordering, verification of
  caller-supplied momentum maps, verified denominator permutations, and an
  on-demand sufficient zero-sector rank test; and
- deterministic ordered work execution, resource-profile validation, campaign
  width preflight, roots-only campaign planning, and canonical TOML output
  through Rust, CLI, and Python application surfaces.

The application `derive` path still emits raw parametric identities. The new
core parametric slice requires a representable fixed-sector interior of a
caller-supplied source span. Its sector-monotone extension records boundary
pinches as unresolved lower-sector dependencies but does **not** feed them
back, refine exceptional guard-zero
domains, prove source-set completeness or closure, or publish an artifact. The
tensor slice does **not** support generic kinematics or even rank above two,
and no current service applies IBP artifacts, selects/substitutes masters, or
evaluates an integral. The six-loop source-count test exercises the generic row
census (`L = 6`, 21 coordinates, 36 ordinary sources); it is not a physical
six-loop closure result.

The exact public Rust surface is the module facade in
[`crates/rustred-core/src/lib.rs`](../crates/rustred-core/src/lib.rs). Public
items remain under their owning modules; the crate root is not a compatibility
re-export catalogue.

## Virtual workspace

The repository root is a virtual Cargo workspace: the
[`Cargo.toml`](../Cargo.toml) root has no `[package]`, and there is no root
`src/` or `tests/` tree. The three live packages are:

| Package | Canonical path | Responsibility |
|---|---|---|
| `rustred` | [`crates/rustred-core`](../crates/rustred-core) | Generic mathematical/domain services. Its directory name is `rustred-core`, but its Cargo package and library names are `rustred`. |
| `rustred-app` | [`crates/rustred-app`](../crates/rustred-app) | Typed application composition, deterministic transport models, bounded I/O policy, and the `rustred` CLI binary. |
| `rustred-python` | [`crates/rustred-python`](../crates/rustred-python) | Thin PyO3 adapter over `rustred-app`. The distribution is imported as `rustred`; `rustred._rustred` is a private native implementation detail. |

The repository-level [`pyproject.toml`](../pyproject.toml) is the maturin
authority. It maps the extension to `rustred._rustred` and packages the public
Python module from
[`crates/rustred-python/python/rustred`](../crates/rustred-python/python/rustred).

The package dependency direction is:

```text
rustred-python  --->  rustred-app  --->  rustred
       |                    |               |
      PyO3        transport/rendering   mathematical CAS
                            |               |
                            +-----> Symbolica <---+
```

`rustred-app` uses Symbolica only at its application boundary, for example to
render canonical expressions and report the resolved Symbolica version. It
does not own coefficient or matrix algebra. `rustred-python` owns no
mathematical representation or independent output schema.

## Core ownership DAG

The eight current core domains are rooted at
[`crates/rustred-core/src`](../crates/rustred-core/src):

```text
input ------> family ------> algebra ------> Symbolica public Rust API
   |             ^
   +-------------+

identity ---> family, algebra
sector -----> family, algebra
tensor -----> family, algebra, Symbolica public Rust API
foundry ----> identity, sector, family, algebra, Symbolica public Rust API
campaign ---> Rayon and Symbolica license admission
```

An arrow means “may depend on.” Same-domain child imports are omitted. There
is no reverse dependency from a mathematical value layer into input,
application transport, CLI, or Python.

### `algebra`

[`algebra`](../crates/rustred-core/src/algebra/mod.rs) owns the authenticated
base coefficient field, its index-extended field, checked exact operations,
and private matrix adapters. It knows nothing about integral families,
sectors, identities, campaigns, or frontends. Symbolica remains the algebra
authority; see [`algebra.md`](algebra.md).

### `family`

[`family`](../crates/rustred-core/src/family/mod.rs) owns authenticated family
kinematics, scalar-product coordinates, affine denominators, power shifts,
family-domain conditions, integral keys, deterministic fingerprints, ISP
completion, Symanzik polynomials, and authenticated family presentations.
The presentation layer exactly replays physical propagators, retains auxiliary
ISP roles, structurally validates caller-attested routing/convention metadata,
and mints sealed common-scale-physical vacuum evidence from semantic
properties. Source-side routing replay remains the topology matcher's proof;
the presentation does not manufacture it. Family construction is the semantic
boundary between a normalized declaration and reusable mathematical data.

### `input`

[`input`](../crates/rustred-core/src/input/mod.rs) owns untrusted expression
admission, parsing, normalization, affine-denominator compilation, and exact
lowering to `family::IntegralFamily`. Every supported frontend converges on
the same normalized `input::Project`. Transport metadata and TOML decoding
remain in `rustred-app`.

### `tensor`

[`tensor`](../crates/rustred-core/src/tensor/mod.rs) owns authenticated caller
heads, numerator-momentum labels, semantic lane selection, Lorentz projection,
and family-aware scalar-product lowering onto integral keys. Its first bounded
slice is the single-scale-vacuum scalar/odd/rank-two service; generic external
kinematics and higher even rank remain typed unsupported frontiers.

### `identity`

[`identity`](../crates/rustred-core/src/identity/mod.rs) owns sparse parametric
relations, shifts, exceptional-domain condition values, stable row identity,
and topology-neutral ordinary-IBP/LI generation. Prepared source batches have
stable ordinals; the application may execute independent rows in parallel and
then complete them in that order. Generation performs no sector solving or
rule publication.

### `foundry`

[`foundry`](../crates/rustred-core/src/foundry/mod.rs) owns rule discovery and
its proof-bearing result values. The anchored boundary specializes supplied
relations at one integer anchor. The parametric boundary eliminates them
directly over `K(n)` on a representable fixed-sector interior. Both order
integral columns through the sector owner and invoke Symbolica's public sparse
row reducer; the latter retains every required guard and source provenance,
proves uniform strict descent, exactly replays the symbolic row, and requires
agreement with the independently derived anchor. It is not yet an
exceptional-domain engine, complete-source/closure search, or artifact
publisher. The targeted parametric API can additionally retain a maximal
parent-sector domain with compact first-pinched witnesses and exact
fixed-target-sector product partitions. `foundry::dependency` owns aggregate
work admission and a stable process-local cursor over O(1) proper-subsector
descriptors; exact cell domains are materialized only on demand. These values
retain rule/coefficient/guard context but do not refine applicability, feed
solved children back, or establish closure.

### `sector`

[`sector`](../crates/rustred-core/src/sector/mod.rs) owns unshifted-index
sector masks, cuts/pattern exclusions, deterministic ordering, exact symmetry
verification, verified internal-permutation transport, and zero-sector proof
results. Cuts and patterns are exclusion metadata, not analytic zero proofs.
The current symmetry service verifies a supplied exact momentum map; graph
canonization and routing-candidate discovery are not implemented yet.

### `campaign`

[`campaign`](../crates/rustred-core/src/campaign/mod.rs) owns checked resource
values, calibrated execution profiles, RAM-aware width selection, and one
bounded ordered execution context. It does not know about roots, sectors,
closure, artifacts, or a solver. Roots-only interning and planning currently
live in the application layer because no reusable foundry plan exists.

## Application and process boundaries

The current derivation flow is:

```text
CLI or Python request
    -> rustred-app ingress and transport decoding
    -> input::Compiler and Project::into_lowered
    -> family::IntegralFamily
    -> identity::ParametricIbpGenerator
    -> campaign::ParallelExecution ordered batches
    -> rustred-app canonical result model and TOML
    -> CLI file/stdout or public Python value
```

The application enforces common input/output byte ceilings and owns error
classification so CLI and Python do not invent competing semantics. The CLI
performs all fallible work before writing output. The Python adapter releases
the GIL for application work and serializes top-level requests through one
process coordinator. A caught native panic permanently poisons that
coordinator, and a post-fork PID mismatch is rejected before reuse.

Trust is established at real boundaries: untrusted input, cross-process data,
and future durable artifacts. Inside a boundary, private fields, consuming
constructors, borrowed views, and sealed result types carry invariants.
Repeated schema round-trips or full proof replay between private functions are
not an architectural requirement.

## Topology neutrality

Production core algorithms receive loop momenta, external momenta,
denominator rows, integral coordinates, sector masks, graphs, and verified
maps as data. They do not dispatch on a topology name or embed a recurrence
for a named family. Loop count is a dimension of the input, not an algorithm
identity.

Optimized paths may be selected only from authenticated semantic properties.
The live vacuum tensor lane and future optimized foundry lanes may, for
example, recognize no external denominator shifts and a common nonzero scale;
they may not recognize a family label or the literal condition
`loop_count == 6`. Concrete topologies belong in fixtures, external oracle
corpora, benchmarks, or shipped artifact metadata.

Symbolica's graph implementation is the intended authority for future graph
canonization and automorphism candidate generation. RustRed will own the
physics-colored graph encoding and exact routing replay, not a second graph
isomorphism engine. This statement is a future ownership rule, not a claim
that graph-driven discovery exists today.

## Reference and repository isolation

The only tracked vendor gitlink is
[`vendor/symbolica`](../vendor/symbolica), the production CAS dependency listed
in [`.gitmodules`](../.gitmodules). The local
`FOR_REFERENCE_ONLY_DO_NOT_PUSH/` tree is ignored reference material. LiteRed2,
GammaLoop/Vakint, and readable external-oracle sources under that tree never
enter RustRed history, the Cargo workspace, or the production dependency
graph.

Vakint integration is developed in the GammaLoop repository on its own
`vakint_rustred` branch. Vakint remains the eventual user-facing steering
layer, while RustRed supplies reusable mathematical services. Existing
FORM-backed Vakint paths and their existing compatibility tests remain
reference-oracle coverage and should be segregated in CI. RustRed, Vakint's
RustRed mode, and tests of that mode never invoke FORM and have no FORM
fallback.

## Planned owners, not current packages

The following domains are required by the goal but do not exist in the live
RustRed core yet:

| Future owner | Intended responsibility | First capability gate |
|---|---|---|
| `artifact` | Immutable closed-rule values, manifests, persistence, and untrusted-load validation | A real closed shard produced and consumed outside the foundry |
| `reduction` | Guarded rule application, termination, stable master keys, and typed master substitution | End-to-end lower-loop application against independent Vakint expectations |

These owners are introduced only with a cohesive contract and first
production caller. Empty shells, restored prototype sessions, and transport-
only microcrates are not milestones. Future dependencies must remain acyclic:
the live foundry may compose the current domains and eventually emit stable
artifact values; reduction may consume artifacts; artifact models must never
depend on foundry internals.

## Change rules

- Put a responsibility under its mathematical owner; do not create
  `generated`, `residual`, `parametric`, `runtime`, `legacy`, or `misc`
  dumping grounds.
- Split large implementations along real value, algorithm, admission, error,
  and test boundaries. A small file count is not an architectural goal.
- Keep visibility minimal and expose public items under the owning module.
- Delete obsolete RustRed APIs directly; there is no pre-release RustRed
  compatibility promise. Preserve backward compatibility in Vakint.
- Add a subcrate only when it creates a demonstrable dependency or independent
  build/test boundary.
- Describe capabilities from passing live evidence. A source count, proposed
  API, historical prototype, or readable oracle is not an implemented
  reduction or closure result.
