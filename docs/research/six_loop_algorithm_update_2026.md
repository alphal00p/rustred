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

The trusted cover should be a canonical disjoint split trie, not a list of sampled points or
overlapping standard pairs. Internal nodes split exact coordinate thresholds, explicitly proved
residue classes, or supported guard predicates; leaves are rule owners or uncovered Stanley cells
`a + N^F`. A nonempty `F` is an open all-rank obligation, while `F = empty` enumerates a finite
terminal. Standard pairs remain the compressed search view, and the trie is the independently
replayed coverage certificate. Affine integer equalities, inequalities, and explicit congruences
are semilinear, but a general nonlinear integer guard locus is not Presburger. Such a branch stays
typed and unresolved unless another guard-sane rule owns it. Coverage-directed completion may
drop overlap pairs used only for confluence once deterministic ownership is fixed, but it may not
drop a prolongation whose conservative possible-leader shadow touches an uncovered free direction,
sector face, or guard-zero leaf.

A concrete implementation should separate three responsibilities. A persistent axis-block trie
owns the disjoint coordinate partition; a reduced ordered decision DAG over exact affine and
congruence atoms owns shifted guards at block leaves; and one frozen Janet or Janet-like epoch
generates candidate cones and supplies fast divisor lookup. Janet multiplicative masks depend on
the ambient basis, so an incrementally changing Janet tree cannot itself be publication authority.
Standard pairs may overlap, and signed rational-generating-function representations do not retain
unique owner provenance, so both remain generators or independent cross-checks. See the basis-
epoch and complement conditions in the complementary-decomposition work above, the Janet-tree
lookup construction in [Gerdt and Blinkov](https://arxiv.org/abs/math/0501180), and the overlap
warning for standard pairs in
[O'Shea and Thomas](https://sites.math.washington.edu/~thomas/papers/ot.pdf).

Candidates are inserted in one canonical order after exact replay. They may replace only
`Unowned` leaves; previously installed owners are immutable. Subtracting one half-open axis block
from another can be compiled in fixed axis order into at most `2K` disjoint lower/upper slabs,
while the guard DAG uses false edges directly for disequalities and congruence complements rather
than expanding them into disjunctive normal form. This makes common insertions output-sensitive in
the number of touched fragments, but generic decision-DAG apply can still be quadratic in its two
input DAGs and must not be advertised as uniformly output-sensitive.

Relaxed minimality permits an even weaker stopping condition than full Janet completion: every
installed infinite owner region has its own exact replay/domain/guard/descent proof, and every
remaining `Unowned` path has a materialized finite upper bound in every coordinate. The finite
residue can then be retained as terminals, with a conservative `BigUint` volume bound equal to the
sum of block volumes. Compactness of the proof does not imply affordable evaluation: the ideal
`<x_1^K,...,x_K^K>` has one bounded box but `K^K` terminal lattice points. An inferred ILP bound
must be converted to an exact implication witness before it enters this trusted boundary. Any
unbounded affine wall, congruence class, or nonlinear integer wall is a hard `Incomplete`, never a
terminal label.

#### Semantic generic-parameter guard atoms

**Verified algebra.** For algebraically independent generic parameters `lambda`, expand a pulled-
back guard as

```text
g(n;lambda) = sum_alpha c_alpha(n) lambda^alpha.
```

After specializing only the integer indices to `N`, the guard is zero in `Q[lambda]` exactly when
every `c_alpha(N)` vanishes. Its exceptional locus is therefore the coefficient-ideal variety
`V(I_g)`, with `I_g=<c_alpha(n)>`; applicability is the grouped open condition `D(I_g)`, not a
hypersurface obtained by choosing numerical generic parameters. Rational denominators must be
cleared first and retained as separate nonzero obligations. The target pullback also precedes the
coefficient split: a coefficient `c(n)` multiplying shift `alpha` is tested as `c(N-alpha)`. For
example, `(n1-n2) S1` has target guard `N1-1-N2`, not `N1-N2`.

Two regressions distinguish this semantics from whole-polynomial identities. The guard
`d(n0-1)+(n1-1)` has ideal `<n0-1,n1-1>`, hence is bad precisely when both indices equal one.
The guards `(d+1)(n0-1)` and `n0-1` define the same generic-`d` bad locus. RustRed's current eager
first-zero fallback deliberately does not merge either case: it canonicalizes only primitive
integer associates, produces `u+1` disjoint children for `u` new atoms, and confers no closure
authority. With `p` inherited and `u` new atoms it retains
`p(u+1)+u(u+3)/2` branch references; `u=4096,p=0` already gives 8,394,752. This is sound but not a
scaling representation.

**RustRed inference.** A trusted leaf should have the factored form

```text
coordinate domain intersect V(E) intersect D(J1) intersect ... intersect D(Js),
```

where each `Jj` is one grouped coefficient ideal. Use a globally ordered, hash-consed decision DAG
whose branch atom stores the complete context, source-guard fingerprint, affine pullback, normalized
generator payload, and normalization proof. Complementary edges make Boolean disjointness
structural; equal children are reduced. Hashes index full structurally compared payloads and never
act as proof. This shares a conjunction of `u` required predicates in `O(u)` nodes rather than
copying quadratic prefixes, though genuinely distinct overlapping conditions can still create an
exponential state space.

For algebraic-closure entailment, a constructible leaf is empty over `Qbar` iff one belongs to the
ideal obtained from `E` and, for each open group `Jj=<f_jk>`, a fresh inverse equation
`sum_k y_jk f_jk - 1`. Containment reduces to the same emptiness test. A publication certificate
must retain a Nullstellensatz identity `sum_i h_i f_i=1`; a computed basis containing one is not
independently checkable unless source cofactors are also recovered. Symbolica 2.2 exposes exact
`Q`, multivariate GCD/factorization, public F4 `GroebnerBasis::new`, basis reduction, and basis
verification ([polynomial API](https://symbolica.io/docs/polynomials.html)), but not extended-basis
cofactors, saturation/radicals, comprehensive systems, hard term/degree/time/scratch limits, or
cancellation. Consequently its Groebner engine is appropriate for a bounded helper-process
prototype, not yet as artifact publication authority. RustRed should not implement a second CAS;
bounded Macaulay cofactor recovery or an upstream Symbolica cofactor API is the preferred bridge.

`Qbar` and integer conclusions must remain different types. Algebraic-closure emptiness safely
prunes integer points; nonemptiness says nothing decisive over the integer lattice. Pell walls can
have infinitely many sparse non-semilinear points, other positive-dimensional curves can have only
finitely many integer points, and modular obstructions can prove integer emptiness even when the
complex variety is nonempty. Generic nonlinear integer solvability is undecidable already in
restricted finite-variable formulations; see [Sun, *Further results on Hilbert's Tenth
Problem*](https://arxiv.org/abs/1704.03504). Such a wall remains an exact `Incomplete` DAG leaf
unless another descending rule owns it or a bounded, affine-lattice, modular, or zero-dimensional
integer certificate resolves it.

The semantic-DAG implementation is a GO now; general Groebner pruning is research-only. K6 must
pass the two coefficient-ideal examples and shifted-lead regression while keeping the existing 205
guard occurrences on their cheap constant/univariate path. K10 must retain a Pell wall as
unresolved and distinguish an algebraically nonempty but modularly integer-empty wall. K15 must
compress the shared-wall family `g_i=(n1-1)(n_i-1)`, `i=2,...,15`, without allocating mask times
truth-table state, and must fail closed under its node/byte cap. Every benchmark records guard-
interaction treewidth, variables, generators, terms, degree, coefficient bits, auxiliary
variables, wall time, and worker peak RSS. Relaxed master minimality changes only the economic
gate: exact finite coverage comes first, followed by terminal count, symmetry compression,
dot/numerator complexity, epsilon debt, and measured AMFlow block/SCC dimensions.

**Implementation status, 2026-08-30.** The first test-only slice now performs the exact target
pullback before Symbolica's coefficient split, binds normalized primitive generator sets to the
indexed context, removes literal-unit ideals, and compiles priority-ordered conjunctions into a
bounded reduced ordered decision DAG. Hash lookup is followed by full structural equality.
Aggregate identity bytes, raw/canonical references, memo states and bitset words, candidate scans,
nodes, edges, and pending work all fail closed under explicit caps. The two semantic-ideal examples,
the shifted-lead regression, an exhaustive small truth table, and the K15 shared-wall proxy pass.
The next slice binds already replayed exact target circuits to the verified target partition,
sorts them by complete structural proof content, assigns stable priority IDs, compiles every exact
guard after target pullback, and returns the same retained `Arc` selected by the guard DAG.
Modular sample and rank telemetry are excluded from semantic priority only after their partition
join is checked; duplicate exact content fails closed so an upstream multi-prime collector must
deduplicate it deliberately. Aggregate caps cover the sealed proof payload, nested condition-source
coordinates, every translated representative/coefficient-system polynomial, monomial cells,
generator identities, and even the retained modular telemetry arrays. Fill-introduced elimination
pivots are correctly joined to the projected target/forbidden block rather than incorrectly
required to occur in the original source-row sparsity.

Each semantic identity retains the least primitive full-guard representative seen, and the exact
API evaluates all requested predicates at one context-bound integer point under cumulative
predicate, input-term, and specialization power-call caps. Per-predicate integer-bit limits remain
active, but a cumulative path bit-volume cap is still required before an untrusted persisted DAG is
accepted. Guard origins are joined against the sealed exact-replay chronology; this internal layer
does not redundantly reconstruct every guard from raw sources. A future artifact load boundary
must independently authenticate persisted proof content once. Independent adversarial re-audit
found this generic same-context semantics sound while explicitly withholding physical-fibre
authority.
Leaves are only discovery candidates or `Incomplete`; no RuleCell, integer-locus owner, or closure
authority has been added. The base variables are declared algebraically independent. A physical
parameter quotient or unit-mass fibre must be imposed before compilation, and a later arbitrary
specialization requires fresh guard evaluation. Production promotion still requires a persisted
physical-fibre signature, binding the selected exact circuit to the same point, proof that no reachable
`Incomplete` branch escapes the finite tail, and a measured RSS envelope; no algebraic-implication or
radical-equivalence pruning is claimed.

The audit separates three certificates which must never be conflated. Let `kappa` be the generic
base parameters after reducing the physical quotient to one certified integral component, let
`F=Q(kappa)`, and clear every rational denominator while retaining its own obligation.

1. **Pointwise semantic ownership.** For a pivot `p(n,kappa)`, the ideal `C_kappa(p)` in `Q[n]`
   generated by all `kappa`-coefficients describes exactly the integer-index points where `p`
   vanishes identically in `F`. A priority cover by these grouped open conditions is sufficient;
   it need not synthesize one row valid on the whole cell.
2. **Global Bezout row compression.** A witnessed unit ideal generated by several pivots in the
   localized cell ring `(F[n]/I_cell)_S` produces one combined owner row. This is an optional
   compression certificate. Its failure may expose an algebraic root depending on `kappa`, but
   cannot by itself create a physical integer-index wall.
3. **Epsilon-local safety.** After setting `epsilon=0`, every other generic parameter remains in
   the coefficient field. A witnessed unit combination of the constant pivots is sufficient for
   an epsilon-regular combined row. Failure falls back to pointwise owners and a separate exact
   epsilon-valuation/debt graph; it does not create a semantic wall.

The mandatory mutants are `p=n+epsilon`, which is pointwise applicable at every fixed integer
`n` but carries one unit of epsilon debt at `n=0`; `p=epsilon*n`, whose true semantic wall is
`n=0` and whose off-wall pivot still has epsilon valuation one; and `p=n(n-1)`, whose relative
index wall is the two terminal keys zero and one. Finiteness and dimension are always relative to
the index variables, never to `(n,kappa)` jointly. Even a proved zero-dimensional relative ideal
must expose its exact quotient degree and enumerate its integer keys before publication: a
Bezout-scale bound such as `delta^K` can be finite yet economically fatal at `K=21`.

#### Nonconfluent corner-cover theorem candidate

**Verified result.** In the rational Weyl setting, a border prebasis already gives a terminating
division into its finite order ideal; the later integrability conditions promote that spanning
collection to an independent basis of the stated rank:
[Rodriguez and Sattelberger, Algorithm 2.9 and Theorem 2.11](https://arxiv.org/abs/2510.23411).
The ordinary IBP left ideal is finitely generated by the standard relations, although direct
noncommutative Groebner computation can be impractical even for much smaller examples:
[Barakat et al., Proposition 2.2](https://arxiv.org/abs/2210.05347).

**RustRed inference.** The analogous positive-difference statement is the weakest promising
six-loop certificate found in this review. Let `J` be generated by admitted exact IBP consequences,
so `J` is a subideal of the complete physical IBP ideal. If every lattice point outside one finite
sectorized tail has an applicable rule and every right-hand side decreases in one global
well-founded order, induction reduces every target into that tail modulo `J`. The same terminals
therefore span the further physical quotient. Rules may overlap and yield different terminal
expressions: that exposes redundancy, not a failure of spanning. Confluence, critical-pair
completion, equality `J = I_IBP`, integrability of shift-action matrices, terminal independence,
and master-count equality are unnecessary unless RustRed later claims a unique/minimal quotient or
uses recovered action matrices as new relations.

The trusted object is consequently a guarded corner cover. Each sector has a compact tail bound
`b`; its disjoint guard DAG ends either at an exact descending owner or at a leaf proved to lie in
`0 <= x_i < b_i` for every coordinate. The learner always attacks the least unowned unbounded leaf,
using differentiated descendants, syzygy-constrained sources, bounded seedless/triangular frames,
and modular lift in a fair deterministic dovetail. Discovery has no termination theorem: resource
exhaustion or any surviving unbounded leaf returns `Incomplete`. This direct cover may bypass a
huge Janet completion, but it cannot bypass pulled-back guards, exact ordinary-source replay,
sector representability, one global descent order, mathematical outer-extension evidence, or the
independent epsilon-debt certificate.

The decisive synthetic separation is `<E_1^q,...,E_K^q>`. Its certificate can contain only `K`
corner rules and one compact bounded tail, while its terminal volume is `q^K`. K6 must certify
`q=4` without enumerating the first border; K10 must keep `q=10` compact while adding all 45
diagonal walls; K15 must keep `q=15` compact while stressing 105 diagonal walls and an injected
same-degree cycle. A compact closure proof on these controls is a GO for the representation only;
the enormous terminal volume is deliberately a NO-GO for direct numerical evaluation.

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

#### Audited proof-carrying descendant compiler

**Verified result.** The expanded generating-function algorithm chooses and caches candidate
"mother" equations heuristically rather than supplying an exhaustive critical-parent theorem. Its
sunset example contains one descendant with two parents: substitution by the generating mother
returns `0 = 0`, whereas substitution by the other parent exposes a new lower-degree equation:
[*Algorithm for Symbolic Reduction*, Sections 3.2.3 and 4.2](https://arxiv.org/html/2605.09541v1#S4.SS2).
The same paper uses a tunable descendant cutoff and relaxes it when completeness stalls; this is
not a generic termination proof.

**RustRed inference.** A proof-carrying descendant is sound only as a typed left Ore-module
circuit. With ordinary sources `P_j` and `E = sum_j C_j P_j`, the permitted provenance updates are

```text
D_i E           -> sum_j (D_i C_j) P_j
E - Q R         -> C(E) - Q C(R)
sum_i a_i E_i   -> sum_j (sum_i a_i C_ij) P_j.
```

`Q` acts from the left. Right multiplication or an unshifted commutative coefficient is invalid.
Lead normalization localizes the circuit; every later shift `delta` must retain
`sigma^delta(lead) != 0` and every shifted denominator factor. Restricted subsector generating
functions also need a distinct proof-node type because differentiation and restriction to a zero
parameter do not commute. A hash-consed DAG can share provenance, but it cannot hide growth in the
normalized candidate rule, shifted denominators, or a flattened ordinary-source certificate.
Compositional exact replay is therefore the authority, while bounded flattening remains mandatory
scaling telemetry.

The search scheduler must enumerate the complete parent-incidence list for an exposed descendant,
exclude the generating mother as the sole attempted reduction, and retain a bounded transverse or
LCM critical halo. Exact-source-signature deduplication and owner-cover intersection prioritize the
useful remainders. Once every guard-specific complement is finite, the remaining parent pairs are
irrelevant to closure and may be discarded. Before then, mother-only normal forming can silently
stall even though a finite cover exists.

The smallest mandatory semantic falsifier uses six commuting shifts:

```text
P1 = S1*S2 - 1        P2 = S1*S3 - S2
P3 = S1^2 - 1         P4 = S3^2 - 1
P5 = S4 - 1           P6 = S5 - 1           P7 = S6 - 1.
```

The initial leading ideal leaves an infinite `S2` direction. For `E = S3*P1`, mother reduction
gives zero, but the alternative parent yields

```text
E - S2*P2 = S2^2 - S3,
cofactor = S3*e(P1) - S2*e(P2).
```

Adding that exact rule leaves five standard monomials. A companion Ore mutation
`G = (n1-n2) S2 - 1` must give `S1 G = (n1+1-n2) S1*S2 - S1`; the infinite wall
`n1+1=n2` remains a separate guard branch. This synthetic regression and one physical K6 `S4a`
descendant must pass before the hybrid can replace target-local translated frames. At K10 and K15,
report alternative-parent multiplicity, retained descendants, proof DAG/flattened sizes, shifted
guard factors, exact replay cost, and free-dimension reduction. If the critical halo recreates the
full translated shell or the full Ore pair set, the candidate is correct but not the required
scaling breakthrough.

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

The exact sufficient certificate is a finite valuation-weighted owner/guard graph. After
substituting `D = 4-2 epsilon`, combine identical children and normalize each coefficient exactly
with Symbolica. On every exhaustive valuation leaf retain a uniform integer lower bound
`w(u,v) <= nu_epsilon(c_uv)`. A potential must satisfy

```text
p(u) <= w(u,v) + p(v),       p(terminal) = 0,       p(start) >= -B.
```

Telescoping and `nu(a+b) >= min(nu(a),nu(b))` prove that every terminal coefficient has pole debt
at most `B`, independently of target rank. A negative cycle rejects the finite abstraction unless
an exact coordinate refinement proves that cycle nonrepeatable. The recurrence
`I_n = epsilon^-1 I_(n-1)` is the mandatory counterexample: it has strict descent and one terminal,
but debt grows as `n`. Generic valuation over `Q(n)` is also insufficient because
`1/(n+epsilon)` has valuation zero generically and minus one at `n=0`. This is an epsilon-valuation
stratum, not an exact applicability wall: `n+epsilon` is nonzero as a generic-parameter polynomial
at every fixed integer `n`. By contrast, `epsilon*n` has both the exact index wall `n=0` and
positive epsilon valuation off that wall. The valuation abstraction must distinguish these cases
or remain inconclusive.

Store the witness as exact epsilon powers times numerator/denominator units whose nonvanishing at
epsilon zero is proved on the declared leaf. Terminal basis changes, gamma factors, evaluator
normalizations, and finite substitutions are graph edges too. Bellman--Ford-style difference-
constraint verification is combinatorial; Symbolica remains responsible for exact cancellation,
factorization, substitution, and coefficient-series primitives. This certificate is independent
of the finite owner-cover proof and is required before a rank-generic nonminimal terminal set is
shippable.

An epsilon-finite or quasi-finite evaluator basis is a useful optimization, not the theorem:
[*An epsilon-finite basis of master integrals for the integration-by-parts method*](https://arxiv.org/abs/hep-ph/0601165)
and [*A toolbox for solving Feynman integrals with dimension shifts*](https://arxiv.org/abs/1411.7392).
Exact finite terminal relations may be eliminated over the epsilon local ring to build a sparse
map `T -> E` with lower debt, without claiming that `E` is a globally minimal RustRed basis. If
independence is not certified, the numerical evaluator must still reduce its target set rather
than using an unsafe skip-reduction option.

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

The recursive auxiliary-flow construction determines integrals in principle once the required
linear/IBP reductions are supplied and demonstrates examples through five loops
([Liu and Ma](https://arxiv.org/abs/2201.11637)). For a vacuum terminal its recursion enters
lower-loop propagator or multiscale auxiliary families. A unit-mass vacuum-only RustRed artifact
does not reduce those families, so the evaluator ledger must name the reducer at every recursion
node. This is a concrete dependency, not a claim that AMFlow is mathematically circular.

The affordability ledger must therefore also retain evaluator target count `e`, unique recursion
families, physical and auxiliary master counts, differential-equation nonzeros and strongly
connected block sizes, flow segments, series orders, epsilon samples, working precision, boundary
children, exact `T -> E` map sparsity, certified per-terminal debt, checkpoint bytes, wall time, and
RSS. Numerical epsilon sampling parallelizes well but turns pole debt into conditioning loss rather
than removing it. Precision ladders must calibrate this full recursion before extrapolating a
20,000-digit campaign. A thousand shipped values can be storage-cheap while one dense auxiliary
system is computationally prohibitive; conversely, a redundant terminal set split into small exact
blocks can be viable.

### Dense-parent extrapolation and sparse controls

**RustRed derivation.** Face-local completion can make complexity follow the dimension of the
currently uncovered inactive/ISP face instead of the complete index count. This is particularly
promising for connected cubic vacuum parents: with `K = L(L+1)/2` scalar-product coordinates and
at most `3(L-1)` propagators, their top-sector inactive dimensions at four, five, and six loops are
only `1`, `3`, and `6`. It is not a topology-generic scaling argument. The corresponding equal-mass
bananas retain `5`, `9`, and `14` inactive directions, while asymmetric three-vertex multitheta
graphs with edge-bundle multiplicities `(1,2,3)`, `(1,2,4)`, and `(1,2,5)` retain `4`, `8`, and
`13`.

This coordinate count does **not** make the root family with lines `q_i` and `q_i-q_j` a universal
physical parent beyond three loops. Its line-vector matroid is graphic, whereas a vacuum graph
`G` presents the cographic matroid `M*(G)`. At four loops the cubic vacuum graph `K_{3,3}` is the
decisive counterexample: because it is nonplanar, `M*(K_{3,3})` is non-graphic and cannot be a
restriction of `M(K_5)` under a unimodular loop routing. K10 is therefore already a multi-parent
campaign; K15 and K21 need matcher-derived physical parent manifests rather than complete-graph
mask counts. Graph or parameter-polynomial canonicalization may propose an equivalence, but every
accepted reuse must retain and replay an exact simultaneous routing witness on propagators, ISPs,
masses, guards, cuts, and ordering. The modern groupoid treatment constructs such affine momentum
maps from parameter permutations in [Duhr et al.](https://arxiv.org/abs/2604.08332); it does not
turn a canonical label into a proof.

The six-loop seven-edge banana makes the symmetry tradeoff concrete. Its loop space is the
standard `S7` representation `V`; its scalar products decompose as

```text
Sym^2(V) = [7] + [6,1] + [5,2].
```

The seven propagator quadrics span the first two summands, leaving the 14-dimensional `[5,2]` ISP
quotient. A Burnside/Molien audit gives 344 invariant degree-ten numerator polynomials on the
fully undotted symmetric stratum instead of 1,144,066 raw monomials, while dot excess ten has 38
`S7` orbits instead of 8,008 raw compositions. These reductions matter in practice, but the pinned
Symbolica graph API has an important representation boundary: canonicalization exposes vertex
maps and vertex-automorphism generators, whereas identical parallel-edge permutations contribute
to the reported automorphism-group size without yielding explicit edge maps. RustRed must therefore
subdivide propagators into colored edge vertices, or generate and verify parallel-bucket
transpositions itself, before a routing witness may use those line symmetries. Mass, cut, sector,
and guard colors must remain canonicalization-visible; hidden graph data is safe only for
provenance that cannot affect equivalence.

Even explicit graph symmetry is not a rank-generic monomial-expansion strategy. For the seven-line
banana, dot excess `r = 8, 16, 32` has respectively `3,003`, `74,613`, and `2,760,681` labelled
assignments but only `21`, `164`, and `2,400` partitions with at most seven parts. By contrast, the
14 ISP directions form the nontrivial `[5,2]` representation rather than a permutation basis. A
dense symmetry image of a degree-`r` ISP monomial can touch `binomial(r+13,13)` monomials:
`203,490`, `67,863,915`, and `73,006,209,045` at those same degrees. Graph orbiting is therefore
appropriate for structural task routing and physical-dot orbits. Rank-generic ISP transport must
retain symbolic generating rules or representation blocks without monomial expansion. A finite
group still supplies only an eventual constant factor; generic unequal dot patterns destroy the
stabilizer and do not change the polynomial growth degree.

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

## Authorized complete-family scaling studies

**RustRed decision, 2026-08-30.** The current research winner is a
generating-function, syzygy-accelerated, stratified universal index-cone
completion lane. This is a RustRed project label, not terminology claimed by
the cited authors. It combines compact generating-function descendants,
sector-local syzygies, exact guard strata, and a deterministic nonconfluent
owner cover. Minimality and equality of overlapping normal forms remain
optional diagnostics: a finite, exactly covered, independently evaluable
redundant terminal set is acceptable.

**Verified input from the recent generating-function literature.** The Weyl
algebra recurrence translation carries falling/rising factorial index factors;
their integer zero walls cannot be erased when a generating-function operator
is converted to an ordinary source. Leader orthants describe structural reach,
not guard validity. The path-agreement master-count criterion used in the
generating-function construction is therefore not imported as RustRed closure
authority ([arXiv:2605.09541](https://arxiv.org/abs/2605.09541)). Recent
syzygy constructions can avoid gratuitous active-line power raises
([arXiv:2507.11140](https://arxiv.org/abs/2507.11140)), while seedless inactive
bulk/face/edge systems offer compact candidate sources without a general
source-degree termination theorem
([arXiv:2602.22111](https://arxiv.org/abs/2602.22111)).

RustRed keeps a generating-function normalization internally and records an
exact diagonal conjugation to freshly regenerated ordinary sources. In one
fixed convention the operator conversion has the form

```text
p(Theta) eta^(delta-) partial^(delta+)
  -> p(x) product_(delta_i>0) (x_i+1)^(rising delta_i) E^delta,
E^delta q(x) = q(x+delta) E^delta.
```

Every admitted operator retains a left-module provenance DAG over the original
ordinary sources. Factorial zero walls, target-pulled coefficient ideals,
sector boundaries, and alternate parent incidences remain explicit. A
modular leader shadow enters the discovery ideal only; only exact replay,
guard ownership, boundary routing, and strict descent can enter the exact
owner ideal.

The user has authorized bounded studies of this winner on the complete
single-scale vacuum families at four, five, and six loops while Stage 1 remains
active. These are scaling studies, not Stage 2 artifact production. The study
uses the following predeclared progression:

1. `Manifested`: independently generate and reconcile the complete raw
   contraction-pseudograph census, canonical hashes, edge action, and every
   parent/child route.
2. `Probed`: enumerate all structural sector/chart orbits and source/syzygy
   envelopes without solving.
3. `ModularCandidate`: run the same capped cheap finite-field probes on every
   manifest member and retain timeouts/OOMs as censored failures.
4. `ExactReplayed`: lift selected stable supports over exact `Q(d)` and replay
   ordinary-source cofactors.
5. `GuardOwned` and `BoundaryDischarged`: compile target-pulled guard strata
   and prove every lower-sector, alternate-parent, zero, and factorization
   route.
6. `ChartClosed` and `FamilyClosed`: claim these states only after the exact
   complement has no positive-dimensional or unresolved guard leaf across
   the entire manifest.
7. `EconomicallyEvaluable`: separately measure finite-terminal count,
   evaluator SCC/block dimensions, epsilon debt, application-state growth,
   precision, checkpointing, and aggregate resource cost.

The manifest keeps both a raw pseudograph DAG and a normalized DAG. Self-loop,
bridge/cut-vertex, and bivalent rewrites require decoration-aware proof
objects. Cross-component ISPs can prevent literal factorization; unequal
masses and cuts can invalidate dot or partial-fraction shortcuts. A physical
propagator must map individually, up to an explicitly represented scalar/mass
normalization, to a physical family slot. General exact linear changes are
allowed only inside the auxiliary ISP subspace, with their numerator expansion
cost measured. Cross-family loop routing normally requires a `GL(L,Z)` witness
with determinant `±1`; otherwise the `|det|^d` measure factor lies outside the
ordinary rational coefficient field unless represented explicitly.

For an uncut fully massive core, loopless minimum-degree-three counting gives
the finite structural ranges `V <= 2L-2` and `E <= 3L-3`, but only after the
normalization obligations above are discharged. One generator/canonicalizer
build is cross-checked against a separately implemented canonical-augmentation
path using Symbolica graph canonicalization. Literature topology lists are
regressions, not the manifest authority. Each family stores an exact cycle
matrix, scalar-product completion and inverse, physical-slot map, mass/cut
decorations, edge generators, parent-local contraction charts, and exact route
witnesses.

At `K=6`, the top-family edge symmetry is the order-24 `S4` action on the six
edges of `K4`, not `S6`. It supplies one six-element leader orbit, but a leader
does not uniquely identify a full circuit: its edge stabilizer can preserve
the pivot while changing exact right-hand-side content. RustRed therefore pins
a deterministic replayed transversal rather than demanding stabilizer-
invariant normal forms. Missing-edge five-line charts split adjacent and
opposite dotted-edge orbits, and the four-line numerator holes remain separate
obligations. Top coverage never certifies these faces.

### Clear solver denominators before branching on guards

The next K6 experiment must separate mathematical applicability from the
chosen elimination path. The present exact circuit records source conditions,
source-coefficient denominators, every intermediate reducer pivot,
source-multiplier denominators, and residual denominators. This is safe but can
create exceptional branches that belong only to Gaussian elimination, not to
the final replayed recurrence.

For every admitted circuit, form and replay a canonical polynomial source
combination after clearing all rational solver denominators. Its final target
coefficient remains an exact polynomial and its nonzero locus is mandatory.
Source- and family-intrinsic conditions also remain mandatory. Intermediate
solver pivots may be removed from the semantic guard set only after the
cleared relation and its ordinary-source cofactors replay exactly. In
particular, `n*I_t = 0` still owns only `n != 0`: a denominator-free normalized
right-hand side does not authorize the `n = 0` branch.

For several replayed rows with the same target, the ideal generated by their
final target coefficients may prove that their domains jointly cover the
physical fibre or leave only a zero-dimensional wall. Symbolica's native F4
Groebner implementation may discover that structure, but publication still
requires bounded exact Bezout/source-cofactor recovery because the public
basis result does not retain the required transformation trace. The A/B/C
experiment compares the existing full-guard circuit, the cleared polynomial
circuit, and this grouped-target cover. It records guard atoms by origin, DAG
nodes and unresolved-leaf dimensions, exact cofactor size, replay time, peak
RSS, leading standard pairs, and terminal count. Promotion requires perfect
replay and either a twofold reduction in nonconstant guards, owner leaves, or
DAG nodes, or exact discharge of a previously open wall; the extra exact work
is limited to 25 percent of baseline lift time and 10 percent artifact growth.

The grouped-target statement is deliberately conditional. A unit ideal in the
final target pivots proves only that at least one pivot is nonzero pointwise; it
does not discharge the source-intrinsic or cleared-denominator gates on which
the corresponding rows are valid. If every such gate is independently proved
true on the complete owner domain, the grouped certificate may be total. If
the rows merely share one identical unresolved gate, it seals only the branch
where that gate holds and leaves its zero branch as an explicit obligation.
Differing unresolved gates are rejected by the first implementation. The
counterexample of a common gate `n != 0` with pivots `n` and `n-1` has a unit
target ideal but remains invalid at `n = 0`; any compiler that calls it globally
total is unsound. A production certificate must also persist a proof of the
physical fibre used before splitting base-parameter coefficients, not only a
free-variable map or fingerprint. Accordingly, literal units, exact univariate
Bezout identities, and tiny exhaustive finite-field obstructions enter first as
bounded test-only, fail-closed certificate producers; every timeout, resource
cap, unresolved gate, or unsupported locus remains typed `Incomplete`.

A bounded test-only fraction-free prototype now clears each ordinary source
row, clears and primitive-normalizes the resulting source-cofactor vector, and
replays every physical column with Symbolica-native exact polynomials. On the
deterministic degree-one K6 S4a fixture it reduces ten elimination-only guards
to the one mandatory final-target guard while retaining six source cofactors,
fourteen nonzero physical terms, and twenty-six total physical-coefficient
terms. The accounted reconstruction uses 422 exact operations, ten GCD
term-pairs, and 221 retained polynomial terms; two complete reconstructions
are structurally identical. One warm local debug run spent about 2.1 ms in the
clearing slice. The tadpole control reduces two elimination-only guards to its
single final-target guard, and the `n*I_t=0` mutant confirms that `n != 0`
cannot be discarded. This is promising A/B evidence only: baseline exact-lift
timing, peak RSS, artifact-size growth, and complete owner-leaf counts have not
yet been measured, so the 25-percent time and 10-percent size promotion gates
remain open.

All-family structural envelopes are reported before promotion. Conditional
one-sided degree-three row counts are `4,576`, `20,400`, and `72,864` for full
`K=10,15,21` frames; these numbers exclude topology multiplicity, sector
orbits, guard leaves, parent incidences, operator support, and elimination
fill. Every report therefore gives raw and authenticated-unique task counts,
per-family median/tail/max costs, the named worst family, cold/warm-cache I/O,
coordinator plus worker RSS/PSS, checkpoint bytes, and the split
`M(W)=M_shared+W*M_private`. Discovery and online application are timed
separately; a finite terminal set does not bound the number of memoized
intermediate integral keys at large rank.

An offline numerical evaluator must also accept the deliberately redundant
tail honestly. AMFlow's `SkipReduction` option is used only for a set already
certified to be an independent basis/subset; it is not enabled merely because
RustRed proved the tail finite. Otherwise AMFlow or the offline oracle first
reduces the retained terminals to its own certified basis, or RustRed supplies
an exact map.

K10 must pass dense planar, nonplanar `K3,3`, low-symmetry, banana, asymmetric
multitheta, and principal-boundary controls without hiding failures behind
orbit-weighted averages. K15 is the out-of-sample design gate: the scaling
model and prediction interval are frozen after K6/K10, and a censored required
dense family falsifies the K21 projection. Global degree-four dependence,
unstable exact lifts, completion degree growing on two enlargements, promotion
fill above 50 times input, rank-unbounded epsilon debt, or aggregate resources
without twofold hardware headroom are kill conditions. K21 begins only with a
complete manifest and uniformly capped structural/cheap modular probes; an
expensive exact sweep still requires K15 promotion. No showcase banana or
complete-graph proxy can substitute for this gate.

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
4. Lift candidates exactly, clear solver denominators, recover the final polynomial target
   coefficient, and replay the polynomial cofactors against regenerated ordinary sources. Keep
   intrinsic source/family conditions and the target coefficient's exact nonzero locus; do not
   promote intermediate reducer pivots into semantic guards merely because discovery divided by
   them.
5. Admit replayed leading classes into the exact symbolic owner cover. Compile grouped target
   coefficients only with replayed Bezout/source cofactors, and retain every unresolved physical
   guard wall as an explicit obligation.
6. Once every remaining complement component is finite, either retain and evaluate the complete
   affordable typed terminal set, or—only when compression is useful—construct its exact relation
   module across the whole guard stratum before recovering shift actions with block-Krylov, FGLM,
   or Hermite methods. Call the compressed set independent only after a separate exact lower-rank
   certificate closes the rank sandwich.
7. Verify source annihilation, guard ownership, lower-sector maps, symmetry routes, and strict
   descent after canonicalization. Overlap normal forms, commuting shift actions, and critical-pair
   completion are optional diagnostics unless RustRed publishes an independent quotient/action
   presentation; a deterministic nonconfluent pointwise owner cover does not require them.
8. Prove a uniform epsilon-valuation bound and derive the Laurent depth required of every terminal.
9. Partition the independent terminal quotient into numerical blocks and measure the largest
   AMFlow auxiliary dimension before commissioning high-precision data.

Discovery output remains untrusted until step 7 completes. Exact replay belongs at the artifact
admission boundary; it is not repeated in the reducer hot path.

## Falsification gates

The numerical thresholds below are engineering gates, not literature theorems.

### `K = 6`

- Replay every rule from regenerated exact sources on every guard stratum.
- Eliminate every positive-dimensional standard pair and explicitly enumerate any resulting
  zero-dimensional finite residue as terminals.
- Verify strict descent after canonicalization and immutable lower-sector boundaries. Compare
  overlapping paths only after an exact MATAD terminal map or numerical evaluation unless a
  separate quotient/action presentation is being claimed.
- Prove a target-rank-independent epsilon-valuation bound; sampled ray reductions are insufficient.
- Treat `t <= 100` typed terminals as the direct numerical-tail target and cap the reference
  pressure run at 16 GiB and two wall-clock hours. Generating-function discovery replaces the
  translated-frame baseline only if it retains at least two times fewer exact rows and lowers peak
  RSS by at least 1.5 times on the same complete manifest.

### `K = 10`

- Freeze a matcher-derived multi-parent manifest and require exact finite-complement results for a
  nine-propagator `K_{3,3}` completion, a planar dense control, the five-edge banana, and the
  `(1,2,3)` multitheta. Any single complete-graph/root-coordinate family is insufficient.
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
