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
- bounded scalar, odd-rank, and global rank-two vacuum tensor projection;
- separate, artifact-bound lowering of already scalarized polynomial vacuum
  numerators onto shifted integral keys, with exact affine coefficients and
  explicit common-mass powers;
- concrete-anchor and fixed-sector parametric foundry boundaries that derive
  guarded strictly descending rows through Symbolica's sparse reducer; the
  latter eliminates directly over `K(n)` and requires exact symbolic replay,
  uniform descent, and exact concrete base-field replay, while both boundaries
  can target a requested pivot through guarded deterministic RREF; the
  targeted path can additionally classify a maximal parent-sector box into
  exact fixed-target-sector product cells and expose a preflighted, resumable
  stream of compact rule-bound proper-subsector obligation descriptors;
- a versioned immutable artifact owner whose current verifier freshly
  generates and seals the mathematically complete canonical one-loop `q^2-1`
  and equal-mass two-loop sunset vacuum partitions, plus a deterministic
  bounded durable codec whose untrusted load boundary authenticates complete
  source, cell, projection, symmetry, factorization, terminal, and homogeneity
  semantics once;
- a topology-independent deterministic memoizing reducer with canonical
  symmetry routing, guarded cells, lower-artifact factorization, explicit
  master and zero terminals, concrete strict-descent checks, retained-payload
  limits, and common-mass homogeneity restoration;
- sector masks, restrictions, deterministic ordering, verification of
  caller-supplied momentum maps, verified denominator permutations, and an
  on-demand sufficient zero-sector rank test; and
- deterministic ordered work execution, resource-profile validation, campaign
  width preflight, roots-only campaign planning, and canonical TOML output
  through Rust, CLI, and Python application surfaces.

The application `derive` path still emits raw parametric identities. The
generic fixed-sector foundry exposes exact translated-source, elimination,
guard, residual-projection, and dependency primitives; an artifact owner must
compose those primitives with complete cell coverage, symmetry, zero,
factorization, and terminal proofs before claiming closure. The installed
`K = 1` and `K = 3` owners do so, including immutable lower-artifact feedback
for the sunset pinch. No application service substitutes or numerically
evaluates masters. The core codec publishes and loads both artifacts, and
`reduction::Reducer` applies their guarded cells with canonical symmetry and
memoized strict descent. The Rust application, CLI, and Python transports use
those same durable bytes. The tensor slice does **not** support generic
kinematics or even rank above two and is frozen during Stage 1. Generic
three- and six-loop source-count tests exercise only row censuses; they are not
physical closure results.

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

The ten current core domains are rooted at
[`crates/rustred-core/src`](../crates/rustred-core/src):

```text
input ------> family ------> algebra ------> Symbolica public Rust API
   |             ^
   +-------------+

identity ---> family, algebra
sector -----> family, algebra
tensor -----> family, algebra, Symbolica public Rust API
foundry ----> identity, sector, family, algebra, Symbolica public Rust API
reduction --> foundry, sector, family, algebra
scalar_numerator --> foundry, family, algebra, Symbolica public Rust API
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
rule publication. A bounded translated-source boundary accepts only one of
those sealed complete batches, binds its sealed family and indexed-context
scope, canonicalizes arbitrary `IntegralShift` offsets, and returns immutable
exact translations with stable source-row/offset provenance. It uses the
existing Symbolica-backed `n -> n+a` algebra and checked lattice-shift
addition; it is source preparation for the generic foundry, not a recurrence
or closure claim.

### `foundry`

[`foundry`](../crates/rustred-core/src/foundry/mod.rs) owns rule discovery and
its proof-bearing result values. The anchored boundary specializes supplied
relations at one integer anchor. The parametric boundary eliminates them
directly over `K(n)` on a representable fixed-sector interior. Both order
integral columns through the sector owner and invoke Symbolica's public sparse
row reducer; the latter retains every required guard and source provenance,
proves uniform strict descent, exactly replays the symbolic row, and requires
an exact specialization replay of the retained source combination at the
declared anchor. Targeted discovery performs
physical-pivot-only deterministic back-substitution while retaining identity
columns as free source weights. Those general rule-discovery paths are not yet
an exceptional-domain engine or complete-source/closure search.
`foundry::search` supplies two smaller topology-neutral primitives required by
that engine. `SectorSearchDiamond` plans a bounded exact-sector L1 neighborhood
around one concrete integral, with exact count/storage preflight and
deterministic lexicographic offsets. `ReachabilityPlanner` binds an ordered set
of authenticated rule cells and follows their exact nonzero concrete
dependencies from a finite root set, including guard fallthrough, terminal
precedence, strict descent, optional symmetry routing, deterministic
uncovered-frontier reporting, and typed work budgets. The former does not
perform translation or elimination, and the latter is a bounded dependency
census rather than an infinite-domain closure proof. Applicability refinement
and symbolic closure remain with their existing owners. The targeted
parametric API can additionally retain a maximal parent-
sector domain with compact first-pinched witnesses and exact fixed-target-
sector product partitions. `foundry::dependency` owns aggregate work admission
and a stable process-local cursor over O(1) proper-subsector descriptors; exact
cell domains are materialized only on demand. These values retain rule,
coefficient, and guard context but do not refine applicability or feed solved
children back. Separately, `foundry::artifact` generates and verifies the
canonical `K = 1` and `K = 3` closures and seals them for `reduction`; the
reusable primitives are topology-neutral, while these first complete
partition verifiers are registered family manifests rather than a generic
closure search. Its schema-v3 codec owns deterministic semantic bytes and
one-time bounded untrusted loading. It reconstructs tagged complete-ordinary
source plans under explicit family/generator/rule policies, compares retained
semantics exactly, and authenticates replay before exposing a sealed owner.
The sunset pinch additionally retains and replays a unimodular loop-basis
certificate proving its denominator blocks factor into immutable `K = 1`
dependencies. Product-sector application is generic over multi-master lower
families: installation compiles the complete finite Cartesian product of typed
dependency masters into authenticated canonical parent terminals, and the
runtime performs deterministic exact convolution without repeating that
authentication. Installation and runtime both bound Cartesian product
cardinality before retaining terms.

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
they may not recognize a family label or a literal loop-count condition.
Concrete topologies belong in fixtures, external oracle corpora, benchmarks,
or shipped artifact metadata.

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
`vakint_rustred` branch. Vakint remains the user-facing steering layer, while
RustRed supplies reusable mathematical services. The additive scalar RustRed
backend now ships and consumes the sealed `K = 1` and `K = 3` artifacts for the
registered one- and two-loop families. It provides a FORM-free scalar tail
while deliberately retaining Vakint's existing FORM tensor prepass.
Tensor-bearing end-to-end tests may therefore execute FORM before entering
that tail; separate invalid-FORM-path scalar tests prove that the backend itself
has no FORM dependency or fallback. Existing FORM-backed scalar methods remain
compatibility oracles in segregated coverage.

## Stage 1 artifact and reduction owners

The live core now contains two cohesive Stage 1 domains:

| Owner | Current responsibility | Remaining production gate |
|---|---|---|
| `foundry::artifact` | Versioned immutable `K = 1` and `K = 3` artifacts; exact source/rule/projection replay; guarded exceptional cells; `S3` symmetry; zero, factorization, lower-artifact, master, and homogeneity proofs; deterministic bounded encoding/loading with one-time authentication at the untrusted boundary. A test-only `K = 6` pressure fixture owns its exact family, nine sources, `S4` sector orbits, revision-stamped internally checked Vakint class snapshots, certified `K3 x K1` plus both inequivalent `K1 x K1 x K1` factorization sectors, a guarded top-sector recurrence, both positive dotted-edge cells and six negative-inactive-power endpoint/bulk cells on the canonical five-line residual face, guarded canonical-dot/mixed-numerator slices on the irreducible four-line face, and exact four-line decorated cells. Complete depth-one searches derive disjoint endpoint/bulk cells for the full-i64 scalar four-line inactive-numerator ray and continue one pinched child through one certified decorated-path `S4` orbit and its undotted descendant; inequivalent path-numerator orbits remain uncovered. A disjoint guard-free depth-zero cell covers the full-i64 one-dot/deeper-numerator ray `J(0,1,1,1,2,N)`, `N <= -2`, from three independently reprojected ordinary rows and routes only to those installed numerator lanes or the existing scalar corner. The complete untranslated nine-row span also supplies compact guard-free full-i64 endpoint/bulk cells for one factorized bridge-dot numerator orbit, terminating at products or strictly lower path numerators. A separate two-row singleton closes the bridge bulk's `J(-1,0,1,0,2,1)` descendant onto the installed decorated-path endpoint and a factorization terminal without growing the frontier. Two depth-one singletons under `four_line::factorized` close `J(0,-1,1,1,1,1)` and `J(0,-1,2,2,1,1)`; the latter compactly reprojects nine selected rows, removes a spurious complete-system guard, and terminates at four authenticated products while exhaustive placement tests prevent an orbit overclaim. A separate depth-one singleton under `four_line::numerator` owns only `J(0,1,2,2,1,-1)`'s incident two-dot/inactive-numerator orbit; five compactly reprojected rows remove the complete system's spurious guard and route to two installed positive-dot cells, one product, and the open scalar corner. The same semantic module adds two exact opposite-inactive-numerator-pair endpoints; their shared three-line incident dot/numerator child has a guard-free two-row endpoint terminating through an installed path cell and a product, so the three-cell cluster introduces no open descendant. The adjacent-pair and first triple-dot cells each come from a complete 28-offset/252-row same-sector diamond, retain separate full projection replay, select 16 exact source rows, and reduce only to a certified spanning-tree child plus the scalar corner. A third complete projection lowers the unique three-distinct-dot orbit through 17 selected rows onto the same children. A fourth complete projection derives the selected repeated-edge ray `J(0,1,1,1,N,0)`, `N >= 3`, from 50 source contributions and retains 32 guards with an exact generic-`d` proof over every positive free index. Two additional complete exact-corner projections lower the adjacent/opposite power-two/power-three orbits from 17/18 selected contributions, each only to the path factorization and scalar corner. A complete depth-three search over 84 translations and 756 rows selects 46 generated sources, whose independent one-index reprojection derives an algebraic recurrence for one `S4` orbit of `J(0,1,2,2,N,0)`, structurally `N >= 3`, with exact replay and a whole-ray guard proof; its concrete i64 cell stops at `N = i64::MAX - 1`. An independent complete depth-three projection closes only the first complementary-orbit point `J(0,1,2,3,2,0)` from 46 selected rows and retains exact singleton non-overclaim evidence. A test-only finite census exercises all 37 current cell owners and reports 86 exact reachable nodes with 13 uncovered obligations; it does not claim closure. All cells have exact provenance, guards, and strict descent without claiming closure. | Remaining mixed-dot and numerator rays, scalar-corner terminal installation, live Vakint matcher comparison, and the remainder of the `K = 6` rule fixed point, installer, durable codec, and complete five-class closure proof |
| `reduction` | Topology-independent deterministic guarded rule selection/application, canonical symmetry routing, lower-artifact factorization, memoization with retained-payload budgets, typed master maps, common-mass restoration, and shared Rust/CLI/Python application surfaces | The additional generic features demanded by `K = 6`; Vakint comparison through two loops is complete |

These are real owners rather than empty shells. Both closed artifacts cross a
durable byte boundary through one user-facing application layer.
Dependencies remain acyclic: the foundry composes current
mathematical domains and emits a sealed artifact value; reduction consumes
artifacts; artifact models do not depend on application transports.

## Change rules

- Put a responsibility under its mathematical owner; do not create
  `generated`, `residual`, `parametric`, `runtime`, `legacy`, or `misc`
  dumping grounds.
- Split large implementations along real value, algorithm, admission, error,
  and test boundaries. A small file count is not an architectural goal.
- Keep visibility minimal and expose public items under the owning module.
- Delete obsolete RustRed APIs directly; there is no pre-release RustRed
  compatibility promise. Preserve Vakint's public API conventions, defaults,
  and existing FORM-backed methods, but never add a Vakint decoder or migration
  layer for obsolete RustRed artifact schemas.
- Add a subcrate only when it creates a demonstrable dependency or independent
  build/test boundary.
- Describe capabilities from passing live evidence. A source count, proposed
  API, historical prototype, or readable oracle is not an implemented
  reduction or closure result.
