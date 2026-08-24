# Symbolica-native Symanzik polynomials and zero-sector certificates

Date: 2026-08-13

Status: implementation-ready source design.  This document specifies the next
generic RustRed layer; it does not implement it.  It was derived by reading the
vendored LiteRed Mathematica source, RustRed's authenticated generic-family and
sector foundations, and the vendored Symbolica Rust API.  No Mathematica or
FORM process is required by the design or its tests.

## 1. Decision summary

RustRed should add two loop-count-independent modules:

- `feynman_polynomials`: construct authenticated Symbolica polynomials
  `U`, `F`, `G = U + F`, and optionally `grad(G)` from any complete affine
  [`IntegralFamily`](../../src/generic_family.rs);
- `zero_sectors`: reproduce LiteRed's default Feynman-parametric rank
  criterion, but return replayable proof objects and preserve RustRed's
  distinction between an analytic zero and a user exclusion.

The construction is generic in the loop count, external-momentum count,
denominator basis, masses, invariants, rational family parameters, external
Gram matrix, and formal power shifts.  Concrete one-, two-, and three-loop
families occur only in tests and source-oracle comparisons.

The production zero test does **not** need rational-function Gaussian
elimination.  If a canonical monomial of `G` is

\[
  t_a=c_a x_0^{a_0}\cdots x_{N-1}^{a_{N-1}},\qquad c_a\in K^\times,
\]

then LiteRed's evaluated row is `c_a [a_0,...,a_{N-1},1]`, with a row
discarded when it contains an inactive Feynman parameter.  After deleting the
inactive zero columns and dividing each surviving row by its nonzero
coefficient, LiteRed's rank is exactly the rank over `Q` of an augmented
integer exponent matrix.  Symbolica constructs and canonicalizes `G`; its
exact rational `Matrix<Q>` API certifies the resulting exponent rank.

The outcome taxonomy must be conservative:

- rank at most the number of effective active parameters gives a sufficient,
  replayable zero-sector certificate;
- full column rank means only **no zero certificate from this criterion**;
- a cut or sector-pattern mismatch is `Excluded`, never `ProvedZero`;
- resource exhaustion and malformed data are typed outcomes, never zeros.

The existing `SectorAnalysisStatus::ProvedNonZero` must not be assigned by this
test.  Failure of one sufficient scalelessness criterion is not an analytic
proof that an integral is nonzero.

## 2. Exact LiteRed source correspondence

The authoritative source for this layer is
[`LiteRed2026.m`](../../vendor/LiteRed2/Source/LiteRed2026.m).

### 2.1 `FeynParUF`

The default option is `Sign -> Plus` at lines 4223--4226.  For denominators
`D_a` and Feynman parameters `x_a`, lines 4229--4248 do the following:

1. form `den = sum_a x_a D_a` (up to the global `Sign` option);
2. form `t1_r = (1/2) d den / d k_r`;
3. form `t2_rs = d t1_r / d k_s`;
4. set `U = det(t2)`;
5. set
   `F = U den|_{k=0} - Inner[sp,t1,adjugate(t2).t1]` for the default sign.

If `U` is identically zero, LiteRed returns `{0,0,xs}`.  The sector overload at
lines 4251--4258 either substitutes zero for absent top-level parameters or
reconstructs the polynomials from the selected denominators.  The overload at
lines 4264--4277 differentiates the parametric integrand for nonpositive
powers; that integrand/numerator operation is outside the zero-sector layer.

### 2.2 `AnalyzeSectors`

The default `FeynParUF -> True` path is lines 3017--3051:

```text
ps = Replace[PowerShifts[nm], Except[0] -> 1]
{u,f,xs} = FeynParUF[top sector]
rows = [x_0 d t/dx_0, ..., x_(N-1) d t/dx_(N-1), t]
       for every canonical monomial t of u+f
zero(mask) = MatrixRank(rows /. x_i -> mask_i) <= Count(mask,1)
effective_mask = BitOr[ps, raw_sector_mask]
```

Thus active parameters are evaluated at one and inactive parameters at zero;
the test is applied to the union of raw sector support and algebraically
nonzero power-shift support.  LiteRed then propagates a zero result to
subsectors and a failed-zero-test result to supersectors as an optimization.
RustRed v1 should instead test every distinct effective mask directly and
cache it.  That produces a certificate for every reported zero and avoids
depending on an unstated monotonicity argument.  Closure can be added later
only with its own proof and replay tests.

Line 3045 also mixes a missing cut with `chzf` and stores both in LiteRed's
`ZeroSectors`.  RustRed deliberately does not copy that representation:
[`sectors.rs`](../../src/sectors.rs) already specifies that cuts and patterns
are admissibility metadata rather than analytic proofs.

### 2.3 `FeynParGdG` and power-shift caveat

`FeynParGdG` at lines 4796--4801 caches the pure function `{G,grad(G)}`.  The
new polynomial module should expose the same mathematical data, although the
fast rank path can read canonical exponent vectors without materializing the
gradient.

LiteRed comments at lines 553--554 and 829--830 that nonzero integer shifts,
shifts differing by nonzero integers, and shifted/cut combinations need a
consistency policy.  This is an explicit TODO in LiteRed, not a solved
semantic issue.  RustRed must therefore make its formal-shift contract
explicit rather than silently treating arbitrary integer reindexings as
ordinary regulator shifts.

## 3. Input contract and conventions

Let the authenticated base field be

\[
  K=\mathbb Q(\theta_0,\ldots,\theta_{P-1}).
\]

An `IntegralFamily` supplies `L` loop momenta, `E` external momenta, and the
complete number

\[
  N=\frac{L(L+1)}2+LE
\]

of affine denominators.  Its coordinate order is all upper-triangular
loop-loop scalar products followed by loop-external scalar products in
loop-major order.  Write denominator `a` as

\[
D_a=c_a+
 \sum_{0\le r\le s<L} b_{a,rs}\,k_r\!\cdot k_s+
 \sum_{0\le r<L,\,0\le\alpha<E}
 e_{a,r\alpha}\,k_r\!\cdot p_\alpha .
\]

All `c`, `b`, `e`, external-Gram entries, and power shifts are authenticated
`Coefficient = RationalPolynomial<IntegerRing,u16>` values on exactly the
family's `CoefficientContext`.  The existing constructor already proves that
the denominator-coordinate matrix is generically invertible and records its
domain in `FamilyDomain`.  No inverse external Gram matrix is required, and a
singular external Gram matrix is a valid input.

The construction uses the actual stored denominator signs.  There is no
hidden Euclidean/Minkowski or propagator-sign normalization.  RustRed's
default corresponds to LiteRed's `Sign -> Plus`.  A future global sign option
`sigma in {+1,-1}` merely multiplies both `U` and `F` by `sigma^L`; it cannot
change the rank predicate.  Individual denominator signs remain data.

## 4. Generic construction of `U` and `F`

Introduce Feynman parameters `x_0,...,x_(N-1)` and define

\[
\begin{aligned}
C(x)&=\sum_a x_a c_a,\\
A_{rr}(x)&=\sum_a x_a b_{a,rr},\\
A_{rs}(x)=A_{sr}(x)&=\frac12\sum_a x_a b_{a,rs}\quad(r<s),\\
Q_{r\alpha}(x)&=\frac12\sum_a x_a e_{a,r\alpha},\\
H_{\alpha\beta}&=p_\alpha\!\cdot p_\beta .
\end{aligned}
\]

Then

\[
 \sum_a x_aD_a=k^T A k+2k^TQp+C,
\]

and LiteRed's default convention becomes

\[
\boxed{U=\det A},\qquad
\boxed{F=UC-
 \sum_{r,s,\alpha,\beta}
 Q_{r\alpha}\,\operatorname{adj}(A)_{rs}\,
 Q_{s\beta}\,H_{\alpha\beta}}.
\]

Compute and canonicalize `U` first.  If it is identically zero, return
`U=F=G=0` immediately, exactly as LiteRed does at line 4240; do not evaluate
the adjugate expression and accidentally obtain a different `F` for a
singular quadratic form.

For `E=0`, the second term is the empty sum.  Use the conventional adjugate

\[
 \operatorname{adj}(A)_{rs}=(-1)^{r+s}
 \det A[\text{delete row }s,\text{ delete column }r].
\]

The distinction between cofactor and adjugate indices should be encoded in a
unit test even though `A` is symmetric.  A zero-by-zero minor is one, so the
one-loop adjugate is `[1]`.

As internal replay checks, every nonzero monomial of `U` must have total
Feynman-parameter degree `L`, and every nonzero monomial of `F` degree `L+1`.
`G=U+F` is generally inhomogeneous.  Cancellation is performed in `K` before
any rank rows are extracted.

### 4.1 Recommended public types

```rust
type RawFeynmanPolynomial = MultivariatePolynomial<
    RationalPolynomialField<IntegerRing, u16>,
    u16,
>;

pub struct FeynmanPolynomialContext { /* authenticated K[x] map */ }
pub struct FeynmanPolynomial { /* raw value + context identity */ }

pub struct SymanzikPolynomials {
    context: FeynmanPolynomialContext,
    u: FeynmanPolynomial,
    f: FeynmanPolynomial,
    g: FeynmanPolynomial,
    family_fingerprint: Arc<str>,
}
```

Required methods are `try_from_family`, `u`, `f`, `g`, `try_gradient`, and
`try_restrict_face`.  The face operation should retain the full ordered
`x` map and set inactive variables to zero; an optional presentation method
may compress the remaining variables afterward.  Proof-bearing code always
uses the uncompressed family order.

The context owns:

- the exact family fingerprint and ordered base-parameter manifest;
- a deterministic, namespaced `x` manifest in denominator order;
- `RationalPolynomialField<IntegerRing,u16>` as coefficient field;
- a zero template with the exact `x` variable map;
- the base `CoefficientContext` and explicit construction limits.

Do not use `ParametricCoefficientContext`: that type means `K(n)` and its
variables are integral indices, not Feynman parameters.  Do not use Atom
pattern matching in this core; sparse typed polynomials have stronger map and
canonical-form guarantees.

### 4.2 Checked polynomial arithmetic

Raw Symbolica polynomial arithmetic normally unifies foreign maps and can
panic on exponent overflow.  The proof boundary needs checked wrappers for
zero, one, monomial, addition, subtraction, multiplication, coefficient
scaling, differentiation, and face evaluation.  Every wrapper must:

1. authenticate the context/family fingerprint and exact `x` map;
2. validate every `K` coefficient with `CoefficientContext`;
3. preflight `usize`, `u32`, and `u16` counts and exponent additions;
4. apply `ExactAlgebraLimits` to coefficient operations;
5. bound input terms, Cartesian term products, and output terms;
6. combine equal exponent vectors exactly and remove exact zero coefficients;
7. return a typed error before calling a panic-prone Symbolica path.

A simple implementation can accumulate terms in a `BTreeMap<Box<[u16]>,
Coefficient>`, use `CoefficientContext::try_add/try_mul/try_neg`, and finally
emit lexicographically sorted terms with `append_monomial_back`.  This is more
important than prematurely using the fastest unchecked multiplication.

### 4.3 Checked determinant and adjugate

Do not call `Matrix<PolynomialRing<_>>::det()` in the proof-bearing path.
Symbolica's determinant uses Bareiss for dimensions above three and unwraps an
assumed exact division; a violated precondition becomes a panic.  It also
cannot enforce RustRed's term/exponent budgets inside raw ring operations.

For the target through five loops, use a division-free subset dynamic program:

```text
dp[0] = 1
for mask in 0 .. 2^rows:
    row = popcount(mask)
    for column not in mask:
        sign = (-1)^(number of selected columns greater than column)
        dp[mask U {column}] += sign * dp[mask] * matrix[row,column]
det(matrix) = dp[all columns]
```

This is `O(L^2 2^L)` checked polynomial operations.  Apply the same routine to
the signed minors needed by the adjugate.  Preflight the number of states,
minor calls, term products, and aggregate operations.  `det([])=1` is handled
explicitly.  A checked fraction-free algorithm can replace this later, but it
must retain typed inexact-division and resource failures.

## 5. Symbolica APIs to use

The relevant vendored APIs are:

- sparse polynomial representation and public canonical arrays:
  `MultivariatePolynomial` at
  `vendor/symbolica/src/poly/polynomial.rs:290-300`;
- constructors and accessors: `new`, `constant`, `monomial`, `variable`,
  `nterms`, and `exponents_iter` at lines 335--451 and 486--568;
- sorted insertion at lines 825--890, `map_coeff` at 1452--1470, `degree` at
  1535--1549, `derivative` at 1607--1624, exact variable replacement at
  1778--1803, and full-point evaluation at 1890--1913;
- `PolynomialRing` at lines 30--88 and
  `RationalPolynomialField<IntegerRing,u16>` at
  `vendor/symbolica/src/domains/rational_polynomial.rs:40-56`;
- exact rational numbers `Rational` and field `Q` at
  `vendor/symbolica/lib/numerica/src/domains/rational.rs:26-28,699`;
- dense `Matrix<F>` and checked shape constructors at
  `vendor/symbolica/lib/numerica/src/tensors/matrix.rs:707-808`;
- exact field row reduction and rank at matrix lines 1607--1756.

`symbolica::tensors::matrix::Matrix` and the rational domain are publicly
re-exported by Symbolica.  For an empty rank matrix, return rank zero directly:
`Matrix::from_nested_vec(Vec::new(), Q)` divides by the inferred zero column
count at matrix line 803.  Prefer `from_linear` after checking that rows,
columns, and their product fit `u32` and `usize`.

`MultivariatePolynomial::replace` and `evaluate_with_coeff_map` are useful in
small independent test oracles.  Production face extraction should scan
canonical exponent vectors directly: it is cheaper and avoids constructing
temporary evaluated coefficient matrices.

## 6. Exact reduction of LiteRed's rank test

Let `S` be a raw sector and `P` the support mask of formal nonzero power
shifts.  Define the effective mask

\[
 T=S\cup P,\qquad q=|T|+1.
\]

For each canonical term `c_a x^a` of `G`:

- discard it if any `a_i > 0` with `i` inactive in `T`;
- otherwise append the integer row
  `[a_i for i active in T, 1]`.

Call the resulting matrix `E_T`.  LiteRed's row before evaluation is

\[
 [x_0\partial_0t_a,\ldots,x_{N-1}\partial_{N-1}t_a,t_a].
\]

At `x_i=1` for active and zero for inactive variables, a surviving row is

\[
 c_a[a_0,\ldots,a_{N-1},1].
\]

Inactive columns are zero, and every canonical `c_a` is a nonzero element of
the field `K`.  Row scaling and zero-column deletion prove

\[
 \operatorname{rank}_K(\text{LiteRed matrix at }T)
 =\operatorname{rank}_{\mathbb Q}(E_T).
\]

Therefore the exact predicate is

\[
 \boxed{\operatorname{rank}_{\mathbb Q}(E_T)\le |T|}
 \quad\Longleftrightarrow\quad
 \boxed{\operatorname{rank}_{\mathbb Q}(E_T)<q}.
\]

No family-parameter sampling, modular guess, floating point, or
rational-function pivot is needed.

### 6.1 Replayable kernel certificate

When the predicate holds, store a nonzero right-kernel vector

\[
 \lambda\in\mathbb Q^q,\qquad E_T\lambda=0.
\]

Construct it deterministically from Symbolica's RREF:

1. call `row_reduce(q)` after shape/resource preflight;
2. identify pivot columns and choose the smallest free column;
3. set that free coordinate to one, all other free coordinates to zero, and
   solve the normalized pivot equations;
4. clear denominators, divide by the integer gcd, and choose the sign whose
   first nonzero component is positive;
5. replay `E_T lambda = 0` with exact arithmetic before returning.

For a zero-row matrix, use a fixed unit vector and rank zero without entering
the matrix API.  The kernel expresses the quasi-homogeneous scaling relation

\[
 (\sum_{i\in T}\lambda_i x_i\partial_{x_i}+\lambda_q)G_T=0,
\]

which is the certificate behind LiteRed's sufficient scalelessness test.

Recommended persisted fields are:

```rust
pub struct ZeroSectorCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    g_fingerprint: Arc<str>,
    raw_sector: SectorMask,
    effective_sector: SectorMask,
    active_parameter_order: Box<[usize]>,
    primitive_kernel: Box<[Integer]>,
    rank: usize,
    domain: ZeroSectorDomain,
}
```

Replay must reconstruct `G` and `E_T` from the family, rather than trusting a
serialized row list.  Stable artifacts store names/manifests and canonical
coefficient text or application serialization, never process-local Symbolica
symbol ids.

### 6.2 Full-rank result

If `rank(E_T)=q`, return `NoZeroCertificate`, optionally with a selected
nonzero `q x q` exponent minor for debugging.  Never convert this to
`SectorAnalysisStatus::ProvedNonZero`.

A generic full-rank witness can lose rows when a monomial coefficient
specializes to zero.  If such a witness is persisted, it is valid only under
nonzero guards for the numerators of the selected monomial coefficients.  It
still proves only full rank of this criterion, not analytic nonvanishing.

## 7. Power shifts, cuts, and domain guards

### 7.1 Raw versus effective support

Raw sector membership remains exactly the existing convention

```text
active i:   n_i >= 1
inactive i: n_i <= 0
```

before every power shift.  The zero analyzer separately computes

```text
power_support[i] = !family.power_shifts()[i].is_zero()
effective[i] = raw[i] || power_support[i]
```

This reproduces LiteRed's `Replace[...,Except[0]->1]` and `BitOr`.  Algebraic
zero is tested on the authenticated rational-function numerator, not on a
formatted expression.

For a nonconstant nonzero numerator of `nu_i`, add the explicit generic-locus
condition `numerator(nu_i) != 0`, with a new flat provenance atom such as
`GuardOrigin::PowerShiftSupport { denominator: i }`.  A constant nonzero
numerator adds no condition.  Its denominator condition is already retained
by `FamilyDomain`.

This support guard matters.  A one-loop off-shell bubble with raw mask `10`
is a zero pinch; a formal nonzero shift in slot one changes its effective mask
to `11`, where the generic bubble is not certified zero.  At `nu_1=0` the
effective mask changes and the old decision must be discarded and recomputed.

### 7.2 Formal-shift policy

The proof API should require an explicit policy, initially
`PowerShiftPolicy::FormalGeneric`.  It means nonzero shifts are formal
regulators for sector analysis, with the support guards above.  A provably
nonzero integer shift must be rejected by the proof-bearing analyzer unless
the caller explicitly opts into a future reindexed-lattice semantics.  A
constant integer shift can cross zero inside one raw sector, so one effective
support bit cannot soundly describe every lattice point.  This restriction
does not alter generic IBP generation; it closes the consistency hole that
LiteRed itself labels experimental.

The same policy should diagnose shifted cut denominators and shifts differing
by a known nonzero integer.  Supporting those cases later requires a stated
sector/reindexing model, not merely another boolean mask.

### 7.3 Cuts and patterns

For every raw sector, call `SectorRestrictions::initial_status` first:

- `Excluded(exclusion)` is returned unchanged and no analytic test runs;
- only `Unanalyzed` sectors enter the effective-mask cache and rank test.

An exclusion must never produce a global zero rule.  If a future cut-integral
frontend wants the convention “a cut integral missing a cut propagator is
zero,” it must emit a cut-scoped rule with exclusion provenance that cannot be
applied to the uncut family.

### 7.4 Generic domain and specialization

Every certificate inherits all `FamilyDomain::conditions()`:

- denominators of supplied family coefficients;
- the numerator of the complete denominator-basis determinant.

No numerator guard is needed for ordinary `G` monomial coefficients in a zero
certificate.  On a specialization where such a coefficient vanishes, an
exponent row disappears, and deleting rows cannot increase rank.  This is a
useful one-way stability property of the exponent certificate.

The power-support guard is different because its failure changes the columns
and the face itself.  It cannot be omitted.  At any specialization where a
family denominator guard fails, the family/certificate is undefined and must
be reconstructed under a valid family definition.

## 8. Proposed analysis API and deterministic algorithm

```rust
pub enum ZeroSectorDecision {
    Excluded(SectorExclusion),
    ProvedZero(ZeroSectorCertificate),
    NoZeroCertificate(FullColumnRankWitness),
    ResourceLimited(ZeroSectorResource),
    Failed(ZeroSectorError),
}

pub struct ZeroSectorAnalysis {
    family_fingerprint: Arc<str>,
    symanzik: SymanzikPolynomials,
    decisions: Vec<(SectorMask, ZeroSectorDecision)>,
}
```

The current `SectorAnalysisStatus` may remain a lightweight presentation
enum, but this proof-bearing result must retain the certificate/error.  Map
only `ProvedZero` into its same-named status; leave `ProvedNonZero` unused.

All-sector analysis is:

```text
construct and replay U,F,G once
validate restrictions arity N
validate formal power-shift policy and construct support/domain guards
preflight sector count 2^N against max_sectors
for raw masks in stable index-major bit order:
    if restrictions exclude mask: record Excluded
    else:
        effective = raw OR power_support
        look up or compute exponent-rank decision for effective
        bind a replayed certificate/witness to this raw mask
return decisions sorted by raw bit string
```

Distinct raw sectors can share one effective-mask rank computation.  Cache
keys contain the family and `G` fingerprints, effective bit string,
power-shift policy, and certificate schema.  Restrictions are not part of the
analytic cache key because they are applied before it.

The computation for distinct effective masks is independent and may run in
parallel after all Symbolica symbols/contexts have been created.  Results are
collected and sorted before persistence, so scheduling never affects output.

### 8.1 Limits and errors

Add explicit limits for at least:

- Feynman-parameter count and `u16` total degree;
- polynomial input/output terms and Cartesian term products;
- determinant states, determinant operations, adjugate minors, and aggregate
  checked algebra operations;
- enumerated raw sectors and cached effective masks;
- rank rows, columns, entries, RREF operations, and certificate size;
- `u32` matrix dimensions and every checked `usize` product.

Failures should distinguish wrong arity, foreign context/family, malformed
canonical polynomial layout, unsupported power-shift semantics, exponent
overflow, exact-algebra failure, resource limit, matrix-shape failure, and
internal certificate replay failure.  No `unwrap`, `assert`, sampling, or
floating-point fallback belongs on input-dependent proof paths.

## 9. Test and oracle plan

Tests use concrete powers and topologies only as validation of this generic
implementation.  No production branch may match a loop count or topology
name.

### 9.1 Hand-derived `U/F` and sector tests

1. **One-loop tadpole**,
   `D_0=k^2+m^2`:

   \[
   U=x_0,\qquad F=m^2x_0^2.
   \]

   The massive top face has full column rank; the empty face is certified
   zero.  At `m^2=0`, the top face is certified zero.

2. **Massless one-loop off-shell bubble**,
   `D_0=k^2`, `D_1=(k+p)^2`, `p^2=s`:

   \[
   U=x_0+x_1,\qquad F=sx_0x_1.
   \]

   At generic `s`, the top face has full column rank and both pinches are
   certified zero.  At `s=0`, the top face is certified zero.  This is the
   required regression showing why a generic full-rank witness needs selected
   coefficient guards and is not an unconditional nonzero proof.

3. **Equal-mass two-loop sunset**,
   `D_0=k_0^2+m^2`, `D_1=k_1^2+m^2`,
   `D_2=(k_0-k_1)^2+m^2`:

   \[
   U=x_0x_1+x_0x_2+x_1x_2,
   \qquad F=m^2U(x_0+x_1+x_2).
   \]

   Exhaust all eight masks.  Faces with at most one active denominator are
   zero; massive two-edge and top faces are not certified zero.  The massless
   specialization is scaleless.

4. **Arbitrary affine basis**: adapt the rational/nonsymmetric one-loop and
   two-loop fixtures at
   [`tests/parametric_ibp_oracle.rs:586-727`](../../tests/parametric_ibp_oracle.rs)
   and the symbolic family at
   [`generic_family.rs:1568-1611`](../../src/generic_family.rs).  Compare
   `A,Q,C,U,F` with an independently assembled expression and verify every
   inherited family guard.

5. **External Gram without inverse**: use the existing two-external fixture at
   oracle lines 643--690, including a singular Gram specialization, to prove
   that only multiplication by `H` occurs.

### 9.2 Independent reproduction of LiteRed's matrix

For every small hand fixture and every mask, build a test-only matrix by:

1. materializing `x_i * G.derivative(i)` with Symbolica;
2. evaluating all `x` at the mask's zeros and ones;
3. forming `Matrix<RationalPolynomialField<IntegerRing,u16>>` over `K`;
4. comparing its exact rank with the exponent-only `Matrix<Q>` rank.

This exercises Symbolica's derivative/evaluation/field-rank path independently
of the optimized exponent scan.  Keep fixtures bounded because this test path
uses more rational-function arithmetic.

### 9.3 Power shifts and exclusions

- Give the off-shell bubble raw mask `10` and symbolic shift `nu_1`.  Verify
  effective mask `11`, condition `numerator(nu_1)!=0`, and invalidation at
  `nu_1=0`.
- Verify exact zero shifts do not enter support and constant nonzero
  noninteger shifts need no numerator guard.
- Verify a known nonzero integer shift produces the policy error, not a zero
  certificate.
- Exhaust cut and pattern combinations on a small family.  Every mismatch is
  `Excluded`; none appears in a zero-rule set or certificate cache.

### 9.4 Vakint source-only oracles

Vakint is an oracle artifact only; do not invoke FORM.

- The two-loop zero rules at
  `vendor/gammaloop/crates/vakint/form_src/alphaloop/integrateduv.frm:65-69`
  state that the equal-mass three-denominator family is zero when at least two
  powers are nonpositive.  Transcribe this downset and compare all eight
  masks with RustRed.
- The three-loop Mercedes/tetrahedron rules at FORM lines 252--279 provide a
  source list of zero patterns.  Transcribe them into a checked Rust test and
  compare all 64 masks.  First derive the edge-order permutation from the
  `vxs` incidence and `kk1..kk6` mapping at FORM lines 162--189; do not assume
  the FORM order equals RustRed's order.
- RustRed's test family routing is the generic affine encoding of
  [`THREE_LOOP_TETRAHEDRON_ROUTINGS`](../../src/three_loop.rs) (`k1`, `k2`,
  `k3`, `k3-k1`, `k1-k2`, `k2-k3`).  The production zero analyzer must know
  nothing about that constant.

Source oracle data should be stored as masks plus a comment with the exact
vendored line, not copied as executable FORM logic.

### 9.5 Property, replay, and resource tests

- random small complete affine families over exact small rationals: compare
  formula construction with direct completion of the square;
- exact equality of the exponent and derivative rank paths for all masks;
- certificate replay after clone, cache lookup, and deterministic
  serialization round trip;
- coefficient specialization can delete ordinary monomial rows without
  invalidating a zero kernel;
- foreign maps, malformed layouts, `u16` boundary exponents, `u32` matrix
  dimensions, term-product limits, determinant-state limits, and sector-count
  limits all return typed errors before allocation/panic;
- deterministic equality under parallel mask scheduling.

The repository's test runner should execute these as ordinary independent
tests under parallel `cargo nextest`; no test may depend on global mutation
order after its contexts have been initialized.

## 10. Implementation sequence and acceptance criteria

1. Add `FeynmanPolynomialContext` and checked sparse `K[x]` primitives.
2. Add generic `A,Q,C` assembly and replay their symmetry/map invariants.
3. Add checked determinant/adjugate and `SymanzikPolynomials` with hand tests.
4. Add face extraction, exponent matrices, deterministic kernel generation,
   and certificate replay.
5. Add power-support guards/policy and integrate `SectorRestrictions`.
6. Add exhaustive one- and two-loop tests, the independent derivative-rank
   path, then the source-only three-loop Vakint oracle.
7. Only after this layer is generic and replayed should rule discovery consume
   its proved-zero sectors.

This milestone is accepted only when:

- one implementation handles arbitrary authenticated `IntegralFamily`
  values without loop-count/topology branches;
- `U/F/G` match all hand and independent Symbolica oracles exactly;
- every reported analytic zero carries a replayed kernel certificate and all
  required generic-locus conditions;
- no full-rank outcome is called an analytic nonzero proof;
- cuts/patterns remain exclusions and never leak into uncut zero rules;
- formal power-shift support and its exceptional locus are explicit;
- all malformed/resource-limit tests fail safely; and
- the tests run in parallel with Symbolica's GMP-backed build, without FORM or
  Mathematica runtime use.

## 11. Scope boundary

This layer is the Feynman-parametric zero-sector part of LiteRed, not the
entire reduction engine.  It does not derive IBPs, solve sectors, find
symmetries, apply reduction rules, or reduce tensor numerators.  Those consume
its authenticated outputs in later layers.  Likewise, LiteRed's
`FeynParUF[dsl,lms,n]` numerator differentiation is not tensor reduction and
must not be substituted for the Vakint-inspired pure-Rust tensor frontend.

The older `VacuumFamily` can later receive a checked adapter into
`IntegralFamily`: its `Denominator::quadratic_form()` and `shift()` already
expose the needed affine data, with zero external momenta and zero power
shifts.  That adapter is a validation bridge for existing one- through
five-loop fixtures, not an alternative topology-specific core.
