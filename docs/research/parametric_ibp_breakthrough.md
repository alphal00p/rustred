# Parametric-IBP completion beyond LiteRed

## Purpose and boundary

RustRed needs exact, closing, all-rank parametric IBPs.  LiteRed and LiteRed2
are valuable semantic controls, but their translated-point heuristic is not a
credible scaling target for a complete six-loop vacuum family.  This note
records the current primary-literature review, separates published results
from RustRed hypotheses, and fixes a falsifiable implementation programme.

Stage 1 still closes the unit-mass `K = 6` three-loop family and validates it
through Vakint.  Four- through six-loop artifact production remains gated.
Algorithm research and bounded prototypes are nevertheless evaluated against
the eventual six-loop problem now: a Stage 1 primitive must not win only by
moving work into an exponential translation grid.

The trust boundary does not change.  Modular or heuristic work may discover a
small candidate source support.  An artifact can contain a rule only after
Symbolica exact algebra reproduces it from freshly generated sources, every
division is guarded, every same-sector term is strictly descending, every
lower-sector term is discharged, and the complete domain complement is
accounted for.  A failed search never creates a master.

The output basis need not be minimal.  A proof that the all-rank residual
complement is finite permits that finite set to become a versioned set of
evaluation terminals, even when it is larger than a conventional master
basis.  This is operationally useful: Stage 1 can attach exact MATAD maps or
very-high-precision MATAD evaluations, while a later high-loop campaign can
attach AMFlow values directly. FMFT can supply future exact four-loop
reductions, but most numerical constants currently shipped with Vakint have
only about 26--50 digits; generic four-loop 20,000-digit terminal data would
need regeneration or AMFlow.
Adding more denominator coordinates is not the mechanism—the completed family
already spans every scalar product.  The mechanism is to retain additional
integral keys as terminal representatives.  A finite bounded sample of misses
does not establish a finite complement.

Scaling to six loops is more important than reproducing a historically minimal
basis.  Each experiment therefore reports a terminal budget alongside solve
cost: terminal count, feasibility of one-off simultaneous AMFlow evaluation,
precision and storage cost, and numerical conditioning.  Vakint/MATAD is an
authorized offline K=6 diagnostic oracle: its exact reductions can reveal
which lowering direction or guard branch RustRed missed, and its evaluations
can provide very-high-precision terminal references.  Oracle data may suggest
or validate a rule, but only an exact RustRed replayed source combination may
enter the production artifact.

## Scaling diagnosis

For an `L`-loop vacuum family completed by all scalar products,

```text
K = L(L + 1) / 2
q = L^2
```

are respectively the number of index directions and ordinary momentum-space
IBP rows.  A signed `L1` translation diamond of radius `h` has

```text
D_K(h) = sum(j = 0..min(K,h), 2^j C(K,j) C(h,j))
```

points.  At six loops (`K = 21`, `q = 36`), radii two, three, and four contain
925, 13,287, and 143,529 translations: roughly 33 thousand, 478 thousand, and
5.17 million ordinary rows before symbolic fill.  Expanding complete diamonds
is therefore the control experiment, not the intended discovery algorithm.

The present three-loop K4 family is a useful falsification laboratory.  Its
current finite census has 115 submitted roots, 44 symmetry-canonical roots,
89 discovered nodes, 53 rule applications, 27 terminals, and nine uncovered
nodes.  Three uncovered scalar corners are terminal-certification obligations:

```text
[0, 1, 1, 1, 1, 0]
[0, 1, 1, 1, 1, 1]
[1, 1, 1, 1, 1, 1]
```

The six current recurrence witnesses are:

```text
[0, -1, 1, 2, 2, 1]
[0, -2, 2, 2, 1, 1]
[0, 1, 1, 2, 4, 0]
[0, 1, 1, 2, 5, 0]
[0, 1, 2, 3, 3, 0]
[0, 1, 3, 2, 3, 0]
```

They are samples of positive-dimensional lattice strata, not six reasons to
add six individually planned Rust modules.

The first exact geometry prototype maps all 46 current cells to sector-local
carrier boxes and agrees with 33,534 direct cell-membership checks. On the two
sectors above, subtracting the 7 and 19 structural domain boxes leaves 20 and
32 disjoint boxes, each with a six-dimensional varying component. This is a
guard-blind lower bound on the genuinely uncovered set: 205 nonzero guards
still require separate stratification. Likewise, 61 of 276 outer endpoints
reach a maximal rule-safe application endpoint, but only 35 touch the `i64`
chart carrier. Neither is called an infinite ray without a symbolic extension
proof.

## What the literature establishes

### Stratified symbolic rules

Lee's LiteRed papers ([arXiv:1212.2685](https://arxiv.org/abs/1212.2685)
and [arXiv:1310.1145](https://arxiv.org/abs/1310.1145)) solve at a generic
symbolic point, translate a useful leader, determine the exact coefficient and
boundary failure locus, and search progressively lower-dimensional exceptional
strata.  The underlying operator argument in
[arXiv:0804.3008](https://arxiv.org/abs/0804.3008) explains why identities at
the point removed by a generic recurrence become dependent modulo translated
instances and simpler points.  This is the correctness baseline for guard and
boundary refinement.  It does not prove termination at a fixed neighborhood
depth and it does not address the six-loop translation-volume cliff.

### Syzygy and seedless lowering operators

Smith and Zeng's symbolic rules
([arXiv:2507.11140](https://arxiv.org/abs/2507.11140)) construct sectorwise
IBP-generating vectors whose action does not raise active propagator powers,
then expose generic-index recurrences by operator row reduction and small
partly symbolic neighborhoods.  The seedless construction of de la Cruz and
Kosower ([arXiv:2602.22111](https://arxiv.org/abs/2602.22111)) grows triangular
levels only in inactive/ISP directions and solves for a combination retaining
the target while cancelling every worse top-sector integral.  Bulk operators
are followed by face, edge, and dotted boundary operators.  These papers give
strong evidence that source preconditioning can be dramatically smaller than
a full signed seed grid, but their demonstrations are at two loops and no
universal finite level is proved.

### Generating-function operator completion

Feng, Li, Liu, Ma, and Zhang
([arXiv:2605.09541](https://arxiv.org/abs/2605.09541)) package a whole sector
into one generating function.  Ordinary IBPs become differential equations in
a Weyl algebra; the operator index is the induced nonnegative lattice shift.
The algorithm extracts rules by small operator-coefficient eliminations,
simplifies every equation globally by installed rules, generates guided
descendants, and measures completeness by the lattice points outside the
union of rule orthants.  It unifies dots and inactive numerators more naturally
than pointwise cells.  Its authors explicitly leave optimized implementation,
systematic symmetry use, and broader high-loop benchmarks for future work;
coefficient-zero strata also need RustRed's stronger exact treatment.

### Modular discovery and triangular systems

FiniteFlow ([arXiv:1905.08019](https://arxiv.org/abs/1905.08019)) establishes
finite-field sampling and reconstruction as a scalable dataflow technique.
Blade ([arXiv:2405.14621](https://arxiv.org/abs/2405.14621)) uses numerical
reductions and ansaetze to discover much smaller block-triangular systems.
Liu and Mitov ([arXiv:2512.05923](https://arxiv.org/abs/2512.05923)) use cheap
rank tests on shifted IBPs to find symbolic diagonal or triangular recurrences.
These results justify numerical *selection* of equations.  They do not replace
exact source replay, and Blade's initial seeded systems may still be very
large.

### Ore ideals, master diagnostics, and graph structure

Barakat et al. ([arXiv:2210.05347](https://arxiv.org/abs/2210.05347)) formulate
IBPs as a left ideal in a rational double-shift algebra.  A noncommutative
Groebner basis would solve the family globally, but their practical fallback
for harder examples is a bounded lowering-monomial linear-algebra ansatz.  A
full noncommutative Groebner engine is consequently not RustRed's first move.

Lee--Pomeransky critical points
([arXiv:1308.6676](https://arxiv.org/abs/1308.6676)), critical syzygies
([arXiv:2509.17681](https://arxiv.org/abs/2509.17681)), and recent
branch-representation intersection theory
([arXiv:2604.05025](https://arxiv.org/abs/2604.05025)) can provide independent
master counts or basis guidance under their stated hypotheses.  They are
diagnostics, not substitutes for a closing rewrite system.  Symbolica's graph
canonicalization, isomorphism, automorphism, orbit, bridge, and generation
primitives can quotient decorated sector/minor work; graph identity must still
include mass, routing, and scalar-product decorations and be replayed exactly.

## RustRed synthesis (hypotheses, not published claims)

The proposed foundry is a portfolio with one common proof boundary:

```text
decorated sector/minor DAG
          |
          v
uncovered lattice strata / target requests
          |
          +--> modular ordinary-source scout        (first prototype)
          +--> seedless syzygy lowering source      (measured competitor)
          +--> bounded Weyl descendant completion   (high-risk competitor)
          |
          v
exact Symbolica target reduction + guard stratification
          |
          v
strict replayed rules / explicit terminals / closed artifact
```

### Common sector and coverage model

Each sector receives nonnegative local coordinates:

- active index `n_i >= 1`: dot depth `x_i = n_i - 1`;
- inactive index `n_i <= 0`: numerator depth `x_i = -n_i`.

One persisted, translation-compatible, well-founded ordering orients all
rules.  Sectors are compiled bottom-up; lower-sector terms are normalized
through immutable child owners; equivalent decorated sectors, strata, source
rows, and operator descendants are transported under authenticated symmetries.
A target task may be quotiented only by the stabilizer of its target, complete
domain/guard stratum, ordering, and family data.  Orbit-related equations are
not discarded unless an exact row-space rank certificate proves redundancy.

A rule with leading coordinate `o >= 0` covers the orthant
`o + N^r` on its current stratum.  Minimal leading coordinates form a monomial
antichain.  The complement is represented exactly by standard pairs or an
equivalent finite box/orthant decomposition, including positive-dimensional
axes and faces.  Completion queues contain these strata rather than sampled
integral keys.  Algebraic coefficient guards refine them separately; generic
`d` is never confused with an integer-index boundary.  This structural
decomposition is not by itself guard coverage: a guard such as `n1-n2` has an
infinite diagonal zero locus which axis-aligned standard pairs do not encode.
Every guard must therefore be certified nonzero over its whole stratum,
factored into a supported exact guard language with every zero branch owned,
covered there by an alternative rule, or rejected as a closure owner.

Closure requires all structural and guard strata to be owned by a descending
rule, zero, factorization, or independently justified master terminal.  Rule
overlaps and every original source identity must reduce consistently.  A
known master count is a stopping diagnostic, never the sole proof.  A finite
nonminimal evaluation-terminal manifest is allowed once finiteness, spanning,
and terminal evaluation or basis-change data are independently established.

### Architecture A: counterexample-guided modular target separation

This is the first implementation experiment because it fits immediately in
front of the existing exact `target_rref`.

For a requested target, partition same-sector columns into `target`, `allowed`
strictly descending RHS columns, and `forbidden` columns.  For a candidate row
matrix `E`, a combination cancelling all forbidden columns while retaining
the target exists at a finite-field sample precisely when

```text
rank([E_forbidden | e_target]) > rank(E_forbidden).
```

Start with authenticated translated rows incident to the target.  For a fixed
radius-limited universe, retain the affine obstruction space `C={c:F c=t}` and
fairly enumerate unseen rows incident to `target union supp(c)`.  Rows which
shrink `C` must be retained even when no single row immediately changes the
rank difference; a deterministic full-radius fallback preserves relative
completeness.  Candidate chronology is fixed by `(radius, authenticated
symmetry key, source ordinal, translation)`.  Several deterministic primes and
generic/domain samples select one fixed sparse row subset whose rank predicate
holds at every generic sample.  Null-vector supports are not compared across
samples, because they are basis-dependent when the nullspace is
multidimensional.  Only the selected row subset is lifted into the existing
exact indexed-field reducer.  Exact lift failure or a pivot guard supplies a
counterexample sample and, where appropriate, a new exceptional stratum.

Finite fields answer only *which rows to keep*.  They never authorize a rule.
Unlucky primes, denominator-zero samples, rank-changing loci, row-subset
instability, and exact-lift failure are typed experimental outcomes.

### Architecture B: seedless syzygy lowering

Behind the same target/coverage engine, construct bounded-degree
no-active-dot-raising IBP vectors and grow only the nonnegative
inactive/numerator shift level.  Solve bulk, faces, edges, and dotted strata
bottom-up.  Begin with a bounded polynomial linear ansatz using Symbolica
polynomial and matrix primitives.  Symbolica currently exposes commutative
Groebner machinery but no turnkey public general syzygy-module or
noncommutative Weyl-Groebner API; RustRed must not grow a private CAS to hide
that fact.

### Architecture C: bounded symmetry-quotiented Weyl completion

Represent an operator narrowly as `(shift, polynomial in number operators)`
and implement only the required shift commutation law.  Generate descendants
at minimal uncovered orthants, cache mother/descendant reductions, batch
critical equations in F4-like sparse elimination, and quotient them by the
authenticated target-domain stabilizer before exact algebra without discarding
independent orbit-related rows.  This can discover an all-rank rule
set globally, but it is retained only if bounded experiments beat Architecture
A.  A general noncommutative Groebner implementation is explicitly out of
scope for the first prototype.

### Architecture D: decorated graph/minor dynamic programming

This is the outer six-loop scaling layer.  Canonical decorated graph or
quadratic-form keys identify sector orbits, zero/factorized minors, simultaneous
routing witnesses, and reusable child artifacts.  Compile the minor DAG
bottom-up and cache finished rules plus sparse source/operator skeletons.
Transported warm starts are hypotheses: every transported identity is replayed
and repair speed is measured.  Even if warm starts fail, symmetry quotienting
and immutable child reuse remain valid.

## Falsification programme

### E0: replace sampled holes by exact strata

Implement the sector chart and exact orthant-complement representation.  Map
the current K=6 rules to their translation-stable structural domains, identify
all unresolved positive-dimensional strata, and probe their symmetry images
through depth 50 only as a corroborating test.  The exact decomposition, not
the probes, is the result.  E0 passes only when every guard has an exact
whole-stratum certificate or a typed unsupported-locus result, every overlap
has a consistent exact normal form, and complement construction time and
compressed size are recorded.  Structural orthants alone are never reported
as complete closure.

Existing `RuleCell` owners require an arbitrary-box union before the orthant
fast path: their assignment is `a`, their target is `a + pivot`, and inactive
chart coordinates reverse interval orientation.  Fixed restrictions and
guards remain functions of `a`.  Carrier saturation at `i64::MIN/MAX` is not
mathematical infinity unless a separately replayed asymptotic-extension witness
authorizes it.  E0 also reports a bounded cardinality for every finite
complement, because finite but astronomically large is not an acceptable
evaluation-terminal budget.

### E1: modular target-separation scout

Apply Architecture A to all six recurrence strata and a leave-one-owner-out
rediscovery corpus, with adaptive translation radius at most four.  Use at
least three fixed large primes, screened generic samples, and independent
holdouts.  Record candidate rows, columns, nonzeros and pivot fill; selected
exact rows; RHS size; coefficient degrees and sizes; guards; retries; hot
application cost; wall/CPU time; peak memory; and remaining strata.

Every accepted rule must lift and replay exactly.  Compare a complete diamond,
an exact backward-incidence frontier, that frontier with modular selection,
and the smaller Lie-generator source set.  Continue only if the whole strata
close, total time and peak memory beat the best exact target-directed baseline,
and rule quality does not regress.  A radius-four failure kills only this
bounded experiment; a K=6 success is not evidence that a fixed radius scales
to `K=21`.

### E2: seedless head-to-head

On the representative four-line sector, generate bounded-degree syzygy
sources and inactive-shift levels zero through four.  Compare total setup plus
solve cost, source count, active-dot growth, guards, RHS size, and remaining
strata against E1.  Promote this backend only if the measured end-to-end cost
wins and its boundary sequence closes.

### E3: bounded generating-function completion

First regenerate the K=3 sunset closure and its master count.  Then attempt
one K=6 four-line representative with initial operator degree at most two,
guided descendants only, and hard limits of 10,000 canonical operator terms
and three completion rounds.  Require exact path consistency and the certified
terminal count.  Hitting the cap or retaining spurious irreducibles kills this
lane for the present stage.

### E4: decorated graph/minor quotient and reuse

Canonicalize all K=6 sector masks and current roots as decorated objects and
verify every claimed transport by exact structural hashes and identity replay.
Perform topology-only dry runs at larger `K` to measure raw versus retained
classes.  Keep warm-start transport only if at least half of a child's selected
basis is reusable and compilation is at least twice as fast.

## Implementation order

1. Preserve the current generated K=6 cells as exact regression fixtures, not
   as the future closure representation.
2. Implement E0's generic nonnegative sector chart and exact coverage strata.
3. Implement E1's bounded finite-field evaluator and target-separation witness
   ahead of the existing exact reducer; do not change the artifact schema.
4. Close the six current recurrence strata and certify the three scalar
   terminals independently.  Query Vakint/MATAD offline for exact reductions
   of every proposed terminal and recurrence witness; use the relations to
   diagnose missing RustRed owners and to build independently checked MATAD
   basis maps, never as runtime reduction logic.
5. Finish the Stage 1 artifact and Vakint acceptance suite.
6. Run E2 and E3 as measured competitors.  Do not start a four- through
   six-loop artifact campaign without the separate Stage 2 authorization.

The intended breakthrough is therefore not “LiteRed in Rust.”  It is an exact
stratified compiler in which lattice geometry and modular algebra prevent most
equations from ever reaching expensive symbolic elimination, while Symbolica
remains the final CAS authority.

The independent adversarial review and its corrected gates are preserved in
[`parametric_ibp_breakthrough_audit.md`](parametric_ibp_breakthrough_audit.md).
The separate survey of primary literature through August 2026 and its five
falsifiable architecture hypotheses are preserved in
[`parametric_ibp_literature_2026.md`](parametric_ibp_literature_2026.md).
