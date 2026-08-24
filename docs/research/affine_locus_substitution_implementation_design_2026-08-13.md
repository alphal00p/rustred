# Bounded unit-affine composition and affine-locus-bound rows

Date: 2026-08-13

Status: implemented and independently re-audited.  The lower-level bounded
simultaneous composition slice lives in `parametric_coefficient.rs`; the
affine-locus-bound relation described in section 8 lives in
`affine_locus_bound_relation.rs`.  The composition plan, mapped values, and
context entry points remain crate-private by design.  Exhaustive algebra tests
live beside that private implementation, while
`tests/residual_unit_affine_composition.rs` checks only the public schema,
budget/stat vocabulary, typed errors, and provenance.  The raw lower-level row
does not escape; only the wrapper gives `J(F(t)+q)` its sound case-bound
meaning.  The final audit found no correctness or aggregate-accounting
blocker; its bounded caveats are recorded in section 13.

This design is based on the current RustRed and vendored Symbolica sources. It
is the next narrow slice after
[`ResidualUnitAffineIndexMapCertificate`](../../src/residual_unit_affine_index_map.rs)
and refines the substitution portion of
[`dependent_symbolic_start_design_2026-08-13.md`](dependent_symbolic_start_design_2026-08-13.md).

## 1. Decision

The next implementation should add one bounded, simultaneous composition
primitive for the certified map

```text
F(t) = b + A t
```

and use it only through a case-bound relation wrapper. For a generated global
identity

```text
R(n) = sum_s c_s(n) J(n+s) = 0,
```

an ambient prepare-point displacement `delta` must be applied first:

```text
R(n)
  --translate(delta)--> sum_s c_s(n+delta) J(n+delta+s)
  --compose(F)--------> sum_s c_s(F(t)+delta) J(F(t)+delta+s).
```

The retained sparse key is `q=delta+s`, with the enclosing wrapper giving it
the only sound meaning:

```text
q means J(F(t)+q), not a global J(n+q).
```

The implementation should use Symbolica's native
`MultivariatePolynomial::evaluate_with_coeff_map` into a
`PolynomialRing<IntegerRing,u16>` for genuinely simultaneous composition.
It must preflight expansion, exponent, integer-bit, and native work bounds
before entering that infallible/panic-capable API. Numerator and denominator
are composed separately; the mapped original denominator is made durable as a
guard before `RationalPolynomial::from_num_den(..., true)` may cancel it.

Do not add a public method which returns the composed inner
`ParametricRelation`. The complete source row and translation should be inputs
to one compiler entry point, so the unsound substitute-then-translate order is
not representable by the production API.

This slice remains topology- and loop-count-independent. The connected
two-loop equal-mass sunset is only a direct-concrete oracle fixture.

## 2. Audited current contracts

### 2.1 Certified map

The current
[`ResidualUnitAffineIndexMapCertificate`](../../src/residual_unit_affine_index_map.rs)
already supplies:

- the exact `K(n)` context fingerprint;
- the source coordinate-locus certificate, source case, predicate ordinal,
  and selected unit-pivot position;
- increasing canonical `free_positions` and literal positions;
- arbitrary-precision integer `constant(i)` and row-major
  `linear_coefficient(i,alpha)` accessors;
- a stable manifest, limits, work census, and full replay; and
- the V1 invariant that each image contains only the unchanged free original
  indices. Free rows are identities, literal rows are constants, and the one
  dependent bound row is integer affine.

Consequently, the simultaneous substitution

```text
sigma_F(n_i) = b_i + sum_alpha A[i,alpha] n[free[alpha]]
```

is idempotent and remains on the exact existing Symbolica variable map. There
is no need to create a second coefficient context or to call Symbolica's
automatic variable-map unification.

### 2.2 Existing coefficient and relation behavior

The current coefficient layer already gets several important details right:

- `ParametricPolynomial` and `ParametricCoefficient` authenticate the exact
  ordered `K(n)` map;
- `ParametricCoefficientContext::translate` translates numerator and
  denominator separately and reconstructs the rational polynomial;
- complete and partial specialization retain the mapped pre-normalization
  denominator as an explicit guard;
- `ParametricNonZeroCondition` stores a deterministic `BTreeSet<GuardOrigin>`;
- `ParametricRelation::translated` transforms keys, coefficients, and guards
  as one operation; and
- `PartialParametricRelationSpecialization` keeps its rebuilt relation private
  and exposes it only through a crate-private elimination accessor.

The affine implementation should mirror these contracts rather than expose a
raw Symbolica substitution.

### 2.3 Relevant Symbolica Rust API

The exact native APIs in the vendored source are:

- `MultivariatePolynomial::evaluate_with_coeff_map` at
  [`polynomial.rs:1890-1913`](../../vendor/symbolica/src/poly/polynomial.rs#L1890).
  It checks the point length with `assert_eq!`, maps each integer coefficient
  into a caller-selected ring, raises every image to the source exponent, and
  accumulates the result.
- `PolynomialRing::from_poly` at
  [`polynomial.rs:69-82`](../../vendor/symbolica/src/poly/polynomial.rs#L69).
- `MultivariatePolynomial::constant` and `variable`, which inherit the exact
  variable map, at
  [`polynomial.rs:398-455`](../../vendor/symbolica/src/poly/polynomial.rs#L398).
- `replace_with_poly` at
  [`polynomial.rs:1937-1962`](../../vendor/symbolica/src/poly/polynomial.rs#L1937).
  It is useful as a test oracle, but it is sequential and performs one
  separately normalized expansion per replacement. It should not be the V1
  production compositor.
- `RationalPolynomial`'s public numerator and denominator and
  `FromNumeratorAndDenominator::from_num_den` at
  [`rational_polynomial.rs:61-95`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L61).
  Integer-polynomial reconstruction with `do_gcd=true` cancels a polynomial
  GCD and normalizes the denominator sign at
  [`rational_polynomial.rs:406-443`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L406).

`evaluate_with_coeff_map` is preferable here to Atom replacement and pattern
matching. The input is already an authenticated sparse polynomial, and the
operation required is algebraic composition, not syntactic matching.

The native call still needs a RustRed boundary:

- point length is an assertion, not a `Result`;
- polynomial arithmetic can panic on `u16` exponent overflow;
- polynomial powers and multiplication allocate before RustRed can inspect
  the result;
- normalization can erase a mapped denominator factor; and
- Symbolica arithmetic may unify a mismatched map automatically unless
  RustRed validates every image and output itself.

## 3. Exact composition semantics

Let the complete Symbolica variable order be

```text
(theta_0,...,theta_(B-1), n_0,...,n_(N-1)).
```

Build a full point vector of polynomials on that same map:

```text
theta_r -> theta_r
n_i     -> b_i + sum_alpha A[i,alpha] n[free[alpha]].
```

Then compose one source polynomial `p` with:

```rust
let ring = PolynomialRing::<IntegerRing, u16>::from_poly(&context.template.numerator);
let mapped = source.evaluate_with_coeff_map(
    |integer| context.template.numerator.constant(integer.clone()),
    &plan.full_images,
    &ring,
);
```

The real implementation must keep `template` access inside
`parametric_coefficient.rs`, validate `full_images.len()` before this call,
preflight all work described below, wrap the call in `catch_unwind`, and
validate that `mapped.variables` is exactly the context's canonical `Arc`
map. It must also verify that every non-free index exponent is zero in the
output. The code above is only the central native call.

Because `evaluate_with_coeff_map` receives the complete point at once, the
semantics are simultaneous even if a future certificate admits more than one
bound row. V1's stronger invariant that all bound images mention only free
variables remains mandatory.

### 3.1 Why sequential replacement is not the proof boundary

For the current V1 certificate, sequential replacement of non-free variables
would happen to be order-independent because their images use only unchanged
free variables. That is a derived invariant, not a good public contract.
Using the full-point evaluator:

- directly states simultaneous composition;
- cannot accidentally rewrite a variable inside a previously installed
  image;
- gives one common implementation for base identities, free identities,
  literal constants, and the dependent affine row; and
- remains valid if a later certified triangular map contains several bound
  rows after it has been normalized to free-variable images.

`replace_with_poly` should still appear in a small independent behavior test
which confirms the full-point result on safe inputs.

## 4. Compile-ready lower-level API

The new coefficient implementation should live in
[`parametric_coefficient.rs`](../../src/parametric_coefficient.rs), where the
authenticated raw fields, template, and variable maps are already private.
The following API is intentionally crate-private; the affine relation compiler
is its production consumer.

```rust
pub const RESIDUAL_UNIT_AFFINE_COMPOSITION_V1_SCHEMA: &str =
    "rustred-residual-unit-affine-composition-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidualUnitAffineCompositionPlanLimits {
    pub max_variables: usize,
    pub max_full_images: usize,
    pub max_total_image_terms: usize,
    pub max_total_image_exponent_entries: usize,
    pub max_image_integer_bits: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidualUnitAffinePolynomialCompositionLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_source_terms: usize,
    pub max_source_exponent_entries: usize,
    pub max_expanded_contributions: usize,
    pub max_output_terms: usize,
    pub max_output_exponent_entries: usize,
    pub max_power_calls: usize,
    pub max_native_power_heap_pairs: usize,
    pub max_multiplication_term_pairs: usize,
    pub max_addition_term_visits: usize,
    pub max_kronecker_exponent_bits: usize,
    pub max_integer_coefficient_bits: usize,
    pub max_integer_bit_work: usize,
    pub max_normalization_input_term_pairs: usize,
    pub max_guard_origins: usize,
    pub max_guard_origin_retained_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidualUnitAffinePolynomialCompositionStats {
    source_terms: usize,
    source_exponent_entries: usize,
    expanded_contribution_bound: usize,
    output_terms: usize,
    output_exponent_entry_bound: usize,
    output_exponent_entries: usize,
    power_calls: usize,
    native_power_heap_pair_bound: usize,
    multiplication_term_pair_bound: usize,
    addition_term_visit_bound: usize,
    largest_kronecker_exponent_bits: usize,
    largest_integer_coefficient_bit_bound: usize,
    native_integer_bit_work_bound: usize,
    integer_bit_work_bound: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ResidualUnitAffineCompositionPlan {
    schema: &'static str,
    context_fingerprint: Arc<str>,
    map: Arc<ResidualUnitAffineIndexMapCertificate>,
    full_images: Box<[CoefficientPolynomial]>,
    image_term_counts: Box<[usize]>,
    // ceil(log2(max(abs(coefficient)))) with 0 for coefficients in {-1,0,1}.
    image_coefficient_growth_bits: Box<[usize]>,
    limits: ResidualUnitAffineCompositionPlanLimits,
}

#[derive(Clone, Debug)]
pub(crate) struct ResidualUnitAffinePolynomialComposition {
    value: ParametricPolynomial,
    stats: ResidualUnitAffinePolynomialCompositionStats,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidualUnitAffineCoefficientCompositionStats {
    numerator: ResidualUnitAffinePolynomialCompositionStats,
    denominator: ResidualUnitAffinePolynomialCompositionStats,
    aggregate: ResidualUnitAffinePolynomialCompositionStats,
    durable_guard_terms: usize,
    durable_guard_exponent_entries: usize,
    durable_guard_integer_bit_payload: usize,
    durable_guard_origin_retained_bytes: usize,
    total_integer_bit_work_bound: usize,
    normalization_input_term_pairs: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct GuardedResidualUnitAffineCoefficientComposition {
    value: ParametricCoefficient,
    mapped_denominator: Option<ParametricNonZeroCondition>,
    stats: ResidualUnitAffineCoefficientCompositionStats,
}

#[derive(Clone, Debug)]
pub(crate) enum ResidualUnitAffineCoefficientComposition {
    Available(GuardedResidualUnitAffineCoefficientComposition),
    ZeroMappedDenominator {
        stats: ResidualUnitAffineCoefficientCompositionStats,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum ResidualUnitAffineConditionClass {
    Unsatisfiable,
    NonzeroIntegerConstant,
    BaseAssumption(ParametricNonZeroCondition),
    IndexDependent(ParametricNonZeroCondition),
}

#[derive(Clone, Debug)]
pub(crate) struct ResidualUnitAffineConditionComposition {
    class: ResidualUnitAffineConditionClass,
    stats: ResidualUnitAffinePolynomialCompositionStats,
}
```

As in the existing certificate types, every stats field gets a read-only
`const` getter and every result type gets only the getters needed by its
crate-private caller. Do not make `full_images` mutable after plan validation.
All public limit types need explicit finite `Default` implementations aligned
with the existing `ExactAlgebraLimits`/`ParametricArithmeticLimits`; do not use
`usize::MAX` as the production default merely to make a probe pass. Re-export
the public limits, stats, errors, and wrapper/compiler outcomes from
`lib.rs`, but keep the plan and composed raw values crate-private.

The context methods should be:

```rust
impl ParametricCoefficientContext {
    pub(crate) fn compile_residual_unit_affine_composition_plan(
        &self,
        map: Arc<ResidualUnitAffineIndexMapCertificate>,
        limits: ResidualUnitAffineCompositionPlanLimits,
    ) -> Result<ResidualUnitAffineCompositionPlan,
                ResidualUnitAffineCompositionError>;

    pub(crate) fn compose_polynomial_on_residual_unit_affine_map(
        &self,
        source: &ParametricPolynomial,
        plan: &ResidualUnitAffineCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualUnitAffinePolynomialComposition,
                ResidualUnitAffineCompositionError>;

    pub(crate) fn compose_coefficient_on_residual_unit_affine_map(
        &self,
        source: &ParametricCoefficient,
        plan: &ResidualUnitAffineCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualUnitAffineCoefficientComposition,
                ResidualUnitAffineCompositionError>;

    pub(crate) fn compose_nonzero_condition_on_residual_unit_affine_map(
        &self,
        source: &ParametricNonZeroCondition,
        plan: &ResidualUnitAffineCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualUnitAffineConditionComposition,
                ResidualUnitAffineCompositionError>;
}
```

The dedicated error should retain stage information rather than flattening
everything into a Symbolica string:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualUnitAffineCompositionError {
    SchemaMismatch,
    WrongContext,
    WrongArity { expected: usize, actual: usize },
    NonFreeIndexSurvived { position: usize },
    ExponentLimit {
        source_term: usize,
        target_variable: usize,
        requested: u128,
        limit: u128,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow { resource: &'static str },
    AllocationFailure { resource: &'static str, requested: usize },
    SymbolicaPanic { stage: &'static str },
    Map(ResidualUnitAffineIndexMapError),
    Coefficient(ParametricCoefficientError),
}
```

`ZeroMappedDenominator` and an identically zero mapped guard are semantic
outcomes with an exact work census, not algebra errors. This avoids the
current partial-specialization problem where an unsatisfiable result has no
stats and an outer aggregate budget has to charge the complete per-call
allowance.

## 5. Plan construction

Plan compilation should do the following once per map, not once per row term:

1. Compare the context fingerprint and arity, then call `map.replay(context)`.
2. Preflight the complete point length `B+N`, total image terms, retained
   exponent entries, and maximum integer bit length before allocating.
3. Build identity images for every base variable.
4. Build each index image directly on `context.template.numerator`:

   ```text
   constant b_i, if nonzero
   + A[i,alpha] * n[free[alpha]], for each nonzero A entry.
   ```

   Append each monomial on the canonical map; do not parse an Atom and do not
   call `unify_variables`.
5. Validate free rows exactly: zero constant and one identity coefficient.
   Validate every image mentions only a certified free index.
6. Record each image's sparse term count and largest integer coefficient bit
   length for the polynomial preflight.

The plan is an ephemeral cache. The persisted proof remains the replayable map
certificate. An affine row certificate stores the map, not a second trusted
copy of the image polynomials.

Change the map certificate's existing private `payload_eq` to
`pub(crate) fn payload_eq`. The affine wrapper needs complete replay equality,
including the typed source coordinate certificate, configured limits, and
stats. The current in-progress map manifest serializes the complete source
partition and is therefore not compact; textual equality still should not
replace typed payload equality. In the planned source-owned-identity
refactor, the large partition identity should be shared once and the local map
manifest should become compact without changing the wrapper's comparison
contract.

## 6. Polynomial preflight

All counts below are computed before `evaluate_with_coeff_map` allocates an
output.

Let `w_i` be the number of nonzero monomials in image `i`. For an affine index
image this is the count of its nonzero constant and nonzero free-variable
coefficients. Base and free identity images have `w_i=1`; an identically zero
literal image can have `w_i=0`.

For source exponent `e` and image support `w>0`, a sparse linear form raised
to `e` has at most

```text
H(e,w) = binomial(e+w-1, w-1)
```

monomials. Compute this with a capped multiplicative binomial routine. It must
return a typed `CappedCount::Exceeded` as soon as the configured cap is
exceeded and must never construct the uncapped binomial. Do not encode that
state unconditionally as `limit+1`, which overflows at `usize::MAX`. With
`e<=u16::MAX` and map arity already bounded, a `u128` intermediate is
sufficient after each step is capped; all additions and products still need
checked arithmetic.

For one source monomial `m`, its pre-collection contribution bound is:

```text
C_m = product over variables i with exponent e_i>0 of H(e_i,w_i).
```

If any such `w_i` is zero, the monomial maps to zero and `C_m=0`. The complete
expanded-contribution bound is:

```text
C = sum_m C_m.
```

Require `C` to fit `max_expanded_contributions`, `max_output_terms`, and
`exact_algebra.max_polynomial_terms`.
The actual collected output may be much smaller; stats must record both `C`
and the actual `mapped.nterms()` so claims do not confuse a work bound with a
result cardinality. Preflight `C * variable_count` against
`max_output_exponent_entries` as well.

### 6.1 Exponent overflow

For every surviving source monomial and every free target coordinate
`f_alpha`, the largest possible target exponent is exactly bounded by:

```text
sum_i e_source[i] * indicator(A[i,alpha] != 0),
```

where the sum runs over all original index rows, including the free identity
row. Equivalently, it is `e_source[f_alpha]` plus the same sum restricted to
non-free rows. Do not include the free row in both terms.

Base-variable exponents are unchanged and every non-free index target degree
must be zero. Check these sums in `u128` against both
`limits.exact_algebra.max_exponent` and
`SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT` before calling Symbolica. This catches,
for example, a source monomial `n_free^65535 * n_bound` when the bound image
contains `n_free`; Symbolica's `u16` multiplication would otherwise panic.

### 6.2 Native power and multiplication work

`evaluate_with_coeff_map` performs one power and one multiplication for every
nonzero source exponent. Record that as `power_calls`.

The current vendored `MultivariatePolynomial::pow` uses `heap_pow` for a
nonconstant affine linear image because each variable degree is at most one.
For an image with `w` terms and power output bound `H`, conservatively charge
`w*H` heap pairs. Sum this across every source occurrence and enforce
`max_native_power_heap_pairs`.

For each source monomial, walk images in the exact full-point order used by
the native call. If the current accumulated term bound is `P` and the next
power bound is `H`, charge `P*H` multiplication term pairs and update
`P=P*H`. Sum with checked arithmetic. Identity monomials still count as one,
but can use the native monomial fast path.

The evaluator implements accumulation as `res = res + term`. It can copy the
growing result on every source monomial. Charge, in source order,

```text
prefix_bound + C_m
```

for every addition and enforce `max_addition_term_visits`. This catches a
quadratic accumulation pattern even when the final collected term count is
small.

Symbolica's dense multiplication path has an audited hard ceiling of
`1<<24` dense slots in this vendored revision
([`polynomial.rs:27,2760-2800`](../../vendor/symbolica/src/poly/polynomial.rs#L2760));
larger boxes fall back to heap multiplication. This is a fixed backend peak,
not a caller-configurable RustRed guarantee. Document it in memory estimates,
and keep term-pair limits as the caller-controlled work boundary. If a future
backend revision removes that native ceiling, the compile probe must fail
until RustRed adds an explicit dense-box preflight or a controlled compositor.

### 6.3 Integer coefficient growth

For one affine image with `w` nonzero coefficients, define

```text
G = ceil(log2(max(1, max absolute image coefficient))).
```

Thus `G=0` for images whose coefficients are all `-1` or `1`. Every
contribution to the image's `e`th power is bounded by:

```text
e * (G + ceil(log2(w))) bits of growth.
```

For a source term with integer coefficient bit length `S`, sum that expression
over all powered images. Take the largest resulting contribution bound across
the source terms, then add `ceil(log2(C))` for worst-case collection of all
expanded contributions (with the zero-polynomial case handled separately).
Check the resulting per-output coefficient bound against
`max_integer_coefficient_bits`. Using ordinary magnitude bit length for `G`
would charge one bit per power of `1` and needlessly reject high-degree
identity images.

The native evaluator clones each source coefficient and evaluates every
nonzero-exponent image before multiplying it into the term.  This remains
true when an earlier or later zero image makes `C_m=0`.  Therefore validate
the source coefficient bit length unconditionally and charge it once. For
every powered nonzero image, charge the final coefficient bound across its
`H(e,w)` outputs. For the audited `heap_pow` path, additionally include the
image-coefficient growth, Kronecker-encoded exponent bits, and
`ceil(log2(w*H))` collision allowance across the conservative heap-pair and
denominator-step census. Then, in exact evaluator order, charge every
prefix-times-power pair at the accumulated prefix width plus final power
width, and charge every nonzero sparse output addition visit at the larger
operand width plus one collision bit.

The simpler final-power component is

```text
H(e,w) * (1 + e * (G + ceil(log2(w))))
```

Validate every power, heap-recurrence, multiplication, and addition temporary
bound against `max_integer_coefficient_bits`; record their total in
`native_integer_bit_work_bound`. These charges may not be skipped merely
because the final prospective contribution count becomes zero.

Let `bit_bound_m` include the global collision allowance. Charge
`C_m * bit_bound_m` in addition to the unconditional native charge, record
their checked sum as `integer_bit_work_bound`, and enforce
`max_integer_bit_work`. Counting only power calls is not sufficient: one high
exponent and one large affine constant can allocate a very large GMP integer.

`heap_pow` uses an integer Kronecker encoding whose degree radix is roughly
the product of `(degree+1)` over target variables. Accumulate the sum of
`ceil(log2(degree+1))` using the already checked target-degree bounds and
enforce `max_kronecker_exponent_bits` before the native power call.
The audited backend stores its running stride in `u32`; preflight the exact
radix product against `u32::MAX` independently of the configurable bit limit.

### 6.4 Execute and authenticate

Only after the complete preflight succeeds:

1. enter `catch_unwind(AssertUnwindSafe(...))` around the simultaneous native
   evaluation;
2. reject a panic as `SymbolicaPanic { stage: "unit-affine polynomial composition" }`;
3. check the actual output term limit again;
   reject `mapped.nterms()>C` as an internal replay/invariant failure;
4. validate the exact canonical variable map and exact-algebra invariants;
5. scan all output exponent rows and reject any non-free index occurrence;
6. wrap the result as an authenticated `ParametricPolynomial`; and
7. return actual and prospective stats.

No output or budget mutation is committed before the operation succeeds.

## 7. Rational coefficients and guards

### 7.1 Coefficients

For `a(n)=P(n)/Q(n)`:

1. preflight both `P` and `Q` against one remaining aggregate allowance;
2. compose them independently through the same immutable plan;
3. if `sigma_F(Q)` is identically zero, return
   `ZeroMappedDenominator { stats }`;
4. before normalization, construct a condition
   `sigma_F(Q) != 0` unless it is a nonzero integer constant;
5. attach typed denominator and affine-map provenance;
6. preflight
   `max(1,mapped_num.nterms())*mapped_den.nterms()` against exact
   normalization input work; using an unadjusted product would charge zero
   for `0/Q` even though GCD normalization may still process `Q`;
7. reconstruct with `from_num_den(mapped_num,mapped_den,&Z,true)` inside
   `catch_unwind`; and
8. validate the normalized value on the unchanged map.

Step 4 needs one durable copy of the mapped denominator while normalization
consumes the numerator/denominator pair. Preflight its term, exponent-entry,
integer, and retained-byte payload first, then copy through a checked
`try_reserve_exact` helper. A direct `Vec::clone` would allocate before RustRed
can return an `AllocationFailure` and would defeat the otherwise explicit
resource boundary.

The guard is retained even if `P=0`, even if `P` contains the same factor, or
if normalization cancels all of `Q`. This matches the guarded division and
partial-specialization contracts already used by RustRed.

The current Symbolica polynomial-GCD call is not cooperatively interruptible.
The input term-pair preflight bounds the normalized input shape and matches
RustRed's existing exact-algebra contract, but it is not a literal bound on
every internal modular-GCD operation or wall-clock time. Name this counter
`normalization_input_term_pairs`, catch backend panics, and do not describe it
as an exact native-operation census. A strict preemptible GCD budget would
require a separately designed bounded/modular normalizer or a worker-process
boundary; using `from_num_den(..., false)` is not an acceptable shortcut
because noncanonical fractions would break exact relation collection.

### 7.2 Existing nonzero conditions

For a source condition `g(n)!=0`, compose its polynomial and retain all source
origins. Add one affine-substitution origin. Classify the result as:

- `Unsatisfiable` when `sigma_F(g)=0`;
- `NonzeroIntegerConstant` when it is a nonzero integer constant;
- `BaseAssumption` when it is independent of every index but is not an
  integer unit; or
- `IndexDependent` otherwise.

The relation compiler drops only the nonzero integer constant, stores base
assumptions separately, and inserts index-dependent conditions into the
private inner relation. An unsatisfiable source guard makes this row
unavailable on the affine locus; it does not prove that the affine locus or
its integrals are zero.

Before retaining a base assumption, add the same
`RelationConditionAttached { row: target_row }` origin that ordinary relation
insertion would add. Equal base polynomials merge their complete origin sets
under the configured origin budget.

An origin-count limit alone does not bound boxed shifts or derived-row labels.
Before copying an existing origin set, compute a checked retained-byte census
covering the enum/node allowance, boxed slices, and shared label bytes; enforce
`max_guard_origin_retained_bytes`, and return the exact census with the
condition composition. `BTreeSet` node allocation itself is not fallible on
stable Rust, so this is a finite input/allocation-shape proof rather than a
promise that process-wide allocator exhaustion can be recovered.

### 7.3 Provenance additions

Add small, stable `GuardOrigin` variants keyed by the map's replay locator,
not a copy of its potentially large manifest:

```rust
ResidualUnitAffineIndexSubstitution {
    source_case: u64,
    predicate_ordinal: usize,
    bound_position: usize,
},
CoefficientResidualUnitAffineSubstitutionDenominator {
    source_case: u64,
    predicate_ordinal: usize,
    bound_position: usize,
},
RelationResidualUnitAffineSubstitutionTermDenominator {
    row: GuardRowId,
    shift: Box<[i64]>,
    source_case: u64,
    predicate_ordinal: usize,
    bound_position: usize,
},
RelationResidualUnitAffineSubstitution {
    row: GuardRowId,
    source_case: u64,
    predicate_ordinal: usize,
    bound_position: usize,
},
```

The enclosing bound certificate owns and replays the complete map manifest,
so these origins are locators inside that proof rather than independent map
authentication. This avoids repeating an arbitrarily large manifest in every
guard while preserving a deterministic audit trail. All variants need
streaming `write_stable` arms and origin-cardinality preflight before
insertion.

## 8. Private affine-locus-bound relation

Add a new module `src/affine_locus_bound_relation.rs`. The public object is a
certificate; its algebraic relation remains private.

```rust
pub const AFFINE_LOCUS_BOUND_PARAMETRIC_RELATION_V1_SCHEMA: &str =
    "rustred-affine-locus-bound-parametric-relation-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffineLocusBoundRelationLimits {
    pub translation: ParametricArithmeticLimits,
    pub plan: ResidualUnitAffineCompositionPlanLimits,
    pub composition: ResidualUnitAffinePolynomialCompositionLimits,
    pub max_terms: usize,
    pub max_source_guards: usize,
    pub max_polynomial_compositions: usize,
    pub max_base_assumptions: usize,
    pub max_retained_guards: usize,
    pub max_target_row_label_bytes: usize,
    pub max_translation_polynomials: usize,
    pub max_total_translation_source_term_allowance: usize,
    pub max_total_translation_output_term_allowance: usize,
    pub max_total_translation_power_operation_allowance: usize,
    pub max_total_translation_integer_bit_allowance: usize,
    pub max_total_source_terms: usize,
    pub max_total_source_exponent_entries: usize,
    pub max_total_expanded_contributions: usize,
    pub max_total_output_terms: usize,
    pub max_total_output_exponent_entry_bound: usize,
    pub max_total_power_calls: usize,
    pub max_total_native_power_heap_pairs: usize,
    pub max_total_multiplication_term_pairs: usize,
    pub max_total_addition_term_visits: usize,
    pub max_total_integer_bit_work: usize,
    pub max_total_guard_origin_retained_bytes: usize,
    pub max_total_normalization_input_term_pairs: usize,
    pub max_retained_terms: usize,
    pub max_retained_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AffineLocusBoundRelationStats {
    source_terms: usize,
    source_guards: usize,
    translated_terms: usize,
    polynomial_compositions: usize,
    base_assumptions: usize,
    retained_guards: usize,
    target_row_label_bytes: usize,
    translation_polynomials: usize,
    translation_source_term_allowance: usize,
    translation_output_term_allowance: usize,
    translation_power_operation_allowance: usize,
    translation_integer_bit_allowance: usize,
    composition_source_terms: usize,
    composition_source_exponent_entries: usize,
    expanded_contribution_bound: usize,
    composition_output_terms: usize,
    composition_output_exponent_entry_bound: usize,
    composition_output_exponent_entries: usize,
    power_calls: usize,
    native_power_heap_pair_bound: usize,
    multiplication_term_pair_bound: usize,
    addition_term_visit_bound: usize,
    largest_kronecker_exponent_bits: usize,
    largest_integer_coefficient_bit_bound: usize,
    native_integer_bit_work_bound: usize,
    durable_guard_terms: usize,
    durable_guard_exponent_entries: usize,
    durable_guard_integer_bit_payload: usize,
    guard_origin_retained_bytes: usize,
    integer_bit_work_bound: usize,
    normalization_input_term_pairs: usize,
    retained_terms: usize,
    retained_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffineLocusBaseAssumption {
    condition: ParametricNonZeroCondition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffineLocusUnavailableReason {
    SourceGuardComposesToZero { guard_ordinal: usize },
    SourceTermDenominatorComposesToZero {
        term_ordinal: usize,
        shift: IndexShift,
    },
}

#[derive(Clone, Debug)]
pub struct AffineLocusUnavailableRowCertificate {
    schema: &'static str,
    source: Arc<ParametricRelation>,
    translation: IndexShift,
    target_row_id: ParametricRowId,
    affine_map: Arc<ResidualUnitAffineIndexMapCertificate>,
    reason: AffineLocusUnavailableReason,
    limits: AffineLocusBoundRelationLimits,
    stats: AffineLocusBoundRelationStats,
}

#[derive(Clone, Debug)]
pub struct AffineLocusBoundParametricRelation {
    schema: &'static str,
    source: Arc<ParametricRelation>,
    translation: IndexShift,
    target_row_id: ParametricRowId,
    affine_map: Arc<ResidualUnitAffineIndexMapCertificate>,
    relation: ParametricRelation, // deliberately private
    base_assumptions: Box<[AffineLocusBaseAssumption]>,
    limits: AffineLocusBoundRelationLimits,
    stats: AffineLocusBoundRelationStats,
}

#[derive(Clone, Debug)]
pub enum AffineLocusBoundRelationCompilation {
    Retained(AffineLocusBoundParametricRelation),
    Unavailable(AffineLocusUnavailableRowCertificate),
}

pub struct AffineLocusBoundRelationCompiler;

impl AffineLocusBoundRelationCompiler {
    pub fn compile(
        context: &ParametricCoefficientContext,
        source: Arc<ParametricRelation>,
        translation: IndexShift,
        target_row_id: ParametricRowId,
        affine_map: Arc<ResidualUnitAffineIndexMapCertificate>,
        limits: AffineLocusBoundRelationLimits,
    ) -> Result<AffineLocusBoundRelationCompilation,
                AffineLocusBoundRelationError>;
}
```

The corresponding error surface should be explicit and compile without
string-matching lower-level failures:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffineLocusBoundRelationError {
    SchemaMismatch,
    ReplayMismatch,
    WrongContext,
    WrongArity { expected: usize, actual: usize },
    ConcreteFreeValueArity { expected: usize, actual: usize },
    ConcreteAffineValueOutOfRange { position: usize },
    ConcretePointOutsideSourceOrthant,
    ConcretePointOutsideSourceCase { predicate_ordinal: usize },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow { resource: &'static str },
    AllocationFailure { resource: &'static str, requested: usize },
    Composition(ResidualUnitAffineCompositionError),
    Relation(ParametricRelationError),
}
```

`From` implementations should preserve the typed composition and relation
errors. The two unavailable-domain reasons remain successful compilation
outcomes and therefore do not appear in this error enum.

Public getters may expose schema, source metadata, translation, target row id,
map, base assumptions, limits, stats, and replay. They must not expose
`relation` or provide a conversion to a global reduction candidate.

The only raw accessor is:

```rust
pub(crate) fn relation_for_affine_reelimination(&self) -> &ParametricRelation;
```

It is for an elimination object which is itself bound to the identical affine
map manifest. Rows with different map manifests must never enter the same
elimination database.

### 8.1 Compiler order

The compiler must implement exactly this order:

1. validate source context, translation arity, and map context;
2. replay the map;
3. call the existing complete-row `source.translated(...)`;
4. compile one composition plan;
5. rebuild every translated source guard through the plan;
6. rebuild every translated coefficient through the plan while leaving the
   translated `IndexShift q` unchanged;
7. classify mapped guards and mapped original denominators into inner guards
   or base assumptions;
8. return an exact unavailable-row certificate at the first zero guard or
   zero mapped denominator, after consuming its returned work stats;
9. compute retained logical terms/bytes under aggregate limits; and
10. replay the result before returning it from the public constructor.

The compiler must not offer an entry point which accepts an already composed
row plus a translation. A private helper may accept the already translated
temporary, but only this compiler calls it.

Every mapped source guard receives the relation-level affine-substitution
origin in addition to its coefficient-level map origin. Every mapped original
term denominator receives the term-denominator origin with the translated
shift `q`. Add these atoms before merging equal conditions so a later
duplicate keeps all source and term locators.

### 8.2 Replay

Replay repeats map replay, complete-row translation, plan construction, and
all compositions. A retained result compares:

- source relation including complete guard provenance;
- translation, target row id, and complete affine-map payload;
- inner relation using `has_identical_guard_provenance`;
- base assumptions;
- limits and complete stats.

An unavailable certificate repeats the same work and must reproduce the exact
reason locator and stats. A zero mapped denominator is not stored merely as a
string error.

### 8.3 Safe concrete specialization

The wrapper should expose one safe validation/query operation without
exposing its inner row:

```rust
pub fn specialize_at_free_values(
    &self,
    context: &ParametricCoefficientContext,
    free_values: &[i64],
    limits: AffineLocusConcreteSpecializationLimits,
) -> Result<ConcreteRelation, AffineLocusBoundRelationError>;
```

with:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffineLocusConcreteSpecializationLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_free_positions: usize,
    pub max_affine_integer_bits: usize,
    pub max_source_case_predicates: usize,
    pub max_source_case_predicate_terms: usize,
}
```

It checks `free_values.len()==map.free_positions().len()`, evaluates every
ambient component

```text
x_i = b_i + sum_alpha A[i,alpha] free_values[alpha]
```

with `Integer`, enforces a caller-supplied integer-bit budget, converts each
component to `i64` with a typed representation-boundary error, and calls the
private relation's ordinary concrete specialization at the full ambient point
`x=F(t)`. Before specializing, it must clone the private row under the query
budget and reattach every stored base assumption as a guarded condition; a
plain specialization of only the inner relation would silently lose part of
the row domain. Its integral keys are therefore exactly `F(t)+q`, and its
concrete guard set contains both index-dependent guards and the separately
retained formal-base assumptions.

Before returning an applicable case-bound query, it also checks the source
partition's orthant and every predicate of `map.source_case()` at `x`, under
the predicate-count/term budgets above. Integer specialization leaves a base
polynomial: `EqualZero` accepts only the identically zero base polynomial and
`NonZero` accepts only a nonzero one in the formal base field. A point outside
the source case receives the typed error above. A crate-private test helper may
specialize the algebraic inner identity without this case check when testing a
global-identity invariant, but the public query must retain the case boundary.

Failure to fit `i64` is a concrete query boundary, not a failure of the
symbolic affine row.

## 9. Aggregate resource and transaction model

The lower-level per-polynomial limits are necessary but not sufficient. A row
may contain millions of individually small coefficients. The relation
compiler must maintain a mutable aggregate budget and restrict each next
composition to the remaining allowance, just as the current generated
cylindrical row system restricts partial specialization.

On every successful or semantic-unavailable composition, consume the exact
returned census. A resource or algebra error commits nothing. Use checked
addition for every aggregate counter.

Before complete-row translation, preflight the number of translated
coefficient polynomials and guards. The existing translation API does not
return polynomial work stats. Until that API gains a census, conservatively
charge its complete per-polynomial allowances across:

```text
2 * relation_terms + relation_guards
```

polynomials, and state clearly that translation counters are allowance bounds,
not measured work.  The implementation also preflights the complete origin
copy/rebuild path before translation: source-set copies, index/relation
translation atoms, attachment atoms, translated input-denominator atoms, and
worst-case duplicate-polynomial merges.  These provenance byte counters are
deliberately conservative and can reject early; they do not undercount the
audited paths.  Do not claim an aggregate translation allocator cap from only
`translated.terms().len()`.

Logical retained bytes must include:

- inner relation shifts, numerator/denominator coefficients, exponent rows,
  guard polynomials, and every guard origin;
- base-assumption polynomials and origins;
- translation and target-row label;
- map/source `Arc` pointer overhead and any owned replay metadata; and
- unavailable reason payload when applicable.

The source relation and map are shared behind `Arc`; their full payload bytes
must be charged once by the owning generated row-system certificate, not once
per expanded row. Per-row stats should report shared referenced bytes
separately from owned retained bytes. This avoids both unbounded hidden clones
and grossly multiplying a shared row-span budget.

The low-level map certificate is context/case-bound but carries no independent
family fingerprint. That is sufficient for the algebraic statement “restrict
this global identity through this same-context affine map,” but not for a
claim that the map was discovered by this family's residual queue. The later
generated affine row-system certificate must prove that stronger lineage by
resolving the map's source coordinate certificate from its owned queue/work
item and comparing the complete payload. Do not infer family lineage merely
from equal arity or a caller-owned context name.

The peak working set temporarily contains the translated row, composition
plan, one mapped numerator/denominator pair, and the growing inner row. The
documented retained-byte limit is not by itself a peak allocator cap. Expose a
`max_peak_working_term_bound` at the generated row-system layer before using
this at large prepare depth.

## 10. Direct-concrete generated-sunset oracle

The first integration test should be
`tests/affine_locus_bound_relation_sunset.rs`, run with the licensed,
GMP-enabled Symbolica build. No FORM, Mathematica, hardcoded recurrence, or
`no_gmp` feature is involved.

### 10.1 Fixture

1. Construct the connected equal-mass two-loop sunset as an ordinary
   `IntegralFamily`, using the same generic fixture shape as
   [`generated_symbolic_row_span.rs`](../../tests/generated_symbolic_row_span.rs).
2. Run `ParametricIbpGenerator::try_new(&family).generate()` and take all four
   generated canonical IBP rows. No expected reduction rule is supplied.
3. In test code only, build an authenticated symbolic sector case containing

   ```text
   (d+1) * (n0+n1-3) = 0.
   ```

   The base factor exercises associate recognition. Extract its exact
   coordinate-locus certificate and compile the existing unit-affine map with
   bound position zero:

   ```text
   F(t,u) = (3-t, t, u), free positions [1,2].
   ```

   This test-only case validates the generic mechanism; it does not claim the
   current search scheduler naturally discovers that particular equality.
4. Use several ambient displacements, including zero and displacements with
   unequal first/second components, for example:

   ```text
   (0,0,0), (1,0,0), (0,-1,1), (-1,1,0), (2,-1,0).
   ```

5. For every generated row and displacement, compile and replay an
   affine-locus-bound row.

### 10.2 Oracle identity

For free values such as

```text
(t,u) in {(1,1), (2,1), (1,2), (2,2)},
```

construct `x=F(t,u)` and compare:

```text
affine_bound.specialize_at_free_values(t,u)
```

against direct concrete specialization of the original generated row at:

```text
x + delta.
```

The comparison must include:

- the complete collected `ConcreteIntegralKey -> Coefficient` map;
- the complete polynomial set of concrete nonzero conditions;
- success versus unsatisfiable-domain outcome; and
- exact family/context identity.

The row ids differ by construction, so compare the mathematical maps rather
than `ConcreteRelation::PartialEq`, which includes the row id.

### 10.3 Provenance comparison is path-aware

Do **not** assert literal equality of complete origin sets between the two
routes. The direct route correctly records

```text
IndexSpecialization { assignment: x+delta },
```

whereas the affine route correctly records translation, affine substitution,
and then

```text
IndexSpecialization { assignment: x }.
```

Their guard polynomials and domains must agree, while their derivation paths
are intentionally different. Test provenance independently in a
module-private unit test which can inspect the private bound row:

- every source origin survives;
- translated guards include `IndexTranslation` and `RelationTranslation`;
- mapped guards include `ResidualUnitAffineIndexSubstitution`;
- mapped original coefficient denominators include the dedicated denominator
  origin; and
- relation-attached and final concrete-specialization origins are present.

A supplemental derived row should scale one genuine generated sunset row by
a guarded rational factor such as `1/(d+n0+n2-2)`. This is not a recurrence;
it only ensures the oracle exercises a mapped pre-cancellation denominator
which remains a nonconstant base-field guard after concrete index
specialization, even if the canonical generated rows happen to have
polynomial coefficients.

### 10.4 Operation-order sensitivity

Include a small focused check with

```text
p(n)=n0,
F(t,u)=(3-t,t,u),
delta=(1,1,0).
```

Translate then compose gives `4-t`; compose then ambient-translate gives
`2-t`. The production compiler must match the first. This prevents the large
sunset oracle from passing accidentally if a selected row/displacement has a
coefficient insensitive to the order.

## 11. Adversarial tests

Add focused unit/integration tests for all proof boundaries:

1. **Simultaneous map:** compare the native full-point compositor with a
   sequential `replace_with_poly` oracle on V1-safe maps, including overlapping
   free-variable support.
2. **Literal plus dependent:** use `n0=3-n1` together with a literal `n2=2`
   and prove both positions disappear from output exponents.
3. **Mapped-zero guard:** impose `n0+n1-3 != 0` on the equality map and require
   an unavailable-row certificate, not an empty-locus or master result.
4. **Mapped-zero denominator:** at coefficient level, compose
   `1/(n0+n1-3)` and require `ZeroMappedDenominator`. In a public relation,
   ordinary term insertion already retains that denominator as a source
   guard, so the declared guard-first compiler order should deterministically
   report `SourceGuardComposesToZero`; the term-denominator check remains a
   defense-in-depth boundary.
5. **Cancellation:** compose `(n0+n2-2)/(n0+n2-2)` and retain the mapped
   nonconstant denominator guard `1-n1+n2 != 0` even though normalization
   returns one.
6. **Zero numerator:** compose `0/(n0+n2-2)` and retain its denominator guard.
7. **Base assumption:** map the guard `n0+n1-3+d != 0` to `d != 0` and retain
   it outside the index-case guards.
8. **Expansion limit:** map `n0^e` through `n0=3-n1` and set the contribution
   cap below `e+1`; reject before Symbolica evaluation.
9. **Exponent overflow:** combine `n_free^u16::MAX` with a bound variable whose
   image contains that free variable and reject the prospective exponent
   `65536` before native multiplication.
10. **Integer bits:** use a large certified affine constant and high exponent;
    hit the integer-bit limit before GMP power allocation.
11. **Aggregate limit:** two individually admissible coefficients must fail a
    row-wide bound when their combined census exceeds it.
12. **Wrong context/map:** reject a same-arity foreign `K(n)` scope before
    building images.
13. **Replay tamper:** alter translation, target row id, source row, outcome
    locator, or map reference and require replay mismatch.
14. **Concrete representation boundary:** allow the symbolic row when an
    affine component exceeds `i64`, but make concrete specialization return a
    typed boundary.
15. **Source-case boundary:** on the all-active sunset fixture,
    `F(3,2)=(0,3,2)` must be rejected as outside the source orthant even
    though the underlying global IBP identity can be evaluated there by a
    crate-private algebra test.
16. **No inner escape:** a compile-time API test should demonstrate that an
    external integration test cannot obtain `&ParametricRelation` from the
    public wrapper.

## 12. Production integration after this slice

Once the standalone sunset oracle passes, add a generated affine row-system
certificate parallel to the current integer-cylinder row system. It should:

- own one shared live-leaf queue/start and one affine map;
- retain source row ordinals, prepare-point ordinals, translations, and
  available/unavailable witnesses;
- expand point-major and source-row-minor;
- call only `AffineLocusBoundRelationCompiler::compile`;
- keep each inner relation private;
- aggregate composition work across every depth; and
- replay from the generated IBP/LI row span and exact source case.

At that stage, avoid cloning one complete source `ParametricRelation` into
every row certificate. The outer generated certificate already owns the row
span through the shared queue; retain a source-row ordinal and use a private
borrow during replay. The `Arc<ParametricRelation>` standalone API above is a
small-slice proof boundary and test convenience, not the final million-row
storage layout.  The live-leaf work item now likewise retains its coordinate
extraction behind one `Arc`, allowing the generated affine map to share the
exact authenticated extraction allocation rather than deep-cloning the
partition proof.

Persistent elimination, affine ordering, grouped target matching,
free-variable recentering, affine `WhenBad`, and effective coverage remain
later slices. In particular, successful row composition does not yet produce
a reduction rule.

## 13. Test command and claim boundary

The focused tests should be run in parallel with the supplied license and no
`no_gmp` feature:

```bash
SYMBOLICA_LICENSE='your-license' \
SYMBOLICA_HIDE_BANNER=1 \
cargo nextest run -j4 \
  --test residual_unit_affine_composition \
  --test affine_locus_bound_relation_sunset
```

The implemented slice was validated with the licensed, default GMP build and
parallel tests (no `no_gmp` feature):

- the focused post-audit wrapper/compositor/sunset set: 15/15 passed;
- the complete affine stack: 45/45 passed;
- all RustRed library unit tests: 169/169 passed; and
- `cargo check --workspace --all-targets`: passed.

The connected-sunset oracle covers four generated ordinary IBP rows, five
ambient translations, and four concrete affine points (80 exact comparisons),
plus four guarded-rational comparisons.  No recurrence is supplied by the
test.

RustRed may therefore claim that generated parametric IBP/LI
rows can be translated and exactly restricted, with Symbolica, to a complete
certified unit-affine residual equality locus. It may also claim that this
restriction agrees term-for-term with direct concrete specialization on the
connected sunset fixture.

The remaining bounded backend caveats are that Symbolica's polynomial GCD is
not cooperatively wall-clock bounded and stable Rust cannot turn individual
`BTreeSet`/GMP allocator exhaustion into a typed error.  RustRed's limits bound
finite input shape, retained logical payload, and the audited arithmetic/copy
work; they are not a process-wide OOM guarantee.

It may not yet claim:

- an affine-locus pivot or reduction rule;
- grouped LiteRed case attachment;
- conditional coverage or `WhenBad` closure;
- support for nonunit, rational, nonlinear, cyclic, or parameter-valued index
  maps;
- a master inferred from an unavailable row or bounded search; or
- complete `SolvejSector` parity.
