# Symbolica Rust API boundary for a LiteRed-complete RustRed

> **Frozen source-API audit.** This report is subordinate to
> [`GOAL.md`](../../GOAL.md) and applies to pinned Symbolica revision
> `77c137481904b8a5531ede86e3ef36b82beed7fd` (2.2.0). It records source
> behavior and safe embedding constraints, not current RustRed implementation
> status.

This report covers the API boundary needed for
generic, parametric IBP/LI generation, sector and symmetry handling, guarded
rule solving/application, persistence, and a Vakint/FORM-inspired
tensor-numerator front end. It does **not** treat concrete topologies as part
of the core algorithm; those belong only in tests and oracle validation.

Code fragments below are source-derived idioms unless the linked upstream
tests establish their behavior. Current implementation and validation claims
belong in the live code and test suite, not this frozen inventory.

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

Access to Symbolica source is not by itself a redistribution grant. Shipping,
CI, wheels, and deployment require explicit permission and the appropriate
professional license; each distribution environment must be reviewed
separately from a developer's local license.

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
automatically retains at most 30 Atom buffers whose capacity is at most
20,000,000 bytes through `RecycledAtom::drop`. Public
`Workspace::return_atom` bypasses both caps, so neither statement is an
absolute Workspace bound and the cache is not persistent algebraic storage.

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
- `reorder<ON>()`: changes monomial ordering only, **not variable order**, `vendor/symbolica/src/poly/polynomial.rs:1406-1429`;
- `rearrange_with_growth(&[PolyVariable])`: rewrites a polynomial into an explicitly supplied variable order, admits new zero-exponent variables, and returns an error if an omitted source variable occurs with nonzero exponent, `vendor/symbolica/src/poly/polynomial.rs:2361-2399`.

For a deterministic remap, RustRed should first validate that the target map is
the intended ordered, duplicate-free context map, preflight the target exponent
storage and sorting work, and then call `rearrange_with_growth`. The method
checks that no used source variable was omitted, but does not itself reject a
duplicate target variable, uses infallible internal allocations, and constructs
a fresh variable-map `Arc`. After the call RustRed must authenticate exact map
equality and may rebind the structurally equal map to the canonical context
`Arc` to preserve sharing.

Manual reconstruction with:

```rust
MultivariatePolynomial::from_coefficient_list(
    coefficients,
    flattened_permuted_exponents,
    target_variables,
    &ring,
)
```

defined at `vendor/symbolica/src/poly/polynomial.rs:1711-1738`, remains only a
lower-level fallback when the native helper cannot express a separately proven
mapping. For a rational polynomial, apply `rearrange_with_growth` independently
to its public `numerator` and `denominator`, authenticate both returned maps,
and reconstruct with `FromNumeratorAndDenominator::from_num_den` if
normalization is desired (`vendor/symbolica/src/domains/rational_polynomial.rs:61-68`).

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

Predicate-locus interning uses two different associate relations. They must
not be cross-dispatched:

```text
base-only P,Q in Z[theta]:
    P ~base Q  iff  P = c Q for some nonzero c in Q;

index-dependent P,Q in Z[theta,n]:
    P ~index Q  iff  P = u Q for some nonzero u in Q(theta).
```

This distinction is semantic, not an optimization. Although every nonzero
base polynomial is invertible in the formal coefficient field, the physical
parameter assumptions `theta != 0` and `theta + 1 != 0` describe different
domains. Thus `theta ~base 2 theta`, while `theta` is not base-associated to
`theta + 1` or `theta^2`. Conversely, an index-dependent locus may discard a
nonzero base-field factor: `theta (1-n) ~index theta^2 (1-n)`, whereas
`1-n` is not associated to `(1-n)^2`. Zero has no projective class in either
lane.

For the base-only lane, RustRed first authenticates that every private-index
exponent is zero. It then asks Symbolica to form the two exact scalar products
`lc(Q) P` and `lc(P) Q` through
`MultivariatePolynomial::mul_coeff`; authenticated equality of those products
is equivalent to association over `Q*`. This avoids a private content, GCD, or
primitive-normalization implementation.

For the index-dependent lane, write the inputs as
`P = sum_a P_a(theta) n^a` and `Q = sum_a Q_a(theta) n^a`. Equal index support
is necessary. After choosing a deterministic nonzero anchor `0`, the exact
projective criterion is

```text
P_a Q_0 = Q_a P_0 for every index-monomial group a.
```

This proves association by a base-field unit. Both lanes are deliberately
stricter than equality of radicals or vanishing sets. At every integer
boundary, numerical comparison with canonical zero is required; the
representation-sensitive Symbolica zero predicates do not by themselves
reject noncanonical `Double(0)` or `Large(0)` coefficients.

A safe index-dependent algebra route uses public Symbolica APIs end to end:

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

RustRed owns only the category-sensitive associate semantics, deterministic
support/anchor routing, admission, authentication, panic containment,
provenance, and transactional census propagation. Symbolica owns scalar and
polynomial multiplication, projection, integer arithmetic, collection, and
equality. No private normalization, GCD, or cross-product implementation is
retained as a production or test oracle.

Resource staging is part of the boundary contract. The base-only lane admits
both sparse copies, the two scalar calls, coefficient/bit work, native
workspace, cross-scaled outputs, and comparison payload before either native
call. The index-dependent lane admits validation payloads, widened and
projected exponent storage, actual GMP capacities, projection grouping and
sorting work, native cross-term and integer-bit work, native dense/heap
dispatch workspace, output envelopes, and RustRed-visible temporary storage
before `map_exp`, `to_polynomial`, or `mul`. After each native call RustRed
reauthenticates maps, denominators where applicable, term counts, exponents,
integer bits, and canonical ordering against those bounds.
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

### 9.4 Rational-function decomposition and calculus

The public rational-polynomial surface already provides `derivative`, `apart`,
`apart_factored_denominators`, `apart_multivariate`, and `integrate`. The latter
paths can invoke factorization or Groebner-basis machinery and contain
assertions/unwraps; none accepts RustRed's caller resource budget or
cancellation token. Use them only behind a checked native boundary with exact
recombination or differentiation replay. Their operational limitations are
not permission to hand-write the same CAS transformations in RustRed.

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
(`vendor/symbolica/lib/numerica/src/tensors/matrix.rs:1557-1579`). A safe
generic-family boundary calls the independent native `Matrix::det` first,
rejects a zero determinant, and retains the determinant numerator as
the family-domain nonzero condition before calling `inv`. Generic parametric
elimination remains a separate guard/provenance problem and still has to retain
its selected pivot conditions.

The Atom-level `solve_linear_system`/`system_to_matrix` convenience API (`vendor/symbolica/src/atom/core.rs:757-835`) auto-extracts symbolic coefficients. It does not provide integral ordering, guard-aware pivots or proof provenance, so it should be confined to diagnostics.

For automatic ISP completion, copy an authenticated nonempty rectangular
coefficient matrix into the checked contextual field and call public
`Matrix::partial_row_reduce` over all columns. This destructive entry point
avoids the extra clone in `Matrix::rank`; RustRed retains only the deterministic
candidate scan, coordinate order, resource policy, and replay evidence. An
independent maximal-minor oracle must delegate every determinant to public
`Matrix::det`.

Two additional native edge cases are relevant when selecting matrix APIs.
`Matrix::rank`/`partial_row_reduce` indexes row zero for a `0 x N` input, so the
checked RustRed boundary rejects empty matrices before entry.  Also, the
single-`u32` row-index implementation in the vendored dense matrix multiplies
by `nrows` instead of `ncols`; RustRed uses tuple indexing and iterators and
does not rely on that rectangular row-slice path.

#### 10.1.1 Checked contextual field boundary

No public Symbolica 2.2.0 field object combines a rational-polynomial element
with RustRed's authenticated variable map and fallible resource policy.
`RationalPolynomialField` creates constants on an empty map and may unify maps
during arithmetic; `RingOps` and `Field` scalar methods provide no typed
resource-failure channel. A narrow contextual adapter may therefore own map
authentication, admission, panic containment, and output checks while
delegating every scalar and matrix operation to Symbolica.

For inverse-dependent work, call native `Matrix::det` first, reject zero, then
call native `Matrix::inv`; verify both `A A^-1` and `A^-1 A` entry by entry
rather than with the affected `Matrix::is_one`. Construct constants from an
authenticated element template, because convenience constructors and a
singular determinant result may carry empty maps. These checks compose public
operations; they are not a second determinant, inverse, or matrix-product
implementation.

#### 10.1.2 Integer affine maps and normal forms

Public `Matrix<IntegerRing>` supports rectangular construction and exact
multiplication, so affine-map composition must use native matrix products.
RustRed may authenticate shapes, integer payloads, and resource envelopes and
replay output geometry, but it must not implement the dot products.

The audited public dense/sparse, solving, polynomial, Groebner, and domain APIs
expose no Smith or Hermite normal form, integer kernel basis, or complete
underdetermined integral-affine parameterization. `solve_fraction_free` is not
such an API. A literal-unit-pivot specialization may be composed from native
normalization and matrix multiplication; general no-unit or simultaneous
equalities require a typed unsupported result until Symbolica supplies the
missing normal-form/kernel API.

### 10.2 Sparse matrices and row reduction

`SparseMatrix<F>` is a CSR matrix. Public constructors include `new`,
`from_csr`, `from_csr_slices`, and ordered `from_triplets`; callers must
sort triplets, combine duplicates, drop zeros, and bounds-check input.
Consuming sparse solve, determinant, inverse, and parallel solve are public.
`SparseRowReducer<F: Field>` exposes `LuLMode::{Full,Pattern,None}`,
constructors, `u`, `l`, `pivots`, incremental `add_row`/`add_matrix`,
dynamic `add_cols`, and back substitution.

The reducer selects the smallest stored nonzero column as pivot and normalizes
it. RustRed can map hardest integral keys to the smallest columns, retain one
unused sentinel column when a final dependent row must still produce an
`L` transcript, and independently replay the source combination. Under
`LuLMode::Full`, public `L`, `U`, and pivots provide the native
decomposition, but an empty input row or an already-full-rank reducer may append
no `L` row. The `L` factor order follows physical pivot traversal rather than
chronological pivot creation.

The sparse inverse has the same augmented-matrix singularity defect as the
dense inverse. Checked inconsistency detection also appears inverted for a row
`[0 ... 0 | c]`; do not use sparse solve as a correctness oracle without the
focused revision-specific probes in the upstream-gap audit. Final
`back_substitute` operations mutate `U` and pivots, clear `L`, and change
the reducer mode, so they belong on a publication clone rather than a retained
forward reducer.

Symbolica exposes no fallible/COW reducer fork, cancellation hook, complete
native-byte census, or parallel forward-reduction path. Cloning overlaps
`U`, `L`, pivots, scratch, and coefficient ownership. Thread-local Atom
workspaces and dense-polynomial scratch are also opaque to a caller budget.
RustRed admission must therefore charge predecessor/trial/successor overlap,
calibrated per-thread native headroom, and result staging before constructing a
clone or worker. Public vector capacities are estimator inputs, not proof of a
hard RSS bound.

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

RustRed integration must expose one deterministic licensed startup and
symbol-registration boundary. In particular, tensor/Vakint symbol helpers must
validate the attributes of an already registered name rather than accept
test-order-dependent plain symbols. Whether that boundary is complete is a
live-roadmap question, not a claim made by this frozen API audit.

Unlicensed mode is not a supported multithreaded RustRed deployment. It must either run a deliberately single-threaded validation path on the initializing thread or fail early with a clear diagnostic; never risk a process abort in the middle of a reduction.

State registration is globally locked, while workspaces are thread-local. Symbols and callback closures are designed for cross-thread use once initialized, but RustRed must not mutate the registry during parallel reduction.

**Behavior probe THR-1:** in a disposable helper process, test licensed startup before and after Rayon initialization and test multi-threaded Atom/polynomial operations. Do not run the unlicensed cross-thread abort case inside the test runner process.

## 14. Streaming and performance

`TermStreamer` defaults to four cores, the current directory and a 1 GB packed-term-payload spill threshold (`vendor/symbolica/src/streaming.rs:63-107`). This is not a hard retained-memory or RSS limit: sorting overlaps the original buffer with a new output vector, and Atom capacities, workspace caches, compression buffers, and allocator overhead are outside the counter. It splits additions into terms, spills sorted chunks and merges them (`287-539`), can map in parallel (`579-627`), and falls back to one core when unlicensed (`649-729`). `TermStreamer::new` constructs its own pool and its private sort also uses ambient Rayon, so it is not admitted inside an ordinary RustRed outer worker. Export/import includes State and term count (`332-390`). Examples/tests are `vendor/symbolica/examples/streaming.rs:4-21` and `vendor/symbolica/src/streaming.rs:747-867`.

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
| Generic-family coefficient matrices | `Matrix` over an authenticated rational-polynomial field adapter | Native determinant, inverse, and products behind contextual admission, determinant guard, authentication, and entrywise two-sided replay |
| Integer affine-map composition | `Matrix<IntegerRing>` over GMP-backed `Integer` | Native rectangular product behind shape/bit/memory admission and exact geometry replay; no RustRed matrix arithmetic |
| General integer affine-lattice parameterization | No public SNF/HNF or integer-kernel API found | Typed unsupported boundary beyond the literal-unit-pivot subset; never substitute a RustRed-authored CAS |
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

### 15.1 Durable boundary requirements

A production boundary fixes coefficient-variable maps, rejects zero
denominators, admits proof-critical arithmetic before native entry, contains
documented panic surfaces, and authenticates every returned value. Family,
symmetry, zero-sector, tensor, and elimination layers may retain structural and
physics decisions, but coefficient, polynomial, determinant, matrix-product,
and row-reduction algebra stays in public Symbolica.

Affine-boundary specialization constructs and maps index loci with authenticated
Symbolica coefficient operations. Divisibility of a boundary polynomial into a
coefficient numerator is delegated to native polynomial division over
`K[n]`; physical parameters are field variables, so this does not create
pointwise assumptions. Resource exhaustion or a missing public primitive is a
typed boundary outcome, never a reason to substitute handwritten CAS code.

## 16. Revision-upgrade probe checklist

Repeat these small probes before relying on a changed Symbolica revision:

- `RP-SUB-1/2`: exact polynomial-to-RP-field partial substitution and checked division, including map retention.
- `MAP-1`: empty/nonempty `from_coefficient_list`, arbitrary variable permutation, and RP numerator/denominator remap.
- `PAT-1`: demonstrate that a deliberately inconclusive cross-wildcard condition is not accepted as a RustRed guard; verify the suspected wildcard-function condition branch.
- `GB-1`: exact Groebner construction/reduction/verification with the chosen types.
- `LA-1/2/3`: exact dense/sparse solve, inconsistent row behavior, and parallel output ordering.
- `SER-1`: state-mapped polynomial/RP round trip with final crate features.
- `TEN-1`: planned tensor-head/index encoding and error behavior.
- `THR-1`: licensed startup ordering and parallel operations in disposable processes.

Each probe should become a regression test. If a probe contradicts a source-based assumption, preserve the safe wrapper contract and update the implementation behind it rather than exposing the raw Symbolica behavior throughout RustRed.

## 17. Bottom line

The safe division of responsibility is clear: Symbolica supplies exact expression and algebra engines; RustRed supplies the full LiteRed/Vakint semantics. The authoritative core should be typed, parametric and guard-aware. Pattern matching, automatic variable discovery, fraction-field cancellation, generic sparse solve and tensor canonicalization are useful tools, but none can independently certify an IBP rule or tensor reduction. Every generated rule must carry its domain, provenance and exact replay proof, and every concrete Vakint comparison must validate—not define—the generic implementation.
