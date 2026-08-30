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
    -> modular signature traces carrying source cofactors
    -> Janet-like complementary decomposition and explicit open obligations
    -> local simplex-face or tube discovery on those obligations
    -> optional action recovery only after exact finite-frame certification
    -> exact rational, guard, sector, and shift-algebra replay
    -> epsilon-valuation certificate and measured numerical-master campaign
```

The central change is that modular discovery must identify a rule by its source provenance, not by
a pivot pattern alone. Janet-like complements provide the global coverage authority. Seedless and
tube methods propose compact local rules, while block-Krylov or FGLM methods may compress an
already proved finite quotient when direct terminal evaluation is too expensive. None of those
discovery or compression methods may replace exact replay.

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

## Architecture delta by component

| component | accepted role | forbidden inference |
| --- | --- | --- |
| Janet-like complement | exact owner of uncovered orthants and prolongation obligations | commutative coverage automatically proves guards or shift-algebra closure |
| signature and source cofactor | stable modular identity and exact replay certificate | agreement of pivots or signatures across primes proves termination |
| seedless/simplex-face and tube systems | compact, parallel candidate-rule discovery | a closing finite collection of tested paths proves all-rank closure |
| CALICO annihilators | physical-stratum source enrichment | a chosen degree/order or numerical filter proves a complete source module |
| block-Krylov, Scalar-FGLM, or Hermite recovery | compress a previously certified finite quotient | stable modular action matrices prove a finite quotient |
| local-ring elimination | avoid bad epsilon pivots and refine a basis | finite-system regularity proves uniform recurrence pole depth |
| AMFlow | evaluate a certified independent terminal quotient offline | terminal count alone predicts auxiliary-system cost |

## Exact implementation sequence

1. Generate ordinary physical-family IBPs and selected low-order Schwinger or
   Lee--Pomeransky annihilators. Preserve exact ordinary-source or annihilator-source cofactors.
2. Build a Janet-like leading-shift complement for each sector chart and exact guard stratum.
   Positive-dimensional standard pairs are queued obligations, never implicit terminals.
3. Attack each obligation with simplex-face and sparse-tube searches. Modular workers share one
   immutable symbolic trace and return signature-keyed coefficients and source cofactors.
4. Lift candidates exactly and replay them over the rational coefficient field before admitting
   their leading classes into the complement.
5. Once every remaining complement component is finite, either retain and evaluate the complete
   affordable typed terminal set, or—only when compression is useful—construct its exact relation
   module before recovering shift actions with block-Krylov, FGLM, or Hermite methods.
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

- Require stable candidate rank and support across at least three good primes and held-out rational
  `D` points, followed by exact lift and replay.
- Run at least two larger outer-frame controls. They are falsification tests only; exact complement
  and prolongation exhaustion remain the proof.
- Reject an implementation whose normal worker exceeds roughly 32 GiB before K15, unless a clear
  chunked representation removes the replicated state.

### `K = 15`

This is the mandatory design gate before K21.

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
