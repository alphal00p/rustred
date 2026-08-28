# Fully parametric IBP representation and API

Date: 2026-08-13

> **Historical draft.**  This file predates the full LiteRed-scope audit and
> is too specific to `VacuumFamily`: in particular its `L^2` generator omits
> external-contraction IBPs and LI identities.  The authoritative replacement
> is [`litered_full_scope_spec.md`](litered_full_scope_spec.md).  Reusable
> details below (authenticated coefficient contexts, typed shifts, guards,
> specialization, and provenance) remain design input only where they agree
> with that specification. The concrete `VacuumFamily`/`IbpGenerator` types
> referenced here are now quarantined in `rustred-legacy-oracles`; production
> uses `IntegralFamily` and `ParametricIbpGenerator`.

## Outcome

The next scalar core should generate each family's IBPs once as exact
relations in symbolic indices.  It should not enumerate concrete seeds as its
primary representation.  For a `VacuumFamily` with `N` denominator-basis
entries, the canonical generated object is

\[
  \sum_{\delta\in\mathbb Z^N} c_\delta(p,a)
  I(a+\delta)=0,
  \qquad
  c_\delta\in\mathbb Q(p_1,\ldots,p_m,a_1,\ldots,a_N),
\]

where `p` is the family's existing ordered coefficient-parameter list and
`a` is a new ordered symbolic index vector.  A concrete `Integral` is only an
assignment to `a`, used by tests, finite discovery, or final reduction.

The recommended implementation split is:

```text
VacuumFamily + CoefficientContext
        |
        v
ParametricCoefficientContext  Q(p,a)
        |
        v
ParametricIbpGenerator         L^2 seed-free identities
        |
        +--> exact specialization --> current concrete IbpIdentity
        |
        +--> translation/elimination --> guarded ParametricRule
                                      --> typed concrete replay
```

Symbolica should perform all polynomial and rational-function algebra.  Rust
types should continue to own integral keys, shifts, guards, provenance, and
resource accounting.  General Symbolica `Atom`/`Pattern` objects are useful
for display and adapters, but are not a safe canonical rule database.

This design is topology-independent.  Nothing in the parametric core depends
on a particular loop count, routing, symmetry group, sector, power assignment,
or master list.

## 1. Existing seam and required separation

The current code already supplies almost all family-level algebra:

- `VacuumFamily` proves that its denominator list is a complete basis of the
  `L*(L+1)/2` vacuum scalar products.
- Its cached `DenominatorLinearForm` is exactly

  \[
    k_j\mathbin\cdot\partial_{k_i}D_r
      = c^{(0)}_{rij}(p)+\sum_s c^{(s)}_{rij}D_s,
  \]

  with `c^(0)` in the current `Coefficient` field and `c^(s)` exact rational
  numbers.
- `CoefficientContext` authenticates the ordered Symbolica variable map for
  `K = Q(p)`.
- `Integral` is the correct concrete integer exponent vector.
- `IbpGenerator` implements the right formula, but only after receiving one
  concrete `Integral` seed.

The new layer should be additive.  The concrete generator remains a useful
oracle and compatibility surface, while production recurrence discovery uses
the parametric representation.

The boundaries must stay explicit:

| Layer | Integral coordinates | Coefficient field | May use family symmetry/zero rules? |
|---|---|---|---|
| Raw parametric generation | symbolic `a + shift` | `Q(p,a)` | no |
| Concrete specialization | checked `i32` vector | `Q(p)` | optional, after specialization |
| Parametric rule | normalized `I(a)` lhs | `Q(p,a)` | only with an explicit guard/orientation proof |
| Concrete recursive replay | checked `i32` vector | `Q(p)` | yes |

Raw generation must not symbolically canonicalize integrals.  Which symmetry
image is canonical depends on sector signs and concrete index order, and zero
sectors are also sign-domain statements.  Applying either operation while
building the universal relation can silently delete valid branches.

## 2. Mathematical normal form

Use the existing convention

\[
  I(a)=\int\prod_{\ell=1}^{L}d^d k_\ell
       \prod_{r=1}^{N}D_r^{-a_r},
  \qquad a_r\in\mathbb Z.
\]

For each ordered pair `(i,j)` of loop positions,

\[
\begin{aligned}
0={}&\partial_{k_i}\mathbin\cdot k_j\,I(a)\\
 ={}&\delta_{ij}d\,I(a)
 -\sum_{r=1}^{N}a_r c^{(0)}_{rij}(p)I(a+e_r)\\
 &-\sum_{r,s=1}^{N}a_r c^{(s)}_{rij}I(a+e_r-e_s).
\end{aligned}
\]

Three details are invariants, not formatting choices:

1. The derivative multiplier is `a_r`, never `a_r + 1` and never the power
   of the shifted integral.
2. A zero concrete power needs no special branch in the parametric generator:
   specializing `a_r = 0` makes all corresponding terms vanish exactly.
3. Equal shifts are combined and exact zero coefficients are removed.  In
   particular, `e_r-e_r=0` contributes to the same key as the dimension term.

Every raw identity therefore uses only the shifts

```text
0
e_r
e_r - e_s
```

but the shift type must support arbitrary checked translations because sector
search and elimination evaluate these identities at displaced index points.

## 3. Coefficient field and authenticated context

### 3.1 Flat Symbolica field

Retain the existing raw representation:

```rust
RationalPolynomial<IntegerRing, u16>
```

The parametric field is the flat fraction field

```text
Q(p_0, ..., p_{m-1}, a_0, ..., a_{N-1})
```

with one exact ordered variable map.  A flat field is preferable to a nested
`Q(p)(a)` representation because it:

- reuses `Coefficient` and Symbolica's exact gcd cancellation;
- permits cancellation between index and family-parameter factors;
- can use the existing exact sparse elimination field;
- supports exact projection back to the current `CoefficientContext`; and
- has one deterministic serialization order.

The index variables are algebraically independent in coefficient arithmetic.
Their integer semantics belong to rule guards and specialization, not to the
polynomial ring.

### 3.2 Context type

Using the same raw type does not make coefficients from different variable
maps interchangeable.  Introduce an authenticated wrapper and make its fields
private:

```rust
pub struct ParametricCoefficientContext {
    family: CoefficientContext,
    extended_variables: Arc<Vec<PolyVariable>>, // [p..., a...]
    indices: Arc<[IndexVariable]>,
    id: ParametricContextId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricCoefficient {
    raw: Coefficient,
    context: ParametricContextId,
}

pub struct IndexVariable {
    position: usize,
    symbol: Symbol,
}
```

`ParametricCoefficientContext` belongs in `coefficient.rs`, or must be built
there through crate-private constructors, because the existing
`CoefficientContext` intentionally keeps its Symbolica variables and template
private.

Required operations are:

```rust
impl ParametricCoefficientContext {
    pub fn for_family(family: &VacuumFamily)
        -> Result<Self, ParametricContextError>;

    pub fn zero(&self) -> ParametricCoefficient;
    pub fn one(&self) -> ParametricCoefficient;
    pub fn integer(&self, value: i64) -> ParametricCoefficient;
    pub fn index(&self, position: usize)
        -> Result<ParametricCoefficient, ParametricContextError>;

    pub fn lift_family_coefficient(&self, value: &Coefficient)
        -> Result<ParametricCoefficient, ParametricContextError>;

    pub fn specialize_indices(
        &self,
        value: &ParametricCoefficient,
        assignment: &IndexAssignment,
        limits: &CoefficientSpecializationLimits,
    ) -> Result<Coefficient, ParametricSpecializationError>;

    pub fn translate_indices(
        &self,
        value: &ParametricCoefficient,
        offset: &IndexShift,
        limits: &ParametricArithmeticLimits,
    ) -> Result<ParametricCoefficient, ParametricArithmeticError>;
}
```

`lift_family_coefficient` must first prove exact equality with the family's
ordered variable map.  It then copies every source monomial and appends `N`
zero exponents.  It must not format and reparse a coefficient, and it should
not rely on Symbolica arithmetic's automatic variable-map unification.
Automatic unification is mathematically convenient but loses RustRed's
context authentication and can choose an order different from the cache
schema.

Index symbols should live in a dedicated internal top-level namespace, for
example

```text
rustred_ibp_index_v1::<context digest>::a0
```

rather than being caller-provided parameter names.  The digest is derived
from the family fingerprint, ordered base-variable names, arity, and context
schema version.  Reconstruct symbols by qualified name when loading a cache;
do not serialize process-local Symbolica symbol IDs as RustRed identities.

### 3.3 Checked specialization

Symbolica's `RationalPolynomial::evaluate` evaluates every variable into a
field value, whereas RustRed needs to evaluate only the index suffix and keep
`p` symbolic.  `MultivariatePolynomial::replace` also retains the original
variable map.  Implement a dedicated exact suffix specialization:

1. authenticate the parametric context and assignment arity;
2. for each source monomial, multiply its integer coefficient by
   `assignment[i]^exponent[i]` for the index suffix;
3. copy the family-parameter exponent prefix into a polynomial created from
   the target `CoefficientContext` template;
4. combine equal monomials exactly;
5. do this independently for numerator and denominator;
6. reject a specialized zero denominator; and
7. reconstruct with `FromNumeratorAndDenominator::from_num_den(..., true)`.

This is a multi-variable counterpart of the existing checked
`project_parameter_free`, not an Atom conversion.

The specialization preflight must bound both source monomial work and big
integer growth.  A small polynomial degree evaluated at a very large `i32`
index can still allocate a large `Integer`.

### 3.4 Rational-function domains are explicit

A rational function is a generic-field element, but applying it at a special
parameter point additionally requires its denominator to be nonzero.  Preserve
that distinction in the API:

```rust
pub struct ParametricDomain {
    context: ParametricContextId,
    nonzero: BTreeSet<ParametricPolynomial>,
}
```

Denominator polynomials are made primitive with a canonical leading sign;
nonzero integer constants are discarded.  Each generated identity carries
the union of the nonconstant denominator conditions of its coefficients, and
the system carries their union.  A derived rule adds its rebased pivot
numerator and the denominators of its normalized rhs coefficients.  This may
be a conservative domain, but it is explicit and correct.

Formal generic-field equality does not require proving these predicates.
Strict evaluation at assigned family parameters does.  Index-only
specialization may leave them as residual parameter conditions, exactly like
the rule guards in section 8.

`ParametricPolynomial` supplies stable `Eq`/`Ord` from its authenticated
variable map and canonical coefficient/exponent term arrays; it must not use a
formatted expression string as the `BTreeSet` key.

## 4. Index-space and sparse relation types

### 4.1 Arity-bound shifts

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexShift(Box<[i32]>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexAssignment(Box<[i32]>);
```

Construction is through an `IndexSpace` so arity is checked once:

```rust
pub struct IndexSpace {
    arity: usize,
    context: ParametricContextId,
}

impl IndexSpace {
    pub fn zero_shift(&self) -> IndexShift;
    pub fn unit_shift(&self, position: usize)
        -> Result<IndexShift, IndexError>;
    pub fn shift(&self, values: impl IntoIterator<Item = i32>)
        -> Result<IndexShift, IndexError>;
    pub fn assignment(&self, integral: &Integral)
        -> Result<IndexAssignment, IndexError>;
}

impl IndexShift {
    pub fn checked_add(&self, rhs: &Self) -> Result<Self, IndexError>;
    pub fn checked_sub(&self, rhs: &Self) -> Result<Self, IndexError>;
    pub fn checked_neg(&self) -> Result<Self, IndexError>;
}
```

Private fields prevent a wrong-arity shift from entering a row.  Dense shifts
are appropriate here: the denominator count is already a dense family
dimension, elementary IBPs touch up to two positions, and deterministic
`Ord` is needed for sparse row keys.  A later small-vector optimization must
not change equality or serialization.

Keep `i32` to match `Integral`, but perform accumulation through checked
`i64`, as `Integral::checked_shifted` already does.  No operation may negate
`i32::MIN` directly.

### 4.2 Sparse combinations

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricLinearCombination {
    context: ParametricContextId,
    arity: usize,
    terms: BTreeMap<IndexShift, ParametricCoefficient>,
}

impl ParametricLinearCombination {
    pub fn try_add_term(
        &mut self,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
        limits: &ParametricArithmeticLimits,
    ) -> Result<(), ParametricArithmeticError>;
}
```

`try_add_term` authenticates context and arity, combines an existing key,
removes exact zeros, and checks the coefficient-operation budget before
calling Symbolica.  A `BTreeMap` makes the identity, hash input, serialized
form, and equality replay deterministic.

An integral object is not needed inside a universal row: every term is
`I(a + shift)` in the row's shared `IndexSpace`.

## 5. Seed-free generator API

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IbpOperator {
    differentiated_loop: usize,
    contraction_loop: usize,
}

pub struct ParametricIbpIdentity {
    pub operator: IbpOperator,
    pub equation: ParametricLinearCombination,
    pub domain: ParametricDomain,
    pub origin: ParametricIdentityOrigin,
}

pub struct ParametricIbpSystem {
    pub family_fingerprint: String,
    pub context: ParametricCoefficientContext,
    pub domain: ParametricDomain,
    pub identities: Vec<ParametricIbpIdentity>,
}

pub struct ParametricIbpGenerator<'family> {
    family: &'family VacuumFamily,
    context: ParametricCoefficientContext,
    limits: ParametricGenerationLimits,
}

impl<'family> ParametricIbpGenerator<'family> {
    pub fn try_new(
        family: &'family VacuumFamily,
        limits: ParametricGenerationLimits,
    ) -> Result<Self, ParametricIbpError>;

    pub fn try_generate(&self)
        -> Result<ParametricIbpSystem, ParametricIbpError>;

    pub fn try_generate_identity(
        &self,
        differentiated_loop: usize,
        contraction_loop: usize,
    ) -> Result<ParametricIbpIdentity, ParametricIbpError>;
}
```

`try_generate` creates exactly `L*L` rows in lexicographic operator order.
The implementation reuses the family's cached derivative contractions:

```text
for i in 0..L:
  for j in 0..L:
    if i == j:
      add shift 0 with coefficient d

    for r in 0..N:
      factor = -a[r]

      if contraction[r][i][j].constant != 0:
        add shift e[r] with factor * lifted_constant

      for each nonzero rational c at s:
        add shift e[r] - e[s] with factor * c
```

The generator takes no concrete seed, sector mask, symmetry, master list,
expected rank, or answer.  Its only mathematical inputs are the authenticated
family and its dimension/coefficient context.

`ParametricIdentityOrigin` records the family fingerprint, operator, generator
schema version, and coefficient-context ID.  This is sufficient to regenerate
every source row independently.

The symbolic `a_i` already provide arbitrary parametric powers.  They must not
be confused with either a translated seed identity or LiteRed's experimental
constant `PowerShifts` option.  The current `VacuumFamily` has no power-shift
field, so the first implementation has exactly zero constant power offsets.
If such offsets are added later, represent the physical exponent explicitly
as `a_i + nu_i` and use that expression only in the derivative multiplier;
do not reinterpret `IndexShift` or silently import an incomplete subset of the
experimental semantics.

## 6. Translation and pivot normalization

Translated generic identities are required for a LiteRed-style neighborhood
search.  Define translation by `s` as

\[
  T_s\!\left[c_\delta(a)I(a+\delta)\right]
    =c_\delta(a+s)I(a+s+\delta).
\]

Thus `translated(s)` substitutes `a_i -> a_i+s_i` in every coefficient and
adds `s` to every shift.  Symbolica's polynomial
`replace_with_poly(variable, a_i+s_i)` can implement the substitution on the
numerator and denominator, subject to explicit expansion limits.  The result
must retain the same authenticated variable map.

When an eliminated row has pivot shift `q`, normalize it to the rule

\[
  I(a)=\sum_{\eta\ne0}r_\eta(a)I(a+\eta)
\]

by renaming `b=a+q`:

```text
coefficient c_delta(a) -> c_delta(a - q)
shift delta            -> delta - q
rhs coefficient        -> -c_delta(a-q) / c_q(a-q)
```

The pivot numerator after rebasing becomes a mandatory nonzero guard.  It is
incorrect to divide in the rational-function field and then present the result
as an unconditional integer-index rule.

Translation must be a checked operation because `(a+s)^e` can create many
monomials even though it does not increase the polynomial degree.  Charge the
expanded source terms before retaining cancellations.

## 7. Exact specialization and compatibility replay

Every parametric identity needs two concrete projections:

```rust
impl ParametricIbpIdentity {
    pub fn specialize_raw(
        &self,
        seed: &Integral,
        family: &VacuumFamily,
        limits: &ParametricSpecializationLimits,
    ) -> Result<IbpIdentity, ParametricSpecializationError>;

    pub fn specialize_canonicalized(
        &self,
        seed: &Integral,
        family: &VacuumFamily,
        limits: &ParametricSpecializationLimits,
    ) -> Result<IbpIdentity, ParametricSpecializationError>;
}
```

For each `(shift, coefficient)`, raw specialization:

1. authenticates the family fingerprint, coefficient context, and seed arity;
2. evaluates every `a_i` at `seed.powers()[i]`, retaining `p` symbolically;
3. computes `seed + shift` with checked exponent arithmetic; and
4. combines equal concrete integrals.

The canonicalized variant then applies `VacuumFamily::try_canonicalize`, drops
proved scaleless images, and combines symmetry images.  Keeping these two
steps separate makes the raw generator independently testable.

The central compatibility property is exact equality:

```text
parametric_identity(i,j).specialize_raw(seed)
    == IbpGenerator::try_generate_raw_identity(seed, i, j)
```

for every representable seed.  This is stronger than comparing a final
reduction: it catches sign, multiplier, shift, and variable-map errors at the
source.

Concrete families, fixed powers, and Vakint outputs belong only in tests and
validation adapters.  They must not appear in generator dispatch, production
recurrence coefficients, expected-rank branches, or default master data.

## 8. Guarded parametric rules

### 8.1 Canonical rule shape

Every accepted rule should have the normalized form

```rust
pub struct ParametricRule {
    pub family_fingerprint: String,
    pub context: ParametricContextId,
    pub domain: ParametricDomain,
    pub guard: RuleGuard,
    pub rhs: ParametricLinearCombination, // no zero shift
    pub provenance: ParametricRuleCertificate,
    pub descent: RuleDescentCertificate,
}
```

The lhs is implicitly `I(a)`.  Fixed-index cases do not require a separate
pattern language: a guard such as `a_i == c` is exact, serializable, and makes
matching deterministic.  Sector patterns are the conjunction

```text
active position:   a_i >= 1
inactive position: a_i <= 0
```

with any family policy for auxiliary entries compiled into the same guard.

`rhs` must not contain the zero shift.  Its coefficients and all guard
polynomials use the rule's exact context.  A rule loaded for a different
family, variable order, denominator convention, or ordering version is a
typed mismatch, never a best-effort conversion.

### 8.2 Serializable guard language

Do not store guard closures.  Use a small algebraic AST:

```rust
pub enum RuleGuard {
    True,
    False,
    All(Vec<RuleGuard>),
    Any(Vec<RuleGuard>),
    Not(Box<RuleGuard>),
    Index(IndexPredicate),
    Algebraic(AlgebraicPredicate),
}

pub struct AffineIndexForm {
    pub terms: Box<[(usize, i64)]>, // sorted, unique, nonzero coefficients
    pub constant: i64,
}

pub enum IndexPredicate {
    EqZero(AffineIndexForm),
    NeZero(AffineIndexForm),
    GeZero(AffineIndexForm),
    LeZero(AffineIndexForm),
}

pub enum AlgebraicPredicate {
    Zero(ParametricPolynomial),
    NonZero(ParametricPolynomial),
}
```

Affine forms use checked `i128` intermediates during concrete evaluation and
return overflow rather than wrapping.  General pivot exceptional sets are
represented by `ParametricPolynomial`, so nonlinear index factors do not need
to be forced into the affine language.

`ParametricPolynomial` is an authenticated
`MultivariatePolynomial<IntegerRing,u16>`.  Normalize its integer content and
leading sign so equal conditions deduplicate.  Factorization may make case
splitting better, but it is an optimization: the exact predicate `P != 0` is
already correct without factoring.

Each division by an elimination pivot adds the rebased pivot numerator
`!= 0`.  Existing rational coefficient denominators remain in the explicit
`ParametricDomain`; the new logical condition introduced by solving a row is
the pivot numerator.  Applicability is the conjunction of `domain` and
`guard`.

### 8.3 Three-valued application

Applying a rule at an integer `Integral` can decide all index predicates, but
may leave a polynomial condition in family parameters.  Never coerce that
state to `true` silently:

```rust
pub enum RuleApplicability {
    NotApplicable,
    Applicable,
    ApplicableUnder(SpecializedParameterGuard),
}
```

After substituting indices into an algebraic predicate:

- the zero polynomial decides `Zero` true and `NonZero` false;
- a nonzero integer constant decides the reverse;
- a nonconstant polynomial in `p` remains a parameter guard.

A generic-parameter reducer may carry `ApplicableUnder` conditions into its
certificate.  A strict specialization reducer must ask a caller-provided
`ParameterAssumptions` object to prove them, otherwise return an exceptional
or undecidable-specialization error.

### 8.4 Typed replay

```rust
impl ParametricRule {
    pub fn try_apply(
        &self,
        target: &Integral,
        family: &VacuumFamily,
        assumptions: &ParameterAssumptions,
        limits: &RuleReplayLimits,
    ) -> Result<GuardedLinearCombination, RuleReplayError>;
}
```

Replay performs, in order:

1. family/context/arity authentication;
2. exact guard evaluation;
3. checked addition of each rhs shift to the target;
4. exact coefficient specialization `a -> target.powers()`;
5. optional zero-sector and symmetry canonicalization; and
6. combination of equal outputs with checked coefficient budgets.

The recursive reducer may use a rule only when its `RuleDescentCertificate`
proves every rhs branch strictly lower on the complete guard domain under a
versioned well-founded order.  A merely discovered but unproved rule can be
stored for diagnostics, but recursive replay must reject it.  A step limit
and cycle detector remain mandatory operational defenses even for certified
rules.

## 9. Provenance and independent verification

An accepted symbolic recurrence must be replayable from generic native rows,
not merely fit concrete samples.  The minimal source record is:

```rust
pub struct WeightedParametricRow {
    pub operator: IbpOperator,
    pub seed_shift: IndexShift,
    pub multiplier: ParametricCoefficient,
}

pub struct ParametricRuleCertificate {
    pub sources: Vec<WeightedParametricRow>,
    pub expected_equation_hash: [u8; 32],
    pub generator_schema: u32,
}
```

Verification regenerates each seed-free source identity, translates it by
`seed_shift`, scales it by `multiplier`, combines the rows, rebases the pivot,
and checks exact equality with

\[
  I(a)-\operatorname{rhs}(a).
\]

Equality is equality of authenticated sparse shift maps after exact
coefficient cancellation.  Concrete samples can reject a bad proposal early,
but cannot replace this polynomial identity check.

If direct source lists grow too large, use a hash-consed derivation DAG with
nodes `Source`, `Translate`, `Scale`, and `Add`.  The verifier must still have
a finite node/work budget and must never trust a stored final hash without
replaying the algebra.

## 10. Resource limits

Every public operation is checked and finite.  Separate limits by phase so a
small replay is not forced to inherit a discovery-sized budget.

```rust
pub struct ParametricGenerationLimits {
    pub max_identities: usize,
    pub max_raw_terms_per_identity: usize,
    pub max_combined_terms_per_identity: usize,
    pub max_total_raw_terms: usize,
    pub max_coefficient_monomials: usize,
    pub max_variable_degree: u16,
}

pub struct ParametricArithmeticLimits {
    pub max_shift_abs: i32,
    pub max_translation_expansion_terms: usize,
    pub max_coefficient_monomials: usize,
    pub max_variable_degree: u16,
    pub max_coefficient_operations: u64,
    pub max_guard_nodes: usize,
}

pub struct CoefficientSpecializationLimits {
    pub max_source_monomials: usize,
    pub max_integer_power_operations: u64,
    pub max_estimated_integer_bits: u64,
}

pub struct ParametricSpecializationLimits {
    pub max_input_terms: usize,
    pub max_output_terms: usize,
    pub max_coefficient_operations: u64,
    pub coefficient: CoefficientSpecializationLimits,
}

pub struct RuleReplayLimits {
    pub max_rule_steps: u64,
    pub max_intermediate_integrals: usize,
    pub max_output_terms: usize,
    pub max_coefficient_operations: u64,
    pub max_coefficient_monomials: usize,
    pub max_guard_nodes: usize,
    pub specialization: CoefficientSpecializationLimits,
}

pub struct ParametricCertificateLimits {
    pub max_source_rows: usize,
    pub max_dag_nodes: usize,
    pub max_total_row_terms: usize,
    pub max_coefficient_operations: u64,
}

pub struct ParametricDiscoveryLimits {
    pub initial_depth: u32,
    pub max_depth: u32,
    pub max_lattice_points: usize,
    pub max_instantiated_identities: usize,
    pub max_matrix_rows: usize,
    pub max_matrix_nonzeros: usize,
    pub max_pivots: usize,
    pub max_candidate_rules: usize,
    pub arithmetic: ParametricArithmeticLimits,
    pub certificate: ParametricCertificateLimits,
}
```

Reasonable initial defaults are `4_096` identities, `1_048_576` raw or
combined terms in one identity, `16_777_216` total raw terms, `1_048_576`
coefficient monomials/translation terms, `16_777_216` coefficient operations,
and `65_536` guard or certificate nodes.  These are operational defaults, not
mathematical restrictions: callers may opt into larger finite values after the
same preflights.  `max_variable_degree` may default to the existing Symbolica
`u16` ceiling, while individual reducers should choose tighter bounds when
they can prove them.

Neighborhood depth is never an implicit unbounded loop.  Discovery may raise
`initial_depth` only through `max_depth`, and it charges every prepared point
even if symmetry, zero-sector rules, or duplicate rows later remove it.  A
time limit can be an additional cancellation mechanism, but deterministic
point/row/nonzero limits remain the reproducible correctness boundary.

Generation admits useful exact preflights before coefficient work:

```text
identity count = L * L
raw terms in row(i,j)
  = [i == j]
    + sum_r ([constant support at r,i,j]
             + denominator-support count at r,i,j)
```

Use checked multiplication/addition for these counts.  Charge raw terms before
deduplication so a highly cancelling input cannot evade the work budget.

The existing `SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT` and degree-bound helpers
should be generalized to the parametric wrapper.  Symbolica's `u16`
polynomial exponent overflow can panic; every caller-controlled multiply,
add, translation, and repeated replay must be preflighted before entering that
path.  Term count, not only degree, needs a runtime check because affine
translation can preserve degree while expanding many monomials.

Default limits should be finite and generous.  An explicit `unbounded()`
constructor, if offered at all, should be visibly opt-in and still retain
machine-overflow checks.

## 11. Symbolica Pattern API: adapter, not core storage

The vendored pattern engine can express an adapter such as
`I(a0_,...,aN_)`, with `WildcardRestriction` filters and a dynamic RHS map.
It also supports `ConditionResult::Inconclusive`, whole-expression matching
through `partial(false)`, and bounded RHS caching.

It should not be the canonical recurrence representation:

- `WildcardRestriction::Filter`, `Cmp`, `Condition::match_stack`, and dynamic
  RHS maps contain Rust closures and are not portable rule data;
- default matching is partial and tree-recursive, which is unnecessary for a
  typed `Integral` key;
- unrestricted `.repeat()` needs an external termination budget;
- Atom ordering and process-local symbol IDs are not RustRed family IDs; and
- embedding integral keys into expression trees would make sparse elimination
  and dependency analysis more expensive.

If an Atom-facing API is needed, compile a verified `ParametricRule` into a
`Replacement` on demand.  Use a whole integral-function match, one application
at a time, and call the same typed guard evaluator from the map closure.  The
typed rule remains the serialized and verified source of truth.

The relevant audited Symbolica facilities are:

- `RationalPolynomial<IntegerRing,u16>` and automatic exact gcd cancellation
  in `vendor/symbolica/src/domains/rational_polynomial.rs`;
- `MultivariatePolynomial::{replace,replace_with_poly}` in
  `vendor/symbolica/src/poly/polynomial.rs`;
- `Pattern`, `Replacement`, `Condition`, `ConditionResult`,
  `WildcardRestriction`, and `MatchSettings` in
  `vendor/symbolica/src/id.rs`.

No Mathematica parser, kernel, rule evaluator, or generated source is needed
at runtime.

## 12. Errors and failure semantics

The checked API should distinguish at least:

```rust
pub enum ParametricIbpError {
    Context(ParametricContextError),
    OperatorOutOfRange { requested: usize, loops: usize },
    IdentityLimitExceeded { requested: usize, limit: usize },
    RawTermLimitExceeded { requested: usize, limit: usize },
    CoefficientLimit(ParametricArithmeticError),
}

pub enum ParametricSpecializationError {
    FamilyFingerprintMismatch,
    ContextMismatch,
    WrongArity { expected: usize, actual: usize },
    ExponentOverflow { seed: Integral, shift: IndexShift },
    ZeroSpecializedDenominator,
    ResourceLimit { kind: SpecializationLimitKind },
}

pub enum RuleReplayError {
    FamilyFingerprintMismatch,
    ContextMismatch,
    GuardNotSatisfied,
    UndecidableParameterGuard(SpecializedParameterGuard),
    MissingDescentCertificate,
    NonDescendingBranch { target: Integral, rhs: Integral },
    CycleDetected(Integral),
    StepLimitExceeded { limit: u64 },
    Specialization(ParametricSpecializationError),
    CertificateMismatch,
}
```

No error should relabel an uncovered point as a master.  `master` is a solver
coverage conclusion, not a fallback value from generation, specialization,
or replay.

## 13. Implementation and acceptance sequence

1. Add `ParametricCoefficientContext`, exact family-coefficient lifting, exact
   multi-index specialization, and context fingerprints in `coefficient.rs`.
2. Add `IndexSpace`, checked `IndexShift`, `ParametricCoefficient`, and
   `ParametricLinearCombination` without changing the concrete `Integral`.
3. Implement the seed-free `ParametricIbpGenerator` directly from
   `VacuumFamily::derivative_contraction`.
4. Prove raw specialization equality against the current concrete generator
   over exhaustive small boxes and selected boundary values.  Tests construct
   families normally; production code receives no frozen powers or answers.
5. Add exact translation/rebasing and verify translated rows by specialization
   at several independent integer assignments, followed by exact symbolic
   equality.
6. Add the serializable guard AST, pivot nonzero extraction, typed rule replay,
   and certificate replay.
7. Only after those foundations, connect adaptive sector-neighborhood
   elimination and patternization.  Concrete reductions and Vakint may serve
   as output oracles, but accepted rules must pass generic source-row replay.

The first milestone is complete when a topology supplied only through
`VacuumFamily` produces `L^2` symbolic rows and every tested concrete
specialization is exactly identical to the independently generated concrete
row.  No expected recurrence, rank, master, or final reduction coefficient is
part of that milestone's production input.
