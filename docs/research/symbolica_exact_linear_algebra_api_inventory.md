# Symbolica exact linear-algebra Rust API inventory

Status: source inventory of the vendored Symbolica 2.2.0 and Numerica 2.2.0
trees plus B0 adapter findings, 2026-08-24. The relevant checkout is
`vendor/symbolica`. Symbolica's GMP backend is the required RustRed
configuration; this inventory does not cover or authorize the `no_gmp`
feature.

## Mandatory RustRed policy

> **Public-Symbolica-first policy:** Before implementing any algebraic
> operation in RustRed, search Symbolica's public Rust API, its public
> rustdocs, examples, tests, and its internal production call sites. A public
> Symbolica operation is mandatory whenever it can supply the required exact
> operation. When a required LiteRed semantic is blocked by a documented and
> audited public-API gap, production stops at a typed unsupported boundary;
> the gap is not permission to grow a second CAS in RustRed. Such a record
> must identify the searched APIs and evidence, the missing semantic, why
> composition around the public operation is insufficient, and the exact
> upstream capability needed to remove the boundary.

Performance preference, familiarity with an existing RustRed routine, or a
desire for a different representation is not by itself a gap. RustRed may and
usually must wrap a Symbolica operation with authentication, resource
admission, panic containment, provenance, guard construction, and transaction
semantics. Those wrappers do not justify reimplementing the underlying
algebra.

The required search order for an exact matrix or solving task is:

1. Search the public exports and trait bounds in `symbolica::prelude`,
   `symbolica::tensors`, and `symbolica::domains`.
2. Search the dense and sparse matrix rustdocs and unit tests in Numerica.
3. Search Symbolica examples and expression-level solver documentation.
4. Search Symbolica's internal call sites for the intended domain and error
   handling.
5. Record any remaining semantic gap before writing algebra.

## Public export surface

Symbolica re-exports Numerica wholesale at
[`vendor/symbolica/src/lib.rs:180`](../../vendor/symbolica/src/lib.rs#L180).
It also exports `Matrix` and `Vector` from the ordinary prelude at
[`vendor/symbolica/src/lib.rs:167`](../../vendor/symbolica/src/lib.rs#L167).
The main concrete Rust paths are therefore:

- `symbolica::prelude::{Matrix, Vector}`;
- `symbolica::tensors::matrix::{Matrix, MatrixError, Vector}`; and
- `symbolica::tensors::sparse::{LuLMode, SparseMatrix,
  SparseMatrixError, SparseRowReducer, SparseVector}`.

The generic model is `Matrix<F: Ring>`, where the matrix stores values of
associated type `F::Element` and retains an `F` domain object. The public ring
traits and their associated-element model are documented in
[`vendor/symbolica/lib/numerica/src/domains.rs`](../../vendor/symbolica/lib/numerica/src/domains.rs).
Exact domains relevant to RustRed include:

| Exact domain | Ring object | Entry type and use |
|---|---|---|
| Integers | `Z: IntegerRing` | GMP-backed `Integer`; fraction-free solving when the result remains integral |
| Rationals | `Q: FractionField<IntegerRing>` | Exact `Rational`; ordinary field elimination |
| Rational functions | `RationalPolynomialField<IntegerRing, E>` | `RationalPolynomial<IntegerRing, E>`; exact parametric coefficients |
| Finite fields | `Zp`, `Zp64`, or `FiniteField<_>` | Modular images, rank probes, and reconstruction workflows |

`Q` and its exact type are defined at
[`vendor/symbolica/lib/numerica/src/domains/rational.rs:24`](../../vendor/symbolica/lib/numerica/src/domains/rational.rs#L24).
Its public checked `try_inv` and `try_div` ring operations are implemented at
[`rational.rs:502`](../../vendor/symbolica/lib/numerica/src/domains/rational.rs#L502)
and are the RustRed scalar error boundary. The public `Integer` enum exposes
`Single`, `Double`, and `Large` at
[`integer.rs:85`](../../vendor/symbolica/lib/numerica/src/domains/integer.rs#L85).
With the required `gmp` feature, `Large` contains the publicly re-exported
`rug::Integer`; RustRed's logical retained-payload census can therefore use
its allocated bit `capacity()` without estimating from the significant value.
The finite-field types and constructors are documented at
[`vendor/symbolica/lib/numerica/src/domains/finite_field.rs:147`](../../vendor/symbolica/lib/numerica/src/domains/finite_field.rs#L147).
`RationalPolynomialField::{new, from_poly}` is public at
[`vendor/symbolica/src/domains/rational_polynomial.rs:45`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L45),
and the type implements `Field` in the same file.
`RationalPolynomial::to_polynomial` is public at
[`rational_polynomial.rs:572`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L572).
It converts selected variables into polynomial indeterminates while keeping
the remaining variables in rational-polynomial coefficients and rejects a
selected variable in the denominator unless explicitly told to ignore the
denominator. This is the preferred decomposition seam for RustRed's index and
momentum-variable grouping; manual exponent-row copying is not required.

## Dense `Matrix` API

The implementation and inline rustdocs are in
[`vendor/symbolica/lib/numerica/src/tensors/matrix.rs`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs).

### Construction and inspection

The public `impl<F: Ring> Matrix<F>` begins at
[`matrix.rs:714`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L714).
Its relevant operations are:

| Operation | Source anchor | Contract |
|---|---|---|
| `Matrix::new` | [`matrix.rs:716`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L716) | Zero matrix with explicit row and column counts |
| `Matrix::identity` | [`matrix.rs:728`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L728) | Square identity |
| `Matrix::eye` | [`matrix.rs:746`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L746) | Matrix with caller-provided diagonal |
| `Matrix::new_vec` | [`matrix.rs:755`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L755) | Single-column matrix, used as a solver right-hand side |
| `Matrix::from_linear` | [`matrix.rs:765`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L765) | Checked row-major construction from a flat vector |
| `Matrix::from_nested_vec` | [`matrix.rs:789`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L789) | Rectangular nested-vector construction, but unsuitable for empty or zero-column input in 2.2.0 because it divides by the first row length |
| `nrows`, `ncols`, `field` | [`matrix.rs:811`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L811) | Dimensions and retained domain |
| `row_iter`, `iter`, `into_vec` | [`matrix.rs:826`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L826) | Row-major inspection or ownership extraction |
| `transpose`, `into_transposed` | [`matrix.rs:844`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L844) | Borrowed or owning transpose |
| `map` | [`matrix.rs:940`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L940) | Entry and domain conversion |
| `to_sparse` | [`matrix.rs:972`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L972) | Dense-to-CSR conversion |
| `augment` | [`matrix.rs:1000`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1000) | Checked `[A B]` construction |
| `split_col` | [`matrix.rs:1019`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1019) | Checked column split |

The matrix supports `(u32, u32)` entry indexing, row indexing, addition,
subtraction, multiplication, negation, and scalar multiplication. RustRed must
still preflight dimensions and allocation bounds before invoking constructors;
the generic API does not implement RustRed's resource policy.

RustRed B0 therefore uses `Matrix::from_linear`, not `from_nested_vec`. Its
adapter first authenticates rectangularity, checks conversion of both
dimensions to `u32`, checks the `usize` entry product and the constructor's
internal `u32` product, then reserves and clones one row-major buffer. This
also gives typed behavior for empty and zero-column matrices instead of
exposing the vendored constructor's division-by-zero panic.

### Ring-level determinant

`Matrix<F: Ring>::det` is public at
[`matrix.rs:1041`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1041).
It has direct formulas through dimension three and uses the fraction-free
Bareiss algorithm above that. The Bareiss implementation assumes exact
division in the chosen ring and panics if `try_div` unexpectedly fails. A
RustRed boundary using it must establish an appropriate exact domain, admit
work and memory, and contain a native panic. It must not reimplement Bareiss
merely to avoid writing that boundary.

### Fraction-free Euclidean-domain operations

The public `impl<F: EuclideanDomain> Matrix<F>` begins at
[`matrix.rs:311`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L311):

- `partial_row_reduce_fraction_free(max_col)` at
  [`matrix.rs:314`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L314)
  writes a non-reduced echelon form and returns the rank. It strips row
  content with gcds to limit coefficient growth.
- `back_substitution_fraction_free(max_col)` at
  [`matrix.rs:374`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L374)
  produces a row-reduced but not necessarily normalized form.
- `solve_fraction_free(&rhs)` at
  [`matrix.rs:420`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L420)
  solves `A x = b` when the solution belongs to the original Euclidean
  domain.

`solve_fraction_free` accepts only a one-column right-hand side. It reports
shape mismatch, inconsistency, and underdetermination explicitly. After
back-substitution it performs exact pivot divisions; failure produces
`MatrixError::ResultNotInDomain`. Over `Z`, this is an appropriate exact
integer-system operation and a useful oracle, but a rational answer should be
solved over `Q` instead of prompting a replacement solver.

### Field operations

The public `impl<F: Field> Matrix<F>` begins at
[`matrix.rs:1474`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1474):

| Operation | Source anchor | Behavior |
|---|---|---|
| `inv` | [`matrix.rs:1477`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1477) | Exact inverse; specialized dimensions two and three, augmented Gaussian elimination otherwise; requires an independent singularity guard in vendored 2.2.0 |
| `det_in_place` | [`matrix.rs:1589`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1589) | Determinant through field elimination |
| `partial_row_reduce` | [`matrix.rs:1609`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1609) | Forward Gaussian elimination on the first `max_col` columns |
| `back_substitution` | [`matrix.rs:1652`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1652) | Normalized backward elimination |
| `solve` | [`matrix.rs:1680`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1680) | Exact `A x = b`, one-column right-hand side |
| `solve_any` | [`matrix.rs:1721`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1721) | Returns one solution for an underdetermined system by selecting zero free entries |
| `row_reduce` | [`matrix.rs:1747`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1747) | In-place RREF and rank |
| `rank` | [`matrix.rs:1754`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1754) | Rank of a cloned matrix |

The built-in pivot rule scans for the first nonzero row in the current column.
There is no public callback for RustRed integral ordering or coefficient-cost
pivot selection.

The vendored generic inverse branch reduces all columns of `[A|I]`. For size
one and sizes four or larger, a singular coefficient block can therefore be
masked by pivots in the identity block and `inv` may return success. RustRed's
B0 adapter evaluates the independent native `Matrix::det` first, rejects zero,
and only then calls `inv`; sizes one through six and singular/nonsingular cases
are regression-tested. This is a checked composition of public Symbolica
operations, not a replacement inverse algorithm.

`MatrixError<F>` is defined at
[`matrix.rs:1391`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1391).
Its variants are `Underdetermined { rank, row_reduced_augmented_matrix }`,
`Inconsistent`, `NotSquare`, `Singular`, `ShapeMismatch`,
`RightHandSideIsNotVector`, and `ResultNotInDomain`. The retained
row-reduced augmented matrix is important evidence for test or adapter code;
RustRed must not collapse it before any required audit or partial-solution
construction.

## Expression-level exact solving

`AtomCore` exposes two convenient public trait methods:

- `AtomCore::solve_linear_system<E, T1, T2>` at
  [`vendor/symbolica/src/atom/core.rs:798`](../../vendor/symbolica/src/atom/core.rs#L798);
  and
- `AtomCore::system_to_matrix<E, T1, T2>` at
  [`vendor/symbolica/src/atom/core.rs:824`](../../vendor/symbolica/src/atom/core.rs#L824).

`solve_linear_system` interprets every input expression as equal to zero. For
an underdetermined system it returns `SolveError::Underdetermined` with the
rank and a partial solution in which high-index requested variables remain
free. `system_to_matrix` returns a typed pair
`(Matrix<RationalPolynomialField<Z, E>>, Matrix<...>)`, preserving access to
the exact matrix backend.

The implementation at
[`vendor/symbolica/src/solve.rs:321`](../../vendor/symbolica/src/solve.rs#L321)
does the following:

1. treats all indeterminates other than the requested unknowns as parameters;
2. converts expressions to exact `RationalPolynomial<Z, E>` values;
3. rejects nonlinear monomials;
4. unifies the parameter variable maps; and
5. builds a `Matrix<RationalPolynomialField<Z, E>>` and exact right-hand side.

When no parameters occur, the implementation takes an exact rational-field
path at
[`vendor/symbolica/src/solve.rs:503`](../../vendor/symbolica/src/solve.rs#L503).
Parameterized systems use ordinary `Matrix::solve` over the exact rational
polynomial field. Matrix errors other than underdetermination are currently
flattened to `SolveError::Other`, so code needing typed inconsistency evidence
should call `system_to_matrix` and then the matrix API directly.

`SolveError` is public at
[`vendor/symbolica/src/solve.rs:28`](../../vendor/symbolica/src/solve.rs#L28).
Relevant variants include `EmptySystem`, `NonLinearSystem`, and
`Underdetermined { rank, partial_solution }`.

The checked coefficient-conversion entry point is
`AtomCore::try_to_rational_polynomial` at
[`vendor/symbolica/src/atom/core.rs:1273`](../../vendor/symbolica/src/atom/core.rs#L1273).
The convenience `to_rational_polynomial` wrapper at
[`core.rs:1241`](../../vendor/symbolica/src/atom/core.rs#L1241) unwraps the
conversion result. Production RustRed ingress should prefer the checked form
or contain the convenience form behind a validated boundary.

## Sparse exact matrices and incremental reduction

The sparse implementation is
[`vendor/symbolica/lib/numerica/src/tensors/sparse.rs`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs).
`SparseMatrix<F: Ring>` uses CSR storage. Relevant public construction and
inspection APIs are:

- `SparseMatrix::new` at
  [`sparse.rs:407`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L407);
- `from_csr` and `from_csr_slices` at
  [`sparse.rs:427`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L427);
- ordered `from_triplets` at
  [`sparse.rs:495`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L495);
- `identity` and CSR accessors beginning at
  [`sparse.rs:532`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L532);
- `to_dense` at
  [`sparse.rs:896`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L896);
  and
- `row_iter` at
  [`sparse.rs:963`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L963).

For `F: Field`, the high-level operations are consuming `solve` at
[`sparse.rs:974`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L974),
`det` at [`sparse.rs:1012`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1012),
`inv` at [`sparse.rs:1052`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1052),
and `solve_parallel` at
[`sparse.rs:1084`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1084).
The parallel solver parallelizes back-substitution. `SparseMatrixError` at
[`sparse.rs:186`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L186)
distinguishes field mismatch, inconsistency, underdetermination, singularity,
and shape errors.

The closest public Symbolica operation to incremental IBP-row elimination is
`SparseRowReducer<F: Field>` at
[`sparse.rs:1497`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1497).
`LuLMode` at [`sparse.rs:1456`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1456)
chooses full `L`, its pattern, or no `L`. Public operations include:

- `new`, `from_matrix`, and `from_matrix_with_back_subs` beginning at
  [`sparse.rs:1519`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1519);
- consistency-checking and dependency-checking constructors through
  [`sparse.rs:1648`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1648);
- read-only `u`, `l`, and `pivots` at
  [`sparse.rs:1693`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1693);
- incremental `add_row` and `add_row_with_back_subs` at
  [`sparse.rs:1718`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1718);
- `add_matrix` at
  [`sparse.rs:1752`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1752);
  and
- `back_substitute` at
  [`sparse.rs:1811`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1811).

The reducer normalizes pivots by field inversion and exposes no custom pivot
policy. It is nevertheless mandatory to evaluate it, and composition around
it, before implementing sparse field elimination in RustRed.

Post-B0 source review found a stronger composition candidate than a simple
rank oracle. RustRed can reindex columns into hardest-first order, feed rows in
authenticated source order, select `LuLMode::Full`, and inspect public `u`,
`l`, and `pivots`; `add_cols` supports a growing authenticated column map.
This may retain the controller's ordering and enough elimination factors for
provenance while delegating row algebra to Symbolica. A transcript-equivalence
spike against `add_row`, `add_row_with_back_subs`, and `back_substitute` is
mandatory before claiming a custom sparse reducer is an irreducible API gap.

## Tests, examples, and documentation evidence

The most useful checked-in evidence is:

- [`vendor/symbolica/examples/solve_linear_system.rs`](../../vendor/symbolica/examples/solve_linear_system.rs):
  expression solving at lines 5--17 and explicit
  `RationalPolynomialField`/`Matrix::solve` use at lines 20--64;
- [`vendor/symbolica/src/atom/core.rs:760`](../../vendor/symbolica/src/atom/core.rs#L760):
  rustdocs for determined and underdetermined expression systems;
- [`vendor/symbolica/src/solve.rs:580`](../../vendor/symbolica/src/solve.rs#L580):
  expression and manual-matrix solver tests;
- [`vendor/symbolica/lib/numerica/src/tensors/matrix.rs:1758`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1758):
  dense exact tests over `Q`, including inverse, determinant, solve,
  `solve_any`, and row reduction;
- [`vendor/symbolica/lib/numerica/src/tensors/matrix.rs:1998`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1998):
  exact fraction-free solve over `Z`;
- [`vendor/symbolica/lib/numerica/src/lib.rs`](../../vendor/symbolica/lib/numerica/src/lib.rs):
  crate-level exact rational solve rustdoc; and
- [`vendor/symbolica/lib/numerica/Readme.md`](../../vendor/symbolica/lib/numerica/Readme.md):
  exact rational and finite-field matrix examples.

Internal production call sites confirm intended uses:

- [`vendor/symbolica/src/solve.rs:519`](../../vendor/symbolica/src/solve.rs#L519)
  uses `Matrix<RationalPolynomialField<Z, E>>::solve` for parameterized
  symbolic systems;
- [`vendor/symbolica/src/domains/algebraic_number.rs:882`](../../vendor/symbolica/src/domains/algebraic_number.rs#L882)
  uses `solve_fraction_free` and field solving in algebraic-number routines;
  and
- [`vendor/symbolica/src/poly/gcd.rs:903`](../../vendor/symbolica/src/poly/gcd.rs#L903)
  uses exact matrix solving inside polynomial GCD reconstruction.

These call sites must be searched again when the vendored Symbolica revision
changes.

## Suitability for RustRed

### Directly suitable uses

- Small exact integer and rational systems can use dense `Matrix<Z>` with
  fraction-free operations or `Matrix<Q>` with field operations.
- Exact affine-map composition can use ordinary rectangular `Matrix<Z>`
  multiplication. RustRed authenticates shapes, integer payloads, resources,
  and output geometry; it does not implement matrix dot products. The current
  boundary additionally supports allocation-free borrowed-entry admission, so
  a virtual affine matrix is checked before dense GMP staging, and a
  conservative prospective output-byte envelope is checked before the native
  product. Exact output capacity is authenticated afterwards.
- Small parametric systems can use
  `Matrix<RationalPolynomialField<Z, E>>`. This is the natural independent
  oracle for RustRed's typed coefficient elimination.
- Human-facing Symbolica equations can use `AtomCore::system_to_matrix` as a
  checked conversion seam, followed by typed matrix operations.
- Modular validation and reconstruction can use matrices over `Zp`, `Zp64`,
  or another `FiniteField` instantiation.
- Sparse concrete or modular systems can use `SparseMatrix` and, where its
  pivot semantics are acceptable, `SparseRowReducer`.

### Required composition around Symbolica

Using these public operations does not transfer RustRed's semantic
responsibilities to Symbolica. RustRed still owns:

- family, coefficient-context, variable-map, and allocation authentication;
- stable integral ordering and chronological event identity;
- explicit prospective and observed resource accounting;
- per-operation work limits and native panic containment;
- exact provenance and replay evidence;
- construction of nonzero pivot guards and `WhenBad` loci;
- consume-once transactional commit; and
- validation against differently written but algebraically identical input.

These are wrappers and certificates around Symbolica algebra, not reasons to
duplicate it.

## Audited semantic gaps for LiteRed-style reduction

The current public matrix API does not directly provide the following
LiteRed/RustRed semantics:

1. **Parametric pivot conditions.** Field elimination treats every nonzero
   rational-function entry as invertible. It does not emit the numerator and
   denominator nonzero conditions required for RustRed guards or `WhenBad`
   branch coverage.
2. **Integral-order pivot policy.** Dense and sparse routines choose their own
   first available pivot. They do not accept RustRed's persisted integral
   ordering, sector ordering, or coefficient-cost policy.
3. **Reduction provenance.** Solvers do not return a chronological trace tied
   to source-row, pivot-row, family, database, and session identities.
4. **Bounded execution.** Public routines do not accept RustRed's exact
   coefficient-operation, degree, bit-work, retained-byte, or combined-live
   limits.
5. **Transactional persistence.** Matrix results are ordinary values, not
   staged and preauthenticated database transitions with rollback evidence.
6. **IBP-key sparsity semantics.** Matrix columns are anonymous positions.
   RustRed must authenticate and preserve the mapping between columns and
   integral keys, including subsector and symmetry policies.
7. **Fraction-free sparse parametric reduction.** `SparseRowReducer` is a
   field algorithm that normalizes pivots; the public sparse surface does not
   expose a fraction-free Euclidean-domain reducer analogous to the dense
   one.
8. **Complete integer affine-lattice parameterization.** Exhaustive searches
   of the public dense/sparse matrix, polynomial, solving, and domain APIs found
   no Smith normal form, Hermite normal form, integer nullspace/kernel basis,
   or complete underdetermined affine-lattice parameterization. `Matrix<Z>`
   fraction-free solve supplies a determined solution, not the integral
   solution lattice. RustRed may use `Matrix<Z>` multiplication for a literal
   unit-pivot specialization, but a general no-unit or simultaneous-equality
   case must return a typed `RequiresIntegerNormalForm` unsupported result.

These gaps justify RustRed-owned semantic layers and typed completeness
boundaries. They do not justify custom integer, rational, polynomial,
rational-function, determinant, matrix, lattice-normal-form, or finite-field
arithmetic.

## Decision record for future algebra work

Any future RustRed change that adds algebra should record:

- the operation required by the LiteRed parity specification;
- all searched public Symbolica types and methods from this inventory;
- the relevant Symbolica tests, examples, and internal call sites;
- whether the public operation can be used directly or behind a bounded
  adapter;
- if not, the precise semantic gap and why composition cannot close it;
- the exact domain and exponent type selected;
- panic, allocation, and resource-limit behavior; and
- independent validation against the closest public Symbolica operation.

This record is part of the implementation requirement, not optional research
commentary.
