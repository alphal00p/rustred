# Finite-frame breakthrough candidates for six-loop closure

## Scope and claim discipline

This note records algorithmic candidates found in a second literature pass through August 2026.
It is a delta to
[`parametric_ibp_literature_2026.md`](parametric_ibp_literature_2026.md): it does not repeat the
targeted-tube, signature/Janet, or PDE generating-function programmes developed there.

The pressure points are:

- `K = 6`: the complete three-loop scalar-product family;
- `K = 10`: a four-loop scaling and oracle proxy; and
- `K = 21`: the complete six-loop scalar-product family.

The success criterion is exact closure with an affordable finite terminal complement. The
terminal set need not be minimal and need not coincide with a conventional master basis.

Statements explicitly attributed to papers are published claims. Sections labelled **RustRed
hypothesis** are proposed adaptations or combinations; the cited sources do not establish their
six-loop practicality.

## Executive verdict

The strongest new candidate is a **finite-frame Macaulay compiler**, preferably supplied with
closed-form Gram/Baikov logarithmic vector fields. It asks directly whether the first border of a
finite terminal frame lies in the IBP source module. Its cost can therefore depend on the border
and a modest completion degree rather than on a large translated lattice tube.

A **resonant GKZ creation/contraction ladder** has greater upside: polytope facets may classify
generic shifts, exceptional hyperplanes, and lower-sector contractions structurally. Its physical
equal-mass restriction and treatment of irreducible scalar products are unresolved enough that it
should be run as a discriminating pilot, not selected as the production architecture yet.

Baikov residue generating functions and structured black-box recurrence discovery are useful
accelerators. Neither is presently an acceptable closure authority on its own.

## Candidate 1: finite-frame Macaulay quotient and border compilation

### Published results

*Macaulay Matrix for Feynman Integrals: Linear Relations and Intersection Numbers*
([arXiv:2204.12983](https://arxiv.org/abs/2204.12983)) constructs Pfaffian matrices from a
holonomic differential ideal without first computing a noncommutative Groebner basis.

For a standard-monomial basis `Std`, it forms a Macaulay matrix from bounded-degree multiples of
the ideal generators. Columns are split into exterior and standard monomials. Expressing each
first-border derivative in the row span yields its action matrix on `Std`. For a
zero-dimensional holonomic ideal, increasing the degree eventually supplies the required
relations. The paper proposes finite-field row selection and rational reconstruction and checks
the resulting Pfaffian system against the source ideal and flatness conditions.

*Restrictions of Pfaffian Systems for Feynman Integrals*
([arXiv:2305.01585](https://arxiv.org/abs/2305.01585)) gives gauge/Moser and Macaulay
algorithms for restricting a Pfaffian or D-module system to hyperplanes and hypersurfaces. The
Macaulay route can construct a restricted system directly with finite-field linear algebra. It is
therefore relevant to the equal-mass and unit-scale locus, where naive substitution can be
singular.

*Complete sets of logarithmic vector fields for IBP identities*
([arXiv:1712.09737](https://arxiv.org/abs/1712.09737)) gives explicit complete sets of
logarithmic vector fields for Gram determinants. These provide low-degree,
dimension-preserving Baikov IBP generators without running a generic syzygy search.

### RustRed hypothesis

Use a connected finite set `O` of proposed evaluation terminals, deliberately allowing `O` to be
redundant. Generate only its first shift border. Construct a modular sparse Macaulay matrix from
exact ordinary IBP sources or the equivalent closed-form logarithmic generators. For every border
element:

1. solve row-space membership against `O`, lower sectors, zero sectors, and factorized sectors;
2. reconstruct the source multipliers exactly;
3. replay the result through RustRed's ordinary-source generator; and
4. stratify every coefficient denominator and close its zero locus separately.

Use the D-module restriction algorithm to specialize to the common-mass physical locus. This is
preferable to substituting equal masses into a generic system and hoping no rank or denominator
singularity was introduced.

This is not a translated-source tube. If completion occurs at a small Macaulay degree, the work is
driven by the first border and the exact quotient presentation rather than by every lattice point
inside a large multidimensional neighborhood.

### Scaling model

The number of commutative derivative monomials through degree `D` in `K` variables is

```text
M(K, D) = binomial(K + D, D).
```

At `K = 21`:

| degree `D` | `M(21, D)` | envelope with 36 base sources |
| ---: | ---: | ---: |
| 1 | 22 | 792 |
| 2 | 253 | 9,108 |
| 3 | 2,024 | 72,864 |
| 4 | 12,650 | 455,400 |

These are only row-count envelopes. Generator degrees reduce some multiplier ranges, while
multiple module components, coefficient monomials, and elimination fill increase the true cost.
Degree three appears plausible for sparse modular linear algebra. Degree four is a warning point:
fill and reconstruction, not the raw row count, may dominate.

Prime instances, border columns, and rational-reconstruction checks can run independently. Workers
should share one immutable sparse pattern and stream only modular values and pivots; copying the
whole matrix per worker would defeat the memory advantage.

### Exact proof obligations

The cited Macaulay theorem uses an actual basis. A deliberately redundant terminal frame requires
an additional presentation certificate.

Let `R` be the exact relation module among the elements of `O`, and let
`Q = span(O) / R`. For every shift action `A_i`, RustRed must prove:

- `A_i` preserves `R`;
- the original IBP source module acts as zero on `Q`;
- `A_i A_j = A_j A_i` modulo `R` for every pair of shifts;
- lower-sector boundary maps agree with the immutable lower-sector artifacts; and
- all coefficient guards and their intersections have complete owners.

Every border relation must carry an exact ordinary-source replay certificate. Exact rank of `R`
then distinguishes a finite nonminimal complement from a finite sample that merely happened not to
expose another missing direction.

### Decisive experiments

- **K=6:** start from a proposed frame containing the current exceptional scalar corners and
  numerator-ray terminals. Find the smallest Macaulay degree that closes the complete first border.
  Remove each border equation in turn and verify that exact membership either recovers it or reports
  the hole. Compare ordinary IBPs with the Gram logarithmic source set.
- **K=10:** attempt degrees one through three, including exact equal-mass restriction. Record rows,
  nonzeros, fill ratio, reconstruction time, frame size, and exact quotient rank. Use FMFT only as
  an offline numerical oracle.
- **K=21:** build degrees one through three for a six-loop banana proxy and a representative
  15-propagator/six-ISP topology before attempting full closure. Reject the lane if degree must keep
  increasing, modular fill becomes dense, or the certified terminal budget is unaffordable.

## Candidate 2: resonant GKZ creation and contraction ladders

### Published results

*A-hypergeometric functions and creation operators for Feynman and Witten diagrams*
([arXiv:2309.15895](https://arxiv.org/abs/2309.15895)) constructs inverse contiguity, or
creation, operators using Newton-polytope facets and Bernstein--Sato factors. The facet data
identify singular parameter hyperplanes. The construction is generic but can require extra
factors for non-normal toric ideals.

*Resonance and Differential Reduction of Feynman Integrals*
([arXiv:2606.09978](https://arxiv.org/abs/2606.09978)) constructs reduction operators on
resonant GKZ faces. Acting at resonance maps an integral to a face subsystem; for edge faces this
can realize a contraction together with parameter or dimension shifts and boundary terms. The
paper demonstrates one-loop families and sunrise/banana examples.

For a generic GKZ system, holonomic rank is related to normalized polytope volume
([Adolphson](https://doi.org/10.1215/S0012-7094-94-07313-4)). Exceptional parameters may
rank-jump; see
[*Homological methods for hypergeometric families*
](https://arxiv.org/abs/math/0406383) and
[*Rank jumps in codimension 2 A-hypergeometric systems*
](https://arxiv.org/abs/math/0404183).

The recent GKZ papers do not claim a complete automated multiloop reduction ladder. They also
identify projection to a low-dimensional physical locus, special equal or zero masses, and the
growth of auxiliary GKZ coefficient variables as open difficulties.

### RustRed hypothesis

Construct a finite operator atlas:

1. use facet creation operators for generic index shifts;
2. at zeros of their Bernstein--Sato factors, use resonant-face operators;
3. interpret edge-face maps as lower-sector contractions;
4. add toric relations to close the finite face ladder; and
5. use Macaulay D-module restriction to reach the equal-mass, unit-scale locus.

This could replace translation search with operators derived from polytope geometry. It may also
turn exceptional-domain discovery into a finite list of facet and resonance strata.

### Scaling and risks

Measure the following before generating large operators:

- number of columns and monomial support size of the `A` matrix;
- number of Newton-polytope facets;
- maximal Bernstein--Sato degree;
- number of states in the resonant face ladder;
- generic volume rank and exactly restricted physical rank; and
- operator order after eliminating auxiliary coefficient directions.

Facet enumeration and face ladders can be exponential. Generic rank-volume statements do not
control rank on a resonant equal-mass locus. Published high-loop creation operators often cannot be
projected to the physical variables because the coefficient-variable space is much larger. The
treatment of ISP-negative indices is also not supplied by the cited constructions.

### Decisive experiments

- **K=6:** construct the generic-mass `A` matrix and facet set, derive both shift directions, and
  restrict exactly to equal mass. Require the resulting atlas to own the currently exposed scalar
  corners and numerator rays.
- **K=10:** compare facet count, maximal operator order, ladder-state count, and restricted rank
  with K=6. Check numerical output against regenerated high-precision data where available.
- **K=21:** perform a structural dry run only. Reject the approach if physical restriction cannot
  be automated, the face ladder approaches all contraction subsets, or the terminal-rank bound alone
  exceeds the evaluation budget.

## Candidate 3: Baikov residue coefficient compilation

### Published results

*Generating Function of Loop Reduction by Baikov Representation*
([arXiv:2504.02573](https://arxiv.org/abs/2504.02573)) shifts the Baikov variables by
generating parameters. The maximal residue gives the top-sector coefficient generating function

```text
G_top(t) = (P(t) / P(0))^gamma,
```

so arbitrary positive propagator powers follow from Taylor coefficients. The paper also derives a
one-less-residue expression for subtop coefficients and gives two- and three-loop vacuum examples
compared with FIRE.

The paper does not claim closure for arbitrary numerator powers or all recursively generated lower
sectors.

### RustRed hypothesis

Compile the residue formula into exact coefficient recurrences for positive-dot directions. Use
the general foundry only for ISP-negative directions, exceptional loci, and deeper contractions.
Map every compiled recurrence back to an ordinary RustRed source certificate before publication.

Unlike PDE descendant completion, this is an analytic coefficient formula. Its benefit is the
fraction of the infinite positive-dot tower that it removes from symbolic search.

### Scaling and falsification

For unrestricted total dot rank `r`, naive multivariate coefficient enumeration grows as
`binomial(K + r, r)`. Sparse polynomial support and a recurrence DAG may reduce this, but the paper
does not provide a general K=21 bound. Recursive cut handling may approach `2^N` sectors.

- **K=6:** reproduce exact mixed positive-dot coefficients through a high rank and measure how many
  current numerator and exceptional strata remain uncovered.
- **K=10:** compile top and immediate-subtop coefficients, then compare the scalar result with an
  FMFT-derived numerical oracle.
- **K=21:** build the sparse Baikov polynomial and compile rank-20 top-sector coefficients plus a
  selected set of subtop sectors. Reject the lane if cut enumeration or intermediate coefficient
  state approaches the full sector or monomial powerset.

## Candidate 4: structured black-box recurrence discovery

### Published results

Relevant primary algorithms include:

- cone-, lattice-, symmetry-, and polynomial-coefficient recurrence guessing in skew-polynomial
  settings ([arXiv:2009.05248](https://arxiv.org/abs/2009.05248));
- sparse Wiedemann/FGLM conversion
  ([arXiv:1304.1238](https://arxiv.org/abs/1304.1238));
- zero-dimensional ideals from multidimensional linearly recurrent sequences
  ([arXiv:1707.01971](https://arxiv.org/abs/1707.01971)); and
- border bases and multiplication tables from finite-rank Hankel data
  ([arXiv:1705.01328](https://arxiv.org/abs/1705.01328)).

These methods can recover relation ideals or multiplication matrices from structured tables. Their
published flat-extension guarantees do not directly prove closure for a polynomial-coefficient IBP
Ore module.

### RustRed hypothesis

Obtain modular master-coefficient probes from small exact IBP solves. Use symmetry-aware
Scalar-FGLM, block Wiedemann, or Hankel rank profiles to propose a terminal staircase and sparse
polynomial-index recurrences. Then lift every proposed recurrence to an exact ordinary-source
combination with the Macaulay compiler.

The discovery phase may scale with terminal rank `t`, sparse action-matrix nonzeros, and roughly
`O(t)` to `O(t^2)` probes instead of a complete translation volume. Different primes, random linear
forms, and probe blocks are embarrassingly parallel. Workers need exchange only compact modular
residue vectors and support candidates.

### Risks and falsification

Generating a table entry may already require the reduction being sought. A finite sample can also
support false recurrences. Therefore:

- hold out lattice points, primes, and random projections;
- require stable support under all held-out data;
- reconstruct coefficients exactly;
- replay the relation as an ordinary IBP source combination; and
- require an exact first-border and guard-coverage proof afterward.

At K=6, hide known rules and test whether the method recovers them and the exposed rays. At K=10,
compare probe cost with direct Macaulay compilation. At K=21, reject the lane when query cost
exceeds the direct method, support keeps changing, or exact lifting fails. Sampled rank
stabilization alone is never a closure certificate.

## Candidate 5: complete Gram logarithmic sources as an accelerator

The explicit logarithmic-vector-field generators of
[arXiv:1712.09737](https://arxiv.org/abs/1712.09737) should be benchmarked as the source
module for candidates 1 and 4. The RustRed hypothesis is that these low-degree complete generators
will reduce Macaulay degree, row count, and elimination fill compared with translated ordinary
momentum-space IBPs.

This is not a standalone closure algorithm. It succeeds only if it yields the same exact complete
border with materially smaller matrices and source certificates that RustRed can replay.

## Finite nonminimal terminals and evaluation cost

### Certification versus sampled misses

A terminal count inferred from numerical rank, a finite lattice box, or failure to discover a new
rule is not authoritative. A finite nonminimal terminal complement is certified only by:

1. exact closure of the complete first border;
2. an exact relation module and its rank;
3. invariance and confluence of all shift actions modulo that module;
4. complete exceptional-guard stratification; and
5. lower-sector induction.

The terminal-frame size `t` is an evaluation and storage cost. The exact quotient rank `r <= t`
is the number of independent degrees of freedom. Minimizing `t` or `r` is useful but is not part of
the mathematical closure requirement.

### Exact and numerical basis mapping

An exact map from RustRed terminals to the conventional MATAD or FMFT master basis is optional.
When bases differ, Vakint may use separately generated numerical Laurent values for the RustRed
terminal set and use numerical Laurent-series parity as the cross-backend acceptance test. Exact
RustRed source replay, closure, and terminal completeness remain mandatory.

The precision of existing oracle data must not be overstated:

- MATAD's shipped three-loop tables are approximately 20,000 digits and can support very-high-
  precision K=6 diagnostics;
- most currently shipped FMFT four-loop constants are only about 26--50 digits, with only a few
  constants near 20,000 digits; and
- generic four-loop terminal values at 20,000 digits would therefore require regeneration or an
  AMFlow campaign rather than simple reuse of the shipped FMFT tables.

FORM-derived recurrence equations must never enter RustRed. Offline numerical values produced by
MATAD, FMFT, or AMFlow may be shipped by Vakint as terminal-evaluation data.

If a retained frame is redundant, its independently generated values must satisfy the exact
relation module. Preferably, evaluate an independent quotient basis and derive the redundant frame
values exactly, when that relation map is inexpensive.

### Storage and AMFlow budget

For `t` terminals, `n_eps` Laurent coefficients, and `p = 20,000` decimal digits, the raw binary
lower bound is

```text
p * log2(10) / 8 ~= 8.3 KiB per Laurent coefficient,
```

or approximately

```text
8.3 KiB * t * n_eps.
```

Decimal text needs roughly `20 KiB * t * n_eps` before metadata. For example,
`t = 1,000` and `n_eps = 20` need about 166 MiB in packed binary or 400 MB as decimal text before
format overhead.

AMFlow cost may be lower than `t` independent evaluations when terminals share a coupled family,
or higher when continuation and precision conditioning dominate. It must be measured with batched
pilots. Record:

- independent quotient rank and retained frame size;
- Laurent depth and requested precision;
- grouped AMFlow wall time and peak memory;
- action-matrix and relation-module nonzeros;
- conditioning and guard-proximity information; and
- final compressed artifact size and load/apply latency.

A candidate is affordable only when the combined foundry, evaluation, artifact, and application
budget is acceptable. A small conventional master count is not required; a huge numerically
expensive terminal frame is still a practical failure.

## Common proof and falsification gates

Every successful candidate must provide:

- exact regenerated-source replay for every compiled relation;
- a connected finite frame with a completely reduced first border;
- strict descent or a finite quotient action with a deterministic normal-form section;
- explicit terminals, zero sectors, factorization, and lower-sector ownership;
- complete guard stratification, including intersections;
- deterministic precedence and identical exact normal forms on overlapping rule domains;
- exact preservation of terminal relations and commuting shift actions modulo those relations; and
- deterministic artifacts across supported worker counts and prime schedules.

The common scaling ladder is:

1. **K=6:** close all currently exposed scalar corners and numerator rays with exact certificates;
   measure degree, fill, quotient rank, redundant terminals, and evaluation cost.
2. **K=10:** repeat with exact physical restriction and FMFT/AMFlow numerical diagnostics; reject
   any candidate showing an uncontrolled degree, fill, facet, cut, or probe-count jump.
3. **K=21:** first run structural dry runs on a banana proxy and a representative
   15-propagator/six-ISP topology. Stop before full closure if the predicted terminal table,
   completion matrices, or lower-sector ladder is unaffordable.

## Priority order

1. Prototype the finite-frame Macaulay compiler with Gram logarithmic sources.
2. In parallel, test whether a GKZ creation/resonance atlas survives exact equal-mass restriction.
3. Use Baikov residue formulas to remove positive-dot towers where they give compact exact code.
4. Use structured black-box recurrence discovery only as a proposal generator whose output is
   certified independently.

No cited primary source currently demonstrates practical all-rank K=21 closure. These experiments
are designed to fail cheaply before RustRed commits to a six-loop architecture.
