# Parametric-IBP closure literature frontier through August 2026

## Scope and claim discipline

This note records an independent review of algorithms that may turn IBP identities into exact,
closing, all-rank parametric rules. It distinguishes claims made in primary sources from RustRed
design hypotheses. In particular, none of the cited papers establishes that a complete,
practically sized rule artifact can be constructed for every six-loop vacuum family.

Two similar labels are kept distinct throughout:

- `K = 6` is RustRed's current complete three-loop scalar-product family;
- `L = 6` is the eventual six-loop target, with `K = L(L + 1) / 2 = 21` scalar-product
  coordinates.

For a trivalent six-loop vacuum parent, 15 of those coordinates can be propagators and six are
irreducible scalar products. The algorithms must nevertheless be generic in topology, loop count,
and the split between propagators and auxiliary coordinates.

The word *closure* has a strict meaning here. Every lattice point in every supported sector and
guard domain must have at least one complete owner selected by deterministic precedence:

1. an exact, strictly descending rule;
2. a proved zero or factorization result;
3. an immutable lower-sector artifact; or
4. an explicit evaluation terminal.

Rule domains may overlap. All applicable owners must give the same exact normal form on overlaps;
unique mathematical applicability is neither assumed nor required.

A finite box of successful tests is not closure. Neither a conventional master count nor failure
to find another rule turns an uncovered integral into a master.

## Executive verdict

No primary source reviewed through August 2026 proves practical all-rank six-loop closure. The
strongest implementable programme is a hybrid:

1. use exact standard-pair and guard coverage as the closure authority;
2. use targeted triangular or tube searches to close the six presently exposed `K = 6` rays;
3. develop signature-filtered Janet completion in a sector-local Ore algebra as the main
   systematic scaling bet;
4. use critical-point, Landau, and Fitting-ideal syzygies to compress the source set, never as a
   standalone closure proof; and
5. stop at a finite, deliberately nonminimal terminal set when exact coverage permits it.

The immediate `K = 6` recommendation is therefore targeted triangular/tube discovery plus the
nonminimal finite-terminal lane. For `L = 6`, signature/Janet completion is the best conservative
research hypothesis. Generating-function border or Pfaffian completion is the highest-upside
alternative, but it needs an exact source-membership and rank certificate before it can own a
production rule.

## Current falsification target

The current `K = 6` campaign exposes three scalar-corner obligations:

```text
[0, 1, 1, 1, 1, 0]
[0, 1, 1, 1, 1, 1]
[1, 1, 1, 1, 1, 1]
```

It also exposes the following representative recurrence points:

```text
[0, -1, 1, 2, 2, 1]
[0, -2, 2, 2, 1, 1]
[0, 1, 1, 2, 4, 0]
[0, 1, 1, 2, 5, 0]
[0, 1, 2, 3, 3, 0]
[0, 1, 3, 2, 3, 0]
```

The six points are samples of positive-dimensional strata. They must not be treated as six
isolated misses. A candidate method succeeds only if it owns the whole corresponding recurrence
strata, including every coefficient-zero and sector-boundary branch.

For scale, a signed `L1` translation diamond in `K` dimensions contains

```text
D_K(h) = sum(j = 0..min(K, h), 2^j C(K, j) C(h, j)).
```

At `L = 6`, ordinary momentum-space IBPs provide `L^2 = 36` base rows. Complete translation
diamonds would therefore generate:

| radius | `D_21(h)` | ordinary shifted rows |
| ---: | ---: | ---: |
| 2 | 925 | 33,300 |
| 3 | 13,287 | 478,332 |
| 4 | 143,529 | 5,167,044 |

Those figures exclude symbolic coefficient growth and elimination fill. Full diamonds are useful
controls, not an acceptable default discovery strategy.

## Findings from primary sources

### Lee's symbolic-rule programme

Lee's group-structure paper
([arXiv:0804.3008](https://arxiv.org/abs/0804.3008)) organizes IBP relations through
raising, lowering, and counting operators and explains dependencies between identities at
neighboring lattice points. The LiteRed papers
([arXiv:1212.2685](https://arxiv.org/abs/1212.2685) and
[arXiv:1310.1145](https://arxiv.org/abs/1310.1145)) document sectorwise symbolic rules,
symmetry detection, boundary handling, and their implementation in LiteRed.

The transferable principle is stratified symbolic solving: derive a generic-index relation,
orient a translated leading integral, determine where its coefficient is nonzero, and revisit
exceptional faces with other sources. The papers do not give a fixed translation-depth bound or a
six-loop complexity theorem. RustRed should reproduce the semantics, not LiteRed's Mathematica
object model, global mutable state, or heuristic search chronology.

### Syzygy-constrained and seedless lowering

The syzygy-constrained symbolic-rule method
([arXiv:2507.11140](https://arxiv.org/abs/2507.11140)) constructs sectorwise
IBP-generating vectors designed not to raise active propagator powers, then derives generic-index
rules with small partly symbolic neighborhoods. *Seedless Reduction of Feynman Integrals*
([arXiv:2602.22111](https://arxiv.org/abs/2602.22111)) constructs generic-index lowering
operators through bulk, face, edge, and dotted boundary problems rather than beginning with a
large seeded Laporta system.

These works support source preconditioning and boundary-first decomposition. Their examples do
not establish a universal finite degree, level, or source count at six loops. In RustRed they are
candidate source generators for an independent coverage engine, not substitutes for that engine.

### Generating-function completion

The generating-function algorithm
([arXiv:2605.09541](https://arxiv.org/abs/2605.09541)) maps sector IBPs to differential
equations in a Weyl algebra. It iteratively extracts rules, reduces the equation set, generates
guided descendants, and describes completeness by the lattice complement of the upward orthants
owned by rule leaders.

This is the clearest published match to RustRed's desired structural coverage model. It treats
dots and inactive numerators in one operator language. The paper does not supply a demonstrated
six-loop implementation, and structural orthants alone do not resolve polynomial coefficient-zero
loci. RustRed must also prove that every retained differential relation is in the original IBP
module and that the resulting discrete rule is strictly descending on its declared domain.

### Triangular, tube, and intermediate-basis methods

*Untangling the IBP Equations*
([arXiv:2512.05923](https://arxiv.org/abs/2512.05923)) uses target-versus-forbidden
rank tests on shifted equations to discover diagonal, block, or triangular recurrences. Shifting
the equations can increase their distance from the target while improving triangularity. The
published construction is an empirical search method; it does not prove a topology-independent
search-depth bound.

*Efficient AI-Inspired Reduction of Feynman Integrals via Tube Seeding*
([arXiv:2606.10698](https://arxiv.org/abs/2606.10698)) translates a compact seed set along
a thin path in index space. The reported examples contrast high-degree growth from conventional
rank seeding with much milder growth along a tube, including a difficult two-loop example through
rank 40. This is strong evidence for thin target-directed searches, not an all-rank fixed-width
theorem.

*Taming Symbolic IBP Reduction with Intermediate Bases*
([arXiv:2606.22500](https://arxiv.org/abs/2606.22500)) composes reductions through small
intermediate target sets and sparse, low-degree matrices. Its examples report reconstruction
sample counts of 3,289 versus 1,407,406 and 13,013 versus 21,638,331 terms in corresponding direct
ansatzes. This is a coefficient-representation and reconstruction accelerator. It does not by
itself discover or certify an all-rank closing rule set.

### Critical loci, Landau geometry, and magic relations

The following papers connect IBP syzygies, reduction rank, or source construction to critical and
Landau geometry:

- [Critical Points and Syzygies, arXiv:2509.17681](https://arxiv.org/abs/2509.17681);
- *Feynman Integral Reduction and Landau Singularities*
  ([arXiv:2512.05869](https://arxiv.org/abs/2512.05869)); and
- *Compact Syzygies from Landau Singularities*
  ([arXiv:2607.06365](https://arxiv.org/abs/2607.06365)).

They develop, in different settings, critical-ideal prelocalization, Fitting-ideal or
determinantal certificates, and compact syzygy construction. Their exact conclusions depend on
stated algebraic hypotheses such as isolatedness, saturation, or radicality, and their practical
demonstrations are concentrated at low loop order. The defensible RustRed use is as a source
preconditioner and a sector diagnostic.

*Magic Relations and Critical Varieties of Feynman Integrals*
([arXiv:2605.29789](https://arxiv.org/abs/2605.29789)) relates higher-dimensional critical
components to relations that can be invisible in a single sector or cut. This warns against
declaring a sector complete from spanning-cut data alone. Supersector relations must be admitted
when the critical geometry indicates missing directions.

*Landau's Leviathans*
([arXiv:2606.29612](https://arxiv.org/abs/2606.29612)) uses finite-field elimination to
find Euler-characteristic drops and reports calculations through the fully massive three-loop
envelope. Its diagnostic tests can identify sectors where naive critical-point assumptions fail.
It estimates quotient dimensions; it does not emit a descending rewrite system.

### Symmetry and graph structure

*Discrete Symmetries of Feynman Integrals*
([arXiv:2604.08332](https://arxiv.org/abs/2604.08332)) proves, at generic kinematics under
its assumptions, a correspondence between affine sector symmetries and permutations of the
Lee--Pomeransky polynomial, with a constructive graph lift. This supports exact orbit quotienting
and transport of rules between authenticated decorated sectors.

The theorem does not license discarding all rows in one source-row orbit. For a fixed target, only
its stabilizer acts on the same reduction problem, and distinct images may remain linearly
independent. Masses, routing, auxiliary scalar products, ordering, and guard domains belong in the
symmetry key. Graph treewidth is also not matrix-elimination treewidth.

### Intersection theory, covariant differentiation, and annihilators

Recent intersection-theory constructions include a branch-recursive sparse formulation
([arXiv:2604.05025](https://arxiv.org/abs/2604.05025)) and geometric-ordered bases with
simpler intersection matrices
([arXiv:2608.03646](https://arxiv.org/abs/2608.03646)). They can provide basis changes,
master counts, or targeted reductions. They do not currently provide a global, integer-index,
strictly descending rule artifact for all numerator ranks.

Covariant differentiation
([arXiv:2604.09810](https://arxiv.org/abs/2604.09810)) handles arbitrary dot powers by a
mass connection in its setting. That does not establish arbitrary-ISP-numerator closure for a
generic vacuum family.

The parametric-annihilator construction
([arXiv:1712.09215](https://arxiv.org/abs/1712.09215)) maps annihilating differential
operators to shift relations through a Mellin transform. It supplies a formal bridge from
parametric `D`-modules to IBP recurrences and relates holonomic rank to Euler characteristics. It
does not remove the computational problem of finding a usable annihilator basis and orienting it
into sparse guarded rules.

### Master-count diagnostics

The Lee--Pomeransky critical-point result
([arXiv:1308.6676](https://arxiv.org/abs/1308.6676)) relates the number of master integrals
to proper critical points of the parametric polynomial, under its stated conditions. The
annihilator work above gives an Euler-characteristic formulation. These are independent quotient
dimension diagnostics when all hypotheses, sectors, and resonance issues are handled.

A predicted dimension `r` is not a rewrite proof. A sampled census can miss an infinite ray while
still displaying exactly `r` survivors. Conversely, a finite rewrite complement may contain
`t > r` evaluation terminals and still be operationally preferable to the work required to expose
all `t - r` relations.

## Relevant computational-algebra and HPC results

### Completion machinery

Several general algorithms supply ingredients rather than drop-in IBP solvers:

- F4 batches polynomial critical-pair work into sparse linear algebra
  ([DOI:10.1016/S0022-4049(99)00005-5](https://doi.org/10.1016/S0022-4049%2899%2900005-5)).
- Signature criteria have been extended to solvable polynomial algebras
  ([DOI:10.1145/2442829.2442879](https://doi.org/10.1145/2442829.2442879)).
- A tropical F5 algorithm has been formulated for Weyl algebras
  ([arXiv:2312.14419](https://arxiv.org/abs/2312.14419)).
- Janet completion is available for linear difference systems
  ([arXiv:1206.3463](https://arxiv.org/abs/1206.3463)).
- Modular algorithms exist for noncommutative Groebner bases
  ([arXiv:1704.02852](https://arxiv.org/abs/1704.02852)).
- Rational Weyl border bases have been developed
  ([arXiv:2510.23411](https://arxiv.org/abs/2510.23411)).
- Standard pairs give finite descriptions of monomial-ideal complements
  ([arXiv:2005.10968](https://arxiv.org/abs/2005.10968)).

The published termination and correctness results apply to their specified algebras and orders.
RustRed's coefficient field, positive and negative index shifts, sector localization, and guard
stratification require a separate proof. In particular, a generic noncommutative Groebner engine
over a double-shift algebra is unlikely to be a practical first implementation.

### Sparse exact linear algebra and reconstruction

Wiedemann's algorithm computes sparse finite-field linear information with low memory
([DOI:10.1109/TIT.1986.1057137](https://doi.org/10.1109/TIT.1986.1057137)). Block
Wiedemann permits parallel Krylov streams and has a detailed complexity analysis
([Kaltofen's analysis](https://kaltofen.math.ncsu.edu/bibliography/95/Ka95_mathcomp.pdf)).
These methods are valuable for rank, nullspace, and obstruction tests. A dense reconstructed
nullspace vector is not automatically a sparse symbolic rule.

Fast elimination for low-treewidth matrices is available
([ESA 2025](https://doi.org/10.4230/LIPIcs.ESA.2025.116)). The relevant graph is the
row-column incidence graph after the chosen source and monomial representation, not the Feynman
graph. RustRed must measure that treewidth and fill; topology treewidth is only a possible feature.

Balanced reconstruction and supercomputer co-design
([arXiv:2409.19099](https://arxiv.org/abs/2409.19099)), FiniteFlow
([arXiv:1905.08019](https://arxiv.org/abs/1905.08019)), and intermediate bases support a
modular-first architecture. They justify reconstructing only a selected support and factoring
known denominator structures. They do not justify accepting a modular relation without exact
source replay.

Finding a globally sparsest relation should not be a core primitive. Minimum-weight codeword
problems are NP-hard
([DOI:10.1109/18.641542](https://doi.org/10.1109/18.641542)), and sparsest null-vector
selection contains closely related instances. Deterministic pivot circuits, greedy deletion under
a rank oracle, or bounded beam searches are suitable heuristics; global minimality should not be
claimed.

### Learning-guided search

Recent work explores reinforcement learning, explainable optimization, and learned integral
ordering or unscrambling:

- [arXiv:2504.16045](https://arxiv.org/abs/2504.16045);
- [arXiv:2502.09544](https://arxiv.org/abs/2502.09544); and
- [arXiv:2604.05034](https://arxiv.org/abs/2604.05034).

Such models can rank paths, seeds, orderings, or likely source supports. Their output must remain a
search heuristic. Every accepted rule needs the same deterministic exact certificate as a rule
found without learning.

## Common exact architecture

All five hypotheses below should share one proof boundary and one data model.

### Sector-local coordinates and order

For each sector, define nonnegative local coordinates:

```text
active denominator: x_i = n_i - 1,  n_i >= 1
inactive numerator:  x_i = -n_i,     n_i <= 0
```

Persist one translation-compatible well-founded order. A practical order is a tuple of sector
complexity, weighted local degree, numerator degree, dot degree, and a deterministic lexicographic
tie-break. A rule may own a stratum only when every same-sector right-hand term is strictly lower
for all points in the declared domain. Lower-sector terms are reduced by immutable child artifacts.

### Minimal data structures

The implementation needs concepts equivalent to:

- `SectorKey`: active mask plus exact family and decorated-symmetry identity;
- `Stratum`: fixed coordinates, free coordinates, sector bounds, and guard atoms;
- `OreTerm`: shift vector with a polynomial in number operators and dimension;
- `OperatorRow`: ordered terms plus an ordinary-source provenance DAG;
- `LeaderAntichain`: minimal leading shifts under divisibility;
- `StandardPair`: a finite base point and its free-coordinate set;
- `GuardOwner`: proof of generic nonvanishing or complete zero-branch ownership;
- `RuleOwner`: exact rule, zero, factorization, lower sector, or terminal; and
- `SourceCertificate`: exact coefficients reproducing a rule from regenerated IBPs.

Coefficient expressions should remain Symbolica polynomials or rational functions at the exact
boundary. RustRed-specific wrappers should represent domain, order, provenance, and serialization
semantics, not reimplement a computer-algebra system.

### Completion criterion

For every sector and every guard stratum:

1. all generated completion obligations reduce to zero or add a new rule;
2. every structural standard pair is owned by a rule or an explicit terminal mechanism;
3. every pivot-coefficient zero locus is proved absent or recursively covered;
4. all boundary and lower-sector terms normalize through immutable owners;
5. all rule overlaps have the same exact normal form; and
6. every rule replays from freshly generated ordinary sources.

Only this certificate, or a mathematically equivalent one, establishes closure.

## Hypothesis 1: signature-filtered Janet completion in an Ore algebra

### Proposed algorithm

Represent a sector operator by

```text
(signature, sector, shift vector, coefficient in Q(d, n), provenance).
```

Use raising shifts `E_i` and counting operators `n_j` with the Ore relation

```text
E_i n_j = (n_j + delta_ij) E_i.
```

Maintain a leading-shift antichain and Janet multiplicative-coordinate bitsets. Queue only
nonmultiplicative prolongations. Before reduction, apply signature rewrite and syzygy criteria;
batch survivors of one degree in an F4-style sparse modular elimination. Lift only selected pivots
and source combinations to exact Symbolica expressions. Split coefficient-zero branches into
separate guarded strata rather than silently dividing.

### Why it may be systematic and efficient

Janet completion turns an open-ended neighborhood search into explicit prolongation obligations.
Signatures can eliminate work whose source ancestry is already represented, while standard pairs
measure the exact unowned complement. Modular batching avoids exact expression swell until the
leading support has stabilized.

### Risks

- The full positive/negative double-shift algebra need not inherit the convenient Noetherian
  behavior of the cited Janet or Weyl settings.
- Guard-zero hypersurfaces can generate many algebraic strata.
- A mathematically finite basis can still be far too large.
- A generic noncommutative CAS would obscure sector and descent invariants and create avoidable
  coefficient swell.

### Falsification tests

For `K = 6`, remove each known rule in turn and require completion to rediscover an equivalent
owner from its exact source module. Then ask it to close all six exposed recurrence strata and all
guard branches without increasing the terminal dimension spuriously.

For `L = 6`, compare degree-two and degree-three completion against full-diamond controls of
33,300 and 478,332 ordinary rows. Reject the implementation direction if it approaches the control
within a factor of two in retained rows or memory, if leading support changes under held-out
primes, or if the standard-pair dimension does not decrease monotonically.

## Hypothesis 2: standard-pair-guided triangular tube compiler

### Proposed algorithm

For each uncovered standard pair:

1. choose a deterministic path from an already reducible base to a generic point on the stratum;
2. translate a compact authenticated source set along a tube around that path;
3. partition columns into target, strictly allowed, and forbidden sets;
4. test modularly whether

   ```text
   rank([E_forbidden | e_target]) > rank(E_forbidden);
   ```

5. select a deterministic fundamental circuit or greedy rank-preserving row subset;
6. lift that fixed support as a symbolic rule in the stratum's free variables; and
7. send every pivot-zero branch and boundary face back to the coverage queue.

The tube widens only in response to a typed obstruction. Several primes and generic samples select
support; held-out samples test it; exact source replay is the sole acceptance condition.

### Why it may be systematic and efficient

Tube seeding attacks the requested recurrence rather than every nearby integral. Triangular rank
separation directly encodes the desired right-hand-side order. It becomes systematic only because
the exact standard-pair queue enumerates every remaining infinite stratum; the tube heuristic
alone has no completeness theorem.

### Risks

- Required tube width may grow with rank.
- Individually neutral rows may be jointly necessary, so greedy immediate-gain pruning can fail.
- A modular rank jump can be caused by an unlucky prime or singular sample.
- Globally sparsest relation search is computationally intractable in general.

### Falsification tests

For `K = 6`, run tube widths zero through three on all six recurrence strata, with at least three
discovery primes and held-out points. Require one exact whole-stratum rule, or a finite guarded
partition, for each ray. Compare retained rows, fill, and wall time with radius-two through
radius-four diamonds.

For `L = 6`, test ranks `1, 2, 4, 8, 12, 20` on three paths: one-axis growth, a two-axis zigzag,
and mixed dot/numerator growth. Use both a six-loop banana control and a genuine 15-propagator,
six-ISP vacuum family. Reject fixed-width tubes if required width grows with rank or if accumulated
lower-sector tubes grow superlinearly in the number of visited strata.

## Hypothesis 3: generating-function border or Pfaffian completion

### Proposed algorithm

Convert sector IBPs to generating-function PDEs. Maintain a finite derivative order ideal `O` and
seek one relation for every first-border derivative. Reduce each relation to `O`, producing
connection matrices `M_i`. Require the exact flatness identities

```text
[M_i, M_j] = partial_i M_j - partial_j M_i.
```

Every border relation must replay from ordinary sources. An independent quotient-rank lower bound
must match `|O|`; otherwise flat matrices could represent an accidental ideal larger than the IBP
ideal. Finally translate the differential normal form to a strictly descending discrete rule set
with complete guard stratification.

### Why it may be systematic and efficient

A stable finite order ideal replaces unbounded lattice sampling by a border. Flat connection
matrices supply compact integrability checks. The method can expose dots and numerators uniformly
and may yield a small Pfaffian representation even when pointwise recurrences are awkward.

### Risks

- Discovering `O` and its border relations can be as hard as a Groebner basis.
- Flatness proves consistency of the proposed connection, not membership in the original IBP
  module.
- Resonant dimensions and coefficient-zero index loci need separate treatment.
- Differential simplicity may not translate into a useful descending index order.

### Falsification tests

For `K = 6`, first reproduce the sunset family, then an irreducible four-line sector, and finally
the full family. Require stable `O`, exact source membership, flatness, independent rank agreement,
and successful translation to guarded descending rules.

For `L = 6`, perform the same ladder on the banana control before the trivalent family. Terminate
the pilot if `O` does not stabilize under increasing derivative degree, if source degree grows
faster than the diamond control, or if the independent quotient rank disagrees with `|O|`.

## Hypothesis 4: Landau/Fitting source compression with magic repair

### Proposed algorithm

For each exact decorated-sector orbit:

1. form the relevant critical ideal;
2. test isolatedness, saturation, and other hypotheses required by the chosen theorem;
3. compute modular dimension and component diagnostics;
4. derive Fitting-ideal or determinantal syzygy candidates;
5. when positive-dimensional components remain, search neighboring supersectors for magic
   relations; and
6. feed the resulting compact source set to Hypothesis 1 or 2.

Symmetry may transport a certified source template, but every transported identity must replay in
the target family and routing.

### Why it may be systematic and efficient

The geometry can explain which source classes are missing and avoid enormous blind polynomial
ansatzes. Exact decorated-sector orbiting can amortize the diagnostic across many vacuum sectors.
This hypothesis compresses the source module; it does not define closure on its own.

### Risks

- Equal-mass vacuum kinematics can be more singular than generic kinematics.
- Saturation and component decomposition can dominate the reduction itself.
- Cut-local syzygies can miss supersector or magic relations.
- A quotient-dimension result does not orient any relation into a descending rule.

### Falsification tests

For `K = 6`, classify every sector orbit, correlate the six uncovered rays with the detected
components, and compare closure cost with and without the proposed sources. Keep the lane only if
it reduces exact source count or exposes a source class that the ordinary translated search missed.

For `L = 6`, run modular diagnostics before exact decomposition. Reject this as the default lane if
many sectors violate the required hypotheses, if the number of exceptional components explodes,
or if exact source construction costs more than the downstream solve it is meant to simplify.

## Hypothesis 5: closure-first finite nonminimal terminals

### Proposed algorithm

Continue exact completion until the leading-rule complement is provably finite. Do not require that
every point of that finite complement be reduced to a minimal conventional master basis. Persist
each remaining point as a typed evaluation terminal with:

- its exact integral key and symmetry owner;
- its dimensional and mass homogeneity;
- an optional exact basis map;
- numerical value, precision, and provenance metadata; and
- the coverage certificate proving that no free lattice direction remains.

The structural test is zero-dimensionality of the leading monomial ideal after sector and guard
splits, equivalently a standard-pair list with no free coordinate. A bounded sample of misses is
not that test.

### Why it may be systematic and efficient

Minimality and closure are different optimization problems. A finite redundant terminal set can
avoid expensive relations whose only effect is to shrink the final evaluation basis. It is often
cheaper to evaluate several additional terminals once than to enlarge and reconstruct a global
symbolic reduction.

### Risks

- The redundant terminal count may grow exponentially.
- Numerical evaluation and precision management may dominate the saved symbolic work.
- Near-dependent terminal bases can be poorly conditioned.
- A terminal list without a zero-dimensional coverage certificate can conceal an infinite ray.

### Falsification tests

For `K = 6`, stop at the first finite complement, record its size `t`, and compare it with an
independent master count `r` and with the existing MATAD basis. Measure the source, time, and fill
needed to force `t` down to `r`. Prefer the larger basis if its one-off mapping or evaluation is
cheaper and stable.

For `L = 6`, extrapolate `t / r` sector by sector and estimate simultaneous AMFlow cost, required
precision, storage, and conditioning. Reject this lane if redundancy grows exponentially or if
terminal evaluation becomes the dominant campaign cost.

## Finite complement versus master minimality

### Exact diagnostic

Suppose the completed leading shifts generate a monomial ideal `J` in a sector's nonnegative local
coordinates. The unreduced structural lattice is finite precisely when the standard monomials of
`J` are finite, or equivalently when every standard pair has no free coordinate. This conclusion
must be repeated on every guard branch. One standard pair with a free direction proves an infinite
uncovered family even when millions of sampled points reduce.

An independent critical-point, Euler-characteristic, or intersection-theory computation can give
a quotient dimension `r`. If the certified finite terminal set has size `t`, then `t >= r` measures
potential redundancy. Equality is useful corroboration but is not, by itself, evidence that every
rewrite rule is valid or every guard branch is covered.

### Exact and numerical basis mapping

For `K = 6`, an exact production map to Vakint's existing MATAD master basis is useful but optional.
If such a map is constructed, every identity in it must be reproduced from exact RustRed sources.
A certified finite nonminimal RustRed terminal basis may instead be assigned separately generated
Laurent values at roughly 20,000-digit precision: from MATAD through three loops and from FMFT
through four loops. Vakint may ship those versioned numerical terminal tables as evaluation data.

Modular or high-precision numerical samples may identify a likely mapping support and reconstruct
rational coefficients. A reconstructed exact map is accepted only after exact Symbolica source
replay and guard validation. When bases differ, high-precision Laurent-series agreement may be the
sole cross-backend parity gate. It validates evaluated output, not RustRed source membership or
closure; those remain exact obligations.

At high loop order, a nonminimal finite terminal set could be evaluated directly by AMFlow. That
choice is practical only if terminal count, simultaneous evaluation, precision loss, and artifact
storage remain controlled. It changes the evaluation basis, not the proof obligation for closure.

### What recent scalable methods optimize

The reviewed methods optimize different costs:

- generating functions naturally target a finite derivative complement;
- seedless and triangular methods target chosen lowering relations;
- tube seeding targets bounded paths or numerator ranks;
- intermediate bases target coefficient reconstruction and expression size;
- critical/Landau methods target source construction or quotient diagnostics.

None of these primary sources proves that a minimal master basis is always the cheapest closing
artifact. RustRed should report closure size and evaluation cost separately from algebraic master
minimality.

## MATAD and Vakint as an offline `K = 6` oracle

MATAD and Vakint can be used without contaminating the production path:

1. query the three scalar corners and points at depths `20` through `50` along all six open rays;
2. compare raw master-coefficient vectors when both backends use a common basis, or compare
   high-precision Laurent series when they do not;
3. infer likely recurrence order, lowering direction, exceptional factors, and terminal-vector
   rank;
4. use these observations to prioritize RustRed source searches; and
5. accept a rule only after independent exact RustRed generation and Symbolica replay.

FORM is allowed only inside this offline diagnostic oracle. No FORM-generated recurrence equation
or reduction rule enters a RustRed artifact merely because it agrees numerically. Separately
generated high-precision MATAD or FMFT terminal values may enter Vakint as versioned numerical
evaluation tables. They are data attached to already certified RustRed terminals, not IBP rules.
The production Vakint RustRed scalar backend must not invoke, require, or fall back to FORM. Oracle
comparison should be repeatable with an invalid FORM path after checked-in artifacts and terminal
tables have been generated.

Raw coefficient comparison is stronger when a common master basis exists: master identities or
truncated expansions can hide a wrong reduction. When bases intentionally differ, sufficiently
deep, high-precision Laurent parity may be the sole cross-backend output comparison. It never
replaces exact regenerated-source replay, guard coverage, descent, or completion inside RustRed.

## Sparse and parallel implementation strategy

The expensive search should be modular and distributed; the exact proof boundary should remain
small and centralized.

### Worker data flow

- Share immutable source templates, exact symmetry maps, monomial dictionaries, and prime tables.
- Distribute probes by `(sector, stratum, prime, sample)` or independent block-Wiedemann stream.
- Return sparse support, ranks, pivot chronology, and source provenance, not cloned CAS states.
- Require support stability across discovery primes and genuinely held-out samples.
- Reconstruct only the fixed selected support.
- Perform the final exact Symbolica lift, guard factorization, descent proof, and replay centrally.

This avoids duplicating large expression arenas per worker and keeps inter-worker traffic close to
sparse integer indices and finite-field values. Deterministic task keys and reductions are needed
so worker count cannot change the artifact.

### Measured accelerators

- Use block Wiedemann for large sparse rank and nullspace obstruction tests.
- Use sparse direct elimination when the measured row-column incidence treewidth and fill predict
  that it is cheaper.
- Use F4-style degree batches in the completion lane.
- Use intermediate bases and balanced rational reconstruction after support is fixed.
- Use graph automorphisms to quotient exact decorated tasks, not to delete potentially independent
  row images.
- Use learning only to rank candidate tubes, sources, or orders; record its proposals and exact
  outcomes so it can be disabled without changing correctness.

Every benchmark must record raw nonzeros, peak fill, peak resident memory, retained source rows,
exact-lift size, number of guard strata, and bytes transferred between workers. Row count alone is
not an adequate scaling metric.

## Discriminating experiment ladder

### Gate A: current `K = 6` closure

Run all five hypotheses against the same immutable source generator and ordering.

Acceptance requires:

- the three scalar corners are proved terminals or reduced;
- every one of the six exposed recurrence strata is wholly owned;
- all guard-zero and boundary branches are owned;
- regenerated-source replay succeeds exactly;
- rule overlaps have identical exact normal forms; and
- closure and artifacts are deterministic across worker counts.

The first production attempt should combine Hypotheses 2 and 5. Hypothesis 1 runs in parallel as a
leave-one-rule-out completeness test. Hypotheses 3 and 4 remain bounded pilots until they beat this
baseline on retained rows or expose a genuinely missing source class.

### Gate B: six-loop controls

Before a genuine trivalent `L = 6` campaign, use a six-loop banana family to isolate numerator-rank
scaling from sector-count scaling. Test fixed paths through rank 20 and compare against full
diamonds at radii two and three.

Proceed only when at least one method demonstrates:

- sub-diamond retained-row and memory growth;
- stable modular support under new primes and samples;
- exact lift whose expression size remains below the modular solve cost;
- monotone reduction of the uncovered standard-pair dimension; and
- manageable finite-terminal growth.

### Gate C: genuine six-loop vacuum pilot

Use one 15-propagator, six-ISP decorated family. Begin with its highest-symmetry sector orbit and
one asymmetric control sector. Measure whether graph orbiting, matrix incidence treewidth,
Landau-source compression, tube width, and signature rejection still help after decorations break
most symmetries.

Stop if any of the following occurs:

- the completion basis or terminal ratio extrapolates exponentially in active coordinates;
- tube width grows with requested rank;
- exact guard decomposition creates unbounded or unsupported algebraic strata;
- modular support is unstable or exact reconstruction repeatedly fails; or
- peak memory approaches a full radius-three diamond without reducing uncovered dimension.

These are falsification gates, not promises that passing rank 20 proves all-rank closure.

## Priority order

1. Implement the finite-complement and nonminimal-terminal stopping criterion.
2. Apply standard-pair-guided triangular tubes to the six current `K = 6` recurrence strata.
3. Prototype modular signature/Janet completion and require leave-one-rule-out rediscovery.
4. Run critical, Fitting, and magic-relation diagnostics to measure source compression.
5. Pilot generating-function border completion on the sunset, then one irreducible `K = 6` sector.
6. Only after those gates, benchmark banana and genuine `L = 6` families.

This order maximizes immediate three-loop closure value while gathering evidence about the two
most plausible high-loop completion architectures.

## What not to port or assume

- Do not port LiteRed's Mathematica architecture, mutable global workflow, or historical file
  formats.
- Do not build a generic free or noncommutative Groebner CAS before a sector-local prototype wins
  its falsification tests.
- Do not call a sampled box, a stable miss count, or agreement with a master count closure.
- Do not globally optimize for the sparsest null vector.
- Do not use learned proposals as correctness evidence.
- Do not assume spanning cuts are complete before checking magic or supersector relations.
- Do not reconstruct every modular matrix entry when only a small certified support is needed.
- Do not assume low Feynman-graph treewidth implies low elimination treewidth.
- Do not copy MATAD or FORM recurrence equations into RustRed artifacts. Versioned numerical
  terminal values generated offline are permitted as Vakint evaluation data.
- Do not introduce Mathematica, FORM, SymPy, or another external CAS into the runtime reduction
  path.
- Do not accept a modular reconstruction without held-out samples and exact Symbolica replay.

## Conclusion

The literature points away from ever-larger translated diamonds. The most defensible near-term
route is an exact coverage engine that requests only the recurrence strata it still lacks, a
triangular tube compiler that searches those strata, and permission to stop at a certified finite
nonminimal terminal set. That combination directly addresses the current `K = 6` failure mode.

For eventual `L = 6`, signature-filtered Janet completion offers the clearest route to explicit
completion obligations, while generating-function border completion may offer a smaller
representation if source membership and quotient rank can be certified. Critical geometry,
symmetry, sparse modular linear algebra, reconstruction, and learning are accelerators around that
proof boundary. None should be allowed to replace it.
