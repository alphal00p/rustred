# Symbolica API notes for RustRed

> **Historical API survey, current build policy.** This early report has been
> corrected to the current project requirements: RustRed uses the licensed
> GMP Symbolica backend and runs tests in parallel. The authoritative API audit is
> [`symbolica_rust_api_for_litered.md`](symbolica_rust_api_for_litered.md), and
> the checked-in `Cargo.toml` is the build authority. Do not copy the old
> feature snippet below.

This note is based only on the vendored Symbolica v2.2.0 source, local Rust examples, and tests at `vendor/symbolica` (git tag `v2.2.0`, commit `77c137481904b8a5531ede86e3ef36b82beed7fd`). It focuses on APIs useful for an exact, pure-Rust IBP implementation.

## Build and workspace integration

- Package: `symbolica` 2.2.0, edition 2024, minimum Rust 1.89 (`vendor/symbolica/Cargo.toml:3-18`). RustRed compiles every target and runs its tests against this exact vendored source with the pinned local Rust 1.89 toolchain. The project runner uses `cargo-nextest` with parallel workers; the old serial wrapper is deprecated and delegates to it.
- Defaults are `tracing_max_level_info`, `faster_alloc` (mimalloc global allocator), and `gmp` (`vendor/symbolica/Cargo.toml:31-38`). RustRed deliberately disables the default feature set only to avoid unrelated defaults, then explicitly enables `gmp` and `tracing_max_level_info`. It must not enable `no_gmp`. Never enable `gmp` and `no_gmp` together (`vendor/symbolica/lib/numerica/src/lib.rs:18-22`). Here “pure RustRed” means that the port and algorithms are Rust/Symbolica and invoke neither FORM nor Mathematica; it does not prohibit Symbolica's required licensed GMP backend.
- The checked-in root-manifest shape is:

  ```toml
  [dependencies]
  symbolica = { path = "vendor/symbolica", default-features = false, features = ["gmp", "tracing_max_level_info"] }

  [patch.crates-io]
  numerica = { path = "vendor/symbolica/lib/numerica" }
  graphica = { path = "vendor/symbolica/lib/graphica" }
  ```

  Cargo only honors `[patch]` in the top-level workspace. The patch declarations inside `vendor/symbolica/Cargo.toml` are not inherited when Symbolica is merely a path dependency; repeat them in RustRed's workspace root or Cargo may resolve crates.io `numerica`/`graphica` instead of the audited vendored source.
- Relevant optional features: `full_fn_cmp` (argument-by-argument function ordering), `serde` (state-independent objects and evaluators, but not general `Atom`), and `bincode` (state-aware encoding for `Atom`, polynomials, symbols). Python/Mathematica API features are irrelevant to a pure-Rust port.
- The Symbolica source license is not an open-source redistribution grant: `vendor/symbolica/License.md` says copying/distribution requires express permission and professional use requires the applicable license. RustRed can call the public API, but redistribution and CI/deployment licensing must be settled.

## Recommended RustRed split

Use Symbolica for exact scalar coefficients and algebra, but keep the integral identity/order layer in ordinary Rust:

- key an integral by a compact Rust exponent vector/topology ID, not by repeatedly pattern-matching `Atom`;
- use `Atom` for input/output and rule prototypes;
- immediately convert coefficients in `d` (and any mass ratios) to `RationalPolynomial<Z, u16>`;
- assemble homogeneous IBP rows directly into `SparseRowReducer<RationalPolynomialField<Z, u16>>` for the first milestones;
- order matrix columns from most reducible/most complicated to masters, because the sparse reducer chooses the first (lowest-index) available nonzero as pivot;
- introduce finite-field sampling plus RustRed-owned interpolation/reconstruction before 4/5 loops.

This avoids using the general expression tree as the database for millions of integrals while still using Symbolica extensively for the hard algebra.

## Expressions, symbols, and functions

The general expression types are `symbolica::atom::{Atom, AtomView, AtomCore, Symbol}` and are all re-exported by `symbolica::prelude::*` (`vendor/symbolica/src/lib.rs:73-158`). `AtomView<'a>` is the cheap borrowed representation; `Atom` owns its compact serialized buffer.

Primary constructors:

- `parse!`, `try_parse!`, `parse_lit!`; direct fallible API `Atom::parse(input, namespace, ParseSettings)` (`atom.rs:3029-3076`, macros at `4104-4199`).
- `symbol!`/`try_symbol!` and `get_symbol!`; symbols live in a process-global `State` and carry a namespace (`atom.rs:598-1170`, macros at `3763-4017`). A conflicting redefinition with different attributes errors/panics.
- `Atom::var(symbol)`, `Atom::num(value)`, `Symbol::to_atom()` (`atom.rs:3015-3095`, `1144`).
- `function!(f, args...)`, `f.call((args...))`, `f.call_args(iterator)`, or `FunctionBuilder::{new,add_arg,add_args,finish}` (`atom.rs:1147-1191`, `3270-3578`). `finish()` normalizes the result.
- Ordinary `+`, `-`, `*`, `/`, unary `-`, and `.pow(...)` are implemented for useful mixtures of `Atom`, `AtomView`, `Symbol`, and exact numbers (`atom/ops.rs`). For many operands prefer `Atom::add_many` and `Atom::mul_many` (`atom.rs:4202-4236`).

```rust
use symbolica::prelude::*;

let (d, a, b, integral) = symbol!("d", "a", "b", "I");
let iab = integral.call((a, b));
let expr = (d - 2) * iab + integral.call((a + 1, b - 1));
assert_eq!(expr, parse!("(d-2)*I(a,b)+I(a+1,b-1)"));
```

The macros automatically prefix otherwise unqualified names with `env!("CARGO_CRATE_NAME")`; thus `symbol!("d")` and `try_parse!("d")` at a RustRed call site both resolve to `rustred::d` (`atom.rs:151-260`). Preserve that stable internal namespace, and use an explicit `default_namespace = "rustred"` or a label-to-symbol map when parsing input at call sites owned by another crate. Do not silently create a second, consumer-namespaced parameter symbol. This avoids collisions and makes state import conflicts easier to reason about.

`ParseSettings::{symbolica,mathematica,polynomial}` and builder methods `.mode`, `.convert_mul_to_atom`, `.distribute_neg` are in `parser.rs:102-166`. Mathematica parsing accepts a subset of `InputForm`; it does not make LiteRed rule/program syntax directly executable.

Useful `SymbolAttribute` variants are `Symmetric`, `Antisymmetric`, `Cyclesymmetric`, `Linear`, `Scalar`, `Real`, `Integer`, and `Positive` (`atom.rs:598-618`). Symmetry and linear/scalar attributes are applied during normalization, so they are useful for scalar products and tensor heads. In particular, a `Linear` function is multilinear in every argument: sums are distributed, numeric coefficients and factors built only from `Scalar` symbols are pulled out, and a zero argument makes the function zero (`normalize.rs:1124-1210`). For example:

```rust
use symbolica::prelude::*;

let sp = symbol!("rustred::sp"; Symmetric, Linear);
let c = symbol!("rustred::c"; Scalar);
let (k1, k2) = symbol!("rustred::k1", "rustred::k2");
let expanded = sp.call((k1 + k2, c * k1));
// `expanded` is normalized as c*sp(k1,k1) + c*sp(k1,k2).
```

`symbol!` also supports `norm`, `der`, `series`, `eval`, `tags`, aliases, and user data (`atom.rs:3595-4017`), but callback-bearing symbols cannot be faithfully restored from an exported state.

## Normal form, expansion, and tensor canonicalization

Parsing, arithmetic operators, function construction, and replacement normalize recursively. Normalization sorts commutative sums/products, merges equal factors/powers, combines equal terms and exact coefficients, and applies function attributes (`normalize.rs`). This is a canonical structural form, but **not an expanded form**: call `AtomCore::expand()` or `expand_via_poly::<E, _>()` explicitly (`atom/core.rs:522-625`).

Consequences:

- structural `Eq`/`Hash` on normalized `Atom` is fast and useful within one Symbolica state;
- internal symbol IDs and hence ordinary ordering depend on symbol definition order;
- for stable text independent of definition order use `to_canonical_string()` or `to_canonically_ordered_string(CanonicalOrderingSettings)` (`atom/core.rs:1408-1446`);
- inspect with `terms()`, `children()`, `visitor(...)`, and the `AtomView::{Num,Var,Fun,Pow,Mul,Add}` variants rather than reparsing strings.

`AtomCore::canonize_tensors(indices)` returns `CanonicalTensor { canonical_form, external_indices, dummy_indices }` and respects symmetric/antisymmetric/cyclic tensor heads (`atom/core.rs:1499-1539`, `tensors.rs:13-177`). It canonicalizes dummy-index names and tensor networks; it does **not** implement Lorentz contractions or vacuum tensor-to-scalar reduction. RustRed still needs the rotational/tensor-reduction identities, metric contractions, and dimension factors outside FORM.

## Pattern matching and rewrites

Core types are in `symbolica::id`: `Pattern`, `Match`, `MatchStack`, `Condition`, `ConditionResult`, `WildcardRestriction`, `MatchSettings`, `Replacement`, and `ReplaceBuilder`; the common ones are in the prelude (`id.rs`, exports at `lib.rs:114-119`).

Wildcard syntax and exact cardinality:

- `x_`: exactly one atom;
- `x__`: one or more members of an argument/addition/multiplication slice;
- `x___`: zero or more members;
- `f_(...)`: wildcard function head, returned as `Match::FunctionName`;
- `Pattern::set_optional(symbol!("x_"))`: allow the context default (0 in a sum, 1 in a product/power); alternatives use `p1 | p2` and must bind the same wildcard set.

The cardinalities come from `MatchStack::get_range_impl` (`id.rs:4810-4860`). A multi-match is `Match::Multiple(SliceType, Vec<AtomView>)`; `Match::to_atom()` wraps an argument sequence in built-in `arg(...)`, whereas wildcard substitution on a RHS splices it into the surrounding function (`id.rs:4371-4500`).

```rust
use symbolica::prelude::*;

let xs = symbol!("xs__");
let n = symbol!("n_");
let condition = xs.restrict(WildcardRestriction::Length(1, Some(3)))
    & n.filter(|a| a.is_integer() && a.is_positive());

let out = parse!("F(a,b,2)+F(c,-1)")
    .replace(parse!("F(xs__,n_)"))
    .when(condition)
    .with(parse!("G(xs__,n_-1)"));
assert_eq!(out, parse!("G(a,b,1)+F(c,-1)"));
```

Conditions compose with `&`, `|`, and `!`. Available restrictions are `Length`, `IsAtomType`, `HasTag`, `IsLiteralWildcard`, custom `Filter`, cross-wildcard `Cmp`, and `NotGreedy` (`id.rs:3610-3840`). Convenience methods are `Symbol::{restrict,filter,filter_match,filter_cmp,filter_tag}`. For a condition depending on several partly bound wildcards, use `Condition::match_stack`; return `ConditionResult::Inconclusive` until enough bindings exist (`id.rs:3680-3986`). Closure restrictions/RHS maps must be cloneable, `Send`, and `Sync`.

Rewrite controls:

- `expr.replace(pattern).with(rhs)` uses the first canonical match at every non-overlapping matched atom;
- `.with_map(|matches: &MatchStack| -> Atom { ... })` computes a dynamic RHS;
- `.when(condition)`, `.once()`, `.repeat()`, `.bottom_up()`, `.nested()`, `.level_range(...)`, `.partial(false)`, `.non_greedy_wildcards(...)`, and `.rhs_cache_size(...)` customize matching (`id.rs:426-780`);
- `.iter(rhs)` enumerates one-replacement alternatives; `.match_iter()` or `expr.pattern_match(...)` enumerates bindings; `next_detailed()` also exposes tree position and matched-slice flags (`id.rs:6166-6539`);
- `replace_multiple([Replacement::new(lhs, rhs), ...])` applies a rule set in one traversal (`atom/core.rs:1900-2008`).

For hot IBP loops, compile `Pattern` and `Condition` once and reuse them. Prefer `with_into`/`ReplaceIterator::next_into` when reusing output buffers.

## Polynomial and rational-function coefficients

Algebraic domains follow `Set -> Ring -> EuclideanDomain -> Field`; the **domain object** performs operations on its `Element` (`vendor/symbolica/lib/numerica/src/domains.rs:1-280`). Important domains are:

- integers: `Integer`, `IntegerRing`, constant/type alias `Z`;
- rationals: `Rational`, `FractionField<IntegerRing>`, constant/type alias `Q`;
- finite fields: `Zp = FiniteField<u32>`, `Zp64 = FiniteField<u64>`, and `Z2`;
- expression field: `AtomField`;
- rational-function field: `RationalPolynomialField<R,E>`.

`MultivariatePolynomial<F,E,O=LexOrder>` stores sparse terms but a dense exponent vector per term; its `coefficients`, `exponents`, `ring`, and `variables` are public (`poly/polynomial.rs:290-326`). Terms remain expanded and sorted. Use `u16` unless bounds prove `u8` safe; exponent overflow/panics in generated IBPs would be a bad failure mode. `GrevLexOrder` is also available.

Conversions on `AtomCore` (`atom/core.rs:1135-1295`):

- `try_to_polynomial(field, var_map)` / `to_polynomial`;
- `to_polynomial_in_vars::<E>(vars)` gives `MultivariatePolynomial<AtomField,E>` and collects everything else into expression coefficients without expanding;
- `try_to_rational_polynomial(input_field, output_field, var_map)` / infallible version;
- `set_coefficient_ring(vars)` embeds rational functions of selected variables as compact `Coefficient::RationalPolynomial` values inside an `Atom`.

For IBPs, use `to_polynomial_in_vars` to extract coefficients of explicit integral functions from an already expanded linear combination, then convert each coefficient to a `RationalPolynomial<Z,u16>`. Alternatively `Atom::system_to_matrix` performs that conversion for dense systems.

Be aware that generic conversions automatically promote non-polynomial subexpressions to new independent `PolyVariable`s. Supply an explicit variable map and validate `poly.variables` when a malformed coefficient must be rejected rather than silently accepted.

`RationalPolynomial<R,E>` has public `numerator` and `denominator`, each a `MultivariatePolynomial`; construction/arithmetic unifies variable maps, cancels polynomial GCDs where supported, and normalizes denominator sign/leading coefficient (`domains/rational_polynomial.rs:40-229`, `297-523`). Main APIs include `inv`, `pow`, `gcd`, `derivative`, `evaluate`, `apart`, and `to_polynomial`. Convert back with `.to_expression()` (`poly.rs:2170-2259`, `2321-2355`).

For LiteRed `WhenBad`, `RationalPolynomial::to_polynomial(base_variables,
true)` is the important parameter-coefficient projection: it returns a
polynomial in the declared base parameters whose coefficients are rational
polynomials in the remaining index variables. A coefficient denominator is
identically zero in the base parameters only when every retained coefficient
is zero. Do not replace this vector condition by the pointwise predicate that
the complete denominator vanishes; for example `n+d` has unit coefficient in
`d` and is never identically zero as a parameter polynomial.

For canonical associate classes after projection to the exact field `K[n]`,
public `MultivariatePolynomial<F: Field>::make_monic` supplies the canonical
representative (`poly/polynomial.rs:4206`). Cache one checked monic form per
unique locus and use a hash only to select candidates; exact Symbolica
polynomial equality is the proof. This avoids quadratic pairwise cross-product
associate tests without implementing algebra in RustRed. Projection, monic
normalization, factorization, GCD, and division remain infallible/unbudgeted
public calls in this vendored version, so wrappers must preflight, catch unwind,
authenticate output bounds/maps, and return operational failure on exhaustion.

```rust
use std::sync::Arc;
use symbolica::prelude::*;

let d = symbol!("d");
let vars = Arc::new(vec![PolyVariable::Symbol(d)]);
let c: RationalPolynomial<_, u16> = parse!("(d-2)/(2*d-3)")
    .try_to_rational_polynomial(&Q, &Z, Some(vars.clone()))
    .expect("coefficient must be rational in d");
assert_eq!(c.to_expression(), parse!("(d-2)/(2*d-3)"));
```

`AtomField::new()` defaults to a statistical zero test and no cancellation-on-division (`domains/atom.rs:20-76`). Do not use that default as the correctness domain for final IBP elimination. If prototyping with `Matrix<AtomField>`, explicitly set `statistical_zero_test: false` and `cancel_check_on_division: true`; exact `RationalPolynomialField` is preferable.

## Dense and sparse linear algebra

Dense APIs are `symbolica::tensors::matrix::{Matrix,Vector,MatrixError}` (only `Matrix`/`Vector` are in the prelude). `Matrix<F>` supports construction, row iteration, transpose, determinant, inverse, `solve`, `solve_any`, and row reduction. For `F: EuclideanDomain`, `solve_fraction_free` and fraction-free row reduction delay denominator growth (`numerica/src/tensors/matrix.rs:311-462`, `707-1055`, `1474-1760`).

At expression level:

- `Atom::solve_linear_system::<E,_,_>(&equations, &unknowns)` treats each expression as zero and returns exact `Atom` solutions; an underdetermined system returns `SolveError::Underdetermined { rank, partial_solution }` (`atom/core.rs:760-862`, `solve.rs:283-571`).
- `Atom::system_to_matrix::<E,_,_>` returns a dense `Matrix<RationalPolynomialField<Z,E>>` and RHS, inferring remaining expressions as parameters.

Use these for the 2-loop correctness milestone, not for the large Laporta systems.

Sparse types are **not** in the prelude; import:

```rust
use symbolica::tensors::sparse::{
    LuLMode, SparseMatrix, SparseRowReducer, SparseVector,
};
```

`SparseMatrix::from_csr` and `from_triplets` require entries sorted by `(row,column)` and do not remove/check zero entries (`numerica/src/tensors/sparse.rs:401-531`). `SparseRowReducer<F: Field>` is the important incremental API: `new`, `add_row`, `add_matrix`, `back_substitute`, `u`, and `pivots` (`sparse.rs:1497-1835`). It discards dependent rows and chooses the lowest column index with an available nonzero as pivot (`sparse.rs:1859-2050`). Therefore RustRed's integral ordering must be reflected directly in column numbering.

```rust
use std::sync::Arc;
use symbolica::prelude::*;
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

let d = symbol!("d");
let vars = Arc::new(vec![PolyVariable::Symbol(d)]);
let c0: RationalPolynomial<_, u16> =
    parse!("d-2").to_rational_polynomial(&Q, &Z, Some(vars.clone()));
let c2: RationalPolynomial<_, u16> =
    parse!("-2").to_rational_polynomial(&Q, &Z, Some(vars));
let field = RationalPolynomialField::from_poly(&c0.numerator);

// Column 0 is deliberately a harder integral than column 2.
let mut reducer = SparseRowReducer::new(3, field, LuLMode::None);
reducer.add_row(&[c0, c2], &[0, 2]); // column indices must be sorted
reducer.back_substitute();
for (_row, columns, values) in reducer.u().row_iter() {
    // A normalized reduction relation; reducer.pivots() identifies its LHS.
    assert_eq!(columns.len(), values.len());
}
```

For IBPs, underdeterminedness is expected: columns with `pivots()[col] == None` are master/free integrals. Read relations through `reducer.u().row_iter()` after back substitution. `SparseMatrix::solve` instead treats underdeterminedness as an error. Also, its returned `SparseVector` currently exposes no public value/index getters or iterator (`sparse.rs:43-180`), so the incremental reducer is more usable without patching the vendored crate.

`SparseMatrix::{values,col_idcs,row_ptrs}` and `SparseRowReducer::pivots` return
their backing `Vec`s, so capacities can support a shallow allocation-slot
census. They do not expose the reducer's private dense scratch capacity, deep
coefficient allocations, allocator overhead, workspace caches, or RSS.

`solve_parallel`/`back_substitute_parallel` only parallelize back substitution; forward elimination remains serial and the parallel version may permute rows (`sparse.rs:1078-1124`, `2393-2455`). There is no sparse fraction-free reducer.

## Finite fields and reconstruction

`Zp::new(p)` and `Zp64::new(p)` construct odd-prime fields using Montgomery arithmetic; import `FiniteFieldCore` for `to_element`, `from_element`, `get_prime`, and symmetric conversion (`numerica/src/domains/finite_field.rs:30-201`, `626-783`). Ring operations are methods on the field (`field.add`, `mul`, `div`, `inv`), not normally operators on bare `FiniteFieldElement`.

Useful paths for later milestones:

- convert a rational polynomial with `.to_finite_field(&field)` and evaluate its variables with `.evaluate(&sample)` (`domains/rational_polynomial.rs:187-229`, `709-734`);
- run `SparseRowReducer<Zp>` or `<Zp64>` at many good primes/sample points;
- combine residues with `Integer::chinese_remainder` and recover scalar rationals with `Rational::maximal_quotient_reconstruction` (`numerica/src/domains/integer.rs:1697-1766`, `rational.rs:1066-1123`).

`Rational::rational_reconstruction` reconstructs the value of a black-box function at a fixed rational sample across primes; it is **not** multivariate rational-function interpolation (`rational.rs:1125-1194`, example `examples/rational_reconstruction.rs`). Symbolica exposes no turnkey public API here for reconstructing every `d`-dependent row-reduction coefficient. RustRed must own univariate/multivariate interpolation, bad-prime/sample rejection, normalization, and verification. Massive single-scale bubbles should reduce this initially to univariate reconstruction in `d`.

## Serialization

Preferred checkpoint format for general expressions:

```rust
use symbolica::prelude::*;

let expr = parse!("f(x)+x^2");
let mut bytes = Vec::new();
expr.export(&mut bytes).expect("export expression plus global State");
let loaded = Atom::import(&mut &bytes[..], None).expect("import expression");
assert_eq!(loaded, expr);
```

`AtomCore::export`/`AtomView::export` writes the Symbolica state followed by the expression; `Atom::import` merges that state and accepts an optional conflict-renaming closure (`atom/core.rs:124-134`, `atom/representation.rs:1958-1991`, `532-607`). Stateless `AtomView::write` requires separately calling `State::export`, then `State::import` and `Atom::import_with_map` (`tests/import_export.rs`). The binary state format is version 4 and rejects other versions (`state.rs:29-33`, `1064-1145`).

The `bincode` feature implements state-context-aware encoding for `Atom`, `Symbol`, and `MultivariatePolynomial`, but decoding an `Atom` needs a context implementing `HasStateMap`; it is not a drop-in `bincode::decode_from_slice::<Atom>` (`atom/representation.rs:472-525`). The `serde` feature does not serialize general `Atom`. For early RustRed checkpoints, use Symbolica's export/import for Atom payloads and version RustRed's surrounding Rust metadata explicitly.

For portable human-readable snapshots use `.printer(PrintOptions::file())` or `.to_canonical_string()`, understanding that parsing text can be slower than binary import.

## Performance and parallelism

- Borrow `AtomView` and use `*_into` methods (`expand_into`, `derivative_into`, replacement `with_into`/`next_into`) in hot loops to reduce allocation. Symbolica's internal `Workspace` recycles per-thread Atom buffers (`state.rs:1290-1335`).
- Prefer polynomial/rational-polynomial representations for coefficients; use `set_coefficient_ring` when keeping a larger `Atom` expression. Once variables are embedded in a coefficient, pattern traversal treats that coefficient atomically.
- Prefer `Atom::add_many` over a long `fold` of binary additions. For large expanded sums, `TermStreamer<W>` can sort/combine terms using a configurable packed-payload spill threshold and optional Brotli streams (`streaming.rs`, `examples/streaming.rs`). That threshold is not a bound on Atom capacity, sorting overlap, compression buffers, workspace caches, allocator overhead, or RSS.
- `map_terms(f,n_cores)` and `map_terms_with_pool` parallelize independent terms (`atom/core.rs:1450-1497`), but only the latter borrows an existing pool. `TermStreamer::new` constructs its own pool. `SparseRowReducer::back_substitute_parallel` uses ambient Rayon independently through vendored `numerica` and accepts no pool argument.
- On the automatic `RecycledAtom::drop` path, Symbolica's private thread-local `Workspace` retains at most 30 Atom buffers whose capacity is at or below 20,000,000 bytes. Public direct `Workspace::return_atom` bypasses both caps, and threads that touch this API may initialize distinct workspaces. There is no public occupancy census or trim API, so high-core RustRed runs must include the coordinator and all potentially warmed worker/inner threads in their opaque-native reserve and external RSS calibration.
- A serious runtime constraint is enforced in `LicenseManager` (`lib.rs:351-758`): without a valid license Symbolica restricts itself to one instance and one core, installs a one-thread global Rayon pool, and aborts if a checked entry point is called from another process/thread relative to the initializing one. A mutex around calls on different worker threads is therefore insufficient: keep every Symbolica entry point in one non-overlapping process on one dedicated OS thread, or configure a valid license. `SYMBOLICA_HIDE_BANNER` only suppresses the banner; it does not relax these checks. `map_terms` and `TermStreamer` silently select one core when unlicensed. With no key, first use also spawns a best-effort connection to Symbolica's server carrying the version (`lib.rs:527-545`). Resolve licensing/offline policy before designing RustRed's parallel execution around `Atom` operations.

## API gaps RustRed must fill

1. Topology-aware sector/symmetry canonicalization and Laporta ordering; Symbolica's general tensor and pattern canonicalizers do not know integral families.
2. Lorentz/tensor reduction (including vacuum rotational averages) and scalar-product-to-denominator linear maps. `canonize_tensors` only canonicalizes index labels/networks.
3. A sparse underdetermined reduction facade that turns `SparseRowReducer::u()`/`pivots()` into `hard_integral -> linear combination of masters` rules.
4. Finite-field sampling, rational-function interpolation/reconstruction, unlucky-prime handling, and exact verification for 4/5-loop scaling.
5. Stable RustRed checkpoint metadata for integral keys/order/topologies around Symbolica's coefficient serialization.

## Most useful local references

- Public prelude and feature surface: `vendor/symbolica/src/lib.rs`, `vendor/symbolica/Cargo.toml`
- Atoms/symbols/builders/macros: `vendor/symbolica/src/atom.rs`, `src/atom/core.rs`, `src/atom/ops.rs`
- Parser: `vendor/symbolica/src/parser.rs`
- Pattern engine: `vendor/symbolica/src/id.rs`, `examples/pattern_match.rs`, `examples/pattern_restrictions.rs`, `tests/pattern_matching.rs`
- Polynomial/rational functions: `src/poly.rs`, `src/poly/polynomial.rs`, `src/domains/rational_polynomial.rs`
- Dense solver: `src/solve.rs`, `lib/numerica/src/tensors/matrix.rs`, `examples/solve_linear_system.rs`
- Sparse solver: `lib/numerica/src/tensors/sparse.rs`
- Finite fields/reconstruction: `lib/numerica/src/domains/{finite_field,integer,rational}.rs`
- State/serialization: `src/state.rs`, `src/atom/representation.rs`, `tests/import_export.rs`
- Performance/streaming: `src/streaming.rs`, `examples/streaming.rs`
