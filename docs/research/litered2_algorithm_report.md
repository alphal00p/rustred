# LiteRed2 algorithm audit for RustRed

> **Audit note.**  This report contains useful source-level findings and
> concrete vacuum oracles, but its old milestone recommendations are not the
> governing scope.  [`litered_full_scope_spec.md`](litered_full_scope_spec.md)
> supersedes every vacuum-only, fixed-loop, or optional-power-shift statement.

This report records the LiteRed2 algorithms and conventions that matter for a
pure-Rust port.  It uses `vendor/LiteRed2/Source/LiteRed2026.m` as the primary
source: it is the newest snapshot in the vendor tree.  The 2025-to-2026 changes
do not alter the main IBP algorithm; the relevant changes are solver variable
ordering, rule-selection simplification, and a few ancillary functions.

The immediate recommendation is to implement the scalar two-loop massive
vacuum family first, with the exact oracle in section 8.  That family is small
enough to audit exhaustively but exercises every essential layer: a complete
scalar-product basis, index shifts, zero sectors, exact symmetries, sector
ordering, bottom-up solving, factorized boundaries, masters, numerators, and
tensor projection.  It also matches Vakint's canonical `I2L` momentum routing.

## 1. Findings that should drive the port

1. LiteRed2 is organized around an **independent denominator basis**, not
   around a graph.  For $L$ vacuum loops it ultimately needs
   $N=L(L+1)/2$ independent scalar products.  Missing directions are appended
   as irreducible scalar products (ISPs); too many linearly dependent
   denominators require the separate `NewDsSet`/partial-fraction path.

2. A scalar integral is an integer exponent vector `j[basis,n1,...,nN]`.
   Positive indices are denominators; zero or negative indices are absent
   denominators/numerators.  Its sector `js[basis,b1,...,bN]` is the sign vector
   $b_i=[n_i>0]$.

3. Ordinary momentum-space IBPs are generated once as parametric index-shift
   functions.  A vacuum family gets $L^2$ identities, from
   $\partial_{k_i}\mathbin\cdot k_j$.  Lorentz-invariance identities are empty
   when there are no external momenta.

4. `AnalyzeSectors` normally detects scaleless sectors with a rank test on
   monomials of $G=U+F$, then propagates zero status to subsectors and nonzero
   status to supersectors.  It can instead solve corner-point IBPs, but the
   Feynman-parameter route is the default.

5. Symmetry detection is two-stage.  A canonical form of $U+F$ cheaply groups
   candidate sectors; exact linear loop-momentum transformations then prove the
   mapping.  The polynomial signature is a filter, never the proof.

6. The main solver is not a conventional Buchberger s-basis implementation.
   `SolvejSector` searches a growing lattice of index points, performs
   complexity-ordered Gaussian elimination (`Solvej`), generalizes successful
   pivots to conditional symbolic recurrences, and treats uncovered points as
   masters.  It is best described as a symbolic recurrence search backed by
   Laporta-like elimination.  The `A`/`B` operator representation and the
   parametric syzygy generator are useful secondary facilities, not the engine
   used by the standard workflow.

7. `IBPReduce` does demand-driven rule selection.  It first closes the set of
   integrals needed by the input, then composes sector rules in dependency order
   and within-sector rules in triangular layers.  RustRed should preserve this
   idea but use an in-memory dependency DAG and an optional serialized cache.

8. All coefficient algebra assumes generic, algebraically independent
   parameters and symbolic dimension.  Rules may be invalid on exceptional
   loci where a pivot coefficient vanishes.  LiteRed2 carries conditions and
   rejects unsafe symbolic rules; RustRed must not silently cancel those cases.

9. LiteRed2's `j` layer is scalar.  Its bundled `Vectors` package has tensor
   structure and angular-average helpers, but tensor projection is not part of
   the IBP pipeline.  RustRed therefore needs an explicit tensor-to-scalar stage
   before denominator conversion and IBP reduction.

10. The Fermat, Sparx/FLINT, FORM, and Mathematica-kernel execution paths are
    accelerators or implementation details, not mathematical requirements.
    They must not be dependencies of RustRed.  Exact arithmetic, pattern
    matching, polynomial operations, and sparse elimination should be supplied
    in Rust, using Symbolica where appropriate.

## 2. Source map and entry points

The package is a monolithic generated Mathematica source file.  These are the
entry points worth following in order.

| Stage | Functions and exact source location | Role in RustRed |
|---|---|---|
| Public surface | [`LiteRed2026.m:39-303`](../../vendor/LiteRed2/Source/LiteRed2026.m#L39) | Package exports and global defaults. |
| Family construction | [`NewDsBasis`, options and implementation at lines 688-874](../../vendor/LiteRed2/Source/LiteRed2026.m#L688) | Validate/complete the scalar-product basis; derive inverse denominator relations and parameters. |
| Overcomplete sets | [`NewDsSet`, `Relations`, `NewDsBases`, `GeneratePFGB`, `PFReduce`, lines 465-686](../../vendor/LiteRed2/Source/LiteRed2026.m#L465) | Partial-fraction linearly dependent propagator sets into independent bases.  This is not required for the first vacuum milestones but is required for feature completeness. |
| Integral conversion | [`Toj`/`Fromj`, lines 1266-1367](../../vendor/LiteRed2/Source/LiteRed2026.m#L1266) | Convert scalar-product rational functions to exponent vectors and back. |
| Ordering | [`MakeOrderMatrix`, `jsOrder`, `jComplexity`, lines 1370-1689](../../vendor/LiteRed2/Source/LiteRed2026.m#L1370) | Sector-aware total order used for pivots and rule orientation. |
| Sectors | [`jSector`, `jSubsectors`, `SectorHierarchy`, `SectorLayer`, lines 1698-1796](../../vendor/LiteRed2/Source/LiteRed2026.m#L1698) | Sign sectors, subset hierarchy, and finite index layers. |
| IBPs | [`GenerateIBP`, lines 1799-1831](../../vendor/LiteRed2/Source/LiteRed2026.m#L1799) | Generate momentum-space index-shift identities. |
| Optional parametric IBPs | [`GenerateFPIBP`, `FPIBP`, lines 1834-1924](../../vendor/LiteRed2/Source/LiteRed2026.m#L1834) | Generate syzygy-based identities per sector; positive indices only. |
| Shift operators | [`ToAB` through `ABIBPLI`, lines 1932-2104](../../vendor/LiteRed2/Source/LiteRed2026.m#L1932) | Raising/lowering operator form.  Useful for diagnostics or a future true s-basis engine. |
| One-equation elimination | [`Solvej`, lines 2121-2200](../../vendor/LiteRed2/Source/LiteRed2026.m#L2121) | Select the highest integral, substitute prior pivots, and append a triangular rule. |
| Sector solver | [`SolvejSector`, lines 2254-2714](../../vendor/LiteRed2/Source/LiteRed2026.m#L2254) | Search symbolic recurrences by increasing index-space depth; manage validity cases and masters. |
| Zero sectors | [`BiggestSectors` and `AnalyzeSectors`, lines 2936-3108](../../vendor/LiteRed2/Source/LiteRed2026.m#L2936) | Enumerate relevant sectors, detect scaleless corners, and build the zero rule. |
| Symmetries | [`FindShifts` and `FindSymmetries`, lines 3111-3473](../../vendor/LiteRed2/Source/LiteRed2026.m#L3111) | Prove loop-momentum mappings; create mapped/unique sectors and self-symmetry relations. |
| Masters | [`IdentifyMIs`, `AddjRule`, `RefreshMIs`, lines 3625-3729](../../vendor/LiteRed2/Source/LiteRed2026.m#L3625) | Record uncovered integrals, identify equivalent masters, and allow custom rules. |
| Demand reduction | [`IBPSelect` and `IBPReduce`, lines 3801-4013](../../vendor/LiteRed2/Source/LiteRed2026.m#L3801) | Select only reachable rules, then compose them bottom-up. |
| Master-basis change | [`ToMIsRule`, lines 4109-4134](../../vendor/LiteRed2/Source/LiteRed2026.m#L4109) | Linear-algebra change to a user master basis. |
| Parametric polynomials | [`FeynParUF`, lines 4205-4280](../../vendor/LiteRed2/Source/LiteRed2026.m#L4205) | Construct $U,F$, including sector restrictions. |
| Tensor helpers | [`Vectors.m:619-714`](../../vendor/LiteRed2/Source/RNL/Vectors.m#L619) | `DAverage`, `VAverage`, `TensorSet`, and `TSCollect`; inspiration for a separate Rust tensor stage. |

The intended high-level call chain is

```text
NewDsBasis
  -> GenerateIBP
  -> AnalyzeSectors
  -> FindSymmetries
  -> SolvejSector on each unique sector
  -> IBPReduce on requested expressions
```

`NewDsBasis[..., SolvejSector -> True]` triggers this chain automatically.  The
first shipped notebook demonstrates that style at
[`Examples/example1.nb:112-149`](../../vendor/LiteRed2/Examples/example1.nb#L112)
and later reduces `j[triangle,1,2,4]` at
[`example1.nb:212-235`](../../vendor/LiteRed2/Examples/example1.nb#L212).
`example2.nb` demonstrates graph-to-denominator construction and mappings
between bases.  `NewDsSet.nb` demonstrates the linearly dependent denominator
set workflow.  There is no automated test directory and no shipped massive
vacuum golden reduction; the notebooks are examples, not regression tests.

## 3. Family representation and conventions

### 3.1 Complete scalar-product basis

For loop momenta $k_1,\ldots,k_L$ and external momenta
$p_1,\ldots,p_E$, LiteRed2 forms the canonical scalar-product list

$$
  \{k_i\mathbin\cdot k_j\}_{i\leq j}
  \cup
  \{k_i\mathbin\cdot p_a\}.
$$

Its size is $L(L+1)/2+LE$, after symmetry and declared external constraints.
Each denominator is expanded as an affine linear form in that list,

$$
  D_r = \sum_\alpha A_{r\alpha}s_\alpha+c_r.
$$

`NewDsBasis` computes the coefficient matrix $A$, rejects an overdetermined
dependent list, appends independent scalar products when the rank is too low
and `Append -> True`, and solves for every $s_\alpha$ in terms of $D_r$ and
parameters.  The relevant checks and inverse construction are at
[`LiteRed2026.m:763-811`](../../vendor/LiteRed2/Source/LiteRed2026.m#L763).

RustRed should store this directly:

```text
IntegralFamily
  name
  loops: [MomentumId; L]
  externals: [MomentumId; E]
  denominators: [Atom; N]
  scalar_products: [ScalarProduct; N]
  denominator_matrix: A
  denominator_constants: c
  sp_from_denominators: A^{-1}(D-c)
  parameters: ordered set of symbols
  sector_pattern, cut_flags, power_shifts
  order_specification
```

The Mathematica implementation encodes a denominator monomial temporarily as
products of `j` objects.  RustRed should not copy that trick.  Store a sparse
affine scalar-product map and a separate exponent-shift algebra.

### 3.2 Integral and sector semantics

LiteRed2 defines

$$
  J(n_1,\ldots,n_N)
  =\int\prod_{i=1}^{L}d^d k_i\;\prod_{r=1}^{N}D_r^{-n_r},
$$

up to whatever common normalization the caller uses.  `j[basis,...]` is the
formal integral.  For integer indices,

```text
n_r > 0   denominator D_r is present with power n_r
n_r = 0   D_r is absent
n_r < 0   D_r^(-n_r) is a numerator factor
```

The sector bit is exactly $b_r=1$ for $n_r>0$, otherwise zero
([`jSector`, lines 1712-1716](../../vendor/LiteRed2/Source/LiteRed2026.m#L1712)).
An active-sector pattern therefore matches positive indices; an inactive bit
matches nonpositive indices.  This boundary at zero must be identical in
RustRed.  Treating zero as a denominator power is an off-by-one error that will
corrupt zero-sector and recurrence conditions.

Sector comparisons are a subset partial order.  Integral comparisons use a
separate total complexity order.  Do not conflate them.

### 3.3 Sector patterns, ISPs, cuts, and shifted powers

When ISPs are appended, LiteRed2's default `SectorsPattern` fixes their sector
bits to zero.  Their indices can still be negative, but they are never treated
as propagators.  A cut flag forces every retained sector to contain the
corresponding denominator; a sector missing any cut is immediately zero.

`PowerShifts` allows formal constant shifts in powers.  It is experimental in
LiteRed2, but it is part of the full LiteRed scope required for RustRed.
Nonzero shifts are treated as present during zero-sector analysis, symmetry
maps must preserve them, and generated shift-operator coefficients use the
shifted power `n_i + nu_i` while sector membership continues to use the raw
integer index `n_i`.  Production RustRed must therefore support and authenticate
symbolic power shifts rather than reject them or silently implement an
integer-power-only subset.

This paragraph corrects the earlier milestone recommendation to reject
nonzero shifts.  That recommendation was superseded by the governing
full-scope specification in
[`litered_full_scope_spec.md`](litered_full_scope_spec.md).

### 3.4 Sign conventions are data, not normalization

LiteRed2 accepts either $D=k^2-s$ or $D=s-k^2$; it does not globally choose
one.  The shipped triangle example uses the latter.  Vakint's canonical
two-loop topology records momenta and a mass-squared argument in
[`topologies.rs:54-70`](../../vendor/gammaloop/crates/vakint/src/topologies.rs#L54),
and the algebraic oracle below deliberately chooses

$$
  D=k^2-s,\qquad s=M^2.
$$

If all three denominators are instead $\widetilde D=s-k^2=-D$, then for
integer powers

$$
  \widetilde J(a,b,c)=(-1)^{a+b+c}J(a,b,c).
$$

This conversion must be applied to golden answers.  RustRed should retain the
user's exact denominator polynomial and never hide this sign in parsing.

## 4. Ordering conventions

### 4.1 Sector complexity

For a numeric integral, `jComplexity` begins with

```text
[basis id, number of active denominators, binary sector id, ...within-sector key]
```

where the leftmost sector bit is the most significant binary digit.  More
active denominators are more complex.  At equal denominator count the binary
sector id breaks ties.  The remaining key is the sector's order matrix times
the index vector.

The default family option is

```text
jsOrder -> {"np", "cp", "-ds", "-ns"}
```

and `MakeOrderMatrix` always starts with the row $2b-1$.  It appends requested
rows only if they increase rank and stops with an independent matrix.  The
full specification language is implemented at
[`LiteRed2026.m:1378-1441`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1378).

For the two-loop oracle below, with no cuts, the exact default matrices are

$$
  M_{111}=\begin{pmatrix}1&1&1\\-1&0&0\\0&-1&0\end{pmatrix},
  \qquad
  M_{011}=\begin{pmatrix}-1&1&1\\-1&0&0\\0&-1&0\end{pmatrix}.
$$

Thus the top-sector within-sector key is
$(a+b+c,-a,-b)$.  Reproducing these matrices gives deterministic agreement
with LiteRed2.  A different well-founded total order can still be mathematically
correct, but it changes pivot rules and masters chosen as representatives; if
RustRed intentionally differs, tests should compare reduced expressions after
canonical master mapping rather than serialized rule text.

### 4.2 Pivot selection

`Solvej` collects a linear relation by integral, identifies `Highj`, checks
whether its coefficient simplifies to zero, substitutes existing rules until
the candidate pivot is irreducible by the database, and creates

$$
  J_{\rm high}\longrightarrow-
  \frac{\sum_{J\ne J_{\rm high}}c_JJ+c_0}{c_{\rm high}}.
$$

New rules are prepended to the database.  With exhaustive substitution enabled,
older rules are also updated.  RustRed should represent a relation as a sparse
ordered map `Integral -> Coefficient`, keep the pivot coefficient explicit,
and normalize coefficients only at controlled points.

## 5. IBP generation

For each loop derivative $\partial/\partial k_i$ and each vector
$q\in\{k_1,\ldots,k_L,p_1,\ldots,p_E\}$, LiteRed2 constructs

$$
  0=\int\prod_jd^d k_j\;
  \frac{\partial}{\partial k_i^\mu}
  \left(q^\mu\prod_rD_r^{-n_r}\right).
$$

Expanding gives

$$
  0=\delta_{q,k_i}\,d\,J(\mathbf n)
  -\sum_r n_r\int\prod_jd^d k_j\;
  \frac{q\mathbin\cdot\partial_{k_i}D_r}{D_r}
  \prod_sD_s^{-n_s}.
$$

Every scalar product in the numerator is replaced by the inverse denominator
map, and every multiplication or division by $D_r$ is an integer shift of
the exponent vector.  The result is stored as a function of symbolic indices.
The implementation is the compact `Outer` expression at
[`LiteRed2026.m:1813-1823`](../../vendor/LiteRed2/Source/LiteRed2026.m#L1813).

Important implementation details:

- the coefficient multiplying a raised propagator is the current symbolic
  power $n_r$, not the shifted power;
- all scalar products must be distributed and canonicalized before coefficient
  extraction;
- coefficients are rational functions in $d$, masses, invariants, and powers;
- a vacuum family generates $L^2$ IBPs and no LI identities;
- generated identities can cross into subsectors and may introduce numerators;
- zero-sector and symmetry rules should be applied before expensive elimination.

The optional `GenerateFPIBP` path instead finds syzygies of
$(\partial_iG,G)$ and converts them to index shifts.  It rejects numerators.
This can be a later optimization, but ordinary momentum-space IBPs are enough
for the first milestones.

## 6. Zero sectors and symmetries

### 6.1 Default zero-sector rank test

For a family, `FeynParUF` forms $U,F$ by completing the loop-momentum
quadratic form.  For a candidate corner $b\in\{0,1\}^N$, `AnalyzeSectors`
expands

$$
  G(x)=U(x)+F(x)=\sum_r g_r(x)
$$

into monomials and constructs one row per monomial,

$$
  \left(x_1\partial_1g_r,\ldots,x_N\partial_Ng_r,g_r\right)
  \Big|_{x=b}.
$$

The sector is marked zero when the row rank is at most the number of active
parameters.  The code is at
[`LiteRed2026.m:3021-3040`](../../vendor/LiteRed2/Source/LiteRed2026.m#L3021).
The alternate path evaluates IBPs at the sector corner and attempts to reduce
that corner to zero.

Enumeration is pruned using sector monotonicity:

- once a sector is zero, every subsector is zero;
- once a sector is nonzero, every supersector is nonzero;
- a sector missing a required cut is zero before the rank test.

After classification LiteRed2 records:

```text
ZeroSectors       all analyzed zero sectors
NonZeroSectors    all analyzed nonzero sectors
SimpleSectors     nonzero sectors with no nonzero proper subsector
BasisSectors      nonzero sectors with at least one zero immediate subsector
ZerojRule         a rule generated from maximal zero sectors
```

RustRed should also keep only maximal zero masks internally.  Testing whether
a sector is below one of those masks is faster and gives the same zero ideal.

### 6.2 Exact sector symmetries

`FindSymmetries` first canonicalizes restricted $U+F$ polynomials to group
possible matches.  It then introduces a general linear ansatz for transformed
loop momenta (and optionally external isometries), rewrites the denominators,
and solves for an affine matrix taking active denominators to active
denominators.  Cut flags and power shifts are checked.  Exact solutions are
extended from simple sectors to their supersectors.

The results are:

```text
UniqueSectors       one representative per proved orbit
MappedSectors       other members of those orbits
SectorsMappings     mapped-sector -> representative-sector masks
jRules              index rules for mapped sectors
jSymmetries         self maps of a representative sector
SR                  linear symmetry relations J - mapped(J)
```

For RustRed, represent a proved symmetry by both a momentum matrix and its
induced denominator permutation/affine map.  Validate the transformed
denominators exactly.  Also explicitly check the loop-measure Jacobian: require
absolute determinant one for a rule with no prefactor, or include the correct
Jacobian.  LiteRed2's graph-like use cases produce unit-Jacobian shifts, but its
rule construction does not expose this check clearly enough to copy blindly.

## 7. Sector solving, masters, and reduction application

### 7.1 What `SolvejSector` actually does

For a unique sector with symbolic indices $n_i$, the solver:

1. declares the sector domain $n_i\ge1$ for active bits and $n_i\le0$ for
   inactive bits;
2. partitions that domain into cases according to requested/no-rule conditions;
3. constructs a diamond of nearby index points with `preparepoints`;
4. evaluates the chosen recurrence functions (`IBP`, `LI`, `IBPLI`, or
   `FPIBP`) at those points;
5. performs complexity-ordered elimination with `Solvej`;
6. shifts a successful numeric/symbolic pivot back to a generic recurrence;
7. patternizes its left side and derives conditions under which no coefficient
   denominator vanishes and no right-side integral leaves the allowed order;
8. adds valid rules, refines the remaining cases, and increases search depth
   when necessary;
9. records currently uncovered points in `MIs` as operational master
   candidates.  This is LiteRed's bounded-search heuristic, not correctness
   evidence that RustRed may use to certify a master.

`WhenBad` and `SmartReduce` use integer-domain logical reduction to protect
against invalid recurrence ranges and exceptional coefficients.  This is the
hardest Mathematica-specific part of the port.  For the initial Rust solver a
safer progression is:

- implement exact finite Laporta elimination for a requested index box;
- add proven symbolic recurrences for simple sectors;
- represent recurrence guards explicitly as conjunctions of integer bounds and
  nonzero polynomial conditions;
- only then implement heuristic recurrence discovery.

That progression still produces a complete on-demand reducer and avoids
pretending that an unguarded symbolic rule is universally valid.

### 7.2 Master semantics

`MIs[basis]` is the list of integrals not covered by found rules, not an a
priori mathematical declaration.  `NMIs` can supply an expected count and stop
the search early; with the optional Mint package, `NMIs -> Automatic` counts
critical points.  `IdentifyMIs` later compares parametric polynomial normal
forms and can remove equivalent masters.  `ToMIsRule` changes to a user basis
by matrix inversion after reducing the requested candidates.

RustRed should distinguish:

```text
candidate masters    uncovered at the current search bound
proved masters       stable under a larger-rank/elimination check
canonical masters    one representative after exact symmetry/equivalence maps
user masters         a verified invertible linear basis change
```

Do not report a candidate as proved merely because a shallow search did not
find a rule.

### 7.3 Demand-driven application

`IBPSelect` starts from the integrals in the input, chooses the most complex
sector, repeatedly applies its rule table, and adds newly reached integrals and
subsectors until closure.  Zero sectors are replaced by zero immediately.
`IBPReduce` then topologically orders sector dependencies, substitutes lower
sectors into higher ones, layers within-sector rules, and finally applies the
minimal selected rule set to the input.

A Rust implementation should use:

```text
RuleCache: (family, sector, integral/guard) -> sparse linear combination
DependencyGraph: integral -> RHS integrals
Reducer:
  canonicalize symmetry
  test zero
  memoized recursive reduction with cycle detection
  collect/simplify coefficients
```

Persist a versioned family fingerprint, ordering fingerprint, and coefficient
normalization version with every cache.  Rules from a different denominator
sign, order, or symbol assumption are not interchangeable.

## 8. Exact two-loop massive-vacuum oracle

This section is intended to become RustRed's first end-to-end golden test.  It
is derived directly from the defining IBPs and factorization, so it does not
depend on Mathematica output or FORM.

### 8.1 Family

Use Vakint's two-loop routing:

$$
  D_1=k_1^2-s,\qquad
  D_2=k_2^2-s,\qquad
  D_3=(k_1+k_2)^2-s,
  \qquad s=M^2\ne0.
$$

Define

$$
  J(a,b,c)=\int d^d k_1d^d k_2\;D_1^{-a}D_2^{-b}D_3^{-c}.
$$

The exact inverse scalar-product map is

$$
  k_1^2=D_1+s,\qquad
  k_2^2=D_2+s,\qquad
  2k_1\mathbin\cdot k_2=D_3-D_1-D_2-s.
$$

This is a complete rank-three basis for the three two-loop vacuum scalar
products.  No ISP is needed.

A corresponding LiteRed2 setup would be

```mathematica
SetDim[d];
Declare[{k1,k2}, Vector, {s}, Number];
NewDsBasis[v2,
  {sp[k1]-s, sp[k2]-s, sp[k1+k2]-s},
  {k1,k2},
  GenerateIBP -> True,
  AnalyzeSectors -> True,
  FindSymmetries -> True
];
SolvejSector[v2, DiskSave -> False];
```

This snippet documents source semantics; RustRed tests must not require a
Mathematica installation.

### 8.2 Parametric and symmetry oracle

With LiteRed2's default `FeynParUF` sign for these denominators,

$$
  U=x_1x_2+x_1x_3+x_2x_3,
  \qquad
  F=-s(x_1+x_2+x_3)U.
$$

For denominators $s-k^2$, the sign of $F$ is positive instead.

The family has the full $S_3$ permutation symmetry of its denominators.  Two
generators are

```text
(k1,k2) -> (k2,k1)          swaps D1,D2
(k1,k2) -> (-k1-k2,k2)      swaps D1,D3
```

Both have unit absolute Jacobian.  Therefore $J(a,b,c)$ is invariant under
every permutation of $(a,b,c)$.

### 8.3 Expected sectors

Before symmetry reduction:

```text
ZeroSectors     = {000, 001, 010, 100}
NonZeroSectors  = {011, 101, 110, 111}
SimpleSectors   = {011, 101, 110}
BasisSectors    = {011, 101, 110}
```

The zero sectors have fewer than two independent massive denominators, leaving
one unconstrained scaleless loop integration.  This is a consequence here, not
a generally valid "fewer denominators than loops" rule; LiteRed2 deliberately
removed that unsafe shortcut.

With LiteRed2's deterministic traversal and default ordering, the symmetry
result is expected to be

```text
UniqueSectors   = {011, 111}
MappedSectors   = {101, 110}
```

The representative choice `011` is conventional.  RustRed may choose another
pair sector if it canonicalizes all three consistently.

### 8.4 The four symbolic IBPs

For all integer $(a,b,c)$, with the usual dimensional-regularization boundary
interpretation, `GenerateIBP` must produce relations equivalent to the
following four equations.

From $\partial_{k_1}\mathbin\cdot k_1$:

$$
\begin{aligned}
0={}&(d-2a-c)J(a,b,c)-2asJ(a+1,b,c)\\
   &-cJ(a-1,b,c+1)+cJ(a,b-1,c+1)-csJ(a,b,c+1).
\end{aligned}
\tag{I11}
$$

From $\partial_{k_1}\mathbin\cdot k_2$:

$$
\begin{aligned}
0={}&(a-c)J(a,b,c)-aJ(a+1,b,c-1)+aJ(a+1,b-1,c)\\
   &+asJ(a+1,b,c)+cJ(a-1,b,c+1)\\
   &-cJ(a,b-1,c+1)-csJ(a,b,c+1).
\end{aligned}
\tag{I12}
$$

From $\partial_{k_2}\mathbin\cdot k_1$:

$$
\begin{aligned}
0={}&(b-c)J(a,b,c)-bJ(a,b+1,c-1)+bJ(a-1,b+1,c)\\
   &+bsJ(a,b+1,c)+cJ(a,b-1,c+1)\\
   &-cJ(a-1,b,c+1)-csJ(a,b,c+1).
\end{aligned}
\tag{I21}
$$

From $\partial_{k_2}\mathbin\cdot k_2$:

$$
\begin{aligned}
0={}&(d-2b-c)J(a,b,c)-2bsJ(a,b+1,c)\\
   &+cJ(a-1,b,c+1)-cJ(a,b-1,c+1)-csJ(a,b,c+1).
\end{aligned}
\tag{I22}
$$

These identities are a stronger generator test than comparing a final answer:
they expose derivative factors, constant mass terms, scalar-product inversion,
and every shift direction independently.

### 8.5 Boundary sectors and complete numerator formula

Let

$$
  T_n=\int d^d k\;(k^2-s)^{-n}.
$$

Dimensional regularization gives $T_n=0$ for $n\le0$ and, for $n\ge1$,

$$
  T_{n+1}=\frac{d-2n}{2ns}T_n,
  \qquad
  T_n=T_1\prod_{r=1}^{n-1}\frac{d-2r}{2rs}.
$$

Every two-denominator sector factorizes after a unit-Jacobian linear momentum
change.  In particular

$$
  J(0,b,c)=T_bT_c.
$$

There is also a closed formula for arbitrary numerator powers in that sector.
For $r\ge0$, $b,c>0$, define

$$
  C_t=\frac{(1/2)_t}{(d/2)_t}.
$$

Then

$$
\begin{aligned}
J(-r,b,c)
={}&\sum_{t=0}^{\lfloor r/2\rfloor}
 \binom r{2t}4^t C_t
 \sum_{u+v+w=r-2t}\frac{(r-2t)!}{u!v!w!}s^w\\
&\times\sum_{i,j=0}^{t}
 \binom ti\binom tj s^{2t-i-j}
 T_{b-u-i}T_{c-v-j}.
\end{aligned}
\tag{B}
$$

Derivation: with $p=k_2$, $q=k_1+k_2$,

$$
  D_1=D_p+D_q+s-2p\mathbin\cdot q.
$$

Odd powers of $p\mathbin\cdot q$ vanish, while

$$
  \left\langle(p\mathbin\cdot q)^{2t}\right\rangle
  =C_t(p^2q^2)^t.
$$

Expanding $p^2=D_p+s$ and $q^2=D_q+s$ gives (B).  This supplies a complete,
FORM-free boundary reducer for all integer indices in the pair sectors.

### 8.6 Masters and golden reductions

For generic $d$ and $s\ne0$, choose

$$
  S=J(1,1,1),\qquad P=J(0,1,1)=T_1^2.
$$

These are the two masters of the full two-loop family: one top-sector sunset
master and one factorized pair-sector master.  LiteRed2's default representative
set should be `j[v2,1,1,1]` and `j[v2,0,1,1]`, modulo list ordering.

The following are exact golden reductions:

$$
\begin{aligned}
J(2,1,1)&=\frac{d-3}{3s}S,\\[2mm]
J(2,2,1)&=\frac{(d-2)(d-3)}{9s^2}S
           -\frac{(d-2)^2}{12s^3}P,\\[2mm]
J(3,1,1)&=\frac{(d-8)(d-3)}{18s^2}S
           +\frac{(d-2)^2}{12s^3}P,\\[2mm]
J(0,2,1)&=\frac{d-2}{2s}P,\\[2mm]
J(0,2,2)&=\frac{(d-2)^2}{4s^2}P,\\[2mm]
J(-1,1,1)&=sP,\\[2mm]
J(-2,1,1)&=s^2\left(1+\frac4d\right)P.
\end{aligned}
\tag{G}
$$

All permutations of the indices give the corresponding same result.

Independent checks on (G):

- mass homogeneity gives
  $\partial_sS=3J(2,1,1)=(d-3)S/s$;
- a second derivative gives
  $6[J(3,1,1)+J(2,2,1)]=(d-3)(d-4)S/s^2$, and the
  $P$ terms cancel;
- substituting the two relations at the seed `(2,1,1)` into (I11) and (I12)
  gives zero;
- the last numerator identity uses
  $\langle(p\cdot q)^2\rangle=p^2q^2/d$, independently checking the
  rank-two tensor projector.

For the opposite denominator convention $s-k^2$, apply
$\widetilde J(a,b,c)=(-1)^{a+b+c}J(a,b,c)$ rather than editing individual
coefficients by inspection.

### 8.7 Required exhaustive regression

The milestone should not stop after reproducing (G).  An exhaustive small-box
test should:

1. enumerate every $a,b,c\in[-2,4]$;
2. classify its sector and apply exact $S_3$ canonicalization;
3. reduce pair-sector numerators with (B), single/empty sectors to zero, and
   all-positive top integrals to a linear combination of $S,P$;
4. evaluate (I11)-(I22) at seeds $1\le a,b,c\le3$, reduce every term, and
   assert an exactly zero coefficient of both masters;
5. assert permutation invariance for every integral in the box;
6. repeat selected tests with all denominator signs reversed and the parity map.

Passing those checks constitutes a complete on-demand two-loop IBP reducer,
not merely a table of hand-coded examples.

## 9. Tensor reduction without FORM

RustRed's IBP core only needs scalar integrals, but Vakint inputs can carry free
Lorentz indices.  The correct boundary is a tensor projector before `Toj`-like
denominator conversion.

For a vacuum integral, no external vector exists.  Therefore:

- every odd total tensor rank vanishes;
- every even-rank result is a linear combination of metric pairings;
- coefficients are obtained by contracting the ansatz with all independent
  metric pairings and solving the resulting rational matrix in $d$;
- contracted loop momenta become scalar products $k_i\cdot k_j$, which are
  then converted through the family's inverse denominator map;
- identical metric-pairing structures must be canonicalized before solving.

At rank two the universal identity is

$$
  \int k_i^\mu k_j^\nu f(k_r\cdot k_s)
  =\frac{g^{\mu\nu}}{d}
   \int(k_i\cdot k_j)f(k_r\cdot k_s).
$$

At rank four there are three metric pairings; at higher rank use perfect
matchings modulo symmetry and solve their contraction Gram matrix.  Cache the
inverse projector by tensor rank and symbolic $d$.

The bundled `Vectors` code confirms the intended primitives:
`DAverage` implements $d(d+2)\cdots$ angular denominators,
`TensorSet` generates metric/vector structures, and `TSCollect` collects their
coefficients.  Those helpers are useful algorithmic references, but applying
an angular average to one loop momentum while denominators still couple it to
other loops is not generally valid.  Use the global tensor ansatz; apply
componentwise angular averages only after a sector has factorized.

The two numerator tests in (G), especially
$J(-2,1,1)=s^2(1+4/d)P$, should be the first scalar/tensor integration test.

## 10. Path from two to five loops

For a vacuum family the basic growth is:

| Loops | Independent scalar products | Momentum-space IBPs | Naive sectors |
|---:|---:|---:|---:|
| 2 | 3 | 4 | 8 |
| 3 | 6 | 9 | 64 |
| 4 | 10 | 16 | 1,024 |
| 5 | 15 | 25 | 32,768 |

An ISP-fixed `SectorsPattern` reduces the physically enumerated sectors, but
the solver still carries all scalar-product indices.

### 10.1 Three loops

Vakint's parent is the six-line tetrahedral family
([`topologies.rs:76-98`](../../vendor/gammaloop/crates/vakint/src/topologies.rs#L76)):

```text
k1, k2, k3, k3-k1, k1-k2, k2-k3
```

All lines have the common mass parameter.  These six denominators span exactly
the six vacuum scalar products, so no ISP is required.  Vakint automatically
registers its distinct contractions.  This is the clean second milestone.

Implementation sequence:

1. register the parent family and exact scalar-product inverse;
2. generate and unit-test all nine IBPs;
3. classify all 64 sectors and prove graph/loop-momentum symmetries;
4. identify factorized and scaleless contractions recursively;
5. run bounded sparse Laporta elimination sector by sector, simplest first;
6. stabilize the candidate master set by increasing dot/numerator bounds;
7. add symbolic recurrences only after finite reductions are trusted;
8. reduce every Vakint contraction of the parent and compare independent
   permutations/routings.

Do not hard-code an expected master count before the rank has stabilized.  Use
exact rank at increasing bounds, parametric critical-point counting when a
pure-Rust implementation exists, and exact equivalence maps as independent
checks.

### 10.2 Four loops

Vakint defines four parent routings at
[`topologies.rs:101-218`](../../vendor/gammaloop/crates/vakint/src/topologies.rs#L101):

```text
H and X     9 propagators -> append 1 ISP to reach 10 scalar products
BMW and FG  8 propagators -> append 2 ISPs to reach 10 scalar products
```

The appended ISP positions must be fixed to sector bit zero.  The physical
sector search is then at most $2^9$ or $2^8$ per parent rather than all
$2^{10}$, though numerator depths still explore the ISP indices.

At this stage the dominant engineering requirements are:

- sparse relation storage with interned integrals;
- symmetry canonicalization before matrix insertion;
- modular/finite-field elimination and rational reconstruction for large
  rational-function systems;
- sector-parallel solving with deterministic merge order;
- recursive factorization into already solved lower-loop components;
- versioned disk caches and resumable per-sector work;
- strict memory budgets and statistics for fill-in/pivot growth.

Vakint automatically registers contractions for FG, while H, X, and BMW are
registered only as unpinched parents.  RustRed should nevertheless derive any
requested contraction from the family rank and sector machinery, rather than
hard-code this current catalog boundary.

The existing FORM/FMFT files may be used only as external inspiration or
eventual output comparison; no RustRed stage should call FORM or translate its
procedures at runtime.

### 10.3 Five loops

Five vacuum loops require 15 scalar products and generate 25 basic IBPs per
seed.  Vakint's current `generate_topologies` list stops after the four-loop
parents and then adds an unknown topology, so RustRed needs a generic family
input format rather than assuming a pre-existing Vakint five-loop catalog.

For a cubic connected five-loop vacuum graph, 12 propagators are typical, hence
three ISPs, but RustRed must compute this from rank rather than assume it.  The
five-loop milestone should begin only after the four-loop engine has:

- finite-field sparse elimination;
- canonical graph and loop-momentum symmetry maps;
- automatic factorization/scaleless detection;
- bounded target-driven reduction rather than eager generation of every rule;
- stable serialization and resumability;
- reproducible master and rule fingerprints.

At 32,768 naive sectors, even cheap metadata should use packed bit masks.  Use
`u16` for up to 15 indices, with a generic larger mask type available for future
families.

## 11. RustRed module blueprint

A practical decomposition is:

```text
family
  momentum/scalar-product basis validation
  denominator affine maps and family fingerprint

integral
  interned exponent vectors, sectors, packed masks
  total complexity and symmetry canonicalization

ibp
  derivative construction
  scalar-product-to-shift conversion
  sparse linear relations

parametric
  U/F/G construction
  zero-sector rank criterion
  optional critical points and syzygies

symmetry
  polynomial candidate signatures
  exact momentum maps, Jacobian checks, induced exponent maps

solver
  finite bounded Laporta engine
  guarded symbolic recurrences
  master stabilization and basis changes

tensor
  metric-pairing basis, contraction matrices, cached projectors
  conversion of tensor numerators to scalar products

reducer
  demand closure, dependency DAG, memoization, collection

cache
  versioned family/order/rule serialization and resumable sector state
```

Coefficient expressions should have three levels:

1. factored/sparse multivariate polynomials in parameters and symbolic indices;
2. normalized rational functions for equality, pivots, and final collection;
3. finite-field images for rank and large elimination.

Avoid simplifying every term after every addition.  LiteRed2's own
`SimplifyAlways`/`CheckZeroAlways` distinction reflects the cost.  Normalize a
pivot coefficient enough to test exact zero, batch row operations, and collect
fully at rule boundaries.

## 12. Acceptance criteria by milestone

### Milestone 1: two-loop fully massive vacuum

- Build the exact three-denominator family and inverse scalar-product map.
- Generate (I11)-(I22) from derivatives, not hard-coded strings.
- Reproduce the sector and symmetry sets in section 8.
- Reduce every index triple in the `[-2,4]^3` test box.
- Reproduce (G) and make all seeded IBP residuals exactly zero.
- Support scalar numerators and rank-two/rank-four tensor projection without
  FORM.
- Serialize/reload rules and obtain byte-stable canonical output.

### Milestone 2: three-loop fully massive vacuum

- Build Vakint's six-line parent and every contraction.
- Generate nine IBPs and exhaustively analyze 64 sectors.
- Prove symmetry maps by exact transformed denominators.
- Reduce a documented target box with a stabilized canonical master set.
- Recursively reduce all factorized sectors to lower-loop masters.
- Include tensor projection and end-to-end Vakint expression conversion.

### Milestones 3 and 4: four and five loops

- Complete ISP-aware families and sector patterns.
- Use target-driven sparse finite-field reduction with reconstruction.
- Resume interrupted per-sector jobs from versioned caches.
- Validate every rule by reducing its generating IBP residual to zero.
- Cross-check equivalent momentum routings, graph automorphisms, mass
  homogeneity, factorization, and independent numerical evaluation where
  available.

## 13. Audit cautions

- `LiteRed2026.m` identifies itself as a beta and has a placeholder release
  date.  Copy mathematics, not incidental front-end or persistence behavior.
- The help text and defaults are not always synchronized.  For example the
  default order in code includes `"cp"`; use executable definitions as the
  authority.
- `FindSymmetries` uses polynomial canonical forms as filters and emits a
  warning when a parametric equivalence has no proved momentum shift.  RustRed
  must retain that distinction.
- The old rule "a sector with fewer denominators than loops is zero" is
  commented out because it fails for nonstandard denominators.  Never restore
  it as a general criterion.
- Symbolic dimension is required by `SolvejSector`; numeric special dimensions
  can have extra degeneracies and need separate reductions.
- Generic-parameter reductions do not automatically cover $s=0$, threshold
  relations, Gram-degenerate external kinematics, or special integer $d$.
- Master lists depend on search depth until rank stability is demonstrated.
- A denominator sign change changes rule coefficients by exponent parity.
- Tensor angular averaging is safe only when its rotational assumptions hold;
  coupled multi-loop denominators require a global tensor projector.
- LiteRed2's disk format consists of Mathematica definitions and delayed rules.
  It should not be treated as a RustRed interchange format.

The two-loop oracle above resolves the main ambiguities before implementation:
the momentum routing, denominator sign, sector boundary, ordering, symmetry
representative, master normalization, and tensor boundary are all explicit.
