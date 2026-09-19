# Exact parametric equivalence of vacuum terminals

Status: focused release validation and saved four-loop measurements passed,
19 September 2026. The core factory remains opt-in. Vakint's four-loop loader
now explicitly selects it in published revision `71e01122b`, pinning RustRed
`2b50267c`; its separate acceptance and benchmark gate is recorded below. The existing
momentum-routing terminal aliases and their four-loop measurements remain a
separate workstream. No new closure, master-minimality, or high-loop scaling
claim is made by this note; the scoped four-loop measurements are reported below.

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
Unsupported inputs and exhausted explicit preparation bounds leave the original
terminal unchanged. An internal mismatch during exact replay is a typed error,
not a silently accepted proposal or an unreported mathematical miss.

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

## Implemented opt-in interface

`TerminalAliasPlan::vacuum_parametric_equivalences(family, raw, ordering, limits)`
builds a fresh plan, rather than chaining the earlier momentum aliases with a
second proof. All representatives are existing declarations and strictly lower
under the caller's ordering. The old product and corank-one factories retain
their algorithms and still return momentum witnesses.

`TerminalAliasWitness` distinguishes `Momentum` from `Parametric`. A sealed
`VerifiedVacuumParameterMap` retains shared source/representative support
polynomials and exposes the source-local to representative-local parameter
bijection. No `VerifiedMap`, integer routing, or unit Jacobian is invented for a
parameter-integral proof.

The implementation reuses the admitted full-family native Symanzik construction
once. Each declared positive support is restricted with native `replace` and
`rearrange_with_growth`, then cached. Integer momentum-square admission is reused
from the existing product service. Only after that admission does nonzero `U`
establish active rank. Every retained coefficient is checked in the family's
ordered coefficient context and must be a positive integer; terms must be
squarefree of degree `L`.

Native graph canonicalization colors parameter vertices by their individual
positive powers, monomial vertices by actual exact `Integer` values, and edges
by exponents. Its proposed bijection is independently checked, replayed using
two-phase native `rename_variable` through collision-free temporary variables,
and aligned using native `rearrange_with_growth`. The complete polynomial and
ordered variable/coefficient contexts are then compared, without monic/content
normalization.

`VacuumParametricLimits` bounds the native Symanzik preparation policy, distinct
supports, canonicalization calls, graph vertices and graph edges. Defaults allow
4,096 supports, 16,384 graph calls, 4,096 vertices and 65,536 edges per graph.
These are structural admission bounds, not a native graph timeout or an RSS
guarantee. Nested exact-algebra budget errors also produce conservative raw-key
retention; malformed contexts and internal verification failures remain errors.

The API audit checked the native polynomial substitution/rename definitions,
the constant-map and copy-on-write regression tests, and native rename callers
in `poly/univariate/roots.rs`. Graph APIs were checked against their definitions,
the Graphica README canonicalization example and permutation-isomorphism test.
No RustRed determinant, factorization, substitution, graph-canonization or
rational-reconstruction kernel was introduced.

The combined release gate passed **75 tests** in 0.19 seconds: the previous 57,
six focused parameter-proof tests, ten independently authored adversarial tests,
and two candidate-reducer integration tests. The integration tests compare full
exact coefficient maps, common-mass exponents and memoized reuse against the
unchanged raw reducer. Direct verifier tests bypass graph proposals and reject
both unequal overall U scale and foreign-map constant coefficients. This is a
test-suite runtime, **not IBP generation or four-loop preparation time**.

Evidence is `TMP/terminal-parametric-reduction-release.log`; the release test
binary SHA-256 is
`4fd9c8e2b809b5e46eb6736e3cc7e009013f2a64b197ff557d6783542c0c5e9a`.
An initial compile-only integer type-inference error is preserved separately in
`TMP/terminal-parametric-reduction-compile-failed.log`; no assertions were relaxed.

## Saved four-loop census

No IBPs or catalogs were regenerated. The H/FG/BMW/X native candidate programs
were loaded once per fresh process, independently of preparation. The frozen
native catalogs were consulted only after preparation and four repeat timings.
Every proposed alias passed exact Symbolica comparison of its catalog projection.

| Family | Raw terminals | Prior routing representatives | U representatives | Exact aliases | Positive keys / supports | First preparation | Median of four repeat preparations |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| H | 386 | 176 | 52 | 334 | 356 / 314 | 147.579 ms | 124.505 ms |
| FG | 145 | 53 | 26 | 119 | 135 / 124 | 52.677 ms | 47.731 ms |
| BMW | 179 | 68 | 37 | 142 | 159 / 134 | 58.832 ms | 56.065 ms |
| X | 445 | 208 | 64 | 381 | 400 / 328 | 178.404 ms | 149.593 ms |
| Total | 1,155 | 505 | 179 | 976 | 1,050 / 900 | 437.492 ms | — |

The 179 representatives comprise **74 positive-power keys and all 105 unchanged
negative-index keys**, not 179 proven independent masters. No preparation bound
was reached. Repeated preparations agreed in statistics, canonical sets,
source/representative support slots and exact parameter permutations. The prior
routing column is a count comparison on identical inputs, not a new paired
performance benchmark against that implementation.

## Exact application and cost

Fresh raw and U-normalized processes used identical saved H/X programs and
`[2,1,1,1,1,1,1,1,1,0]` targets. All output coefficients were compared exactly
after independent raw-key coalescing, including their homogeneity exponents.

| Family | Raw first apply | U first apply | U preparation | Load + preparation + install + first apply, raw → U | Output terms | Median cached lookup, raw → U |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| H | 11.472871 s | 4.070188 s | 0.146766 s | 12.614351 → 5.316594 s | 248 → 46 | 54 → 9 µs |
| X | 61.555383 s | 20.189644 s | 0.176700 s | 63.700758 → 22.456571 s | 437 → 59 | 101 → 15 µs |

These individual pairs give 2.82×/3.05× first-application ratios and 2.37×/2.84×
ratios for the shown phase sums. H load times were 1.141480/1.099602 seconds
(raw/U), with a 38 µs installation; X load times were 2.145375/2.090179 seconds,
with a 48 µs installation. File reading, later exact comparison and compilation
are outside those phases. These are not whole-process wall-time ratios.

| Family | Accounted cache coefficient bytes, raw → U | Coalescing additions, raw → U | GNU whole-process peak RSS, raw → U |
| --- | ---: | ---: | ---: |
| H | 86,688,448 → 29,950,048 | 1,099,045 → 418,514 | 570,792 → 491,568 KiB |
| X | 381,494,842 → 123,085,704 | 5,889,714 → 2,139,884 | 1,480,872 → 1,079,860 KiB |

Accounted coefficient payload decreased by 65.45%/67.74%; these percentages are
**not RSS reductions**. Rule applications stayed at 26,956/82,637 and cached
integral counts at 27,306/83,082. This is coefficient-output coalescing within
the existing applier, not a different rule search or reduction algorithm.

The control target `[0,1,1,1,1,1,1,1,1,0]` was already a declared terminal:
both H and X used zero rules and returned one term. There is no speedup claim
for that workload. First applications were 17/401 µs for H and 18/326 µs for X
(raw/U), while the phase sums were 1.124981/1.261440 seconds and
2.278809/2.361002 seconds. The preparation overhead must be amortized. Cached
lookups rounded to 0 µs at the reporting resolution; they do not take zero time.

All twelve processes exited successfully with empty error logs. These are
shared-host, single-pair release diagnostics on CPUs 74–79 with nested pools
limited to one, not six-worker IBP-generation timings or confidence bounds.
The previous routing measurements used different CPUs; no controlled speed
ratio against them is claimed. Whole-process resource peaks can include
untimed validation work. Core defaults are unchanged. These isolated measurements
preceded the separate Vakint activation below.

Commands, exact outputs, repeat values, independent audit and a checked
13-entry source/binary/input/catalog/script hash manifest are retained in
`TMP/terminal-parametric-census.ooS5Jl/`. The measurement executable SHA-256 is
`ec965e9551e7c9a5de2283ab366f9e2157bf635b6f3ddfcd452b6599425336c7`.

## Vakint activation and matched public benchmark

GammaLoop `vakint_rustred` revision `71e01122b467870d351354e4809ed3fac158ef16`
pins both RustRed dependencies to `2b50267c5b1df10e4cd2798d7863d3f8c1b47383`.
Its explicit alias constructor now selects the parameter-equivalence factory
with default preparation bounds. The plain constructor, public evaluation
options, default backend order and FORM-backed modes remain unchanged. All
proof construction and coefficient accumulation stay in RustRed. No saved
programs, catalogs or expected numerical values changed.

An independent agent ran every one of the 15 original reference comparisons
and 16 expanded-numerator/pinch comparisons with forbidden FORM paths in the
FeynKit/RustRed lane; all pass. The unchanged 83-test through-three-loop
selection, nine focused constructor/catalog/loader checks and three fixture
checks also pass. The two four-loop correctness processes take 22.56/46.90 s,
including the separate FMFT oracle; these are not scalar-only timings. The
runtime revision and both benchmark executables were independently checked.

A fresh matched public benchmark uses the previous **routing-enabled** runtime
as its control. Each process evaluates the same nine scalar inputs through
both backends, with one initial pair and five alternating repeat pairs:
108 timed calls and 54 passing numerical comparisons per process. Timers
include scalar application, lazy loading/preparation on first parent use,
master substitution and public dispatch. Tensor preparation and final
numerical comparison lie outside each scalar timer. Later inputs may reuse
subproblems, so their initial calls are not fresh-parent measurements.

| Observation | Routing plan | Parameter plan | FMFT with parameter-plan run |
| --- | ---: | ---: | ---: |
| H, cubed first propagator, first call (s) | 7.177 | 6.263 | 1.485 |
| X, cubed first propagator, first call (s) | 31.272 | 29.450 | 5.052 |
| H, same input, warm median (ms) | 90.211 | 82.422 | 1476.941 |
| X, same input, warm median (ms) | 125.005 | 116.638 | 4798.671 |

These are much smaller incremental gains than the earlier raw-versus-normalized
core comparisons. First-use cubed-parent calls remain slower than FMFT; warm
RustRed calls are faster on the nine tested inputs. FG's expanded-D7 repeated
cost is effectively unchanged. A smaller terminal count does not imply a
proportional end-to-end speedup: loading, recursion and coefficient arithmetic
remain, and cache hits are not a forecast for unseen points.

Whole two-backend harness time is **98.78 → 95.83 s**, user plus system CPU
**94.32 + 3.81 → 91.82 + 3.40 s**, and peak RSS
**2,215,440 → 2,161,216 KiB**. These totals include both backends, initialization
and untimed comparisons, not only RustRed. Affinity is 88–93, one scalar caller
and nested pools capped at one. These are shared-host release diagnostics;
other compilation/validation used disjoint cores. They are not confidence
bounds or a claim of uniform speedup. Compilation and dependency fetches are
excluded from every reported timer.

All nine rows, the unchanged public reproduction command and the complete
boundary are documented in Vakint's `data/rustred/four_loop/README.md`.
Raw observations, frozen executable hashes, commands and independent audits
are retained in `TMP/gamma-terminal-u-rollout.oFoQwr/`. This publication does
not promote the four-loop candidates to independently certified artifacts or
assert a minimal terminal basis.

## First-use application profile

A separate read-only profile uses the same frozen release public executable
and U-normalized assets. All nine inputs and 54 numerical comparisons pass.
The first X cubed-parent call takes 29.332 s (the preceding unprofiled run took
29.450 s). These are single shared-host observations, not a profiler-overhead
estimate or a new speedup measurement.

The 99 Hz user-CPU recording contains 7,971 samples: 3,992 in the benchmark
process and 3,979 in separate FORM oracle processes. The analysis explicitly
excludes the latter. Its main window starts five seconds after the X request
and ends one second before the estimated RustRed return, retaining 2,012
parent-process samples. Existing output plus an external monotonic-clock
observer supplies approximate boundaries; the timing line itself appears only
after the FMFT call, so treating that receipt as RustRed's return would be wrong.
No samples were lost. This is a flat profile without caller stacks.

| Disjoint symbol-name group | X-interior samples | Fraction |
| --- | ---: | ---: |
| Integer/polynomial GCD names and helpers | 220 | 10.93% |
| Division/remainder/degree helpers | 278 | 13.82% |
| Polynomial content | 33 | 1.64% |
| Direct libc allocation/free/copy/set names | 307 | 15.26% |
| RustRed coefficient validation | 65 | 3.23% |
| RustRed indexed specialization | 82 | 4.08% |

These are exclusive symbol-name groups, not an exhaustive partition or
inclusive caller costs. Allocation may originate in arithmetic or traversal;
inlined work cannot be cleanly separated. No single self symbol dominates.
Decoding/inflation/graph-canonicalization names occur in the early window but
not this interior, supporting an application-dominated interpretation without
measuring an exact loading boundary.

The active coefficient type remains native Symbolica
`RationalPolynomial<IntegerRing, u16>`, with expanded numerator and denominator.
Symbolica also provides `FactorizedRationalPolynomial`, retaining denominator
factors but still expanding the numerator. The distributed arithmetic costs
justify a controlled experiment with that native representation, not a promised
speedup. A real coefficient-combine replay must include initial factorization,
output materialization, exact parity and memory measurements. It must not
replace the rule applier, weaken original denominator guards or introduce a
custom algebra kernel. Isolated-frame savings would not by themselves measure
a persistent factorized cache across the complete reduction DAG.

The frozen executable, collection/filter commands, exact symbol-bin membership,
raw sampling data, complete numerical output and independent audit are in
`TMP/vakint-u-application-profile.gxZUy1/`. The profile does not alter any
generation or certification result.
