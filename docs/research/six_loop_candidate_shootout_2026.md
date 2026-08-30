# Six-loop parametric-IBP candidate shootout

## Scope and acceptance standard

This note compares post-LiteRed2 algorithmic candidates for exact, universal parametric-IBP
closure. It uses the Symbolica 2.2.0 feasibility boundary established in
[`symbolica_finite_frame_feasibility.md`](symbolica_finite_frame_feasibility.md) and focuses on:

- Baikov logarithmic vector fields and syzygy-constrained sources;
- finite-frame border bases and Macaulay quotients;
- modular black-box Scalar-FGLM, Krylov, and block Wiedemann methods;
- signature, Janet, and Ore-algebra completion; and
- GKZ/Pfaffian systems and D-module restriction.

The stress points are `K = 6`, `10`, `15`, and `21`, corresponding to complete vacuum
scalar-product spaces through three, four, five, and six loops. Statements attributed to papers
are published results. Sections labelled **RustRed inference** or **RustRed proposal** are not
claims made by the sources.

Master minimality is deliberately relaxed. That does not relax closure. A candidate succeeds only
if it produces:

1. exact relations replayable into regenerated ordinary IBP sources;
2. complete ownership of every sector, boundary, and coefficient-guard stratum;
3. a well-founded, deterministic normal form for every supported integral rank;
4. a finite universal terminal frame with exact relations among redundant terminals;
5. a finite bound on accumulated epsilon-pole depth; and
6. a terminal quotient that can realistically be evaluated simultaneously by AMFlow or an
   approved offline evaluator.

Finite-box success, stable modular rank, agreement between sampled reduction paths, a critical
point count, or numerical parity is evidence. None is an all-rank closure certificate.

## Executive ranking

No reviewed method has demonstrated practical universal closure for `K = 21`. The recommended
research order is:

| rank | architecture | role | `K = 21` verdict |
| ---: | --- | --- | --- |
| 1 | Baikov + physical finite frame | Primary hybrid | Conditional promotion |
| 2 | Generating-function guided extensional completion | Parallel closure bet | Prototype |
| 3 | Seedless or syzygy symbolic lowering plus exact coverage | Rule discoverer | Prototype |
| 4 | Signature/Janet completion in a sector-local Ore algebra | Systematic fallback | High risk |
| 5 | Generic GKZ/Pfaffian followed by physical restriction | Alternative theory lane | Defer |

Black-box Krylov, Scalar-FGLM, and block Wiedemann are not a separate closure architecture. They
rank first as a possible accelerator when sparse elimination fill dominates, but last as a source
of certificates: a projected sequence can miss quotient directions and exceptional strata.

The best composite is therefore:

```text
closed-form Baikov/logarithmic generators
        -> direct equal-mass translated-source frame
        -> modular sparse or black-box row discovery
        -> exact source-certificate reconstruction and replay
        -> exact border, relation-module, guard, and pole-depth proof
```

This order exploits Symbolica's arithmetic without depending on facilities it does not provide.

## Shared scaling envelope

For an `L`-loop vacuum family, ordinary momentum-space IBPs supply `L^2` base sources and
`K = L(L + 1) / 2` scalar-product coordinates. A one-sided Macaulay translation space through
total degree `d` has

```text
M(K, d) = binomial(K + d, d)
```

monomials. The resulting raw row envelopes are:

| `K` | loops | sources | degree 2 | degree 3 | degree 4 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 6 | 3 | 9 | 252 | 756 | 1,890 |
| 10 | 4 | 16 | 1,056 | 4,576 | 16,016 |
| 15 | 5 | 25 | 3,400 | 20,400 | 96,900 |
| 21 | 6 | 36 | 9,108 | 72,864 | 455,400 |

These numbers exclude module components, sectors, guards, and elimination fill. They also assume
that a one-sided chart is valid. A two-sided signed `L1` shift ball contains

```text
D_K(h) = sum(j = 0..min(K,h), 2^j binomial(K,j) binomial(h,j)).
```

At radius three the translated-source counts are already:

| `K` | signed points | translated sources |
| ---: | ---: | ---: |
| 6 | 377 | 3,393 |
| 10 | 1,561 | 24,976 |
| 15 | 4,991 | 124,775 |
| 21 | 13,287 | 478,332 |

At `K = 21`, radius four gives 5,167,044 translated sources. A global double-shift completion can
therefore lose more than an order of magnitude relative to a sector-local one-sided frame.

For illustration, 50 nonzeros per row give 3.64 million input nonzeros at `K = 21`, degree three,
and 22.77 million at degree four. A finite-field value plus a `u32` column has an eight-byte floor:
about 29 MiB and 182 MiB before row pointers, allocation, dense scratch, duplicated primes, `U`, or
fill. At 12--16 bytes per stored input entry, the inputs alone are about 44--58 MiB and 273--364
MiB. Tenfold fill is already a serious degree-four failure; hundredfold fill is fatal.

The average support of 50 is an explicit modelling assumption, not a measurement. Every proxy run
must replace it with observed rows, nonzeros, fill, peak resident memory, and I/O.

## Candidate 1: Baikov logarithmic sources

### Published result

[*Complete sets of logarithmic vector fields for integration-by-parts identities of Feynman
integrals*](https://arxiv.org/abs/1712.09737) proves that the logarithmic vector fields of the
Gram determinant relevant to dimension-preserving Baikov IBPs are generated by `L(L + E)` explicit
Laplace-expansion syzygies. Their coefficients have degree at most one in Baikov variables and the
formula is valid at arbitrary loop and external-leg count.

For a six-loop vacuum, this is a closed-form set of 36 low-degree generators. It avoids a generic
S-polynomial computation for the dimension-shift-free syzygy module. The paper's completeness
theorem is exactly about that syzygy module, not about completion of all integral-index recurrences.

To prohibit raised powers of selected propagators, the paper intersects the logarithmic module
with additional divisibility constraints. It discusses cut reductions with unit propagator powers.
Thus the low-degree 36-source statement must not be confused with a proof that every dotted,
pinched, or numerator boundary is reduced by those 36 rows without further module work.

The newer symbolic-rule method
[*Feynman Integral Reduction using Syzygy-Constrained Symbolic Reduction Rules*
](https://arxiv.org/abs/2507.11140) uses sector syzygies, operator-level row reshuffling, and small
target-neighbourhood systems. It demonstrates rank-20 two-loop examples. Its neighbourhood size and
index ordering are explicitly heuristic, and its large application still generated 152,113
equations and 152,104 variables before fast finite-field solving.

[*Seedless Reduction of Feynman Integrals*
](https://arxiv.org/abs/2602.22111) constructs generic-index lowering operators from IBP-generating
vectors and treats bulk, boundary, and boundary-of-boundary problems. Its examples require level
extensions as high as four for some boundaries. It supplies no topology-independent bound on the
required level or on the number of boundary systems at high loop order.

### RustRed inference

Baikov logarithmic generators are the best source preconditioner because they are explicit,
low-degree, topology-generic, and have a precise completeness theorem. They do not reduce the
six-loop source count below the 36 ordinary vacuum IBPs automatically. Their likely gain is smaller
coefficient support, avoidance of dimension shifts, and better-conditioned frame matrices.

For the unit-mass vacuum lane, use the full closed-form logarithmic set first. Do not pay for the
no-doubled-propagator module intersection unless measurements show that it reduces total completion
cost. RustRed permits dots during artifact discovery, so a larger but easily generated source set
may be better than an expensive constrained syzygy basis.

### Hidden failures

- Completeness of logarithmic vector fields is not recurrence closure.
- Equal-mass specialization, sector pinches, and divisibility constraints can change module rank.
- A spanning-cut reconstruction needs exact compatibility on cut intersections and lower sectors.
- Symbolic leading coefficients introduce exceptional integer and `D` hyperplanes.
- Boundary systems can approach the number of subsets of active numerator or propagator directions.

### Symbolica boundary

Symbolica supplies exact multivariate polynomials, rational functions, differentiation, evaluation,
finite fields, dense matrices, and commutative ideal F4. RustRed must implement the Gram/Baikov
coordinate map, closed-form Laplace generators, cut and sector bookkeeping, constrained module
intersection, and ordinary-source replay. Symbolica exposes no free-module syzygy or module-Groebner
API, so arbitrary constrained intersections are not a library call.

### Cheap falsification

- At `K = 6`, compare the exact row span and support of the nine logarithmic and nine ordinary
  sources on every K4 contraction chart through translation degree two. Require identical ownership
  of the complete first border after lower-sector feedback.
- At `K = 10`, compare unconstrained logarithmic sources against a no-raised-power module
  intersection on both the nine-propagator/one-ISP parent and a five-propagator banana. Reject the
  constrained lane if module construction costs more than the rows it removes.
- At `K = 15`, generate the 25 closed-form sources for a 12-propagator/three-ISP parent and a
  six-propagator banana. Measure coefficient support and modular rank through degree two before any
  symbolic module completion.

## Candidate 2: finite-frame border and Macaulay completion

### Published result

[*Macaulay Matrix for Feynman Integrals: Linear Relations and Intersection Numbers*
](https://arxiv.org/abs/2204.12983) constructs Pfaffian action matrices by multiplying a
zero-dimensional differential ideal to increasing degree and solving first-border membership. The
algorithm assumes a standard-monomial basis; termination follows from zero-dimensionality but no
small completion degree is known a priori. Its six-point example contains a `945 x 958` block at
degree two, and full functional reconstruction is left for future work.

Classical border-basis algorithms give exact quotient normal forms when a zero-dimensional ideal
and a valid order ideal are supplied. Kehrein and Kreuzer's
[*Computing border bases*](https://doi.org/10.1016/j.jpaa.2005.07.006) gives algorithms based on
stable spans and linear algebra. Braun and Pokutta show that selecting a maximum-weight admissible
order ideal is NP-hard in
[*Border bases and order ideals: a polyhedral characterization*
](https://arxiv.org/abs/0912.1502).

The last result supports relaxed minimality: RustRed should not solve an NP-hard terminal-frame
optimization problem before it knows whether a modest redundant frame closes. It does not license an
ever-growing terminal list.

### RustRed proposal

Work directly on the equal-mass physical stratum. For a connected candidate frame `O`, generate
translated ordinary or logarithmic sources extensionally in sector-chart coordinates. Split columns
into exterior, frame, and lower-sector integral keys. Search for exact relations for the complete
first border, not only for currently requested targets.

If `O` is redundant, compute its exact relation module `R` and treat the terminal space as
`span(O) / R`. Prove that every shift action preserves `R`, that commuting shifts commute modulo
`R`, and that all action rows replay into original IBP sources. This additional presentation proof
is not part of a classical border basis with an independent order ideal.

The discrete IBP coefficients depend on index variables, so a commutative border calculation alone
is insufficient. Extensional source translation must enforce the Ore relation between shifts and
coefficients. All leading-coefficient zeros require exact guard strata.

### Why it ranks first

- It asks the desired question directly: does a finite universal terminal frame have a closed
  border?
- Relaxed minimality lets it avoid expensive frame optimization.
- It can use one-sided sector charts rather than a global signed translation ball.
- Every accepted border row can carry a compact original-source certificate.
- Modular discovery and exact replay are cleanly separated.

### Hidden failures

- A guessed frame can keep growing with the inspected border and never prove finite complement.
- Completion degree may rise beyond three; row count and fill then dominate.
- A generic modular quotient can hide coefficient-zero strata.
- First-border closure without relation preservation or commuting actions is not enough for a
  redundant frame.
- A finite but large quotient can be impossible to evaluate through AMFlow.
- Rule coefficients can accumulate unbounded negative epsilon valuation with target rank.

### Symbolica boundary

Symbolica supplies rational-polynomial coefficients, `Zp`/`Zp64`, CSR matrices, incremental sparse
row reduction, dense `solve_any`, CRT, scalar rational reconstruction, and univariate Newton
interpolation. RustRed must own checked CSR assembly, frame and border enumeration, source
provenance, prime/sample scheduling, pivot consensus, adaptive multivariate reconstruction,
relation-module recovery, guard completion, exact replay, and pole-depth analysis.

### Cheap falsification

- `K = 6`: attempt degrees zero through three on the complete K4 family. Remove each source and each
  border owner in turn; the corresponding hole must be exposed. Require exact closure of every guard
  stratum and deterministic artifacts across worker counts.
- `K = 10`: attempt degrees one through three on the parent and banana. Record terminal-frame size
  `t`, quotient rank `r`, relation support, input nonzeros, elimination fill, exact certificate
  size, and peak memory.
- `K = 15`: construct degree-one and degree-two frames, then degree three only if projected fill is
  within budget. A terminal set that continues to grow when the tested border is enlarged fails.

## Candidate 3: black-box Scalar-FGLM and block Wiedemann

### Published result

Wiedemann's
[*Solving sparse linear equations over finite fields*
](https://doi.org/10.1109/TIT.1986.1057137) derives minimal-polynomial, rank, determinant, and solve
algorithms from projected Krylov sequences. Sparse matrix-vector products replace fill-heavy
elimination and auxiliary memory is linear in the matrix dimension, but the principal algorithms are
probabilistic.

Coppersmith's
[*Solving homogeneous linear equations over GF(2) via block Wiedemann algorithm*
](https://doi.org/10.2307/2153413) uses block projections to expose more invariant-factor
information and parallelize Krylov generation. Villard analyses the parallel sparse-system method in
[*Further analysis of Coppersmith's block Wiedemann algorithm*
](https://doi.org/10.1145/258726.258742).

Faugère and Mou's
[*Sparse FGLM algorithms*](https://arxiv.org/abs/1304.1238) exploits sparse multiplication matrices
of a zero-dimensional quotient. In the shape-position case, its probabilistic Wiedemann lane has
complexity `O(r (N1 + K log r))`, where `r` is quotient dimension and `N1` is the number of
nonzeros of the selected multiplication matrix. The assumptions are a known finite quotient and a
sufficiently revealing projection.

[*Block-Krylov techniques in the context of sparse-FGLM algorithms*
](https://arxiv.org/abs/1712.04177) computes a univariate description for a zero-dimensional ideal
from block Krylov sequences without assumptions beyond dimension zero. This is stronger than scalar
projection, but dimension zero remains an input fact, not an output closure proof for the IBP
problem.

The multidimensional Scalar-FGLM literature also contains a direct warning. Berthomieu, Boyer, and
Faugère's
[*Linear algebra for computing Groebner bases of linear recursive multidimensional sequences*
](https://doi.org/10.1016/j.jsc.2016.11.005) needs genericity and finite-recursion assumptions; a
truncated sequence can satisfy apparent relations without determining all later terms. Adaptive
query reduction trades information for a greater need to certify the guessed relations afterwards.

### RustRed inference

Black-box Krylov is attractive only after a candidate finite frame and its exact source operator
exist. It can answer modular rank, row-membership, or nullspace questions without storing a filled
echelon form. It cannot decide that the frame is universal, find all symbolic guard strata, or prove
that a projected recurrence owns every quotient direction.

For a large rectangular Macaulay matrix, RustRed needs a rank-preserving rectangular block-Krylov
formulation or certified preconditioner. Forming `A^T A` is unsafe over finite fields because
isotropic vectors can destroy rank. Source multipliers, rather than only a kernel vector of a
preconditioned matrix, must be recovered and replayed exactly.

### Memory and parallelism

For a square dimension `N`, scalar Wiedemann uses roughly `O(N)` sparse matrix-vector steps and
stores the sparse operator plus linear-sized Krylov state instead of a filled `U`. Blocking shortens
the sequential Krylov chain and exposes matrix-by-block parallelism, at the cost of block sequence
storage and dense polynomial-matrix work.

This changes the `K = 21` risk from fill to repeated I/O. A 3.64-million-nonzero degree-three matrix
read tens of thousands of times is practical only if its pattern and values stay resident or the
matvec is regenerated cheaply. Distributed workers should partition rows and reduce block vectors;
shipping matrices or full Krylov states between workers defeats the method.

### Symbolica boundary

Symbolica supplies finite-field arithmetic, sparse matrices, sparse matrix multiplication, CRT, and
rational reconstruction. No Wiedemann, block Krylov, block minimal-generator, Scalar-FGLM, sparse
transpose solve, or checkpoint/restart controller was found. RustRed must implement all of those,
including deterministic random streams, rectangular preconditioning, bad-projection detection,
multi-prime consensus, provenance recovery, and exact lifting.

### Cheap falsification

- `K = 6`: compare black-box rank and recovered nullspaces against Symbolica sparse elimination for
  every frame degree. Use deliberately rank-deficient projections and leave-one-source-out matrices.
- `K = 10`: measure matvec bandwidth, Krylov length, checkpoint volume, and total bytes read.
  Recover several border certificates, not merely the rank, and replay them exactly.
- `K = 15`: run a degree-three modular structure test only if its source matrix fits resident
  memory. Kill the lane if I/O time exceeds direct elimination or if provenance recovery densifies
  the certificate beyond the direct method.

## Candidate 4: generating functions and Ore completion

### Published result

[*An Algorithm for the Symbolic Reduction of Multi-loop Feynman Integrals via Generating
Functions*](https://arxiv.org/abs/2605.09541) rewrites sector IBPs as differential equations in a
noncommutative Weyl algebra. It iterates equation generation, descendant construction, rule
extraction, global substitution, and lattice-complement analysis. Its examples include sunset and
planar/nonplanar double boxes.

The paper's lattice geometry is the closest published match to RustRed's desired coverage engine:
rule leaders own upward orthants and the remaining lattice complement diagnoses missing rules.
However, when the master count is unknown, its stated stopping check reduces selected exterior
points by different paths. Agreement of selected paths does not prove that no hidden relation or
coefficient-zero branch remains.

Barakat et al. formulate IBPs as a finitely generated left ideal in a rational double-shift algebra
in
[*Feynman integral reduction using Groebner bases*
](https://arxiv.org/abs/2210.05347). A complete noncommutative Groebner basis would give normal
forms once and for all. The practical counterexample is decisive: existing implementations failed
to compute the basis even for the on-shell two-loop kite, motivating a linear-algebra ansatz whose
generation claim could not be verified in that example.

Signature methods can suppress redundant critical pairs and retain input provenance. Hofstadler and
Verron prove a signature algorithm with syzygy and F5-style criteria for free algebras in
[*Signature Groebner bases, bases of syzygies and cofactor reconstruction in the free algebra*
](https://arxiv.org/abs/2107.14675). It terminates when a finite signature basis exists.
Free-algebra ideals and syzygy modules may have infinite bases, and the paper is not an Ore-algebra
implementation or an IBP complexity theorem.

The older sector-basis programme in
[*Applying Groebner Bases to Solve Reduction Problems for Feynman Integrals*
](https://arxiv.org/abs/hep-lat/0509187) and the Janet-oriented recurrence programme in
[*Groebner Bases in Perturbative Calculations*
](https://arxiv.org/abs/hep-ph/0501053) establish the conceptual relevance of shift-operator normal
forms. Their small-loop examples do not resolve the critical-pair explosion observed in later work.

### RustRed inference

There are two distinct bets:

1. **Generating-function guided extensional completion.** Reuse the paper's descendant and lattice
   selection strategy, but materialize each accepted operator as exact translated ordinary-source
   certificates. Let RustRed's guard-aware standard-pair coverage be the closure authority.
2. **Full signature/Janet Ore basis.** Implement a sector-local one-sided shift algebra with
   signatures, involutive divisions, critical-pair criteria, and comprehensive guard strata.

The first bet ranks second overall because it can improve source selection without demanding a full
new algebra system. The second is a high-risk fallback. A Janet basis can make coverage explicit,
but nonmultiplicative prolongations often enlarge the basis; signatures remove provably redundant
pairs, not intrinsically difficult ones.

### Hidden failures

- Rationalizing leading coefficients loses the exceptional integer hyperplanes where they vanish.
- A global double-shift order sees the signed-ball growth shown above.
- Critical-pair counts can be quadratic in a rapidly growing basis; coefficient support and
  provenance grow as well.
- Termination of a generic signature algorithm does not imply an affordable finite IBP basis.
- Sector and boundary restriction can require separate comprehensive bases.
- Lattice orthant coverage without exact guard ownership is incomplete.

### Symbolica boundary

Symbolica supplies coefficient arithmetic and commutative ideal F4. It supplies no Ore or Weyl
algebra, operator monomial order, noncommutative normal form, signature criterion, Janet division,
critical-pair queue, free-module basis, or source-cofactor reconstruction. The guided extensional
lane can use Symbolica arithmetic while RustRed implements a small canonical operator record. The
full lane requires substantial new algebra infrastructure and must not wrap commutative F4 as if it
were noncommutative completion.

### Cheap falsification

- `K = 6`: reproduce the complete lattice complement and every guard branch with both extensional
  descendants and a small signature prototype. Compare number of accepted rules, discarded pairs,
  maximum support, and exact replay size.
- `K = 10`: cap completion by operator degree and critical-pair count. If the basis or pending queue
  grows faster than the corresponding finite-frame rows without closing more border, stop.
- `K = 15`: perform a leading-monomial and signature simulation before coefficient arithmetic.
  Reject full Ore completion if radius-three support approaches the 124,775 translated-source
  envelope or if predicted critical-pair storage exceeds the finite-frame degree-three matrix.

## Candidate 5: GKZ and D-module restriction

### Published result

Macaulay/Pfaffian and GKZ methods can expose finite holonomic quotients, creation operators, and
resonant face systems. They are theoretically attractive because polytope faces may organize
contractions and exceptional parameters.

The physical-stratum evidence is adverse. In
[*Restrictions of Pfaffian Systems for Feynman Integrals*
](https://arxiv.org/abs/2305.01585), physical restrictions are commonly singular and require
gauge/Moser or D-module restriction. Examples reduce generic ranks `9`, `33`, and `115` to physical
ranks `3`, `7`, and `7`; another generic rank `238` example needs an additional invariant subspace
to reach the known physical rank `8`.

[*Resonance and Differential Reduction of Feynman Integrals*
](https://arxiv.org/abs/2606.09978) emphasizes that integer propagator powers are resonant and form
reducible subsystems. It reports no straightforward generic-GKZ restriction for the equal,
nonzero-mass bubble, and higher sunrise/banana systems require enlarged derivative vectors because
Euler equations do not eliminate all nonphysical directions.

Rank jumps at exceptional parameters are mathematically real. Matusevich and Walther construct
arbitrarily large rank-volume gaps in
[*Arbitrary rank jumps for A-hypergeometric systems through Laurent polynomials*
](https://arxiv.org/abs/math/0404183).

### RustRed inference

Generic GKZ construction followed by equal-mass restriction ranks last for implementation. It moves
the problem into a larger coefficient-variable space and then requires exactly the singular
restriction machinery missing from Symbolica. A generic rank or Newton-polytope volume cannot
certify the physical unit-mass terminal quotient.

GKZ facets, Bernstein--Sato factors, and critical-point counts remain useful discovery aids for
candidate guards and terminal-rank expectations. Any resulting recurrence must still be expressed as
ordinary IBP source combinations on the physical stratum.

### Symbolica boundary

Symbolica can represent the commutative toric polynomials, run ideal F4, and solve finite matrices.
RustRed would need to implement the `A`-matrix and face atlas, Weyl action, standard monomials,
Bernstein--Sato computation, holonomic rank stratification, Moser/gauge transforms, D-module
restriction, invariant-subspace extraction, and exact map back to ordinary IBPs.

### Cheap falsification

- `K = 6`: construct generic and direct equal-mass systems independently. Require identical exact
  physical rank, first-border actions, and ordinary-source replay. Naive coefficient substitution is
  not an allowed restriction algorithm.
- `K = 10`: count generic coefficient directions, polytope facets, generic rank, restricted rank,
  operator order, and derivative-vector enlargement for parent and banana proxies.
- `K = 15`: perform only structural enumeration. Defer the lane if the physical basis must be
  guessed, if restriction rank is unknown, or if auxiliary directions exceed the direct `K = 15`
  frame by a large factor.

## Universal terminal and epsilon gates

Every candidate must report terminal-frame size `t`, exact quotient rank `r`, and a sparse exact map
from all `t` terminals to an independent `r`-element quotient basis. A terminal is not justified by
failure to find a rule. Adding every miss to `O` is acceptable only when the complete border closes
and `R` proves the quotient finite.

The evaluation gate is simultaneous rather than one-terminal-at-a-time. Before a `K = 15` or
`K = 21` promotion, estimate or measure:

- the coupled AMFlow system dimension and singular-point structure;
- requested epsilon order and precision loss for all quotient elements;
- peak RAM, checkpoint I/O, and wall time at the intended precision;
- whether AMFlow itself needs the unavailable reduction, creating a circular dependency; and
- whether the values can be shipped in a practical immutable artifact.

After `D = 4 - 2 epsilon`, compute the exact epsilon-adic valuation of every rule coefficient. Build
the reduction-transition graph annotated by negative valuation and prove a global maximum pole debt.
If a repeatable transition accumulates ever more negative valuation as target rank grows, the
algebraic frame may be finite but no fixed shipped Laurent depth is universal.

MATAD, FMFT, Vakint, and AMFlow may validate numerical Laurent output offline. Their recurrences or
FORM reductions never become RustRed sources. Numerical agreement in a different basis is a parity
gate, not a closure proof.

## Cross-candidate experiment ladder

### Gate A: `K = 6` exact shootout

Use all five three-loop vacuum graph classes and every current corner or numerator-ray obligation.
For every candidate:

1. generate source/rule candidates independently;
2. translate every accepted relation back to exact ordinary-source provenance;
3. close the complete first border and every discovered guard intersection;
4. run source/rule deletion negative controls;
5. compare exact normal forms on overlapping rule domains;
6. prove terminal relations, strict descent, and bounded pole debt; and
7. reproduce artifacts byte-for-byte across supported worker counts.

Only candidates passing Gate A may consume a `K = 10` completion budget.

### Gate B: `K = 10` measured scaling

Use a nine-propagator/one-ISP trivalent parent and a five-propagator/five-ISP banana. Record, rather
than extrapolate:

- base-source count and coefficient support;
- translated rows, columns, and nonzeros by degree or radius;
- rank, terminal `t/r`, and guard-stratum count;
- sparse fill or Krylov matvec count and total bytes read;
- maximum certificate support and exact reconstruction degree;
- peak RAM, I/O, wall time, and parallel efficiency; and
- terminal-evaluation and epsilon-depth estimates.

### Gate C: `K = 15` design kill

Use a 12-propagator/three-ISP parent and six-propagator/nine-ISP banana. Begin with source and
leading-structure generation, not full symbolic completion. Degree or radius three is the maximum
default probe.

A candidate is removed from the six-loop path if it requires an unexplained growing completion
degree, loses physical-stratum rank, cannot recover compact exact provenance, produces a terminal
quotient outside the evaluation budget, or has unbounded pole debt. A good `K = 6` implementation is
not a reason to waive the `K = 15` kill gate.

## Final recommendation

Implement one experiment harness shared by every lane. Its immutable inputs are the physical
family, sector charts, exact ordinary sources, symmetry/lower-sector maps, and resource budget. Its
outputs are candidate rules plus evidence; only the common exact verifier can promote them.

The implementation order should be:

1. closed-form Baikov/logarithmic source generation and comparison with ordinary sources;
2. direct physical finite-frame completion with exact replay and redundant-frame proofs;
3. modular sparse elimination, followed by black-box Krylov only if measured fill warrants it;
4. generating-function descendant selection feeding the same extensional verifier;
5. a bounded signature/Ore prototype, killed early on pair and support growth; and
6. GKZ restriction only after a direct lane establishes the physical quotient to compare against.

This programme treats Symbolica as the arithmetic engine and RustRed as the owner of IBP semantics,
completion geometry, provenance, and proof. It also makes failure informative: each `K = 6`, `10`,
or `15` gate can reject a scaling hypothesis before a costly `K = 21` implementation.

## Primary sources

- J. Böhm et al., *Complete sets of logarithmic vector fields for integration-by-parts identities
  of Feynman integrals*, [arXiv:1712.09737](https://arxiv.org/abs/1712.09737).
- S. Smith and M. Zeng, *Feynman Integral Reduction using Syzygy-Constrained Symbolic Reduction
  Rules*, [arXiv:2507.11140](https://arxiv.org/abs/2507.11140).
- L. de la Cruz and D. A. Kosower, *Seedless Reduction of Feynman Integrals*,
  [arXiv:2602.22111](https://arxiv.org/abs/2602.22111).
- B. Feng et al., *An Algorithm for the Symbolic Reduction of Multi-loop Feynman Integrals via
  Generating Functions*, [arXiv:2605.09541](https://arxiv.org/abs/2605.09541).
- V. Chestnov et al., *Macaulay Matrix for Feynman Integrals: Linear Relations and Intersection
  Numbers*, [arXiv:2204.12983](https://arxiv.org/abs/2204.12983).
- A. Kehrein and M. Kreuzer, *Computing border bases*,
  [doi:10.1016/j.jpaa.2005.07.006](https://doi.org/10.1016/j.jpaa.2005.07.006).
- G. Braun and S. Pokutta, *Border bases and order ideals: a polyhedral characterization*,
  [arXiv:0912.1502](https://arxiv.org/abs/0912.1502).
- D. Wiedemann, *Solving sparse linear equations over finite fields*,
  [doi:10.1109/TIT.1986.1057137](https://doi.org/10.1109/TIT.1986.1057137).
- D. Coppersmith, *Solving homogeneous linear equations over GF(2) via block Wiedemann algorithm*,
  [doi:10.2307/2153413](https://doi.org/10.2307/2153413).
- G. Villard, *Further analysis of Coppersmith's block Wiedemann algorithm for the solution of
  sparse linear systems*,
  [doi:10.1145/258726.258742](https://doi.org/10.1145/258726.258742).
- J.-C. Faugère and C. Mou, *Sparse FGLM algorithms*,
  [arXiv:1304.1238](https://arxiv.org/abs/1304.1238).
- S. G. Hyun et al., *Block-Krylov techniques in the context of sparse-FGLM algorithms*,
  [arXiv:1712.04177](https://arxiv.org/abs/1712.04177).
- J. Berthomieu, B. Boyer, and J.-C. Faugère, *Linear algebra for computing Groebner bases of
  linear recursive multidimensional sequences*,
  [doi:10.1016/j.jsc.2016.11.005](https://doi.org/10.1016/j.jsc.2016.11.005).
- M. Barakat et al., *Feynman integral reduction using Groebner bases*,
  [arXiv:2210.05347](https://arxiv.org/abs/2210.05347).
- C. Hofstadler and T. Verron, *Signature Groebner bases, bases of syzygies and cofactor
  reconstruction in the free algebra*,
  [arXiv:2107.14675](https://arxiv.org/abs/2107.14675).
- A. V. Smirnov and V. A. Smirnov, *Applying Groebner Bases to Solve Reduction Problems for Feynman
  Integrals*, [arXiv:hep-lat/0509187](https://arxiv.org/abs/hep-lat/0509187).
- V. P. Gerdt, *Groebner Bases in Perturbative Calculations*,
  [arXiv:hep-ph/0501053](https://arxiv.org/abs/hep-ph/0501053).
- V. Chestnov et al., *Restrictions of Pfaffian Systems for Feynman Integrals*,
  [arXiv:2305.01585](https://arxiv.org/abs/2305.01585).
- R. Britto, T. W. Grimm, and A. Hoefnagels,
  *Resonance and Differential Reduction of Feynman Integrals*,
  [arXiv:2606.09978](https://arxiv.org/abs/2606.09978).
- L. F. Matusevich and U. Walther,
  *Arbitrary rank jumps for A-hypergeometric systems through Laurent polynomials*,
  [arXiv:math/0404183](https://arxiv.org/abs/math/0404183).
