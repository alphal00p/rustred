# TIDE five-loop census and a reproducible RustRed programme

Status: research and offline input specification, 20 September 2026. This document
does **not** report a completed RustRed five-loop family, establish minimality of
any terminal basis, or establish coverage of the census by a particular campaign.
The concrete momenta and sector numbers below belong in input manifests and
regression data, never in the production solver's strategy selection.

## 1. Sources and the main conclusions

The supplied local `FOR_REFERENCE_ONLY_DO_NOT_PUSH/TIDE_5loops.pdf` is Thomas
Luthe's August 2015 dissertation, *Fully massive vacuum integrals at 5 loops*,
147 PDF pages. A public copy is available from the
[Bielefeld thesis repository](https://noah.nrw/ubbihs/download/pdf/5131192).
Printed page numbers in this document refer to the dissertation; add four for
one-based PDF page numbers. The particularly relevant parts are §§2.1–2.4,
5.1–5.3, 6.2, 8.1, 9, and Appendices A–C. The source PDF remains reference-only.

The concrete findings are:

1. One 15-denominator auxiliary family covers the thesis's fully massive
   single-scale five-loop classification. Fifteen is the number of independent
   scalar products, **not** the number of physical edges of a parent graph.
2. There are four inequivalent 12-line parent sectors in that classification:
   **32745, 31740, 30699, and 30527**. Their denominator definitions and precise
   bit convention are given below. The 2016 follow-up independently identifies
   these four parents in its Figure 1 and §3.
3. The full published classification has **67 integral topologies**, of which
   **48 do not factorize** and **19 factorize**. These are not 67 distinct
   12-line parents and not 67 distinct labelled sectors.
4. The thesis did not finish the 12-line numerical evaluations. Completing all
   four parents is therefore a stronger objective than merely reproducing the
   thesis's completed numerical results.
5. Its most immediately useful performance ideas are modular elimination-trace
   pruning, deferred lower-sector work, avoiding high-order scalar decoupling,
   and cheap modular searches for a better basis. They are all structural ideas,
   not five-loop-specific algorithms.

The independent follow-ups are Luthe and Schröder,
[Fun with higher-loop Feynman diagrams](https://arxiv.org/pdf/1604.01262)
(2016), and
[Five-loop massive tadpoles](https://arxiv.org/pdf/1609.06786)
(2016). The latter reports 63 sectors with at most 11 lines and 103 masters,
and four missing 12-line sectors with nine masters. Its result count should not
be silently substituted for the different basis count in the thesis.

## 2. What family, topology, sector, and master mean here

In the thesis, the Euclidean integral is

\[
 I(a_1,\ldots,a_{15})=
 \int\prod_{r=1}^{5}\frac{d^d k_r}{\pi^{d/2}}
 \prod_{i=1}^{15}(q_i^2+1)^{-a_i}.
\]

A positive exponent represents an actual denominator; zero removes that
denominator; a negative exponent represents a numerator factor in the chosen
auxiliary basis. There need not be a physical graph with all 15 denominators
present. An input that sets all 15 exponents positive is not one of the physical
12-line parent sectors.

For generic loop count, the scalar-product dimension is `L*(L+1)/2` for a vacuum
family; a connected trivalent graph with no external legs has `3*(L-1)` edges
and `2*(L-1)` vertices, for `L > 1`. These formulas are descriptions of an input,
not permission to special-case a particular `L` in RustRed.

The thesis calls two graphs the same *topology* when the corresponding integral
families are related by an admissible loop-momentum transformation. It gives an
example of distinct graphs representing the same integral topology in Figure
8.2, p.87. Therefore graph isomorphism is a valuable first filter, but a graph
canonical label by itself is not the complete integral-equivalence witness.
The final routing must act simultaneously on denominators and numerators;
numerator scalar products can transform into sums rather than permutations.

A *master integral* is an element of a chosen reduction basis. A *zone* in TIDE
additionally singles out a propagator whose exponent is the symbolic variable
`x`; its coupled difference-equation basis is not the same object as the final
ordinary-integer master basis. Its recurrence order must not be counted as a
RustRed terminal count. See thesis pp.43–53.

The four H/X/BMW/FG workloads used in RustRed's four-loop comparison are a
practical delivery/benchmark decomposition. The coincidence that the published
five-loop classification also has four maximal 12-line representatives does not
establish that the two decompositions have the same definition of “parent”.
Indeed, the thesis's four-loop census has two nine-line representatives, 1022
and 511 (Table B.1), as well as lower-line topologies.

## 3. Exact auxiliary momenta and parent inputs

Table A.1, printed p.111, specifies the following ordered list. Every denominator
in the reference is `D_i = q_i^2 + 1`.

| Slot | Momentum | Slot | Momentum | Slot | Momentum |
|---:|---|---:|---|---:|---|
| 1 | `k1` | 6 | `k1-k3` | 11 | `k2-k5` |
| 2 | `k2` | 7 | `k1-k4` | 12 | `k3-k5` |
| 3 | `k3` | 8 | `k1-k5` | 13 | `k4-k5` |
| 4 | `k4` | 9 | `k2-k3` | 14 | `k1+k2-k4` |
| 5 | `k5` | 10 | `k2-k4` | 15 | `k3-k4` |

The unusual fourteenth momentum is essential. This list is **not** simply the
five loop momenta plus all ten pairwise differences. It supplies a common
classification which includes all four published parents. The squared momenta
span the 15 scalar products: the five diagonal terms and nine available pair
differences determine nine mixed terms, and slot 14 then determines the missing
`k1.k2` term.

Equation (2.6), p.13, fixes the **big-endian** sector label:

\[
 \operatorname{sID}(a)=\sum_{i=1}^{15}[a_i>0]\,2^{15-i}.
\]

Thus the first printed bit refers to slot 1. Do not import these integers into
an API using least-significant-bit-first labels without conversion.

| Parent sID | Positive-denominator bit string, slots 1→15 | Slots absent from the parent | Thesis's selected masters in that parent |
|---:|---|---|---:|
| 32745 | `111111111101001` | 11, 13, 14 | 1 |
| 31740 | `111101111111100` | 5, 14, 15 | 2 |
| 30699 | `111011111101011` | 4, 11, 13 | 1 |
| 30527 | `111011100111111` | 4, 8, 9 | 5 |

The bits and masters are transcribed from Tables B.2–B.3, pp.115–116; the nine
master total is also reported in the 2016 update. Each root has twelve positive
slots and three auxiliary slots. The latter must remain available for arbitrary
numerators, but they are not to be turned into physical denominators by the
offline parent manifest.

RustRed's existing example convention may instead use `q_i^2-1`. Such an input
can be useful for a reduction benchmark, but is not numerically interchangeable
with the thesis's Euclidean values without an explicit Wick-rotation/measure
and exponent-dependent sign conversion. A numerical reproduction must freeze
that conversion rather than compare unadjusted decimals.

### Explicit graph witnesses for the four roots

The following is an independently derived offline check of the transcription,
not a new graph algorithm in RustRed. For each parent, there are eight vertices;
each row below gives their incident signed momentum slots. At every vertex the
signed vector sum is zero, and every physical slot occurs twice with opposite
sign. The graph is connected, cubic, and has `12-8+1=5` loops.

```text
32745:
 (1,-4,-7) (-1,5,8) (2,-3,-9) (-2,4,10)
 (3,-5,-12) (-6,7,-15) (6,-8,12) (9,-10,15)

31740:
 (1,-3,-6) (-1,4,7) (-2,3,9) (2,-4,-10)
 (6,-8,12) (-7,8,-13) (-9,11,-12) (10,-11,13)

30699:
 (1,-5,-8) (-1,-10,14) (-2,3,9) (2,7,-14)
 (-3,5,12) (6,-7,15) (-6,8,-12) (-9,10,-15)

30527:
 (1,-3,-6) (-1,-10,14) (-2,5,11) (2,7,-14)
 (3,-5,-12) (6,-7,15) (10,-11,13) (12,-13,-15)
```

For example, the first vertex of 30699 means
`k1-k5-(k1-k5)=0`; its second means
`-k1-(k2-k4)+(k1+k2-k4)=0`. Parent 31740 has a cube graph
representation. These witnesses check the four parent graphs only; they are
not yet a proof that an implemented contraction/routing procedure covers all
67 representative classes.

## 4. The complete published 67-representative census

The following data come from Table B.2, p.115. An asterisk marks a factorized
topology, exactly as in the thesis. They form an independent acceptance ledger
for an offline contraction and routing census.

| Physical lines | Representative sector IDs | Nonfactorized / factorized |
|---:|---|---:|
| 5 | `31744*` | 0 / 1 |
| 6 | `32256*`, `31746*`, `29702*`, `28686` | 1 / 3 |
| 7 | `32512*`, `32288*`, `32258*`, `31754*`, `30872*`, `30858`, `30214`, `29703` | 3 / 5 |
| 8 | `32640*`, `32576*`, `32528*`, `32513*`, `32386`, `32274`, `32266`, `32259*`, `31380`, `31246`, `30876`, `30862`, `30222` | 8 / 5 |
| 9 | `32704`, `32648*`, `32608*`, `32592`, `32529*`, `32518`, `32394`, `32390`, `32329`, `32278`, `32270`, `32267`, `31516`, `31388`, `30231` | 12 / 3 |
| 10 | `32736`, `32712`, `32708`, `32674`, `32652*`, `32596`, `32562`, `32534`, `32398`, `32391`, `32279`, `31420`, `30563*`, `30239`, `29550` | 13 / 2 |
| 11 | `32744`, `32737`, `32713`, `32682`, `31736`, `30691`, `30526` | 7 / 0 |
| 12 | `32745`, `31740`, `30699`, `30527` | 4 / 0 |
| Total | 67 representatives | 48 / 19 |

For this particular auxiliary basis, Table 9.1, p.101, classifies all `2^15`
labelled sectors as follows:

| Category | Count | Meaning |
|---|---:|---|
| Physical | 22,051 | Nonzero, graph-realizable labelled sectors |
| Zero | 5,566 | Vanish in dimensional regularization |
| Anti-sector | 5,151 | Not realizable as the physical vacuum graphs in the census |
| Total | 32,768 | All auxiliary sign sectors |

These are historical reference counts, **not measured RustRed counts**. An
anti-sector is not interchangeable with a zero-sector: it must not be silently
declared zero by a generic solver. Nor should the generic family engine reject
an arbitrary non-graph denominator family merely because it lies outside this
particular physical benchmark.

The factorized count follows the loop partitions on p.100:

```text
19 = 1×(1+1+1+1+1) + 1×(2+1+1+1) + 1×(2+2+1)
   + 3×(3+1+1)     + 3×(3+2)       + 10×(4+1).
```

Factorized topologies must be covered, either by exact product reduction to
lower-loop artifacts or by the same generic rules; simply excluding their
sectors would leave the evaluator incomplete.

### Independently checked offline routing cover

The four roots now have an exact, independently replayed routing cover of all
67 listed representatives. The coverage witnesses remain offline input data,
not a strategy installed in RustRed. This establishes which physical families
to solve; it establishes neither any IBP rule nor a solved sector.

The new external inputs are
[`tide_five_loop.toml`](../../examples/input/tide_five_loop.toml) and
[`tide_five_loop_manifest.json`](../../examples/input/tide_five_loop_manifest.json).
The exact maps are shipped separately in
[`tide_five_loop_routing_witnesses.json`](../../examples/input/tide_five_loop_routing_witnesses.json).
An independent transcription check against Table B.2 checked all 67 IDs,
corner-power vectors, line counts, row numbers and factorization flags. The
input lane also ran exact Symbolica basis derivation: determinant `-1024`,
rank 15, 25 ordinary IBP sources, and identical CLI/Python derivation output.
These are **input-validation** results, not a completed solve.

The direct bit-subset check covers only **37 of the 67** printed representatives.
The remaining 30 are
`31746, 32258, 31754, 30858, 32528, 32386, 32274, 32266, 32259, 31246,
30862, 32592, 32529, 32518, 32394, 32390, 32278, 32270, 32267, 32708,
32674, 32652, 32596, 32562, 32534, 32398, 32391, 32279, 29550, 32682`.
They are all covered once explicit loop-momentum transformations are included.
The initial transcription evidence is
`TMP/tide-input-audit.zDZsTt/audit-final.log`.

The offline producer used Symbolica's native exact matrix ranks to identify
vector-matroid circuits. Native canonicalization of their colored incidence
graphs proposed line bijections; native matrix inverses and products then
constructed and verified signed integer loop maps. Every accepted map obeys
`q_source_i R = sign_i q_target_i`, with integer `R` and `det(R)=±1`, and its
target slots form a subset of the recorded parent. Graph equivalence was only
a proposal, never the authority for an integral equality.

The separate checker replayed **all 67 maps and 599 active-line equalities**,
checked masks, bijections, deleted edges, matrix rank and determinant, and
compared all fifteen momenta with the manifest and TOML input. Thirteen
corrupted/missing/duplicate witness controls were rejected. Since the same
loop map acts on the entire integrand, it also specifies the transformation of
all numerator scalar products; these need not be permutations of auxiliary
denominators. Their expansion must use the ordinary exact family basis.

The producer found 277 circuits with 3,338 native rank tests and reproduced
the same 67 witnesses in two fresh runs, each about 0.06 seconds. This tiny
offline input-preparation measurement is **not an IBP generation time**.
Evidence lives in `TMP/tide-routing-census.f3acKs/` and
`TMP/tide-routing-audit.6SNo2V/`. The cover is of the published 67-entry ledger;
it does not independently reproduce the reference classification of all
32,768 labelled sectors.

Use Symbolica's graph facilities for structural comparisons, followed by exact
denominator/numerator routing checks. The graph census is offline input
preparation; do not put the 67 IDs, four masks, or a loop-count switch into a
production strategy.

## 5. How many masters, and what was actually evaluated?

There is a real distinction between published basis counts:

| Source / scope | Reported count or achievement |
|---|---|
| 2015 thesis, p.101 and Tables B.2–B.3 | 131 selected five-loop masters including products; 109 excluding factorized topologies |
| Same thesis, p.102 | Numerical results for 44 of 48 nonfactorized topologies, all with at most 11 lines; 250 or more digits, 15–30 epsilon orders in 3d/4d |
| 2016 tadpole update, §3 | 103 masters in 63 sectors with at most 11 lines; nine masters in the four not-yet-evaluated 12-line sectors |
| 2017 QCD renormalization calculation, §2.2 | 110 five-loop masters in that calculation; three combinations of the unresolved 12-line integrals determined to 260 digits |

The last statement is from
[Complete renormalization of QCD at five loops](https://arxiv.org/pdf/1701.07068),
printed p.3. It explicitly says the individual 12-line-family evaluations were
still unavailable, although the combinations needed in that calculation could
be determined. The QCD-specific result does not supply a public numerical value
for every possible RustRed terminal.

The count differences must not be “reconciled” by guessing which extra IBPs or
products were omitted. They may involve different reductions, dependencies or
workload conventions; the cited passages alone do not provide an exact basis
map. For RustRed, the acceptance objective remains a finite, practical terminal
basis with exact identities and numerical values where available, not a
hard-coded target of 109, 110, 112, or 131. Numerical linear dependence at one
dimension is not a proof of an identity over `Q(d)`.

The thesis supplies roughly 50 printed digits for corner-integral expansions
in Appendix C, not a machine-readable 20,000-digit five-loop master archive.
Its p.99 timing discussion starts with 20,000-digit numerical precision and
ends with about 280 digits after unstable recurrence steps. Starting precision
must not be advertised as final master precision. Full high-precision results
and equations were described as available on request, not as an included
public TIDE source distribution.

The 12-line corner integrals are finite near four dimensions, but omitting them
because they did not enter an earlier UV application would be inappropriate
for a general integral evaluator. Later QCD work in fact required particular
combinations. The RustRed coverage ledger must keep all four parents.

## 6. TIDE's reduction algorithm, and what transfers

### 6.1 Finite integer reduction versus parametric closure

TIDE constructs Laporta systems for bounded integer indices and systems with
one symbolic exponent `x` for numerical difference equations. RustRed seeks
generic parametric reduction rules across index domains. These are different
workloads: a completed TIDE zone is not automatically a closing RustRed family
artifact, and a timing for a zone cannot be called a timing for all-index
parametric closure.

For ordinary indices the thesis orders by number of denominators, sector,
extra dots, numerator powers, then index tie-breakers. Generated equations are
sorted by their hardest integral before elimination. It observes that missing
relations in an intermediate subsystem can create large spurious combinations
whose cancellations are delayed, so source order matters independently of the
final span. See pp.20–23.

Define `r = sum(max(a_i-1,0))` over positive slots and
`s = sum(max(-a_i,0))`. TIDE distinguishes the requested region
`r<=r_max, s<=s_max` from a larger generated halo
`r<=r_gen, s<=s_gen`. In its typical five-loop difference-equation work it uses
`r_max=s_max=2`, `r_gen=s_gen=3` (pp.88–89). Auxiliary integrals outside the
requested region are eliminated first and not retained as output. This is a
useful generic policy for RustRed's finite terminal-relation searches, not
evidence that those small bounds certify every parametric ray.

Three source classes help fill the boundary: ordinary IBPs, combinations which
do not increase numerator rank, and syzygy relations which do not increase
dot count. TIDE uses the latter two only at the relevant boundary, where they
add information not already cheaply supplied by ordinary seeds. The simple
mass-derivative combination is already an IBP combination; it is not an
independent new identity. Any RustRed implementation must use Symbolica's
existing algebra APIs rather than implementing another syzygy or polynomial
engine.

### 6.2 Modular pruning and delayed lower-sector execution

In §8.1.4 TIDE first specializes `d` and performs arithmetic over a finite
field. The difference variable `x` remains symbolic because the algorithm
shifts it. A pilot identifies necessary equations and their dependencies,
including only the small part of the generated halo actually needed by the
targets. This is a pruning pass; the 2015 implementation did not reconstruct
its whole rational answer from these samples.

It then separates the main-sector elimination from lower-sector execution.
Main-sector steps are recorded first while the lower-sector tails are left
unevaluated. Once the main elimination is fixed, the lower tails can be
processed with immutable lower-sector solutions, dependency scheduling, and
fewer repeated shift/reduce cycles. Critically, the thesis acknowledges rare
relations discovered from a higher sector which constrain a lower one;
numerical pilot checks are used to detect them. Thus deferred processing is not
permission to assume that every lower-sector basis is already minimal.

The thesis's worked example, zone `31246#6`, is especially informative:

| Configuration, Table 8.1 p.94 | Full input equations | Full reduction | Pilot + full |
|---|---:|---:|---:|
| Neither optimization | 15,630 | 2,671 s | 2,671 s |
| Modular pilot only | 462 | 46 s | 1,029 s |
| Deferred lower work only | 15,630 | 276 s | 276 s |
| Both | 462 | 17 s | 128 s |

Only about 3% of the original equations survive the pilot. The pilot itself
falls from 983 s to 111 s when lower-sector work is deferred. This is strong
motivation to measure discovery, exact materialization, and lower-tail replay
separately. It is **not** a transferable 20.9× performance guarantee for RustRed.

For the current RustRed work, reconstructing a compact source-weight vector
and validating its exact product with the original source matrix is aligned
with the principle of avoiding a second full symbolic elimination. The
implementation still has to retain source maps, poles, guards and the chosen
canonical target row. A modular hit or sampled identity alone is not authority
to publish an exact rule.

### 6.3 Coupled low-order systems, not forced scalar decoupling

For an equation involving `I_j(x+k)`, ordinary elimination prioritizes removing
the harder integral label `j`, even if the offset span in `k` grows. TIDE's
Minimal Order Reduction Algorithm (MORA, pp.49–53) instead prioritizes a small
offset span, allowing several integral labels to remain coupled. It maintains
top and bottom pivot equations, shifts them consistently in `x`, and revisits
displaced equations. Its first-order coupled system may have more components
but much smaller rational coefficients than a single high-order scalar
recurrence. Coefficients must shift together with integrals: `c(x)I(x+k)`
becomes `c(x+s)I(x+k+s)`.

Algorithm 3, §6.2, also minimizes the polynomial degree in `x` before the final
MORA pass. It helps when translating the difference equations into recurrences
for factorial-series coefficients; a high polynomial degree otherwise becomes
a difficult recurrence order. This is a specific numerical-evaluation
motivation, not a drop-in all-index SpIReD closure algorithm.

The transferable RustRed experiment is to prefer small exact blocks or coupled
relations when scalar elimination creates much larger coefficients. It must
retain a well-founded application order and exact exceptional-case handling.
Adopting the full TIDE numerical factorial-series solver is neither necessary
for the present closure work nor part of this research lane.

### 6.4 Basis search and parallel work

TIDE changes the basis of a reduced modular system rather than restarting its
IBP generation for every candidate. It begins with several orderings, explores
one-element basis replacements, and scores by coefficient degree in the
remaining symbolic variable; the best small set is expanded until improvement
stalls. This local search does not prove optimality. The thesis reports that
for difficult zones its chosen basis can substantially reduce coefficient size
(pp.94–95).

This suggests generic cost scores based on observed fill, source-trace size,
coefficient degree/term count, and application cost—not on a named parent or
loop count. The score should not be terminal count alone: a slightly larger
finite basis can be far cheaper to generate and use.

TIDE parallelizes independent zones, independent deferred elimination steps,
and coefficient algebra, scheduling larger expressions first. RustRed should
use shared immutable sources and Symbolica objects, bounded worker pools, and
explicit memory budgets rather than copy TIDE's separate Fermat processes or
its string-based expression representation.

## 7. Modern literature cross-checks

These are options to evaluate against profiles, not promises of universal
closure or a proposal to add external CAS dependencies.

The repository already has the broader studies
[`high_loop_proposal_experiments_2026.md`](high_loop_proposal_experiments_2026.md),
[`parametric_ibp_breakthrough.md`](parametric_ibp_breakthrough.md), and
[`six_loop_algorithm_update_2026.md`](six_loop_algorithm_update_2026.md).
The brief cross-check below connects that existing research to TIDE's concrete
implementation lessons rather than starting another parallel roadmap.

| Primary reference | Relevant result | RustRed consequence / limitation |
|---|---|---|
| [Kant, 2013/2014](https://arxiv.org/pdf/1309.7287) | Modular linear-dependency detection avoids expensive symbolic processing of redundant IBPs. | Cheap discovery must still be followed by exact rule authority; unlucky samples need retry. |
| [von Manteuffel–Schabinger, 2014/2015](https://arxiv.org/pdf/1406.4513) | Finite-field evaluation and rational reconstruction avoid intermediate rational-function growth. | Use Symbolica's reconstruction and finite-field services, not a RustRed reconstruction kernel. |
| [FiniteFlow, 2019](https://arxiv.org/pdf/1905.08019) | Numerical dataflow graphs support repeated modular evaluation and reconstruction. | Reuse one sampled elimination result across coefficient outputs; distinguish retained cache size from transient reducer memory. |
| [Kira 3, 2025](https://arxiv.org/pdf/2505.20197), §3.2 | Forward-elimination dependency selection avoids some redundant dependencies introduced by later substitution; a modular retry fills missing targets. | Audit traces for cancelled/hidden-zero work before making them exact. Keep a target-based completion check and do not equate a bounded seed test with parametric closure. |
| [Blade, 2024](https://arxiv.org/pdf/2405.14621), §§2.5–3 | Smaller block-triangular systems and spanning-sector reductions reduce repeated solve work. | Small exact blocks are worth profiling; naive sector cuts can miss relations revealed by higher sectors. |

Kira 3 explicitly notes that arbitrary random row reordering can densify the
system; its tested local batch reordering brought limited additional equation
reduction. That favors bounded structurally scored ordering experiments over
an expensive uncontrolled search. This is evidence about the cited examples,
not a universal bound on possible ordering improvements.

Blade explicitly states an unproved assumption in its sampled identification
of all higher-sector-induced relations, with a failure check when subsequent
reductions expose additional ones (§2.5). Its method should not be presented
as a no-shortfall certification theorem. RustRed can use such ideas for
discovery while preserving exact replay and honest incomplete results.

For the offline graph census, the thesis cites
[Kajantie–Laine–Schröder, A simple way to generate high-order vacuum graphs](https://arxiv.org/pdf/hep-ph/0109100).
Its graph-generation approach is useful provenance, but RustRed's reusable
implementation should leverage Symbolica graph tools and keep the actual
five-loop graph list as data.

## 8. A progressive, measurable reproduction sequence

This is an implementation recommendation, not a report that the following
experiments have completed.

1. **Freeze the offline census.** Store the exact 15-vector basis, big-endian
   mask convention, four parent roots, all 67 target representatives and the
   source/page provenance. The four graph witnesses and the 67-class
   routing/contraction cover now pass independent checks. Keep witness
   generation outside the production solver and use generic APIs for the
   mathematics.
2. **Finish a genuinely complete small five-loop input.** The six-line banana
   is sector 28686 in the TIDE basis. It has an especially useful published
   numerical expansion and a symmetric graph. Preserve the existing unfinished
   campaign evidence; do not call a solved selected sector a solved family.
3. **Advance by census strata, not a single ever-larger blind run.** Add
   seven-line representatives 30858, 30214 and 29703; then the eight-line
   stratum. Include all required factorized owners and shared lower sectors.
   At each step cold-load outputs and reduce explicit corner, dotted,
   numerator and pinched canaries before moving upward.
4. **Use paired easy/hard cases at each line count.** TIDE's Tables 8.2–8.3
   identify such contrasting cases. For example, 30876 versus 31246 at eight
   lines and 32518 versus 30231 at nine lines isolate structural difficulty
   without changing loop count. These are input-selected experiments, never
   topology-name dispatch inside RustRed.
5. **Complete all seven eleven-line representatives.** This reproduces the
   class of topologies for which TIDE reported numerical results. Store one
   canonical reusable owner/result per equivalence class; avoid regenerating
   successful lower-sector rules for every parent.
6. **Attempt all four twelve-line parents.** They are separate difficult
   workloads, not an already-completed TIDE oracle. The cube representative
   31740 is a useful symmetric member; no source here proves it is always the
   fastest. Use measured modular fill, trace size and exact-lift cost to order
   the attempts, rather than promise a complexity order from a graph picture.
7. **Reduce the finite residual catalogue afterward/in parallel.** Apply the
   existing generic finite-terminal relation lane, include cross-sector
   routing and factorization, and keep a practical nonminimal basis whenever
   further elimination increases cost. Every accepted identity must be exact.
8. **Numerical validation is a distinct milestone.** First compare published
   corner coefficients with the correct measure and mass convention; then
   evaluate any additional retained terminals using an authorized numerical
   master provider. Do not claim TIDE supplies all those numbers automatically.

The stopping/status categories should remain explicit:
`selected sector solved`, `all requested sectors generated`, `exact rule replay
passed`, `bounded application verified`, `unbounded closure proved`, and
`numerical master evaluation available`. They must not collapse into one
ambiguous “solved” flag.

## 9. Performance evidence to collect and decisions it can support

Use release builds, frozen input/orderings and identical requested sectors,
separating one-worker and six-worker runs. Exclude compilation and downloads
from solver timings. Retain total CPU time, wall time, peak RSS, completion or
termination reason, and reproducible artifact identifiers. A timeout/resource
limit is a censored experiment, not a completed solve timing.

For every generic backend measure at least:

- initialization and family/sector canonicalization;
- modular source generation, rank progress, GPLU fill, accepted/rejected probes;
- winning dependency-trace size and reconstructed coefficient/source-weight
  counts;
- reconstruction sampling time versus exact product validation time;
- exact expression term counts, factorization/GCD cost, and cache occupancy;
- main-sector versus lower-sector execution time;
- guard/exception branching, final rule count, finite residual count and any
  still-uncovered positive-dimensional cases;
- artifact write/load/decompression and cold canary application separately.

Choose the next optimization from the measured dominant component. If exact
lifting dominates, modular source-weight reconstruction is a direct candidate.
If lower tails dominate, defer them and reuse immutable lower owners. If
modular fill dominates, prune sources or try a small ordering/block portfolio.
If reconstruction probe count dominates, reduce the degree/complexity of the
chosen basis before adding threads. Preserve the existing exact path as a
differential oracle for workloads small enough to finish.

Historical TIDE timings are useful shape information only. For example,
Table 8.2 reports 25 s for zone 28686#1, 667 s for 31246#1, and 2.91 days for
32744#1 in one preprocessing stage. The full zone workflow also has subsequent
reductions and numerical evaluation; it is not a RustRed parametric-generation
timing boundary. The thesis's machines had 24 cores and 48 GB RAM (p.85), and
these old timings cannot support a direct normalized speed ratio without a
matched workload and resource model.

## 10. Immediate conclusions and open work

The five-loop target can now be named precisely: the supplied reference gives
four concrete 12-line roots and a 67-class fully massive coverage ledger in a
single 15-slot coordinate family. It also explains why a generic `K=15`
all-positive run would be the wrong physical benchmark.

The useful near-term route is to finish and measure small complete members,
progress through shared lower sectors, exploit modular source pruning and
late exact/lower-tail materialization, and then take the four parents one at a
time. TIDE's coupled low-order philosophy supports accepting a somewhat larger
finite universal terminal basis when scalar elimination is the source of
expression swell. It does not remove the need for exact identities or turn a
bounded integer reduction into an unbounded parametric closure proof.

The offline four-root-to-67 routing cover is independently checked. Still open
are completed RustRed five-loop family runs, a mapped and numerically evaluated
final terminal basis, and meaningful matched solver timings. None of those
follows from the successful input-census check.
