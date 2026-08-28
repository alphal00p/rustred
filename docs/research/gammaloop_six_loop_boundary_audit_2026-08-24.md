# GammaLoop BPHZ to RustRed six-loop boundary audit

Date: 2026-08-24

Status: source audit and integration contract. This note does not claim a
six-loop reduction. It records the exact GammaLoop handoff, the useful
single-scale specialization, and the missing RustRed services required for a
two-stage rule-foundry/application architecture.

Audited sources are the working-tree RustRed checkout and vendored GammaLoop
revision `dce95392653aedeb10319c8d0897f4a2de3dbecd`. RustRed uses vendored
Symbolica revision `77c137481904b8a5531ede86e3ef36b82beed7fd`.

## 1. Exact replacement seam

GammaLoop already performs the BPHZ/forest construction. RustRed must not
duplicate it. For each connected operation node GammaLoop constructs a local
four-dimensional counterterm, then calls `Integrated::run`; disconnected
operation nodes are assembled from already integrated components
([`hedge_poset.rs:960-1017`](../../vendor/gammaloop/crates/gammalooprs/src/uv/hedge_poset.rs#L960)).

For the MUV and pole-part schemes, `Integrated::run` simplifies the local
counterterm and calls `Integrated::integrate`
([`integrated.rs:208-265`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L208)).
The latter performs exactly this sequence:

```text
GammaLoop local counterterm Atom
-> to_vakint_integrand(..., substitute_masses_to_m_uv = true)
-> VakintExpression::canonicalize
-> VakintExpression::tensor_reduce
-> VakintExpression::evaluate_integral
-> GammaLoop postprocessing and d = 4 - 2 epsilon
```

The three Vakint calls are adjacent at
[`integrated.rs:324-342`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L324).
This is the replacement seam. A RustRed integration should preserve the first
conversion and the later GammaLoop postprocessing, while replacing the
canonicalization/tensor/scalar-reduction middle with a native typed call.

There is a second non-BPHZ analytical-evaluation caller with the same sequence
at
[`amplitude.rs:1141-1161`](../../vendor/gammaloop/crates/gammalooprs/src/processes/amplitude.rs#L1141).
The adapter should be usable at both sites, even though the six-loop priority
is the BPHZ path.

## 2. What GammaLoop has already normalized

`to_vakint_integrand` is not a superficial printer. Before it returns, it:

1. undoes GammaLoop tensor shorthands and contracts exposed metrics;
2. converts every selected `Den` factor to a `prop` with an explicit power;
3. shrinks the dependent subgraph;
4. flips negatively routed edges and fuses equal-mass two-bonds;
5. rebuilds contiguous graph, edge, and propagator identifiers;
6. determines a loop-momentum basis and solves the old momentum variables in
   that basis with Symbolica's linear-system solver;
7. rewrites topology and numerator consistently;
8. applies GammaLoop's configured per-loop normalization; and
9. emits the Vakint tensor vocabulary `k`, `p`, `g`, and `dot`.

The main source blocks are
[`integrated.rs:559-704`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L559),
[`integrated.rs:724-870`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L724),
and
[`integrated.rs:893-1018`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L893).

The semantic return type is already useful:

```rust
pub struct VakintExpression(pub Vec<VakintTerm>);

pub struct VakintTerm {
    pub integral: Atom,
    pub numerator: Atom,
    pub vectors: Vec<(String, i64)>,
}
```

See
[`vakint/src/lib.rs:2182-2197`](../../vendor/gammaloop/crates/vakint/src/lib.rs#L2182)
and
[`vakint/src/lib.rs:2571-2617`](../../vendor/gammaloop/crates/vakint/src/lib.rs#L2571).
The `integral` is a single `topo(product(prop(...)))` Atom and `numerator` is
its exact Symbolica coefficient. The splitter itself uses Symbolica's
coefficient-list operation. Arbitrary scalar spectators and tensor structures
therefore remain separate from topology metadata.

RustRed should consume at least the `integral` and `numerator` fields. It
should recompute or validate vector identities rather than trusting
`VakintTerm::vectors`: GammaLoop rewrites the numerator after splitting, while
Vakint's current tensor reducer independently rediscovers vectors before use.

## 3. Conventions that the adapter must authenticate

The full topology spelling is

```text
topo(
  prop(id, edge(source,sink), momentum, mass_squared, power)
  * ...
)
```

The fourth field is a squared mass. GammaLoop initially associates a mass
with each converted edge and explicitly squares it when rebuilding the final
Vakint topology
([`integrated.rs:846-866`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L846)).
Positive `power` denotes a denominator power; conversion of an outer Atom
power negates the exponent at
[`integrated.rs:571-596`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L571).

The checked alphaLoop oracle uses the Minkowski convention

```text
D(q) = q^2 - mUV^2,
q^2 / D(q)^n = 1 / D(q)^(n-1) + mUV^2 / D(q)^n.
```

The second identity is literal source at
[`integrateduv.frm:17-25`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm#L17).
A RustRed campaign may instead normalize to Euclidean
`D_E(q)=q_E^2+m^2`, but that must be an explicit, tested conversion. The
family/artifact fingerprint must include Minkowski versus Euclidean metric,
denominator sign, mass-squared convention, loop-measure convention, and the
map back to GammaLoop. Setting `m^2=1` does not erase these distinctions.

GammaLoop applies `settings.additional_normalization^nloops` before returning
the terms; its default is `-1`
([`integrated.rs:958-969`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L958),
[`settings.rs:406-426`](../../vendor/gammaloop/crates/gammalooprs/src/uv/settings.rs#L406)).
RustRed must not apply this factor a second time. GammaLoop also owns the final
conversion of surviving external vectors and metrics and the substitution
`d=4-2 epsilon`
([`integrated.rs:359-435`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L359)).

The topology is vacuum, but the numerator may retain external spectator
vectors and free Lorentz indices generated by the local counterterm. Native
tensor projection must retain these covariant structures; it cannot treat the
whole term as a scalar vacuum integral prematurely.

## 4. What `m^2 = 1` buys

For an `L`-loop complete vacuum family with `K=L(L+1)/2` quadratic
coordinates, define

```text
I(a) = integral product_i D_i^(-a_i),
w(a) = sum_i a_i.
```

With one common nonzero mass,

```text
I(a; m^2) = (m^2)^(L*d/2 - w(a)) I(a; 1),
c_ab(m^2) = (m^2)^(w(b)-w(a)) c_ab(1).
```

This includes negative auxiliary powers representing scalar numerators. A
unit-mass campaign therefore has:

```text
parametric rule coefficients: Q(d, n_1, ..., n_K)
concrete specialized coefficients: Q(d)
```

instead of carrying `m^2` through every hot coefficient. At six loops this
removes one reconstruction variable from all 36 ordinary IBPs per seed,
allows univariate sampling/reconstruction in `d` after integer-index
specialization, shrinks rule artifacts, and makes one rule database reusable
for every nonzero choice of the auxiliary UV mass.

It does not reduce the 21-dimensional index space, the number of physical
sectors, tensor rank, numerator degree, graph count, or exceptional index
loci. It is a major coefficient-algebra simplification, not a substitute for
sector solving.

Mass restoration must remain outside the rule hot path and be checked by
homogeneity on every generated row and published rule. GammaLoop separately
tracks consumed loop measures as `integrated_loop_scale^(4 L)`, deliberately
independent of `mUV`
([`integrated.rs:53-113`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L53),
[`integrated.rs:259-264`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L259)).
Unit-mass reduction must not consume or conflate that marker.

## 5. Required two-stage architecture

### 5.1 Offline foundry

The foundry operates once per canonical family/sector, not once per
GammaLoop numerator:

```text
normalized physical topology
-> exact loop routing and complete 21-coordinate family
-> deterministic ISP completion
-> zero/factorization/symmetry quotient and sector DAG
-> all generic parametric IBPs
-> target-reachable modular pivot discovery
-> rational reconstruction in d and symbolic index variables
-> exact generated-row replay, guards, WhenBad, and coverage
-> content-addressed rule artifact
```

Each artifact needs family/routing/sign/unit-mass/order/domain fingerprints,
source-row provenance, rules and guards, exceptional branches, lower-sector
dependencies, modular samples, exact replay state, and an explicit terminal
policy. A timeout or uncovered key must never be serialized as a master.

### 5.2 Online application runtime

The hot path should ingest batches of normalized terms and perform:

```text
VakintTerm { integral, numerator }
-> family fingerprint and simultaneous routing map
-> native tensor projection with external spectators retained
-> scalar-product lowering and propagator cancellation
-> collected concrete integral keys
-> compiled guarded-rule application with shared memoization
-> sparse master-coefficient map over Q(d)
```

Pattern matching and Atom rendering belong at the boundary. Interned exponent
vectors, compiled integer predicates, cached coefficient specializations, and
shared normal forms belong in the hot loop. Terms must be grouped by canonical
family and artifact before reduction. GammaLoop currently computes operation
nodes in dependency order
([`hedge_poset.rs:1150-1181`](../../vendor/gammaloop/crates/gammalooprs/src/uv/hedge_poset.rs#L1150));
the adapter can retain that order while memoizing identical family/integral
normal forms across nodes and forest terms.

Master evaluation is a separate concern. RustRed should first return stable
unsubstituted master keys and exact coefficients. A GammaLoop-side master
evaluator may then provide the Laurent expansions required before
`Integrated::run` truncates in epsilon.

## 6. Current reusable RustRed pieces

The following are real generic building blocks and should be extended rather
than replaced:

- compact topology-neutral family parsing and affine denominator lowering in
  [`symbolica_integral_input.rs`](../../src/symbolica_integral_input.rs);
- deterministic affine-basis/ISP completion in
  [`automatic_isps.rs`](../../src/automatic_isps.rs);
- fully generated ordinary parametric IBPs in
  [`parametric_ibp.rs`](../../src/parametric_ibp.rs);
- family/sector/zero/symmetry proof components and the generic affine
  `WhenBad` work in progress;
- topology-independent Vakint tensor syntax and spectator-covariant numerator
  compilation in
  [`symbolica_tensor_numerator.rs:1-125`](../../src/symbolica_tensor_numerator.rs#L1);
- exact tensor projection, scalar-product lowering, and generic guarded rule
  application in the library pipeline; and
- one-loop numerator/propagator cancellation closure tests.

The older `VakintTwoLoopAdapter` is explicitly narrow and builds an authored
two-loop pipeline
([`vakint_adapter.rs:1-27`](../../crates/rustred-legacy-oracles/src/vakint_adapter.rs#L1),
[`vakint_adapter.rs:154-193`](../../crates/rustred-legacy-oracles/src/vakint_adapter.rs#L154)). It is useful
only as an oracle. It is not the GammaLoop integration architecture.

## 7. Critical missing services

In priority order, the source audit finds:

1. **Complete generic rule publication.** RustRed derives generic rows and has
   substantial exact-session/`WhenBad` components, but does not yet publish a
   replay-certified complete LiteRed-like reduction for an arbitrary family.
2. **GammaLoop full-topology importer.** There is no generic decoder from
   `topo(product(prop(...)))` to an authenticated `IntegralFamily`, including
   routing witness, sign/mass conversion, deterministic ISP completion, and a
   stable family fingerprint. The generic tensor parser alone is not this
   importer.
3. **A public typed seam.** GammaLoop's useful
   `to_vakint_integrand` is currently `pub(crate)`. The clean integration is a
   small GammaLoop-side `VacuumIntegralEngine`-style interface or exported
   normalized-term type, not copied graph surgery in RustRed.
4. **One shared Symbolica crate revision.** RustRed currently uses its local
   path revision, while the vendored GammaLoop workspace patches Symbolica to
   its `dev` git branch. A zero-copy `Atom` boundary requires both crates to
   resolve to the same Symbolica package instance and pinned revision.
5. **Scalable topology/symmetry candidate generation.** The generic affine map
   verifier is appropriate, but bounded integer-matrix enumeration cannot be
   the primary six-loop candidate source. Graph automorphisms and routing
   solutions must generate candidates, which are then certified generically.
6. **Generic unit-mass modular foundry.** Existing modular experiments are
   bounded loop-specific oracles. The production path still needs generic
   `Q(d)` finite-field sampling, stable sparse pivot discovery, adaptive
   rational reconstruction, bad-sample handling, and exact replay.
7. **Persistent artifacts.** In-memory certificates and deterministic CLI
   relation output are not yet a resumable, content-addressed family/sector
   rule database with atomic publication and dependency metadata.
8. **Batch application runtime.** Current library reductions are primarily
   per target. Six-loop deployment needs grouping, interning, compiled rule
   dispatch, sharded memoization, parallel scheduling, and a sparse master-map
   output boundary.
9. **Generic graph factorization and overcomplete families.** Complete
   component decomposition, transport proofs, duplicate/dependent physical
   denominators, and partial fractions remain incomplete for arbitrary
   imported topologies.
10. **Output/master adapter.** Stable RustRed master keys, GammaLoop Atom
    rendering, normalization ownership, and the optional master-Laurent
    evaluator need an explicit contract.

There is also a narrow defensive issue in the current GammaLoop converter:
after an underdetermined momentum solve it verifies that free variables are
absent from the rewritten topology, but does not make the analogous production
check on the rewritten numerator
([`integrated.rs:931-941`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L931)).
The RustRed boundary should reject free routing variables in both fields.

## 8. First integration acceptance gates

Before a six-loop campaign, the adapter and foundry should pass:

1. exact round trips for arbitrary edge IDs, edge orientations, loop bases,
   and simultaneous numerator routing substitutions;
2. explicit Minkowski `q^2-m^2` and Euclidean `q_E^2+m^2` conversion pairs;
3. unit-mass versus symbolic-mass homogeneity restoration at fresh powers;
4. scalar, rank-two, rank-four, and rank-six one-loop GammaLoop terms;
5. the representation-closure oracle
   `(q^2-m^2)/D(q)^a == 1/D(q)^(a-1)` and its chosen Euclidean equivalent;
6. two- and three-loop Vakint comparisons before master substitution;
7. all four-loop Vakint topology/routing fixtures as frozen inputs;
8. repeated numerators over one family to measure cache reuse and batching;
9. held-out primes, routings, edge permutations, and numerator shells; and
10. typed failure for every missing artifact, uncovered key, convention
    mismatch, free routing variable, or exhausted resource budget.

The first credible six-loop milestone is therefore a declared
GammaLoop/BPHZ-derived corpus reduced to stable unsubstituted master keys by
replay-certified artifacts. It is not merely successful generation of the 36
IBPs for one topology, and it is not a finite reduction whose unresolved
columns were silently accepted as masters.
