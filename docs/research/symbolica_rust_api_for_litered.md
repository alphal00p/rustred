# Symbolica Rust API boundary for a LiteRed-complete RustRed

Status: source audit of vendored Symbolica 2.2.0, reconciled with the RustRed
source through the automatic-ISP-rank and tensor-projector milestones on
2026-08-25.
This report is implementation-oriented and covers the API boundary needed for
generic, parametric IBP/LI generation, sector and symmetry handling, guarded
rule solving/application, persistence, and a Vakint/FORM-inspired
tensor-numerator front end. It does **not** treat concrete topologies as part
of the core algorithm; those belong only in tests and oracle validation.

The initial source audit did not run Cargo, so code fragments below remain
source-derived idioms unless an executed regression is cited.  The reconciled
RustRed boundary was subsequently exercised under licensed, GMP-enabled
Symbolica in parallel run `5ae578f9-5bff-4cf9-bf3f-7013730923ee`: 20/20 generic
IBP, Symbolica tensor-numerator, and FORM-free Vakint oracle tests passed with
`cargo nextest run -j4`.  That checkpoint is evidence for the named exercised
APIs, not a substitute for the still-open probes in section 17.

Later parallel checkpoints migrated the generic-family denominator-basis
determinant/inverse, automatic-ISP rank, and tensor-projector Gram
power/determinant/inverse/two-sided replay to Symbolica's public APIs through
the checked contextual field adapter documented in section 10.1.1. This is not
a claim that all RustRed matrix, polynomial, or elimination consumers have
migrated.

Confirmed upstream correctness defects and the fallible/resource-aware API gap
are summarized with standalone reproductions in
[`symbolica_upstream_gap_audit_2026-08-25.md`](symbolica_upstream_gap_audit_2026-08-25.md).

## 1. Executive decision

Symbolica is a strong exact-algebra backend for RustRed, but it is not a LiteRed algorithm library. Use it for:

- canonical symbolic `Atom` construction and inspection at I/O boundaries;
- sparse multivariate polynomials and exact rational-polynomial coefficients;
- GCD, factorization, Groebner bases, finite fields, CRT/rational reconstruction primitives;
- dense and sparse linear-algebra building blocks;
- optional tensor-network canonical naming after RustRed has performed physics-aware contractions;
- binary Atom/state transport and selected out-of-core Atom term transforms.

RustRed must itself own:

- the typed loop/external-momentum lattice and scalar-product algebra;
- propagator-family definitions, independent scalar products, kinematic ideals and assumptions;
- integral keys, sectors, zero sectors, routing/symmetry maps and integral ordering;
- generic parametric IBP and Lorentz-invariance identity generation;
- a serializable guard language and its exact evaluator;
- guard/provenance-aware sparse elimination and reconstructed-rule verification;
- direct parametric rule matching, index substitution, termination and memoization;
- Lorentz tensor contraction, projector/basis construction, Gram guards, and conversion of tensor numerators to scalar integrals;
- the persistent rule database and its schema/version/proof metadata.

The central safety rule is: **an algebraically simplified rational function is not a complete conditional identity**. Symbolica correctly cancels in a fraction field, but the exceptional loci erased by cancellation must remain in RustRed's explicit guard set.

## 2. Audited version, features and preludes

The vendored crate is `symbolica = 2.2.0`, edition 2024, requiring Rust 1.89 (`vendor/symbolica/Cargo.toml:7-18`). Default features are `tracing_max_level_info`, `faster_alloc`, and `gmp`; `serde` and `bincode` are optional and distinct (`vendor/symbolica/Cargo.toml:38-72`). `bincode` is the feature that enables state-mapped encoding for polynomial objects; enabling `serde` alone is not enough.

The public prelude reexports Atom construction, domains, pattern matching, polynomial types, tensors and state facilities (`vendor/symbolica/src/lib.rs:90-175`). RustRed should nevertheless import concrete types and traits in core modules, so a Symbolica upgrade cannot silently change behavior through a prelude expansion.

The `faster_alloc` feature installs mimalloc globally (`vendor/symbolica/src/lib.rs:263-270`). That is a process-wide choice, not a local coefficient-engine setting.

RustRed deliberately disables Symbolica's default features and enables only
`gmp` and `tracing_max_level_info` (`Cargo.toml:33-37`). Consequently the
reference build is GMP-backed, never `no_gmp`, and does **not** install the
`faster_alloc`/mimalloc global allocator.  Any future feature change is an
artifact/toolchain change and must be recorded and retested.

RustRed's direct `rand = "0.9"` dependency is an interface dependency only.
The public Symbolica `Ring` trait requires every implementation to provide
`sample(&mut impl rand::RngCore, ...)`; the checked contextual coefficient
field must therefore name the same `rand::RngCore` type. Its implementation
delegates sampling to Symbolica's integer ring. This dependency does not add a
RustRed random matrix method or a second computer-algebra implementation.

## 3. Recommended RustRed architecture

The Symbolica boundary should be narrow and explicit:

1. `SymbolRegistry`: initialize licensing first and pre-register a deterministic, namespaced symbol set before worker threads start.
2. `FamilyModel`: typed loop/external vectors, affine quadratic propagators, independent scalar products, coefficient parameters and kinematic relations.
3. `IntegralKey`: family id, integer or parametric-affine index vector, and numerator/tensor descriptor. Its ordering is RustRed-defined.
4. `CoeffContext`: one canonical ordered `Arc<Vec<PolyVariable>>` plus strict construction, remapping and guarded substitution for `RationalPolynomial<Z, E>`.
5. `Guard`: a serializable AST for integer/sign/sector constraints and exact polynomial equalities/nonzero assumptions.
6. `IdentityGenerator`: generic IBP total derivatives and non-vacuum Lorentz-invariance generators, emitting sparse shifted integral equations.
7. `SectorEngine`: bitset/cone sectors, zero-sector analysis, graph/routing symmetries and exact integer-affine maps.
8. `RuleSolver`: complexity-ordered sparse elimination, pivot guards, provenance, optional modular reconstruction, mandatory exact replay.
9. `RuleApplier`: typed key matching and exact coefficient specialization. Symbolica patterns may be an outer syntax dispatcher only.
10. `TensorReducer`: Vakint/FORM-inspired typed Lorentz reduction before scalar integral rules are applied.
11. `ArtifactStore`: application schema, Symbolica state remap, family/variable hashes, guards, provenance and verification records.
12. `OracleAdapter`: concrete non-parametric examples and Vakint comparisons only in tests.

## 4. Atoms, views, symbols, state and native functions

### 4.1 Atom ownership and construction

Use owned `Atom` for stored expressions and `AtomView<'_>` for traversal/hot-path borrowing. `AtomCore` exposes common operations over both. `AtomCore::export` delegates to `AtomView::export` (`vendor/symbolica/src/atom/core.rs:129-133`).

Prefer typed construction with `Symbol::call`, `Symbol::call_args`, `FunctionBuilder`, `function!`, `symbol!`/`try_symbol!`, and `Atom::num` rather than repeated string parsing. The relevant APIs are at:

- symbol parsing/calls: `vendor/symbolica/src/atom.rs:1100-1187`;
- `FunctionBuilder`: `vendor/symbolica/src/atom.rs:3269-3338`;
- `function!`: `vendor/symbolica/src/atom.rs:3546-3580`;
- `symbol!` and `try_symbol!`: `vendor/symbolica/src/atom.rs:3603-3897`;
- mutually dependent callback symbols via `symbol_group!`: `vendor/symbolica/src/atom.rs:3984-4040`.

For large flat sums use `Atom::add_many`, which performs an n-way merge rather
than building a left-deep chain.  `Atom::mul_many` also avoids a caller-built
left-deep expression, but it constructs a product and normalizes it; it is not
the same n-way merge algorithm (`vendor/symbolica/src/atom.rs:4199-4235`). Use
the thread-local `Workspace`/`RecycledAtom` facilities in intensive Atom
transforms (`vendor/symbolica/src/state.rs:1285-1444`). The local recycled cache
holds at most 30 atoms and caps retained atom capacity at 20 MB, so it is a
scratch allocator, not persistent storage.

`to_canonical_string` is process-independent and appropriate for diagnostics and semantic hashes (`vendor/symbolica/src/atom/core.rs:1412-1446`). It is not a replacement for a versioned rule artifact.

### 4.2 Symbol identity is global and order-sensitive

`Symbol` contains a numeric id and immutable attributes (`vendor/symbolica/src/atom.rs:633-645`). Equality, hashing and ordering ultimately depend on that id (`vendor/symbolica/src/atom.rs:647-722`). Registration is serialized through global state and assigns ids in insertion order (`vendor/symbolica/src/state.rs:658-697`). Therefore:

- never define integral or sector ordering through `Atom`/`Symbol` ordering;
- pre-register all fixed names in deterministic order before parallel work;
- persist names and a variable-order manifest, not raw ids;
- use a RustRed comparator for integral complexity and artifact sorting.

`SymbolBuilder` and its immutable-attribute checks are at `vendor/symbolica/src/atom.rs:758-1041`. Re-registering the same name with different attributes or callbacks is an error (`vendor/symbolica/src/state.rs:736-960`).

### 4.3 Native symbolic callbacks: restricted use only

Symbolica supports normalization, derivative, series and numerical-evaluation callbacks. Callback types are `Send + Sync` closures (`vendor/symbolica/src/atom.rs:265-430`), and builders attach them at `vendor/symbolica/src/atom.rs:837-928`.

Normalization callbacks are suitable only for a small set of fixed, pure, idempotent canonical heads whose semantics are independent of a family/session. They are **not** appropriate for dynamic IBP rules, guard evaluation, family-dependent scalar products, or a persistent rule database because:

- callback-bearing symbol definitions are process-global and effectively permanent;
- callback symbols are not exportable (`Symbol::is_exportable`, `vendor/symbolica/src/atom.rs:1450-1483`);
- exported/imported definitions lose callbacks and issue a warning (`vendor/symbolica/src/state.rs:1168-1173` and `vendor/symbolica/src/atom.rs:1701-1761`);
- callback redefinition conflicts with an existing symbol;
- normalization callbacks have no RustRed artifact version, family scope, proof provenance or guard result channel.

`EvaluationInfo` is numerical evaluation metadata, not a symbolic replacement engine (`vendor/symbolica/src/atom.rs:320-430`). Keep family rules in ordinary Rust objects.

### 4.4 Parsing

Use `try_parse!`/`Atom::parse` for checked user or artifact text and reserve `parse!` for trusted constants; `parse!` unwraps (`vendor/symbolica/src/atom.rs:4043-4197`). Parser settings and polynomial modes live at `vendor/symbolica/src/parser.rs:103-180`. Parsing takes a state write lock to register symbols, then normalizes outside the lock (`vendor/symbolica/src/parser.rs:557-577`), another reason to pre-register names.

The Mathematica parser is only a subset and excludes constructs including `.` and `->` (`vendor/symbolica/src/atom.rs:4092-4094`). Do not make Mathematica syntax the RustRed artifact language.

The fast expanded-polynomial parser uses unsafe byte assumptions, unchecked indexing, `expect` and `unwrap` (`vendor/symbolica/src/parser.rs:1800-2000`). It is only acceptable for trusted internal serialization with a separately validated envelope, never user input.

## 5. Pattern matching and replacements

### 5.1 Exact API surface

Pattern variants include literals, wildcards, functions, powers, products, sums, alternatives and transformers (`vendor/symbolica/src/id.rs:45-64`). A replacement is:

```rust
pub struct Replacement {
    pub pat: Pattern,
    pub rhs: ReplaceWith<'static>,
    pub conditions: Option<Condition<PatternRestriction>>,
    pub match_settings: MatchSettings,
}
```

as defined at `vendor/symbolica/src/id.rs:240-259`. Construction is `Replacement::new`, with `.when(...)`, `.partial(...)`, level limits, RHS cache size and non-greedy settings (`vendor/symbolica/src/id.rs:273-347`). Tree strategy is controlled by `ReplaceSettings::{once,bottom_up,nested}` (`vendor/symbolica/src/id.rs:390-421`).

The fluent `ReplaceBuilder` supports optional wildcards, `.when`, `.repeat`, `.once`, `.bottom_up`, `.nested`, `.try_with`, `.with_map` and iterator access (`vendor/symbolica/src/id.rs:423-720`). Prefer `try_with` because it detects RHS wildcards absent from the LHS unless explicitly allowed (`vendor/symbolica/src/id.rs:611-637`). `MatchMap` is a cloneable `Send + Sync` closure for computed RHS values (`vendor/symbolica/src/id.rs:179-190`).

Restrictions include arity/length, atom type, tags, literal wildcard, unary filter, two-match comparator and non-greedy behavior (`vendor/symbolica/src/id.rs:3610-3834`). Match and tree iterators are reusable primitives at:

- `AtomMatchIterator`: `vendor/symbolica/src/id.rs:5266-5335`;
- `AtomTreeIterator`: `vendor/symbolica/src/id.rs:6165-6243`;
- `PatternAtomTreeIterator`: `vendor/symbolica/src/id.rs:6246-6343`;
- `ReplaceIterator`: `vendor/symbolica/src/id.rs:6346-6540`.

For a whole-integral match, set `MatchSettings::default().partial(false)`; default matching is partial (`vendor/symbolica/src/id.rs:4509-4596`). Use single wildcards with exact arity for integral arguments, not sequence wildcards over an index vector.

`AtomCore::replace_multiple` tests replacements in the caller-supplied order
during one traversal (`vendor/symbolica/src/atom/core.rs:1856` onward).  It is
the appropriate simultaneous-substitution primitive for a checked momentum
map; it does not repeat the rule set to a fixed point.  Repetition must be an
explicit, bounded outer operation, and every parser must still validate the
complete matched head, arity, and typed arguments after matching.

### 5.2 Conditions are not a proof-grade guard language

`ConditionResult` is `True | False | Inconclusive` (`vendor/symbolica/src/id.rs:3916-3984`). Cross-wildcard restrictions can be inconclusive until the other wildcard is bound (`vendor/symbolica/src/id.rs:4098-4258`). During insertion into the match stack, only `False` rejects a candidate; `Inconclusive` is allowed to continue (`vendor/symbolica/src/id.rs:4765-4776`). The audited production paths did not show a mandatory final “condition must evaluate to True” gate.

Also, `Relation::Gt/Ge/Lt/Le` compares canonical Atom ordering, not mathematical numeric ordering (`vendor/symbolica/src/id.rs:4023-4095`, especially `4053-4061`). Consequently, never encode index positivity, sector membership, nonzero denominators or kinematic assumptions solely in Symbolica matcher conditions.

RustRed should use the matcher only to recognize an outer syntactic form, then extract typed values and run a RustRed `Guard` evaluator whose result policy is explicit:

- `True`: apply;
- `False`: reject;
- `Unknown`: reject, branch into a guarded result, or request a declared assumption; never silently apply.

The guard AST should at minimum represent integer domain/sign/range, index affine equalities, sector predicates, polynomial equality/nonzero, kinematic ideal membership, conjunction, disjunction and negation.

### 5.3 Source-level hazards

`ReplaceBuilder::with` builds extra restrictions for wildcard function heads, but the condition is attached only under `matches!(c, Condition::True)` (`vendor/symbolica/src/id.rs:573-590`). That branch appears inverted: when wildcard function names exist, `c` is no longer `True`, so the intended restriction is not attached. Do not rely on this automatic behavior; explicitly validate function heads after matching. This should be reported upstream, but RustRed should remain safe even if Symbolica later changes it.

Pattern RHS expansion contains an `unwrap` and TODO in `ReplaceIterator` (`vendor/symbolica/src/id.rs:6495-6512`). Dynamic rule application should return RustRed errors instead of exposing this panic surface.

Several `Transformer` paths are also unsuitable for proof-bearing rules:
`MapTerms`, `ForEach`, and some collect chains discard or unwrap child errors,
and `IfElse` sends both `False` and `Inconclusive` to the else branch
(`vendor/symbolica/src/transformer.rs:216` onward).  Use transformers for
cosmetic, regression-probed syntax normalization only.  `FunctionMap` and
`ExternalFunction` are numerical-evaluation facilities, not exact symbolic
rule stores; generated IBP rules remain typed Rust objects with explicit
guards and replay.

Audited usage examples/tests:

- `vendor/symbolica/examples/pattern_match.rs:4-45`;
- `vendor/symbolica/examples/pattern_restrictions.rs:4-67`;
- `vendor/symbolica/examples/replace_once.rs:4-16`;
- `vendor/symbolica/examples/replace_all.rs:4-9`;
- `vendor/symbolica/examples/tree_replace.rs:4-40`;
- `vendor/symbolica/tests/pattern_matching.rs:7-72`.

## 6. Polynomial variables and strict coefficient conversion

### 6.1 Variable kinds and maps

`PolyVariable` can be a `Symbol`, `Function`, `Power` or `Temporary` (`vendor/symbolica/src/poly.rs:723-748`). For the authoritative RustRed coefficient domain, allow only the exact declared kinds—normally plain `Symbol`s for `D`, masses, invariants and parametric indices. A surprise `Function`, `Power` or `Temporary` is a conversion error, not an extra independent parameter.

`IntoVariableMap` accepts `Arc<Vec<PolyVariable>>`, symbols, atoms, tuples, arrays, vectors and slices; `()`/`None` means the map is unknown (`vendor/symbolica/src/poly.rs:892-1064`). RustRed should always pass the canonical `Arc<Vec<PolyVariable>>` in production.

Atom conversion signatures are:

```rust
fn try_to_polynomial<R: EuclideanDomain + ConvertToRing, E: Exponent>(
    &self,
    field: &R,
    var_map: impl IntoVariableMap,
) -> Result<MultivariatePolynomial<R, E>, PolynomialConversionError>;

fn try_to_rational_polynomial<
    R: EuclideanDomain + ConvertToRing,
    RO: EuclideanDomain + PolynomialGCD<E>,
    E: PositiveExponent,
>(
    &self,
    field: &R,
    out_field: &RO,
    var_map: impl IntoVariableMap,
) -> Result<RationalPolynomial<RO, E>, PolynomialConversionError>;
```

from `vendor/symbolica/src/atom/core.rs:1135-1294`.

Despite accepting an explicit map, both conversion families may extend it when they see undeclared variables or non-polynomial pieces (`vendor/symbolica/src/atom/core.rs:1135-1231`; implementation at `vendor/symbolica/src/poly.rs:1306-1455`). There is also a source TODO that function coefficients are not checked for dependence on an existing variable (`vendor/symbolica/src/poly.rs:1413-1415`). Therefore “fallible” does not mean “strict”.

Required wrapper policy:

1. Pass the canonical map.
2. Check `result.get_vars_ref() == canonical_map.as_slice()` exactly, including order and length.
3. Check every entry has an allowed `PolyVariable` variant.
4. Reject any extension or dependency ambiguity.
5. Never use Atom automatic-indeterminate detection in the proof-bearing coefficient path.

### 6.2 Variable-map extension, renaming and remapping

Exact APIs:

- `get_vars() -> Arc<Vec<PolyVariable>>` and `get_vars_ref() -> &[PolyVariable]`: `vendor/symbolica/src/poly/polynomial.rs:578-586`;
- `rename_variable(&mut self, old, new)`: changes the name in the same slot, `vendor/symbolica/src/poly/polynomial.rs:588-595`;
- `unify_variables(&mut self, other: &mut Self)`: inherits `self` order, appends variables found only in `other`, and rewrites exponents, `vendor/symbolica/src/poly/polynomial.rs:597-677`;
- `unify_variables_list(&mut [Self])`: two-pass list unification, `vendor/symbolica/src/poly/polynomial.rs:679-691`;
- `RationalPolynomial::unify_variables`: unifies numerator/denominator maps and renormalizes both operands, `vendor/symbolica/src/domains/rational_polynomial.rs:155-184`;
- `reorder<ON>()`: changes monomial ordering only, **not variable order**, `vendor/symbolica/src/poly/polynomial.rs:1406-1429`.

There is no audited public “reorder variables to this arbitrary map” helper. For a deterministic remap, RustRed should validate a bijection, permute each dense exponent row, and reconstruct with:

```rust
MultivariatePolynomial::from_coefficient_list(
    coefficients,
    flattened_permuted_exponents,
    target_variables,
    &ring,
)
```

defined at `vendor/symbolica/src/poly/polynomial.rs:1711-1738`. Apply the same permutation independently to a rational polynomial's public `numerator` and `denominator`, then reconstruct with `FromNumeratorAndDenominator::from_num_den` if normalization is desired (`vendor/symbolica/src/domains/rational_polynomial.rs:61-68`).

Hazards:

- `from_coefficient_list` computes `exponents.len() / coefficients.len()` and therefore needs a RustRed precondition for an empty coefficient vector; validate dimensions before calling.
- `monomial` checks exponent length only with `debug_assert` (`vendor/symbolica/src/poly/polynomial.rs:427-443`).
- `condense` removes unused variables and changes the map (`vendor/symbolica/src/poly/polynomial.rs:1741-1776`); do not call it on persisted coefficients.
- ordinary polynomial/RP arithmetic automatically calls `unify_variables` when maps differ (`vendor/symbolica/src/domains/rational_polynomial.rs:980-1119`). Wrap arithmetic with a debug/release invariant that operands and results use the family map.

## 7. Exact rational-polynomial substitution

### 7.1 Relevant APIs

For `MultivariatePolynomial<F,E>`:

- `replace(n, &F::Element)` substitutes one variable by a coefficient-ring element but retains the map slot (`vendor/symbolica/src/poly/polynomial.rs:1778-1803`);
- `replace_with_poly(n, &Self)` substitutes by a polynomial and requires identical maps (`1937-1962`);
- `shift_var(n, &F::Element)` implements `x_n -> x_n + a` through a
  degree-triangular Horner update (`2007-2033`);
- `evaluate_with_coeff_map(map_coeff, point, ring)` maps coefficients into a target ring and asserts `point.len() == nvars` (`1890-1913`);
- `replace_all(&[F::Element])` silently zips the supplied points with exponents and does **not** assert the length (`1915-1935`);
- `replace_except` is an interpolation-oriented partial evaluation helper (`1964-2005`).

These low-level APIs have caller preconditions which RustRed must enforce:
`from_coefficient_list` divides by the list length and therefore fails on an
empty list (`polynomial.rs:1712`); `replace_all` silently accepts a substitution
slice shorter than the variable map (`:1917`); and `rearrange` does not prove
that its mapping is a bounded bijection (`:2301`).  Production wrappers must
check nonemptiness, exact arity, range, injectivity, and surjectivity before
calling them.

For `RationalPolynomial<R,E>`:

- numerator and denominator are public (`vendor/symbolica/src/domains/rational_polynomial.rs:90-95`);
- `evaluate(&[R::Element])` uses `replace_all` and divides directly (`709-715`);
- `evaluate_with_coeff_map(..., point, &target_field)` evaluates both parts and divides directly (`718-733`);
- `RationalPolynomialField::try_div` returns `None` when the divisor's numerator is zero (`vendor/symbolica/src/domains/rational_polynomial.rs:910-919`), while `div`/`inv` otherwise expose panic-prone field behavior.

Never use RP `evaluate` for rule specialization. It has no denominator-zero `Result`, and its underlying `replace_all` accepts a short point silently.

### 7.2 Required guarded partial-substitution algorithm

There is no audited single-call partial substitution of an RP by other RPs that preserves exceptional-locus information. Implement it in RustRed as follows:

1. Validate the source RP uses the expected source variable map.
2. Build a target polynomial zero with the canonical target map using `MultivariatePolynomial::new(&Z, None, target_map.clone())` (`vendor/symbolica/src/poly/polynomial.rs:330-343`).
3. Build identity images with `base.variable(&target_var)` (`445-455`) and lift them to RP using `RationalPolynomial::from(poly)` (`vendor/symbolica/src/domains/rational_polynomial.rs:144-152`).
4. Build constants with `base.constant(integer).into()` so they retain the canonical target map (`vendor/symbolica/src/poly/polynomial.rs:398-413`). Do **not** use `RationalPolynomialField::nth` for this purpose: it starts from an empty variable map (`vendor/symbolica/src/domains/rational_polynomial.rs:847-872`).
5. Construct a full point vector, one RP image per source variable. Unsubstituted variables map to their identity RPs. Validate exact length and every image's target map.
6. Add every image denominator to the RustRed nonzero guard set before any cancellation.
7. Let `target_field = RationalPolynomialField::new(Z)` (`vendor/symbolica/src/domains/rational_polynomial.rs:38-58`). Evaluate source numerator and denominator separately with `MultivariatePolynomial::evaluate_with_coeff_map`, mapping each source integer coefficient to `base.constant(c.clone()).into()`.
8. Call the results `mapped_num` and `mapped_den`. Add `mapped_den.numerator != 0` to the guard set. This is the original source denominator after specialization, modulo the already-recorded image-denominator assumptions.
9. Use the field's checked `try_div(&mapped_num, &mapped_den)` and turn `None` into an unsatisfied/contradictory guard result. Normalize the returned RP, but retain the pre-cancellation guard polynomials separately.
10. Canonically remap every guard polynomial and result back to the target map, verify maps exactly, and return `Result<GuardedCoeff, CoeffError>`.

This supports both concrete index specialization (constant images) and fully parametric affine substitutions (RP/polynomial images). It also makes the domain of a rule explicit rather than pretending fraction-field equality holds at cancelled poles.

**Compile probe RP-SUB-1:** confirm imports and inference for `RationalPolynomialField::<IntegerRing, u16>::new(Z)`, `RationalPolynomial::<IntegerRing,u16>::from(base.variable(...)?)`, and the closure passed to `evaluate_with_coeff_map`. The source proves the generic operations exist, but no vendored example exercises this exact “polynomial over `Z` evaluated into the RP field over `Z`” composition.

**Compile probe RP-SUB-2:** confirm whether `Ring::try_div` is directly in method resolution with the planned imports or needs fully qualified syntax. Do not replace it with `/` merely to satisfy inference.

### 7.3 Exact integer translations used by grouped reduction

For the special map `n_i -> n_i + a_i`, sequential substitutions at distinct
index positions commute: each image contains only its own index variable and
an integer constant. This is not true for general affine maps which mix index
variables; those require RustRed's simultaneous affine-composition path.

Keep `replace_with_poly` as the production primitive for now. RustRed's
translation preflight counts the support expansion and power calls made by
that path. Although `shift_var` is a valuable independent test oracle, its
workload is different: for degree `d` it owns `d+1` coefficient polynomials
and performs `d(d+1)/2` Horner updates. Substituting it under the existing
certificate would make the resource census false (one sparse `n^65535`
monomial is the sharp counterexample). Benchmark and certify it separately
before considering it as an optimization.

`Integer` has an unusually important representation invariant. Its public
`Single`, `Double`, and `Large` variants derive representation-sensitive
`Eq`/`Hash`, while `Ord` compares numeric values. Moreover,
`Integer::is_zero`, `Integer::is_one`, and `IntegerRing::{is_zero,is_one}`
recognize only canonical `Single(0)`/`Single(1)`
(`vendor/symbolica/lib/numerica/src/domains/integer.rs:81-92,829-841,2008-2027,2187-2201`).
A raw `Double(0)` or `Large(0)` passed to `Polynomial::constant` can therefore
be retained as a malformed one-term zero polynomial. Every exact RustRed
boundary must inspect values numerically and canonicalize before polynomial
construction, hashing, or provenance retention. `Integer::from(i128)` and
`Integer::from(MultiPrecisionInteger)` downcast canonical small values; for a
borrowed genuine `Large`, arithmetic with canonical zero constructs a
right-sized canonical result without inheriting an adversarial spare GMP
capacity.

The exact translation implementation must complete all term, exponent,
power, integer-bit, and retained-output preflights before cloning a GMP
offset. It must also skip a nonzero offset when the source polynomial does not
contain that index: such an offset contributes no coefficient growth and is
therefore deliberately absent from the preflight. Canonicalization, constant
construction, replacement-polynomial addition, and `replace_with_poly` belong
inside the checked backend panic boundary. Diagnostics report only shift
arity, never the possibly enormous/private vector.

For validation, use `shift_var` only as a differential implementation and add
a second exact point-evaluation oracle. `replace_all` is suitable in a test
only after explicitly checking `point.len() == nvars`, because it otherwise
silently zips a short point. A degree-complete grid proves the translated
polynomial identity independently of the production replacement sequence.

## 8. Rational-polynomial normalization and unsafe alternatives

`RationalPolynomial::from_num_den` supports coefficient-domain-specific normalization. For integer polynomials it can compute a polynomial GCD, cancel, and force a positive denominator leading coefficient (`vendor/symbolica/src/domains/rational_polynomial.rs:406-443`). Arithmetic also performs cross-GCD cancellation (`vendor/symbolica/src/domains/rational_polynomial.rs:980-1119`). This is desirable for coefficient size but reinforces the need for a separate guard set.

`from_num_den` itself does not reject a zero denominator before inspecting its
leading coefficient.  RustRed's validated constructor must keep its explicit
zero-denominator check before entering Symbolica.

`inv` panics on zero and `pow` uses repeated multiplication with a TODO for
binary exponentiation
(`vendor/symbolica/src/domains/rational_polynomial.rs:524-564` and `874-891`).
RustRed's checked coefficient-power boundary authenticates the map, exponent,
degree/term envelope, operation allowance, and retained bytes before calling
the public `RationalPolynomialField::pow`, then reauthenticates the output. It
records the current linear native schedule rather than implementing its own
power algorithm. Symbolica still exposes no cancellation or internal-scratch
budget for that call.

Do not use `FactorizedRationalPolynomial` as the authoritative coefficient/guard representation:

- `InternalOrdering` is `todo!()` and will panic (`vendor/symbolica/src/domains/factorized_rational_polynomial.rs:79-83`);
- its variable unification is asymmetric and incomplete-looking (`98-108`);
- duplicate factor fusion has a TODO (`440-543`, especially `497`);
- inversion/evaluation can panic or divide without a checked result (`714-737`, `823-868`).

If factorization is used to optimize guard evaluation, store the expanded polynomial as truth and verify that the multiplied factors with powers reproduce it exactly.

### 8.1 Verified GMP boundary and native projective associates

RustRed's supported integer backend is Symbolica's default `gmp` feature. GMP
is mandatory; the alternative `no_gmp` feature is unsupported and must not be
exposed as a RustRed build mode. `Integer` publicly distinguishes
`Single(i64)`, `Double(i128)` and `Large(MultiPrecisionInteger)`
(`vendor/symbolica/lib/numerica/src/domains/integer.rs:81-94`). Exact
coefficient arithmetic, including arbitrary-precision multiplication and
collection, stays inside Symbolica. RustRed does not export magnitudes, pack
private limbs, or maintain a second integer-arithmetic engine.

The polynomial-associate relation is strict association over
`K = Q(theta)`. For nonzero `P,Q in Z[theta,n]`,

```text
P ~ Q  iff  P = u Q for some nonzero u in Q(theta).
```

Writing the inputs as `P = sum_a P_a(theta) n^a` and
`Q = sum_a Q_a(theta) n^a`, equal index support is necessary. After choosing
a deterministic nonzero anchor `0`, the exact projective criterion is

```text
P_a Q_0 = Q_a P_0 for every index-monomial group a.
```

This proves association by a base-field unit. It is deliberately stricter
than equality of radicals or vanishing sets: for example, `p` and `p^2` are
not associates. Zero has no projective class, so either zero input returns
`false` before any native projection or multiplication. At every integer
boundary, numerical comparison with canonical zero is required; the
representation-sensitive Symbolica zero predicates do not by themselves
reject noncanonical `Double(0)` or `Large(0)` coefficients.

The implemented algebra route uses public Symbolica APIs end to end:

1. Authenticate both context fingerprints, complete ordered variable maps,
   sparse shapes, canonical monomial order, exponent ranges, and integer
   payloads.
2. Widen every authenticated exponent from `u16` to `u32` with
   `MultivariatePolynomial::map_exp`. Convert the widened polynomial through
   `RationalPolynomial::from`; widening is required because a native cross
   product can contain a base exponent as large as `2 * u16::MAX`.
3. Call `RationalPolynomial::to_polynomial(index_variables, true)` to obtain a
   polynomial in the index variables over `Q(theta)`. The `true` denominator
   mode is safe here only because the input was constructed from an
   authenticated polynomial and therefore has denominator one.
4. Authenticate the returned outer/index map, every coefficient's ordered
   base map and unit denominator, source-term conservation, canonical support,
   exponent bounds, and integer payload before using the projection.
5. Compare index support, choose the exact minimum-cost anchor, and form each
   pair of projective cross products with `RationalPolynomialField::mul`.
   Authenticate each native product against its prospective output envelope,
   then use exact Symbolica equality.

RustRed owns only the associate semantics, deterministic support/anchor
routing, admission, authentication, panic containment, provenance, and
transactional census propagation. Symbolica owns projection, polynomial
multiplication, integer arithmetic, collection, and equality. No private
cross-product implementation is retained as a production or test oracle.

Resource staging is part of the boundary contract. Before `map_exp`,
`to_polynomial`, or `mul`, RustRed admits validation payloads, widened and
projected exponent storage, actual GMP capacities, projection grouping and
sorting work, native cross-term and integer-bit work, native dense/heap
dispatch workspace, output envelopes, and RustRed-visible temporary storage.
After each native call it reauthenticates maps, denominators, term counts,
exponents, integer bits, and canonical ordering against those bounds.
Resource failures retain exact `resource`, `requested`, and `limit`
attribution. Outer condition compilation passes only the remaining allowance
to each child and transactionally accumulates the new projection/native
counters; the obsolete magnitude-copy, limb-operation, and product/accumulator
scratch counters no longer exist.

## 9. GCD, factorization, Groebner bases, finite fields and reconstruction

### 9.1 GCD and factorization

`PolynomialGCD` is the public trait (`vendor/symbolica/src/poly/gcd.rs:3396-3419`). The implementation uses randomized sampling in several paths; sampled degree bounds can be unlucky (`vendor/symbolica/src/poly/gcd.rs:186-250`). Reconstructed GCD candidates are checked for exact divisibility before return in an important path (`3360-3384`), but RustRed should still verify any proof-critical factor/GCD-derived transformation.

`Factorize` is defined at `vendor/symbolica/src/poly/factor.rs:1-29,60-69` and also uses randomized algorithms. Use factorization as an accelerator/pretty-printer, not the sole persisted evidence for a nonzero guard.

### 9.2 Groebner bases

`GroebnerBasis::new` unifies variables, runs F4 and reduces (`vendor/symbolica/src/poly/groebner.rs:117-148`). Reduction and verification APIs are:

- reduction by a basis: `vendor/symbolica/src/poly/groebner.rs:423-481`;
- `reduce_basis(self) -> Self`: `544-586`;
- `is_groebner_basis(system: &[...]) -> bool`: `588-627`.

The vendored idiom is `vendor/symbolica/examples/groebner_basis.rs:8-37`.

Appropriate uses are kinematic quotient-ring reduction, ideal membership for declared invariant relations, and algebraic components of zero/scaleless-sector checks. It is not a replacement for Laporta/parametric-IBP elimination because it knows no integral ordering, sector condition, pivot guard or rule provenance.

For persisted claims, retain original ideal generators, verify `is_groebner_basis`, and exact-reduce claimed relations. `Echelonize` contains a TypeId-specialized unsafe finite-field path (`vendor/symbolica/src/poly/groebner.rs:630-679`), so feature/version changes require regression tests.

**Compile probe GB-1:** instantiate the precise coefficient/exponent/order combination RustRed will use, including imports for `GrevLexOrder`, `LexOrder`, `Echelonize` and `GroebnerBasis`. The generic bounds differ by coefficient domain.

### 9.3 Finite fields and reconstruction

`FiniteField::new` marks a modulus as prime but does not perform a primality test; `new_non_prime` is also public (`vendor/symbolica/lib/numerica/src/domains/finite_field.rs:168-201`). Select primes from Symbolica's prime iterator rather than arbitrary user input (`finite_field.rs:3061` onward). Convert through `to_element`/`from_element`; the stored values are Montgomery representations (`246-270`).

CRT is available via `Integer::chinese_remainder` (`vendor/symbolica/lib/numerica/src/domains/integer.rs:1694-1723`). Scalar rational reconstruction is at `vendor/symbolica/lib/numerica/src/domains/rational.rs:1120-1196`, with an example at `vendor/symbolica/examples/rational_reconstruction.rs:3-17`.

Newton interpolation is public at `vendor/symbolica/src/poly/gcd.rs:341-383`; its caller must validate nonempty equal-length point/value lists, a valid interpolation-variable index, and distinct points with invertible pairwise differences.

No public, complete multivariate rational-function reconstruction API was found. RustRed must implement sampling/support/degree management, bad-prime and bad-point rejection, CRT accumulation, multivariate rational reconstruction, and exact replay. Every reconstructed parametric identity must be replayed over exact integer/RP coefficients; fresh finite-field checks are secondary evidence only.

## 10. Dense and sparse linear algebra

### 10.1 Dense matrices

The exact dense matrix type is in `vendor/symbolica/lib/numerica/src/tensors/matrix.rs`. Useful APIs include:

- fraction-free row reduction/solve over suitable exact domains: `311-466`;
- `Matrix` and constructors: `707-808`;
- Bareiss determinant: `1041-1124`;
- field inversion, row reduction, exact solve, `solve_any` and rank: `1474-1756`.

`solve_any` sets free variables to zero (`1721-1745`), which is a policy choice and normally wrong for discovering a general reduction basis unless RustRed has explicitly selected dependent columns. A working RP matrix construction/solve idiom is in `vendor/symbolica/examples/solve_linear_system.rs:20-64`.

Do not call bare dense `Matrix::inv` as a correctness oracle. Its general path,
used for size one and sizes four and larger, row-reduces every column of the
augmented matrix `[A|I]`; the appended identity can make the augmented rows
independent even when `A` itself is singular, so singularity may be missed
(`vendor/symbolica/lib/numerica/src/tensors/matrix.rs:1557-1579`). The
implemented generic-family boundary calls the independent native `Matrix::det`
first, rejects a zero determinant, and retains the determinant numerator as
the family-domain nonzero condition before calling `inv`. Generic parametric
elimination remains a separate guard/provenance problem and still has to retain
its selected pivot conditions.

The Atom-level `solve_linear_system`/`system_to_matrix` convenience API (`vendor/symbolica/src/atom/core.rs:757-835`) auto-extracts symbolic coefficients. It does not provide integral ordering, guard-aware pivots or proof provenance, so it should be confined to diagnostics.

For automatic ISP completion, RustRed now copies each authenticated nonempty
rectangular coefficient matrix into the checked contextual field and calls
public `Matrix::partial_row_reduce` over all columns.  This destructive entry
point avoids the extra clone in `Matrix::rank`; RustRed retains only the
LiteRed identity-row scan, coordinate order, resource policy, V2 work census,
and replay certificate.  A test-only maximal-minor oracle delegates every
determinant to public `Matrix::det` and independently verifies one- and
two-loop examples with external momenta.

Two additional native edge cases are relevant when selecting matrix APIs.
`Matrix::rank`/`partial_row_reduce` indexes row zero for a `0 x N` input, so the
checked RustRed boundary rejects empty matrices before entry.  Also, the
single-`u32` row-index implementation in the vendored dense matrix multiplies
by `nrows` instead of `ncols`; RustRed uses tuple indexing and iterators and
does not rely on that rectangular row-slice path.

#### 10.1.1 Checked contextual field for coefficient matrices and powers

The generic-family, automatic-ISP-rank, and tensor-projector P1 slices use
`src/symbolica_coefficient_matrix.rs`. `CheckedCoefficientField` supplies
Symbolica's public `Set`, `RingOps`, `Ring`, `EuclideanDomain`, and `Field`
traits with RustRed's existing rational-polynomial `Coefficient` element and
ordered `CoefficientContext`. Matrix-used scalar operations forward to the
context's checked Symbolica arithmetic; constants are constructed on the
context's map. Symbolica itself performs rank reduction, determinant, inverse,
both native products, and coefficient powers. There is no private determinant,
inversion, elimination, matrix multiplication, coefficient-power,
rational-function, or integer implementation in this boundary.

This adapter is required because no closer public Symbolica 2.2.0 abstraction
satisfies the boundary contract:

- `RationalPolynomialField<R,E>` stores `R` but no variable map, and its
  `zero`, `one`, and `nth` make empty-map elements
  (`vendor/symbolica/src/domains/rational_polynomial.rs:38-58,847-872`). Its
  ordinary arithmetic may unify mismatched element maps instead of rejecting
  them.
- `RingOps` and `Field::{div,inv}` are infallible trait methods, and
  `Ring::{try_inv,try_div}` return only `Option`
  (`vendor/symbolica/lib/numerica/src/domains.rs:111-182,250-254`). Native
  matrix code therefore has no public result channel for RustRed's typed
  context and resource failures.
- `FactorizedRationalPolynomialField` changes the element representation; its
  element ordering remains `todo!()`, and its field `is_one` does not check
  `numer_coeff`
  (`vendor/symbolica/src/domains/factorized_rational_polynomial.rs:79-83,1041-1043`).
  `AtomField` likewise loses the strict declared polynomial map and bounded
  coefficient contract.

The adapter is deliberately narrow: RustRed owns admission, authentication,
failure transport, and replay, while all algebra stays in Symbolica. It admits
an exact scalar-operation bound, individual and simultaneously live
matrix-entry bounds, clone-owned retained bytes for all authenticated inputs,
and aggregate clone-owned retained bytes for the determinant, inverse, and
both verification products. Each native output is authenticated against the
ordered context and exact coefficient limits before use. Symbolica exposes no
complete public bound for its temporary polynomial GCD, quotient, or dense
multiplication scratch; retained-byte accounting is therefore explicit about
that native-scratch limitation and is not presented as a total-memory proof.

Two postconditions compensate for audited defects without replacing the native
algorithms. The determinant-first singularity guard precedes every inverse.
Then `A A^-1` and `A^-1 A` are computed with Symbolica multiplication and
checked entry by entry for exact diagonal ones and off-diagonal zeros. The
entrywise check is necessary because Symbolica's `Matrix` `SelfRing::is_one`
accepts a zero diagonal entry
(`vendor/symbolica/lib/numerica/src/tensors/matrix.rs:1128-1134`).

Typed checked-scalar failures cross Symbolica's infallible field methods as a
private unwind payload caught and downcast at the immediately enclosing native
call. Consequently this boundary requires `panic = "unwind"`; the module emits
a compile error for `panic = "abort"`. The adapter's `Ring::sample` method is
also why RustRed directly depends on `rand` 0.9: its signature must use
Symbolica's `rand::RngCore` version, and the implementation delegates to `Z`.

The production integrations are topology- and loop-count-generic. The
generic-family black-box test oracle does not
read the cached production inverse: it solves every `A x=e_j` independently
with Symbolica's public `Matrix::solve`, then replays both products. Concrete
analytic inverses in `tests/parametric_ibp_oracle.rs` are independently checked,
test-only fixtures, including perturbation rejection. They validate one-,
two-, three-, and five-loop examples; they neither define production
recurrences nor add topology/loop dispatch to family construction or
parametric IBP generation.

Parallel, licensed GMP validation for this milestone recorded:

- 17/17 focused adapter tests in
  `111ef62d-3de6-4957-b6b4-b2e04820375f`;
- 42/42 combined debug tests in
  `3197a8d5-70a9-49de-9d78-5415374f46bc`;
- 71/71 downstream tests in
  `1820a2af-baa8-4f82-a103-b70b81c52b4d`;
- 42/42 combined release tests in
  `dfe1042d-2509-4bad-87b6-87df1281cf6c`;
- final hardening reruns of 43/43 debug tests in
  `d1519c8d-05c4-44e3-aefc-88ea68be936f`, 35/35 selected release library tests in
  `053604e4-0ec6-4dad-9c4f-90aee31af8c2`, and 8/8 release independent-oracle
  tests in `99fff53c-0fbf-46a9-9fc7-b63c8bd9795b`;
- the complete optimized default-feature library suite passed 969/969 tests
  under `cargo nextest run --release --lib -j4 --no-fail-fast`, with four
  workers and no failures; and
- a passing `cargo check --all-features --all-targets -j4`.

The generic-family coefficient-matrix, automatic-ISP rank, and tensor-projector
P1 rows are complete at this checkpoint. The automatic-ISP slice passed 23/23 adapter
tests (`9b7572e7-ef39-4e21-bd15-5165c714985b`), 30/30 combined internal tests
(`b7a94288-7f4c-470c-ae60-461e633c5fe0`), 8/8 public completion tests
(`5699bda7-d1a3-480b-803e-0ab0dbcf7c30`), and 3/3 independent maximal-minor
oracle tests (`2df0c433-3b73-4101-8ef2-7726bda190ac`) under parallel licensed
nextest.  The frozen optimized gate then passed 30/30 adapter/internal tests in
`88208064-7cd3-46b4-b4f5-807953c2232f` and 13/13 public/oracle/downstream tests
in `0a0f4f11-09b0-4d0e-a9b8-f9adad877989`; the latter includes the existing
four- and five-loop factorized reductions. The tensor-projector oracle solves
all 15 rank-six inverse columns independently with `Matrix::solve` and passed
in both debug and release. Optimized parallel run
`fb9a0cda-1b7a-4494-8032-d7cbc8ea1422` then passed 28/28 selected tensor,
closure, and Vakint-oracle tests. The all-feature/all-target compile check
passed as well. Symmetry matrices, symmetry-discovery determinants, and
Feynman-polynomial matrix consumers remain separate migration work.

### 10.2 Sparse matrices and row reduction

CSR `SparseMatrix` is defined at `vendor/symbolica/lib/numerica/src/tensors/sparse.rs:230-249`; constructors are at `401-529`. `from_csr` mostly asserts shape lengths, and `from_triplets` relies on a debug assertion for ordering (`495-529`). RustRed must sort triplets, combine duplicates, drop zeros and bounds-check before construction.

Sparse solve/determinant/inversion and parallel solve are at `972-1124`. Incremental `SparseRowReducer` and `LuLMode::{Full,Pattern,None}` are at `1454-1775`; back substitution starts at `1811`. The reducer chooses the first nonzero/smallest stored column as pivot and normalizes by it (`1855` onward). Thus RustRed can encode a fixed integral complexity priority by column order, but not a dynamic coefficient-size/guard-aware pivot score.

`SparseMatrix::inv` has the same augmented-row singularity problem as the
dense inverse (`vendor/symbolica/lib/numerica/src/tensors/sparse.rs:1051-1075`).
It is not a proof oracle.

Important source concern: checked inconsistency detection in sparse row-reducer constructors tests a one-entry final-column row only when that stored coefficient `is_zero` (`vendor/symbolica/lib/numerica/src/tensors/sparse.rs:1551-1577` and `1585-1615`). A valid CSR matrix normally omits zeros, so this appears inverted. `SparseMatrix::solve` and its parallel variant depend on these paths (`974-995`, `1084-1110`). Do not use them as a correctness oracle until a focused probe/test resolves this.

Even after that issue is resolved, direct RP-field elimination normalizes by a pivot and therefore loses the assumption that the pivot was nonzero. RustRed needs its own elimination wrapper/layer that records the pivot numerator/nonzero condition before division, carries row/equation provenance, enforces sector applicability and exact-replays emitted rules. Symbolica sparse storage/reduction may still be used for finite-field rank/elimination and carefully wrapped exact systems.

**Compile probe LA-1:** build a tiny exact RP matrix through both dense and sparse APIs with the intended aliases and solve it.

**Behavior probe LA-2:** construct consistent, underdetermined and inconsistent sparse systems, including a `[0 ... 0 | c]` row, to confirm or refute the inverted check before any production use.

**Behavior probe LA-3:** confirm parallel back-substitution output order; the implementation may permute output rows (`vendor/symbolica/lib/numerica/src/tensors/sparse.rs:2393-2498`). RustRed artifacts must be sorted semantically regardless.

## 11. Tensor support and the Vakint boundary

`CanonicalTensor` is reexported by `vendor/symbolica/src/tensors.rs:1-31`. `AtomCore::canonize_tensors` enters it at `vendor/symbolica/src/atom/core.rs:1499-1539`.

What it does:

- relabels repeated/dummy indices and returns external/dummy lists (`vendor/symbolica/src/tensors.rs:91-178`);
- strips scalar/non-index factors and canonicalizes a tensor graph (`180-289`);
- reconstructs with symmetric, antisymmetric and cycle-symmetric attributes (`379-503`);
- accepts functions, products and sums, detects repeated-contraction errors, and supports positive integer tensor powers (`515-705`).

Audited tests cover open indices, nested sums, index groups and symmetry attributes (`vendor/symbolica/src/tensors.rs:716-854`).

What it does **not** implement:

- Lorentz metric trace `g(mu,mu)=D`;
- metric-vector and vector-vector contractions;
- bilinear scalar products and loop-momentum routing;
- D-dimensional tensor-integral decomposition/projectors;
- symmetric tensor bases, Gram solves or exceptional Gram guards;
- conversion of tensor numerators into propagator/ISP powers;
- Vakint/FORM tensor-numerator reduction.

RustRed must implement a typed Lorentz layer with index space/dimension, variance if needed, metric and vector contraction, symmetric tensor basis generation, exact projector systems and Gram nonzero guards. Its output is scalar products/ISP shifts, after which parametric IBP rules apply. Symbolica's tensor canonicalizer is optional final canonical naming/network equivalence, not the physics reducer.

**Compile/behavior probe TEN-1:** test the exact head/index encoding RustRed plans to pass into `canonize_tensors`, especially separate Lorentz spaces/dimensions and scalar-tagged functions. Do not infer contraction physics from successful canonicalization.

## 12. Serialization and persistence

### 12.1 Symbolica state and Atom transport

`State::export` and `State::import` are at `vendor/symbolica/src/state.rs:1062-1282`. Import merges symbols, finite fields and interned variable maps and returns a `StateMap` for id remapping. Callback-bearing symbols cannot be exported and must be re-registered before import (`vendor/symbolica/src/state.rs:705-710,1168-1173`).

`AtomView::export` writes the entire State, a term count and expression bytes (`vendor/symbolica/src/atom/representation.rs:1959-1971`). `Atom::import(source, conflict_fn)` imports/merges State then renames with the returned map (`564-596`). `Atom::import_with_map(source, &StateMap)` reads a stateless expression under an already imported map (`598-606`). `AtomView::write` writes only expression bytes and requires the state to be handled separately (`1973-1983`).

Import/export behavior and conflict remapping are exercised at `vendor/symbolica/tests/import_export.rs:6-63`. `State::reset()` is unsafe and invalidates all existing atoms (`vendor/symbolica/src/state.rs:584-606`); it is a test-only operation and must never appear in RustRed production code.

With the `bincode` feature, multivariate polynomials derive state-aware encoding/decoding (`vendor/symbolica/src/poly/polynomial.rs:286-327`) and `PolyVariable` participates in state mapping (`vendor/symbolica/src/poly.rs:723-737`).

Those raw readers are not a bounded artifact envelope.  `State::import` reads
unbounded symbol, finite-field, and variable-map counts and recreates fields
through unchecked constructors (`vendor/symbolica/src/state.rs:1149-1240`);
Atom reading trusts a `u64` byte length before resizing and indexing
(`vendor/symbolica/src/atom/representation.rs:530` onward); polynomial bincode
decoding reconstructs coefficient/exponent/map payloads without RustRed's
post-validation.  `RationalPolynomial` does not itself supply the required
state-aware bincode artifact.  RustRed must frame all of these behind its own
count/byte limits and validated numerator/denominator wrapper.

### 12.2 Required RustRed artifact schema

Do not serialize raw `Replacement`, `Condition` or closure-bearing matcher objects. Store a RustRed schema containing at least:

- schema version and Symbolica version/features;
- family canonical hash and source-definition hash;
- ordered variable names, kinds and coefficient exponent type;
- typed integral LHS and affine index shifts;
- coefficient numerator/denominator sparse terms in the declared map;
- explicit serializable Guard AST and pre-cancellation denominator/pivot factors;
- sector/symmetry applicability and provenance;
- generator equation ids/hashes and elimination provenance;
- reconstruction primes/points/support metadata where applicable;
- exact-replay status and proof hashes.

On load: initialize and pre-register fixed callback symbols, import the Symbolica State once, obtain a `StateMap`, decode/remap objects, then verify the RustRed manifest, variable order, family hash, guards and exact identities. Binary format acceptance alone is not artifact validity.

**Compile probe SER-1:** enable `bincode` explicitly and round-trip the precise `MultivariatePolynomial<Z,E>`/RP wrapper with `StateMap` context. Symbolica's Cargo features show support, but the final RustRed dependency feature set must be proven.

## 13. Threading, licensing and process safety

This is deployment-critical.

`LicenseManager::new` initializes on the first Symbolica use (`vendor/symbolica/src/lib.rs:425-520`). In unlicensed mode it attempts to install a global Rayon pool with one thread and uses `unwrap` (`478-481`), which can panic if another component already initialized the global pool. Licensing may read environment/key data, spawn a network check and a watchdog (`522-600`).

Most importantly, in unlicensed mode a Symbolica call from a thread different from the initializing thread aborts the process (`vendor/symbolica/src/lib.rs:678-700`). `set_license_key` is legal only before any Symbolica call (`703-710`). `is_licensed` is at `755-758`.

Required startup order:

1. Configure/set the license before any Atom, parser, state, polynomial or Symbolica domain call.
2. Initialize Symbolica and verify the intended license mode.
3. Deterministically register symbols.
4. Only then initialize RustRed/Rayon worker pools and dispatch parallel work.

This is still an implementation gap: the test runner checks the environment
license, but the library does not yet expose one deterministic licensed startup
and symbol-registration boundary.  In particular, tensor/Vakint symbol helpers
must validate the attributes of an already registered name rather than accept
test-order-dependent plain symbols.

Unlicensed mode is not a supported multithreaded RustRed deployment. It must either run a deliberately single-threaded validation path on the initializing thread or fail early with a clear diagnostic; never risk a process abort in the middle of a reduction.

State registration is globally locked, while workspaces are thread-local. Symbols and callback closures are designed for cross-thread use once initialized, but RustRed must not mutate the registry during parallel reduction.

**Behavior probe THR-1:** in a disposable helper process, test licensed startup before and after Rayon initialization and test multi-threaded Atom/polynomial operations. Do not run the unlicensed cross-thread abort case inside the test runner process.

## 14. Streaming and performance

`TermStreamer` defaults to four cores, the current directory and a 1 GB in-memory limit (`vendor/symbolica/src/streaming.rs:63-107`). It splits additions into terms, spills sorted chunks and merges them (`287-539`), can map in parallel (`579-627`), and falls back to one core when unlicensed (`649-729`). Export/import includes State and term count (`332-390`). Examples/tests are `vendor/symbolica/examples/streaming.rs:4-21` and `vendor/symbolica/src/streaming.rs:747-867`.

Hazards: temporary filenames use path/pid/counter; `Drop` deletes files with `unwrap`, a crash can leave files, and a missing temp file can panic during drop (`vendor/symbolica/src/streaming.rs:142-257`). `to_expression` can defeat out-of-core operation by materializing the full sum (`541-560`).

Term streaming is useful only for huge expanded Atom sums at an I/O or algebraic-normalization boundary. It is not an IBP equation store: integral keys, guards, coefficients and provenance need a typed sparse external-sort/checkpoint format. If RustRed wraps it, allocate a uniquely owned temporary directory, record recovery metadata, handle cleanup outside the streamer's panic-prone default, and never call `to_expression` on an unbounded reduction.

Performance rules for the first implementation:

- keep integral equations typed and sparse; avoid Atom round-trips per term;
- keep coefficients in a single canonical RP map;
- use `Workspace`/`RecycledAtom` only around unavoidable Atom transforms;
- batch Atom sums with `Atom::add_many`;
- order sparse columns once by RustRed integral complexity;
- modularize large solves, but exact-replay all emitted rules;
- cache guarded coefficient substitutions by `(rule id, index image, assumption context)`.

## 15. API suitability matrix

| Need | Symbolica API | RustRed policy |
|---|---|---|
| Symbolic syntax/I/O | `Atom`, `AtomView`, typed builders, checked parser | Use at boundaries; typed structures internally |
| Integral rule matching | Pattern iterators/replacements | Outer structural dispatch only; typed matching and guards own correctness |
| Coefficients | `MultivariatePolynomial`, `RationalPolynomial<Z,E>` | Primary exact backend under one validated map |
| Generic-family coefficient matrices | `Matrix<CheckedCoefficientField>` over rational-polynomial elements | Completed P1 slice: native determinant, inverse, and products behind contextual admission, determinant guard, authentication, and entrywise two-sided replay |
| Partial RP substitution | polynomial `evaluate_with_coeff_map` into `RationalPolynomialField` | Wrap with full point length, map checks and explicit denominator guards |
| Variable remap | `unify_variables`, `rename_variable`, `from_coefficient_list` | Never accept implicit extension; explicit checked exponent permutation |
| Factorized denominators | `FactorizedRationalPolynomial` | Optimization only; not canonical/persistent truth |
| Kinematic ideals | `GroebnerBasis` | Useful with retained generators and exact verification |
| IBP elimination | dense/sparse matrices/reducers | Storage/accelerator only; RustRed owns ordering, pivots, guards and provenance |
| Finite-field reconstruction | finite fields, CRT, scalar rational reconstruction, Newton interpolation | Primitives only; RustRed owns multivariate rational reconstruction and exact replay |
| Tensor numerator reduction | `CanonicalTensor` | Optional dummy-index/network canonicalization; RustRed owns Lorentz physics/projectors |
| Persistence | State/Atom export, optional bincode | Embed in a versioned RustRed schema; never persist ids/closures alone |
| Parallelism | Rayon internals, thread-local workspace | License first; fixed registry; controlled RustRed pools |
| Out-of-core equations | `TermStreamer` | Not suitable as rule DB; use typed checkpoint/external sort |

### 15.1 Current RustRed implementation boundary

The generic coefficient contexts enforce fixed ordered variable maps, reject
zero denominators, and wrap proof-critical arithmetic and specialization with
explicit growth limits. Both the authenticated generic projector and the
still-public legacy `VacuumTensorProjector` now delegate Gram determinant,
inverse, two-sided replay, and coefficient powers to the checked Symbolica
boundary. New authenticated projectors record determinant and inverse-output
denominator conditions; they do not retain a private pivot transcript.

This completed projector algebra slice does not mean that the entire tensor
front end is free of custom algebra. The bounded tensor-aware normalization in
`src/symbolica_tensor_numerator.rs` still selectively distributes additions,
products, and tensor-containing powers. Whole-expression `AtomCore::expand`
does not preserve its opaque scalar-weight semantics, and no suitable public
selective-expansion API has yet been verified. That exact gap remains in the
migration table and requires a native differential route before the private
distributor can be deleted. Other open policy gaps include tensor-family
shift-polynomial arithmetic, remaining symmetry/Feynman matrix consumers, no
generic finite-field `K(n)` sampling/reconstruction engine, and no complete
durable parametric-rule artifact. These gaps block a complete LiteRed/Vakint
parity claim; they do not make the generic IBP identity generator
topology-specific.

The same licensed checkpoint generated and checked generic one- through
five-loop IBP/LI identities, including all 25 ordinary identities of a complete
five-loop massive-vacuum family; parsed and replayed the Symbolica tensor
numerator boundary; and compared freshly discovered one-loop scalar/tensor
reductions with FORM-free Vakint oracles.  Concrete families in those tests are
validation fixtures only.  They do not turn the core generator into
loop-count-specific code, and this checkpoint does not claim that the generic
whole-family LiteRed solver or the two- through five-loop reductions are
complete.

## 16. First generic implementation slice

The first code slice should establish correctness infrastructure before attempting one-loop examples:

1. Deterministic startup/license/symbol registry.
2. `CoeffContext` with canonical map, strict Atom conversion, explicit remap, checked arithmetic invariants and guarded partial RP substitution.
3. Serializable `Guard` AST/evaluator with polynomial equalities/nonzero assumptions and integer/sector predicates.
4. Typed `FamilyModel`, momentum/scalar-product basis and `IntegralKey` independent of Atom ordering.
5. Fully parametric IBP identity generator for arbitrary loop/external momentum counts; add non-vacuum LI generation in the same typed algebra.
6. Sparse equation representation and exact small-system elimination with pivot guards/provenance; modular acceleration only after exact replay exists.
7. Typed rule matcher/applier with a strict complexity-decrease check and memoization.
8. Artifact round trip with family/variable hashes and exact identity replay.
9. Tensor front end implementing metric/vector contractions and one-loop tensor projectors, optionally canonicalized by `CanonicalTensor` afterward.
10. Only then instantiate concrete one-loop families and compare scalar/tensor reductions to Vakint without replacing master topologies; advance to two and three loops only after parametric-rule replay and oracle comparisons pass.

This ordering keeps every concrete topology in validation while the implementation remains as generic as LiteRed.

## 17. Mandatory probe checklist before relying on the API

Because this audit intentionally did not run Cargo, the following are required, small and non-negotiable:

- `RP-SUB-1/2`: exact polynomial-to-RP-field partial substitution and checked division, including map retention.
- `MAP-1`: empty/nonempty `from_coefficient_list`, arbitrary variable permutation, and RP numerator/denominator remap.
- `PAT-1`: demonstrate that a deliberately inconclusive cross-wildcard condition is not accepted as a RustRed guard; verify the suspected wildcard-function condition branch.
- `GB-1`: exact Groebner construction/reduction/verification with the chosen types.
- `LA-1/2/3`: exact dense/sparse solve, inconsistent row behavior, and parallel output ordering.
- `SER-1`: state-mapped polynomial/RP round trip with final crate features.
- `TEN-1`: planned tensor-head/index encoding and error behavior.
- `THR-1`: licensed startup ordering and parallel operations in disposable processes.

Each probe should become a regression test. If a probe contradicts a source-based assumption, preserve the safe wrapper contract and update the implementation behind it rather than exposing the raw Symbolica behavior throughout RustRed.

## 18. Bottom line

The safe division of responsibility is clear: Symbolica supplies exact expression and algebra engines; RustRed supplies the full LiteRed/Vakint semantics. The authoritative core should be typed, parametric and guard-aware. Pattern matching, automatic variable discovery, fraction-field cancellation, generic sparse solve and tensor canonicalization are useful tools, but none can independently certify an IBP rule or tensor reduction. Every generated rule must carry its domain, provenance and exact replay proof, and every concrete Vakint comparison must validate—not define—the generic implementation.
