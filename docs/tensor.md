# Tensor reduction and Vakint integration

[`GOAL.md`](../GOAL.md) is the sequencing and capability authority. This
document preserves the stable implementation contract for Track B: a generic
RustRed tensor service, its optimized single-scale vacuum lane, and the thin
Vakint adapter that will call it. It records the live frontier and the
remaining implementation contract.

## Current frontier

RustRed now exposes a first cohesive public `tensor` service. It authenticates
caller-supplied Symbolica heads, admits `Auto` and explicit vacuum execution
only through sealed `SingleScaleVacuumEvidence`, and keeps key-aware
projection, family-aware scalar-product lowering, and their composed reduction
as separate typed operations. Projection requires an integral key and rejects
nonzero auxiliary powers or power shifts before applying vacuum parity or
isotropy. The live production slice supports scalar terms, exact
odd-internal-rank zero, and the global rank-two isotropic projector. Its custom
head sentinel exactly lowers `dot(k,p) k(mu)` on the one-denominator base key
`[1]` to the keys `[0]` and `[1]` with coefficients `1/d` and `-m^2/d`, while
retaining the exact polynomial guard `d != 0`.

Ingress is deliberately bounded to root sums of explicit products. Reserved
tensor heads may occur only as arity-two indexed loop/external vectors,
metrics, or scalar products; every other factor is retained as an opaque
scalar Atom only when it contains no configured nonnumeric loop-momentum
label. Numeric momentum IDs acquire that meaning only inside reserved tensor
heads, so ordinary numeric arguments of opaque scalar functions remain
scalars. Tensor-bearing powers and nested tensor sums remain unsupported,
as do even rank above two. Shared Lorentz indices involving retained metrics or
external vectors are rejected until native contraction/canonicalization is
implemented. The `Generic` lane returns the typed
`UnsupportedGenericKinematics` boundary rather than selecting a vacuum
projector or another backend. Scalar lowering currently also requires every
auxiliary/ISP base-key power and auxiliary family power shift to be zero;
otherwise numerator content would be invisible to the bounded tensor grammar,
so both positive and negative auxiliary powers are typed unsupported results.

In the independent GammaLoop repository, branch `vakint_rustred` still has
only an additive dispatch seam:

- `TensorReductionMode::Form` remains the default and preserves existing
  Vakint behavior;
- `TensorReductionMode::RustRed(RustRedOptions)` reaches a distinct adapter;
- that adapter currently returns
  `TensorReductionError::RustRedUnavailable`; and
- selection of the RustRed mode never calls or falls back to FORM.

Those current Vakint tests prove dispatch and backward-compatibility boundaries
only; RustRed's native core sentinel is tested independently. A test that
intentionally executes Vakint's existing FORM backend belongs to a separately
declared compatibility or oracle job; it is not a RustRed-mode test.

The first service described below now lives in the `rustred` core package.
Vakint, `rustred-app`, the CLI, and Python will adapt to that one service rather
than carry independent projectors or scalar-lowering algorithms.

## Ownership and vertical seam

GammaLoop's current BPHZ path already performs the forest construction and
turns each connected local counterterm into a Vakint expression. Its relevant
sequence is:

```text
GammaLoop local counterterm Atom
  -> to_vakint_integrand(..., substitute_masses_to_m_uv = true)
  -> Vakint topology matching and canonical routing
  -> tensor reduction
  -> scalar integral reduction and optional master evaluation
  -> GammaLoop metric, dimension, normalization, and series postprocessing
```

The non-BPHZ analytical-evaluation caller uses the same middle sequence. The
new mode replaces the tensor and scalar-reduction mathematics without
duplicating GammaLoop's forest construction or Vakint's topology registry.

Vakint owns:

- parsing and splitting a user expression into `VakintTerm` values;
- `Topologies::match_topologies_to_user_input`, graph matching, canonical
  routing, pinches, and the simultaneous numerator routing map;
- backend selection, user configuration, orchestration, and result rendering;
  and
- backward compatibility for the existing FORM-backed mode.

RustRed owns:

- authentication of the matched family presentation;
- Lorentz projection and family-aware scalar-product lowering;
- exact guards, resource admission, and deterministic results; and
- later, guarded artifact application and stable master output.

The adapter must remain thin. It must not copy Vakint's topology tables into
RustRed or implement a second tensor projector in GammaLoop.

## Tensor expression contract

Vakint's boundary vocabulary is:

```text
k(loop_id, lorentz_index)
p(external_id, lorentz_index)
g(left_index, right_index)
dot(left_momentum, right_momentum)
```

`g` and `dot` are symmetric, and `dot` is linear in its momentum arguments.
Lorentz indices are arbitrary Symbolica Atoms, not integer-only labels. User
coefficients, namespaced functions, complex constants, regulator dependence,
external vectors, free metrics, and other scalar spectators must survive.

The RustRed core must not hard-code the Vakint namespace. Its request carries a
caller-supplied head vocabulary or an equivalent typed tree; the Vakint adapter
maps `k`, `p`, `g`, and `dot` into it. This also makes the service usable from
the Rust CLI, Python, and other Symbolica callers.

Ingress follows these rules:

- split tensor-bearing sums and expand nonnegative integer powers only under a
  checked term, depth, rank, and retained-memory budget;
- leave scalar-only factors opaque rather than widening the coefficient field;
- accept both indexed vectors and proven scalar `dot` expressions;
- allow a negative integer power of a proven scalar dot product to remain a
  rational spectator;
- reject a negative, noninteger, or symbolic power whose base contains a
  reducible tensor;
- reject unknown use of reserved tensor heads instead of silently treating it
  as a scalar; and
- preserve the distinction between loop vectors, external spectator vectors,
  metrics, and scalar functions throughout projection.

Before creating any dummy index, the request is censused for every existing
index Atom, including decorated and namespaced function-valued indices. Fresh
indices come from a deterministic private namespace disjoint from that census.
Canonical dummy renaming happens only after reconstruction, so generated
indices cannot capture caller indices or alter opaque spectators.

`VakintTerm::vectors` is a convenience cache, not authenticated evidence. The
adapter recomputes or validates vector occurrence from the actual numerator.

## Topology matching and routing

Vakint's existing matcher remains the steering authority. It handles short and
full topology syntax, arbitrary edge/node/loop identifiers, edge orientations,
masses, propagator powers, pinches, and deterministic selection among graph
automorphisms. Its contraction generation and loop-basis construction already
use graph canonization and exact Symbolica solves. Extension through six loops
must improve that generic engine and its manifest coverage rather than bypass
it in RustRed.

Routing substitutions must be simultaneous. A map such as `a -> b, b -> c`
is one basis permutation and must not cascade into `a -> c`. Vakint currently
uses one multiple-replacement operation for its numerator map; the RustRed
adapter must preserve the same observable rule for integral routings,
numerators, and scalar products.

After routing, the boundary rejects any source loop variable or free variable
from an underdetermined momentum solve that remains in either the canonical
integral or the rewritten numerator. Checking only the denominator topology is
insufficient. Vakint owns that source-side routing proof. The family
presentation retains its resulting exact map as caller-attested metadata and
checks coefficient domains, shape, loop unimodularity, and external
invertibility without relabeling it a second topology-match proof.

Concrete names such as `I1L`, `I2L`, `H`, or `BMW` are fixtures and Vakint
presentation data. RustRed family presentations exactly replay physical rows
and common-scale claims, retain auxiliary roles, and structurally admit the
attested routing/convention map; they never dispatch on those names.

## Authenticated family presentation and lanes

The adapter presents matched data; RustRed mints the proof used for lane
selection. The presentation retains at least:

- loop and external momentum order and caller-attested exact routing map,
  after structural/unimodularity/invertibility checks;
- dimension, metric signature, and denominator sign. Loop-measure
  normalization remains caller/Vakint-owned and is deliberately outside this
  tensor/IBP presentation; future artifacts must map it explicitly at their
  boundary;
- every physical denominator's affine scalar-product row, mass squared,
  external shift, and family-owned power shift; target powers remain owned by
  the tensor/reduction request's integral key and are never duplicated in the
  presentation;
- every auxiliary denominator/ISP and its role, rather than flattening it into
  the physical propagator list;
- external Gram data and family-domain nonzero conditions; and
- common-scale evidence for physical denominators. A separate future proof is
  required for whole-family unit-scale homogeneity, including auxiliary rows
  and powers.

The live presentation contract retains exact nonzero guards for every
presentation-coefficient denominator and for a symbolic common scale's
numerator. The affine-family fingerprint alone is not yet a presentation
cache key; callers must retain the full presentation metadata until a
versioned presentation fingerprint is introduced.

The tensor selection contract has three choices:

- `Auto` uses only RustRed-authenticated semantic evidence;
- the optimized vacuum lane accepts a common nonzero single scale and no
  external shift in any physical denominator; and
- the generic external-kinematics lane initially returns a typed unsupported
  result until it is genuinely implemented.

External spectator vectors or free Lorentz indices in the numerator do not
invalidate vacuum admission. An external momentum shift inside a physical
denominator does. Neither Vakint topology names nor a literal loop-count test
can mint admission.

After projection, scalar products are lowered generically through the family's
authenticated inverse affine denominator/ISP map. This step performs exact
propagator cancellation and emits typed integral keys; it never uses a list of
two-, three-, or six-loop scalar-product formulas.

## Global isotropic projector

Vacuum tensor reduction uses one global Lorentz projector across all loop
vectors. Independent per-loop angular averages are incorrect for mixed-loop
tensors.

For each tensor monomial the planned kernel:

1. contracts unambiguous metric chains and already paired internal indices;
2. extracts the ordered internal loop-vector slots and the outside tensor;
3. returns exact zero for odd internal rank;
4. enumerates perfect pairings for even rank;
5. quotients pairings by permutations of slots carrying identical loop-vector
   labels;
6. classifies relative pairings by the full alternating-cycle partition of
   their overlay, not only by the number of cycles;
7. constructs the quotient Gram/projector matrix, where every closed Lorentz
   cycle contributes one power of symbolic dimension `d`;
8. constructs the contracted scalar right-hand sides by replacing each paired
   slot pair with the corresponding scalar product;
9. solves the quotient system exactly over a Symbolica rational-function
   field; and
10. reconstructs orbit sums with caller-supplied heads and canonical dummy
    indices.

RustRed owns pairing/orbit enumeration, slot meaning, shapes, cache identity,
and resource admission. Symbolica owns exact coefficient extraction,
rational-polynomial operations, matrix solving, simplification, and supported
tensor canonicalization. Adoption of each Symbolica primitive follows the
[algebra gate](algebra.md); a missing public primitive produces a typed
unsupported result rather than a handwritten replacement CAS.

Projectors are cached by mathematical data such as rank, loop-label
multiplicity partition, orbit convention/algorithm revision, dimension
coefficient context, and head-independent normalization. They are never cached
by topology name or concrete numerator. The universal pairing-overlap
coefficients at ranks 2, 4, 6, 8, and 10 fall into respectively 1, 2, 3, 5,
and 7 alternating-cycle classes. These counts are oracle sentinels, not a
shipped table or a ceiling; higher rank is limited only by explicit resource
admission.

Every denominator introduced by the exact solve remains a nonzero guard. This
includes familiar special-dimension factors such as `d`, `d - 1`, or `d + 2`.
A singular dimension is a typed guarded-domain result, not permission to cancel
the locus or use a numerical sample.

The first standalone sentinel uses custom heads and proves

```text
dot(k,p) * k(mu) / (k^2 + m^2)
  -> p(mu)/d * [I(0) - m^2 I(1)]
```

with the guard `d != 0`. Rank-four equal-vector projection supplies the first
nontrivial orbit sentinel, with denominator `d(d + 2)` and the three metric
pairings.

## Conventions and unit-scale specialization

Vakint's full propagator spelling is semantically

```text
prop(id, edge(source,sink), momentum, mass_squared, power)
```

The mass field is a squared mass and a positive power is a denominator power.
The alphaLoop oracle uses Minkowski denominators `q^2 - m^2`, whereas a
Euclidean family may use `q_E^2 + m^2`. Metric, sign, mass-squared, index order,
and the map between conventions enter the family/artifact identity. Setting the
scale to one erases none of them.

GammaLoop already multiplies its configured per-loop
`additional_normalization` during `to_vakint_integrand`; RustRed must not apply
it again. GammaLoop also owns its final metric conversion and substitution
`d = 4 - 2 epsilon`. Vakint owns any measure, MS-bar, epsilon-series, and master
value normalization. RustRed keeps `d` symbolic through projection and returns
exact scalar/master coefficients before those layers.

For `L` loops with one common nonzero squared mass `s` and denominator powers
`a`, the admitted homogeneity relation is

```text
I(a; s) = s^(L*d/2 - sum(a)) I(a; 1).
```

Consequently a unit-scale rule coefficient is restored by

```text
c[a -> b](s) = s^(sum(b) - sum(a)) c[a -> b](1).
```

This includes negative auxiliary powers representing scalar numerators. The
specialization is accepted only after exact row/rule homogeneity replay and a
proof that the shared scale is nonzero. It reduces hot coefficient work; it
does not reduce the index dimension, tensor rank, sector count, or exceptional
domain.

## Validation ladder

Evidence is reported at three distinct boundaries, as detailed in
[`validation.md`](validation.md):

1. **Tensor:** exact projected Atom after canonical head and dummy-index maps,
   before scalar IBP application.
2. **Reduction:** exact rational coefficients multiplying unsubstituted stable
   master keys, after explicit convention and master-basis maps.
3. **Evaluation:** optional measure/master substitution and Laurent or numerical
   comparison.

A higher-layer agreement cannot compensate for a lower-layer failure. The
staged oracle programme is:

- one loop: scalar, odd rank, free/external/mixed rank two and four, arbitrary
  index Atoms, opaque spectator heads, powers, the custom-head sentinel, and
  byte-stable `n_cores = 1, 2, 4` results;
- two loops: sunset powers and pinches, alternate loop bases, simultaneous
  numerator routing, mixed-loop tensors, and exact unsubstituted masters;
- three loops: alphaLoop/MATAD scalar, numerator, routing, and rank-four
  fixtures; and
- four loops: `H`, `X`, `BMW`, and `FG` inputs compared against frozen raw FMFT
  `PR`-master combinations before master expansion.

Current Vakint expected expressions and frozen outputs may be consumed without
FORM. Regeneration may run the existing FORM-backed alphaLoop, MATAD, or FMFT
paths only in a segregated, pinned oracle job recording executable identity,
input, conventions, Vakint revision, and raw output. RustRed, its tests,
Vakint's RustRed mode, and RustRed-mode tests never invoke FORM, load FORM rule
tables, or fall back to FORM.

Every accepted tensor result also passes malformed-head, unknown reserved
syntax, index-capture, singular-dimension, allocation-limit, and one-below-limit
tests. Unsupported generic kinematics, missing family evidence, routing
residue, or exhausted resources remain typed failures; none is converted to a
zero, a master, or a FORM request.

## Source anchors

The live implementation anchors in the independent reference repository are
Vakint's `src/tensor_reduction.rs`, `VakintTerm`, `VakintExpression`,
`Topologies::match_topologies_to_user_input`, graph contraction/canonization,
and GammaLoop's `to_vakint_integrand` and `Integrated::integrate`. The existing
`tensorreduce.frm`, `pvtab10.h`, `integrateduv.frm`, MATAD, and FMFT sources are
oracle evidence only. Their rank tables and topology-authored recurrences must
never enter RustRed production code or shipped rule artifacts.

See also the [architecture](architecture.md), [interface ownership](interfaces.md),
[foundry design](foundry.md), and [LiteRed2 semantics](references/litered2.md).
