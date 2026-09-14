# Symbolica API audit for modular parametric-IBP reconstruction

> Live-dependency update (2026-09-09): RustRed now tracks Symbolica's `dev`
> branch and is pinned at `3805d02ed6de0ee3fd3011cdf584cc3972aff40e` with
> `integer-gmp`, `float-mpfr`, and `tracing_max_level_info`. The inventory below
> remains the historical audit of revision `77c1374`; changed APIs must be
> rechecked against the live tree before new implementation work.

## Sector-parallel port audit (2026-09-14)

Against the pinned `3805d02` public API, its implementation, and the port's
actual call sites, the required finite-field, integer, polynomial, rational,
GCD, factorization, and incremental sparse operations are native services.
No new CAS primitive is needed for sector scheduling. In particular:

- `LicenseManager::{max_threads,execution_capabilities}` provides the native
  license/target thread limits. Caller-local unlocks do not propagate to workers.
- Numerica `SparseRowReducer::add_row` and its forward reduction are serial.
  `back_substitute_parallel` exists but is not used by this forward-only port.
- The used polynomial/rational, GCD, and factorization paths do not create
  additional pools. Native scratch is thread-local and source contexts are
  immutable shared data. This Rust dependency graph uses no BLAS/OpenMP path.
- A private Rayon pool confines current sector computation; ordinary nested
  Rayon calls reuse that pool. The library must not mutate the global pool or
  process environment to control unrelated application code.

The `solver` port uses the reference's name GPLU for its incremental elimination
workflow. Like the C++ implementation, its native forward kernel uses dense
scratch; this does not claim a new Gilbert--Peierls reachability kernel.
Sparse multivariate rational-function reconstruction remains deferred to
Symbolica rather than implemented in RustRed.

## Linear-cut, fixed-source, and ordering audit (2026-09-14)

This additional audit checked the actual local dependency pinned at
`3805d02ed6de0ee3fd3011cdf584cc3972aff40e`: public exports/signatures, their
implementations, and the new RustRed call sites. The following paths refer to
that live tree, not the historical `77c1374` inventory below.

| Required operation | Native service checked | RustRed responsibility |
| --- | --- | --- |
| Exact parameter/index rational functions | `RationalPolynomial<IntegerRing,u16>` in `vendor/symbolica/src/domains/rational_polynomial.rs` | Integral-key bookkeeping only; use native addition, multiplication, division, and normalization |
| Translate a source or pre-rule by integer shifts | `MultivariatePolynomial::shift_var` in `src/poly/polynomial.rs` | Translate numerator and denominator together with the integral key |
| Fix a removed cut to one | `MultivariatePolynomial::replace` | Specialize both numerator and denominator, reject a zero denominator, then request native GCD normalization |
| Authenticate the simple cut pivot | Native `degree`, `contains`, `replace`, `is_zero` | Check the domain-specific shape `n_i × nonzero parameter polynomial`; retain its parameter condition |
| Polynomial GCD and exact quotient | `MultivariatePolynomial::gcd` in `src/poly/gcd.rs`, `try_div_exact` in `src/poly/polynomial.rs` | Call native services; do not implement GCD or division algorithms |
| Rebuild a rational coefficient | `FromNumeratorAndDenominator::from_num_den` | Disable redundant GCD only for an invertible integer translation; enable it after specialization |
| Clear a row's denominators | Native GCD, exact quotient, and polynomial multiplication | A short shared adapter folds an LCD and scales the row while preserving caller-owned applicability conditions |
| Ordering permutation | No algebraic operation required | Validate a coordinate bijection and change only the reference's final two tie-breaks |

In particular, integer translation is a polynomial-ring automorphism and
preserves coprimality. It is sound to rebuild its already-normalized numerator
and denominator with `do_gcd=false`. Specialization is not an automorphism:
it can introduce common factors or a zero denominator, so the cut frame checks
the denominator and rebuilds with `do_gcd=true`. Native `replace` retains the
variable map; the prepared-source boundary requires the fixed variable to be
absent from coefficients, not deleted from their shared map.

The audit found native integer `lcm`, but no public batch LCD-clearing method
for a row of multivariate rational-polynomial coefficients. The row adapter
therefore uses `gcd` and `try_div_exact` to fold `C ← C × (D/gcd(C,D))`, then
scales each native numerator by `C/D`. It introduces no independent CAS
representation or arithmetic kernel. Original family conditions and cut-pivot
parameter conditions are retained before this operation.

The existing topology-generic `IntegralFamily::derivative_contraction` and
ordinary/LI generator supply the cut derivative identity; no additional
symbolic differentiation implementation is needed. The initial admission is
restricted to independent unshifted linear cuts with no noninteger power
offsets when cut preparation is requested. Unsupported incidence or shifted
geometry is reported explicitly. Noninteger offsets remain supported when no
cuts are removed.

No reconstruction, interpolation, CRT controller, or competing rational
function framework was added by this slice. Public univariate interpolation
and internal GCD reconstruction helpers do not constitute the awaited sparse
multivariate rational-function reconstruction service. That backend remains
deferred to Symbolica.

## Affine equality and oracle-notation audit (2026-09-14)

The new standalone `solver::AffineCase` geometry service was checked against
the pinned public APIs, native implementations, and its Rust call sites, with
an independent mathematical audit. It does not yet change sector search.

- Numerica `Matrix<Q>::row_reduce(N)` computes reduced elimination in the
  first `N` columns of `[A|b]`; residual constant rows detect inconsistency.
  Coordinate-only intersections reuse the existing service without a matrix.
- Native rational matrix `primitive_part` removes the rational content,
  clearing denominators. Native `Integer::gcd` and `quot_rem` check whether
  each resulting integer equation's coefficient gcd divides its constant.
  This check is necessary, not a general integer-system feasibility solver.
- Admission additionally requires an integral canonical unit-pivot chart.
  Fractional charts return `UnsupportedCongruence`; rational consistency is
  never presented as integer feasibility. General HNF/SNF and congruence-chart
  construction remain absent, not independently reimplemented here.
- Native polynomial `replace` and `replace_with_poly` apply the compiled
  chart; native subtraction/zero tests establish equality implications.
  Source coefficients must be translated **before** this substitution.
  Only candidate target shifts must satisfy the homogeneous constraints;
  ordinary source shifts and physical integral columns remain unrestricted.
- Sector inequalities remain caller-owned. This primitive proves selected
  empty cases but does not claim a general integer-polyhedron feasibility test.

The reference-comparison helper also uses Symbolica's parser for explicit
notation aliases such as `dot[p,p]` to the independent scalar `s`. Whole
function-call tokens are renamed; the delimiter flag for `[]` versus `()` is
syntax only. No algebra, kinematic substitution, index rewrite, or numerical
specialization is performed by this adapter. Native polynomial conversion and
rational-function arithmetic still perform all exact coefficient comparisons.
The helper is invoked only after generation; reference equations never enter
search. Native Atom replacement was also inspected, but this narrow token
adapter avoids redundant global symbol registration and index remapping.

No interpolation, rational reconstruction, or independent CAS arithmetic
was introduced by either slice.

## Scope and pinned dependency

This audit covers the public Rust API actually pinned by RustRed, with emphasis
on the operations needed by a modular, traceable parametric-IBP completion
pipeline. RustRed currently uses:

- Symbolica `2.2.0`, vendored at commit
  `77c137481904b8a5531ede86e3ef36b82beed7fd`;
- Graphica `2.1.0`, re-exported as `symbolica::graph`;
- the workspace features `gmp` and `tracing_max_level_info`, with default
  Symbolica features disabled.

The dependency and local patches are declared in `Cargo.toml`. The Graphica
re-export is in `vendor/symbolica/src/lib.rs` (`pub use graphica as graph`).

## Capability matrix

| Capability | Public API status | Consequence for RustRed |
|---|---|---|
| Incremental sparse row reduction | Available | Suitable for modular rank scouting and direct dependency patterns. |
| True Gilbert--Peierls reachability reduction | Not exposed | Do not describe the current reducer as GPLU; its forward path uses a dense scratch row. |
| Dynamic empty-column insertion | Available | Historical-zero columns can be introduced without rebuilding a reducer. |
| Retrofitting populated columns into old rows | Missing | A late column may only be inserted when all already-consumed rows are known to contain zero there. |
| Raw `L` pattern or full `L` multipliers | Available | Direct row-dependency incidence can be retained; expanded source circuits require RustRed bookkeeping. |
| High-level dependency/circuit tracing | Missing | Keep a thin domain-specific trace layer above the reducer. |
| Prime fields and prime iterators | Available | Modular scouting and independent-prime verification are supported. |
| Reusable optimized expression evaluators over rings | Available | Benchmark for batched coefficient evaluation; do not assume it beats sparse direct polynomial evaluation. |
| Exact multivariate rational-polynomial arithmetic | Available | Appropriate for small exact replay and final lifted identities, not global high-rank elimination. |
| Factorized rational functions | Available with caveats | Not a drop-in solution to expression swell; inversion may factor a numerator. |
| Scalar rational reconstruction and CRT | Available | Useful inside a controller, but insufficient for rational-function reconstruction. |
| Univariate Newton interpolation and Padé approximation | Available | Useful primitives for probes, not a complete multivariate pipeline. |
| Sparse multivariate rational-function reconstruction | Missing | Await the optimized Symbolica facility; do not build a competing CAS subsystem in RustRed. |
| Colored multigraph canonization/isomorphism | Available | Use Graphica for symmetry discovery and canonical graph keys. |
| Explicit propagator-edge action | Not returned directly | Derive only a thin, checked slot permutation from vertex maps and physics colors. |
| Hermite/Smith normal forms | Missing | Do not substitute dense echelon or LLL routines for HNF/SNF. |

## Sparse modular elimination

The relevant implementation is
`vendor/symbolica/lib/numerica/src/tensors/sparse.rs`:

- `SparseMatrix<F>` and its `from_csr`, `from_triplets`, `add_row`, `add_cols`,
  `append_col`, and `row_iter` methods;
- `LuLMode::{None, Pattern, Full}`;
- `SparseRowReducer<F>` and its `new`, `add_row`, `add_matrix`, `add_cols`,
  `u`, `l`, `pivots`, `back_substitute`, and
  `back_substitute_parallel` methods.

`SparseRowReducer::add_row` performs incremental forward reduction and returns
the new pivot column, or `None` for a dependent/empty row or when rank is
already full. The reducer chooses the leftmost surviving pivot. Its scratch row
is a dense `Vec`, and `forward_solve_row` scans columns linearly, so this is not
an exposed reachability-driven Gilbert--Peierls kernel.

`add_cols` inserts zero columns into the existing `U`, shifts pivots, and grows
the scratch row. It cannot inject nonzero values into rows already consumed by
the reducer. Consequently, a dynamically discovered column is safe to insert
only after RustRed has proved it was structurally zero in every historical row.
Insertion positions must be sorted and range-checked by the caller.

With `LuLMode::Pattern`, `l()` stores CSR row pointers and column indices but no
numeric values. Consumers must read `row_ptrs()` and `col_idcs()` directly;
generic sparse value iteration assumes values exist. The pattern is also
sample-specific: an unlucky modular zero can remove an edge. Full `L` records
multipliers, but recovering dependencies in terms of original source rows
still requires triangular/transitive trace expansion.

`back_substitute` consumes/clears `L`. The parallel variant documents that it
may do more total work and may emit differently ordered rows, so deterministic
artifact generation needs canonical sorting and independent replay. The
incremental forward path itself is serial. There is no public pivot callback,
rollback, column deletion/reordering, cancellation hook, or native resource
census. Callers must also reject zero pivots before any infallible inversion.

RustRed's existing modular sampler follows the safe pattern: it evaluates
numerator and denominator separately, rejects a zero denominator, and only then
divides. Its rank scout reads the pattern CSR directly, while exact replay uses
full `L` only for small selected circuits. Those choices match the public API.

## Finite fields and coefficient evaluation

The field API is in
`vendor/symbolica/lib/numerica/src/domains/finite_field.rs`:

- `Zp`, `Zp64`, `FiniteFieldCore`, `FiniteField`, and
  `FiniteFieldWorkspace`;
- `Mersenne32` and `Mersenne64` (the latter uses the fixed prime `2^61-1`);
- `PrimeIteratorU64`, `SmoothPrimeIterator`, and `PrimitiveRootIterator`.

`Zp::new` and `Zp64::new` enforce an odd modulus but do not establish
primality, so prime selection remains a checked caller responsibility. A fixed
Mersenne field is attractive for cheap scouting; independently selected
`Zp64` primes are appropriate for verification and CRT lifting.

Expression-side entry points are:

- `AtomCore::evaluate_in_ring`, `AtomCore::evaluator`, and
  `AtomCore::try_to_rational_polynomial` in
  `vendor/symbolica/src/atom/core.rs`;
- `EvaluatorBuilder` in
  `vendor/symbolica/src/evaluate/function_map.rs`;
- `ExpressionEvaluator::try_evaluate_in_ring`, `evaluate_in_ring`, and
  `map_to_ring` in `vendor/symbolica/src/evaluate/evaluator.rs`.

The evaluator can optimize shared work with Horner schemes and common
subexpressions. The JIT backends in `vendor/symbolica/src/evaluate/backend.rs`
target floating-point representations, not finite fields. Moreover, mapping a
rational numeric coefficient into a finite field can perform infallible modular
division. Prime/point validation and pole rejection must therefore remain at a
checked boundary; an optimized evaluator should be adopted only after a
representative benchmark and independent exact replay.

## Exact polynomial and rational-function operations

The principal exact types are:

- `RationalPolynomialField` and `RationalPolynomial` in
  `vendor/symbolica/src/domains/rational_polynomial.rs`;
- `MultivariatePolynomial` in
  `vendor/symbolica/src/poly/polynomial.rs`;
- `FactorizedRationalPolynomialField` and
  `FactorizedRationalPolynomial` in
  `vendor/symbolica/src/domains/factorized_rational_polynomial.rs`;
- public factorization and polynomial-GCD facilities in
  `vendor/symbolica/src/poly/factor.rs` and
  `vendor/symbolica/src/poly/gcd.rs`.

`RationalPolynomial` exposes its numerator and denominator and supports finite
field mapping, inversion, powers, GCD, evaluation, differentiation, and field
arithmetic. Addition uses denominator GCD/cancellation; multiplication uses
cross-GCDs. These are valuable exact operations, but repeated polynomial GCDs
and denominator cross-products inside a large elimination are precisely the
kind of work that can cause expression swell. Evaluation is infallible at the
field layer, so callers must precheck poles.

`MultivariatePolynomial` exposes coefficients, exponent storage, variables,
coefficient mapping, specialization/replacement, shifts, evaluation, and a
univariate rational approximant. The public positive exponent choices are
`u8`, `u16`, and `u32`; Symbolica recommends `u16` as its normal compromise.
There is no public `u128` exponent implementation.

The factorized rational representation is useful when factor structure is
already known, but inversion factors the numerator and can be expensive. It
also contains unfinished internal ordering functionality, so it should not be
treated as a general replacement for modular elimination plus reconstruction.

## Reconstruction boundary

Available building blocks include:

- `Rational::maximal_quotient_reconstruction` and
  `Rational::rational_reconstruction` in
  `vendor/symbolica/lib/numerica/src/domains/rational.rs`;
- integer CRT in
  `vendor/symbolica/lib/numerica/src/domains/integer.rs`;
- integer-polynomial CRT and a univariate rational approximant in
  `vendor/symbolica/src/poly/polynomial.rs`;
- Newton interpolation helpers in `vendor/symbolica/src/poly/gcd.rs`.

These primitives do not constitute a sparse multivariate rational-function
reconstructor. A production reconstruction controller still needs degree
discovery, normalization shifts, homogenized probes, sparse support learning,
balanced numerator/denominator recovery, cross-prime support alignment,
bad-prime and bad-point retries, batched shared samples, CRT/rational lifting,
probabilistic stopping, and independent verification.

RustRed should therefore expose a narrow internal black-box interface around
coefficient probes, sample metadata, candidate results, and verification. It
should not implement its own FireFly-style reconstruction engine. The final
optimized reconstruction capability should come from Symbolica, while RustRed
owns only IBP-specific sample scheduling, support/circuit selection, resource
budgets, and exact replay.

## Graph symmetry

Graphica's public API is in
`vendor/symbolica/lib/graphica/src/lib.rs`. Relevant symbols include `Node`,
`Edge`, `Graph`, `HiddenData`, `Graph::add_node`, `Graph::add_edge`,
`Graph::canonize_edges`, `Graph::canonize`, `Graph::is_isomorphic`,
`Graph::generate`, and `CanonicalForm`.

The implementation supports colored directed or undirected multigraphs,
self-loops, canonical labeling, isomorphism testing, vertex orbit generators,
and automorphism group sizes. It does not directly return generators acting on
propagator or parallel-edge slots. RustRed should encode physical distinctions
as graph colors, use Graphica for canonization, and derive only the checked
induced slot mapping needed by its integral representation. Reimplementing
graph isomorphism or canonicalization would be both slower and riskier.

## Integer normal forms and nearby facilities

No public HNF, SNF, integer-kernel, or lattice-saturation API is present in the
pinned sources. Nearby operations include dense fraction-free echelonization,
fraction-free solving in
`vendor/symbolica/lib/numerica/src/tensors/matrix.rs`, and integer LLL basis
reduction. These solve different problems and must not be presented as HNF/SNF
substitutes.

## Design and safety implications

The safe near-term architecture is:

1. scout rank and direct dependency patterns over checked finite fields;
2. register structural columns independently of their value at any one sample;
3. insert a late reducer column only when it is proven zero in all prior rows;
4. retain compact source-trace metadata outside Symbolica's reducer;
5. reject bad primes, poles, unlucky points, and inconsistent modular support;
6. use exact rational-polynomial arithmetic only for bounded candidate replay;
7. verify reconstructed rules at independent points and then exactly;
8. canonicalize output ordering before publishing an artifact.

Resource safety cannot be delegated completely to the current public API. The
caller needs checked dimensions and indices, term/row/column/nonzero budgets,
wall-time or cooperative abort checks around API calls, and panic isolation at
untrusted boundaries. Process isolation is the only hard memory ceiling for
long native polynomial/GCD operations because those operations expose no
scratch-memory callback.

The performance conclusion is equally clear: Symbolica already supplies the
fast algebraic primitives RustRed should reuse, but the pinned release does not
yet expose the complete sparse multivariate rational-function reconstruction
controller required to avoid exact K6-scale expression swell. RustRed should
not duplicate that CAS technology. It should keep the modular and tracing
layers generic, make the future reconstructor replaceable behind a narrow
interface, and use exact Symbolica arithmetic for verification rather than for
global elimination.
