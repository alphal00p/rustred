# RustRed reducer scaling audit

Priority update (2026-08-24): the governing deployment target is now a
six-loop, unit-mass vacuum-rule foundry plus a separate high-throughput
GammaLoop/BPHZ application runtime.  The measurements and failure analysis in
this audit remain valid.  Section 8 has been revised accordingly; the complete
architecture is
[`six_loop_single_scale_vacuum_priority_2026-08-24.md`](six_loop_single_scale_vacuum_priority_2026-08-24.md).
The deterministic multicore decomposition and multi-start scheduler are
specified separately in
[`parallel_campaign_foundry_design_2026-08-26.md`](parallel_campaign_foundry_design_2026-08-26.md).

## Scope and conclusion

This audit covers RustRed's generic seed enumeration, momentum-space IBP
generation, integral order, sparse elimination, cache/certificate boundary, and
the observed three-loop scaling behaviour.  It compares those pieces with the
checked-in LiteRed2 and Symbolica sources.  The sources were read as text; FORM,
Mathematica, and Cargo were not invoked.

The short conclusion is:

- the current `IbpGenerator` is a sound exact generator for the ordinary
  vacuum identities `d/dk_i . k_j`;
- the current seed enumerator correctly describes a finite total-degree set,
  but it is being asked to serve three different jobs: public targets, solver
  seeds, and the IBP dependency halo;
- the current `SparseReducer` is a useful exact prototype, but its eager global
  `BTreeMap` substitution and full rational-function arithmetic cannot be the
  four-/five-loop engine;
- the three-loop dot/numerator failures are expected from the shift geometry,
  not evidence for extra masters: a seed at `(D,N)` can emit a same-sector
  column at `(D+1,N+1)`, and the first observed extra terminal is itself a
  factorized tree-sector numerator which the present scalar boundary reducer
  deliberately does not handle;
- complete finite three-loop boxes with `D,N >= 1` should be obtained first by
  exact numerator-aware boundary reduction plus bottom-up, sector-local,
  target-driven Laporta elimination with an adaptive seed halo;
- scalable four-/five-loop work additionally requires integral interning,
  automatic factorization, ISP-aware symmetry maps, finite-field pivot/rank
  discovery, rational reconstruction, resumable per-sector caches, and exact
  row provenance;
- neither a noncommutative s-basis nor parametric syzygies are prerequisites for
  the next finite three-loop box.  They become useful for compact all-index
  recurrences and for reducing the size of high-loop scalar systems after the
  finite solver is trustworthy.

## 1. What the current code actually computes

### 1.1 Seed domain

`SeedConfig` uses the two total degrees

\[
 r(a)=\sum_i\max(a_i-1,0),\qquad
 s(a)=\sum_i\max(-a_i,0).
\]

For a physical propagator a seed entry is either positive, `1+dots`, or, when
subsectors are enabled, nonpositive.  An auxiliary entry is always
nonpositive.  Enumeration keeps `r <= max_dots` and
`s <= max_numerator_degree`, removes scaleless integrals and exact permutation
duplicates, then sorts by the family order
([`src/reduction.rs:8-248`](../../src/reduction.rs#L8)).  This is the right
finite-domain semantics.

For `P` physical propagators, `A` auxiliary scalar products, and all
subsectors, the pre-filter candidate bound implemented by the code is

\[
 \sum_{q=0}^{P}{P\choose q}{D+q\choose q}
 {N+A+P-q\choose A+P-q},
\tag{1}
\]

where `q` is the number of active physical lines.  Equation (1) is conservative
and correct.  Its size also shows why eager all-sector enumeration is not a
high-loop policy:

| family shape | raw candidates `(D,N)=(1,1)` | raw candidates `(2,2)` |
|---|---:|---:|
| four-loop H/X, `P=9,A=1` | 17,152 | 200,832 |
| four-loop BMW/FG, `P=8,A=2` | 8,448 | 98,112 |
| current five-loop banana, `P=6,A=9` | 3,232 | 57,352 |
| representative five-loop cubic shape, `P=12,A=3` | 274,432 | 5,876,224 |

Each seed emits `L^2` rows.  Thus the last `(2,2)` entry would mean about
147 million raw five-loop identities before useful sector pruning.  Symmetry
and zero filtering help, but they cannot turn eager enumeration into a viable
general strategy.

The enumerator should therefore remain as a low-level exact primitive.  A new
solver policy should choose a sector, an exact shell, and a demand frontier;
`include_subsectors=true` should not be the normal high-loop entry point.

### 1.2 IBP row geometry

For a seed `a`, the generator emits all `L^2` vacuum identities.  Its only
index shifts are

```text
a + e_r
a + e_r - e_s
```

with the coefficient `-a_r`, plus the diagonal `d I(a)` term
([`src/ibp.rs:31-114`](../../src/ibp.rs#L31)).  It checks exponent-vector
arity and checked `i32` shifts, and production generation canonicalizes every
term through exact zero and symmetry rules.  This agrees with LiteRed2's
`GenerateIBP`, which constructs the same symbolic shifts once and evaluates
them at index points
([`LiteRed2026.m:1799-1831`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1799)).

The important halo bound is two-dimensional.  If `r` is active and `s` is
inactive at power zero, the second shift produces

```text
dot degree       D -> D + 1
numerator degree N -> N + 1
```

simultaneously.  A row generated from a `(D,N)` seed therefore has columns
inside `(D+1,N+1)`, not merely a one-dot halo.  Other shifts can cross to a
proper subsector or redistribute dot degree.  One layer is an exact bound on
the columns of one row; it is **not** a theorem that one layer of extra seeds
closes a requested target box.

The generator itself does not need a new identity family for the next
milestone.  It does need a streaming API so a sector solver can consume one
seed's rows without materializing every `IbpIdentity`, and each row needs a
stable origin ID `(sector, seed, differentiated_loop, contraction_loop)`.

### 1.3 Integral order

RustRed currently compares

```text
(active propagators, r+s, r, physical-sector mask, exponent vector)
```

and treats the maximum as the pivot
([`src/family.rs:620-649`](../../src/family.rs#L620)).  It is a total,
well-founded order on the bounded `i32` representation and has the essential
dependency property: a proper subsector is easier because it has fewer active
physical lines.

LiteRed2 instead places the binary sector ID before its within-sector order
matrix.  Its default matrix starts with total displacement and then refines
denominator/numerator powers
([`LiteRed2026.m:1378-1441`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1378)).
The difference is not an algebraic bug, but RustRed's ordering interleaves
unrelated equal-line sectors by displacement.  That loses locality in a global
solve and changes pivots and candidate masters relative to LiteRed2.

The scalable design should make order an explicit, versioned object:

```text
global sector order:
  dependency rank, canonical sector ID

local order inside one sector:
  r+s, r, order-matrix dot products, exponent-vector tie break
```

Solving one sector at a time makes the cross-sector tie break irrelevant while
preserving strict descent.  Existing `order-v1` caches should not silently
change; introduce an `order-v2` fingerprint.

There is a separate symmetry subtlety.  `VacuumFamily::canonicalize` chooses
the lexicographically largest exponent vector, not the maximum under
`compare_integrals`
([`src/family.rs:526-547`](../../src/family.rs#L526)).  This is mathematically
valid as an orbit representative, but the representative sector can depend on
where inactive negative powers sit.  For example the observed canonical
terminal `I(2,1,-1,0,0,1)` is a three-edge factorized sector, yet its labelled
mask is not one of the scalar corner masks used by the current boundary
dispatcher.  Future numerator boundary code must classify the sector orbit and
retain the transforming map; it must not assume that numeric-integral
canonicalization always produces masks 7, 11, or 15.

### 1.4 Current elimination

`SparseReducer` clones all nonzero equations, sorts them by initial leading
integral, and processes them once.  For every row it repeatedly scans all terms
for a known pivot, substitutes a `HashMap` rule, divides by the new pivot, and
stores a triangular rule
([`src/reduction.rs:628-733`](../../src/reduction.rs#L628)).  Earlier rules are
not back-substituted when later rules appear; recursive reduction supplies the
final normal form.

This algorithm is exact.  Its performance costs are structural:

- `LinearCombination` is a `BTreeMap<Integral,Coefficient>`, ordered by raw
  exponent lexicography rather than pivot hardness, so every leading-term
  query scans the row
  ([`src/linear.rs:1-68`](../../src/linear.rs#L1));
- integral vectors and rational-polynomial coefficients are repeatedly cloned;
- each substitution performs fully normalized Symbolica rational-polynomial
  additions and multiplications;
- row choice among equations with the same leading column is generation order,
  with no nonzero-count or fill-in heuristic;
- all sectors and their boundary dependencies share one table;
- all identities are materialized before elimination;
- normal-form memoization is local to one reduction call, so coverage sweeps
  redo work;
- the table discards row origins and elimination operations.

The result is a good correctness oracle for small systems, not the backend to
optimize incrementally into a 5-loop solver.

## 2. Explaining the three-loop observations

The diagnostic all-sector runs gave:

| seed bound `(D,N)` | symmetry-unique seeds | rows | rules | observation |
|---|---:|---:|---:|---|
| `(0,0)` | 6 | 54 | 18 | six scalar corners plus `I(2,1,-1,0,0,1)` among all rule terminals |
| `(1,0)` | 16 | 144 | 68 | additional dotted/numerator terminals |
| `(2,0)` | 41 | 369 | 163 | additional terminals remain |
| `(3,0)` | 84 | 756 | 329 | additional terminals remain |
| `(1,1)` | 36 | 324 | 155 | numerator terminals remain |
| `(2,1)` | 95 | 855 | 346 | numerator terminals remain |
| `(2,2)` | — | — | — | global exact substitution became impractically slow |

These counts are exploratory terminal censuses over **every stored rule**.  An
extra terminal in an irrelevant halo rule does not by itself prove that a
public target fails to reduce.  Certification should trace only rules reachable
from the declared targets, while independently checking the algebraic origin
of every selected rule.

The first extra terminal is especially informative:

```text
I(2,1,-1,0,0,1)
```

It has three active lines and one inactive numerator.  Its sector is a
factorized spanning tree under an `S4` image, so the finite tensor/tadpole
algorithm in
[`three_loop_reduction_plan.md:209-319`](three_loop_reduction_plan.md#L209)
reduces it to `T1^3`.  It survives only because the implemented boundary layer
currently rejects every negative power
([`crates/rustred-legacy-oracles/src/three_loop_pipeline.rs:276-303`](../../crates/rustred-legacy-oracles/src/three_loop_pipeline.rs#L276)).
Consequently the first action for `D,N >= 1` is not a larger global matrix; it
is completion of the already-derived tree and paw numerator boundaries.

After boundary closure, the three genuine representative sectors must be
solved bottom-up:

```text
mask 43 (four-line banana)
  -> mask 31 (five lines)
    -> mask 63 (top tetrahedron)
```

Rows from a sector can only remain in that sector or enter a proper subsector.
Reducing proper-subsector terms before matrix insertion therefore removes a
large fraction of columns without changing the row space.  This is the key
structural advantage missing from the global solve.

The current corner pipeline correctly limits its public claim to `(D,N)=(0,0)`
and candidate masters.  It also now reduces a whole generated identity through
the sparse table before applying the terminal whitelist, allowing halo terms
to cancel
([`crates/rustred-legacy-oracles/src/three_loop_pipeline.rs:199-227`](../../crates/rustred-legacy-oracles/src/three_loop_pipeline.rs#L199)).
That whole-row behaviour should become a generic validation primitive.

The target/seed conflation is literal in the current builder: it assigns
`certified_targets(...)` to `seeds`, generates rows from that same collection,
and later calls the same target enumerator for coverage
([`crates/rustred-legacy-oracles/src/three_loop_pipeline.rs:88-102`](../../crates/rustred-legacy-oracles/src/three_loop_pipeline.rs#L88),
[`crates/rustred-legacy-oracles/src/three_loop_pipeline.rs:306-345`](../../crates/rustred-legacy-oracles/src/three_loop_pipeline.rs#L306)).
This is adequate for the corner slice, but it gives the solver no independent
halo policy at the next depth.

## 3. Correctness and certification findings

### 3.1 High priority: a loaded table is triangular, not proved

`ReductionTable` serialization checks the family fingerprint, canonical
integrals, triangular order, coefficient syntax, counts, limits, and a payload
checksum.  It does not store the seed set, input row hashes, row provenance, or
an elimination certificate.  A syntactically valid but algebraically false
triangular table can therefore be loaded.

`ThreeLoopReductionPipeline::from_table` calls only target-coverage validation
([`crates/rustred-legacy-oracles/src/three_loop_pipeline.rs:105-114`](../../crates/rustred-legacy-oracles/src/three_loop_pipeline.rs#L105)).
For the corner box, an empty table is enough to leave the three genuine corners
as whitelisted candidates while the other corners are handled analytically.
Thus the method name/comment “certify an externally loaded sparse table” is too
strong.  The build path additionally checks its generated identities, but the
cache format cannot reproduce that evidence after loading.

Required change:

```text
CertifiedReductionTable {
  family_fingerprint,
  order_fingerprint,
  solver_and_boundary_versions,
  target_spec,
  seed_shells,
  input_row_hashes,
  rules,
  derivation_certificate,
  pole_conditions,
}
```

Until this exists, rename the current constructor to `from_table_unchecked` or
require callers to provide and replay the identities used to build the table.
Coverage is necessary, but it is not algebraic certification.

### 3.2 High priority: no persisted rule provenance

`IbpIdentity` has a seed and derivative/contraction labels, but
`SparseReducer::reduce` copies only its equation and discards those labels.
Generated-row validation is then circular: the same rules built from the rows
reduce those rows to zero.

A compact exact certificate can be a derivation DAG:

```text
Input(row_id)
Scale(node, exact_coefficient)
AddScaled(node, pivot_rule_node, exact_coefficient)
Boundary(boundary_algorithm_version, transformation, input)
```

For a matrix backend, persist the ordered input-row hashes and enough exact row
operations to replay each selected pivot.  Symbolica's
`SparseRowReducer` with `LuLMode::Full` records an `L` for which its own test
checks `L*U=A`; however `back_substitute()` clears `L`
([`sparse.rs:1497-1513`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1497),
[`sparse.rs:1808-1829`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1808)).
RustRed can either retain triangular `U` and verify `L*U=A`, or record its own
back-substitution DAG.  The triangular `U` is already sufficient for recursive
rule application, so full RREF is optional.

### 3.3 Public arity and reducer-input validation

`IbpGenerator` returns a typed wrong-length error.  In contrast,
`ReductionTable::reduce_integral` reaches an assertion in
`VacuumFamily::canonicalize`, even though its public signature returns a
`Result`.  `SparseReducer::reduce` also accepts arbitrary public
`IbpIdentity` values without checking exponent arity, symmetry canonicality,
or scaleless keys.  Foreign or raw rows can panic or create unusable rule keys.

Add typed errors for:

- wrong integral arity on every public reduction surface;
- wrong arity in any input equation;
- noncanonical/scaleless terms passed to the production sparse reducer;
- optionally, an identity family fingerprint once identities become portable.

Keep a deliberately named `reduce_raw_unchecked` only for diagnostics.

### 3.4 Parameter domain is implicit

Every numeric finite-box pivot divides by a rational function of `d` and `m2`.
The table is valid over the generic field `Q(d,m2)`, not at special dimensions
or zero mass where a pivot denominator vanishes.  The current coefficient
object retains the denominator algebraically, but the table has no explicit
domain statement.  Guarded all-index recurrences will also have polynomial
conditions involving the matched indices.

Each rule/certificate should store its denominator factors, and a table should
expose the union of excluded parameter loci.  For a numeric finite box this is
metadata plus exact verification.  For a symbolic recurrence the nonzero
conditions are part of the rule guard and cannot be omitted.

### 3.5 Conservative zero detection is a scaling gap, not a wrong zero

The current family layer never declares a positive auxiliary-denominator
sector zero without the full Lee criterion
([`src/family.rs:549-617`](../../src/family.rs#L549)).  Production seeds keep
auxiliaries nonpositive, so this is safe.  Four-/five-loop sector pruning still
needs the parametric `G=U+F` rank/ideal test and monotone propagation used by
LiteRed2's `AnalyzeSectors`
([`LiteRed2026.m:2936-3108`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2936)).

## 4. Concrete three-loop finite-box solver

### 4.1 Separate targets, seeds, and row halo

Replace the single `ThreeLoopReductionConfig.max_dots` role with:

```rust
TargetBox { max_dots: D, max_numerators: N }
SeedShell { sector, max_dots: Ds, max_numerators: Ns }
HaloPolicy { initial_depth, max_depth, candidate_budget, row_budget }
```

The public certificate covers only `TargetBox`.  `SeedShell` is solver state
and may extend outside it.  The row halo is computed exactly from the generated
shifts and is not advertised as public coverage.

Start with the target shell, eliminate, reduce only target normal forms, and
inspect their unresolved same-sector integrals.  For each unresolved integral
`x`, enqueue valid inverse-shift parents

```text
x
x - e_r
x - e_r + e_s
```

that can emit `x` through the diagonal, constant, or denominator-cancellation
term, canonicalizing the candidates after the domain checks.  Add the next
deterministic total-degree shell if no inverse parent is new.  Rebuild or
increment the sector matrix after each batch.  Stop
successfully only when all targets close; stop with a typed resource error when
the configured halo budget is exhausted.  No fixed halo depth should be
described as universally sufficient.

### 4.2 Sector-local workflow

Build a canonical sector DAG once.  For the tetrahedron it has the six nonzero
orbits already enumerated in the three-loop plan.  Then, for each genuine
sector in bottom-up order:

1. enumerate seeds in exactly that sector, using only its self-symmetry
   stabilizer;
2. generate nine rows per seed as a stream;
3. canonicalize each term and immediately apply zero rules;
4. reduce every proper-subsector term with a proved boundary algorithm or an
   already certified sector table;
5. keep only same-sector unknown columns plus already named lower masters;
6. intern all integrals and sort unknown columns from hardest to easiest;
7. sort rows by leading column, then nonzero count, then stable row origin;
8. eliminate and extract only pivots reachable from the target set;
9. if a target reaches an unapproved terminal, extend only this sector's seed
   frontier;
10. persist the sector result and certificate before moving upward.

This mirrors the mathematical dependency order in LiteRed2's `IBPSelect` and
`IBPReduce`, which discover reachable rules and then compose lower sectors
before within-sector layers
([`LiteRed2026.m:3801-4013`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3801)).

### 4.3 Boundary prerequisite

Implement the finite algorithms already derived in the three-loop plan:

- tree sectors: expand every inactive denominator polynomial, perform
  componentwise angular averages, and reduce each radial moment to a one-loop
  tadpole;
- paw sectors: angularly reduce the bridge-loop tensors, turn the remainder
  into scalar two-loop sunset integrals, and call the existing complete
  two-loop pipeline;
- dispatch by a proved sector-orbit transformation, not by the mask of a
  lexicographically canonical numerator integral;
- place explicit expansion, tensor-rank, and generated-term limits before the
  work begins.

This closes the numerator halos generated by scalar genuine-sector IBPs and is
also required for public `N>=1` coverage.

### 4.4 First acceptance target

The next honest certificate should be the complete three-loop
`TargetBox(D=1,N=1)`.  Existing enumeration finds 36 symmetry-unique nonzero
seeds in that box and therefore 324 basic rows before sector splitting.  The
acceptance gate is:

- every labelled integral in the box canonicalizes, is proved zero, or reduces
  to the proposed fixed candidate set; a counterexample must fail construction
  rather than be promoted silently;
- every selected rule has exact order descent and exact row provenance;
- all generated rows reduce as whole equations to zero;
- at least one held-out shell, preferably `(2,1)` and then `(1,2)`, is tested
  without using its rows; failures enlarge only the affected sector and do not
  invalidate the certified smaller box;
- reconstructed rules pass exact symbolic replay and fresh modular samples;
- master status is reported as “candidate for this bounded certificate” until
  an independent rank/critical-point argument establishes minimality.

## 5. Symbolica backend plan

### 5.1 Exact sparse backend for three loops

Symbolica exposes
`SparseRowReducer<RationalPolynomialField<IntegerRing,u16>>`.  It accepts rows
incrementally, always pivots the lowest present column, exposes the pivot map
and triangular `U`, and can back-substitute
([`sparse.rs:1497-1829`](../../vendor/symbolica/lib/numerica/src/tensors/sparse.rs#L1497)).
Therefore RustRed must assign column 0 to the hardest integral and provide
sorted column indices.

For the first sector-local three-loop implementation:

- intern `Integral -> u32` once per sector;
- assemble rows as sorted `Vec<(u32,Coefficient)>`;
- map hardest-to-easiest integrals onto ascending columns;
- use `LuLMode::Full` during certified forward elimination or record an
  equivalent RustRed derivation DAG;
- keep triangular `U`; recursive rule reduction avoids mandatory RREF;
- compare its output with the existing `SparseReducer` on small shells.

Symbolica's sparse reducer uses a dense scratch row internally and has no sparse
fraction-free mode.  This is acceptable for the three-loop sector sizes, but
not a reason to perform high-loop exact rational-function elimination eagerly.

### 5.2 Single-scale finite-field reconstruction

All current massive-vacuum families have one scale.  If
`w(a)=sum_i a_i`, including auxiliary scalar-product powers, dimensional
homogeneity gives

\[
 I(a)\sim (m^2)^{Ld/2-w(a)},\qquad
 I(a)=c_{ab}I(b)\Longrightarrow
 c_{ab}\sim(m^2)^{w(b)-w(a)}.
\tag{2}
\]

Thus the large solve can set `m2=1`, reconstruct a univariate rational
function of `d`, and restore the exact monomial in (2).  This is much simpler
than generic multivariate reconstruction and should be enforced by a
mass-homogeneity check on every row and rule.

A deterministic modular workflow is:

1. choose a fixed sequence of large primes which do not divide routing
   denominators;
2. convert/evaluate Symbolica rational polynomials with
   `to_finite_field` and `evaluate`;
3. eliminate with `SparseRowReducer<Zp64>` at several `d` samples;
4. reject samples with zero input/pivot denominators;
5. require the pivot bitmap and reachable dependency structure to agree across
   independent primes and samples;
6. reconstruct each requested coefficient `p(d)/q(d)` with adaptive numerator
   and denominator degrees, normalizing `q`;
7. combine coefficient residues with `Integer::chinese_remainder` and recover
   rational contents with `Rational::maximal_quotient_reconstruction`;
8. validate at held-out primes and `d` samples;
9. finally replay the reconstructed rules over exact
   `RationalPolynomial<IntegerRing,u16>` rows.

Symbolica supplies the finite fields, sparse reducer, CRT, scalar rational
reconstruction, and polynomial primitives.  It does not supply turnkey
multivariate rational-function reconstruction.  RustRed must own degree
discovery, bad-sample handling, normalization, and the final proof replay.

Finite-field results may select a generic pivot skeleton, but they must never
be the sole correctness certificate: unlucky specializations can decrease
apparent rank or hide parameter poles.

### 5.3 Coefficient and fill control

The high-loop engine should have three coefficient modes:

```text
structural/modular: Zp64 at sampled d
reconstruction:     univariate p(d)/q(d), m2 power stored separately
certificate/final:  exact Symbolica RationalPolynomial<IntegerRing,u16>
```

Do not normalize every exact coefficient after every exploratory row operation.
Discover the pivot skeleton and reachable rule set modulo primes, then perform
exact arithmetic only for the selected result or reconstruct it.  Track at
least row count, nonzeros, peak row width, pivot count, coefficient degrees,
bad samples, reconstruction samples, and memory estimates per sector.

## 6. Four- and five-loop prerequisites

### 6.1 Interned data and packed sectors

Use a family-owned exponent arena and compact IDs:

```text
IntegralId(u32) -> Box<[i32]>
SectorId(u16/u32) -> packed physical mask
ColumnId(u32) -> IntegralId
Row -> sorted (ColumnId, coefficient) pairs
```

Keep auxiliaries out of physical sector masks.  A 15-index family fits a
`u16` physical mask when all positions are physical, while a generic bit-vector
fallback avoids baking in that limit.  Hash/intern once; do not clone exponent
vectors through every row operation.

### 6.2 Automatic factorization and lower-loop closure

High-loop tables must not rediscover products of solved lower-loop bubbles.
For each active sector:

- determine the rank and connected/block decomposition of its momentum forms;
- construct and verify an exact unimodular loop-momentum transformation;
- map scalar factors into registered lower-loop families;
- when numerator scalar products couple blocks, perform the required tensor
  decomposition in Rust before multiplying lower-loop reductions;
- cache the factorization proof and component maps.

This component service is a dependency of the sector solver, not an optional
post-processing pass.  It prunes the sector DAG and closes the large boundary
halos before matrix insertion.

### 6.3 ISP-aware symmetries

The current four-loop and five-loop family constructors intentionally register
only the identity because generated auxiliaries are generally transformed into
linear combinations rather than permutations
([`crates/rustred-legacy-oracles/src/four_loop.rs:1-14`](../../crates/rustred-legacy-oracles/src/four_loop.rs#L1),
[`crates/rustred-legacy-oracles/src/five_loop.rs:1-7`](../../crates/rustred-legacy-oracles/src/five_loop.rs#L1)).  Leaving this unchanged
would discard one of the largest high-loop reductions in seed count.

Introduce a proved family map containing:

```text
loop transformation and determinant
physical-propagator permutation
affine linear image of every denominator-basis entry
inverse map
```

For scalar sectors, use the physical permutation immediately.  For numerator
monomials, expand the affine ISP images into a linear combination with a strict
term budget and cache the result.  A symmetry-adapted auxiliary basis may turn
some maps back into permutations, but the API cannot require that special
case.

### 6.4 Resumable sector caches

One cache unit should be one `(family, order, canonical sector, shell)` job.
Its header should include:

- family, sign convention, symmetry, zero/factorization, and order
  fingerprints;
- physical sector and self-symmetry fingerprints;
- target set, seed shells, row-origin hashes, and resource limits;
- coefficient mode, primes/samples, pivot skeleton, reconstruction state;
- triangular rules, dependency IDs, pole factors, provenance roots;
- completion state: generated, modular rank stable, reconstructed, exact replay
  passed, held-out targets passed.

Write a temporary complete record and atomically rename it.  Merge independent
sector jobs only in canonical sector order.  Current `ReductionTable` caches
remain useful as small final rule snapshots, but they are not resumable solver
state.

## 7. S-bases, syzygies, and all-index recurrences

Ordinary momentum-space IBPs are sufficient for a finite Laporta certificate
if adaptive seeding closes the declared target set.  Implementing an s-basis
before the `(1,1)` three-loop box would add a much larger correctness surface
without fixing the known factorized numerator terminal.

LiteRed2's main `SolvejSector` is also not a conventional Buchberger s-basis.
It searches a growing diamond of nearby points, eliminates in complexity order,
patternizes successful pivots, computes bad conditions, and partitions the
integer sector domain
([`LiteRed2026.m:2254-2714`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2254)).
RustRed should follow the same broad progression:

1. trusted finite sector tables;
2. interpolate a recurring pivot shape from several shells;
3. reconstruct symbolic index-dependent coefficients;
4. verify the recurrence exactly against generic IBPs;
5. derive guards for active `a_i>=1`, inactive `a_i<=0`, nonzero pivot factors,
   and strict RHS descent;
6. prove that guarded rules plus named master points cover the integer domain.

Parametric syzygy IBPs are a valuable later optimization for positive-index
sectors.  LiteRed2's optional `GenerateFPIBP` computes syzygies of
`(partial_i G, G)` and explicitly rejects numerators
([`LiteRed2026.m:1834-1924`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1834)).
They can reduce doubled propagators and matrix size, but cannot replace the
momentum-space identities in numerator sectors.

Symbolica exposes an efficient F4 polynomial `GroebnerBasis`, but the vendored
Rust API does not expose a module-syzygy/Schreyer-basis facility.  RustRed would
need to implement the module layer (using Symbolica polynomials and sparse
linear algebra) or construct a verified equivalent encoding.  Do this only
after profiling identifies scalar high-sector rows as the dominant cost.

A noncommutative shift-operator s-basis can be considered after guarded
recurrence discovery.  It is not required for finite 3-loop completion, and a
partially implemented unguarded s-basis is less trustworthy than explicit
finite certificates.

## 8. Revised incremental implementation order

The earlier three-loop-first sequence identified the right enabling services,
but it treated four and five loops as successive endpoints.  The six-loop
campaign instead needs those services to form one reusable offline/online
architecture:

1. **Symbolica-native proof kernels and API hardening**
   - finish verified family/symmetry maps and the sparse-row API
     spike without retaining a private algebra implementation;
   - keep typed arity/canonicality errors, separate target and seed
     specifications, versioned order, and honest uncertified-table status.

2. **Reusable guarded parametric-rule publication**
   - finish the generic persistent cylindrical/residual solver;
   - compile `WhenBad` branches, feed solved subsectors upward, prove descent
     and domain coverage, and publish immutable coverage-closed job shards;
   - keep incomplete/resource-limited workspaces separate so they cannot be
     loaded as rules or silently promoted to masters;
   - use one-loop through three-loop cases as correctness gates, not as the
     architectural endpoint.

3. **Unit-mass modular acceleration**
   - record `m2=1` in the campaign/job specialization key so concrete
     coefficients live in `Q(d)`;
   - use Symbolica finite fields for pivot/rank discovery and univariate
     reconstruction in `d`;
   - reconstruct only reachable rules, then require fresh-prime checks and
     exact symbolic replay.

4. **Topology/sector foundry through six loops**
   - ingest a declared graph corpus, complete generic families/ISPs, and build
     the canonical sector DAG;
   - factorize certified lower-loop components before matrix insertion;
   - generate symmetry candidates from graph automorphisms/routing maps rather
     than bounded `GL(L,Z)` enumeration, then certify full ISP-aware affine
     maps;
   - maintain resumable `(family, sector, shell)` derivation workspaces;
   - compile closed jobs into deterministic multi-start campaign bundles with
     verified root ingress maps and shared subsector, factorization, and cross-
     family dependency nodes.

5. **Vakint through-four-loop foundry and oracle gate**
   - derive every replacement system needed by the Vakint H/X/BMW/FG corpus
     without FORM or copied recurrence tables;
   - use a minimal generic application seam to compare every contraction and
     numerator target with Vakint while leaving terminals unsubstituted;
   - require exact regenerated-IBP residuals and cancellation closure
     independently of the oracle.

6. **Five- and six-loop derivation-only scalability gates**
   - close multiple general five-loop families, including ISP-rich and
     duplicate-denominator cases rather than only the banana;
   - close a pre-run-frozen structurally representative QCD-valid quartic/cubic
     six-loop corpus and all reachable dependencies, then a small
     GammaLoop/BPHZ-derived multi-root corpus;
   - require no reachable unsupported/resource/timeout/uncovered leaf, exact
     rule residuals, strict dependency descent, deterministic worker-count
     semantics, and the pre-run numerical time/memory/artifact/parallel-
     scaling thresholds defined by the six-loop campaign manifest.

7. **Separate high-throughput application runtime and later online milestone**
   - consume normalized GammaLoop/BPHZ vacuum terms only after the foundry
     gates above pass;
   - tensor/scalar lower once, intern/canonicalize integral keys, and apply
     compiled parametric rules in batches;
   - share normal-form and coefficient-specialization caches across numerator
     terms and parallel workers without mixing discovery into the hot path;
   - reduce the declared QCD numerator corpus to unsubstituted terminals with
     no uncovered keys, reporting online throughput separately from derivation.

At every stage, “complete” means complete for an explicit topology and target
manifest, coefficient domain, and integer coverage domain, with exact
provenance and held-out validation.  A stable list of free columns at shallow
seed depths remains useful evidence, but is neither a coverage proof nor a
master-count proof.
