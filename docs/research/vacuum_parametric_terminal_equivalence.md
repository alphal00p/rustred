# Exact parametric equivalence of vacuum terminals

Status: read-only design study, 19 September 2026. This mechanism is **not
implemented or enabled**. The existing momentum-routing terminal aliases and
their four-loop measurements are a separate workstream. This note proposes a
possible next proof type, not a closure, master-minimality, or performance claim.

## Mathematical basis

The Feynman-parameter representation expresses a scalar integral in terms of its
propagator powers and the first and second Symanzik polynomials. For a vacuum
integral with a common mass, the second polynomial is determined by the first
and the mass linear form. Equality of the relevant polynomials under a matching
parameter permutation therefore gives an integral identity. The representation
and determinant construction are given in equations (7)–(9) of
[Bogner and Weinzierl, *Feynman graph polynomials*](https://arxiv.org/pdf/1002.3458).

Identifying integrals through canonical parameter polynomials is an established
idea; it need not depend on isomorphism of the original diagram drawing.
[Pak, *The toolbox of modern multi-loop calculations*, section 2](https://arxiv.org/pdf/1111.0868)
discusses polynomial canonicalization for this purpose. The proposed use of
Symbolica's incidence-graph canonicalization below is an implementation choice,
not a claim that the cited papers prescribe this particular representation.

Here is the sufficient criterion specialized to the intended RustRed domain.
Let there be `L` independent loop momenta and `N` active denominators

```text
D_i = q_i(k)^2 - 1,       q_i(k) = sum_j Q_ij k_j,
a_i > 0,                 U(t) = det(sum_i t_i Q_i^T Q_i).
```

All inactive powers are zero. There are no external momenta, numerator factors,
or analytic power offsets. In Euclidean convention, apart from a common
loop-measure normalization, the integral has the Schwinger representation

```text
I(a,d) = 1 / product_i Gamma(a_i)
         * integral_(t_i > 0) product_i [dt_i t_i^(a_i-1)]
           * exp(-sum_i t_i) * U(t)^(-d/2).
```

Our inference is direct: if a bijection of the active parameters preserves each
power and maps the **entire exact polynomial** `U_source` to `U_target`, changing
integration variables gives identical integrals. The equality is first obtained
in a convergent domain and then continued meromorphically in the dimension.
Ultraviolet poles do not invalidate that identity. Converting back to the same
Minkowski convention introduces the same factors on both sides because `L` and
the sum of the powers agree. RustRed's current vacuum construction has
`F = -U * sum_i t_i` at unit mass; its sign convention must remain consistent.

This is an equality with coefficient one. It can prove relations without
producing an integer change of loop basis, so it must have its own sealed proof
type rather than masquerading as an existing `VerifiedMap`.

## Admission and failure conditions

The first implementation should be deliberately restricted:

- Same admitted family, loop count, measure convention, and unit mass.
- Exact physical squared linear loop momenta, no external shifts, and full active
  rank. A zero `U` is not an equivalence certificate. Positivity after Wick
  rotation follows from real squared momenta with full rank, not from a generic
  quadratic form of unspecified signature.
- Positive integer active powers, zero inactive powers, and zero analytic
  offsets. Negative-index numerators require a different parameter integrand;
  they cannot simply be discarded.
- A bijection preserving individual line powers, not just their total.
- Exact equality including all coefficients and overall scale. `U` and `c U`
  differ by a dimension-dependent factor and are not unit aliases.
- Strict coefficient-context binding, including constants. Reuse the existing
  admitted coefficient service rather than relying on an equality operation
  that ignores variable maps for constants.
- An existing declared terminal must be the representative. Preserve raw keys
  and provenance; never create a new terminal or infer family closure.

If masses are generalized later, the permutation must preserve the whole linear
form `sum_i m_i^2 t_i`. Merely preserving a scalar sum of masses is insufficient.
Branch conventions and any analytic continuation also need a common owner.
Unsupported inputs, excessive preparation work, or a failed exact replay leave
the original terminal unchanged.

## Existing native APIs and proposed seam

Reuse `family::symanzik::SymanzikPolynomials::try_from_family_with_limits`.
Its implementation in `family/symanzik/construction.rs` already constructs
`U`, `F`, and `G` through Symbolica arithmetic, with the determinant delegated to
native `Matrix::det` by `family/symanzik/operations.rs`. The public `u()` result
exposes `terms()`, `term_count()`, and `is_zero()`; its native polynomial is
available inside the crate through `raw()`.

The existing Feynman-polynomial context does not expose a support-restriction
operation. A narrow checked adapter could use Symbolica's
`MultivariatePolynomial::replace(index, zero)` to set inactive parameters to
zero, then reindex with native variable-map operations. Do not rebuild a
determinant, polynomial substitution, or graph-isomorphism kernel. In particular,
`rearrange` changes both exponent order and the variable map: it is not, by
itself, a substitution of one named parameter for another. A simultaneous
permutation must be replayed with explicit, checked variable binding, without
colliding sequential renamings.

A proposal graph can contain two distinct node types:

1. Parameter nodes colored by their positive powers (and masses if generalized).
2. Monomial nodes colored by their exact coefficients.

Connect a parameter to a monomial when it occurs, coloring the edge by its
exponent. Include every active parameter. Coefficient colors must retain actual
values, not only unrelated per-graph palette numbers. Symbolica's
`Graph::canonize()` and `CanonicalForm::vertex_map` provide the input-to-canonical
map; composing two such maps proposes the parameter permutation.

Graph equality or a fingerprint is only a proposal. Reconstruct the bijection,
check powers and support, and replay exact polynomial equality through the
native polynomial service before creating a proof. Bind the proof to the
family, source, representative, and normalization. Keep the reducer seam the
same as the existing aliases: a descending one-hop representative before
memoization, with raw catalog coverage unchanged. A future proof enum can
distinguish momentum-routing witnesses from parameter-polynomial witnesses.

## Cost and scope

For rank-one physical momentum forms, `U` is squarefree of degree `L`. Its term
count is at most `binomial(N,L)`; this is a combinatorial bound, not a timing
prediction:

| Loops | Cubic physical parent: active lines | Maximum terms | All K coordinates active | Maximum terms |
| --- | ---: | ---: | ---: | ---: |
| 4 | 9 | 126 | 10 | 210 |
| 5 | 12 | 792 | 15 | 3,003 |
| 6 | 15 | 5,005 | 21 | 54,264 |

An incidence graph has approximately `N + terms` vertices and at most
`L * terms` edges. The complete-coordinate six-loop bound is therefore already
substantial. Canonicalization may also encounter difficult symmetries; native
implementation alone does not make its cost negligible or supply a timeout.

Cache polynomial geometry by active support, reuse it across dotted terminals,
and inspect only declared terminal supports. Preflight polynomial and graph
sizes; do not enumerate all `2^K` sectors. Initially this is a plausible bounded
four-loop study. Actual preparation time, successful alias counts, RSS, and
reduction impact must be measured before extending or enabling the lane.

Necessary adversarial tests include power-color mismatch, changed overall
coefficient, zero-rank support, unsupported numerators/offsets/masses, nontrivial
permutation orientation, native context collisions, deterministic one-hop
representatives, and exact comparison with the existing saved terminal catalog.
Such a catalog comparison is an independent check of implemented identities,
not the source of those identities. Even a successful lane will not find every
dimension-dependent IBP relation or establish a minimal master basis.
