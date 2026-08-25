# Symbolica upstream gap audit (vendored 2.2.0)

Date: 2026-08-25

Audited Symbolica revision:
`77c137481904b8a5531ede86e3ef36b82beed7fd` (`symbolica` 2.2.0).
All findings below were checked against that exact vendored source. The
correctness findings were also reproduced in a standalone GMP-enabled Rust
probe using only public Symbolica APIs. No license value is stored here.

## Author-ready summary

The most urgent issues are that both dense and sparse inverse routines test
the rank/independence of the whole augmented matrix `[A|I]`, so the identity
half hides singularity. There are also independently reproduced correctness
bugs in dense/sparse `is_one`, dense in-place determinant sign, zero-row rank,
dense rectangular row indexing, and sparse inconsistency detection. The exact
rational-polynomial power API is infallible, panics above `u32::MAX` or on
coefficient-exponent overflow, and performs `e` full rational multiplications.
A fallible, cancellable, resource-observable exact-algebra boundary would make
these APIs substantially safer for embedding applications.

## Confirmed correctness defects

| Severity | Confidence | Finding and exact evidence | Smallest upstream correction |
|---|---|---|---|
| Critical | Source proof + runtime reproduction | **Dense `Matrix::inv` can return a value for singular matrices.** The generic path builds `[A|I]` at [`matrix.rs:1557-1564`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1557), then calls `row_reduce(m.ncols)` over all `2n` columns at [`matrix.rs:1566-1569`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1566). Since the appended identity makes the `n` augmented rows independent, this rank test cannot detect singular `A`. The generic path applies to size one and sizes four and above; sizes two and three have separate determinant guards at [`matrix.rs:1482-1555`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1482). Public-API probe: `Matrix::new(4,4,Q).inv()` returned `Ok(I)`, while `A * I` remained zero. | Restrict pivot/rank discovery to the original left-block width, e.g. `row_reduce(self.ncols)`, while still updating all augmented columns; add singular 1x1 and 4x4 regression tests. |
| Critical | Source proof + runtime reproduction | **Sparse `SparseMatrix::inv` has the same defect for every positive size.** It appends identity at [`sparse.rs:1057-1058`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1057), then asks `from_matrix_check_dependent` to test entire augmented-row independence at [`sparse.rs:1060-1064`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1060). That reducer has no left-block pivot limit ([`sparse.rs:1622-1640`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1622)). Public-API probe: the zero 4x4 sparse matrix returned `Ok(I)` as its inverse, and the replay product was zero. | Add a pivot-column limit to the reducer and require `n` pivots in the original left block before extracting the right half. |
| High | Source proof + runtime reproduction | **Dense `Matrix::is_one` accepts zero diagonal entries.** Its predicate is `(diagonal && is_one(e)) || is_zero(e)` at [`matrix.rs:1128-1133`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1128), so the all-zero matrix reports `is_one() == true`; it also does not require a square shape. This can mask the inverse defect if replay is checked only with `Matrix::is_one`. | Require square shape and use `if diagonal { is_one(e) } else { is_zero(e) }`. |
| High | Source proof + runtime reproduction | **Sparse `SparseMatrix::is_one` uses the ordinal in `values`, not CSR row/column coordinates.** See [`sparse.rs:379-384`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L379). Empty `values` makes every sparse zero matrix vacuously one, while `SparseMatrix::identity(2,Q)` reports false because stored-value ordinal 1 is interpreted as dense offset 1. | Require square shape and inspect `row_ptrs` plus `col_idcs`, requiring exactly one nonzero unit at each diagonal coordinate and no other nonzero value. |
| High | Source proof + runtime reproduction | **`Matrix::det_in_place` loses row-swap parity.** It calls `partial_row_reduce` and multiplies the resulting diagonal at [`matrix.rs:1589-1604`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1589), but row swaps at [`matrix.rs:1614-1619`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1614) return no parity information. Probe: `det([[0,1],[1,0]]) == -1`, while `det_in_place` returned `+1`. | Return swap parity from elimination or track it in `det_in_place`, then negate for odd parity. |
| High | Source proof + runtime reproduction | **Dense row indexing is wrong for rectangular matrices.** `Index<u32>` uses `nrows` as both stride and row length at [`matrix.rs:1223-1230`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1223); row-major storage requires `ncols`. Probe: row 1 of a 2x3 matrix containing `1..=6` returned `[3,4]` rather than `[4,5,6]`; other shapes can panic. Tuple indexing correctly uses `ncols` at [`matrix.rs:1233-1248`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1233). | Replace both `self.nrows` factors in `Index<u32>` with `self.ncols`; add wide and tall rectangular tests. |
| High | Source proof + runtime reproduction | **Sparse inconsistent-system detection is inverted.** Both checked reducer constructors classify a sole last-column entry as inconsistent only when the stored value `is_zero`: [`sparse.rs:1551-1577`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1551) and [`sparse.rs:1585-1615`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1585). An inconsistent row is `[0 ... 0 | b]` with `b != 0`, and ordinary CSR normally omits zero entries. `SparseMatrix::solve` depends on this check at [`sparse.rs:982-1008`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L982). Probe: solving `0*x = 1` returned `Ok([1])`. | Change the predicate to `!is_zero`; add serial and parallel consistent, underdetermined, and inconsistent tests. |
| Medium | Source proof + runtime reproduction | **`rank()` panics on a valid 0xN dense matrix for N > 0.** `rank` delegates to `partial_row_reduce(self.ncols)` at [`matrix.rs:1753-1756`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1753). Elimination starts with `i=0` and indexes `(i,j)` before testing whether any row exists at [`matrix.rs:1609-1615`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1609). Probe: `Matrix::new(0,3,Q).rank()` panicked at line 1614. | Return rank zero immediately when `nrows == 0`, or guard `i >= nrows` before the first index. |

### Additional determinant boundary inconsistency

`Matrix::det` reports `MatrixError::Singular` for 0x0 at
[`matrix.rs:1040-1049`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1040),
whereas `det_in_place` returns the multiplicative identity through its empty
diagonal product at
[`matrix.rs:1589-1604`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1589).
Whichever empty-determinant convention is intended, the two public methods
should agree. Confidence is high; severity is low to medium.

## Rational-polynomial power and embedding API gaps

### Power schedule and panic surface

Confidence: source proof plus runtime reproduction. Severity: medium for
correctness/robustness and high for large-power performance.

- The inherent `RationalPolynomial::pow` rejects `e > u32::MAX` by panic and
  then performs a linear `for _ in 0..e` repeated multiplication loop at
  [`rational_polynomial.rs:538-553`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L538).
- The `RationalPolynomialField` implementation of `Ring::pow` duplicates that
  behavior at
  [`rational_polynomial.rs:874-891`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L874),
  even though the public trait accepts `u64` at
  [`domains.rs:151-165`](../../vendor/symbolica/lib/numerica/src/domains.rs#L151).
- Every loop iteration is a complete rational-function multiplication. That
  operation computes two cross GCDs and may perform exact polynomial quotients
  before multiplying at
  [`rational_polynomial.rs:1065-1105`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L1065).
  Thus cancellation exists and is mathematically useful, but its work is
  unconditional and opaque to the caller.
- There is no degree preflight for the chosen exponent representation. A probe
  of `(x^65535)^2` with `E=u16` panicked at
  [`polynomial.rs:1488`](../../vendor/symbolica/src/poly/polynomial.rs#L1488)
  with `overflow in adding exponents`.
- The polynomial-level power routine has specialized and heap-based paths at
  [`polynomial.rs:2401-2443`](../../vendor/symbolica/src/poly/polynomial.rs#L2401),
  but the rational-polynomial power routines do not use those numerator and
  denominator power APIs.

Suggested upstream API: a fallible `try_pow` (or checked power session) that
returns typed exponent/domain/resource failures; checks `E` degree growth;
accepts optional work/term/output limits and an abort callback; and reports a
small operation census. The implementation could retain sparse repeated
multiplication when beneficial, but it should not expose a `u64` exponent that
panics at the `u32` boundary.

### Fallibility, cancellation control, and resource observability

Confidence: high from repository-wide public-API search. Severity: medium as an
API gap, high for proof-bearing or untrusted-input embeddings.

- `RingOps` arithmetic returns elements directly, `Ring::pow` returns an
  element directly, and only `try_inv`/`try_div` return `Option`:
  [`domains.rs:113-180`](../../vendor/symbolica/lib/numerica/src/domains.rs#L113).
  `Field::{div,div_assign,inv}` are also infallible at
  [`domains.rs:250-254`](../../vendor/symbolica/lib/numerica/src/domains.rs#L250).
  There is no typed scalar-error channel for matrix algorithms.
- `MatrixError` contains structural/linear-system errors but no scalar panic,
  allocation, cancellation, or resource-limit variants at
  [`matrix.rs:1389-1432`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1389).
  `Matrix::det`, for example, panics when a Bareiss division is not exact at
  [`matrix.rs:1106-1113`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1106).
- Dense constructors and inverse allocate directly rather than using fallible
  reservation: [`matrix.rs:715-724`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L715)
  and [`matrix.rs:1557-1559`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1557).
  The public API has no term, degree, retained-byte, scratch-memory, or scalar
  operation budget.
- Numerica defines an `AbortCheck` trait at
  [`utils.rs:46-49`](../../vendor/symbolica/lib/numerica/src/utils.rs#L46),
  but a repository-wide search finds no use of it elsewhere under
  `lib/numerica/src`; matrix, GCD, quotient, and rational-power operations do
  not accept it. Expression optimization has separate abort settings, but they
  do not propagate into these exact-domain APIs.
- `SparseMatrix::solve_parallel` performs the same serial forward reduction at
  [`sparse.rs:1095-1101`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1095)
  and parallelizes only back substitution at
  [`sparse.rs:1117-1123`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1117).

The useful upstream abstraction is not a different CAS representation. It is a
fallible exact-algebra session/context that can carry a cancellation predicate,
resource policy, and operation census through `Ring`/`Field`, dense/sparse
elimination, polynomial GCD/quotient, and rational power.

## Reproduction snapshot

The standalone public-API probe produced the following relevant observations:

```text
dense_zero_4_inv_ok=true
dense_zero_4_replay_is_zero=true
sparse_zero_4_inv_ok=true
sparse_zero_4_replay_is_zero=true
swap_det=-1
swap_det_in_place=1
empty_det=Err(Singular)
empty_det_in_place=Ok(1)
rank_0x3_panics=true
rectangular_row_1=[3,4]
dense_zero_2_is_one=true
sparse_zero_2_is_one=true
sparse_identity_2_is_one=false
sparse_zero_x_eq_one_result=Ok([1])
rpf_pow_above_u32_panics=true
rpf_pow_degree_overflow_panics=true
```

## RustRed exposure and mitigation

RustRed does not rely on the affected identity predicates or bare inverse as a
correctness oracle. Its checked Symbolica adapter independently computes a
determinant before inverse, authenticates every output coefficient, and checks
both inverse products entry by entry. It rejects empty matrices, uses tuple or
iterator access rather than `Index<u32>`, and wraps rational power with map,
degree, exponent, term, operation, and retained-byte admission plus unwind
containment. These are necessary mitigations for this vendored revision, not
substitutes for upstream fixes.
