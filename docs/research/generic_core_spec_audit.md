# Audit of the first generic RustRed core specification

Date: 2026-08-13

## Scope and method

This is an independent, source-level audit of the first vertical slice in
[`litered_full_scope_spec.md`](litered_full_scope_spec.md): generic family
algebra with external momenta, ordinary parametric IBPs, power shifts,
Lorentz-invariance identities, coefficient contexts, relation translation,
and exact integer specialization.  It compares that specification with
[`LiteRed2026.m`](../../vendor/LiteRed2/Source/LiteRed2026.m), the relevant
vendored Symbolica implementation, and the current RustRed compatibility
code.

No Mathematica or FORM process was run.  No Rust source was changed.

## Verdict

The ordinary-IBP equation in the authoritative specification is
mathematically correct, including its sign, its `n_r + nu_r` multiplier, its
`e_r` and `e_r-e_t` shifts, and LiteRed's contraction-major row order.  The
proposed sparse `IndexShift -> ParametricCoefficient` representation is also
the right core representation.

The slice is not yet implementation-complete, however.  Four points are
correctness blockers:

1. the family coefficient field and the index-extended parametric field are
   not separated strongly enough;
2. the LI prose omits the coefficients, directions, and deterministic sign of
   the translated relations;
3. exact specialization has no defined target context or exceptional-locus
   behavior; and
4. a parameter-dependent denominator basis needs a proved generic inverse and
   an explicit nonzero-determinant domain.

The findings below state the missing invariants and a corrected minimal
contract.  They do not require changing the intended architecture.

## Findings

### G1 — Blocker: use two authenticated coefficient contexts, not one undifferentiated field

The specification first defines a family coefficient field over kinematics
([specification lines 99--109](litered_full_scope_spec.md#L99)), then calls the
canonical relation field `Q(family_parameters,n_0,...,n_{N-1})`
([lines 191--205](litered_full_scope_spec.md#L191)).  It does not state that
all family data must be independent of every `n_i`.  Without that invariant,
a denominator coefficient, external Gram entry, dimension, or power shift
could accidentally contain a parametric index and still type-check as a
coefficient.

LiteRed keeps these roles separate.  It derives `Parameters[nm]` from
denominators, external scalar products, power shifts, and dimension at
[`LiteRed2026.m:797-809`](../../vendor/LiteRed2/Source/LiteRed2026.m#L797), but
creates a fresh array of index symbols only inside `GenerateIBP` at
[`LiteRed2026.m:1813-1816`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1813).
The sector solver also explicitly treats family parameters as independent at
[`LiteRed2026.m:2403-2408`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2403).

Required contract:

- `BaseContext K = Q(theta_0,...,theta_{P-1})` owns dimension, denominator
  rows and constants, external Gram entries, power shifts, and normalized
  kinematic expressions.
- `ParametricContext K(n) = Q(theta_0,...,theta_{P-1},n_0,...,n_{N-1})`
  extends `K` in that exact order.
- Every family input is checked to use exactly `K`; every relation coefficient
  is checked to use exactly `K(n)`.
- Lifting `K -> K(n)`, projecting `K(n) -> K` after specialization, and
  remapping between equivalent maps are explicit checked operations.  No
  arithmetic operation may decide or extend a variable map implicitly.
- Parameter and index roles are part of the context fingerprint.  User labels
  cannot collide with generated index identities even when their display
  strings coincide.

This is an API requirement, not just documentation.  Symbolica's checked Atom
conversion is deliberately permissive: it adds undeclared variables and
non-polynomial pieces to an explicit map
([`atom/core.rs:1228-1293`](../../vendor/symbolica/src/atom/core.rs#L1228)), and
the implementation can add `Power` or `Function` variables
([`poly.rs:1382-1432`](../../vendor/symbolica/src/poly.rs#L1382)).  Rational-
polynomial addition and multiplication silently unify different maps
([`rational_polynomial.rs:980-993`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L980),
[`1065-1077`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L1065)).
RustRed must reject those implicit extensions in proof-bearing code.

The current `CoefficientContext::parse` passes a nominal map but does not
verify the returned map
([`src/coefficient.rs:358-363`](../../src/coefficient.rs#L358)), while
`LinearCombination::add_term` adds coefficients without a context check
([`src/linear.rs:40-54`](../../src/linear.rs#L40)).  These APIs may remain
compatibility surfaces, but the generic core cannot reuse them unwrapped.

### G2 — Blocker: the LI construction needs an explicit weighted translation formula

The LI description at
[`litered_full_scope_spec.md:207-224`](litered_full_scope_spec.md#L207) correctly
says that both integral keys and coefficient indices must be translated, but
“rewrites every `k_i.p_a` into denominator shifts” is under-specified.  It does
not say that the scalar-product expansion contributes coefficients, that the
denominator shifts are negative, or which antisymmetric sign and pair order
match LiteRed.

Let the inverse denominator map over `K` give

\[
  k_i\!\cdot p_a=\beta_{ia,0}+\sum_{t=1}^{N}\beta_{ia,t}D_t.
\]

Multiplication by this scalar product is represented on the integral lattice
by the weighted translation operator

\[
  X_{ia}=\beta_{ia,0}T_0+
         \sum_{t=1}^{N}\beta_{ia,t}T_{-e_t}.
\]

For a relation

\[
  R(n)=\sum_\delta c_\delta(n)J(n+\delta),
\]

translation must be the single atomic operation

\[
  T_sR(n)=\sum_\delta c_\delta(n+s)J(n+s+\delta).
\]

If the relation continues to use `n` as its base, the stored term is therefore
`(delta+s) -> c_delta(n+s)`.  Translating just the key, or translating the
coefficient without the key, is wrong.

With `B_{b i}` denoting the ordinary IBP row for
`p_b . d/dk_i`, define

\[
  M_{ab}=\sum_{i=1}^{L}X_{ia}B_{b i}.
\]

For each lexicographically ordered pair `a < b`, LiteRed's structural choice
at [`LiteRed2026.m:1818`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1818) is

\[
  LI_{ab}=M_{ba}-M_{ab}.
\]

The opposite overall sign is an equivalent equation but is not an equivalent
serialized row or structural test fixture, so the sign must be fixed.  The
source constructs pairs with `Subsets[Range[E],{2}]`, hence lexicographic pair
order, also at line 1818.

The translation operation should satisfy and test

\[
 T_0R=R,\qquad T_s(T_tR)=T_{s+t}R,\qquad
 \operatorname{Spec}_a(T_sR)=\operatorname{Spec}_{a+s}(R).
\]

A nonzero power shift is the essential regression case: it proves that
`n_r+nu_r` became `n_r+s_r+nu_r`, rather than only moving integral keys.

### G3 — Blocker: exact specialization must be guarded and must project back to `K`

The immediate boundary promises “exact specialization to concrete integer
powers” at
[`litered_full_scope_spec.md:404-416`](litered_full_scope_spec.md#L404), but it
does not define the operation.  For an integer vector `a` of arity `N`, the
required result is

\[
 \operatorname{Spec}_a R=
   \sum_\delta c_\delta(a)\,I(a+\delta).
\]

The concrete key is `a+delta`, not `a+delta+nu`; `nu` is never part of an
integral key.  The result coefficients must use exactly the base map `K`, not
the larger `K(n)` map with now-unused index slots.  After substitution, both
numerator and denominator must be proved independent of every `n_i` before a
bulk exact remap to `K`.

Specialization must return a checked/guarded result:

- reject a wrong-length assignment;
- reject a noninteger assignment;
- use checked or arbitrary-precision addition for every `a_i+delta_i`;
- evaluate numerator and denominator separately;
- return an inapplicable/pole result if the mapped denominator is the zero
  polynomial;
- retain the mapped original denominator as a nonzero parameter guard before
  fraction-field cancellation; and
- combine equal concrete keys and remove exact zeros only after successful
  evaluation.

Symbolica's `RationalPolynomial::evaluate` is not suitable: it divides without
a `Result` at
[`rational_polynomial.rs:709-715`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L709),
and its polynomial `replace_all` silently zips a short point at
[`polynomial.rs:1915-1935`](../../vendor/symbolica/src/poly/polynomial.rs#L1915).
`evaluate_with_coeff_map` at least asserts the full point length
([`polynomial.rs:1890-1913`](../../vendor/symbolica/src/poly/polynomial.rs#L1890)),
but the wrapper still has to evaluate the original denominator separately and
use checked `try_div`
([`rational_polynomial.rs:910-919`](../../vendor/symbolica/src/domains/rational_polynomial.rs#L910)).

The initial raw IBP/LI rows happen to be only affine-polynomial in the indices
apart from family-parameter denominators, but the guarded substitution API
will also be reused by discovered rules, whose denominators do depend on
indices.  Implementing an unguarded shortcut now would create two conflicting
specialization semantics later.

### G4 — Blocker: a symbolic denominator basis has a domain, not an unconditional inverse

The specification deliberately allows denominator-row coefficients in `K`,
not just rationals
([`litered_full_scope_spec.md:99-115`](litered_full_scope_spec.md#L99)).  For

\[
  D=c+A S,
\]

“`A` is invertible over the field” means generic invertibility.  If
`det(A)=P/Q`, construction must preserve the input coefficient-denominator
guards and add `P != 0` as the basis-domain guard.  A concrete kinematic
specialization at `P=0` is not a valid specialization of that family map.

The constructor must verify, exactly in the canonical context,

\[
  A A^{-1}=A^{-1}A=1,
  \qquad S=A^{-1}(D-c).
\]

It must also verify every cached derivative expansion by reducing

\[
 q\!\cdot\partial_{k_i}D_r-
 \gamma_{riq,0}-\sum_t\gamma_{riq,t}D_t
\]

to zero in the free scalar-product module over `K`.  This catches transpose,
off-diagonal-factor, and constant-sign mistakes without using the resulting
IBP row as its own oracle.

LiteRed constructs its scalar-product inverse with `Solve` at
[`LiteRed2026.m:780-809`](../../vendor/LiteRed2/Source/LiteRed2026.m#L780).
Importantly, a singular external Gram matrix merely produces a warning at
[`LiteRed2026.m:776-779`](../../vendor/LiteRed2/Source/LiteRed2026.m#L776);
ordinary IBP/LI generation does **not** require an invertible external Gram
matrix.  RustRed must not confuse the required denominator-basis determinant
with an optional external-Gram guard.

The existing matrix path is not reusable for the generic case:
`Denominator::quadratic_form` and `inverse_basis` contain `ExactRational`
([`src/family.rs:13-18`](../../src/family.rs#L13),
[`169-177`](../../src/family.rs#L169)), and `invert_matrix` only accepts a
nonempty square matrix of that type
([`src/exact.rs:158-205`](../../src/exact.rs#L158)).  Its `i64` arithmetic also
panics on overflow
([`src/exact.rs:82-123`](../../src/exact.rs#L82)).  The new core needs exact
Symbolica/Rust algebra over `K`, with typed resource/degree failures.

### G5 — Major: sector signs are defined before every power shift, not only noninteger shifts

The wording at
[`litered_full_scope_spec.md:243-254`](litered_full_scope_spec.md#L243) says the
sector sign domain is taken “before any noninteger power shift.”  LiteRed's
`jSector` tests the raw `j` indices at
[`LiteRed2026.m:1700-1716`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1700),
whereas `PowerShifts` enter only the coefficient transformation inside
`GenerateIBP` at
[`LiteRed2026.m:1815-1818`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1815).

The invariant should be: sector membership is determined from `n_r` before
**any** `nu_r`, integer or noninteger, is applied.  Power shifts have arity
`N`, live in `K`, and modify only coefficient multipliers.  The source contains
an explicit TODO rather than a completed consistency policy for shifted/cut
denominators at
[`LiteRed2026.m:829-831`](../../vendor/LiteRed2/Source/LiteRed2026.m#L829), so
the Rust port should not silently invent additional prohibitions in the raw
generator.

This also forbids the common concrete optimization “skip denominator `r` when
`n_r == 0`” in a power-shifted family: `n_r+nu_r` may still be nonzero.  The
current vacuum generator does exactly that at
[`src/ibp.rs:80-83`](../../src/ibp.rs#L80), but it has no power-shift model and
must remain only a zero-shift compatibility oracle.

### G6 — Major: specify the scalar-product derivative convention explicitly

The abstract `gamma` formula is correct, but the implementation boundary does
not define the off-diagonal convention.  With coordinates

\[
 S=(k_a\!\cdot k_b)_{a\le b}\;\Vert\;
   (k_a\!\cdot p_\alpha)_{a,\alpha},
\]

the derivative module must implement

\[
 q\!\cdot\partial_{k_i}(k_a\!\cdot k_b)=
 \delta_{ia}\,q\!\cdot k_b+
 \delta_{ib}\,q\!\cdot k_a,
\]

including the factor two when `a=b=i`, and

\[
 q\!\cdot\partial_{k_i}(k_a\!\cdot p_\alpha)=
 \delta_{ia}\,q\!\cdot p_\alpha.
\]

For loop `q`, the resulting dots are coordinates; for external `q`,
`q.p_alpha` is the declared symmetric external Gram entry.  This is precisely
the factor encoded by `2^Boole[#1===lm]` in LiteRed at
[`LiteRed2026.m:1817`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1817).  The
current vacuum implementation contains a useful independent reference for the
loop-loop cases at
[`src/family.rs:1009-1025`](../../src/family.rs#L1009), but it has no
loop-external or external-external cases.

The family constructor must require distinct loop identities, distinct
external identities, disjoint loop/external sets, a symmetric and complete
external Gram table, and an independent external **vector basis** for the
chosen coordinate model.  A singular Gram determinant is allowed.  A declared
vector relation such as `p_3=p_1+p_2` must be resolved before construction;
otherwise `N=L(L+1)/2+LE` overcounts independent coordinates.

### G7 — Major: kinematic relations need one canonical algebra, not an ambiguous “relations” field

The specification simultaneously calls the coefficient variables
algebraically independent
([`litered_full_scope_spec.md:105-109`](litered_full_scope_spec.md#L105)) and
stores “kinematic relations”
([`lines 122-137`](litered_full_scope_spec.md#L122)).  A
`RationalPolynomial<IntegerRing, E>` is a fraction field in independent
variables; it is not automatically a quotient by arbitrary polynomial
relations.

For this slice, choose one of two explicit contracts:

1. preferred initially: family construction rewrites all external scalar
   products and masses into a declared independent parameter basis, and every
   accepted expression is already a canonical element of `K`; or
2. support a kinematic ideal, with a fixed authenticated Groebner basis and a
   mandatory normal-form reduction for equality, zero tests, determinant
   checks, guards, hashing, and specialization.

Plain assumptions such as `m2 != 0` remain guard facts, not polynomial
rewrite rules.  Failing to make this choice allows two mathematically equal
gamma expansions to compare unequal, or a determinant zero modulo kinematics
to be accepted as invertible.

### G8 — Major: relation identity and deterministic order need to be structural

The map `IndexShift -> ParametricCoefficient` is necessary but not sufficient
as a standalone relation object.  It must also carry or be scoped by:

- family fingerprint;
- exact parametric-context fingerprint;
- shift arity `N`;
- row kind and source coordinates (`IBP(q,i)` or `LI(a,b)`); and
- a canonical zero-equation normalization policy, if rows are normalized.

The specification correctly records that LiteRed orders ordinary rows with
contraction momentum major and differentiated loop minor
([`litered_full_scope_spec.md:177-181`](litered_full_scope_spec.md#L177)); this
comes from `Outer[...,qms,lms]` followed by `Flatten` at
[`LiteRed2026.m:1813-1819`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1813).
The current vacuum generator iterates differentiated loop first at
[`src/ibp.rs:49-58`](../../src/ibp.rs#L49), so preserving its vector order
would be a structural mismatch even in the `E=0` adapter.

For deterministic combined output, use:

1. all ordinary rows in `(q,i)` order, with `q=(k_0,...,k_{L-1},p_0,...,p_{E-1})`;
2. all LI rows in lexicographic `(a,b)`, `a<b`, order; and
3. `IBPLI = IBP || LI`, as LiteRed does at
   [`LiteRed2026.m:1826-1831`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1826).

### G9 — Major: numeric widths and empty contexts need a stated policy

LiteRed's integer lattice is not `i32`-bounded.  The current `Integral` stores
`Vec<i32>` and specializes shifts through checked `i32` conversion
([`src/integral.rs:5-18`](../../src/integral.rs#L5),
[`50-62`](../../src/integral.rs#L50)).  The generic core should either use an
arbitrary-precision integer key or make every boundary and addition checked
and return a typed range error.  Silent saturation, wrapping, or a panic is not
acceptable.  `N` and `L*(L+E)` also need checked `usize` arithmetic and resource
limits; LiteRed requires at least one loop momentum through the nonempty
`lms:{__...}` pattern at
[`LiteRed2026.m:763-764`](../../vendor/LiteRed2/Source/LiteRed2026.m#L763), so
the core should reject `L=0` explicitly.

The base field `K` may have no symbolic variables after all data and dimension
are fixed numerically.  Current `CoefficientContext::try_new` rejects an empty
parameter list at
[`src/coefficient.rs:263-269`](../../src/coefficient.rs#L263).  The new
context layer must support `K=Q`, even though `K(n)` is nonempty for `L>=1`.

Symbolica's selected exponent type is also part of the exact domain.  Current
RustRed uses `u16` and documents that arithmetic can panic on exponent overflow
([`src/coefficient.rs:11-19`](../../src/coefficient.rs#L11)).  Context lifting,
translation, substitution, determinant construction, and gamma verification
need explicit degree preflights/resource errors, not only the loop-specific
reducers that currently use those helpers.

## Corrected minimal contract for the first slice

The following is sufficient to make the authoritative direction
implementation-ready.

### Family

1. Validate `L>=1`, checked `N=L(L+1)/2+LE`, and checked row counts.
2. Validate unique/disjoint momentum identities and an independent external
   vector basis; accept a singular external Gram matrix.
3. Construct and fingerprint `K` with ordered, role-tagged variables and an
   explicit kinematic-normal-form policy.
4. Validate every denominator as `D=c+A S`, with `c,A in K`, exactly `N` rows,
   and no hidden momentum or index dependence.
5. Prove generic basis invertibility, retain input denominator guards and
   `numerator(det A) != 0`, construct `A^-1`, and verify both inverse products.
6. Validate dimension and all `nu_r` in `K`, with exactly `N` shifts.
7. Construct `K(n)` by deterministic exact map extension, with all index roles
   disjoint from parameter roles.
8. Build every derivative contraction in the free scalar-product module,
   rewrite it using `A^-1`, and exact-replay the gamma expansion before caching.

### Parametric relation

```text
ParametricRelation {
    family_fingerprint,
    context_fingerprint,
    row_id,
    terms: BTreeMap<IndexShift[N], CoefficientInKOfN>,
    inherited_nonzero_guards,
}
```

Every insertion checks arity and exact context equality, combines duplicates,
and drops exact zeros.  Raw generation performs no sector, zero, symmetry, or
concrete canonicalization.

### Ordinary IBP

Generate exactly `L*(L+E)` rows in contraction-major order using the equation
already given in the authoritative specification.  Lift `gamma`, dimension,
and `nu` from `K` to `K(n)` explicitly.  Power shifts occur only in
`n_r+nu_r`; keys remain integer shifts from `n`.

### LI

Build `X_ia`, `T_s`, `M_ab`, and `LI_ab=M_ba-M_ab` exactly as defined in G2.
Return exactly `E*(E-1)/2` structurally ordered rows, including a canonical
zero row if a degenerate kinematic configuration makes an identity vanish;
do not silently change row numbering.

### Exact specialization

`specialize(a)` validates an integer vector of arity `N`, simultaneously maps
all parameter variables to themselves and all index variables to integer
constants, evaluates source numerator and denominator separately, retains pole
guards, checked-divides, proves all index dependence gone, remaps to `K`, and
forms checked concrete keys `a+delta`.  It returns a typed guarded concrete
relation or a typed failure, never a partially evaluated coefficient.

## Acceptance tests required before rule discovery

### Family algebra

- `D -> S -> D` exact round trips for nonsymmetric symbolic `A`, so a hidden
  transpose mistake cannot pass.
- Parameter-dependent `A` with a determinant that has a visible exceptional
  locus; generic construction succeeds with a guard and specialization on the
  locus fails.
- External Gram symmetric/null/singular cases accepted when the denominator
  basis is valid.
- Rejection of an undeclared Symbolica variable, a `Function`/`Power`
  indeterminate, index dependence in family data, wrong arity, duplicate
  momentum identity, dependent external-vector declaration, and zero
  determinant modulo the chosen kinematic normal form.
- Exact replay of every cached derivative contraction against the direct
  scalar-product derivative.

### Ordinary IBP

- Row count and exact `(q,i)` ordering for `(L,E)=(1,0),(1,1),(1,2),(2,1)`.
- Direct coefficient/shift comparison against an independently differentiated
  scalar-product expression, not against cached gamma.
- Nonzero symbolic power shifts, especially `n_r=0`, proving no term is
  skipped and no `nu_r` enters a key.
- Duplicate-shift cancellation and canonical zero coefficients.
- Raw identities unchanged by any configured sector/cut/symmetry data.

### LI and translation

- No LI rows for `E=0,1`; one correctly signed row for `E=2`; lexicographic
  rows for `E=3`.
- Weighted constant and `-e_t` contributions from `k_i.p_a` checked
  separately.
- `T_0`, composition, and specialization-commutation properties.
- A power-shifted LI fixture where key-only translation gives a detectably
  different wrong answer.
- Direct antisymmetric Lorentz-generator check after several concrete integer
  assignments, while retaining symbolic kinematics.

### Contexts and specialization

- Exact lift `K -> K(n)` and bulk projection `K(n) -> K`, including `K=Q`.
- Parameter/index display-name collision without symbol-role collision.
- Rejection of map reordering and implicit extra variables.
- Negative and large integer assignments, checked key overflow if a bounded
  representation remains, and wrong assignment length.
- A coefficient with an index-dependent denominator that becomes zero, one
  that becomes a nonconstant parameter guard, and one whose factors cancel in
  the normalized result while the original pole guard remains.
- `Spec_a(T_s R) == Spec_{a+s}(R)` as guarded concrete relations.

## Disposition of the current code

The authoritative specification is correct to classify the existing modules
as compatibility/oracle layers.  They do not yet implement this slice:

| Current component | Useful part | Generic-core gap |
|---|---|---|
| [`CoefficientContext`](../../src/coefficient.rs#L88) | canonical Symbolica constants, exact arithmetic, and a variable-map equality check | no base/parametric role split, no strict parsed-map check, no arbitrary extension/remap/substitution, empty `K` rejected |
| [`VacuumFamily`](../../src/family.rs#L168) | loop-loop coordinate ordering, rational basis inversion, and derivative-factor reference | no externals, `A` restricted to `ExactRational`, no power shifts, no symbolic determinant domain |
| [`IbpGenerator`](../../src/ibp.rs#L11) | concrete zero-shift vacuum derivative oracle and uncanonicalized debug path | concrete seeds only, `L^2` rows, wrong structural row nesting for LiteRed, no LI, no power shifts |
| [`Integral`](../../src/integral.rs#L5) | typed concrete key with checked shifting | `i32` lattice and no family identity |
| [`LinearCombination`](../../src/linear.rs#L7) | sparse duplicate combination | no family/context authentication; Symbolica may unify a foreign coefficient map |

The safest implementation boundary is therefore a new generic family/context/
relation layer, followed by a deliberately checked adapter to these concrete
types for oracle comparisons.  Extending `VacuumFamily` or `IbpGenerator` in
place would make it too easy to retain their rational-only matrix, loop-only
row ordering, concrete canonicalization, or unguarded coefficient composition.
