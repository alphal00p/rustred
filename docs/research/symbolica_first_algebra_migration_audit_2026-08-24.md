# Symbolica-first algebra migration audit

Status: read-only production-code audit, 2026-08-24. This document records
the migration work required by RustRed's public-Symbolica-first policy. It
does not change Rust code and does not claim a fresh test pass; the shared
tree had an unrelated event-ledger test run in progress during the audit.

The companion API inventory is
[`symbolica_exact_linear_algebra_api_inventory.md`](symbolica_exact_linear_algebra_api_inventory.md).

## Executive decision

RustRed already uses Symbolica's GMP-backed rational-polynomial values for
most coefficient arithmetic, but several default-production modules still
implement exact rationals, matrix algorithms, polynomial algorithms, or
integer primitives themselves. Those implementations are not permitted when
a public Symbolica operation supplies the same algebra.

The first blocking migration after the current chronological event-ledger
milestone is [`src/exact.rs`](../../src/exact.rs). Its fixed-width `i64`
`ExactRational` and hand-written matrix routines must be replaced before the
next algebraic milestone is treated as Symbolica-first. This is more than a
performance cleanup: the current type can overflow where the required GMP
domain is exact.

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
| B0 | [`exact.rs:10-303`](../../src/exact.rs#L10): fixed-width rational normalization, gcd, inverse, rank, multiply, transpose, determinant | Must replace now | `Rational`/`Q`; `Matrix<Q>::inv`, `rank`, `det`, multiplication and `transpose` | Remove all fixed-width algebra. A compatibility `ExactRational` name may be a type alias or a thin wrapper over Symbolica `Rational`, but it must not implement arithmetic itself. |
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
| P2 | [`parametric_coefficient.rs:4503-5417`](../../src/parametric_coefficient.rs#L4503), especially [`parametric_coefficient.rs:1422-1665`](../../src/parametric_coefficient.rs#L1422): polynomial-associate proof with private arbitrary-precision limb multiplication and signed accumulation | Must replace arithmetic now | Grouping by index support may remain; construct base-variable polynomials and use native polynomial `*`, `-`, and equality, or native rational-polynomial equality | Delete all private limb arithmetic. Preserve strict-associate semantics, anchor provenance, bounded grouping and zero handling. |
| P2 | [`parametric_coefficient.rs:10133-10173`](../../src/parametric_coefficient.rs#L10133) and [`parametric_coefficient.rs:10411-10455`](../../src/parametric_coefficient.rs#L10411): manual permutation and full specialization/collection | Must replace now | Simultaneous `evaluate_with_coeff_map` into `PolynomialRing<Z,u16>` | Keep context-map validation and prospective/observed envelopes. Translation and partial specialization already use native `replace_with_poly`/`replace`. |
| P2 | [`symbolica_tensor_numerator.rs:1044-1180`](../../src/symbolica_tensor_numerator.rs#L1044): tensor-head-aware distributive expansion of `Atom` addition, multiplication and powers | Native-first API-gap migration | Use `AtomCore::expand` or `expand_via_poly` on the smallest authenticated tensor-containing subtree, or a public Symbolica transformer that preserves selective expansion | RustRed deliberately leaves scalar-only powers as opaque weights and enforces limits before allocation, so whole-expression `expand` is not a drop-in. Retain tensor grammar/preflight/decoding. If public composition cannot preserve those semantics, keep only the selective syntax wrapper, document the exact gap locally, and differentially test every admitted input against native expansion. |
| P2 | [`symbolica_affine_denominator.rs:3585-3645`](../../src/symbolica_affine_denominator.rs#L3585): private coefficient power loop | Must replace now | `RationalPolynomialField::pow` through `Ring::pow` | The surrounding `AtomView` traversal and scalar-product contraction are justified semantic lowering and already use native coefficient arithmetic. The resource-envelope helper also named `checked_power` at line 4603 is bookkeeping, not algebra. |
| P3 | [`residual_affine_integer_lattice_kernel.rs:1382-1472`](../../src/residual_affine_integer_lattice_kernel.rs#L1382) and [`residual_affine_integer_system.rs:3038-3099`](../../src/residual_affine_integer_system.rs#L3038): private gcd and extended gcd | Must replace primitives | `Integer::gcd` and `Integer::extended_gcd` | Preserve positive-gcd and deterministic Bezout conventions by normalizing and verifying the native result; retain transcript and budget accounting. |
| P3 | [`four_loop_next_modular_rank.rs:738-838`](../../src/four_loop_next_modular_rank.rs#L738) and [`four_loop_next_modular_rank.rs:1068-1130`](../../src/four_loop_next_modular_rank.rs#L1068): private modular arithmetic, inversion, powering and primality | Must replace if the legacy feature is maintained | `Zp64`/`Zp`, finite-field `Ring` operations, and `Integer::is_prime` | This module is feature-gated by `legacy-authored-oracles`; it must remain evidence-only and must not become a generic production path. Its restricted Markowitz pivot controller may remain. |

### B0 migration impact

`ExactRational` is public at [`lib.rs:320`](../../src/lib.rs#L320), and the
private matrix routines are consumed throughout [`family.rs`](../../src/family.rs),
including basis rank/inversion and symmetry transformations. The migration
therefore needs one coherent change, not a second parallel rational type:

1. select `Rational`/`Q` as the only exact constant field;
2. adapt constructors and public compatibility methods without narrowing back
   to `i64`;
3. convert family matrices with `Matrix::from_nested_vec`;
4. call the public matrix operations behind checked shape and panic boundaries;
5. convert results back only where a certificate structure requires owned
   row-major values; and
6. delete the private gcd and matrix algorithms.

No `no_gmp` compatibility path should be added.

## RustRed-owned semantic wrappers that remain justified

These components are not generic replacements for Symbolica algebra. They
encode semantics that the public matrix API does not expose.

| RustRed owner | Why it remains RustRed-owned | Native differential oracle |
|---|---|---|
| [`exact_sparse_elimination.rs:315-595`](../../src/exact_sparse_elimination.rs#L315), [`exact_sparse_elimination.rs:1399-1637`](../../src/exact_sparse_elimination.rs#L1399) | The caller authenticates a hardest-first integral-column/source-row skeleton; the result retains a full reduction trace, provenance and replay. `SparseRowReducer` has no custom pivot callback or RustRed certificate transaction. Coefficient operations already call Symbolica. | Compare rank, row span and solved rows with `Matrix<RPF>` or `SparseMatrix<RPF>` after mapping integral keys to columns. |
| [`certified_rewrite.rs:1856-2000`](../../src/certified_rewrite.rs#L1856) | Scout reduction discovers the exact integral-order skeleton later replayed by `ExactSparseElimination`; it is pivot planning and certificate construction, not a new coefficient field. | Compare the selected system's rank and source-row dependencies with a public dense/sparse reducer. |
| [`parametric_elimination.rs:704-1182`](../../src/parametric_elimination.rs#L704), [`parametric_elimination.rs:1765-1924`](../../src/parametric_elimination.rs#L1765), and [`persistent_parametric_elimination.rs`](../../src/persistent_parametric_elimination.rs) | LiteRed ordering, index-shift columns, conditional pivot numerators/denominators, `WhenBad`, chronological traces and clean-prefix persistence are absent from public field elimination. Public elimination would silently treat every formal nonzero rational function as invertible. | At generic concrete specializations, compare rank/row span and normalized solutions with `Matrix<RPF>` or a finite-field image. Verify every retained guard separately. |
| [`residual_affine_integer_lattice_kernel.rs:970-1213`](../../src/residual_affine_integer_lattice_kernel.rs#L970) and [`residual_affine_integer_lattice_kernel.rs:1545-1660`](../../src/residual_affine_integer_lattice_kernel.rs#L1545) | Produces the complete integral affine solution lattice and a unimodular transform transcript. Public `Matrix<Z>::solve_fraction_free` returns one determined solution, not a Smith/Hermite-style lattice parameterization. | Check the rational affine span with `Matrix<Q>` and bounded integer-point enumeration; use native gcd inside the wrapper. |
| [`residual_affine_integer_system.rs:2273-2865`](../../src/residual_affine_integer_system.rs#L2273) and [`residual_affine_integer_system.rs:3293-3379`](../../src/residual_affine_integer_system.rs#L3293) | Implements LiteRed's original-coordinate unit-pivot cylinder search, unsupported-congruence boundary, affine projection and row-operation replay. | Compare satisfiability/rational rank with `Matrix<Q>` and exhaust small bounded integer boxes. Use native gcd/extended gcd. |
| [`zero_sectors.rs:774-776`](../../src/zero_sectors.rs#L774) and [`zero_sectors.rs:1047-1154`](../../src/zero_sectors.rs#L1047) | This is already the desired composition: native `Matrix<Q>::row_reduce`, followed by deterministic kernel-vector choice and primitive-integer certificate formatting. | Existing replay plus independent matrix-kernel checks. |
| [`symmetry.rs:941-1066`](../../src/symmetry.rs#L941) and [`symmetry.rs:1167-1360`](../../src/symmetry.rs#L1167) | Upper-triangular scalar-product coordinates, off-diagonal folding, affine denominator semantics and an independently derived replay are domain-specific tensor-map logic. Scalar coefficient operations remain native. | Native ordinary matrix products for the standard sub-kernels, plus direct denominator substitution/replay. |
| [`symbolica_affine_denominator.rs:1511-1765`](../../src/symbolica_affine_denominator.rs#L1511) | Recognizing and contracting the declared scalar-product function into loop/external coordinates is expression-language and family semantics. Grouping monomials by a scalar-product coordinate is not a replacement polynomial engine. | Recompile the retained `Atom`, compare explicit bilinear expansions, and compare differently parenthesized inputs. |
| [`tensor_family.rs:183-246`](../../src/tensor_family.rs#L183) and [`generic_tensor_family.rs:677-750`](../../src/generic_tensor_family.rs#L677) | These maps convolve affine scalar-product expansions under denominator-power-shift keys, not coefficient-ring monomials. The generic path intentionally admits `u64` shift coordinates, beyond Symbolica polynomial exponent domains, and retains per-input origin/resource semantics. Coefficient arithmetic is already Symbolica-owned. | On representable small shifts, encode the map as a Symbolica polynomial and compare every coefficient and key after each multiplication/power. |
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

1. Finish and validate the current chronological event-ledger milestone.
2. Complete B0: replace `ExactRational` and all `exact.rs` matrix consumers
   with `Q`/`Matrix<Q>`. Do not begin another private algebra implementation
   while this blocker remains.
3. Migrate small direct matrix consumers: generic family, automatic ISP,
   tensor projectors, symmetry and symmetry discovery.
4. Migrate Feynman-polynomial operations and determinants.
5. Replace parametric polynomial evaluation/composition, associate arithmetic,
   full specialization/permutation and tensor `Atom` expansion.
6. Replace gcd/extended-gcd and any maintained modular primitives.
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
