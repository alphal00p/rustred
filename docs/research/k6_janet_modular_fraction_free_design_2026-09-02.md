# K6 Janet modular scheduling and fraction-free exact replay design — 2026-09-02

Status: implementation design and public-API audit. This note proposes two
complementary changes to the current exact Janet/Ore completion engine:

1. a support-first modular lane for discovering stable leader, obligation, and
   reduction schedules cheaply; and
2. a projective fraction-free lane for replaying a selected schedule over the
   exact coefficient field with substantially less rational-denominator swell.

Neither lane is publication authority on its own. A closing artifact still
requires exact source replay, exact guard accounting, strict descent, an
exactly exhausted Janet queue, and the existing exact complement and artifact
validation boundaries. This note does **not** claim K6 closure and records no
new K6 timing.

The design was audited against the live RustRed implementation, the vendored
Symbolica 2.2.0 source used by the workspace, and the matching
`FOR_REFERENCE_ONLY_DO_NOT_PUSH/symbolica` checkout. No private Symbolica API
is required. RustRed must continue to delegate all finite-field, polynomial,
GCD, exact-division, and generic matrix algebra to Symbolica.

## Executive conclusion

The recommended architecture is:

```text
independent finite-field Janet/Ore traces
       |
       | stable structural consensus only
       v
deterministic modular schedule guide
       |
       | every consequential decision replayed
       v
exact projective/fraction-free Ore replay over Z[d,n]
       |
       | exact row + exact source provenance + exact guards
       v
final exact monic artifact materialization and validation
```

The modular lane is the higher-leverage scaling intervention. It can screen
variable orders and completion trajectories before committing to expensive
exact arithmetic. The fraction-free lane is an exact arithmetic backend for
the winning schedule: it removes coefficient denominators from the hot loop,
but cannot rescue a structurally poor order by itself.

The smallest useful implementation should therefore build a modular normal
form against one frozen exact Janet epoch, validate its Ore translations and
multi-probe trace stability against small exact cases, and only then add full
modular completion. In parallel or immediately afterward, a private
projective-polynomial consequence can replay one selected trace
fraction-free. A full replacement of the existing monic basis should wait for
bounded A/B evidence.

## Crucial correction about the observed term stops

The 18–25 million and 94 million `exact addition numerator terms` reported by
the bounded K6 studies are conservative RustRed preflight projections. They
are not necessarily polynomial terms that Symbolica actually materialized.

RustRed's current checked rational addition estimates the unreduced cross sum

```text
N_left * D_right + N_right * D_left
```

from the full sparse supports before entering Symbolica. See
[`trusted_coefficient_sum_on_map`](../../crates/rustred-core/src/algebra/coefficient/operations.rs),
especially its `exact addition numerator terms` preflight.

Symbolica's native `RationalPolynomial` addition is more refined. It first
computes `gcd(D_left, D_right)`, divides both denominators by that GCD, and
cross-multiplies only the reduced parts. It then tests for further numerator
cancellation against the denominator GCD. See the borrowed `Add`
implementation in
[`domains/rational_polynomial.rs`](../../vendor/symbolica/src/domains/rational_polynomial.rs).
Native multiplication similarly cross-cancels numerator and denominator GCDs
before multiplying.

Consequences for interpreting the release study:

- the stop remains real under RustRed's declared resource policy;
- the requested count is a safe upper envelope, not an observed canonical
  output size or a measurement of Symbolica's peak scratch allocation;
- merely raising or bypassing the cap is not a sound cure, because Symbolica
  exposes no scratch-memory callback or hard workspace limit for the native
  GCD and quotient routines;
- a cold diagnostic at the first rejected operation should compare the
  existing Cartesian estimate with a GCD-reduced estimate; and
- projective polynomial rows remain attractive because they eliminate this
  rational-denominator cross-sum mechanism instead of trying to predict it.

The diagnostic must remain optional and cold: computing a denominator GCD
solely to improve the preflight and then asking Symbolica's rational addition
to recompute that GCD can double expensive work. RustRed should not duplicate
Symbolica's rational-addition implementation merely to reuse the first GCD.

## Existing exact ownership and bottleneck

RustRed's exact indexed coefficient is currently
`RationalPolynomial<IntegerRing, u16>`; the authenticated value and context
live under [`algebra/indexed`](../../crates/rustred-core/src/algebra/indexed/).
The exact Ore row stores one such coefficient per sparse forward shift, and
its provenance stores one per `(source_ordinal, left_shift)` entry in
[`involutive/ore/model.rs`](../../crates/rustred-core/src/foundry/completion/involutive/ore/model.rs).

The current left AXPY correctly implements

```text
E^a c(n) = c(n + signed_sector(a)) E^a
```

and applies the same action to the source-module provenance in
[`involutive/ore/arithmetic.rs`](../../crates/rustred-core/src/foundry/completion/involutive/ore/arithmetic.rs).
Janet basis admission projectively normalizes every leader to one, and exact
normal form then subtracts the target coefficient times the shifted monic
divisor. These invariants are implemented in
[`involutive/janet.rs`](../../crates/rustred-core/src/foundry/completion/involutive/janet.rs)
and
[`involutive/normal_form.rs`](../../crates/rustred-core/src/foundry/completion/involutive/normal_form.rs).

Monic normalization is mathematically useful but introduces rational
denominators early. Every later AXPY translates, multiplies, and adds those
rational functions in both the physical row and its provenance. The bounded
release study shows that making rows monic by itself did not change five of
six structural trajectories and did not cure the projected coefficient
swell. This motivates both selecting a better structural trajectory before
exact work and retaining polynomial projective representatives during exact
replay.

## Authority boundary

The following distinction is mandatory.

### Modular information may guide

A stable modular trace may nominate:

- the next completion obligation;
- a variable or block order;
- a candidate Janet divisor;
- a candidate leading shift;
- an expected remainder support;
- a likely zero remainder to defer until later exact checking;
- an F4/Macaulay block shape; and
- a resource-aware score for competing schedules.

At a valid sample, a nonzero modular image is one-sided algebraic evidence
that the represented exact expression is not identically zero. RustRed should
still treat the trace as a proposal because the modular lane does not own the
authenticated exact consequence or its source proof.

### Modular information may not certify

No finite collection of zero modular images may:

- certify an exact coefficient as zero;
- discharge a Janet obligation;
- suppress an exact exceptional branch;
- certify source provenance;
- certify a localization guard;
- declare an exact complement finite;
- declare queue exhaustion; or
- publish an artifact.

Multi-sample agreement is probabilistic stability evidence, not a no-relation
or closure certificate. A modularly zero obligation may be deprioritized, but
it must eventually receive an exact normal-form proof or an independent exact
syzygy/criterion witness.

### Exact publication authority remains unchanged

Exact replay must start from the original authenticated consequences and use
the exact coefficient context, exact Ore action, exact ordering identity, and
exact source chronology. It must validate the guide at every step and fall
back to the deterministic exact queue on the first divergence. Closure is
claimed only after exact queue exhaustion, exact localization handling,
finite complement construction, and the ordinary artifact validation path.

This matches the existing safety contract under
[`completion/frame/evidence`](../../crates/rustred-core/src/foundry/completion/frame/evidence/):
modular discovery nominates an exact replay; it never owns the resulting
relation.

## Support-first modular Janet/Ore lane

### Why not expanded finite-field rational polynomials

Changing every exact coefficient to
`RationalPolynomial<Zp64, u16>` would remove multiprecision integer growth but
would retain the same expanded multivariate monomial support. That can still
be the dominant K6 cost. It also pays polynomial translations and
normalizations separately for every shifted derived coefficient.

The primary modular representation should instead be a hash-consed
coefficient-expression DAG evaluated as a black box at deterministic finite
field points. Expanded finite-field polynomials remain useful as a small-case
oracle and optional intermediate lane.

### Field-independent coefficient DAG

A suitable representation is:

```rust
struct CoeffRef {
    node: CoeffNodeId,
    translation: PhysicalDeltaId,
}

enum CoeffNode {
    Zero,
    One,
    Integer(i64),
    ExactLeaf(ExactCoefficientId),
    Neg(CoeffRef),
    Add(CoeffRef, CoeffRef),
    Mul(CoeffRef, CoeffRef),
    Inv(CoeffRef),
}
```

The DAG shape is field-independent. A probe-specific evaluator maps it to
`Zp64` elements. `CoeffRef.translation` is essential: it makes translation of
a derived coefficient constant-time while allowing the children of a sum or
product to carry different translations.

Hash-cons nodes and delta vectors. Canonicalize commutative operand order and
perform only indisputably exact structural simplifications:

- `x + 0 = x`;
- `x * 0 = 0`;
- `x * 1 = x`;
- `-(-x) = x`; and
- `x + (-x) = 0` when the child identity and translation are identical.

Do not infer an exact zero merely because a probe vector is zero. Record the
difference between `KnownZero`, established structurally or by an exact leaf,
and `SampledZero`, observed only in one or more fields.

### Evaluation cache and singular probes

Cache evaluation by

```text
(probe ordinal, node id, accumulated physical delta id).
```

Evaluation is recursive or stack-driven over the acyclic node IDs:

- `ExactLeaf` evaluates the exact numerator and denominator separately at the
  translated full point;
- `Neg`, `Add`, and `Mul` call the Symbolica finite-field ring operations;
- `Inv` rejects the probe if its child evaluates to zero; and
- translation adds to index coordinates only, never to base parameters.

An intermediate zero denominator is conservatively singular even if a later
formal cancellation might remove it. That is safe because the modular trace
is valid only in the localization it actually traversed.

Each rejected probe owns a separate cache that can be dropped immediately.
Do not allow partial sampled rows from a rejected probe to enter consensus.

### Correct Ore translation after specialization

For an operator shift `a`, the coefficient must be evaluated at

```text
n + physical_translation(a),
```

where active sector coordinates receive `+a_i` and inactive coordinates
receive `-a_i`. A cached scalar value at `n` must never be reused for the
coefficient at `n+a`.

Translation is represented without expanding the leaf:

```text
translate(CoeffRef(node, delta), a)
    = CoeffRef(node, intern(delta + physical_translation(a))).
```

Nested translations must be normalized so that

```text
translate(translate(c, a), b) == translate(c, a + b).
```

The modular left AXPY is consequently

```text
new[s + a] += multiplier * translate(source[s], physical_translation(a)),
```

with the same translated action on any modular provenance structure retained
for trace identity. Monic normalization is represented by an `Inv` node; a
zero modular leader rejects that lane rather than inventing a row.

### Independent lanes, not per-term voting

Every `(prime, point)` pair must run as an independent deterministic lane.
Do not combine majority decisions term by term. A point-specific vanished
pivot can change all later reductions; taking the leader from one lane and a
later zero decision from another constructs a trace that no field actually
followed.

Group only complete structural trace identities. Select a trace group by:

1. greatest number of agreeing discovery lanes;
2. greatest number of distinct primes in that group; and
3. lexicographic trace identity as a deterministic tie-breaker.

Then require held-out lanes to reproduce that complete trace before exact
replay.

### Deterministic prime and point portfolio

The initial implementation should reuse the already validated `Zp64` primes
present in RustRed:

```text
998244353
1000000007
1000000009
```

Use at least two nonuniform discovery points per prime and a third disjoint
held-out point. A reasonable first policy is:

- six discovery lanes: three primes times two points;
- three held-out lanes: the same primes times a third point;
- a quorum of at least three identical discovery traces spanning at least two
  primes; and
- held-out agreement before exact replay.

Generate coordinates deterministically from a semantic scope fingerprint,
probe ordinal, and coordinate ordinal, with rejection sampling into
`[2,p-2]`. Store the full integer point and its residues in the trace.
Reject duplicate residue-equivalent tasks. Fixed tables are acceptable for
the first slice, but an eventual generator avoids hand-tuned K6 points and
remains topology-generic.

Avoid the current all-zero chart as the only anchor. It unnecessarily raises
the chance of specialized zeros. Generic dimension/base-parameter anchors
such as the already used `29` and `43`, combined with nonuniform chart
coordinates, are better discovery points.

`FiniteField<Mersenne64>` at `p=2^61-1` is advertised by Symbolica as faster
and may later serve as one inexpensive screening lane. It cannot replace
multi-prime consensus because it supplies only one characteristic. `Zp64`
should remain the first implementation type.

### Bad-prime and bad-point handling

A lane is rejected, not voted against, if:

- its modulus is not an admitted odd prime;
- a source or family guard vanishes;
- an exact-leaf denominator vanishes;
- an `Inv` node sees zero;
- the sample is residue-equivalent to an earlier lane; or
- any declared DAG, cache, row, trace, or work limit is exceeded.

A nonzero exact coefficient can reduce to zero modulo one prime or vanish at
one point without a zero denominator. Such a lane is not necessarily invalid;
it may simply follow a different trace. Complete-trace grouping and held-out
agreement handle these unlucky images. If too few stable lanes remain, the
result is a typed `InsufficientStableModularGuide`, followed by exact fallback
or deterministic replenishment—not a closure failure and never a skip.

### Structural trace and provenance signatures

A guide should be bound to a complete semantic scope:

```rust
struct GuideScope {
    indexed_context_fingerprint: ...,
    source_module_fingerprint: ...,
    ore_action_fingerprint: ...,
    sector_fingerprint: ...,
    ordering_fingerprint: ...,
}
```

Rows receive stable birth IDs rather than hashes of field values:

```rust
enum GuideRowId {
    Initial { ordinal: u32 },
    Inserted { ordinal: u32 },
}
```

A reduction record contains at least:

```rust
struct GuideReductionStep {
    target_shift: ForwardShift,
    divisor_row: GuideRowId,
    left_operator_shift: ForwardShift,
}

struct GuideInsertion {
    parent_row: GuideRowId,
    prolongation_variable: u16,
    expected_start_leader: ForwardShift,
    reductions: Box<[GuideReductionStep]>,
    expected_remainder_leader: Option<ForwardShift>,
    expected_remainder_support: Box<[ForwardShift]>,
}
```

The consensus key contains obligation order, epoch boundaries, leader shifts,
divisor birth IDs, left translations, and canonical support shifts. It
excludes finite-field values. An incremental digest may accelerate grouping,
but the selected lane must retain the full bounded record and traces with the
same digest must receive full structural comparison.

The modular trace does not attempt to replace `OreConsequence` provenance.
Exact replay constructs and validates the existing exact source-module
witness. A modular support signature can retain sorted
`(source_ordinal,left_shift)` pairs for debugging, but it is not an exact
coefficient witness.

### Sparse matrix role

Termwise Janet normal form is sequential because a reduction changes the
subject support and hence later divisor choices. Symbolica's
`SparseRowReducer<Zp64>` is therefore not a drop-in replacement for the whole
completion loop.

It is appropriate for a later F4/Macaulay-style layer:

1. collect obligations from one fixed degree/epoch;
2. enumerate translated divisor rows selected by a stable modular trace;
3. map forward shifts to columns in the exact descending Ore order;
4. build a CSR `SparseMatrix<Zp64>` per probe; and
5. use serial `SparseRowReducer::add_row` with `LuLMode::Pattern` to discover
   pivot/support structure.

Avoid parallel `back_substitute` until outputs are explicitly canonicalized:
Symbolica documents that the parallel variant may permute output rows. Probe
lanes may run concurrently, but their results must be collected and grouped
by immutable probe ordinal so worker count cannot affect the selected guide.

## Projective fraction-free exact replay

### Representation

Let

```text
A = Z[base parameters, n_1, ..., n_K].
```

Store a consequence as an augmented sparse vector over `A`:

```rust
struct PrimitiveOreConsequence {
    row: Box<[PrimitiveOreTerm]>,
    provenance: Box<[PrimitiveProvenanceTerm]>,
    localization: LocalizationWitness,
}
```

Both physical row coefficients and source-module coefficients are
`IndexedPolynomial`. The row is projective: multiplying the complete row and
provenance by one nonzero element of `A` does not change its generic
`K(n)[E]` span. The implementation should retain a canonical sign/orientation
and exact guards, but should not force the leader to one inside the completion
hot loop.

### Clearing rational ingress once

For an incoming rational consequence:

1. compute an LCM `L` of every nontrivial denominator in both row and
   provenance;
2. replace `N_i/D_i` by `(L/D_i) N_i` using Symbolica exact polynomial
   division;
3. retain every original denominator condition in localization;
4. replay the complete polynomial row from the cleared polynomial
   provenance; and
5. reject any rational coefficient that survives clearing.

The existing
[`completion/frame/exact/cleared`](../../crates/rustred-core/src/foundry/completion/frame/exact/cleared/)
prototype already implements the required LCM/GCD/exact-division and replay
pattern. Its bounded `PolynomialBudget` should be promoted to a shared
internal indexed-polynomial service. RustRed must not create a second native
GCD or quotient implementation.

This conversion should normally occur at original source ingress, where
coefficients are polynomial or have only small constant denominators. Do not
allow a hot loop that repeatedly clears already swollen rational Janet rows.

### GCD-scaled pseudo-reduction

Suppose the selected subject coefficient is `A`, the basis leader is `b`, and
the Ore operator translation is `delta`. Translate the complete divisor
first, so its effective leader is

```text
B = sigma_delta(b).
```

Ask Symbolica for

```text
g = gcd(A, B)
u = B / g
v = A / g
```

and update the augmented consequence by

```text
F' = u F - v E^delta G.
```

The target cancels exactly because `u*A == v*B`. Apply `u` and `v` to every
row and provenance entry. Translate every divisor row coefficient,
provenance coefficient, and localization guard with the current exact Ore
automorphism before multiplication.

Using `gcd(A,B)` is important. The naive formula `B*F-A*G` is sound but can
retain avoidable common factors and recreate the large Cartesian product
problem. Symbolica's native dense fraction-free matrix reducer uses this same
GCD-scaled pattern.

No handwritten polynomial GCD, division, content, multiplication, or
subtraction belongs in RustRed. The Ore shift/key mapping and the augmented
row update are RustRed orchestration; scalar algebra is Symbolica-owned.

### Common-content removal

Only divide by content proven to divide the complete augmented vector. A GCD
of physical row entries alone is insufficient: dividing only the displayed
row changes or destroys the exact source-module witness.

A safe normalization is:

1. choose a small-support nonzero augmented entry as the first candidate;
2. compute GCDs across all row and provenance entries, stopping at one;
3. exact-divide every entry by the candidate; and
4. fail if any exact division returns `None`.

Alternatively, use Symbolica's `gcd_multiple` on cloned polynomials and then
exact-divide every augmented entry. Its fast path is described as
high-likelihood internally; maximality affects only performance. Exact
division of every output makes a nonmaximal candidate harmless and rejects a
non-divisor.

Recommended first policy:

- remove cheap integer coefficient content;
- compute full augmented polynomial content at basis admission;
- repeat only when a configurable retained-support threshold is crossed; and
- compare the cost of GCD work with the term support it removes.

Computing a multivariate GCD after every AXPY may cost more than it saves.

### Guard semantics

The first implementation should preserve, not optimize, the current
localization semantics:

- retain every source/family guard;
- retain every coefficient denominator cleared at ingress;
- retain the exact leader nonzero condition associated with projective basis
  admission;
- translate and merge the divisor localization on every Ore action; and
- retain the final leader guard when materializing a solved rule.

Dividing an augmented vector by an exactly common polynomial does not require
a new guard: the divided provenance remains a polynomial source proof. By
contrast, dividing row content that does not divide provenance would require
rational source multipliers and additional localization; the first lane must
reject that shortcut.

There is a possible later refinement in which a cleared unconditional
polynomial identity permits removal of intermediate elimination guards while
retaining only semantic source/family and final-target conditions. RustRed's
cleared-circuit prototype already demonstrates such guard separation. It is
not part of the first performance experiment because it would conflate an
arithmetic change with a branch-semantics change.

### Exact provenance

At all times the polynomial identity must be replayable as

```text
row = sum(source_coefficient * E^left_shift * original_source).
```

Pseudo-reduction updates the physical row and provenance with identical
`u`, `v`, and Ore translations. Common-content normalization divides both.
After every test reduction, reconstruct the row from original sources and
compare every sparse shift coefficient exactly.

A later compact provenance circuit may delay expansion, but its final exact
replay remains mandatory. A modular trace or a row-only identity is not a
substitute.

### Final monic boundary

The completion hot loop may remain projective. Materialize a conventional
monic relation only when:

- an exact row must cross the existing artifact boundary;
- a current consumer explicitly requires `OreConsequence`; or
- an A/B validation compares against the legacy monic lane.

For a polynomial row with leader `L`, construct each final coefficient
through Symbolica's rational-polynomial division by `L`, retain `L != 0`, and
authenticate the complete exact row and source provenance. This pays rational
normalization once per retained result instead of at every intermediate
normal-form operation.

### Dense fraction-free matrix as oracle, not default storage

Symbolica already exposes:

- `Matrix::partial_row_reduce_fraction_free`;
- `Matrix::back_substitution_fraction_free`;
- `Matrix::solve_fraction_free`; and
- `Matrix::content` / `Matrix::primitive_part`.

For a small trace-selected block, map Ore shifts to leading columns and
provenance keys to appended columns, construct a dense matrix over
`PolynomialRing<IntegerRing,u16>`, and use Symbolica's fraction-free reducer
as an independent exact oracle. Provenance columns must be included whenever
matrix content is stripped.

This dense representation is not appropriate for an unrestricted K6
Macaulay block. The production representation should remain sparse in Ore
keys. Symbolica does not currently expose a sparse fraction-free row reducer;
its `SparseRowReducer` is field-based. RustRed may orchestrate sparse Ore
entries and call Symbolica scalar ring operations, but must not implement a
parallel polynomial/GCD subsystem.

## Symbolica public API inventory

The workspace's actual dependency is the vendored Symbolica tree declared in
the workspace `Cargo.toml`. The reference-only checkout currently exposes the
same relevant APIs. The following are public and should be used directly.

| Purpose | Public Symbolica API | Local source |
|---|---|---|
| Prime fields | `Zp`, `Zp64`, `FiniteFieldCore`, `FiniteFieldElement`, `ToFiniteField` | [`domains/finite_field.rs`](../../vendor/symbolica/lib/numerica/src/domains/finite_field.rs) |
| Faster single-prime screen | `FiniteField<Mersenne64>` for `2^61-1` | [`domains/finite_field.rs`](../../vendor/symbolica/lib/numerica/src/domains/finite_field.rs) |
| Sparse polynomial evaluation | `MultivariatePolynomial::evaluate_with_coeff_map` | [`poly/polynomial.rs`](../../vendor/symbolica/src/poly/polynomial.rs) |
| Polynomial shifts | `shift_var`, `shift_var_cached` | [`poly/polynomial.rs`](../../vendor/symbolica/src/poly/polynomial.rs) |
| Coefficient mapping | `map_coeff` | [`poly/polynomial.rs`](../../vendor/symbolica/src/poly/polynomial.rs) |
| Polynomial content | `content`, `div_coeff`, `make_primitive` | [`poly/polynomial.rs`](../../vendor/symbolica/src/poly/polynomial.rs) |
| Exact polynomial quotient | `MultivariatePolynomial::try_div` | [`poly/polynomial.rs`](../../vendor/symbolica/src/poly/polynomial.rs) |
| Pair GCD | `MultivariatePolynomial::gcd` | [`poly/gcd.rs`](../../vendor/symbolica/src/poly/gcd.rs) |
| Multiple GCD | `MultivariatePolynomial::gcd_multiple` | [`poly/gcd.rs`](../../vendor/symbolica/src/poly/gcd.rs) |
| Ring adapter | `PolynomialRing::new`, `PolynomialRing::from_poly` | [`poly/polynomial.rs`](../../vendor/symbolica/src/poly/polynomial.rs) |
| Rational normalization | `RationalPolynomial` `Add`, `Mul`, `Div`, `FromNumeratorAndDenominator` | [`domains/rational_polynomial.rs`](../../vendor/symbolica/src/domains/rational_polynomial.rs) |
| Expanded finite-field oracle | `RationalPolynomial::to_finite_field` | [`domains/rational_polynomial.rs`](../../vendor/symbolica/src/domains/rational_polynomial.rs) |
| CSR sparse matrix | `SparseMatrix::from_csr`, `from_triplets` | [`tensors/sparse.rs`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs) |
| Field row reduction | `SparseRowReducer::new`, `add_row`, `pivots`, `u` | [`tensors/sparse.rs`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs) |
| Dense fraction-free reduction | `Matrix::partial_row_reduce_fraction_free`, `back_substitution_fraction_free`, `solve_fraction_free` | [`tensors/matrix.rs`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs) |
| Dense content removal | `Matrix::content`, `div_scalar`, `primitive_part` | [`tensors/matrix.rs`](../../vendor/symbolica/lib/numerica/src/tensors/matrix.rs) |

RustRed already calls the correct black-box finite-field evaluator in
[`completion/frame/modular/sample.rs`](../../crates/rustred-core/src/foundry/completion/frame/modular/sample.rs):
it maps exact integer coefficients into `Zp64`, evaluates numerator and
denominator separately, rejects a zero denominator, and only then divides.
That code should be extracted or generalized rather than replaced.

RustRed also already wraps native polynomial GCD and exact division with
context validation, typed failures, unwind containment, output
authentication, and retained-support accounting in
[`completion/frame/exact/cleared/budget.rs`](../../crates/rustred-core/src/foundry/completion/frame/exact/cleared/budget.rs).
The new exact lane should reuse that owner.

`FactorizedRationalPolynomial` was considered. Symbolica retains denominator
factors, but addition still expands adjusted numerators and division may
factor a numerator. It therefore does not remove the principal numerator
support risk and is not the recommended Janet hot representation. It remains
a possible diagnostic comparison, not a new RustRed abstraction.

## RustRed orchestration versus CAS ownership

### Symbolica must own

- finite-field construction, conversion, arithmetic, and inversion;
- sparse polynomial construction and evaluation;
- polynomial translation primitives;
- polynomial addition, subtraction, multiplication, and powers;
- integer and polynomial content;
- pair and multiple polynomial GCD;
- exact polynomial division;
- rational-polynomial normalization and cancellation;
- dense fraction-free matrix elimination; and
- sparse field row reduction.

### RustRed must own

- the exact coefficient-context and variable-map authentication boundary;
- conversion between sector forward shifts and physical index translations;
- the Ore action and ordering identity;
- Janet divisor eligibility and completion obligation selection;
- sparse mapping from Ore/provenance keys to matrix columns;
- deterministic probe scheduling and trace consensus;
- localization origin and exceptional-domain bookkeeping;
- source-module provenance identity and exact replay;
- operation, support, cache, and trace resource policy; and
- artifact admission and publication.

Implementing a hash-consed scheduling DAG is not a replacement CAS: its
leaves and all scalar evaluations delegate to Symbolica, and it is incapable
of publishing an exact coefficient. Likewise, a sparse Ore container is
domain orchestration. It must call Symbolica for every polynomial operation
rather than growing handwritten finite-field, GCD, quotient, factorization,
or polynomial-normalization code.

## Resource boundaries

### Modular lane limits

Declare and preflight at least:

- `max_scheduled_probes`;
- `max_good_probes` and `max_rejected_probes`;
- `max_dag_nodes` and `max_dag_edges`;
- `max_dag_depth` or iterative-evaluation work;
- `max_distinct_translations`;
- `max_evaluation_cache_entries_per_probe`;
- `max_total_evaluation_cells`;
- `max_live_row_terms` and `max_total_basis_terms`;
- `max_basis_rows` and `max_pending_obligations`;
- `max_normal_form_steps` and `max_divisor_visits`;
- `max_trace_steps` and `max_trace_bytes`;
- `max_sparse_rows`, `max_sparse_columns`, and `max_sparse_nonzeros`;
- `max_sparse_reducer_fill`; and
- `max_exact_guided_replay_steps`.

Use checked `usize` arithmetic and fallible reservations. Node IDs must be
acyclic by construction; do not permit unbounded recursive evaluation.

### Fraction-free lane limits

Declare separately:

- maximum polynomial operations;
- prospective multiplication term pairs;
- cumulative GCD operand term pairs;
- retained polynomial terms;
- augmented row and provenance entries;
- denominator-LCM steps and retained LCM support;
- exact divisions;
- translated polynomial support and exponent cells;
- dense oracle rows, columns, cells, and bytes; and
- final rational materialization support.

The existing cleared-circuit limits are a sound starting pattern. Native
Symbolica GCD, quotient, factorization, and multiplication do not expose a
hard scratch census. Catching a panic is not protection from process-level
OOM. Document these limits as conservative admission envelopes, not hard RSS
guarantees.

## Implementation slices

### Slice M0: bounded modular coefficient evaluator

Add a private `involutive/modular` module containing only:

- probe identity and limits;
- the coefficient DAG arena;
- exact-leaf evaluation through the existing Symbolica path;
- shifted evaluation caches;
- typed singular/budget failures; and
- unit tests.

No completion or artifact API enters this slice.

### Slice M1: modular normal form against a frozen epoch

Implement modular monic normalization, Ore left AXPY, greatest-reducible-term
selection, and a bounded structural reduction trace against one immutable
exact Janet epoch. Compare three independent probes with the existing exact
normal form on synthetic, one-loop, and two-loop cases.

This is the first meaningful performance seam: it measures coefficient DAG
growth and translated-evaluation reuse without restructuring the whole
completion engine.

### Slice M2: stable completion guide

Run independent modular completion lanes, group complete traces, require
discovery quorum and held-out agreement, and return a
`ModularJanetGuide`. The type must have no conversion into a relation,
completion certificate, or artifact.

Screen a small deterministic portfolio of admissible variable/block orders
and score each stable guide by a lexicographic resource tuple such as:

```text
(incomplete,
 rejected_probes,
 basis_rows,
 peak_row_terms,
 total_dag_nodes,
 normal_form_steps,
 divisor_visits,
 trace_bytes,
 canonical_order_id)
```

Do not introduce MCTS until this bounded portfolio establishes that modular
scores predict exact replay cost.

### Slice E0: shared exact indexed-polynomial owner

Promote the GCD, LCM, exact-division, polynomial conversion, and budget logic
from the cleared-circuit prototype into a narrow internal service. Preserve
its variable-map authentication and typed failure behavior.

### Slice E1: one projective fraction-free cancellation

Add a private `PrimitiveOreConsequence`, rational ingress clearing, exact
augmented source replay, and one GCD-scaled pseudo-reduction. Compare it with
the existing rational AXPY on small rows, including active and inactive
sectors.

Do not yet replace `JanetBasisEpoch` or remove its monic invariant.

### Slice E2: guided exact replay

Replay one stable modular trace using projective polynomial rows. Validate
every expected target, divisor, operator translation, leader, and support.
On divergence, return the exact partial state to the canonical exact
scheduler; never improvise from the remaining modular trace.

Materialize monic `OreConsequence` values only for retained rows that must
cross an existing exact boundary. Measure whether rational materialization
merely moves the peak to that boundary.

### Slice E3: bounded completion integration

Only if E2 materially improves the peak, add projective rows as a Janet epoch
representation and audit all explicit monic assumptions. Then exactly process
every deferred modular-zero obligation and run the ordinary exact complement
and exceptional-domain checks.

### Grounding experiment

After each completed slice, use the same release-built bounded K6 orbit
matrix, but stop after a fixed obligation/revision budget. Compare against the
recorded exact trajectory. Do not call a modularly exhausted queue or a
resource stop closure.

A practical integration gate is:

- identical exact leaders and zero/nonzero outcomes through the bounded
  reference window;
- deterministic output across supported worker counts;
- no source replay or guard divergence;
- at least a substantial reduction in peak retained/projected support and
  RSS—roughly fourfold would justify the additional representation; and
- GCD time not becoming the new dominant unbounded cost.

## Required tests

### Modular coefficient and Ore tests

- Direct exact specialization of `c(n+delta)` equals modular DAG evaluation
  at the translated point.
- Active and inactive sector translations use opposite physical signs.
- Nested translations compose exactly.
- Translation wraps correctly in the finite field without machine-integer
  overflow.
- A denominator valid at the base point but zero after a shift rejects only
  that probe.
- An `Inv` node with a zero image rejects the lane.
- A modular accidental zero in one prime does not become `KnownZero`.
- A nonzero image restores the expected generic leader in another lane.
- Source/family guard zeros and denominator zeros are distinguished.
- Residue-equivalent probe tasks are rejected.
- Complete-trace disagreement returns an inconclusive guide, not a skip.
- Trace selection is unchanged by probe execution order and worker count.
- Hash collisions receive full structural comparison.

### Modular/exact authority tests

- A deliberately false modular zero remains in the exact obligation queue.
- Exact replay detects a deliberately false expected leader.
- Exact replay detects a wrong divisor birth ID or operator shift.
- Replay divergence falls back to exact scheduling from an authenticated
  state.
- A modularly zero final queue cannot construct an artifact.
- Exact queue exhaustion and complement validation remain mandatory.
- Every source-module provenance and localization guard is exact after
  replay.

### Symbolica parity tests

- DAG leaf evaluation equals separate Symbolica numerator/denominator
  `evaluate_with_coeff_map` evaluation.
- On nonsingular small examples, DAG results equal
  `RationalPolynomial::to_finite_field(...).evaluate(...)`.
- Direct exact translated specialization equals modular shifted evaluation.
- Small CSR rank/pivot chronology agrees with Symbolica's serial
  `SparseRowReducer`.

### Fraction-free tests

- Rational row and provenance denominators clear exactly.
- A nonexact polynomial division is rejected.
- `gcd(A,B)` scaling cancels the selected leader exactly.
- The subject target strictly descends under the frozen Ore order.
- Active and inactive translations are identical to the rational reference.
- Augmented content removal divides every row and provenance entry.
- A row-only common factor not shared by provenance is not removed.
- Exact source replay succeeds after every cancellation and normalization.
- Projective and rational normal forms have equal support and leaders after
  cross-multiplication by a common scalar.
- Zero/nonzero outcomes agree with the rational exact lane.
- Localization contains all cleared denominator, source/family, translated
  divisor, and final leader conditions required by policy.
- Final monic materialization reproduces the projective row and provenance.
- The small dense Symbolica fraction-free matrix oracle agrees with the sparse
  projective update.
- Input permutation and worker count do not change canonical exact output.

### Resource tests

Exercise typed failures for every DAG-node, translation, cache, row-term,
trace, probe, polynomial-operation, term-pair, GCD-pair, LCM-support,
exact-division, retained-support, and matrix-cell cap. Where feasible, inject
fallible-allocation failures. A rejected modular probe must release its cache,
and no partial output may escape either arithmetic boundary.

## Failure modes and mitigations

### Modular support may remain unstable

Different points can follow different pivot histories. Whole-trace consensus,
held-out probes, deterministic replenishment, and exact fallback are the
mitigation. Increasing the number of zero samples is not a proof.

### The DAG may become an unbounded arithmetic circuit

Hash-consing removes repeated structure but cannot guarantee compactness.
Bound nodes, edges, translations, depth, and evaluation cells. Periodically
measure live reachability and permit deterministic arena compaction only at an
epoch boundary.

### Exact replay may diverge early

A modular leader can vanish exactly only if the modular construction or trace
binding is wrong; more commonly, an exact cancellation can change later
support relative to an unlucky modular lane. Treat divergence as normal
proposal failure and resume exact scheduling. Never patch one step using a
different probe's trace.

### Fraction-free multiplication may still explode

Removing denominators does not compress polynomial numerators. The term stop
may move from rational addition to polynomial multiplication. GCD-scaled
pseudo-reduction and bounded augmented content extraction mitigate this, but
the outcome must be measured.

### GCD may dominate

Symbolica's multivariate GCD is optimized but exposes no scratch census.
Compute it at deliberate boundaries, stop content scans at one, and retain
operation/operand telemetry. Do not GCD every coefficient pair blindly.

### Denominator LCM may explode

Clear original polynomial-like sources once. Refuse repeated conversion of
mature rational rows. Bound LCM steps and retained support.

### Provenance can erase apparent content

A physical row may have a large common factor that its source-module vector
does not share. Removing it is not an exact polynomial-source operation.
Normalize the augmented vector or retain an explicit rational projective
scale and its guard; the first implementation should choose augmented
normalization.

### Dense fraction-free matrices may exhaust memory

Use Symbolica's dense routine only for small trace-selected blocks and tests.
Keep the production Ore representation sparse. A generic sparse
fraction-free linear-algebra engine should be added to Symbolica, not
duplicated privately in RustRed, if it becomes necessary.

### Guard sets may accidentally weaken

Clearing denominators and avoiding explicit divisions can make intermediate
conditions disappear syntactically. Preserve the current localization lineage
in the first lane. Any later guard minimization requires an explicit cleared
identity proof and exceptional-domain audit.

### Existing monic assumptions may leak

The current epoch builder, normal form, tests, and coefficient census assume
unit leaders. Keep the projective lane behind a distinct type until every
consumer is audited. Do not smuggle a nonmonic row into `OreConsequence` and
rely on debug assertions.

### `gcd_multiple` may find nonmaximal content

Nonmaximal content removal affects performance only. Exact division of every
augmented entry is the authority check. A candidate that fails any division
is rejected.

## Relative value and recommended order

| Intervention | Expected value | Engineering cost | Main risk | Recommendation |
|---|---:|---:|---|---|
| Janet divisor index / structural sharing | High | Medium | bookkeeping bugs | retain as prerequisite infrastructure |
| Modular schedule/order discovery | Very high | Medium-high | unstable traces or oversized DAG | implement first |
| Projective fraction-free exact replay | Medium-high | High | products/GCD replace denominator swell | implement as guided replay backend |
| GCD-reduced stop diagnostic | Medium diagnostic value | Low-medium | double GCD work | cold, first rejected operation only |
| Factorized rational coefficients | Low-uncertain | Medium | numerator expansion and factorization cost | do not select as primary lane |
| Dense fraction-free F4 blocks | High on small blocks | Medium | density and provenance width | oracle and bounded blocks only |
| Raising exact term caps | Low | Low | worse time/RSS without mechanism change | not a cure |

The two proposed lanes address different scaling axes:

- modular scheduling reduces how much exact work is attempted and discovers
  order sensitivity cheaply;
- fraction-free replay reduces the rational-function cost of exact work that
  remains; and
- only the final exact replay and artifact boundary establish mathematical
  authority.

That separation should remain explicit in code, telemetry, documentation, and
user-visible status. A stable modular trace is encouraging evidence; an exact
queue-exhausted, guard-complete artifact is the milestone.
