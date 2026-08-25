# Symbolica-first algebra migration audit

Status: production-code audit and B0 implementation record, 2026-08-24. This
document records the migration required by RustRed's public-Symbolica-first
policy and the completed first implementation slice. Later priorities remain
an audit plan rather than a claim that all production algebra has migrated.

The companion API inventory is
[`symbolica_exact_linear_algebra_api_inventory.md`](symbolica_exact_linear_algebra_api_inventory.md).

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

RustRed may continue to own LiteRed semantics around Symbolica operations:
integral ordering, pivot-condition guards, `WhenBad` branches, source-row
provenance, chronological replay, resource admission, panic containment, and
transactional commit. Those wrappers are not permission to reimplement the
underlying arithmetic.

The current dependency configuration is correct:
[`Cargo.toml:35`](../../Cargo.toml#L35) enables Symbolica's `gmp` feature and
does not enable `no_gmp`. Every migration and validation build must preserve
that configuration.

## Public APIs that form the replacement boundary

The exact replacement surface in the vendored revision is:

- the `Q` domain and `Rational` element for GMP-backed exact rationals at
  [`rational.rs:24`](../../vendor/symbolica/lib/numerica/src/domains/rational.rs#L24)
  and [`rational.rs:698`](../../vendor/symbolica/lib/numerica/src/domains/rational.rs#L698);
- `RationalPolynomialField::{new, from_poly}` at
  [`rational_polynomial.rs:45`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L45);
- `RationalPolynomial::to_polynomial` at
  [`rational_polynomial.rs:572`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L572),
  which splits selected variables into native polynomial indeterminates with
  rational-polynomial coefficients;
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
| P1 | [`generic_family.rs:1861-1994`](../../src/generic_family.rs#L1861): symbolic inverse, determinant and inverse verification | Must replace now | `Matrix<RationalPolynomialField<Z,u16>>::inv`, `det`, and matrix multiplication | Preserve the family-domain determinant condition and validation wrappers; replace only the algebra core. |
| P1 | [`automatic_isps.rs:684-746`](../../src/automatic_isps.rs#L684): hand-written Gaussian rank | Must replace now | `Matrix<RationalPolynomialField<Z,u16>>::rank` or `row_reduce` | Retain deterministic candidate order and rank-progression certificate. No custom pivot transcript is currently retained, so there is no semantic gap. |
| P1 | [`tensor.rs:1364-1486`](../../src/tensor.rs#L1364) and [`generic_tensor_projector.rs:2501-2763`](../../src/generic_tensor_projector.rs#L2501): Gram inversion and coefficient exponentiation | Must replace now | `Matrix<RationalPolynomialField<Z,u16>>::inv`; `Ring::pow` | Retain pairing enumeration, Gram construction, determinant/inverse-denominator guards and resource wrappers. Per-Gaussian-pivot provenance alone does not justify a private inverse. |
| P1 | [`symmetry.rs:1384-1440`](../../src/symmetry.rs#L1384): subset-DP determinant | Must replace now | `Matrix<RationalPolynomialField<Z,u16>>::det` | Keep determinant nonzero guards and panic/resource boundaries. |
| P1 | [`symmetry_discovery.rs:1209-1275`](../../src/symmetry_discovery.rs#L1209): private integer Bareiss determinant | Must replace now | `Matrix<Z>::det` | Keep candidate enumeration and conservative integer-bit/work admission. Symbolica's matrix determinant is already fraction-free/Bareiss. |
| P1 | [`symmetry.rs:907-938`](../../src/symmetry.rs#L907) and [`symmetry.rs:1087-1127`](../../src/symmetry.rs#L1087): recognizable `C G C^T`, `R T`, and product-with-inverse kernels | Must move native matrix kernels | Symbolica matrix multiplication and transpose | Keep the independently derived scalar-product map and denominator replay as semantic verification; use native matrices for ordinary products. |
| P1 | [`feynman_polynomials.rs:969-1080`](../../src/feynman_polynomials.rs#L969): subset-DP determinants for `U` and every adjugate minor | Must replace now | `Matrix<PolynomialRing<RationalPolynomialField<Z,u16>,u16>>::det` | Minor selection/adjugate placement remains structural bookkeeping. The determinant of each selected matrix is native. |
| P2 | [`feynman_polynomials.rs:294-380`](../../src/feynman_polynomials.rs#L294) and [`feynman_polynomials.rs:491-717`](../../src/feynman_polynomials.rs#L491): derivative, face restriction, add/subtract/multiply/scale and collection over a native polynomial type | Must replace now | `derivative`, `replace(..., 0)`, native `+`, `-`, `*`, coefficient-ring operations | Keep prospective exponent/term limits, context authentication and panic containment. |
| P2 | [`feynman_polynomials.rs:887-924`](../../src/feynman_polynomials.rs#L887): standard adjugate/Gram quadratic contraction | Must move native matrix kernels | Matrix multiplication/transpose over `PolynomialRing<RPF,u16>` | Preserve the Feynman-polynomial construction formula and homogeneity checks. |
| P2 | [`base_specialization.rs:677-768`](../../src/base_specialization.rs#L677): manual polynomial evaluation and binary exponentiation | Must replace now | `evaluate_with_coeff_map` over `RationalPolynomialField`; `Ring::pow` | Retain family-domain guard classification and operation/resource admission. |
| P2 | [`coefficient.rs:1260-1331`](../../src/coefficient.rs#L1260): manual exponent-row reconstruction after dropping a parameter | Must replace now | After the existing proof that every dropped exponent is zero, use `evaluate_with_coeff_map` into the target `PolynomialRing<Z,u16>`; map retained variables to target generators and the dropped variable to zero | Preserve malformed-layout, dependence and exponent-range diagnostics. There is no need for a private term copier once absence has been authenticated. |
| P2 | [`parametric_coefficient.rs:9839-9861`](../../src/parametric_coefficient.rs#L9839): manual extension of every exponent row from the base map to the parametric map | Must replace now | Clone the authenticated base polynomial and call `MultivariatePolynomial::add_variables` with the index variables | Preserve exact variable-order/context validation and retained-memory admission. |
| P2 | [`parametric_coefficient.rs:8997-9445`](../../src/parametric_coefficient.rs#L8997) and [`parametric_coefficient.rs:12407-12978`](../../src/parametric_coefficient.rs#L12407): RustRed-owned affine polynomial exponentiation, weak-composition and Cartesian enumeration, radix sorting and collection | Must leave production | `MultivariatePolynomial::evaluate_with_coeff_map`, already used by the normal path at [`parametric_coefficient.rs:9458-9472`](../../src/parametric_coefficient.rs#L9458) | Generated-affine V2 currently selects the private engine at [`residual_affine_branch_guard_composition.rs:1741-1776`](../../src/residual_affine_branch_guard_composition.rs#L1741). Exact private-memory census is not an algebraic API gap. Keep the private engine only as a test oracle if useful. |
| P1 | [`parametric_coefficient.rs:4503-5417`](../../src/parametric_coefficient.rs#L4503), especially [`parametric_coefficient.rs:1422-1665`](../../src/parametric_coefficient.rs#L1422): polynomial-associate proof with private arbitrary-precision limb multiplication and signed accumulation | Must replace arithmetic now | Use `RationalPolynomial::to_polynomial(index_variables, false)`, then native rational-polynomial division and polynomial scaling, subtraction, and equality | Delete all private limb arithmetic. Preserve index-support grouping, strict-associate semantics, anchor provenance, bounded grouping and zero handling. |
| P2 | [`parametric_coefficient.rs:10133-10173`](../../src/parametric_coefficient.rs#L10133) and [`parametric_coefficient.rs:10411-10455`](../../src/parametric_coefficient.rs#L10411): manual permutation and full specialization/collection | Must replace now | Simultaneous `evaluate_with_coeff_map` into `PolynomialRing<Z,u16>` | Keep context-map validation and prospective/observed envelopes. Translation and partial specialization already use native `replace_with_poly`/`replace`. |
| P2 | [`symbolica_tensor_numerator.rs:1044-1180`](../../src/symbolica_tensor_numerator.rs#L1044): tensor-head-aware distributive expansion of `Atom` addition, multiplication and powers | Native-first API-gap migration | Use `AtomCore::expand` or `expand_via_poly` on the smallest authenticated tensor-containing subtree, or a public Symbolica transformer that preserves selective expansion | RustRed deliberately leaves scalar-only powers as opaque weights and enforces limits before allocation, so whole-expression `expand` is not a drop-in. Retain tensor grammar/preflight/decoding. If public composition cannot preserve those semantics, keep only the selective syntax wrapper, document the exact gap locally, and differentially test every admitted input against native expansion. |
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
| [`symmetry.rs:941-1066`](../../src/symmetry.rs#L941) and [`symmetry.rs:1167-1360`](../../src/symmetry.rs#L1167) | Upper-triangular scalar-product coordinates, off-diagonal folding, affine denominator semantics and an independently derived replay are domain-specific tensor-map logic. Scalar coefficient operations remain native. | Native ordinary matrix products for the standard sub-kernels, plus direct denominator substitution/replay. |
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
- the `u128` gcd at
  [`parametric_coefficient.rs:13205`](../../src/parametric_coefficient.rs#L13205)
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
| Affine composition | For V1 and V2 plans, compare native simultaneous evaluation with the controlled oracle on constants, translations, mixed affine images and cancellation-heavy inputs. Exercise exact and one-below resource gates and catch native panics as typed failures. |
| Polynomial associates | Test equality up to nonzero base-field rational factors, different index support, zero input, cancellation, large GMP coefficients and multiple base/index variables. Compare each grouped cross-product with native polynomial arithmetic. |
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
3. Move generated-affine production composition to native
   `evaluate_with_coeff_map`, and replace the private limb-based associate
   proof through `RationalPolynomial::to_polynomial` plus native operations.
4. Migrate direct matrix consumers: generic family, automatic ISP, tensor
   projectors, symmetry, symmetry discovery, and Feynman determinants.
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
