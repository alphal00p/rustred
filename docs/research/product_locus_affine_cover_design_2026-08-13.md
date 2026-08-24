# Product-locus branching and simultaneous affine residual covers

Date: 2026-08-13

Status: authoritative next-stage design.  It refines the earlier dependent
affine-start notes after inspecting the exceptional leaves naturally generated
for the connected equal-mass sunset.  Production remains topology- and
loop-count-independent.  Sunset powers and sectors appear only as validation
fixtures.

Implementation status (2026-08-13): Coverage V4 now retains and replays the
exact product-factor provenance described below.  A bounded representation
decision either materializes a checked concrete product witness or keeps the
same zero locus as canonical factor atoms without multiplying.  In both cases
`ResidualProductLocusBooleanCoverCertificate` compiles that provenance into a
deterministic disjoint Boolean cover.  The next implementation boundary is the
simultaneous primitive-integer affine system for each ready Boolean terminal.
The older one-predicate `ResidualUnitAffineIndexMapCertificate` is not this
boundary: it cannot consume several zero atoms and its context-only replay does
not authenticate the family-scoped Boolean path.

## 1. Correct semantic order

An exceptional leaf can contain predicates such as

```text
p_1(n) * ... * p_r(n) = 0.
```

In the integral domain `K[n]`, this means

```text
p_1=0 OR ... OR p_r=0,
```

not one affine equation.  Several retained product equalities form a
conjunction of such disjunctions.  Therefore the production order is:

```text
authenticated atomic-locus canonicalization
  -> bounded product representation selection
       -> checked concrete product plus exact factor witness, or
       -> canonical factor-only disjunction with no product multiplication
  -> exact provenance replay
  -> Boolean CNF normalization and disjoint branching
  -> affine recognition of selected zero atoms
  -> simultaneous integer-affine system compilation per branch
  -> affine prepare points and generated-row restriction
  -> persistent case-bound elimination and coverage feedback
```

Skipping the Boolean step and row-reducing an expanded product is unsound.
Sequentially applying the existing one-equality map is also insufficient: it
makes substitution order semantic, cannot prove simultaneous consistency, and
can lose path nonzero guards.

This Boolean-first layer is the Symbolica-native counterpart of the
`LogicalExpand`/residual-case workflow around LiteRed's `SolvejSector`; it is
not a topology-specific recurrence.

## 2. Retain construction provenance; do not refactor post hoc

RustRed coverage already knows the atomic polynomials before it considers
product compression in `CandidateBadFormula::try_new`.  The implementation
must retain that knowledge in the replayable coverage proof and must decide
whether to expand the product before native multiplication.

A public typed witness should have the logical shape

```rust
pub struct ProductLocusDecompositionWitness {
    product_locus_ordinal: usize,
    factor_locus_ordinals: Box<[usize]>,
    reconstruction: ProductLocusReconstructionStats,
}
```

and the coverage certificate should retain one canonical structural-locus
table.  A future queue/extraction certificate can then resolve a final case
predicate's polynomial to its exact product witness and factor polynomials.
Every factor and product remains authenticated by the same `K(n)` context and
source coverage transcript.

For a materialized product, replay repeats the same bounded multiplication in
canonical factor order and requires the reconstructed polynomial to be a
proved associate of the retained representative by a nonzero unit of the base
field `K`.  Literal polynomial equality is the fast path; the nonliteral path
uses checked exact `K(n)` division and accepts only a quotient in `K`.  This is
the correct identity for zero loci: `p=0` and `u*p=0` are the same predicate
for every `u in K^*`.  Downstream partition predicates clone the exact retained
representative, so predicate-to-witness resolution itself needs neither
division nor factorization.

Before materialization, compute the conservative whole-product support bound

```text
min(product_j terms(p_j), product_i(1 + sum_j degree_i(p_j))).
```

The calculation saturates just above a persisted representation cutoff and
scans each flat exponent payload once.  The componentwise maximum-degree box
deliberately matches and dominates every prefix box used by sequential checked
Symbolica multiplication; translated sparse supports are therefore not shifted
down to their minimum exponents for this representation decision.  If the
bound exceeds the cutoff, do no product multiplication: retain the sorted
canonical factors directly as `p_1=0 OR ... OR p_r=0`.  The ordinary coverage
router then creates the exact disjoint refinement `p_1=0`, `p_1!=0 AND p_2=0`,
and so on.  Such a fallback creates neither a concrete product representative
nor a decomposition witness; replay rebuilds the same factor route from the
authenticated candidate atoms, stored cutoff, limits, and statistics.  The
final typed partition and complete payload equality authenticate that private
representation decision.

Nonzero base-only factors are units of `K` and are omitted from the zero-locus
factor list only after ordinary exact context validation proves they contain no
index variable.  Factor ordinals are then sorted and deduplicated.  Thus a
repeated source factor does not survive as duplicate Boolean or reconstruction
payload: multiplicity is immaterial to the reduced zero-locus witness.

Do not make `MultivariatePolynomial::factor()` part of this production proof.
The current Symbolica factorizer is valuable as an optional oracle, but its
randomized and internally unmetered work is not a hard RustRed resource
boundary.  An opaque product without construction provenance remains an
honest unsupported factor branch unless a separately designed bounded
factorization certificate is supplied.

## 3. Boolean normalization

After resolving retained provenance:

- `EqualZero(ProductOf[f_1,...,f_r])` becomes the positive clause
  `(f_1=0 OR ... OR f_r=0)`;
- `NonZero(ProductOf[f_1,...,f_r])` becomes the conjunction
  `f_1!=0 AND ... AND f_r!=0`;
- an opaque recognized affine equality is a unit positive clause; and
- an opaque nonlinear equality remains a typed unsupported atom.

Canonicalize factors up to a proved unit of `K`, deduplicate literals, remove
duplicate clauses, and apply only sound clause subsumption: if `A` is a subset
of `B`, then `B` is redundant in `A AND B`.  No heuristic radical or sampling
equivalence is allowed.

Build a disjoint cover with deterministic bounded DPLL/Shannon expansion:

1. Propagate zero/nonzero path facts.
2. Remove satisfied clauses and known-nonzero literals.
3. An empty clause is a proved contradiction.
4. A unit clause forces its atom to zero.
5. Apply exact coordinate contradiction and source-orthant pruning.
6. Otherwise branch on the canonical smallest atom of the shortest clause,
   zero child first and nonzero child second.

The path facts are part of the certificate.  Two terminals with the same
affine map but different nonzero exclusions are not interchangeable; either
keep them separate or prove and retain the exact guard disjunction before
merging.

The public result is a `ResidualAffineLocusCoverCertificate`, not a naked list
of maps.  Each branch owns its Boolean path and has one outcome:

```text
proved empty | unsupported | affine map with retained nonzero guards
```

## 4. Recognizing integer-affine atoms

Reuse the base-block associate method already implemented for the one-row map.
For every candidate factor:

1. require index degree at most one and at most one index variable per
   monomial;
2. group terms by their formal-base exponent block;
3. find an integer coefficient vector `c + sum_i a_i n_i`;
4. require every other base block to be an integer scalar multiple of the
   same primitive vector using exact `Z.quot_rem`; and
5. primitive-normalize the integer row by its coefficient gcd and a fixed sign
   convention.

Literal coordinate assignments become rows in the same system.  A base-
dependent affine offset, nonlinear factor, or nonassociate base block is a
typed unsupported branch, never an empty branch.

## 5. Simultaneous integer-affine systems

For one Boolean terminal, collect its selected zero rows and literal rows into
an arbitrary-precision integer augmented matrix.  Deterministically
primitive-normalize, sort, and deduplicate rows.  Run bounded fraction-free
elimination with an explicit source-lineage and row-operation transcript.
Symbolica's fraction-free matrix routines may be used as a test oracle, but
the certificate needs RustRed's own replayable transcript.

V1 of this multi-row compiler may accept only unit-pivot systems.  This is
enough for the currently observed natural sunset components.  Outcomes are:

- `[0 ... 0 | q]`, `q != 0`: proved inconsistent/empty;
- a zero row: redundant;
- a complete unit-pivot system: exact integer-affine parametrization; and
- a consistent nonunit system requiring congruences: typed
  `GeneralCongruenceCaseNotSupported`.

HNF/SNF can later extend the last case; rationalizing it would silently admit
noninteger points and is not acceptable.

### 5.1 Concrete bounded V1 algorithm

Rows use the convention

```text
c + a_0 n_0 + ... + a_(N-1) n_(N-1) = 0.
```

All entries are Symbolica `Integer` values.  Canonical input rows are sorted
and duplicate rows merge their complete structural-locus lineage.  At a solver
state with some pivot columns already eliminated, candidate columns are tried
in increasing original-index order.  A column is eligible when the gcd of its
coefficients in the active rows is one.  For two active rows `P,R` with column
entries `a,b`, use

```text
(g,s,t) = extended_gcd(a,b)
P' = s P + t R
R' = -(b/g) P + (a/g) R.
```

The two-by-two transformation has determinant one.  Repeating it creates a
`+1` pivot, after which ordinary integral row additions clear that column from
every other row.  A bounded deterministic DFS is required: an eligible column
need not belong to a complete unit minor even when another eligible column
does.  The lexicographically first successful pivot sequence is canonical.
Every branch charges its state clone, row entries, integer-bit growth,
operations, and lineage before allocation or GMP arithmetic.

At every state:

- a zero coefficient row with nonzero constant proves the branch empty;
- `gcd(a_i)` not dividing `-c` also proves it empty;
- no remaining nonzero row completes the affine map; and
- remaining equations with no complete unit-pivot path return
  `GeneralCongruenceCaseNotSupported`.

The last outcome is intentional.  For example, `2 n_0+n_1=0` is supported by
pivoting `n_1`, while `2 n_0+3 n_1=0` needs a new lattice parameter and is not
silently widened to rational points.

### 5.2 Symbolica Rust API boundary

RustRed's authenticated polynomial is
`MultivariatePolynomial<IntegerRing,u16>`; coefficients are arbitrary-
precision Symbolica `Integer` values.  After
`ParametricCoefficientContext::validate_polynomial_with_limits`, recognition
uses `raw.coefficients.iter().zip(raw.exponents_iter())`.  Base variables
precede private index variables and the polynomial uses lexicographic order, so
equal base-exponent prefixes are contiguous and can be streamed without a hash
map or factorization.

Use `Integer::gcd`, `Integer::extended_gcd`, and `Z.quot_rem` (requiring zero
remainder) for exact integer work.  Do not use integer `/` as an exactness
proof.  Symbolica's `Matrix<IntegerRing>` fraction-free routines are useful as
test oracles only: they expose neither a RustRed work budget nor a row-
operation/source-lineage transcript.  Production therefore owns the bounded
row engine above.  Polynomial `content`, `make_primitive`, unrestricted
rational-polynomial variable unification, `Atom` pattern matching, and
post-hoc polynomial factorization are outside this proof boundary.

For pivot positions `P` and free positions `F`, construct

```text
n_p = b_p + sum_f B[p,f] t_f,
n_f = t_f.
```

Verify every original row exactly by substituting `b` and every column of
`B`.  Verify the full projection form `F(n)=b+A n` satisfies

```text
A*A = A,
A*b = 0,
```

so `F(F(n))=F(n)`.  Unit pivots also prove that every integer solution has one
unique integer free parameter tuple.  Reduce retained nonzero path atoms
through this map: zero proves a contradiction, a nonzero constant is
discharged, and a nonconstant result remains a branch guard.

## 6. Replay identity and resource limits

The cover must bind structurally:

- family/context/sector and exact source queue/extraction allocation at the
  live construction seam;
- source case, work-item, predicate, factor, and bound/pivot locators;
- the retained structural-locus table and product reconstructions;
- canonical atoms, CNF, DPLL ordering, path facts, propagations, and prunes;
- normalized affine source rows and lineages;
- every fraction-free row operation, rank, pivots, free columns, `b`, and
  matrix `A`;
- reduced nonzero guards and terminal outcome; and
- schemas, configured limits, and complete statistics.

Use pointer equality only to ensure a freshly generated map shares the exact
queue extraction allocation.  Persisted/reconstructed replay uses complete
typed payload equality.

Budgets are cumulative and checked before allocation or native work.  The
representation decision separately charges canonical factor scans, inspected
flat exponent entries, factored disjunctions, and factor references.  A bound
strictly above `max_materialized_product_zero_support_terms` selects the
factor-only route and leaves all product-witness, multiplication,
reconstruction, and associate counters untouched.  Equality to the cutoff
still selects materialization.  The cutoff is a representation policy, not an
algebra allowance: callers may configure it independently, and selecting
materialization never catches or launders a later exact-algebra failure into a
fallback.

Each selected product reconstruction charges candidate monomial pairs, actual
sparse output terms, dense exponent entries, and actual integer
coefficient-magnitude bits.  Before calling Symbolica, the proved direct-
polynomial support envelope is checked against both a coverage-local per-
product native-output ceiling and the remaining aggregate output, exponent,
and coefficient-bit budgets.  This transient envelope is separate from
`ExactAlgebraLimits::max_polynomial_terms`: that retained exact-algebra ceiling
continues to authenticate both inputs and the actual canonical output after
native multiplication.  Only the term-operation allowance is decreased by the
remaining aggregate pair budget.  Associate lookup similarly charges every
comparison and the checked division term-pair bound, passing only the remaining
aggregate allowance to the exact operation.  A sequence of calls therefore
cannot reset a per-call algebra budget or widen the retained polynomial cap.

Replay has a deliberately algebra-free first phase.  It exactly recenses the
retained structural terms and bounded display bytes, concrete witness count,
witness factor references, and implied multiplication count; rejects singleton
or unsorted witnesses, invalid ordinals, and noncanonical witness ordering; and
compares this census with stored statistics.  Factor-only lists are private
compiler material rather than duplicated certificate payload, so this phase
can only range-check their aggregate statistics.  Deterministic full rebuild
from the authenticated attempts then reconstructs the same factor route and
the final complete payload comparison authenticates its exact count and
partition.  Only materialized routes perform reconstruction multiplication or
`K`-unit-associate checks.  Factor and decomposition staging is preflighted
against the remaining aggregate limits before allocation.

Budgets must cover at least:

- retained locus/product/factor terms, exponent entries, coefficient bits,
  and bytes;
- representation-bound factor scans/exponent entries, factored disjunctions
  and factor references;
- product witnesses, witness factor references, multiplication term pairs,
  reconstructed output terms/exponent entries/coefficient bits, and
  structural-locus associate comparisons/division term pairs;
- Boolean atoms, clauses, literals, associate/subsumption comparisons;
- DPLL nodes, frontier, depth, branches, path facts, propagations, and state
  bytes;
- affine rows, matrix entries, coefficient-bit growth, row operations,
  lineage entries, and retained `b/A` bytes; and
- replay and peak scratch shapes.

Catch Symbolica panics at native boundaries, but do not describe this as a
process-wide OOM or wall-clock guarantee.

## 7. Natural sunset validation

Let `M=i64::MAX`, the checked index-boundary root occurring in the current
depth-zero search.  After removal of nonzero base units, the expected clauses
are:

- sector `011`:
  `(n0=0 OR n2=M OR n1=1) AND
   (n0=0 OR n1=M OR n2=1)`;
- sector `101`: the corresponding `n0 <-> n1` image;
- sector `110`:
  `(n2=0 OR n2=-1 OR n0=M OR n1=1) AND
   (n2=0 OR n2=-1 OR n0=1) AND
   (n2=0 OR n0=1 OR n1=M)`;
- sector `111`:
  `(n0=1 OR n2=M) AND
   (n1=1 OR n2=M) AND
   (n0=0 OR n0=M OR n2=1)`.

Orthant and Boolean pruning yield the familiar set-theoretic components, but
the certificate must retain disjoint path guards rather than merely listing
overlapping affine varieties.

Tests must replay every product witness, compare the cover truth table on a
bounded integer grid, verify pairwise branch disjointness and union equality,
check every map's original-row identities/idempotence/free-variable bijection,
and compare affine-restricted generated rows with direct concrete
specialization.  Synthetic tests must cover overlapping clauses, repeated and
associate factors, base units, contradictions, dependent/redundant rows, a
genuinely dependent unit system, nonunit congruence rejection, nonlinear and
base-dependent factors, every resource boundary, and replay tampering.
