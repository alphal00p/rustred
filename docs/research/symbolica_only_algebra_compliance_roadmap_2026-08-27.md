# Symbolica-only production algebra compliance roadmap

Date: 2026-08-27

RustRed baseline audited: `4edebd02012b7b4839e0b7e0688a69cad5552112`.
This is a source and call-path audit, not an implementation milestone. None of
the gaps below is fixed by this document.

## Contract

All production algebra must be performed through public Symbolica APIs.
RustRed may own integral ordering, topology and tensor semantics, typed input
validation, resource admission, provenance, guards, scheduling, and independent
replay. It must not own a second polynomial, determinant, matrix-product, or
Gaussian-elimination engine. RustRed is pure Rust plus licensed, GMP-enabled
Symbolica: production and tests use no FORM process, no Mathematica kernel, and
no Symbolica `no_gmp` feature.

A lane cannot be called **production Symbolica-only** until its complete
reachable call graph contains no handwritten pivot/reduction, determinant,
matrix-product, polynomial expansion, or polynomial-collection algebra. When a
required operation has no adequate public Symbolica API, the production path
must return a typed unsupported/resource pause at that boundary; it must not
silently select private fallback algebra. An explicitly audited structural
wrapper may remain only when Symbolica still performs all algebra inside it.

For the six-loop programme, **P0** means “must be closed before a credible
production six-loop foundry or numerator-reduction claim.” It does not mean
that every item is on the immediate exceptional-publication implementation
path.

## Current production findings

| Priority and lane | Current path | Audit finding and required boundary |
|---|---|---|
| P0, foundry | [`parametric_elimination.rs:668`](../../src/parametric_elimination.rs#L668) | `ParametricElimination` and `PreorderedParametricElimination` still implement pivot selection, prior-pivot reduction, division, and normalization in handwritten Rust. They are reached by adaptive, conditional, persistent, sector-formula, `WhenBad`, and generated case/branch paths. Keep ordering, guards, chronology, and replay; move the row algebra and disposition authority to Symbolica `SparseRowReducer`/`SparseMatrix`. |
| P0, foundry/application seam | [`certified_rewrite.rs:734`](../../src/certified_rewrite.rs#L734) and [`exact_sparse_elimination.rs:315`](../../src/exact_sparse_elimination.rs#L315) | Concrete quotient rewriting still discovers a skeleton and runs a handwritten exact sparse Gaussian engine. This has production provider callers as well as loop-specific consumers. Keep integral-column planning and proof records; use a native Symbolica reducer for arithmetic, pivots, normalization, and dependencies. |
| P0, zero-sector foundation | [`feynman_polynomials.rs:294`](../../src/feynman_polynomials.rs#L294) and [`feynman_polynomials.rs:969`](../../src/feynman_polynomials.rs#L969) | The generic zero-sector path calls custom derivative, face, add/subtract/multiply/collect code and a subset-DP determinant/adjugate. The stored polynomial type is already Symbolica-native, but the algebra schedule is not. Use native polynomial arithmetic, `derivative`, `replace`, and `Matrix<PolynomialRing<_>>::det`; retain context authentication, structural minor placement, homogeneity checks, and resource envelopes. |
| P0, generic family | [`generic_family.rs:863`](../../src/generic_family.rs#L863) | Determinant/inverse/rank have migrated, but scalar-product expansion and derivative-contraction construction still contain handwritten affine matrix/vector products. These are in generic family construction and raw IBP generation, not a topology fixture. Route ordinary products through the authenticated Symbolica matrix boundary; retain coordinate conventions and direct physics replay. |
| P0, case algebra | [`parametric_coefficient.rs:11482`](../../src/parametric_coefficient.rs#L11482) and [`parametric_coefficient.rs:11760`](../../src/parametric_coefficient.rs#L11760) | Index-variable permutation and full specialization still rebuild exponent rows and collect/evaluate terms manually. Use `evaluate_with_coeff_map` or the appropriate simultaneous native substitution; retain map authentication and pre/postflight bounds. |
| P0, high-loop symmetry support | [`symmetry_discovery.rs:1276`](../../src/symmetry_discovery.rs#L1276) | The generic discovery fallback contains a private Bareiss determinant. Use `Matrix<Z>::det`. The exhaustive bounded candidate enumeration remains a small-family fallback/oracle and must not become the six-loop candidate source; graph automorphisms and routing equivalences feed the generic verifier instead. |
| P0, exceptional-domain closure | [`residual_affine_integer_system.rs`](../../src/residual_affine_integer_system.rs) | The live affine/congruence boundary owns an integer row-operation search and private gcd/extended-gcd loops. Native integer primitives should replace the latter. The complete integral-affine solution-lattice semantics cannot currently be replaced wholesale because of the public API gap below; the deterministic parameterization, lineage, and replay may remain RustRed-owned. |
| P0, online numerator path | [`symbolica_tensor_numerator.rs:1044`](../../src/symbolica_tensor_numerator.rs#L1044) | Tensor-head-aware distributive expansion is handwritten. Whole-expression `expand` is not automatically equivalent because scalar weights and selected powers are intentionally opaque. Prefer a proven composition of native `expand_in`/`expand` and collision-checked masking; otherwise retain only the selective syntax traversal as an explicitly tested API-gap wrapper. |
| P0, online numerator path | [`generic_tensor_family.rs:677`](../../src/generic_tensor_family.rs#L677) | Denominator-shift polynomial multiplication, exponentiation, collection, and coefficient accumulation are a private polynomial engine. Use a Symbolica multivariate polynomial over the authenticated coefficient field and native `Ring::pow`; retain only conversion of final monomials to integral shifts and origin bookkeeping. |
| P0, online numerator path | [`generic_tensor_projector.rs:2098`](../../src/generic_tensor_projector.rs#L2098) | Gram inversion and ordinary matrix products have migrated, but tensor metric/vector contraction and coefficient accumulation remain a mixed semantic/algebraic boundary. Lorentz-index graph connectivity, pairing enumeration, spectator conventions, and typed witnesses may remain; all coefficient and polynomial algebra inside the contraction must continue through Symbolica, with a differential native-oracle test for the retained structural wrapper. |

The P0 label on the tensor rows is for the complete six-loop QCD deployment.
They do not block the first derivation-only physical-family gate, but they do
block the later GammaLoop numerator corpus reduction.

## Already-native and non-production lanes

- [`exact.rs`](../../src/exact.rs), generic-family determinant/inverse,
  automatic-ISP rank, the authenticated tensor-projector Gram matrices, and
  the affine-family symmetry verifier delegate their algebra to Symbolica.
- The current generated-affine exact-group database owns a retained
  `SparseRowReducer<CheckedParametricField>` and treats Symbolica as transcript
  authority. This positive result does **not** migrate the separately reachable
  `parametric_elimination` or `exact_sparse_elimination` APIs.
- The old exact-group database rebuilding bridge is a `cfg(test)` differential
  oracle. It must stay out of production authority.
- Loop-authored reduction modules and code behind `legacy-authored-oracles`,
  including the private finite-field arithmetic in
  [`four_loop_next_modular_rank.rs`](../../src/four_loop_next_modular_rank.rs),
  are evidence/oracle lanes. They must not be linked into a generic production
  decision path.
- [`residual_affine_integer_lattice_kernel.rs`](../../src/residual_affine_integer_lattice_kernel.rs)
  is a crate-private isolated prototype with no non-test caller at this
  baseline. It is not evidence that exceptional-domain production ingress is
  complete.

## Public Symbolica API gaps

1. The pinned Symbolica API exposes fraction-free solving of a determined
   integer system, but no public Smith/Hermite normal form, complete integer
   affine parameterization, or integer kernel-basis API. RustRed therefore may
   retain the topology-neutral integer-lattice semantic controller, while
   delegating available integer/matrix primitives and checking its rational
   span with `Matrix<Q>`. An upstream SNF/HNF plus kernel-basis API would remove
   most of this unavoidable gap.
2. Symbolica has native expression expansion, but no documented fallible,
   resource-censused selective tensor-subtree expansion that preserves
   RustRed's opaque-spectator grammar. The wrapper must remain syntax control,
   not general algebra, until a native composition is authenticated. Inputs
   outside that proven composition must receive a typed unsupported pause.
3. Matrix/polynomial operations and `SparseRowReducer::{clone,add_cols}` have
   no typed allocation failure, cancellation hook, or complete retained/scratch
   byte census. RustRed can preflight shapes and visible outputs, but cannot
   claim a hard bound on Symbolica's internal allocations.

The retained sparse candidate stage is especially important on a roughly
100-core, 1-TiB host. Near
[`parametric_coefficient/symbolica_sparse/persistent.rs`](../../src/parametric_coefficient/symbolica_sparse/persistent.rs),
`self.native.clone()` makes committed and trial reducer states simultaneously
live. Admission must charge the committed reducer, the full trial (including
growth), candidate/result buffers, and opaque-scratch headroom before the
clone. `--n-cores` remains a ceiling: the scheduler must leave cores idle when
that old-plus-trial envelope, multiplied by active lanes, reaches the
operational RAM budget.

## P0 closure order and evidence

1. Migrate generic-family products, symmetry-discovery determinant, and the
   generic Feynman/zero-sector polynomial boundary, because they are upstream
   of family construction and sector pruning.
2. Replace every reachable handwritten parametric and concrete sparse
   elimination authority with the retained/native Symbolica reducer contract.
   Do not delete RustRed ordering, guards, provenance, or exact regenerated-row
   replay.
3. Migrate parametric permutation/specialization and native integer
   gcd/extended-gcd primitives. Preserve the integer-lattice controller only
   for the documented SNF/HNF/kernel-basis gap.
4. Before the online six-loop numerator campaign, authenticate native-first
   tensor expansion and polynomial lowering, and sharply separate structural
   tensor contraction from coefficient algebra.

Each migration needs a production call-graph check, parallel default-GMP
tests, native-versus-old differential fixtures before quarantine/deletion,
exact and one-below resource tests, and algebraically equivalent input forms.
A test-only oracle may remain only behind an explicit non-production boundary.
No acceptance command may enable `no_gmp` or invoke FORM or Mathematica.
