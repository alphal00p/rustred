# Symbolica-first algebra migration audit

Status: production-code audit and implementation record, begun 2026-08-24 and
updated 2026-08-25. This document records the migration required by RustRed's
public-Symbolica-first policy and the completed exact-matrix, affine-composition,
strict polynomial-associate, generic-family coefficient-matrix, and
affine-family symmetry slices.
Later priorities remain an audit plan rather than a claim that all production
algebra or all matrix consumers have migrated.

The companion API inventory is
[`symbolica_exact_linear_algebra_api_inventory.md`](symbolica_exact_linear_algebra_api_inventory.md).
The source-line-backed author escalation for defects and missing embedding APIs
in the pinned vendored revision is
[`symbolica_upstream_gap_audit_2026-08-25.md`](symbolica_upstream_gap_audit_2026-08-25.md).

## Executive decision

RustRed already uses Symbolica's GMP-backed rational-polynomial values for
most coefficient arithmetic, but several default-production modules still
implement exact rationals, matrix algorithms, polynomial algorithms, or
integer primitives themselves. Those implementations are not permitted when
a public Symbolica operation supplies the same algebra.

The first blocking migration after the chronological event-ledger milestone
was [`src/exact.rs`](../../src/exact.rs). B0 is now implemented: the public
`ExactRational` name is a private-field nominal wrapper around Symbolica's GMP
`Rational`, every scalar operator forwards to Symbolica, and the handwritten
gcd, normalization, Gaussian elimination, rank, multiplication, transpose,
and determinant code has been deleted. Checked adapters now call
`Matrix<Q>::{inv,rank,det,transpose}` and native matrix multiplication.

The generic-family, automatic-ISP-rank, tensor-projector, and affine-family
symmetry P1 matrix slices are complete. Their determinant, rank, inverse,
coefficient-power, transpose, and matrix-product operations now run through
Symbolica's public APIs over a checked contextual coefficient field. The
remaining P1 matrix rows in this audit are still pending unless their table
row explicitly says otherwise.

RustRed may continue to own LiteRed semantics around Symbolica operations:
integral ordering, pivot-condition guards, `WhenBad` branches, source-row
provenance, chronological replay, resource admission, panic containment, and
transactional commit. Those wrappers are not permission to reimplement the
underlying arithmetic.

The current dependency configuration is correct:
[`Cargo.toml:36`](../../Cargo.toml#L36) enables Symbolica's `gmp` feature and
does not enable `no_gmp`. Every migration and validation build must preserve
that configuration.

## Public APIs that form the replacement boundary

The exact replacement surface in the vendored revision is:

- the `Q` domain and `Rational` element for GMP-backed exact rationals at
  [`rational.rs:24`](../../vendor/symbolica/lib/numerica/src/domains/rational.rs#L24)
  and [`rational.rs:698`](../../vendor/symbolica/lib/numerica/src/domains/rational.rs#L698);
- `RationalPolynomialField::{new, from_poly}` at
  [`rational_polynomial.rs:45`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L45);
- the public `From<MultivariatePolynomial>` conversion for
  `RationalPolynomial` at
  [`rational_polynomial.rs:144`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L144)
  and `RationalPolynomialField`'s native `Ring::mul` at
  [`rational_polynomial.rs:817`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L817);
- `RationalPolynomial::to_polynomial` at
  [`rational_polynomial.rs:572`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L572),
  which splits selected variables into native polynomial indeterminates with
  rational-polynomial coefficients;
- `MultivariatePolynomial::map_exp` at
  [`polynomial.rs:1661`](../../vendor/symbolica/src/poly/polynomial.rs#L1661),
  which widens authenticated exponent storage without changing coefficient
  arithmetic;
- `PolynomialRing::{new, from_poly}` at
  [`polynomial.rs:69`](../../vendor/symbolica/src/poly/polynomial.rs#L69);
- `Matrix::{from_linear, from_nested_vec, transpose}` at
  [`matrix.rs:765`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L765),
  matrix operators, and `Matrix::det` at
  [`matrix.rs:1041`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1041);
- field matrix `inv`, `solve`, `row_reduce`, and `rank` beginning at
  [`matrix.rs:1474`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1474);
- fraction-free integer operations beginning at
  [`matrix.rs:311`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L311);
- `SparseMatrix` and `SparseRowReducer` beginning at
  [`sparse.rs:407`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L407)
  and [`sparse.rs:1497`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1497);
- `MultivariatePolynomial::derivative`, `replace`, `replace_with_poly`,
  `evaluate_with_coeff_map`, `add_variables`, `shift_var`, and `pow` at
  [`polynomial.rs:1608`](../../vendor/symbolica/src/poly/polynomial.rs#L1608),
  [`polynomial.rs:1780`](../../vendor/symbolica/src/poly/polynomial.rs#L1780),
  [`polynomial.rs:1938`](../../vendor/symbolica/src/poly/polynomial.rs#L1938),
  [`polynomial.rs:1891`](../../vendor/symbolica/src/poly/polynomial.rs#L1891),
  [`polynomial.rs:769`](../../vendor/symbolica/src/poly/polynomial.rs#L769),
  [`polynomial.rs:2008`](../../vendor/symbolica/src/poly/polynomial.rs#L2008),
  and [`polynomial.rs:2402`](../../vendor/symbolica/src/poly/polynomial.rs#L2402);
- the general `Ring::pow` operation at
  [`domains.rs:151`](../../vendor/symbolica/lib/numerica/src/domains.rs#L151);
- `AtomCore::expand` and `expand_via_poly` at
  [`core.rs:522`](../../vendor/symbolica/src/atom/core.rs#L522);
- `Integer::gcd`, `Integer::extended_gcd`, and `Integer::is_prime` at
  [`integer.rs:1576`](../../vendor/symbolica/lib/numerica/src/domains/integer.rs#L1576),
  [`integer.rs:1614`](../../vendor/symbolica/lib/numerica/src/domains/integer.rs#L1614),
  and [`integer.rs:1433`](../../vendor/symbolica/lib/numerica/src/domains/integer.rs#L1433);
- `Zp`, `Zp64`, and `FiniteField` in
  [`finite_field.rs`](../../vendor/symbolica/lib/numerica/src/domains/finite_field.rs).

RustRed should convert authenticated vectors into a Symbolica matrix only at
the algebra boundary and convert the result back into certificate-owned
storage afterward. Shape, allocation, guard, and provenance checks remain on
both sides of that call.

## Prioritized migration table

| Priority | Current production code | Classification | Exact Symbolica replacement | Required outcome |
|---|---|---|---|---|
| B0 (complete) | [`exact.rs`](../../src/exact.rs): formerly fixed-width rational normalization, gcd, inverse, rank, multiply, transpose, determinant | Migrated | `Rational`/`Q`; `Matrix<Q>::inv`, `rank`, `det`, multiplication and `transpose` | The compatibility `ExactRational` wrapper owns only a Symbolica `Rational`; its operators are forwarding adapters, not an arithmetic implementation. |
| P1 (complete) | [`generic_family.rs:1888-1919`](../../src/generic_family.rs#L1888): symbolic inverse, determinant and inverse verification | Migrated | `Matrix<CheckedCoefficientField>::det`, `inv`, and native matrix multiplication in [`symbolica_coefficient_matrix.rs`](../../src/symbolica_coefficient_matrix.rs) | Symbolica owns the determinant, inverse, and both products. RustRed retains the authenticated ordered coefficient context, determinant nonzero condition, resource admission, typed errors, and entrywise two-sided replay. |
| P1 (complete) | [`automatic_isps.rs`](../../src/automatic_isps.rs): formerly hand-written Gaussian rank | Migrated | Authenticated `Matrix<CheckedCoefficientField>::partial_row_reduce` through [`symbolica_coefficient_matrix.rs`](../../src/symbolica_coefficient_matrix.rs) | Symbolica owns every pivot, inverse, multiplication, subtraction, and row reduction. RustRed retains deterministic candidate order, native-operation/resource admission, the rank-progression certificate, and replay. New certificates use the V2 schema because their exact work census describes Symbolica's schedule. |
| P1 (complete) | [`tensor.rs`](../../src/tensor.rs) and [`generic_tensor_projector.rs`](../../src/generic_tensor_projector.rs): formerly handwritten Gram inversion and binary coefficient exponentiation | Migrated | Authenticated `Matrix<CheckedCoefficientField>::inv` plus determinant/two-sided replay; public `RationalPolynomialField::pow` | Symbolica owns every Gram inverse, verification product, and coefficient power. RustRed retains pairing enumeration, contraction connectivity, determinant/inverse-denominator guards, typed resource admission, and replay. V2 projector schemas replace pivot-schedule provenance with the basis-independent Gram determinant. |
| P1 (complete) | [`symmetry.rs`](../../src/symmetry.rs): formerly a subset-DP coefficient determinant | Migrated | Authenticated `Matrix<CheckedCoefficientField>::det` through [`symbolica_coefficient_matrix.rs`](../../src/symbolica_coefficient_matrix.rs) | Symbolica owns every nonempty determinant; RustRed retains nonzero guards, typed admission/panic boundaries, and the structural vacuum convention `det(0x0)=1`. V2 certificates report native calls and admitted versus actual work; the legacy subset-state census is always zero. |
| P1 | [`symmetry_discovery.rs:1209-1275`](../../src/symmetry_discovery.rs#L1209): private integer Bareiss determinant | Must replace now | `Matrix<Z>::det` | Keep candidate enumeration and conservative integer-bit/work admission. Symbolica's matrix determinant is already fraction-free/Bareiss. |
| P1 (complete) | [`symmetry.rs`](../../src/symmetry.rs): formerly handwritten `C G C^T`, `R_s h`, `R_s T R_t^-1`, and `P c_t` kernels | Migrated | Authenticated public Symbolica transpose and matrix multiplication through [`symbolica_coefficient_matrix.rs`](../../src/symbolica_coefficient_matrix.rs) | Symbolica owns every ordinary matrix operation. RustRed retains scalar-product-coordinate construction, affine sign/placement semantics, complete guard provenance, and a fresh direct denominator replay that does not consume the retained scalar-product map. |
| P1 | [`feynman_polynomials.rs:969-1080`](../../src/feynman_polynomials.rs#L969): subset-DP determinants for `U` and every adjugate minor | Must replace now | `Matrix<PolynomialRing<RationalPolynomialField<Z,u16>,u16>>::det` | Minor selection/adjugate placement remains structural bookkeeping. The determinant of each selected matrix is native. |
| P2 | [`feynman_polynomials.rs:294-380`](../../src/feynman_polynomials.rs#L294) and [`feynman_polynomials.rs:491-717`](../../src/feynman_polynomials.rs#L491): derivative, face restriction, add/subtract/multiply/scale and collection over a native polynomial type | Must replace now | `derivative`, `replace(..., 0)`, native `+`, `-`, `*`, coefficient-ring operations | Keep prospective exponent/term limits, context authentication and panic containment. |
| P2 | [`feynman_polynomials.rs:887-924`](../../src/feynman_polynomials.rs#L887): standard adjugate/Gram quadratic contraction | Must move native matrix kernels | Matrix multiplication/transpose over `PolynomialRing<RPF,u16>` | Preserve the Feynman-polynomial construction formula and homogeneity checks. |
| P2 | [`base_specialization.rs:677-768`](../../src/base_specialization.rs#L677): manual polynomial evaluation and binary exponentiation | Must replace now | `evaluate_with_coeff_map` over `RationalPolynomialField`; `Ring::pow` | Retain family-domain guard classification and operation/resource admission. |
| P2 | [`coefficient.rs:1260-1331`](../../src/coefficient.rs#L1260): manual exponent-row reconstruction after dropping a parameter | Must replace now | After the existing proof that every dropped exponent is zero, use `evaluate_with_coeff_map` into the target `PolynomialRing<Z,u16>`; map retained variables to target generators and the dropped variable to zero | Preserve malformed-layout, dependence and exponent-range diagnostics. There is no need for a private term copier once absence has been authenticated. |
| P2 | [`ParametricCoefficientContext::extend_base_polynomial`](../../src/parametric_coefficient.rs): manual extension of every exponent row from the base map to the parametric map | Must replace now | Clone the authenticated base polynomial and call `MultivariatePolynomial::add_variables` with the index variables | Preserve exact variable-order/context validation and retained-memory admission. |
| B0 (complete) | [`parametric_coefficient.rs`](../../src/parametric_coefficient.rs): affine polynomial composition | Migrated | `MultivariatePolynomial::evaluate_with_coeff_map` on safe mixed-radix inputs; simultaneous `AtomCore::replace_multiple`, direct `expand`, and polynomial conversion otherwise | Production and tests use Symbolica algebra exclusively. The former RustRed weak-composition, Cartesian enumeration, radix sorting, collection, workspace types, and reference entry points were deleted rather than retained as an oracle. |
| P1 (complete) | [`ParametricCoefficientContext::polynomial_loci_are_associates_with_census`](../../src/parametric_coefficient.rs): strict polynomial-associate proof over `K = Q(theta)` | Migrated | Public `map_exp(u16 -> u32)`, `RationalPolynomial::from`, `to_polynomial(index_variables, true)`, `RationalPolynomialField::mul`, and authenticated exact equality | Symbolica owns projection, arbitrary-precision arithmetic, collection, and comparison. RustRed retains only strict associate semantics, deterministic support/anchor routing, resource admission, authentication, panic containment, provenance, and transactional census propagation. The former private magnitude/limb engine and its counters were deleted. |
| P2 | [`ParametricCoefficientContext::permute_polynomial_raw` and `execute_specialize_polynomial_raw`](../../src/parametric_coefficient.rs): manual permutation and full specialization/collection | Must replace now | Simultaneous `evaluate_with_coeff_map` into `PolynomialRing<Z,u16>` | Keep context-map validation and prospective/observed envelopes. Translation and partial specialization already use native `replace_with_poly`/`replace`. |
| P2 | [`symbolica_tensor_numerator.rs:1044-1180`](../../src/symbolica_tensor_numerator.rs#L1044): tensor-head-aware distributive expansion of `Atom` addition, multiplication and powers | Native-first API-gap migration | Use `AtomCore::expand` or `expand_via_poly` on the smallest authenticated tensor-containing subtree, or a public Symbolica transformer that preserves selective expansion | RustRed deliberately leaves scalar-only powers as opaque weights and enforces limits before allocation, so whole-expression `expand` is not a drop-in. Retain tensor grammar/preflight/decoding. If public composition cannot preserve those semantics, keep only the selective syntax wrapper, document the exact gap locally, and differentially test every admitted input against native expansion. |
| P1 (next/high) | [`vakint_adapter.rs:652-713`](../../src/vakint_adapter.rs#L652): private `controlled_distribute` Cartesian Atom distribution | Pending native-first migration; not part of the affine-composition milestone | Prefer public `AtomCore::expand_in` for authenticated tensor/topology symbols; alternatively mask Pow/Fun leaves with collision-rejected simultaneous replacements around `AtomCore::expand` | A naked whole-expression `expand` is not semantics-preserving: it expands additive power bases and normalizes/collects terms that the current decoder treats as opaque or distinct. Preserve the admitted Vakint numerator grammar, typed preflight, source provenance, and spectator opacity; differential tests must authenticate the chosen native route before deleting the private distributor. |
| P2 | [`symbolica_affine_denominator.rs:3585-3645`](../../src/symbolica_affine_denominator.rs#L3585): private coefficient power loop | Must replace now | `RationalPolynomialField::pow` through `Ring::pow` | The surrounding `AtomView` traversal and scalar-product contraction are justified semantic lowering and already use native coefficient arithmetic. The resource-envelope helper also named `checked_power` at line 4603 is bookkeeping, not algebra. |
| P1 | [`symbolica_affine_denominator.rs:1511-2153`](../../src/symbolica_affine_denominator.rs#L1511): manual numerator grouping, projection, and lifting into momentum variables | Must replace algebra now | `RationalPolynomial::to_polynomial(momentum_variables, false)` | Keep momentum-degree classification and scalar-product semantics; delete manual exponent-row copying. |
| P1 | [`tensor_family.rs:181-286`](../../src/tensor_family.rs#L181) and [`generic_tensor_family.rs:677-913`](../../src/generic_tensor_family.rs#L677): private denominator-shift polynomial multiplication, power, and collection | Must replace algebra now | `MultivariatePolynomial<RationalPolynomialField<Z,u16>,u32>`, native multiplication, and `Ring::pow` | Converting authenticated final monomials into integral shifts remains RustRed semantic lowering. An exponent wider than the public domain requires an explicit audited policy, not a second polynomial engine. |
| P3 | [`residual_affine_integer_lattice_kernel.rs:1382-1472`](../../src/residual_affine_integer_lattice_kernel.rs#L1382) and [`residual_affine_integer_system.rs:3038-3099`](../../src/residual_affine_integer_system.rs#L3038): private gcd and extended gcd | Must replace primitives | `Integer::gcd` and `Integer::extended_gcd` | Preserve positive-gcd and deterministic Bezout conventions by normalizing and verifying the native result; retain transcript and budget accounting. |
| P3 | [`four_loop_next_modular_rank.rs:738-838`](../../src/four_loop_next_modular_rank.rs#L738) and [`four_loop_next_modular_rank.rs:1068-1130`](../../src/four_loop_next_modular_rank.rs#L1068): private modular arithmetic, inversion, powering and primality | Must replace if the legacy feature is maintained | `Zp64`/`Zp`, finite-field `Ring` operations, and `Integer::is_prime` | This module is feature-gated by `legacy-authored-oracles`; it must remain evidence-only and must not become a generic production path. Its restricted Markowitz pivot controller may remain. |

### B0 implementation and impact

`ExactRational` is public, and the private matrix routines were consumed
throughout [`family.rs`](../../src/family.rs), including basis rank/inversion
and symmetry transformations. The coherent migration used one exact constant
field, not a second parallel rational type:

1. select `Rational`/`Q` as the only exact constant field;
2. adapt constructors and public compatibility methods without narrowing back
   to `i64`;
3. preflight rectangular shape, `usize` allocation, and Symbolica's `u32`
   dimensions, then convert row-major values with `Matrix::from_linear`;
4. call the public matrix operations behind checked shape and panic boundaries;
5. convert results back only where a certificate structure requires owned
   row-major values; and
6. delete the private gcd and matrix algorithms.

Two vendored Symbolica 2.2.0 boundary defects were found while implementing
the adapter and are now frozen by RustRed tests:

- `Matrix::from_nested_vec` divides by the first row length and is therefore
  unsuitable for empty or zero-column input. RustRed uses the checked
  row-major `from_linear` constructor after its own shape preflight.
- the generic `Matrix::inv` path can accept a singular size-one or size-four
  and larger matrix because augmented identity columns may supply pivots.
  RustRed calls the independent native `Matrix::det` first and rejects a zero
  determinant before invoking `inv`.

Arbitrary-precision numerator and denominator values cross the coefficient
bridge as cloned Symbolica `Integer` values, never through `i64` or text.
Because `Rational` is not `Copy`, downstream storage and access sites now use
borrows or explicit clones, and the former fixed-width `ZERO`/`ONE` constants
become `zero()`/`one()` constructors. Numerator and denominator access returns
`&Integer`, not a narrowing primitive. The compatibility constructor retains
its old panic on a zero denominator; new input-dependent code has checked
`try_new`, `try_reciprocal`, and `try_div` boundaries.

No `no_gmp` compatibility path should be added.

### B0 validation evidence

All licensed test commands received `SYMBOLICA_LICENSE` only in their process
environment. The key is not stored in the repository or documentation. Tests
were split across concurrent processes and each nextest process also used
parallel workers.

- `cargo check --all-features --all-targets -j4`: passed.
- nextest `ed5dbab1-7073-4d30-ad86-074ab1c2be8d`: 32/32 exact and family
  unit tests passed, including arbitrary precision, singular/nonsingular sizes
  one through six, an exhaustive 625-matrix two-by-two differential census,
  shape boundaries, and generic-family consumers.
- nextest `f78c2398-dba4-4d4f-8877-29c9b2dc494c`: 23/23 public integration
  tests passed across the GMP coefficient bridge, Feynman polynomials, tensor
  family lowering, and zero-sector certificates.
- concurrent legacy-oracle nextest runs
  `160e5673-6697-46fa-93af-2d577b70357b` and
  `f71e7505-d97b-4503-89eb-ee29cbb01cb0`: 5/5 and 8/8 passed across migrated
  two-, three-, four-, and five-loop consumers, including exhaustive
  four-loop boundary and genuine-corner catalogs.
- the independent release audit then passed 103/103 default/core consumers in
  nextest `d2614ea6-688c-4d3d-9635-7a9ef057a849`, 10/10 legacy two-/three-loop
  consumers in `1700ebf2-fdaa-45ec-a1b2-dfb27e4324d0`, and 12/12 legacy
  four-/five-loop consumers in `9e700546-912f-4edc-bff0-cfaea4d5eaf9`.
- nextest `5de5e721-af87-4af4-8048-cc6677b0f800`: 5/5 retained-payload and
  admission tests passed. Every rational-bearing witness location and every
  owned vector buffer class has a delta census; exact-limit, one-below, and
  cumulative-overflow behavior is frozen. The authenticated full inventory
  now records `peak_charged_bytes = 1_070_904` under the documented logical
  payload metric.

### Generic-family coefficient-matrix migration and validation evidence

[`symbolica_coefficient_matrix.rs`](../../src/symbolica_coefficient_matrix.rs)
is the narrow adapter for this completed P1 slice. `CheckedCoefficientField`
implements Symbolica's public `Set`, `RingOps`, `Ring`, `EuclideanDomain`, and
`Field` traits for RustRed's existing `Coefficient` element. The matrix-used
add, subtract, multiply, divide, negate, zero, one, and zero/one-test operations
delegate to checked `CoefficientContext` operations; the adapter does not
implement a second rational-function or matrix algebra. Symbolica owns
`Matrix::det`, `Matrix::inv`, and the native products `A A^-1` and `A^-1 A`.
The generic family converts only its authenticated denominator-basis rows into
that boundary and retains the returned row orientation and determinant-domain
condition.

No better public context-carrying field API exists in the vendored Symbolica
2.2.0 revision. `Matrix<F>` receives one `F: Ring`/`Field`, but the public
`RationalPolynomialField` stores only its coefficient ring, not RustRed's
ordered variable map, and its `zero`, `one`, and `nth` construct elements with
an empty map
([`rational_polynomial.rs:38-58`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L38),
[`rational_polynomial.rs:847-872`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L847)).
Its ordinary element arithmetic can also unify differing maps. Symbolica's
`RingOps` and `Field` arithmetic methods have infallible return types, while
only `Ring::{try_inv,try_div}` return `Option`, so the native matrix calls have
no public channel for RustRed's typed context or resource errors
([`domains.rs:111-182`](../../vendor/symbolica/lib/numerica/src/domains.rs#L111),
[`domains.rs:250-254`](../../vendor/symbolica/lib/numerica/src/domains.rs#L250)).
The factorized rational-polynomial field changes the element representation
and retains audited gaps including a `todo!()` ordering and an `is_one` that
does not test `numer_coeff`; `AtomField` would discard the strict polynomial
map and coefficient-resource contract. A private contextual trait adapter is
therefore the smallest public-API composition that lets Symbolica remain the
algebra engine.

The adapter closes two audited dense-matrix boundary defects without replacing
the native algorithms. First, the generic `Matrix::inv` implementation can let
the identity half of `[A|I]` provide pivots for a singular `A`. RustRed calls
the independent native determinant first and rejects a zero determinant before
calling the native inverse
([`symbolica_coefficient_matrix.rs:1387-1454`](../../src/symbolica_coefficient_matrix.rs#L1387)).
Second, `Matrix`'s public `SelfRing::is_one` accepts a zero diagonal entry
([`matrix.rs:1128-1134`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs#L1128)).
RustRed therefore authenticates both native products and explicitly requires
one on every diagonal entry and zero everywhere else
([`symbolica_coefficient_matrix.rs:1313-1347`](../../src/symbolica_coefficient_matrix.rs#L1313)).

Admission is explicit at four levels: exact scalar-operation count, individual
and simultaneously live matrix-entry counts, clone-owned retained bytes of all
authenticated inputs, and aggregate clone-owned retained bytes of the
determinant, inverse, and both verification products. Every produced
coefficient is reauthenticated against the ordered context and its exact
algebra limits. This is not a complete native-memory proof: Symbolica's public
API exposes no bound for all temporary polynomial GCD, quotient, or dense
multiplication scratch, so that scratch limitation is documented at the module
boundary rather than hidden behind the retained-byte census.

Because Symbolica's required `RingOps`/`Field` methods cannot return the typed
checked-algebra errors, the adapter transports its private error payload only
across the immediately enclosing native call with `resume_unwind` and
`catch_unwind`. The boundary consequently requires `panic = "unwind"` and has a
compile-time rejection for `panic = "abort"`. The direct `rand = "0.9"`
dependency exists only because an implementation of Symbolica's public
`Ring::sample` must name the matching `rand::RngCore` type; sampling delegates
to Symbolica's integer ring and is not a RustRed matrix or CAS algorithm.

The black-box parametric-IBP oracle was made independent of the migrated
production inverse. It reconstructs each inverse column with public
`Matrix::solve(A, e_j)`, without reading `IntegralFamily::inverse_basis`, and
uses native products for two-sided replay. Concrete analytic inverses for the
one-, two-, three-, and five-loop fixtures are test-only, independently checked
oracle data; a deliberately perturbed fixture is rejected. They do not enter
production derivation. Production `IntegralFamily` construction and
parametric-IBP generation remain topology- and loop-count-generic.

All milestone lanes were licensed GMP builds and ran in parallel. The key was
provided only through the process environment and is not stored here.

- focused checked-adapter nextest: 17/17 passed in
  `111ef62d-3de6-4957-b6b4-b2e04820375f`;
- combined debug nextest: 42/42 passed in
  `3197a8d5-70a9-49de-9d78-5415374f46bc`;
- downstream nextest: 71/71 passed in
  `1820a2af-baa8-4f82-a103-b70b81c52b4d`;
- combined release nextest: 42/42 passed in
  `dfe1042d-2509-4bad-87b6-87df1281cf6c`;
- final hardening reruns passed 43/43 debug tests in
  `d1519c8d-05c4-44e3-aefc-88ea68be936f`, 35/35 selected release library tests in
  `053604e4-0ec6-4dad-9c4f-90aee31af8c2`, and 8/8 release independent-oracle
  tests in `99fff53c-0fbf-46a9-9fc7-b63c8bd9795b`;
- the complete optimized default-feature library suite passed 969/969 tests
  under `cargo nextest run --release --lib -j4 --no-fail-fast`, with four
  workers and no failures; and
- `cargo check --all-features --all-targets -j4` passed.

### Automatic-ISP rank migration and validation evidence

The former Gaussian rank implementation was deleted.  Each nonempty
rectangular denominator-coefficient matrix now crosses the same checked,
map-aware field boundary and is destructively reduced by public Symbolica
`Matrix::partial_row_reduce`.  RustRed still owns LiteRed's deterministic
identity-row scan, coordinate order, resource admission, and replay metadata;
it owns no pivot or row arithmetic.  Because the authenticated operation
census now describes Symbolica's arithmetic schedule, new certificates use
`rustred-automatic-isp-completion-v2`; V1 remains only a legacy identifier.

A separate black-box oracle defines rank by maximal nonzero minors and calls
public `Matrix::det` for every determinant.  Final parallel optimized nextest
run `88208064-7cd3-46b4-b4f5-807953c2232f` passed 30/30 adapter and internal
completion tests.  Run `0a0f4f11-09b0-4d0e-a9b8-f9adad877989` passed 13/13
public/oracle/downstream tests, including complete four- and five-loop
factorized reductions.  An all-feature/all-target compile check also passed.

### Tensor-projector migration and validation evidence

Both the legacy vacuum projector and the authenticated generic projector now
construct their exact Gram matrices structurally and delegate coefficient
powers and all matrix algebra to the checked Symbolica boundary. Public
`RationalPolynomialField::pow` owns contraction powers. Public
`Matrix::{det,inv}` owns the Gram determinant and inverse, and public matrix
multiplication replays both `G G^-1` and `G^-1 G` entry by entry. RustRed still
owns perfect-matching enumeration, contraction-cycle connectivity, admission,
Lorentz-covariant bookkeeping, domain guards, and certificate replay; it owns
no projector elimination or coefficient-power algorithm.

The independent determinant check is semantically required with the vendored
revision because bare `Matrix::inv` may miss singularity when the augmented
identity supplies pivots. V2 projector certificates therefore retain the
basis-independent Gram-determinant numerator and denominator provenance rather
than V1's elimination-pivot transcript. V1 names and guard variants remain
exported for source compatibility, but RustRed currently has no persisted V1
decoder or schema-aware V1 replay path.

The black-box projector oracle independently enumerates contraction cycles,
constructs rank-zero, rank-two, rank-four, and rank-six Gram systems, solves
each of the 15 rank-six inverse columns separately with public
`Matrix::solve`, and checks every production inverse entry. It also checks the
frozen Vakint rank-four/rank-six coefficient classes, identical-vector
contractions, and the legacy-versus-generic orientation. Concrete dimensions
prove that determinant guards accept a regular `d=-1` rank-four point and
reject the singular `d=0`, `d=1`, and `d=-2` cases at the applicable ranks.

One-loop end-to-end closure tests independently rebuild generated parametric
IBPs for integrands written with an explicit denominator factor versus the
same factor already cancelled. They cover free tensor ranks two, four, and
six; the rank-six result contains all 15 metric pairings with coefficient
`m2/(8*d)` multiplying the selected `I(1)` master. A traced rank-six spelling
is also compared with the corresponding scalar-product-times-rank-four input
at the public Symbolica covariant-rendering boundary, so private dummy-index
allocation identities cannot create a false mismatch. No FORM process or
topology-authored recurrence participates in these tests.

Final licensed GMP validation used parallel workers throughout: the complete
checked coefficient/matrix adapter module passed 27/27 tests; the independent
rank-six oracle passed in debug run `a527dbcc-a3cf-46db-8a00-9c067e5956b0`
and release run `6c67dc52-c12d-4132-aa51-f907d3b00457`; and optimized run
`fb9a0cda-1b7a-4494-8032-d7cbc8ea1422` passed 28/28 tests across seven tensor,
closure, and Vakint-oracle binaries with four workers. A final
`cargo check --all-features --all-targets -j4` also passed.

### Affine-family symmetry migration and validation evidence

The generic verifier now sends every nonempty loop/external determinant,
external-Gram congruence, denominator-basis product, and affine matrix-vector
product through the checked public Symbolica matrix boundary.  The former
subset-state determinant implementation has been deleted.  A vacuum external
map is the one structural exception: the vendored `Matrix::det` reports a
`0x0` matrix as singular, so RustRed returns the mathematical empty
determinant one before entering Symbolica.

The V2 certificate retains both the conservative admitted native-operation
envelope and the actual checked-field operation count.  Its enforceable
aggregate exact-operation cap is the admitted envelope, which can be replayed
exactly even when Bareiss exits early; actual native work remains separately
observable.  Single-matrix entries, simultaneously live entries, authenticated
input/output bytes, determinant/product/transpose call counts, and exact
one-below boundaries are all reported.  The V1 subset-state field remains in
the public limits structure only for source compatibility and is ignored by
V2.

RustRed still constructs upper-triangular scalar-product coordinates and
affine constants because those are family semantics.  It also independently
re-expands every source denominator directly from the momentum witness rather
than trusting the retained scalar-product map.  The external oracle goes in
the other direction: it uses bare public Symbolica matrices to form the full
momentum congruence, scalar-coordinate response matrices,
`R_s T R_t^-1`, and affine shifts, then compares every production entry.  It
covers rational four-by-four maps, singular rejection, the vacuum `0x0`
boundary, non-vacuum external shifts, and simultaneous denominator-basis
shears.

Final licensed GMP validation used four nextest workers:

- checked coefficient-matrix run
  `5923f3c1-4ad3-43f0-844a-fb463f548ab7`: 29/29 passed;
- debug symmetry/discovery/provider/transport/oracle run
  `c5b6a94c-5233-4231-96c3-df75a205da9c`: 36/36 passed;
- optimized repeat
  `f6cb622a-1c8d-4a77-83e4-2ac2dae5dc91`: 36/36 passed; and
- `cargo check --all-features --all-targets -j4` passed.

The license was supplied only to each process environment.  No FORM process,
`no_gmp` feature, topology-specific verifier, or authored recurrence took part.

### Polynomial-associate migration and validation evidence

The strict `K = Q(theta)` associate proof now widens authenticated exponents
to `u32`, projects the index variables with public Symbolica APIs, and asks
`RationalPolynomialField::mul` for every projective cross product. Both
projections and products are authenticated before exact equality is trusted.
Zero inputs return `false`, and numeric zero validation also covers
noncanonical `Integer::Double(0)` and `Integer::Large(0)` representations.
The old magnitude extraction, limb multiplication/accumulation, and scratch
workspace implementation was deleted. Its resource fields were replaced by
projection, native multiplication/output/bit-work, native dense-or-heap
workspace, and RustRed-visible temporary envelopes.

All licensed commands received `SYMBOLICA_LICENSE` only in their process
environment; the key is not stored in this repository or documentation. The
final frozen tree passed:

- `cargo check --lib` and `cargo test --lib --no-run` with default
  GMP-backed Symbolica;
- `cargo check --all-features --all-targets -j4`;
- nextest `3f7f5d42-f882-47e0-83c0-22b2218ba5a7`: 58/58 focused strict
  associate, authentication, native-dispatch, and exact/one-below resource
  tests;
- nextest `4b2dcb72-2c7b-4892-ab80-b72f2a181354`: 11/11 generated-condition
  and aggregate-census consumers;
- release nextest `d4f5db37-0cda-408b-b78f-ad8129a1043a`: 69/69 associate
  and numeric-zero tests; and
- a complete optimized `cargo nextest run --release --lib -j4
  --no-fail-fast`: 945/945 library tests passed, with no skipped or failed
  tests. The suite ran independent test processes concurrently, and nextest
  used four workers throughout.

## RustRed-owned semantic wrappers that remain justified

These components are not generic replacements for Symbolica algebra. They
encode semantics that the public matrix API does not expose.

| RustRed owner | Why it remains RustRed-owned | Native differential oracle |
|---|---|---|
| [`exact_sparse_elimination.rs:315-595`](../../src/exact_sparse_elimination.rs#L315), [`exact_sparse_elimination.rs:1399-1637`](../../src/exact_sparse_elimination.rs#L1399) | The caller authenticates a hardest-first integral-column/source-row skeleton and retains full provenance/replay. That controller may remain, but its private row algebra is not yet accepted permanently: column reindexing plus `SparseRowReducer` with `LuLMode::Full` may preserve the required order and elimination factors. | First build a transcript-equivalence spike with native `add_row`, `u`, `l`, `pivots`, and `back_substitute`; retain private row arithmetic only if that composition leaves a precisely documented semantic gap. |
| [`certified_rewrite.rs:1856-2000`](../../src/certified_rewrite.rs#L1856) | Scout reduction discovers the exact integral-order skeleton later replayed by `ExactSparseElimination`; it is pivot planning and certificate construction, not a new coefficient field. | Compare the selected system's rank and source-row dependencies with a public dense/sparse reducer. |
| [`parametric_elimination.rs:704-1182`](../../src/parametric_elimination.rs#L704), [`parametric_elimination.rs:1765-1924`](../../src/parametric_elimination.rs#L1765), and [`persistent_parametric_elimination.rs`](../../src/persistent_parametric_elimination.rs) | LiteRed ordering, index-shift columns, conditional pivot numerators/denominators, `WhenBad`, chronological traces and clean-prefix persistence are absent from public field elimination. Public elimination would silently treat every formal nonzero rational function as invertible. | At generic concrete specializations, compare rank/row span and normalized solutions with `Matrix<RPF>` or a finite-field image. Verify every retained guard separately. |
| [`residual_affine_integer_lattice_kernel.rs:970-1213`](../../src/residual_affine_integer_lattice_kernel.rs#L970) and [`residual_affine_integer_lattice_kernel.rs:1545-1660`](../../src/residual_affine_integer_lattice_kernel.rs#L1545) | Produces the complete integral affine solution lattice and a unimodular transform transcript. Public `Matrix<Z>::solve_fraction_free` returns one determined solution, not a Smith/Hermite-style lattice parameterization. | Check the rational affine span with `Matrix<Q>` and bounded integer-point enumeration; use native gcd inside the wrapper. |
| [`residual_affine_integer_system.rs:2273-2865`](../../src/residual_affine_integer_system.rs#L2273) and [`residual_affine_integer_system.rs:3293-3379`](../../src/residual_affine_integer_system.rs#L3293) | Implements LiteRed's original-coordinate unit-pivot cylinder search, unsupported-congruence boundary, affine projection and row-operation replay. | Compare satisfiability/rational rank with `Matrix<Q>` and exhaust small bounded integer boxes. Use native gcd/extended gcd. |
| [`zero_sectors.rs:774-776`](../../src/zero_sectors.rs#L774) and [`zero_sectors.rs:1047-1154`](../../src/zero_sectors.rs#L1047) | This is already the desired composition: native `Matrix<Q>::row_reduce`, followed by deterministic kernel-vector choice and primitive-integer certificate formatting. | Existing replay plus independent matrix-kernel checks. |
| [`symmetry.rs`](../../src/symmetry.rs) | Upper-triangular scalar-product coordinates, off-diagonal folding, affine denominator semantics and an independently derived replay are domain-specific tensor-map logic. Scalar coefficient operations and all ordinary matrix kernels are native. | The V2 verifier uses native determinant/transpose/products, then directly substitutes and replays every denominator without consuming the retained scalar-product map. |
| [`symbolica_affine_denominator.rs:1511-1765`](../../src/symbolica_affine_denominator.rs#L1511) | Recognizing and contracting the declared scalar-product function into loop/external coordinates is expression-language and family semantics. Only that grammar and contraction remain justified; manual polynomial projection/lifting must migrate to `RationalPolynomial::to_polynomial`. | Recompile the retained `Atom`, compare native momentum-polynomial decomposition, explicit bilinear expansions, and differently parenthesized inputs. |
| [`tensor_family.rs:183-246`](../../src/tensor_family.rs#L183) and [`generic_tensor_family.rs:677-750`](../../src/generic_tensor_family.rs#L677) | Mapping final monomials to denominator-power shifts and retaining per-input origin/resource semantics are RustRed responsibilities. Polynomial multiplication, exponentiation, and collection are not; they must migrate to Symbolica even though the current key representation admits `u64`. | Differentially compare the native polynomial result with every old coefficient/key. Define a checked policy for exponents outside Symbolica's public exponent domain instead of retaining a private algebra engine. |
| [`residual_unit_affine_index_map.rs:686-825`](../../src/residual_unit_affine_index_map.rs#L686) and [`coordinate_equality_loci.rs:798-888`](../../src/coordinate_equality_loci.rs#L798) | These routines recognize and certify restricted affine forms/associates used for branching; they are not general polynomial solvers. Exact division already delegates to `Z.quot_rem`. | Compare accepted forms with native polynomial/rational-polynomial equality, and reject perturbed coefficients, supports and constants. |

Any retained custom solver must keep a code-local gap comment naming the
public APIs considered and the missing semantic. A resource limit, preferred
pivot for speed, or exact memory census is not by itself such a gap.

## Non-algebra bookkeeping that may remain

The following patterns do not compete with Symbolica's algebra and should not
be removed merely because they contain loops or `BTreeMap`s:

- [`symmetry.rs:31-118`](../../src/symmetry.rs#L31) stores checked rectangular
  certificate matrices. It should convert to `Matrix` at an algebra boundary.
- [`linear.rs`](../../src/linear.rs),
  [`parametric_relation.rs`](../../src/parametric_relation.rs), and
  [`generic_tensor_polynomial.rs`](../../src/generic_tensor_polynomial.rs)
  collect coefficients under integral-shift or tensor-structure keys. These
  keys are not polynomial variables in the coefficient ring.
- [`symbolica_affine_denominator.rs:3790-3886`](../../src/symbolica_affine_denominator.rs#L3790)
  copies authenticated scalar-product terms into the compiler's declared
  coordinate map. This remains expression-language bookkeeping; ordinary
  polynomial parameter projection belongs to the migration table above.
- affine-map construction, exact recentering, stable sorting, hashing,
  retained-byte census and guard-origin storage are certificate/representation
  work. For example,
  [`generated_affine_residual_group_exact_recenter_kernel.rs`](../../src/generated_affine_residual_group_exact_recenter_kernel.rs)
  applies a specific affine geometry and delegates coefficient translations
  to the parametric context.
- the bounded-count helper
  [`residual_affine_u128_gcd`](../../src/parametric_coefficient.rs)
  is used only to compute a capped binomial resource bound. It is ordinary
  bounded-count bookkeeping, not an exact algebra domain.
- [`exact_identity.rs`](../../src/exact_identity.rs) inspects limbs solely for
  deterministic serialization; it does not implement integer arithmetic.

## Required equivalence tests

Every migration row above requires differential tests before the private
implementation is deleted. During transition, the old path may be compiled
only as a test oracle. Tests must run in parallel, with the Symbolica license
set, using `cargo nextest run --workspace --all-targets` when available or
`cargo test --workspace --all-targets -- --test-threads <N>`. FORM must never
be invoked. The `legacy-authored-oracles` feature may be tested separately but
must not be needed by the default generic path.

| Test family | Required assertions |
|---|---|
| Exact rational and dense matrix migration | Compare old and native results on nonsingular/singular rectangular and square matrices of dimensions 0 through 6; check determinant, rank, inverse, transpose and multiplication. Add coefficients beyond `i64` to prove GMP exactness. Verify family basis inverse and fingerprint/domain conditions are unchanged. |
| Generic family and automatic ISP completion | Use symbolic masses, invariants and `d`; compare basis determinant, inverse identity, ISP candidate order and complete rank progression. Specialize at several exact rational points and compare native ranks again. |
| Tensor projectors | For ranks 0, 2, 4 and at least one higher feasible even rank, verify `Gram * inverse = identity`, the complete guard set, and projector contractions. Reduce one-loop tensor numerators and compare the scalar coefficients with stored Vakint oracle data without running FORM. |
| Feynman polynomials | Differentially compare `U`, `F`, `G`, all gradients and every tested sector face for one- and two-loop generic families. Verify homogeneity, adjugate identity `A adj(A) = det(A) I`, and native matrix contraction. |
| Base specialization and polynomial substitution | Compare native evaluation with the old path on zero, constants, sparse multivariate values, rational images and denominator-zero points. Test parameter projection after an authenticated zero-exponent proof, rejection of genuine dropped-parameter dependence, base-map extension, simultaneous permutations with nontrivial cycles and full specializations into a smaller base map. |
| Affine composition | For V1 and V2 plans, compare the selected Symbolica compositor with independent exact fixtures on constants, translations, mixed affine images and cancellation-heavy inputs. Include differently written equal inputs, the polynomial-evaluator/Atom-fallback stride boundary, GMP cancellation, exact and one-below resource gates, and typed panic boundaries. |
| Polynomial associates | Test strict equality up to a nonzero `Q(theta)` factor, different index support, zero input, cancellation, large GMP coefficients, multiple base/index variables, noncanonical integer-zero variants, and the `u16::MAX` cross-product boundary after `u32` widening. Require `p` versus `p^2` to be false, authenticate every native projection/product, exercise exact and one-below resource gates and the typed panic boundary, and compare with an independent public-Symbolica quotient oracle in tests. |
| Tensor `Atom` expansion | Compile factored, expanded, reordered and differently parenthesized but equal tensor expressions to the same structured numerator. Include opaque scalar weights, reserved heads, powers, malformed inputs and exact/one-below expansion limits. |
| Symmetry | Compare native determinants and standard matrix products with the previous result; replay external Gram preservation, scalar-product maps and every source denominator independently. Test singular maps and parameter-specialization guard failures. |
| Integer affine solvers | Differentially compare native and old gcd/Bezout outputs after convention normalization. Replay every row operation, verify all returned lattice basis vectors, and exhaust small integer boxes for completeness. Compare rational rank and satisfiability with `Matrix<Q>`. |
| Sparse and parametric semantic wrappers | Map integral keys to columns, compare row span/rank against public dense and sparse reducers at exact rational and finite-field images, then verify the RustRed-selected pivot order, guards and chronological trace separately. Tampering with any source row, pivot, guard or event identity must fail replay. |
| Algebraic input closure | Reduce identical integrands written differently: in particular a numerator factor such as `q_i^2-m^2` against the input where the matching denominator is cancelled explicitly. Also vary scalar-product order, factored/expanded numerator form and simultaneous index renaming. The same masters and coefficients must result. |

Concrete topology fixtures are validation inputs only. No equivalence test may
introduce loop-count or topology dispatch into production rule derivation.

## Migration sequence and acceptance gates

1. Finish and validate the chronological event-ledger milestone. **Complete.**
2. Replace `ExactRational` and all `exact.rs` matrix consumers with
   `Q`/`Matrix<Q>`. **B0 complete.**
3. Move generated-affine production composition to the selected dual-Symbolica
   polynomial-evaluator/Atom-expansion backends. **Composition complete.**
   Move the strict associate proof through public exponent widening,
   `RationalPolynomial::to_polynomial`, and native coefficient-field
   multiplication/equality. **Associate migration complete.**
4. Migrate direct matrix consumers. **The generic-family coefficient-matrix,
   automatic-ISP rank, tensor-projector, and affine-family symmetry-verifier
   slices are complete.** Integer symmetry-discovery and Feynman determinants
   remain pending.
5. Build the `SparseRowReducer` transcript-equivalence spike; move row algebra
   native wherever column reindexing and full `L` preserve RustRed's ordering,
   provenance, guard, and replay semantics.
6. Migrate the remaining Feynman, tensor-family, specialization, and
   affine-denominator polynomial operations; then native tensor-expression
   expansion and integer gcd primitives.
7. Run the complete parallel default suite, the focused differential suites,
   and the feature-gated legacy-oracle suite separately.
8. Re-run generic closure validation from one loop upward. Use concrete
   one-loop tensor examples first, then two-loop and higher vacuum examples;
   never substitute a topology-authored recurrence for derived parametric IBPs.

A migration is complete only when:

- production algebra crosses a public Symbolica API;
- no `no_gmp` or FORM path is added;
- the RustRed semantic wrapper still authenticates context, resource,
  provenance, guards and chronological replay;
- native-vs-old differential tests pass before the old implementation is
  removed or made test-only;
- differently written but algebraically identical inputs reduce identically;
- tests execute in parallel; and
- no default-production branch depends on `legacy-authored-oracles`.
