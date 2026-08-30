# Universal nonminimal IBP closure: evidence update through August 2026

## Scope and claim discipline

This note is a fresh, bounded review of primary literature and software papers relevant to exact
parametric-IBP closure. It concentrates on results from 2015 through August 2026 and asks one
specific question:

> Does a recent method make it materially easier to certify a finite, universal, but not
> necessarily minimal terminal set for the equal-mass vacuum family at `K = 21`?

Here `K = L(L + 1) / 2` is the complete scalar-product coordinate count of an `L`-loop vacuum
family. Thus `K = 6`, `10`, `15`, and `21` are the three- through six-loop pressure points.

Statements under **Published result** are claims or limitations stated in the cited papers.
Statements under **RustRed inference** or **RustRed proposal** are conclusions drawn for this
project; the cited authors do not claim a practical six-loop RustRed compiler.

Master minimality is relaxed, not correctness. A finite list left by a finite-box experiment is
not a universal terminal set. A stable modular rank, a critical-point count, or agreement on
sampled reductions is evidence, not an all-rank closure certificate.

## Executive verdict

No reviewed paper demonstrates exact, all-rank, equal-mass vacuum closure at `K = 21`. The
earlier architecture ranking therefore remains unchanged:

1. direct physical-stratum completion, supplied by low-degree Baikov logarithmic sources;
2. generating-function or seedless symbolic lowering feeding the same exact verifier;
3. intersection/action-matrix methods as terminal diagnostics and target projectors;
4. full signature, Janet, or Ore completion as a high-risk fallback; and
5. generic GKZ or D-module restriction as a theory lane, not the first production compiler.

Recent work nevertheless changes four implementation priorities:

- **Specialize before certification.** Equal masses can create cross-sector IBP relations that a
  generic or sector-local count misses. Physical-locus rank must not be inferred by generic
  specialization alone.
- **Do not make cut stitching authoritative.** NeatIBP 1.1 reports that cut reductions can be
  inconsistent, with massive-propagator examples more prone to the problem.
- **Certify epsilon debt, not only descent.** Local-ring pivoting is a useful construction tool,
  but a finite-system singularity-free basis does not by itself bound all-rank recurrence debt.
- **Use branch intersection theory as a measured diagnostic.** Its `3L - 3` variable bound is
  interesting, but the published implementation is demonstrated at two loops and requires
  recursive bases for sub-branches.

The strongest composite remains:

```text
ordinary physical-family IBPs and closed-form logarithmic sources
        -> modular candidate discovery or generating-function guidance
        -> exact source provenance and physical guard stratification
        -> exact all-rank coverage and deterministic pointwise ownership
        -> uniform epsilon-debt certificate
        -> finite universal terminal table evaluated offline
```

## What exactly has to be certified

There are two valid, distinct certificate styles. RustRed should not mix their obligations.

### Descending rewrite certificate

Let `T` be a finite set of terminal integral keys. `T` may contain exact linear dependencies. A
rewrite artifact is universal if it proves all of the following:

1. every nonterminal lattice point belongs to at least one complete rule domain;
2. coefficient guards, sector boundaries, zero sectors, factorizations, and symmetries are
   covered exactly;
3. every accepted rule replays into an exact combination of regenerated ordinary IBP sources;
4. every application strictly decreases a declared well-founded order;
5. deterministic precedence is defined where rule domains overlap;
6. every selected owner and its canonicalized children retain the same typed family routes; and
7. lower-sector dependencies form an acyclic, already-certified graph.

This certificate does **not** require minimizing `T` or finding exact relations among its members.
Vakint may ship a separately generated numerical Laurent value for every member of `T`.
Redundant terminals cost evaluation and storage, but do not invalidate the rewrite proof.
Two legal paths need not produce identical raw coordinates in a redundant terminal spanning set.
Overlap equality is a diagnostic unless an independently certified quotient/action presentation is
claimed; otherwise compare only after an exact terminal map or independent numerical evaluation.

### Finite quotient or action certificate

An action-matrix compiler instead starts from a finite frame `O` and an exact relation module
`R`. It must prove that each shift action is well defined on `span(O) / R`, that the complete
first border closes, that the actions obey the relevant commutative or Ore relations, and that
the original IBP module acts as zero. If `O` is redundant, exact presentation of `R` is mandatory.

A Macaulay row-space hit at one degree is not enough unless the reconstructed multipliers, border
closure, physical restriction, and relation-module invariance are all checked exactly.

### Count diagnostics are not completion

Smirnov and Petukhov prove that the number of masters for a fixed graph is finite
([arXiv:1004.4199](https://arxiv.org/abs/1004.4199)). Lee and Pomeransky relate the number to
proper critical points and Milnor numbers
([arXiv:1308.6676](https://arxiv.org/abs/1308.6676)). Bitoun, Bogner, Klausen, and Panzer derive
an Euler-characteristic formulation using holonomic D-modules
([arXiv:1712.09215](https://arxiv.org/abs/1712.09215)). These establish finiteness and valuable
reference ranks; they do not construct a descending parametric rule artifact.

There is a further qualification. The momentum-IBP annihilator module is contained in the full
parametric annihilator, and arXiv:1712.09215 explicitly leaves equality after localization as an
open question. Consequently, a parametric Euler rank is a lower-bound or discrepancy diagnostic
for an ordinary-IBP quotient unless equality is proved for the family under study.

For a proposed nonminimal `T`:

- `|T|` below a rigorously applicable physical quotient rank falsifies the artifact;
- `|T|` above that rank merely records redundancy;
- equality of counts does not prove coverage; and
- a rank measured in generic masses or on isolated sectors may be wrong on the unit-mass locus.

Only exact coverage distinguishes a finite complement from finite-box misses.

## Baikov logarithmic sources and module intersections

### Published result

Boehm et al. give an explicit complete generating set of logarithmic vector fields along the Gram
determinant, valid for arbitrary loop and external-leg counts
([arXiv:1712.09737](https://arxiv.org/abs/1712.09737)). These syzygies avoid dimension shifts and
have a closed construction from Laplace expansion. Completeness here concerns the logarithmic
vector-field module, not a terminating recurrence orientation on the integer index lattice.

Module-intersection IBPs can control doubled propagators and reduce equation sizes. The complete
non-planar hexagon-box calculation reduced degree-four numerator targets to 73 masters
([arXiv:1805.01873](https://arxiv.org/abs/1805.01873)). That is an exact, impressive bounded
target reduction, but it is not an all-rank closure theorem.

NeatIBP 1.0 generates compact target systems using syzygies and module intersections and
parallelizes by sector ([arXiv:2305.08783](https://arxiv.org/abs/2305.08783)). NeatIBP 1.1 adds
spanning cuts and maximal-cut generator selection
([arXiv:2502.20778](https://arxiv.org/abs/2502.20778)). The latter paper records several limits:

- the spanning-cut consistency check is numerical over a finite field;
- consistency is not always guaranteed, and massive diagrams are reported as more prone to
  inconsistencies;
- cutting sectors before reduction can create inconsistent truncated relations;
- the usual no-higher-seed assumption has counterexamples;
- the multiple-propagator layer simplification assumes lower layers reduce as expected; and
- generators deleted as redundant can still act as useful seeding catalysts.

### RustRed inference

The closed-form logarithmic vectors remain the best source preconditioner in the review. Their
low degree is especially attractive at `L = 6`, where there are only `L^2 = 36` base momentum
IBP directions. They must still feed RustRed's exact ordinary-source replay and coverage engine.

Spanning cuts may discover rules or partition finite linear algebra. They must not define the
artifact's mathematical ownership. The equal-mass vacuum target sits precisely in the massive
regime for which the NeatIBP authors report more frequent inconsistency.

## Finite fields, reconstruction, and block systems

### Published result

FiniteFlow composes finite-field calculations as dataflow graphs and supports massively parallel
functional reconstruction ([arXiv:1905.08019](https://arxiv.org/abs/1905.08019)). FireFly gives
multivariate rational reconstruction
([arXiv:1904.00009](https://arxiv.org/abs/1904.00009)), and Kira 2 integrates finite-field
reconstruction of final coefficients
([arXiv:2008.06494](https://arxiv.org/abs/2008.06494)). Blade constructs target-specific
block-triangular systems with far fewer equations than plain IBP systems
([arXiv:2405.14621](https://arxiv.org/abs/2405.14621)).

These methods reduce expression swell and expose prime, sample, block, and sector parallelism.
Their published reductions begin from finite target systems. They do not prove that a chosen
translation degree, seed region, or terminal list covers every rank.

### RustRed inference

Modular arithmetic is an accelerator, not a relaxation of proof. RustRed may use modular rank,
pivot patterns, and black-box solves to discover candidates. Promotion requires rational
reconstruction followed by exact source replay. At least one fresh prime and parameter point
should be withheld from discovery, but even successful withheld checks remain tests rather than
coverage certificates.

Bad primes and specialized parameter points can hide a rank defect. Coefficient-zero loci in the
integer indices are separate exact guard strata; random finite-field sampling cannot discharge
them. Workers should share immutable sparse structure and distribute modular values, rather than
copy a symbolic matrix per prime or per thread.

## Generating functions and symbolic lowering

### Published result

Guan, Li, and Ma encode numerator powers in generating functions and construct closed
differential systems ([arXiv:2306.02927](https://arxiv.org/abs/2306.02927)). In their two-loop
double-pentagon example, memory for each differential-system solve is independent of target rank,
but the number of generating-direction samples grows with rank, and the closed systems are built
using Blade reductions.

The 2026 generating-function algorithm converts sector IBPs to Weyl-algebra equations, iterates
descendant generation and rule extraction, and analyzes the complement of rule-leading orthants
([arXiv:2605.09541](https://arxiv.org/abs/2605.09541)). This is the closest published analogue of
RustRed's desired structural coverage geometry. The paper does not demonstrate six-loop closure,
and orthant coverage alone does not classify zeros of symbolic leading coefficients.

Seedless reduction constructs bulk, face, edge, and dotted-boundary lowering problems rather than
starting from a large Laporta seed box
([arXiv:2602.22111](https://arxiv.org/abs/2602.22111)). Syzygy-constrained symbolic reduction also
targets small generic-index neighborhoods
([arXiv:2507.11140](https://arxiv.org/abs/2507.11140)). Neither paper supplies a universal
six-loop degree or source bound.

### RustRed inference

These papers reinforce, rather than replace, the top-ranked hybrid. Use their descendant, face,
and source-selection strategies to propose relations. Reconstruct each accepted relation as an
exact combination of ordinary sources, then let guard-aware physical-lattice coverage decide
whether it contributes to closure.

A generating function can package arbitrary rank, but a finite differential system is not yet a
discrete, strictly descending rule artifact. The compiler must still prove orientation,
coefficient applicability, lower-sector closure, and finite terminal complement.

## Finite quotients, Pfaffian systems, and action matrices

### Published result

The Macaulay-matrix construction derives Pfaffian matrices from a holonomic differential ideal
and a standard-monomial basis
([arXiv:2204.12983](https://arxiv.org/abs/2204.12983)). Restrictions of Pfaffian systems treats
singular limits with D-module restriction and offers gauge/Moser and Macaulay algorithms
([arXiv:2305.01585](https://arxiv.org/abs/2305.01585)). Companion tensor algebra accelerates
intersection-number projection and has been demonstrated on two-loop five-point massless
functions ([arXiv:2408.16668](https://arxiv.org/abs/2408.16668)).

These methods are constructive once an appropriate finite cohomology or standard-monomial basis
is available. They do not make a guessed frame spanning merely because its first sampled border
happens to reduce.

### RustRed inference

A finite-frame Macaulay pilot remains worthwhile at `K = 6`. Its result becomes authoritative
only after exact relation-module and border certificates on the physical equal-mass family.
Companion matrices and black-box action recovery are efficient ways to discover or apply a
quotient; they are not a substitute for proving which quotient was recovered.

Physical restriction must be part of the construction, not an unchecked final substitution. A
generic Pfaffian rank can fall on an equal-mass locus, and denominators used in a generic action
matrix can vanish there.

## Intersection theory: useful projection, incomplete authority

### Published result

Fontana and Peraro give a rational polynomial-series algorithm for intersection numbers and a
finite-field proof-of-concept at one and two loops
([arXiv:2304.14336](https://arxiv.org/abs/2304.14336)). Their basis-selection procedure may start
from an overcomplete list, build an enlarged intersection metric, and select independent rows and
columns by finite-field linear algebra. This proves independence inside a list already assumed to
span; it does not prove that the list spans every integral.

Relative cohomology in the Feynman parametrization represents pinches as boundary-supported forms
([arXiv:2411.05226](https://arxiv.org/abs/2411.05226)). Its equal-mass, light-like bubble example
is directly relevant: two subsector tadpoles become related by IBP only when the top sector and
both subsectors are considered together. The paper calls these cross-sector effects magic
relations and warns that sector-by-sector dimension counting becomes subtle.

The 2026 branch representation reduces an `L`-loop intersection calculation to at most `3L - 3`
branch variables ([arXiv:2604.05025](https://arxiv.org/abs/2604.05025)). Fixed-branch integrals
have a one-loop-like reduction, while outer layers use intersection numbers. The paper also
requires separate bra and ket bases when pinching an entire branch creates a sub-branch. Its
examples are at two loops; higher-loop optimized implementations are left for future work.

Generalized loop-by-loop Baikov spaces have an additional risk. Feynman integrals can span only a
proper FI-subspace of the generalized cohomology, so critical-point counts may overcount physical
integrals ([arXiv:2202.08127](https://arxiv.org/abs/2202.08127)). Identifying that FI-subspace can
itself require an independent reduction method.

### RustRed inference

At six loops, the branch bound is 15 variables rather than 21. This is a meaningful research
reduction, but 15 recursive layers with growing intermediate bases can still be large. More
importantly, the method currently projects selected integrals after suitable bases and sub-branch
data have been constructed; it does not emit RustRed's all-rank discrete closure certificate.

Intersection metrics should therefore serve three roles initially:

1. detect dependencies inside a proposed terminal frame;
2. compare target projections with the rewrite compiler at `K = 6`; and
3. expose physical rank changes and cross-sector relations.

They should not certify universal span without an independent border or rewrite proof.

## Uniform epsilon-pole debt

### Published result

Singularity-free bases can be constructed by sequential four-dimensional projection or Gaussian
elimination over the local ring that forbids division by epsilon
([arXiv:2508.04394](https://arxiv.org/abs/2508.04394)). The paper starts from IBP relations for
all integrals in a given finite process system and demonstrates planar and nonplanar two-loop
double boxes. It proves a strong property for the supplied finite matrix, not an all-rank
parametric recurrence.

An epsilon-finite basis was constructed earlier for four-loop tadpoles
([arXiv:hep-ph/0601165](https://arxiv.org/abs/hep-ph/0601165)). Quasi-finite bases instead use
dimension shifts and higher propagator powers
([arXiv:1411.7392](https://arxiv.org/abs/1411.7392)); those auxiliary dimensions are unsuitable as
unexamined production terminals for the fixed-dimensional RustRed family.

### RustRed proposal

Attach an epsilon valuation `v_epsilon(c)` to every exact rule coefficient. Define pole debt on an
edge as `max(0, -v_epsilon(c))`. Strict descent makes every individual reduction finite, but the
path length grows with input rank. It therefore does not imply a rank-independent debt bound.

For every recurrent all-rank rule cell, prefer a local-ring pivot for which all right-hand-side
coefficients have nonnegative epsilon valuation. If a recurrent stratum can be traversed
arbitrarily often and adds positive pole debt, reject it unless an exact potential proves that
the number of such traversals is uniformly bounded.

After quotienting recurrent cells, form a finite directed stratum graph. Exceptional bands,
sector transitions, and terminal edges may carry debt. A valid certificate supplies a constant
`B_epsilon` bounding the maximum accumulated edge weight on every path. A positive-weight cycle
that corresponds to arbitrarily repeatable rank descent falsifies uniform boundedness.

Index values at which an epsilon-regular pivot vanishes must become exact guard strata. A random
generic-index pivot is not enough. The artifact should record `B_epsilon`, so an evaluator asking
for Laurent order `p` knows that terminal data through at least `p + B_epsilon` are required.

This graph certificate is a RustRed proposal. The singularity-free-basis paper motivates
local-ring pivot selection but does not state this all-rank extension.

## Avoiding auxiliary-family circularity

### Discovery boundary

Syzygies, cuts, generalized Baikov spaces, intersection projections, Pfaffian restrictions, and
external reducers may all propose relations. A production RustRed relation is accepted only if
it replays into ordinary IBP sources of the exact physical family, plus explicitly certified
symmetry, zero, and factorization identities.

Relations whose derivation uses an auxiliary family are not automatically invalid. They become
valid only when the final same-family provenance is reconstructed. A projection that identifies
the FI-subspace by calling another reducer is a discovery aid, not an independent closure proof.

### Terminal-evaluation boundary

AMFlow 2.0 improves recursion modes and its numerical differential-equation solver
([arXiv:2607.08477](https://arxiv.org/abs/2607.08477)). It still requires IBP reduction of targets
and construction of master differential equations. Its published benchmark evaluates 316
three-loop five-point masters and reports substantial symbolic costs with Blade or Kira.
`SkipReduction` helps when supplied targets are already known masters; it does not remove the IBP
work needed for auxiliary differential systems.

It is acceptable to evaluate every RustRed terminal offline with MATAD, FMFT, AMFlow, or another
trusted workflow and ship the resulting Laurent tables in Vakint. FORM-derived recurrence
equations must never enter RustRed. An exact map to the MATAD master basis is optional: when the
bases differ, high-precision numerical Laurent parity may be the sole cross-backend parity gate.
Exact RustRed source replay and universal coverage remain mandatory independently of that gate.

To avoid evaluation circularity, terminal promotion must include an executable plan listing:

- every universal terminal and requested Laurent depth;
- the certified `B_epsilon` surcharge;
- auxiliary families and coupled-system dimensions used by the evaluator;
- which independent reducer constructs each differential system; and
- measured CPU, memory, and precision scaling on a representative batch.

If AMFlow uses RustRed itself to close an auxiliary system, that can be a future production use,
but it cannot simultaneously serve as independent validation of the same closure artifact.

## Updated roles and ranking

- **Baikov logarithmic vectors:** remain the first-ranked low-degree source basis, but become
  authoritative only after exact replay and coverage.
- **Generating functions:** remain second as descendant and orthant-candidate generators; the common
  verifier remains authoritative.
- **Seedless and syzygy lowering:** remain the third discovery lane for boundary-aware rules.
- **Modular and block solvers:** remain accelerators for sparse solving and reconstruction, not
  closure authorities by themselves.
- **Branch intersection theory:** merits a `K = 6` target-projection and rank-diagnostic pilot, but
  is not yet an all-rank artifact compiler.
- **Macaulay and action matrices:** remain a conditional finite-frame pilot; a full presentation
  proof could make this lane authoritative.
- **Cut stitching:** is downgraded to finite-system partitioning and discovery for massive families.
- **Full Ore or Janet completion:** remains a potentially authoritative but high-risk fallback.
- **Generic GKZ and D-module methods:** remain a count, restriction, and theory lane until physical
  provenance is established.

The 2026 branch and local-ring results improve components, not the winning closure architecture.
The physical, guard-aware verifier remains the authority shared by every discovery lane.

## Falsifiable experiment ladder

Every experiment must pre-register wall-time, RAM, row-count, terminal-count, and epsilon-debt
budgets. Increasing a budget after seeing failure is a new experiment, not a pass.

### Gate A: `K = 6` exact discrimination

Use the K4/Mercedes parent, all four inequivalent contractions, and every current exceptional or
numerator-ray obligation.

1. Generate the nine ordinary momentum sources and the logarithmic-vector candidates. Replay every
   promoted rule exactly into ordinary sources.
2. Construct at least one generic-mass diagnostic system and one unit-mass physical system. Kill
   any lane that silently specializes a singular denominator or misses a physical rank drop.
3. Compare full uncut relations with spanning-cut reconstruction on the massive family. Any exact
   mismatch disqualifies cuts as an authority, even if sampled reductions agree.
4. Prove guard-aware coverage of the entire lattice complement. Test ranks through at least 16,
   but label this only regression evidence beside the symbolic proof.
5. Remove one source, one boundary rule, and one guard branch in separate negative controls. The
   verifier must expose a nonempty uncovered region or an invalid selected owner.
6. Build an overcomplete intersection metric for the proposed terminal set. Check that its rank
   diagnoses known dependencies, while deliberately omitting a spanning candidate demonstrates
   that metric rank alone cannot assert universal span.
7. Compute the recurrent stratum graph and `B_epsilon`. A repeatable positive-debt cycle kills the
   rule orientation even when every tested rank terminates.
8. Produce the offline evaluation manifest and evaluate a nontrivial terminal batch to the required
   order plus `B_epsilon`.

Pass condition: exact source replay, zero uncovered symbolic regions, deterministic precedence,
strict descent, finite `T`, constant `B_epsilon`, and an executable terminal-value plan. Raw overlap
normal forms are required only for a separately claimed quotient/action presentation.

### Gate B: `K = 10` scaling and physical-stratum test

Use both a trivalent parent with one ISP and a banana-like family with many ISPs. Permit completion
degree or translation radius at most three in the first registered run.

Record source rows, columns, nonzeros, modular primes, fill, reconstruction degree, certificate
support, guard strata, terminals, peak RAM, bytes read, and parallel efficiency. Run generic-mass
and equal-mass modular ranks at several fresh points, then reconstruct every claimed physical rank
change exactly.

The lane fails this gate if it needs an unexplained increase in completion degree, loses compact
ordinary-source provenance, leaves a sampled-only complement, or produces terminals whose batch
evaluation exceeds the pre-registered offline budget. Successful reductions through rank 24 are
required regression tests but are not a substitute for the symbolic certificate.

### Gate C: `K = 15` six-loop design kill

Use a 12-propagator/three-ISP trivalent parent and a six-propagator/nine-ISP banana proxy. Begin
with source generation, leading geometry, modular rank, and border membership at degree three.
Do not launch an open-ended completion.

For the branch-intersection pilot, measure every recursive layer dimension, sub-branch count,
largest sparse system, and accumulated basis storage. The variable bound of 12 at five loops is
not a pass if intermediate bases or physical-boundary systems explode.

Kill a candidate before `K = 21` if any of the following occurs:

- completion degree grows without a structural stopping certificate;
- reconstructed source multipliers densify beyond the declared storage budget;
- a generic quotient changes rank on the physical locus without exact restriction support;
- the terminal frame or coupled evaluation system exceeds the one-off evaluation budget;
- `B_epsilon` grows with tested rank or a repeatable positive-debt cycle appears; or
- a generalized or auxiliary space cannot be projected to the physical FI-subspace without
  importing the relation system being certified.

Only a lane surviving Gate C with measured headroom should be extrapolated to `K = 21`.

## Recommended implementation sequence

1. Keep the current ordinary-source, sector, symmetry, and guard semantics as the proof boundary.
2. Add closed-form Baikov logarithmic sources and compare their exact span with ordinary sources at
   `K = 6`.
3. Run generating-function and seedless selection as competing candidate producers.
4. Add local-ring-aware pivot scoring and the finite stratum-graph epsilon certificate.
5. Pilot physical-locus Macaulay border closure and branch-intersection projection at `K = 6`.
6. Promote no rule, terminal frame, or auxiliary relation without exact same-family provenance.
7. Advance only the lanes that pass the shared `K = 6`, `10`, and `15` kill gates.

This sequence allows recent methods to improve discovery and arithmetic without weakening the one
property RustRed must own: an exact, universal, finite closure certificate.

## Primary sources

- A. V. Smirnov and A. V. Petukhov, *The number of master integrals is finite*,
  [arXiv:1004.4199](https://arxiv.org/abs/1004.4199).
- R. N. Lee and A. A. Pomeransky, *Critical points and number of master integrals*,
  [arXiv:1308.6676](https://arxiv.org/abs/1308.6676).
- T. Bitoun, C. Bogner, R. P. Klausen, and E. Panzer, *Feynman integral relations from parametric
  annihilators*, [arXiv:1712.09215](https://arxiv.org/abs/1712.09215).
- J. Boehm et al., *Complete sets of logarithmic vector fields for integration-by-parts identities
  of Feynman integrals*, [arXiv:1712.09737](https://arxiv.org/abs/1712.09737).
- J. Boehm et al., *Complete integration-by-parts reductions of the non-planar hexagon-box via
  module intersections*, [arXiv:1805.01873](https://arxiv.org/abs/1805.01873).
- T. Peraro, *FiniteFlow: multivariate functional reconstruction using finite fields and dataflow
  graphs*, [arXiv:1905.08019](https://arxiv.org/abs/1905.08019).
- J. Klappert et al., *FireFly: reconstructing rational functions from black box evaluations*,
  [arXiv:1904.00009](https://arxiv.org/abs/1904.00009).
- J. Klappert et al., *Integral reduction with Kira 2.0 and finite field methods*,
  [arXiv:2008.06494](https://arxiv.org/abs/2008.06494).
- X. Xu et al., *Baikov representations, intersection theory, and canonical Feynman integrals*,
  [arXiv:2202.08127](https://arxiv.org/abs/2202.08127).
- V. Chestnov et al., *Macaulay Matrix for Feynman Integrals: Linear Relations and Intersection
  Numbers*, [arXiv:2204.12983](https://arxiv.org/abs/2204.12983).
- G. Fontana and T. Peraro, *Reduction to master integrals via intersection numbers and polynomial
  expansions*, [arXiv:2304.14336](https://arxiv.org/abs/2304.14336).
- V. Chestnov et al., *Restrictions of Pfaffian Systems for Feynman Integrals*,
  [arXiv:2305.01585](https://arxiv.org/abs/2305.01585).
- Z. Wu et al., *NeatIBP 1.0, a package generating small-size integration-by-parts relations for
  Feynman integrals*, [arXiv:2305.08783](https://arxiv.org/abs/2305.08783).
- X. Guan, X. Li, and Y.-Q. Ma, *Exploring the linear space of Feynman integrals via generating
  functions*, [arXiv:2306.02927](https://arxiv.org/abs/2306.02927).
- X. Guan et al., *Blade: A package for block-triangular form improved Feynman integrals
  decomposition*, [arXiv:2405.14621](https://arxiv.org/abs/2405.14621).
- G. Brunello, V. Chestnov, and P. Mastrolia, *Intersection Numbers from Companion Tensor
  Algebra*, [arXiv:2408.16668](https://arxiv.org/abs/2408.16668).
- M. Lu, Z. Wang, and L. L. Yang, *Intersection theory, relative cohomology and the Feynman
  parametrization*, [arXiv:2411.05226](https://arxiv.org/abs/2411.05226).
- Z. Wu et al., *Performing Integration-by-Parts Reductions Using NeatIBP 1.1 + Kira*,
  [arXiv:2502.20778](https://arxiv.org/abs/2502.20778).
- S. Smith and M. Zeng, *Feynman Integral Reduction using Syzygy-Constrained Symbolic Reduction
  Rules*, [arXiv:2507.11140](https://arxiv.org/abs/2507.11140).
- S. De Angelis et al., *Singularity-Free Feynman Integral Bases*,
  [arXiv:2508.04394](https://arxiv.org/abs/2508.04394).
- L. de la Cruz and D. A. Kosower, *Seedless Reduction of Feynman Integrals*,
  [arXiv:2602.22111](https://arxiv.org/abs/2602.22111).
- L.-H. Huang et al., *Feynman integral reduction with intersection theory made simple*,
  [arXiv:2604.05025](https://arxiv.org/abs/2604.05025).
- B. Feng et al., *An Algorithm for the Symbolic Reduction of Multi-loop Feynman Integrals via
  Generating Functions*, [arXiv:2605.09541](https://arxiv.org/abs/2605.09541).
- R.-J. Huang, X. Liu, and Y.-Q. Ma, *AMFlow 2.0: significant algorithmic and software
  improvements for Feynman integral evaluation*,
  [arXiv:2607.08477](https://arxiv.org/abs/2607.08477).
- K. G. Chetyrkin et al., *Epsilon-finite basis of master integrals for the integration-by-parts
  method*, [arXiv:hep-ph/0601165](https://arxiv.org/abs/hep-ph/0601165).
- A. von Manteuffel, E. Panzer, and R. M. Schabinger, *A quasi-finite basis for multi-loop
  Feynman integrals*, [arXiv:1411.7392](https://arxiv.org/abs/1411.7392).
