# Six-loop algorithm update: provenance, complements, and affordability

## Purpose and claim discipline

This note records the architectural delta from a focused primary-literature and public-software
scan through August 2026. It does not replace the longer reviews:

- [`parametric_ibp_literature_2026.md`](parametric_ibp_literature_2026.md) develops the original
  signature/Janet, tube, generating-function, and nonminimal-terminal programme;
- [`finite_frame_breakthrough_2026.md`](finite_frame_breakthrough_2026.md) develops finite-frame,
  Macaulay-border, restriction, and black-box action candidates;
- [`universal_nonminimal_closure_review_2026.md`](universal_nonminimal_closure_review_2026.md)
  reviews physical-stratum source generation and exact all-rank obligations; and
- [`nonminimal_terminal_viability_audit_2026.md`](nonminimal_terminal_viability_audit_2026.md)
  audits proof, pole-depth, storage, and numerical-master budgets.

Statements below labelled **Verified result** summarize claims or measurements in the cited
primary sources. Statements labelled **RustRed inference** are project conclusions; the cited
authors do not claim a practical, exact six-loop RustRed compiler.

The target remains a finite universal reduction for the unit-mass vacuum families with

```text
K = 6, 10, 15, 21
L = 3,  4,  5,  6 loops.
```

Minimality is secondary. Exact finite coverage, an affordable finite terminal-evaluation plan,
and a target-rank-independent epsilon-depth bound are not. An independent quotient is optional
compression, not a prerequisite when every retained typed terminal can be evaluated directly.

This is Stage 2 research guidance. It does not authorize work beyond the active scope in
[`GOAL.md`](../../GOAL.md).

## Executive decision

No reviewed paper or public implementation establishes exact guarded all-rank closure for the
fully massive `K = 21` family. Recent work nevertheless strengthens the hybrid design:

```text
physical-stratum source generation
    -> sector-local syzygy compression and generating-function operators
    -> modular signature traces carrying source cofactors
    -> Janet-like complementary decomposition and explicit open obligations
    -> descendant/prolongation, simplex-face, or tube discovery on those obligations
    -> optional action recovery only after exact finite-frame certification
    -> exact rational, guard, sector, and shift-algebra replay
    -> epsilon-valuation certificate and measured numerical-master campaign
```

The central change is that modular discovery must identify a rule by its source provenance, not by
a pivot pattern alone. Janet-like complements provide the global coverage authority.
Generating-function descendants, seedless faces, and tubes propose compact local rules, while
sector-local syzygies can shrink the source module before any translated frame is materialized.
Block-Krylov or FGLM methods may compress an already proved finite quotient when direct terminal
evaluation is too expensive. None of those discovery or compression methods may replace exact
replay.

## Verified primary-source deltas

### Janet-like complementary decompositions

**Verified result.** Hashemi, Orth, and Seiler show that strong involutive bases induce direct-sum
complementary decompositions of a monomial ideal. Janet and Pommaret divisions yield such
decompositions, while Janet-like bases can give a more condensed complement. They also give a
Janet-tree construction and characterize when the associated Hironaka decomposition is finite:
[*Complementary decompositions of monomial ideals and involutive bases*](https://doi.org/10.1007/s00200-022-00569-0),
DOI `10.1007/s00200-022-00569-0`.

Their recursive follow-up develops variable-recursive criteria and algorithms for Janet and
Janet-like bases, minimization, syzygies, and deterministic transformations toward quasi-stable or
Noether position. It explicitly leaves a full practical implementation comparison for future
work: [*Recursive structures in involutive bases theory*](https://doi.org/10.1016/j.jsc.2023.01.003),
DOI `10.1016/j.jsc.2023.01.003`.

An iterative comprehensive-Groebner algorithm branches on parameter-coefficient vanishing while
using comparatively cheap ideal-membership checks to suppress redundant branches:
[*A new iterative algorithm for comprehensive Gröbner systems*](https://arxiv.org/abs/2404.13514),
arXiv:`2404.13514`.

**RustRed inference.** A condensed Janet-like complement is the best exact data model found for
uncovered orthants and free directions. It can turn every surviving standard pair into an explicit
completion obligation. These results are commutative monomial theory: RustRed must separately
prove coefficient guards on the integer index domain, sector boundaries, ordinary-source
provenance, and the relevant shift/Ore critical pairs. The comprehensive algorithm is a guard-
branching blueprint, not evidence that multivariate physical strata will remain small.

### The weaker owner-cover certificate

**Verified result.** Effective standard-pair algorithms compute irreducible and standard-pair
decompositions for monomial ideals, while complementary-decomposition algorithms can compile
overlapping descriptions into direct sums:
[*Standard pairs for monomial ideals in semigroup rings*](https://arxiv.org/abs/2005.10968),
arXiv:`2005.10968`, together with the complementary-decomposition work cited above. Comprehensive
involutive systems branch on leading-coefficient zero/nonzero conditions and terminate through
Noetherian ideal chains in their stated commutative parameter setting:
[*Comprehensive involutive systems*](https://arxiv.org/abs/1206.0181), arXiv:`1206.0181`.

**RustRed inference.** Universal descending reduction needs less than a complete Ore, Janet, or
Groebner basis. At every exact guard leaf, it is sufficient to retain a finite set of exactly
replayed rules, one global well-founded rank, and a finite symbolic owner cover. For each module
component, let `I` be the commutative monomial ideal generated only by the applicable leading
shifts. A disjoint Stanley cover `C=(a,F)` of its complement can be authenticated by the fine
Hilbert-numerator identity

```text
Q_I(z) = sum_C z^a product_{i not in F} (1-z_i).
```

Every cell with a nonempty free-coordinate set `F` must delegate to another rule, guard wall, or
lower-sector owner. Only a zero-free-direction cell, or a separately certified zero-dimensional
integer wall, may become a finite terminal. This proves span and termination without proving
confluence, exhausting every critical pair, interreducing a basis, or matching an independent
master count. Those omitted steps are precisely the potential saving allowed by nonminimal
terminals.

Index-dependent coefficients cannot be treated as static comprehensive-system parameters. If a
lead is `c(n) S^alpha`, application at target `N` requires the pulled-back guard
`c(N-alpha) != 0`, and prolongation obeys
`S^delta c(n) = sigma^delta(c(n)) S^delta`. Guard-DAG atoms must therefore bind an exact
polynomial identity to its affine target pullback. A commutative leading ideal remains a coverage
index only; it never proves the shifted coefficient wall or the source identity. Discovery should
use a deterministic breadth-fair queue of Ore pairs, nonmultiplicative prolongations, guard
splits, and owner restrictions, but stop as soon as the weaker owner-cover verifier succeeds.
Fairness prevents starvation; it does not turn discovery into a guaranteed terminating
algorithm.

### Signatures, source cofactors, and reusable modular traces

**Verified result.** Hofstadler and Verron construct signature Groebner bases and syzygy bases in
free algebras and reconstruct cofactors in the original generators. The cofactor representation is
an exact ideal-membership certificate:
[*Signature Gröbner bases, bases of syzygies and cofactor reconstruction in free algebra*](https://arxiv.org/abs/2107.14675),
arXiv:`2107.14675`, DOI `10.1016/j.jsc.2022.04.001`.

Hofstadler and Levandovskyy combine signatures with modular computation, Chinese remaindering,
rational reconstruction, exact verification, reusable traces, and parallel primes:
[*Modular Algorithms for Computing Gröbner Bases in Free Algebras*](https://arxiv.org/abs/2502.11606),
arXiv:`2502.11606`, with the public
[`signature_gb`](https://github.com/ClemensHofstadler/signature_gb) implementation. Their reported
benchmarks include speedups above twenty-fold and verification often below fifteen percent of
runtime. They also report serious memory pressure, including unsuccessful 16-thread runs at a
250 GB limit. A finite partial basis below a signature bound is available even when the full basis
does not terminate.

**RustRed inference.** A modular rule should be keyed across primes and `D` specializations by its
leading signature and exact ordinary-source cofactor, not by row or pivot ordinal. The shared
symbolic trace is immutable; workers evaluate primes or parameter points and return compact
modular shards. The lifted rule and its source cofactors are replayed centrally over the exact
rational coefficient field. A signature cutoff is useful discovery data but is not universal
closure, and duplicating a complete basis per worker would defeat the memory model.

Minimum-cofactor optimization must not become a prerequisite. Finding shortest bounded-term
membership proofs is computationally hard, while practical nonminimal certificates are available:
[*Short proofs of ideal membership*](https://arxiv.org/abs/2302.02832), arXiv:`2302.02832`,
DOI `10.1016/j.jsc.2024.102325`.

### Sparse action recovery after a finite-frame proof

**Verified result.** Block-Krylov methods recover change-of-order data for a finite
zero-dimensional quotient in parallel and with sparse matrix-vector products:
[*Block-Krylov techniques in the context of sparse-FGLM algorithms*](https://arxiv.org/abs/1712.04177),
arXiv:`1712.04177`, DOI `10.1016/j.jsc.2019.07.010`. Fast finite-dimensional syzygy algorithms
then operate once multiplication matrices are available:
[*Computing syzygies in finite dimension using fast linear algebra*](https://arxiv.org/abs/1912.01848),
arXiv:`1912.01848`, DOI `10.1016/j.jco.2020.101502`.

Under shape and stability hypotheses, a large multiplication matrix can instead be compressed to
a smaller polynomial-matrix Hermite problem. The reported prototype reaches speedups around five-
fold on its examples:
[*A faster change of order algorithm for Gröbner bases under shape and stability assumptions*](https://arxiv.org/abs/2202.09226),
arXiv:`2202.09226`, DOI `10.1145/3476446.3535484`.

For noncommutative action systems, rational-Weyl border bases require the appropriate integrability
conditions on formal multiplication matrices rather than naive matrix commutativity:
[*Border Bases in Rational Weyl Algebra*](https://arxiv.org/abs/2510.23411),
arXiv:`2510.23411`, DOI `10.1016/j.aam.2026.103065`.

**RustRed inference.** Block Wiedemann, Scalar-FGLM, and Hermite recovery are compression tools
only after an exact finite complement or border certificate exists. Their modular outputs are
probabilistic discovery. RustRed must lift source cofactors as well as action coefficients and
verify the correct discrete shift-algebra flatness relations, source annihilation, guards, and
sector maps. Matrix commutativity at sampled points is insufficient.

### Seedless simplex faces and sparse tubes

**Verified result.** The seedless-reduction construction organizes generic-index lowering into
triangular irreducible-scalar-product levels with bulk, boundary, and boundary-of-boundary systems.
The demonstrated systems retain exact source combinations, but propagator lowering and a complete
general implementation remain future work:
[*Seedless Reduction of Feynman Integrals*](https://arxiv.org/abs/2602.22111),
arXiv:`2602.22111`.

Tube seeding uses sparse paths rather than full rank shells. In the reported two-loop nonplanar
double-pentagon experiments, seed counts grow linearly with target rank at fixed numerical
kinematics, dimension, and prime. The complete rank-20 spanning-cut experiment still reached about
136 GiB peak memory, while chunking rank-10 targets kept individual jobs below about 21 GiB. The
authors explicitly leave a proof that convolving a closing base set with a path closes as future
work:
[*Efficient AI-Inspired Reduction of Feynman Integrals*](https://arxiv.org/abs/2606.10698),
arXiv:`2606.10698`, with public
[`tube_seeding`](https://github.com/andreslunagodoy/tube_seeding) examples.

CALICO obtains parametric annihilators by sparse finite-field linear algebra in several
representations, including Schwinger and Lee--Pomeransky parameters. Higher-order annihilators can
supply relations absent from a first-order ansatz. Its stopping degree and order are user-selected,
and numerical filtering is not a completeness proof:
[*Calico: a general tool for constructing parametric annihilators for Feynman integrals*](https://arxiv.org/abs/2506.13653),
arXiv:`2506.13653`, DOI `10.1007/JHEP10(2025)018`, with public
[`calico`](https://github.com/fontana-g/calico) sources.

**RustRed inference.** Replace translated `L1` boxes with simplex rank layers and recursive faces,
then use sparse tube charts only on Janet-exposed obligations. This can reduce local discovery
volume and allows independent chunks, but it does not remove the possible exponential number of
faces or charts. Every proposed symbolic rule must retain exact source provenance, and every tube
boundary must either join an already certified rule cell or remain an explicit standard-pair
obligation. CALICO is a promising source generator on the physical unit-mass stratum, not a
closure authority.

### Generating-function prolongations and syzygy-compressed rules

**Verified result.** The generating-function construction packages every integral in one sector
as a coefficient of a single generating function and rewrites IBPs as differential equations.
Its expanded algorithm iterates three operations: generate and simplify descendant equations,
solve the surviving operator system for symbolic rules, and inspect the remaining irreducible
index lattice for completeness. A failed completeness check feeds the geometry of the surviving
set back into the next descendant round. The published demonstrations cover the massive and
massless sunset, planar and nonplanar massless double boxes, representative subsectors, and a
sector with no master:
[*Symbolic Reduction of Multi-loop Feynman Integrals via Generating Functions*](https://arxiv.org/abs/2509.21769),
arXiv:`2509.21769`, and
[*An Algorithm for the Symbolic Reduction of Multi-loop Feynman Integrals via Generating Functions*](https://arxiv.org/abs/2605.09541),
arXiv:`2605.09541`.

Syzygy-constrained symbolic reduction instead solves sector-local syzygies that avoid artificial
propagator-power increases, row-reduces the resulting shift/number operators, and falls back to
small symbolic neighborhoods only when operator reshuffling is insufficient. The reported
rank-20 tests are two-loop scattering families. The examples also exhibit leading coefficients
that vanish on exceptional index walls, so the extracted rule is not automatically valid on the
whole integer sector:
[*Feynman Integral Reduction using Syzygy-Constrained Symbolic Reduction Rules*](https://arxiv.org/abs/2507.11140),
arXiv:`2507.11140`.

**RustRed inference.** The irreducible lattice left by the generating-function completeness module
is the geometric object that RustRed already represents as a leader complement or collection of
standard pairs. Under the relaxed minimality requirement, success means driving every
positive-dimensional component to a finite, affordable spanning terminal set; its cardinality
need not equal an independently minimal master count. This removes a needless stopping
condition, but not the hard obligations: every exceptional coefficient wall needs its own exact
guard stratum, every subsector or factorizing boundary needs an owner, and every accepted rule
needs a fixed well-founded descent order and ordinary-source replay.

The most promising hybrid is therefore to use sector-local no-raised-propagator syzygies as a row
compressor, use standard-pair free directions to choose generating-function derivatives rather
than differentiating indiscriminately, and submit the resulting small descendant systems to the
same target-local modular circuit and exact replay boundary as ordinary translated sources. The
published examples do not yet establish that the number of descendants or guard strata stays
affordable at dense five or six loops; K6/K10/K15 falsification remains mandatory.

### Finite spanning closure versus terminal independence

**Verified result.** Relations visible only after supersector elimination can connect integrals
that appear independent in sector-local systems. Relative-cohomology analysis gives a small
physical-locus regression: after the relevant Lee--Pomeransky polynomial degenerates, the
equal-mass bubble's two tadpole subsectors are related only when the top sector and both subsectors
are considered together:
[*Intersection theory, relative cohomology and the Feynman parametrization*](https://arxiv.org/abs/2411.05226),
arXiv:`2411.05226`, Section 3.3. Block-triangular reduction software likewise warns that such
``magic'' relations may require equations from supersectors:
[*Blade: A package for block-triangular form improved Feynman integrals decomposition*](https://arxiv.org/abs/2405.14621),
arXiv:`2405.14621`.

**RustRed inference.** Closure and independence are different theorems. Exact guarded rules with
strict well-founded descent onto a finite terminal collection `T` prove that `T` spans the physical
quotient. They are already sufficient for a universal reducer if every member of `T` has an
independently supplied numerical value; no minimality or independence claim is needed.

Optional compression to a claimed independent set `J` needs a separate guard-stratum-wide rank
sandwich. Reducing an exact finite terminal-relation envelope through the closing rules and
exhibiting `|T|-|J|` independent relation witnesses proves an upper bound on the physical rank and
valid `T -> J` identities. It does not prove that `J` is independent. That lower bound needs an
exact physical-rank theorem or a complete dual pairing with nonzero determinant. Stable modular
rank, maximal cuts, AMFlow values, and SCC-local kernels are valuable diagnostics but do not by
themselves supply this lower half. If it remains unaffordable, RustRed must retain the honest
finite spanning set and call it a spanning set, not a basis.

### Regulated Lee--Pomeransky rank as a global falsifier

**Verified result.** For a fixed, possibly coefficient-special Laurent polynomial `G`, generic
twist parameters concentrate the relevant twisted cohomology and relate its dimension to the
Euler characteristic and logarithmic critical-point count. Special coefficients can lower that
dimension, so a generic Newton-polytope volume cannot simply be specialized to the equal-mass
family:
[*Vector Spaces of Generalized Euler Integrals*](https://arxiv.org/abs/2208.08967),
arXiv:`2208.08967`. Regulating every Feynman parameter combines top sectors and subsectors in one
global critical system; this is the correct setting in which cross-sector relations can change
the count:
[*Magic Relations and Critical Varieties of Feynman Integrals*](https://arxiv.org/abs/2605.29789),
arXiv:`2605.29789`.

For the equal-unit-mass K6 family, a bounded exact experiment should construct `G` from the
authenticated routing and work over `Q(d,rho_1,...,rho_6)` with the likelihood ideal

```text
2 rho_i G - d x_i partial_i G,   z G - 1.
```

Generic nonzero `rho_i` and `zG-1` avoid a separate coordinate/product saturation. Homogeneity and
`G=U(1+sum x_i)` also give a simplex formulation eliminating one parameter and the highest-degree
`G` inversion equation. The direct and simplex ideals must yield the same exact zero-dimensional
degree. The equal-mass lightlike bubble, whose global rank is one although separate face counting
retains two tadpoles, is the mandatory cross-sector regression.

**RustRed inference.** This degree is an independent whole-family rank/finiteness diagnostic, not
a reducer. It supplies a fatal lower-bound test when a proposed expanded terminal collection has
fewer than the regulated rank and measures possible redundancy when it has more. It does not show
that RustRed's terminals span, provide coefficient projectors, include graph symmetry, or prove
that the nine ordinary K6 momentum-space sources generate the complete parametric relation
space—the exact Mellin-transform relation does not currently settle that generation question
([arXiv:1712.09215](https://arxiv.org/abs/1712.09215)). A finite nonminimal `T` is cheaper than a
projector only because RustRed's own guarded descending owner cover proves span. Constructing a
relative de Rham presentation that maps every boundary terminal would recover most of the hard
projection machinery and is not a Stage 1 dependency.

### Local-ring pivots and epsilon debt

**Verified result.** A sequential `D = 4` projection and local-ring Gaussian elimination can avoid
division by epsilon and construct singularity-free bases for the studied planar and nonplanar
two-loop double boxes:
[*Singularity-Free Feynman Integral Bases*](https://arxiv.org/abs/2508.04394),
arXiv:`2508.04394`.

**RustRed inference.** Local-ring valuations should guide pivot selection and terminal-basis
refinement. A finite example with no spurious pole does not bound an all-rank recurrence. Each rule
edge needs its `D = 4` valuation, and the complete descending rule graph needs a proof of bounded
accumulated negative valuation on every cell and guard stratum. Without such a potential or
transition bound, repeated divisions can create epsilon debt proportional to target rank even when
the integral order strictly descends.

### Five-loop master counts and AMFlow auxiliary blocks

**Verified result.** The fully massive five-loop tadpole classification gives a concrete `K = 15`
scale: 63 sectors with five through eleven lines contain 103 masters, while four twelve-line
sectors add nine masters in that convention. Roughly 300 digits and ten epsilon orders were
obtained for the lower sectors, while the hardest top sectors were then resource limited:
[*Five-loop massive tadpoles*](https://arxiv.org/abs/1609.06786), arXiv:`1609.06786`,
DOI `10.22323/1.260.0074`. Other five-loop basis conventions give nearby but not identical counts;
the relevant conclusion is an intrinsic scale of order one hundred, not one canonical number.

AMFlow's auxiliary systems can be substantially larger than the requested basis. In the original
paper, a 108-master double-pentagon problem grew to 476 auxiliary masters when all propagators were
massive and to 176 in the best single-propagator mode:
[*AMFlow: a Mathematica package for Feynman integrals computation via auxiliary mass flow*](https://arxiv.org/abs/2201.11669),
arXiv:`2201.11669`, DOI `10.1016/j.cpc.2022.108565`.

AMFlow 2.0 reports a benchmark with 316 three-loop five-point masters. Its first auxiliary block
contained 521 masters with auxiliary-mass flow and 150 with Feynman-trick recursion. Producing 20
digits through the finite epsilon term cost about 174 CPU-hours in the best reported setup, of
which about 134 were symbolic and 41 numerical; the auxiliary-mass-flow route cost about 600
CPU-hours. The paper recommends basis refinement for order-one-hundred-digit work:
[*AMFlow 2.0: significant algorithmic and software improvements*](https://arxiv.org/abs/2607.08477),
arXiv:`2607.08477`, with public sources at
[`multiloop-pku/amflow`](https://gitlab.com/multiloop-pku/amflow).

**RustRed inference.** Track three sizes separately:

- `t`, the number of typed terminals retained by the reducer;
- `r`, when computed, the exact independent quotient rank after relations and symmetries; and
- `m`, the largest auxiliary system in the numerical evaluation campaign.

The K15 evidence makes an independent block of roughly one hundred plausible. It does not make
thousands of redundant terminals, or a 20,000-digit K21 campaign, plausible. `m`, not `t`, is the
dominant numerical feasibility measure and can exceed both `t` and `r`. No public complete fully
massive K21 master count or universal reduction was found in this scan.

### Dense-parent extrapolation and sparse controls

**RustRed derivation.** Face-local completion can make complexity follow the dimension of the
currently uncovered inactive/ISP face instead of the complete index count. This is particularly
promising for connected cubic vacuum parents: with `K = L(L+1)/2` scalar-product coordinates and
at most `3(L-1)` propagators, their top-sector inactive dimensions at four, five, and six loops are
only `1`, `3`, and `6`. It is not a topology-generic scaling argument. The corresponding equal-mass
bananas retain `5`, `9`, and `14` inactive directions, while asymmetric three-vertex multitheta
graphs with edge-bundle multiplicities `(1,2,3)`, `(1,2,4)`, and `(1,2,5)` retain `4`, `8`, and
`13`.

The six-loop seven-edge banana makes the symmetry tradeoff concrete. Its loop space is the
standard `S7` representation `V`; its scalar products decompose as

```text
Sym^2(V) = [7] + [6,1] + [5,2].
```

The seven propagator quadrics span the first two summands, leaving the 14-dimensional `[5,2]` ISP
quotient. A Burnside/Molien audit gives 344 invariant degree-ten numerator polynomials on the
fully undotted symmetric stratum instead of 1,144,066 raw monomials, while dot excess ten has 38
`S7` orbits instead of 8,008 raw compositions. These reductions matter in practice, and Symbolica
graph stabilizers should provide their exact transport. A finite group supplies only an eventual
constant factor, however; generic unequal dot patterns destroy the stabilizer and do not change
the polynomial growth degree.

The asymmetric multitheta controls make the opposite structural caution concrete. For bundle
multiplicities `(1,2,3)`, `K = 10`, and four inactive directions, the complete one-sided
degree-one and degree-two face plans have only 80 and 240 rows. For `(1,2,4)`, `K = 15`, and eight
inactive directions, they have 225 and 1,125 rows. These graphs have treewidth two, admit sparse
routings, and their codimension-one daughters factorize. Success is useful evidence that the
generic sparse lane is implemented correctly, but it is not evidence that a dense parent will
scale.

**Verified result.** The equal-mass `L`-loop banana family has `L+1` masters and an all-loop
differential-equation treatment:
[*Bananas of equal mass: any loop, any order in the dimensional regularisation parameter*](https://arxiv.org/abs/2212.08908),
arXiv:`2212.08908`. For generic masses, `L+3` explicit differential operators annihilate the
banana integral; Macaulay calculations through `L=8` give rank `2^(L+1)-1`, while generation of
the complete annihilating ideal remains a conjecture:
[*D-ideal of generic mass banana integrals in dimensional regularization*](https://arxiv.org/abs/2508.04309),
arXiv:`2508.04309`.

**RustRed inference.** The banana is a valuable high-dimensional-face and specialized-lane
control, but its small equal-mass evaluator block and exceptional symmetry make it too friendly as
the only sparse benchmark. Every K10/K15 promotion must also pass an asymmetric multitheta family,
whose distinct bundle sizes remove vertex permutations and whose generic dot patterns retain only
small within-bundle stabilizers. Dense-parent, banana, and multitheta results must be reported
separately.

### Source-aware modular batching, not rank-only Krylov

**RustRed API audit.** Symbolica's public sparse matrix owns its CSR pattern and values; its
forward sparse reduction is serial and fill-retaining, while its parallel solve accelerates only
back substitution. The public tree supplies finite fields, exact polynomials, evaluation,
univariate Newton interpolation, and integer/rational reconstruction pieces, but no turnkey block
Wiedemann, distributed sparse operator, shared-pattern matrix, or multivariate rational-function
reconstructor. RustRed should therefore keep one versioned immutable structural CSR and exact
coefficient-evaluation trace per frame, memory-map it on each node, and share one evaluated value
array only among workers using the same prime and point. The structural plan may be shared, but a
sampled compact CSR that drops modular zeros has sample-dependent offsets and column indices and
must stay sample-local (or retain explicit zeros). Prime gangs cannot share values or elimination
arithmetic. This is necessary RAM engineering, not an algorithmic breakthrough.

Black-box rank is insufficient because exact lift needs deterministic original-source provenance.
A Krylov lane is admissible only after a source-aware modular pilot selects a square minor and a
canonical row/column trace. If selecting that minor already incurs global fill, or its inverse
produces unaffordable dense source multipliers, Krylov is falsified for this pipeline. Every
reconstructed multiplier must still replay through all exact physical columns and every
denominator-zero stratum must close separately. Never reconstruct or ship a global filled `U`.

The more promising batching opportunity is target-role structure. If exact owner partitions prove
that forbidden sets are laminar, or admit a small cover by maximal chains, one
forbidden-before-target rank-profile pass and one selected minor can answer many targets with a
multi-RHS transpose solve. Approximate grouping is unsound. The K15 gate must therefore report the
number of targets, distinct forbidden sets, exact chain-cover size, selected-minor dimensions,
fill, multiplier-support quantiles, and exact replay cost. Require at least a tenfold reduction in
factorizations before retaining this extra architecture. A bounded prototype is attempted only
when the minimum forbidden-set chain-cover width is at most eight, the ninetieth-percentile
target-specific delta is at most eight columns, and measured work and RSS improve by at least
threefold. Crossing sets otherwise retain independent target-local queries. A shared-pattern or
Krylov campaign is
rejected when deterministic hashes change with worker count, trace pivots are unstable at held-out
samples, provenance densifies beyond exact reconstruction, or communication exceeds half of
iteration time. The underlying black-box boundaries are consistent with sparse exact SpMV and
Krylov-certificate work
([arXiv:1004.3719](https://arxiv.org/abs/1004.3719),
[arXiv:1507.01083](https://arxiv.org/abs/1507.01083)); neither result proves RustRed closure.

### Provisional global separator-circuit oracle

**Research candidate, not a project result.** For routing vectors `c_e`, write
`Q = sum_e x_e c_e c_e^T`, `U = det Q`, `h = 1 + sum_e x_e`, and `G = U h`. One exact global lift
of the regulated critical ideal introduces symmetric `Y` and `w`, imposes `QY = I` and `wh = 1`,
and replaces inverse occurrences by `Y,w`. A second lift represents `U` and all derivatives by a
triangular spanning-forest/configuration arithmetic circuit with reverse differentiation. Both
preserve the one global regulated object only if torus localization, multiplicity, regulators,
`h`, all derivative equations, and every separator boundary state are retained. A separator may
choose a compact circuit and elimination order; it may never justify adding or multiplying local
critical-point counts. This distinction is essential because global regulation absorbs magic
relations that a sector-wise sum can miss
([arXiv:2605.29789](https://arxiv.org/abs/2605.29789)).

A scratch three-prime analysis, followed by an independent exact-BigInt subset-rank replay of the
published standard five-loop A15 routing, found no nontrivial direct or two-separation in its four
twelve-line parents. For
`q(A) = rank(C_A) + rank(C_(E\\A)) - rank(C_E)`, an exact subset-DP *provisionally* found recursive
maximum raw overlaps `3,3,2,2`. This suggests small configuration interfaces, not small Gröbner
bases: `q` is a matroid overlap, not the primal treewidth or degree of regularity of the lifted
ideal, and global `h`, regulators, inverse-`Q` variables, reverse derivatives, and message degrees
may still expand to the whole quotient. The numbers remain provisional project evidence until a
Rust replay binds the published routing to RustRed's manifest, computes every rational subset
rank, verifies the optimal trees, proves the direct and circuit `U` and derivative representations
equal in Symbolica, and reproduces them under rerooting and edge permutations. The first algebraic
falsifier is a K6 K4-parent-versus-five-edge-pinch direct/inverse/circuit shootout including a known
magic-relation control; K10 must show an actual reduction in rows, fill, and RSS before any K15
algebra. The inverse lift and circuit lift are independent global oracles, not terminal-span
certificates.

Relaxed minimality changes neither candidate's proof boundary. A finite universal typed set may be
kept without a unique normal form, but the campaign still records `t`, independent rank `r` when
available, value count `e`, largest numerical block `m`, coefficient support, epsilon debt, and
shipped table size. The global rank oracle can falsify `t < r`; it cannot prove `t` spans. The
rewrite certificate must close every unbounded direction, and a target-rank-independent epsilon
valuation bound plus a measured AMFlow pilot must show that the finite nonminimal set is actually
evaluable.

## Architecture delta by component

| component | accepted role | forbidden inference |
| --- | --- | --- |
| Janet-like complement | exact owner of uncovered orthants and prolongation obligations | commutative coverage automatically proves guards or shift-algebra closure |
| signature and source cofactor | stable modular identity and exact replay certificate | agreement of pivots or signatures across primes proves termination |
| sector-local syzygies | compress ordinary sources and avoid gratuitous raised propagators | a chosen syzygy degree or small symbolic neighborhood spans the full source module |
| generating-function descendants | target free directions of an infinite complement with symbolic operator rules | a finite generic rule list proves exceptional-wall or subsector coverage |
| seedless/simplex-face and tube systems | compact, parallel candidate-rule discovery | a closing finite collection of tested paths proves all-rank closure |
| CALICO annihilators | physical-stratum source enrichment | a chosen degree/order or numerical filter proves a complete source module |
| block-Krylov, Scalar-FGLM, or Hermite recovery | compress a previously certified finite quotient | stable modular action matrices prove a finite quotient |
| guard-wide terminal relation audit | prove exact identities inside a finite spanning set | enough relations for an upper rank bound prove the retained terminals independent |
| exact dual pairing or global physical-rank theorem | supply the lower half of an optional independence certificate | maximal-cut or stable finite-frame rank automatically certifies all sectors |
| local-ring elimination | avoid bad epsilon pivots and refine a basis | finite-system regularity proves uniform recurrence pole depth |
| AMFlow | evaluate a certified independent terminal quotient offline | terminal count alone predicts auxiliary-system cost |

## Exact implementation sequence

1. Generate ordinary physical-family IBPs and selected low-order Schwinger or
   Lee--Pomeransky annihilators. Build sector-local no-raised-propagator syzygies where they reduce
   the source volume. Preserve an exact expansion into ordinary or annihilator sources.
2. Build a Janet-like leading-shift complement for each sector chart and exact guard stratum.
   Positive-dimensional standard pairs are queued obligations, never implicit terminals.
3. Attack each obligation with generating-function descendants directed along its standard-pair
   free variables, then bounded simplex-face or sparse-tube searches when that is cheaper. Modular
   workers share one immutable symbolic trace and return signature-keyed coefficients and source
   cofactors.
4. Lift candidates exactly and replay them over the rational coefficient field before admitting
   their leading classes into the complement.
5. Once every remaining complement component is finite, either retain and evaluate the complete
   affordable typed terminal set, or—only when compression is useful—construct its exact relation
   module across the whole guard stratum before recovering shift actions with block-Krylov, FGLM,
   or Hermite methods. Call the compressed set independent only after a separate exact lower-rank
   certificate closes the rank sandwich.
6. Verify source annihilation, guard ownership, lower-sector maps, symmetry routes, strict descent,
   overlap normal forms, and all required discrete shift-algebra critical pairs.
7. Prove a uniform epsilon-valuation bound and derive the Laurent depth required of every terminal.
8. Partition the independent terminal quotient into numerical blocks and measure the largest
   AMFlow auxiliary dimension before commissioning high-precision data.

Discovery output remains untrusted until step 6 completes. Exact replay belongs at the artifact
admission boundary; it is not repeated in the reducer hot path.

## Falsification gates

The numerical thresholds below are engineering gates, not literature theorems.

### `K = 6`

- Replay every rule from regenerated exact sources on every guard stratum.
- Eliminate every positive-dimensional standard pair and explicitly enumerate any resulting
  zero-dimensional finite residue as terminals.
- Verify strict descent, overlap normal forms, shift-algebra critical pairs, and immutable
  lower-sector boundaries.
- Prove a target-rank-independent epsilon-valuation bound; sampled ray reductions are insufficient.

### `K = 10`

- Require exact finite-complement results for a nine-propagator dense parent, the five-edge
  banana, and the `(1,2,3)` multitheta. Dense-parent or banana success alone cannot promote the
  architecture.
- Require stable candidate rank and support across at least three good primes and held-out rational
  `D` points, followed by exact lift and replay.
- Run at least two larger outer-frame controls. They are falsification tests only; exact complement
  and prolongation exhaustion remain the proof.
- The `(1,2,3)` multitheta structure probe is bounded to descendant/face degree two, with at most
  one degree-three exception on a single unresolved standard pair. Stop that lane above two GiB or
  one wall-clock hour, above 20-times input `L+U` fill for promotion (50-times is an immediate
  kill), above twice the pair-directed tube count after deduplication, or above 16 exact guard
  strata. A dense-parent result is still mandatory.
- Reject an implementation whose normal worker exceeds roughly 32 GiB before K15, unless a clear
  chunked representation removes the replicated state.

### `K = 15`

This is the mandatory design gate before K21.

- Require exact finite complements for a twelve-propagator dense parent, the six-edge banana, and
  the `(1,2,4)` multitheta; the sparse multitheta generic lane may not escalate beyond face level
  four without rejecting or redesigning the architecture.
- Its structure-only multitheta probe stops above 16 GiB or four wall-clock hours, above 20-times
  input fill for promotion (50-times is an immediate kill), above twice the pair-directed tube
  count after deduplication, or above 64 exact guard strata. Repeated negative epsilon valuation
  without a bounded transition potential is also a kill, not a request for a wider frame.
- When an independent quotient is computed, compare its rank with the known fully massive scale
  of roughly one hundred and explain large discrepancies by exact maps or physical-stratum
  relations. Otherwise the complete typed set `t` must itself pass the numerical budget.
- If `t > 1,000`, or if a known `t / r > 10`, require exact terminal compression before numerical
  evaluation.
- Require the largest measured evaluator block `m <= 300`, preferably split into certified weakly
  coupled blocks of order one hundred or less.
- Complete a 50–100-digit pilot with measured Laurent depth, conditioning, time, memory, and
  restart behavior before proposing a 20,000-digit campaign.

### `K = 21`

- Admit no finite frame until every Janet/Ore prolongation and guard branch has an exact owner and
  all critical paths replay; repeated stable borders alone are not a proof.
- Require an independent rank diagnostic before publishing a compressed quotient/action
  presentation. A direct nonminimal rewrite presentation may instead evaluate all typed
  terminals, provided `t` and every numerical block pass the declared affordability gates.
- Require epsilon debt bounded independently of target rank.
- Require `m <= 300` or a certified split into weakly coupled blocks near one hundred.
- Stop if the number of tube charts or faces grows explosively, shared symbolic state approaches a
  terabyte, modular support changes at held-out primes or `D` points, or nonminimal terminals remain
  more than ten times the independent rank without exact compression.

Passing K6 demonstrates correctness of the certificate machinery. Passing K10 demonstrates that
its sparse implementation scales beyond the development family. Only K15 can authorize a genuine
K21 campaign.
