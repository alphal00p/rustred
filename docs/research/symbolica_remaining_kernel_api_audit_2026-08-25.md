# Symbolica API audit for remaining symmetry and Symanzik kernels

Date: 2026-08-25

RustRed baseline: `52a7b12310501eadce2f410d5557bb664d1a0062`.
Symbolica baseline: `77c137481904b8a5531ede86e3ef36b82beed7fd`
(`symbolica` 2.2.0). This is a read-only production audit. The probe described
below used the public GMP-enabled API; no license value is stored here.

Post-audit implementation status: prioritized sequence items 1 and 2 have
landed in the affine-family-map V2 verifier.  `symmetry.rs` now delegates its
nonempty determinants, transpose, congruence, products, and matrix-vector
products through the authenticated public Symbolica boundary.  The integer
determinant in `symmetry_discovery.rs` and the Feynman-polynomial kernels remain
pending; the API findings below continue to govern those migrations.

## Decision

Every remaining generic algebra kernel in `symmetry.rs`,
`symmetry_discovery.rs`, and `feynman_polynomials.rs` has a public Symbolica
replacement. RustRed should retain shape/context authentication, physics
bookkeeping, resource admission, panic containment, and independent replay,
but it should not retain a determinant, polynomial collector, matrix product,
transpose, derivative, or substitution implementation.

Two narrow adapters are needed rather than new algebra:

1. extend the existing checked coefficient-matrix boundary with composed
   product/transpose and matrix-vector calls; and
2. add an authenticated polynomial-ring boundary whose scalar methods
   delegate to Symbolica's public polynomial operators.

The second adapter is necessary because the public `PolynomialRing` domain
does not retain a polynomial variable map in its `zero`, `one`, or `nth`
constructors, and because public ring/matrix operations have no typed resource
or cancellation channel. It must not implement polynomial arithmetic itself.

## Exact public types and constructors

RustRed's coefficient element already is
`RationalPolynomial<IntegerRing, u16>`. Its public field object is
`RationalPolynomialField<IntegerRing, u16>`, defined at
[`rational_polynomial.rs:38-58`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L38),
with native ring operations at
[`rational_polynomial.rs:803-923`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L803).
The corresponding dense matrix is:

```rust
Matrix<RationalPolynomialField<IntegerRing, u16>>
```

For proof-bearing RustRed coefficients, use the existing
`CheckedCoefficientField` indirectly through the checked functions in
[`symbolica_coefficient_matrix.rs`](../../src/symbolica_coefficient_matrix.rs),
not a bare field: bare zero/one elements have empty variable maps and ordinary
arithmetic may unify foreign maps.

The Feynman-parameter element and ring types are exactly:

```rust
type K = RationalPolynomialField<IntegerRing, u16>;
type FeynmanPoly = MultivariatePolynomial<K, u16>; // default LexOrder
type FeynmanPolyRing = PolynomialRing<K, u16>;
type FeynmanMatrix = Matrix<FeynmanPolyRing>;
```

`PolynomialRing::{new,from_poly}` are public at
[`polynomial.rs:69-88`](../../vendor/symbolica/src/poly/polynomial.rs#L69), and
its associated element is `MultivariatePolynomial` at
[`polynomial.rs:96-104`](../../vendor/symbolica/src/poly/polynomial.rs#L96).
Use the authenticated context template to construct constants and monomials
through `template.{zero,one,constant,monomial,variable}` at
[`polynomial.rs:330-455`](../../vendor/symbolica/src/poly/polynomial.rs#L330).

The symmetry-discovery determinant uses:

```rust
Matrix<IntegerRing> // entries are Symbolica Integer, field object is Z
```

All three boundaries should preflight dimensions and the `u32` entry product,
then use `Matrix::from_linear`, whose public signature is at
[`matrix.rs:765-786`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L765).
Do not use `from_nested_vec` for empty input.

## Handwritten-kernel mapping

| RustRed kernel | Exact public Symbolica operation | RustRed semantics that remain |
|---|---|---|
| `symmetry::checked_determinant` | Existing `determinant_of_coefficient_matrix`, ultimately `Matrix<CheckedCoefficientField>::det` | Treat `0x0` as one; retain determinant guards, context/output authentication, panic/resource mapping |
| `symmetry::verify_external_gram` (`C G C^T`) | `Matrix::transpose` plus two native `&Matrix * &Matrix` products | Shape checks and entrywise comparison with source Gram |
| `symmetry::derive_denominator_map` (`R_s T`, product with `R_t^-1`) | Native matrix products | Construction of the physics-specific `T`, affine constants, orientations, and denominator replay |
| Symmetry matrix-vector sums (`R_s h`, `P c_t`) | Native matrix product against a one-column `Matrix`, or checked `Vector::dot` | Affine sign convention and result placement |
| `symmetry_discovery::checked_integer_determinant` | `Matrix<Z>::det` | Finite-alphabet enumeration, conservative bit/work admission, typed incomplete result |
| Feynman `combine`, `mul`, `scale`, `neg`, `accumulate` | Public polynomial `+`, `-`, `*`, unary `-`, and `mul_coeff` at [`polynomial.rs:1124-1450`](../../vendor/symbolica/src/poly/polynomial.rs#L1124) | Prospective input/product bounds, context authentication, postflight output limits |
| `try_gradient` | `MultivariatePolynomial::derivative(variable)` at [`polynomial.rs:1607-1627`](../../vendor/symbolica/src/poly/polynomial.rs#L1607) | Variable-order iteration and authenticated output wrapping |
| `try_restrict_face` | Sequential native `replace(variable, &context_zero)` for inactive variables at [`polynomial.rs:1778-1803`](../../vendor/symbolica/src/poly/polynomial.rs#L1778) | Mask arity, full-map retention, call/output limits |
| Feynman subset-DP determinant | `Matrix<FeynmanPolyRing>::det` | `0x0 = 1`, native-call admission, output authentication |
| Feynman adjugate | Structural minor selection; each minor uses native `Matrix::det`; native negation supplies cofactor sign | Deleted-row/column choice and transposed cofactor placement; no public adjugate API exists |
| `Q adj(A) Q H` four-index loop | `Q.transpose() * adj(A) * Q`, followed by public `Vector::dot` with the lifted Gram entries | Lift each authenticated `K` coefficient with `template.constant`; preserve the declared contraction orientation |
| `U*C`, `F=U*C-contraction`, `G=U+F` | Native polynomial multiplication, subtraction, and addition | LiteRed sign convention and homogeneity replay |

`Matrix::transpose` is public at
[`matrix.rs:843-860`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L843),
matrix multiplication at
[`matrix.rs:1332-1356`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1332),
and `Vector::dot` at
[`matrix.rs:108-123`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L108).
The scalar-product-coordinate construction and the fresh direct denominator
replay in `symmetry.rs` are physics semantics, not generic matrix algebra, and
should remain. Their scalar operations already delegate to the authenticated
Symbolica coefficient context.

## Determinant behavior and mandatory guards

The correct public determinant is `Matrix<F: Ring>::det` at
[`matrix.rs:1040-1124`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1040).
It uses explicit formulas through size three and fraction-free Bareiss above
that. The Bareiss path tracks row-swap parity at lines 1081-1097 and applies
the sign at lines 1121-1122. Use this method, not `det_in_place`, whose swap
sign is defective in the pinned revision.

Boundary behavior:

- a nonempty singular square matrix returns `Ok(zero)`;
- a nonsquare matrix returns `MatrixError::NotSquare`;
- a `0x0` matrix returns `MatrixError::Singular`, while RustRed requires the
  standard empty determinant `1` for a vacuum external map and a one-loop
  adjugate minor; handle only this structural case before the native call;
- dimensions above three can panic if Bareiss `try_div` unexpectedly reports
  inexact division at [`matrix.rs:1106-1113`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1106);
  contain this as an internal algebra failure, never as zero/singular;
- the direct polynomial-ring zero returned for a singular matrix has an empty
  Feynman-variable map because `PolynomialRing::zero` constructs it that way
  at [`polynomial.rs:198-212`](../../vendor/symbolica/src/poly/polynomial.rs#L198).
  A checked polynomial-ring adapter should instead create zero/one from the
  authenticated template, and every native output must be reauthenticated.

Avoid the known-bad `Matrix::is_one`, `det_in_place`, single-`u32` rectangular
row indexing, and bare inverse paths. Use tuple indexing or `iter()` at
[`matrix.rs:934-946`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L934).

## Error, cancellation, and resource surface

Matrix addition/product panic on shape mismatch rather than returning a typed
error; product behavior is visible at
[`matrix.rs:1332-1356`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1332).
Transpose and product allocate directly. `MatrixError` has structural and
linear-system variants but no cancellation, allocation, scalar panic, or
resource-limit variant at
[`matrix.rs:1389-1432`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1389).
The generic `RingOps` methods return elements directly and `Ring::try_div`
returns only `Option` at
[`domains.rs:113-180`](../../vendor/symbolica/lib/numerica/src/domains.rs#L113).

Numerica defines `AbortCheck` at
[`utils.rs:46-49`](../../vendor/symbolica/lib/numerica/src/utils.rs#L46), but it
is not threaded through matrix, polynomial, GCD, quotient, or rational-
polynomial operations. Therefore RustRed can currently provide only:

1. shape, exponent, term-product, native-call-count, and retained-output
   admission before entry;
2. an unwind boundary for native panic and checked-adapter failures; and
3. full context/shape/term/exponent/retained-byte authentication afterward.

It cannot claim a hard bound on Symbolica's internal Bareiss, polynomial-GCD,
quotient, dense/heap multiplication, or allocation scratch. Existing
custom-schedule counters must be versioned into native admission/call/output
counters rather than retained with a false meaning.

## Public-API probe

A standalone GMP-enabled compile/behavior probe instantiated all three exact
matrix types. It confirmed:

```text
Matrix<PolynomialRing<RPF<Z,u16>,u16>>::det/product/transpose compile
nontrivial 4x4 polynomial Bareiss determinant succeeds with a row swap
singular polynomial 4x4 det is zero but has an empty variable map
0x0 det = Err(Singular)
0x0 transpose/product preserve 0x0; 0x3 transpose produces 3x0
native polynomial derivative and zero replacement preserve the x map
Matrix<RPF<Z,u16>>::det/product/transpose compile
Matrix<Z> swapped 4x4 determinant = -1
```

No vendored example exercises the exact nested polynomial-ring matrix type;
the generic public trait implementation and this compile probe establish the
boundary.

## Prioritized implementation sequence

1. **Symmetry determinant now:** use the existing checked coefficient
   determinant, with the explicit `0x0 -> 1` convention. Remove subset-DP
   determinant arithmetic and reinterpret obsolete determinant-state stats.
2. **Symmetry products next:** extend the existing adapter with one composed
   authenticated native session for transpose, matrix products, and
   matrix-vector products. Migrate `C G C^T` and denominator-map products;
   retain the independent physics replay.
3. **Integer determinant:** replace the private Bareiss prefilter with
   `Matrix<Z>::det`; keep the finite search and prospective bounds, catch
   native panic, and compare the returned `Integer` with canonical `+/-1`.
4. **Native Feynman scalar operations:** introduce the delegating checked
   polynomial-ring adapter, then move constructors, addition, subtraction,
   multiplication, scaling, negation, derivative, and face substitution.
   Delete the production `BTreeMap` collector after differential tests pass.
5. **Native Feynman matrices:** move `U` and every adjugate minor to native
   determinant; move the quadratic contraction to native transpose/products
   and dot. Preserve `0x0` minor one and adjugate index-placement tests.
6. **Validation:** run parallel debug and release differential tests for
   dimensions zero through six, swapped/singular matrices, arbitrary-precision
   coefficients, one-/two-loop `U/F/G`, gradients and faces, the adjugate
   identity, symmetry Gram/denominator replay, and algebraically equivalent
   input spellings.
