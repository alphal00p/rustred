# Vakint integration audit for RustRed

Status: source audit, 2026-08-12. This report is deliberately based only on the checked-in Rust, Symbolica, FORM-source, and test fixtures. No FORM process was invoked.

## Executive result

Vakint already provides a useful pure-Rust front half around Symbolica:

- it splits sums into `(numerator, vacuum topology)` terms;
- matches graph topology, masses, powers, edge orientation, and loop-momentum bases;
- canonicalizes the numerator into the selected loop-momentum basis;
- finds graph contractions and representative loop-momentum bases; and
- performs normalization, epsilon expansion, master-value substitution, and numerical evaluation in Rust with Symbolica.

The two operations RustRed must replace are currently external algebra engines:

1. Lorentz tensor reduction is entirely performed by `tensorreduce.frm`.
2. Scalar-integral reduction is performed by `integrateduv.frm` at one through three loops, by MATAD at up to three loops, and by FMFT at four loops.

Every analytic `EvaluationMethod` advertises a FORM dependency. PySecDec is also declared FORM-dependent because the public Vakint pipeline tensor-reduces first, although its sector-decomposition subprocess is Python rather than FORM. The only direct `run_form` call sites are the tensor reducer, AlphaLoop/integrated-UV evaluator, MATAD adapter, and FMFT adapter.

The narrow integration seam is stable and appears in both GammaLoop consumers:

```text
VakintExpression
  -> canonicalize(...)
  -> tensor_reduce(...)
  -> evaluate_integral(...)
  -> Symbolica Atom
```

RustRed should preserve that semantic boundary while replacing the opaque `Atom -> text -> FORM -> text -> Atom` round trip with typed Rust requests. It should keep reduction separate from master-integral evaluation: an IBP reducer should return an exact linear combination of stable master IDs, while a distinct value provider can expand those masters in epsilon. This separation is important at four and five loops, where reduction can be complete before all analytic master values are available.

The current Vakint registry covers one, two, three, and selected four-loop single-scale massive vacuum families. It contains no five-loop type, family, backend, fixture, or test. Four-loop H/X families also require one irreducible scalar product (ISP), BMW/FG require two, and a typical five-loop cubic vacuum family has fewer than the 15 independent loop scalar products. A general ISP-capable family representation is therefore a prerequisite, not a later optimization.

## Primary source map

The important sources are:

- [`vakint/src/lib.rs`](../../vendor/gammaloop/crates/vakint/src/lib.rs): expression model, matching, tensor FORM bridge, evaluation dispatch, normalization, and public API.
- [`vakint/src/topologies.rs`](../../vendor/gammaloop/crates/vakint/src/topologies.rs): built-in one- through four-loop families, contractions, canonical routing, and topology matching.
- [`vakint/src/graph.rs`](../../vendor/gammaloop/crates/vakint/src/graph.rs): vacuum graph model, contraction deduplication, graph canonization, and loop-momentum-basis discovery.
- [`vakint/src/symbols.rs`](../../vendor/gammaloop/crates/vakint/src/symbols.rs): Symbolica functions and attributes used at the boundary.
- [`run_tensor_reduction.txt`](../../vendor/gammaloop/crates/vakint/templates/run_tensor_reduction.txt) and [`tensorreduce.frm`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/tensorreduce.frm): complete current tensor implementation.
- [`run_alphaloop_integral_evaluation.txt`](../../vendor/gammaloop/crates/vakint/templates/run_alphaloop_integral_evaluation.txt) and [`integrateduv.frm`](../../vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm): one-, two-, and three-loop scalar reduction and master substitutions.
- [`vakint/src/matad.rs`](../../vendor/gammaloop/crates/vakint/src/matad.rs) and [`run_matad.txt`](../../vendor/gammaloop/crates/vakint/templates/run_matad.txt): MATAD translation for one through three loops.
- [`vakint/src/fmft.rs`](../../vendor/gammaloop/crates/vakint/src/fmft.rs), [`run_fmft.txt`](../../vendor/gammaloop/crates/vakint/templates/run_fmft.txt), and [`fmft.frm`](../../vendor/gammaloop/crates/vakint/form_src/fmft/fmft.frm): four-loop translation and reduction.
- [`gammalooprs/src/uv/approx/integrated.rs`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs): GammaLoop-to-Vakint conversion and integrated-counterterm consumer.
- [`gammalooprs/src/processes/amplitude.rs`](../../vendor/gammaloop/crates/gammalooprs/src/processes/amplitude.rs): whole-amplitude component evaluation through the same pipeline.
- [`vakint/tests`](../../vendor/gammaloop/crates/vakint/tests): matching, tensor, analytic, differential-backend, and numerical reference fixtures.

## Current data model and conventions

### Expression-level representation

`VakintExpression(Vec<VakintTerm>)` is a sum of terms. Each `VakintTerm` contains:

```rust
pub struct VakintTerm {
    pub integral: Atom,
    pub numerator: Atom,
    pub vectors: Vec<(String, i64)>,
}
```

`VakintExpression::split_integrals` finds every `topo(...)` atom and uses a Symbolica coefficient list to separate it from its numerator. A term without exactly one first-power topology is rejected. This is a good adapter-level representation, but it is too opaque for the RustRed core: family validation and exponent manipulation should not repeatedly pattern-match arbitrary `Atom`s.

The full integral syntax is:

```text
topo(
  prop(prop_id, edge(source_node, sink_node), momentum, mass_squared, power)
  * ...
)
```

There is also a shorthand such as `topo(I2L(m2,a1,a2,a3))`. Important conventions are:

- Input propagator and node IDs are nonnegative integers. Known-topology input IDs may be arbitrary; unknown-topology IDs must be contiguous from one.
- Canonical propagator IDs are one-based and contiguous.
- Edge direction carries the sign convention for momentum routing. Topology matching first checks the undirected graph, then scores oriented matches and records edge flips.
- Momentum is a linear Symbolica expression in `k(i)`. Known generic families require loop IDs `1..=L`. GammaLoop solves and rewrites its own momentum variables to that basis before calling Vakint.
- The fourth `prop` argument is mass squared. `identify_uv_mass_symbols` accepts either an atom interpreted as `m^2` or an explicit square `m^2` from which it recovers `m`.
- The current known families are single-scale: every canonical edge uses the same `msq(1)` placeholder. Unknown/PySecDec input can carry more general masses, but the analytic registry cannot reduce a general mass assignment.
- Positive `power` means a denominator power; zero denotes a pinch. During scalar numerator conversion, nonpositive indices naturally represent numerator insertions.

`Graph` stores `Edge { id, left_node_id, right_node_id, momentum, mass }` and `Node { id, edges: Vec<(edge_id, direction)> }`. A self-loop contributes both an incoming and outgoing incidence and is accepted. Nodes of valence zero or one are rejected as malformed vacuum graphs.

### Numerator and Lorentz representation

Vakint supports both explicit indices and dot notation:

```text
k(loop_id, lorentz_index)
p(external_id, lorentz_index)
g(index_1, index_2)
dot(k(loop_id), k(other_loop_id))
dot(k(loop_id), p(external_id))
```

The registered `dot` function is symmetric and linear. `convert_from_dot_notation` expands a dot-product power into pairs of explicit vector factors with generated dummy indices; `convert_to_dot_notation` contracts matching indices and metrics back into dots. Index atoms are not limited to integers: GammaLoop uses decorated tensor-index expressions. The FORM bridge temporarily maps arbitrary indices to integers above `13_370_000` and reverses the map on return. RustRed can and should preserve index atoms directly.

Only loop vectors are integrated. External `p(...)` factors, external scalar products, free metrics, user symbols, and tensor indices are spectators that must survive tensor and scalar reduction. A scalar integral with an odd number of uncontracted loop-vector indices vanishes.

### Denominator convention and backend sign changes

The integrated-UV numerator identity is

```text
k^2 / D(k)^n = 1 / D(k)^(n-1) + m^2 / D(k)^n,
```

so its algebraic denominator convention is `D(k) = k^2 - m^2` before measure/metric normalization. MATAD instead uses Euclidean denominators: the adapter inserts a minus sign for every loop-loop scalar product and an overall `(-1)^(sum of denominator powers)`. FMFT uses dimensionless internal scalar products and restores powers of the mass afterward. RustRed should define one explicit core convention, preferably

```text
D_a = q_a^2 - m_a^2,   d = 4 - 2 epsilon,
```

and isolate all Euclidean or legacy-backend conversions in adapters. IBP reduction coefficients must not include loop-measure normalization, factors of `i`, `pi`, MS-bar factors, or GammaLoop's `additional_normalization`; those belong to the master-value/evaluation layer already present around Vakint.

## Built-in family registry

### One through three loops

The complete scalar-product families are:

```text
I1L:
  q1 = k1

I2L:
  q1 = k1
  q2 = k2
  q3 = k1 + k2

I3L (tetrahedron / Mercedes completion):
  q1 = k1
  q2 = k2
  q3 = k3
  q4 = k3 - k1
  q5 = k1 - k2
  q6 = k2 - k3
```

The two-loop registry explicitly includes the full family and the edge-3 pinch. The three-loop registry discovers all graph-inequivalent contractions that preserve the loop count using Symbolica graph canonization. Pinched shorthand names append `_pinch_<edge ids>` and retain zeroes in the full exponent vector.

The MATAD edge-order adapters are fixed as:

| Family | MATAD labels for Vakint edges `1..N` |
| --- | --- |
| `I1L` | `1` |
| `I2L` | `2, 3, 1` |
| `I3L` | `4, 5, 6, 1, 2, 3` |

RustRed should make backend-specific order maps unnecessary. A family fingerprint should be derived from canonical momentum coefficient vectors, mass labels, and ISP definitions rather than from a display name.

### Four loops

Vakint registers four FMFT families. Their momentum lists are:

```text
I4L_H (9 denominators):
  k1, k2, k3, k4,
  k1-k3, k2-k3, k3-k1+k4, k3-k2+k4, k3+k4

I4L_X (9 denominators):
  k1, k2, k3, k4,
  k1-k3, k2-k3, k3-k1+k4, k3-k2+k4,
  k3-k1-k2+k4

I4L_BMW (8 denominators):
  k1, k2, k3, k4,
  k1-k2, k3-k4, k2+k3-k1, k3-k4-k1

I4L_FG (8 denominators):
  k1, k2, k3, k1-k3, k4, k2-k3, k1-k3+k4, k1-k2
```

There are `4*5/2 = 10` independent loop scalar products, so H/X need one ISP and BMW/FG need two. The current adapter lets FMFT handle those numerator directions implicitly. A direct RustRed port cannot represent these only as an exponent vector over the physical propagators.

H, X, and BMW register only the top family. FG automatically registers contractions and is intended to absorb further pinches. Power-zero edges can still allow a full family to match, so present behavior is broader than the explicit contraction list suggests. The fixed FMFT edge maps are:

| Family | FMFT labels for Vakint edges `1..N` |
| --- | --- |
| `I4L_H` | `2, 3, 4, 5, 6, 7, 8, 9, 1` |
| `I4L_X` | `2, 3, 4, 5, 6, 7, 8, 9, 10` |
| `I4L_BMW` | `3, 4, 5, 6, 7, 8, 9, 10` |
| `I4L_FG` | `1, 3, 4, 5, 6, 7, 8, 9` |

The entries `10` are FMFT labels, not Vakint propagator indices. These maps are a warning against exposing backend numbering as the RustRed identity of an integral.

### Five loops

There is no five-loop code in Vakint: no `FiveLoop` topology variant, no `I5L` family, no reduction resource, and no test. At five loops there are 15 independent `k_i dot k_j` variables. RustRed must support:

- multiple top-level families rather than one universal graph;
- explicit auxiliary denominators/ISPs;
- products of lower-loop masters from disconnected sectors;
- graph and loop-basis symmetries beyond simple edge permutations; and
- a reduction database that can be generated, cached, and resumed without compiling reduction rules into source code.

This is a qualitative jump from translating the current hand-written three-loop rules.

## What is already native and reusable

The following algorithms are already implemented without FORM and can either be retained in the Vakint adapter or moved behind RustRed types:

1. **Topology parsing and validation.** `Integral::new` validates masses, powers, nodes, and momentum functions and constructs oriented and unoriented Symbolica patterns.
2. **Topology matching.** `match_integral_to_user_input` enumerates graph matches, rejects non-bijective node maps, scores loop-basis complexity and edge flips, and produces simultaneous numerator substitutions.
3. **Term-safe momentum substitution.** `replace_multiple` avoids the cascade error in a map such as `a -> b, b -> c`.
4. **Contraction generation.** `Graph::find_unique_contractions` contracts edges, preserves loop count, canonizes the undirected mass-labelled graph, and keeps one representative per isomorphism class.
5. **Loop-momentum-basis discovery.** `Graph::get_one_lmb` removes a spanning tree using a stable pattern rewrite; `Topology::force_an_lmb` solves a linear system with Symbolica and rewrites every edge so each `k(i)` occurs as a bare momentum on some edge.
6. **Dot/index conversion.** This is already Symbolica-native, though RustRed should avoid unnecessary conversion for typed tensor input.
7. **Exact epsilon series and postprocessing.** Normalization factors, `d -> 4-2 epsilon`, logarithm replacements, Gamma/master expansions, arbitrary-precision numeric constants, and external-momentum evaluation are already Rust code.
8. **GammaLoop graph conversion.** `to_vakint_integrand` turns denominator atoms into propagators, shrinks dependent subgraphs, fuses same-mass two-bonds, rebuilds contiguous IDs, solves the component loop routing, and translates GammaLoop tensor symbols to Vakint symbols.

Two validation improvements should accompany reuse:

- Compare the number/rank of momentum variables with the graph cyclomatic number. Vakint often infers loop count from distinct `k(i)` IDs, which can accept a linearly dependent or incomplete routing until a later operation fails.
- Canonical family identity must include the full momentum coefficient matrix and mass assignment. `Graph::to_symbolica_graph` uses only mass as edge data; that is sufficient for its present contraction deduplication but not for a reduction-family cache key.

## What FORM currently does

### Lorentz tensor reduction

`VakintTerm::tensor_reduce` performs only marshaling:

1. expand `dot` into indexed vectors;
2. rename loop vectors to FORM `vec(...)` and external vectors to `vec1(...)`;
3. serialize user symbols and indices;
4. call `TensorReduce()`; and
5. parse `rat`, `g`, `vec1`, indices, and vector names back into Symbolica.

The FORM procedure itself:

- contracts metrics and already-paired internal indices;
- tags every remaining internal loop-vector factor and counts tensor rank;
- sets every odd-rank term to zero;
- enumerates pair partitions of the internal indices;
- uses inverse projector tables at ranks 2, 4, 6, 8, and 10;
- contracts the resulting metrics with external vectors/metrics; and
- evaluates closed metric traces at `d = 4 - 2 epsilon`.

The default preloaded maximum is rank eight, and `pvtab10.h` is loaded on demand. No rank-12 resource is present, so ten is the effective hard ceiling. The FORM source also contains unresolved comments around external-vector handling (`TODO: why this?`) and the final `vxx` conversion (`TODO: is this correct?`). It should be used to establish compatibility tests, not transliterated line for line.

### One-loop scalar reduction

`IntegrateUV1L` rewrites all `k^2` numerator factors against the denominator, drops nonpositive powers as scaleless, and applies the recurrence

```text
I(n) -> I(n-1) * (d + 2 - 2 n) / (2 (n-1) m^2),   n > 1.
```

It then maps `I(1)` to the one-loop master normalization used by the master table.

### Two-loop scalar reduction

`IntegrateUV2L` recognizes either two disconnected one-loop vertices or the three-edge sunset. It maps every edge to

```text
uvid(2, 1, n1, n2, n3),
```

rewrites every `k_i dot k_j` into shifted denominator powers and `m^2`, applies hard-coded zero-sector rules and sector permutations, and repeatedly applies ordered recurrence rules until only two master symbols remain:

- `uvid(2,1)`: the factorized/product-tadpole master normalization;
- `uvid(2,2)`: the genuine two-loop massive sunset master normalization.

The two named corners immediately before master normalization are `tp2P011` and `tp2P111`. Reduction coefficients are exact rational functions of epsilon and powers of `mUV`.

### Three-loop scalar reduction

`IntegrateUV3L` recognizes five graph shapes by products of vertex signatures `vxs(...)`:

1. six-edge Mercedes/tetrahedron;
2. a five-edge contraction;
3. the four-edge banana;
4. sunrise times bubble; and
5. triple bubble.

All are embedded into

```text
uvid(3, 1, n1, n2, n3, n4, n5, n6).
```

The procedure contains explicit formulas for all 21 pairwise scalar products of its six routed edge momenta in terms of shifted denominators, a large zero-sector list, sector canonicalization maps, and a long ordered list of recurrence rules. Some rules temporarily emit products of `uvid` shift operators; a final repeated rewrite merges their exponent vectors. Reduction loops stop only when no `uvid(3,...)` remains outside the recognized master corners.

The five master corners substituted by `Masters()` are:

| Shape | Exponent vector |
| --- | --- |
| Mercedes | `(1,1,1,1,1,1)` |
| five-edge | `(0,1,1,1,1,1)` |
| banana | `(0,1,1,1,0,1)` |
| sunrise-bubble | `(0,0,1,1,1,1)` |
| triple-bubble | `(0,0,1,0,1,1)` |

The master table includes Laurent expansions and placeholder constants for unavailable higher coefficients. For three loops only, the reduction deliberately keeps one extra spurious epsilon order and aborts on a detected epsilon pole of depth two in a remaining coefficient. This is evidence that output expansion depth cannot simply equal requested physical depth.

The legacy source has signs of accumulated table fragility, including a duplicated triple-bubble expression and hand-coded raising-operator products. Native IBP generation plus algebraic certificates is safer than porting this control flow verbatim.

### MATAD and FMFT

MATAD receives a numerator whose internal scalar products have been translated to its propagator labels plus a product such as `s2m^a2*s3m^a3*...`. `#call matad(L)` performs both reduction and evaluation. Rust then maps its masters, Gamma functions, harmonic polylogarithms, and constants into Symbolica and corrects Euclidean signs and normalization.

FMFT similarly receives powers `d_i^(-a_i)`. `#call fmft` performs high-level X/H/BMW reduction, maps simpler sectors to FG, reduces factorized two- and one-loop subgraphs, and substitutes four-loop master names. Its checked-in source is about 12,000 lines and combines topology mapping, tensor/scalar numerator handling, recurrences, and master tables.

Neither adapter is an appropriate RustRed API. The useful compatibility information is the family routing, exponent convention, resulting master basis, and golden outputs.

## Native tensor algorithm for RustRed

A table-free exact projector can reproduce the current behavior.

For one numerator monomial with `r = 2n` internal loop-vector factors

```text
k_{a1}^{mu1} ... k_{ar}^{mur},
```

enumerate perfect matchings `P` of the Lorentz slots. Write the invariant ansatz

```text
sum_P c_P * product_(i,j in P) g^{mu_i mu_j}.
```

Contract the ansatz with every matching `Q`. The Gram matrix entry is

```text
G[Q,P] = d ^ cycles(P union Q),
```

where the union of two matchings is a set of alternating cycles. The contracted source is

```text
b[Q] = product_(i,j in Q) (k_ai dot k_aj).
```

Solve `G c = b` exactly over `Q(d)`, then substitute `d = 4 - 2 epsilon` only at the adapter boundary. Rank two immediately gives

```text
k_a^mu k_b^nu -> g^{mu nu} (k_a dot k_b) / d.
```

This method naturally handles several distinct loop momenta and reproduces the rank-four and higher Passarino-Veltman projectors. Odd rank returns zero.

A naive basis has `(2n-1)!!` matchings: 945 at rank ten and 10,395 at rank twelve. Production implementation must quotient by permutations of identical loop-vector labels and identical external contractions, or use the corresponding Brauer-algebra/orbit basis. Cache the inverse projector by `(rank, label multiplicities)` and perform sparse exact solves. The output should remain a Symbolica polynomial in scalar products and metrics; no component-wise dimension expansion is needed.

Required tensor invariants and error checks are:

- preserve arbitrary external/free index atoms;
- contract pre-existing metrics before projecting, but do not assume every metric lies outside the tensor;
- canonicalize symmetric metric and dot arguments deterministically;
- return zero for odd internal rank even when external vectors are present;
- expose a configurable rank/resource limit rather than silently looking for a missing table; and
- verify the result by recontracting all orbit representatives with the source monomial.

## Native scalar/IBP algorithm for RustRed

### Typed family algebra

For `L` loop momenta define the `K = L(L+1)/2` scalar variables

```text
s_ij = k_i dot k_j,  i <= j.
```

Each propagator momentum is an integer vector

```text
q_a = sum_i C[a,i] k_i,
```

and

```text
D_a = q_a^2 - m_a^2
    = sum_i C[a,i]^2 s_ii
      + 2 sum_(i<j) C[a,i] C[a,j] s_ij
      - m_a^2.
```

Build the exact linear map from `s_ij` to physical denominators. If its rank is below `K`, add explicit ISP linear forms until the combined matrix is invertible. Store ISPs separately from physical propagators in the public model; an internal auxiliary-denominator exponent encoding is fine, but a zero physical power and a numerator ISP must remain distinguishable to callers.

After tensor reduction, expand the scalar numerator as a polynomial in `s_ij`, solve it into denominators plus ISPs, and convert every denominator factor into a downward exponent shift. This replaces the 2/3-loop hand-written `g(k_i,k_j)` identities and generalizes immediately to four and five loops.

### IBP generation

For each pair of loop momenta `(i,j)`, generate

```text
0 = integral d^d k_1 ... d^d k_L
    partial/partial k_i dot
    [ k_j / product_a D_a^n_a ].
```

For a scalar key `n` this is

```text
0 = d delta_ij I(n)
    - sum_a n_a * integral(
        k_j dot partial(D_a)/partial(k_i)
        / D_a^(n_a+1)
        * product_(b != a) D_b^(-n_b)
      ),
```

with

```text
partial(D_a)/partial(k_i) = 2 C[a,i] q_a.
```

The scalar-product map turns each identity into a sparse row over shifted integral keys with coefficients in the rational-function field generated by `d` and mass invariants. For numerator targets, either apply the same signed-exponent formula to a completed set of physical and auxiliary quadratic forms, or include the derivative of explicit ISP numerator powers; omitting that term gives invalid rows. Generate rows on demand for the target region rather than expanding an unbounded lattice.

### Canonicalization and solve

Before inserting a key into a system:

1. identify zero/scaleless sectors;
2. map sector and exponent vector under family symmetries to a canonical representative;
3. split disconnected sectors into products of lower-loop families where possible; and
4. order the remaining key by a documented Laporta order: sector, total dots, total numerator/ISP degree, then stable lexicographic tie breakers.

Solve sparse systems sector by sector, highest keys first. Two and three loops are small enough for direct exact Symbolica rational-function arithmetic. Four and especially five loops require modular images over finite fields, sparse elimination, interpolation/reconstruction in `d` (and masses if multi-scale support is later added), plus deterministic verification in fresh primes. A pure-Rust implementation can still use Symbolica's polynomial/rational domains; "pure Rust" only rules out external algebra executables.

Reduction rules should be stored as a DAG and memoized. A serialized database needs a versioned family fingerprint, dimension convention, ordering version, master list, coefficient-domain metadata, and checksums. It must never depend on hash-map iteration order.

### Zero sectors, symmetries, and factorization

The present 2/3-loop zero sectors and symmetry maps are hard-coded. RustRed needs algorithms:

- graph/parametric scaleless-sector tests, including massless components with no scale;
- corner-IBP tests for zero sectors when graph inspection is insufficient;
- graph automorphisms combined with unimodular loop-basis transformations that preserve the denominator/ISP set;
- canonical exponent orbits under those transformations; and
- explicit factorization into a commutative product of connected lower-loop masters.

Factorization is observable behavior: current fixtures include double/triple/fourfold bubbles, and the two-loop master basis contains a product-tadpole normalization.

### Reduction certificates

Every generated rule should be independently checkable without the solver that produced it. At minimum:

- substitute rules back into all generating IBP rows and verify exact zero;
- check unused fresh finite-field points after reconstruction;
- verify mass dimension/homogeneity;
- verify symmetry-related targets reduce identically; and
- record the master coefficient vector before any master-value substitution.

This is the native replacement for trusting that a large ordered recurrence table reached its terminal labels.

## Recommended RustRed API

The core should be typed and independent of Vakint's symbol names. The following is an API shape, not a required spelling:

```rust
pub struct Momentum {
    /// Coefficients of k_1 ... k_L.
    pub loop_coefficients: Box<[i16]>,
}

pub struct Propagator {
    pub id: PropagatorId,
    pub momentum: Momentum,
    pub mass_squared: Atom,
}

pub struct Isp {
    pub id: IspId,
    /// Exact linear form in the canonical scalar products s_ij.
    pub scalar_form: Box<[i16]>,
}

pub struct IntegralFamily {
    pub name: String,
    pub loop_count: usize,
    pub propagators: Box<[Propagator]>,
    pub isps: Box<[Isp]>,
    pub convention: DenominatorConvention,
}

pub struct IntegralKey {
    pub family: FamilyId,
    pub denominator_powers: Box<[i32]>,
    pub isp_powers: Box<[i32]>,
}

pub struct IntegralTerm {
    pub coefficient: Atom,
    pub integral: IntegralKey,
}

pub struct ReductionRequest {
    pub targets: Vec<IntegralTerm>,
    pub dimension: Atom,          // normally d, not yet 4-2 epsilon
    pub ordering: OrderingSpec,
    pub limits: ReductionLimits,
}

pub struct ReductionResult {
    pub combination: LinearCombination<MasterProduct>,
    pub masters: Vec<MasterIntegral>,
    pub certificate: ReductionCertificate,
    pub statistics: ReductionStatistics,
}

pub trait TensorReducer {
    fn reduce_tensor(&self, input: TensorPolynomial) -> Result<Atom, RustRedError>;
}

pub trait ScalarReducer {
    fn reduce(&self, family: &IntegralFamily, request: ReductionRequest)
        -> Result<ReductionResult, RustRedError>;
}

pub trait MasterValueProvider {
    fn epsilon_series(
        &self,
        master: &MasterIntegral,
        request: SeriesRequest,
    ) -> Result<Atom, RustRedError>;
}
```

Additional API requirements:

- `IntegralFamily::validate()` must check coefficient-vector lengths, momentum rank, graph loop count, denominator/ISP rank, contiguous local IDs, and mass convention.
- `FamilyId` and `MasterIntegral` must be stable canonical fingerprints, not process-local symbol IDs.
- Coefficients and spectator factors should remain Symbolica `Atom`s, while integral keys remain typed.
- Results need explicit `Complete`, `Partial { unreduced }`, and resource-limit errors. Never silently promote an unreduced key to a master merely because a limit was reached.
- Cancellation/progress hooks and resumable database generation will be necessary at four/five loops.
- The library API must not expose a command path, temporary directory, FORM syntax, `rat(...)`, or backend edge numbering.

### Vakint/GammaLoop adapter

Avoid making RustRed depend on Vakint, which would create an awkward cycle. Put an adapter in Vakint or GammaLoop:

```text
VakintTerm Atom syntax
  -> canonical RustRed IntegralFamily + typed IntegralKey(s)
  -> RustRed tensor projection
  -> scalar-product-to-denominator/ISP conversion
  -> RustRed IBP reduction
  -> optional master-value provider
  -> Vakint-compatible Symbolica Atom
```

For an incremental migration, add a native evaluation method and leave `canonicalize` intact. Replace these two calls at the consumers:

```rust
integrand_vakint.tensor_reduce(...)?;
integrand_vakint.evaluate_integral(...)?;
```

with one adapter that can return either master combinations or fully evaluated series. Once selected, settings validation must not probe a FORM executable. GammaLoop currently has `form_exe_path` only because Vakint exposes it; native execution should not read or preserve that setting.

The adapter must preserve:

- arbitrary user namespaces and decorated index atoms;
- `use_dot_product_notation` behavior;
- external `p` and metric output expected by GammaLoop;
- `epsilon_symbol` and `mu_r_sq_symbol` customization;
- requested series depth and extra spurious-pole depth;
- loop normalization and GammaLoop's per-loop `additional_normalization`; and
- the current distinction between canonicalization, reduction, master substitution, and numerical evaluation in debug traces.

## GammaLoop integration seams

### Integrated UV counterterms

`Integrated::integrate` in `uv/approx/integrated.rs` constructs a `VakintExpression`, logs each stage, canonicalizes, tensor-reduces, evaluates, translates Vakint metrics/momenta/constants back to GammaLoop symbols, simplifies tensor chains, and finally forms the requested epsilon series.

`to_vakint_integrand` is the upstream normalization seam. It:

- undoes Schoonschip, chain, and trace shorthands;
- converts each `den(edge,momentum,mass,...)` into a one-powered `prop`;
- converts inverse powers into positive propagator powers;
- optionally replaces masses with the common UV vacuum mass;
- shrinks the dependent subgraph;
- flips simple negative edge momenta;
- fuses same-mass degree-two bonds by adding powers;
- rebuilds one-based contiguous propagator IDs;
- solves the graph loop routing and rewrites numerator and integral consistently;
- squares the mass before writing Vakint's `mass_squared` field; and
- translates GammaLoop loop/external momenta, dots, and metrics to Vakint symbols.

RustRed should consume the normalized output of this function first. Reimplementing GammaLoop subgraph surgery inside RustRed would mix graph-generation concerns with IBP reduction.

### Whole-amplitude component evaluation

`AmplitudeGraph` uses the same converter and the same three stages when analytically evaluating a selected connected component. This call can preserve physical masses (`substitute_masses_to_m_uv = false`) even though the current analytic Vakint families are single-scale. RustRed's initial adapter should reject a genuinely multi-scale target with a typed error rather than accidentally matching it to the common-mass family.

### Concrete coupling today

Both consumers hold `&vakint::Vakint` and call concrete methods; there is no reducer trait. A small GammaLoop-facing trait is useful during migration:

```rust
trait VacuumIntegralEngine {
    fn canonicalize(&self, terms: &mut VakintExpression, settings: &Settings) -> Result<()>;
    fn reduce(&self, terms: &mut VakintExpression, settings: &Settings) -> Result<()>;
}
```

Keep the trait at the semantic engine boundary rather than mirroring separate FORM-era methods forever. The native implementation can internally expose tensor/scalar phases and retain the existing trace stages.

## Existing tests and reusable expected outputs

### Matching fixtures

[`input_matching_tests.rs`](../../vendor/gammaloop/crates/vakint/tests/input_matching_tests.rs) covers:

- arbitrary one- and two-loop momentum/edge IDs;
- canonical momentum replacement in numerators;
- shorthand/full-form equivalence;
- a pinched two-loop graph and loop-basis rotation;
- a three-loop shorthand containing zero powers; and
- unknown-integral validation.

These tests do not require FORM conceptually and should be retained as adapter tests.

### Exact tensor fixtures

[`tensor_reduction_tests.rs`](../../vendor/gammaloop/crates/vakint/tests/tensor_reduction_tests.rs) contains three compact exact goldens. In dot notation the key expectations are:

```text
k1(mu) k1(nu)
  -> g(mu,nu) dot(k1,k1) / (4-2 epsilon)

an odd loop-vector term such as k1(mu) p(mu)
  -> 0

k1(mu) k2(nu) p2(mu) p3(nu)
  -> dot(p2,p3) dot(k1,k2) / (4-2 epsilon)
```

The checked-in expressions spell `1/(4-2 epsilon)` as `-(2 epsilon-4)^-1`. The two-loop fixture also checks that an already contracted rank-four scalar term remains `dot(k1,k1)*dot(k2,k2)*g(mu,nu)` while the mixed external contraction receives the projector factor.

Add native tests for odd ranks 1/3/5, all rank-four pairing classes, repeated and distinct loop labels, metrics inside the numerator, decorated indices, rank 8/10 compatibility, and a resource-limited rank above ten.

### Scalar/evaluation goldens

[`integral_evaluation_analytic_tests.rs`](../../vendor/gammaloop/crates/vakint/tests/integral_evaluation_analytic_tests.rs) supplies end-to-end numerical series after Vakint normalization and master substitution. Representative reference vectors are:

| Case and settings | Epsilon coefficients `(power -> value)` |
| --- | --- |
| `I2L(1,1,1)`, `m^2=mu_R^2=1`, MS-bar | `-2 -> -6.015223977354102649894208945578e-5`; `-1 -> -1.804567193206230794968262683673e-4`; `0 -> -3.790208765869208760159732351246e-4`; `1 -> -9.708141619945564042866174113010e-4` |
| full `I3L(1,1,1,1,1,1)`, same scales, MS-bar | `-1 -> -6.105142965702799027412986390958e-7 i`; `0 -> 2.548415539172476714996527540285e-6 i` |
| rank-four full three-loop fixture, `m^2=1`, `mu_R^2=2`, MS-bar | `-3 -> 1.314137110381316809259740852289e-3 i`; `-2 -> 1.027363035879144648390245670512e-2 i`; `-1 -> 4.561360375915191500552582268384e-2 i`; `0 -> 1.409480043924165037345830826758e-1 i` |
| four disconnected one-loop bubbles, unit scales, FMFT/MATAD normalization | `-4 -> 1`; `-3 -> 4`; `-2 -> 13.28986813369645287294483033329`; `-1 -> 31.55672999723968577791300378449`; `0 -> 67.98165058904685502307905531744` |
| four-loop PR9d fixture, unit scales, FMFT/MATAD normalization | `-4 -> 1/12`; `-3 -> 1/3`; `-2 -> -0.3144646082033583725553786166618`; `-1 -> 5.421352941798334340259377610275`; `0 -> -28.31064373017674207211847384976` |

These are end-to-end fixtures, not raw IBP coefficients. RustRed needs a second golden layer that freezes the pre-substitution master coefficient vector. Initially those vectors can be captured from existing checked-in references or derived independently; new tests must not launch FORM.

Other valuable suites are:

- [`integral_alphaloop_vs_matad_tests.rs`](../../vendor/gammaloop/crates/vakint/tests/integral_alphaloop_vs_matad_tests.rs): independent-backend agreement at one through three loops, including basketball and rank-four numerators.
- [`integral_comparison_vs_pysecdec_tests.rs`](../../vendor/gammaloop/crates/vakint/tests/integral_comparison_vs_pysecdec_tests.rs): numerical checks for pinches, alternate loop bases, and tensor numerators through three loops.
- four-loop analytic tests for H, equivalent PR9d embeddings, BMW/PR11d, clover, dotted clover, and numerator insertions.
- GammaLoop's `scalars_integrated_cts_compare_legacy_and_hedge_poset` and ignored `scalars_integrated_banana_hedge_poset`, which exercise the real integrated-UV conversion boundary.

There is no five-loop expected output in Vakint. Five-loop acceptance therefore requires independent identities/certificates and reference reductions from the LiteRed2 side, not merely regression against this crate.

### Native validation matrix

For every milestone, test all of the following without external executables:

1. family parsing and denominator/ISP rank;
2. invariance under propagator permutation, edge reversal, and unimodular loop-basis change;
3. numerator-to-denominator/ISP round trip;
4. tensor projector recontraction identities;
5. each generated IBP row before solving;
6. exact residuals after substituting reduction rules;
7. fresh-prime checks after modular reconstruction;
8. mass-dimension and epsilon-pole-depth bounds;
9. disconnected-sector factorization; and
10. adapter-level equality with the existing Symbolica and numerical goldens.

## Milestones and acceptance criteria

### Milestone 1: two-loop massive vacuum bubble

Minimum complete scope:

- typed `I2L` family with all three denominators and no ISP;
- scalar-product map for `k1^2`, `k2^2`, and `k1 dot k2`;
- native tensor projection through at least rank four;
- zero-sector and `S3` denominator symmetry canonicalization;
- generated IBPs and reduction to the factorized and sunset masters;
- exact rule certificates;
- optional one/two-loop master values sufficient to reproduce the existing 2-loop series fixture; and
- Vakint/GammaLoop adapter passing input matching, tensor tests, and two-loop integrated-UV regression without FORM installed.

Do not define success as reproducing only `I(1,1,1)`. Include dotted denominators and negative-index/numerator targets covering the region exercised by rank-four tensor numerators.

### Milestone 2: three-loop massive vacuum bubbles

Minimum complete scope:

- the full six-denominator family and all loop-preserving pinched sectors;
- algorithmic zero sectors and symmetry orbits rather than copied tables;
- reduction to the five established connected/factorized master corners;
- tensor projection through the highest rank exercised by GammaLoop fixtures;
- spurious-pole depth reported by reduction and propagated into master series requests;
- full and pinched three-loop goldens, including the rank-four numerator; and
- differential comparison of native reductions reached from the AlphaLoop and MATAD edge orderings.

### Milestone 3: four loops

Minimum complete scope:

- H/X/BMW/FG canonical families;
- a complete 10-dimensional scalar basis with one or two ISPs as appropriate;
- canonical mapping of equivalent PR9d embeddings and pinches;
- connected and factorized sector reductions;
- a stable four-loop master basis and exact reduction database;
- rank-10 tensor compatibility; and
- reproduction of H, PR9d, PR11d/BMW, clover, dotted-clover, and numerator fixtures when master values are available.

### Milestone 4: five loops

Before claiming a five-loop reduction:

- publish the covered family set and all 15-dimensional scalar bases;
- generate and fingerprint their symmetry transformations and sector DAGs;
- demonstrate modular/reconstructed exact reductions for a declared target region;
- provide fresh-prime and exact IBP certificates;
- handle products of lower-loop masters canonically;
- show deterministic resume/cache behavior; and
- validate against an independent LiteRed2 reduction or another non-FORM reference.

"Supports five loops" must name families and target complexity bounds. An arbitrary graph parser plus unreduced integrals is not five-loop reduction support.

## Risk register through five loops

| Priority | Risk | Consequence | Mitigation |
| --- | --- | --- | --- |
| P0 | Denominator, metric, or measure sign is mixed between Minkowski, MATAD, and FMFT conventions | Numerically plausible but globally wrong answers | One explicit core convention; adapter-only conversions; one-loop/two-loop sign goldens and dimensional checks |
| P0 | Four/five-loop families omit ISPs | Tensor numerators cannot be encoded; IBP system is incomplete | Validate scalar-basis rank at family construction and require explicit ISP completion |
| P0 | Exact symbolic Gaussian elimination explodes | Four/five-loop generation becomes unusable | Sparse sector solve, finite fields, interpolation/reconstruction, caching, fresh-prime verification |
| P0 | Tensor perfect-matching basis grows factorially | Rank 10+ consumes excessive memory/time | Orbit/Brauer basis, multiplicity-aware cache, sparse solves, explicit limits |
| P0 | A resource limit silently turns hard integrals into masters | Incomplete reductions appear correct | Typed partial result and hard error by default; master status only from completed elimination |
| P1 | Spurious epsilon poles request too few master coefficients | Finite term is truncated incorrectly | Track coefficient valuation per master and request extra series depth dynamically |
| P1 | Factorized/disconnected sectors are treated as unrelated high-loop masters | Master count and evaluation are wrong | Canonical connected-component factorization and `MasterProduct` output |
| P1 | Graph isomorphism ignores routing or masses in a cache key | Rules are reused for a non-equivalent family | Fingerprint coefficient matrix, masses, ISP basis, and convention after canonicalization |
| P1 | Loop IDs are contiguous but routing rank is deficient | Singular scalar map or wrong loop count later | Cross-check momentum rank and graph cyclomatic number during family validation |
| P1 | Existing hand-written FORM quirks are copied as specification | Legacy typo/TODO becomes native behavior | Derive algorithms; use FORM source only for goldens; certify identities independently |
| P1 | Master numbering changes across family embeddings | Equivalent four-loop graphs fail to merge | Stable canonical master fingerprint and explicit embedding maps |
| P1 | No independent five-loop oracle exists in Vakint | A solver bug can survive regression testing | LiteRed2 cross-check, exact IBP residuals, symmetry tests, and independent modular points |
| P2 | Hash-map/matcher iteration affects canonical choices | Non-reproducible databases and test output | Sort all IDs, transformations, monomials, pivots, and master candidates explicitly |
| P2 | Arbitrary Symbolica index/function attributes are lost | GammaLoop decorated-index cases regress | Keep indices as `Atom`; eliminate FORM name sanitation from native path |
| P2 | A native matcher fails to preserve the current common-mass wildcard equality | A multi-scale graph is sent to a single-scale reduction | Family match must compare every mass label and return a typed multi-scale unsupported error |

## Recommended implementation order

1. Define typed momentum, family, physical denominator, ISP, integral-key, and exact linear-combination types with validation and stable fingerprints.
2. Implement the Vakint `Atom` adapter and preserve the existing canonicalization path initially.
3. Implement native tensor ranks 0-4 with recontraction certificates, then general orbit-based even rank.
4. Implement the scalar-product map and numerator-to-shift conversion for complete 1/2/3-loop families.
5. Generate one/two-loop IBPs, sparse exact Laporta reduction, zero sectors, and symmetry canonicalization; freeze raw master-coefficient goldens.
6. Integrate the two-loop reducer at both GammaLoop call sites and remove FORM validation from the native settings path.
7. Generalize sector discovery and factorization to the three-loop family; track spurious-pole depth.
8. Add finite-field/reconstruction infrastructure before starting four-loop production reductions.
9. Add four-loop ISP families and stable master embeddings, then expand coverage to five-loop family databases.

This order makes the first milestone a real vertical slice: GammaLoop graph conversion, tensor numerators, IBP reduction, master output/evaluation, and exact validation all run in one Rust process with no external algebra engine.
