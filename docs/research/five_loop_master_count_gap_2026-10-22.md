# Five-loop vacuum masters: why roughly 100 in the literature, but 862 retained here?

Implementation follow-up: the matched current-set census and broader exact
symmetry provider are now tested in `d3549795`. Existing normalization reduces
862 -> 693; generator relations reach667 and optional full orbits660, with
all939 original maps/conditions retained. These remain nonminimal finite bases.
See [the combined refinement report](combined_sector_symmetry_refinement_2026-10-22.md)
for exact workload, timings, lower-loop controls and recovery tests. The study
below preserves the original research snapshot and its then-pending proposals.

Research date: 22 October 2026. This study followed completion of the parallel
refinement delivery. It adds no solver code, performs no new IBP search or
numerical master evaluation, and changes no production campaign. Four independent
lanes investigated primary literature, the implementation, possible methods and
adversarial mathematical review. Code references concern the isolated
`codex/numerator-symmetry-refinement` implementation at `191c0965`.

## Conclusions

**862 is a finite retained output basis, not a demonstrated number of independent
five-loop master integrals.** The comparable published basis is about **131–132
including factorized products**, or **109–110 excluding them**. Products explain
part of the terminology, not the several-hundred-key gap. The small published
bases admit numerator and dotted inputs; they are not restricted to scalar
starting integrals or to numerical leading poles at exactly four dimensions.
This comparison assumes the retained supports belong to those published physical
families; the matched862-key topology census is still future work.

The strongest next steps are concrete existing-service checks: normalization
admission, more complete symmetry relations, and target-directed ordinary IBP
elimination with sufficient auxiliary/higher-sector sources. There is no evidence
that reproducing TIDE requires a mysterious extra physical identity. Nor has this
study proved that a few changes will immediately reach 131 or that 131 is a
mathematically certified minimum.

## 1. Match the counts before comparing

| Source | Reported count and convention | Relevance |
| --- | --- | --- |
| Luthe, TIDE dissertation, 2015, §9.1 and Tables B.2–B.3 | 67 topology classes; **131 masters =109 nonfactorized +22 products** | Explicit full index catalogue, including all four twelve-line classes |
| Luthe–Schröder, 2016, §2 | 48 nonfactorized +19 factorized sectors; 62+3 additional dotted integrals: **132 =110+22** | Comparable classification, differing by one from the thesis |
| Luthe et al., QCD renormalization, 2017, §2.2 | **110** five-loop masters in that calculation | Physical-use-case evidence, but its product convention is not specified explicitly |
| Current RustRed scope `528866...` | 939 retained source keys, **862 remaining outputs** | Exact finite maps and guards, not an independence claim |

Primary sources: [TIDE thesis](https://noah.nrw/ubbihs/download/pdf/5131192#page=105),
[2016 classification](https://arxiv.org/pdf/1604.01262#page=2),
[2017 renormalization](https://arxiv.org/pdf/1701.07068#page=4).

The 19 factorized **topologies** give 22 product **basis elements** because three
have an additional dotted master. A dot denotes an extra denominator power, not
a different graph. The 2016 classification therefore cannot be summarized as
“67 graphs, hence 67 masters.” Its 62 additional nonfactorized dotted elements
also show that its small count is not a restriction to unit-power outputs.

Another update reports 103 evaluated masters in 63 sectors with at most eleven
lines and nine masters in four twelve-line sectors. Its prose does not resolve
product counting consistently with the full catalogues. These are not safe
numbers to combine into one supposedly unique master count.
[Luthe–Schröder, 2016 update, §3](https://arxiv.org/pdf/1609.06786#page=3).

For prioritization, the 2015 catalogue has at most six masters in a nonfactorized
sector. Its four twelve-line counts are `32745:1`, `31740:2`, `30699:1`,
`30527:5`. A future census should map our actual retained supports to these
representatives and explain each excess, rather than compare a single total.
Do not delete a key solely because a reference count is smaller.

## 2. Bounds and notation are different

For signed indices `n`, define

\[
A=\sum_{n_i>0}n_i,\qquad R=-\sum_{n_i<0}n_i,\qquad
t=\#\{i:n_i>0\},\qquad D=A-R.
\]

TIDE uses `r` for **dots** and `s` for numerator degree:

\[
r=A-t,\qquad s=R,\qquad D=t+r-s.
\]

Thus `D` is **not** TIDE's dot count. In a fixed sector the proposed starting
bounds imply `s≤6` and `r−s≤10−t`. The enclosing maximum dot count is `16−t`,
not two. In TIDE's example `I(3,2,0,-2,1,1)`, `t=4`, `r=3`, `s=2`; consequently
`A=7` and our `D=5`. This agrees with its Eq.(2.6)–(2.7), not `r=D`.

The actual campaign supports a recorded domain union, not automatically every
point in this enclosing box. Neither starting bound clips valid intermediate
IBP integrals. TIDE's cited `rmax=smax=2`, `rgen=sgen=3` is a difference-equation
example, not an equivalent workload to our master refinement.

## 3. What is actually implemented, and what remains incomplete?

| Facility | Existing capability | Important limitation |
| --- | --- | --- |
| Exact normalization | Symbolica-backed scalar parametric aliases, weighted projections and factorization | Current publication uses a restrictive normalization recipe; skip diagnostics are not recorded sufficiently in its summary |
| Numerator symmetry | Exact transport of the **complete** numerator, constants and cancellations; replayable witnesses | Automatic discovery tries signed transpositions/reflections, not all support automorphisms |
| Dotted symmetries | Scalar aliases and verified weighted-preserving numerator maps | The numerator witness is a stabilizer: unequal dotted powers cannot simply be permuted while pretending the denominator stayed fixed |
| Finite IBPs | All ordinary loop-contracted derivative operators; terminal-neighborhood and containing-sector seeds | A bounded neighborhood is not the full ordinary-IBP row span |
| Parallel refinement | Work-stealing exact source/block reduction, ordered complete residual join | Changes execution cost, not the mathematical span of a fixed source set |
| Modular route | Discovery and exact-identity authority are separated | Sampling a rank neither supplies an omitted equation nor certifies independence |

### A. A current normalization gate deserves the first experiment

`application/master_reduction/profile.rs` still defines the conservative
Symanzik preparation profile with a **20,000-term** admission allowance, versus
4,000,000 in the standard profile. The current publication explicitly records
`conservative-v1`, and all 939 raw keys remain distinct after its initial
normalization. This does not prove that every current key was skipped; that
requires the missing matched skip census. These allowances are separate from
the unbounded ordinary relation-algebra/column grants and the separate numerator
symmetry service.

There is strong historical evidence, not a transferable current count. A
different 829-key five-loop fixture skipped normalization because prospective
polynomial preparation requested 20,970 terms. Using the existing standard
service found 164 scalar aliases and 14 weighted projections: **829→651** before
ordinary elimination, then **608** with circuit-assisted finite elimination.
The experiment is documented in
`docs/research/terminal_compression_experiments_2026-10-08.md`, under “Larger
normalization budget.” It does **not** establish `862→608` for today's scope.

The next diagnostic is a standalone same-939-key conservative/standard or
RAM-governed normalization census with exact map comparison and explicit skip
reasons. No suitable existing binary was available for that current-input census
without a small new client/build, so we have not claimed it ran. Do not change
the live profile in place: import pins that authority, and fresh-search checks
currently require the inherited maps to preserve their output convention.
Integrating stronger aliases requires composing every inherited map and guard,
not just editing a profile field. Exposing user-governed limits and reporting
all skips is preferable to replacing one hidden ceiling with another.

### B. Full transport is not full symmetry discovery

`terminal_normalization/numerator_symmetry/mod.rs` explicitly documents heuristic
discovery; `discovery.rs` proposes signed single-line reflections/transpositions.
The exact transport and Jacobian/power/mass checks are sound for accepted maps,
but this is not a certificate that all automorphism relations were generated.
Longer simultaneous permutations can exist even when individual transpositions
are not symmetries.

There is a second distinction. The current weighted stabilizer requires the
denominator, including its powers, to remain fixed. A more general valid relation
can transform an integral into a **different** dotted integral while transporting
its numerator simultaneously. That needs a cross-integral identity, not an
invalid fixed-denominator orbit average. Existing unequal-power rejection tests
must remain correct. Scalar positive-only aliases partly address this issue;
they do not imply full numerator-bearing coverage.

An exhaustive generator set alone also need not close the polynomial relation
space: transformed intermediate monomials may be outside the finite retained
set. Conversely, compositions add no independent rank if all generator images
are already included and the space is closed. Measure new exact rank, not the
number of proposed permutations. The present P102 identity is a successful
example, not proof that this entire class is exhausted.

### C. A huge matrix can contain few master relations

The frozen five-loop hot state used for parallel acceptance has 939 terminal
columns, 1,453,350 auxiliary columns and 607,556 independent rows; it retains 864
basis keys. This snapshot precedes the later two-key import producing 862.

Write its rows as `[A_U A_T]`, with auxiliary and terminal columns. The dimension
of terminal-only relations in that finite row span is

\[
\operatorname{rank}([A_U\ A_T])-\operatorname{rank}(A_U).
\]

For example, `X+M1=0` alone gives no relation among `M1,M2`; adding `X+M2=0`
produces `M1−M2=0`. Millions of rows each introducing a fresh auxiliary can
therefore contribute almost nothing to terminal compression. Our executor
retains auxiliary residuals specifically to preserve such cancellations.
More workers speed this elimination; they do not create missing row rank.

TIDE distinguishes wanted outputs from larger generation halos, uses boundary
IBP combinations/syzygies, and checks whether higher-sector rows constrain lower
sectors. Our global eliminator can preserve such relations **if their source
rows are generated**; the study found no basis for claiming it inherently drops
all of them. The remaining question is finite source coverage, not merely queue
exhaustion or more cores.

The original vendored SpiReD likewise retains numerical cases left by a bounded
search. No omitted automatic global-minimization stage was found that would
magically convert its residuals to the published TIDE basis.

## 4. Literature lessons that are genuinely actionable

**A real missed-relation regression exists.** Möller's 2012 thesis §3.4,
Eq.(3.32), describes an extra dotted master in seven-line sector30858:132
instead of an independent Reduze result131. A small ordinary-IBP reduction
without sector symmetries recovered the relation; its coefficient contains
`1−d/4`. The thesis suspects ordering/symmetrization interplay, not new physics.
This is a useful future regression, not evidence that our P102 is the same
integral. It also warns against sampling exactly `d=4`.
[Primary discussion, pp.53–54](https://noah.nrw/ubbihs/download/pdf/5124967#page=59).

**Source selection can matter more than a larger blind radius.** Kira3 improves
seeding and selects contributing equations after forward elimination, including
hidden cancellations. It rechecks selected systems over finite fields and adds
unreduced targets. These are methods for obtaining the needed ordinary-IBP span
efficiently, not extra physical identities. Its published speedups concern other
topologies and cannot be transferred to our run.
[Kira3, §§3.1–3.2](https://arxiv.org/pdf/2505.20197#page=8).

**Critical points offer diagnostics, not ready-made relations.** Roughly, they
are complex stationary points of the parameter-space polynomial controlling an
integral. With `G=U+F`, proper points satisfy `∂G=0`, `G≠0`. When all proper
points are isolated and the required assumptions/conventions apply,
multiplicities help count the relevant basis. Nonisolated loci, boundary/infinity
contributions, symmetry identifications and products need separate treatment.
Generic-index/regulator counts over `C(d,ν)` are not automatically the same as
our integer-index relations over `Q(d)`. A count does not supply a certified
coefficient identity.
[Lee–Pomeransky](https://arxiv.org/abs/1308.6676),
[Bitoun et al.](https://arxiv.org/abs/1712.09215).

There is a useful vacuum-specific mathematical simplification, not an implemented
solver: for equal mass, `G=U(1+m²Σx)` and `U` is homogeneous of degree `L`.
Euler's identity implies `m²Σx=−L/(L+1)` at an unregulated proper critical point.
This can reduce a future diagnostic by one variable. It is not the corresponding
equation for an independently regulated `d log Φ`, nor a proof of the master
number. Even a one-loop tadpole's stationary point is at negative complexified
parameter `x=−1/(2m²)`; it is not a physical loop momentum.

**Higher-sector “magic” relations are relevant research, not a guaranteed cure.**
A 2026 paper connects higher-dimensional critical varieties with IBPs whose
generating-sector terms cancel, leaving relations among subsectors. The stated
equivalence is conjectural with assumptions, and no demonstrated count reduction
for our five-loop equal-mass catalogue follows. Its methods could prioritize
where to seek containing-sector relations rather than expanding every sector.
[Crisanti et al., v2 September2026](https://arxiv.org/abs/2605.29789v2).

**Sparse tubes are another source-selection candidate.** A 2026 study generates
thin target-to-basis neighborhoods instead of full balls. Its demonstrations are
different two-loop finite-field workloads, so it is an idea to test, not a
five-loop performance prediction.
[Berman et al.](https://arxiv.org/abs/2606.10698).

**Dimensional shifts are not the first missing ingredient.** TIDE2015 §2.3.4
explicitly says they are unnecessary for that thesis. Such relations can provide
alternative computational routes, but do not establish that the ordinary IBP
system lacks the required physics. Nor should we impose five-vector Gram
determinants equal to zero at exactly four dimensions: these are not valid
generic `d=4−2ε` identities. Fixed-order epsilon/PSLQ relations or observable
cancellations are also not interchangeable with exact rational-in-d master
maps.

## 5. Recommended next sequence and stopping criteria

1. **Current-input normalization census.** Same939 keys, same mass/routing,
   exact maps and skip diagnostics; compare conservative and user/RAM-governed
   admission. Falsifier: no extra exact aliases/projections or excessive cost.
2. **Convention-preserving import.** If useful, compose proved normalization
   identities into all existing maps/guards, preserve published source keys,
   cold-load and validate every map. Do not mutate a live profile or silently
   rename masters.
3. **Map remaining keys to the67 reference sectors.** Separate products,
   positive-power scalars, numerator degrees and dots. Rank sectors by excess
   versus the published catalogue, acknowledging reference-count uncertainty.
4. **Complete useful symmetry coverage selectively.** Broader verified generators
   and cross-dotted/numerator maps; reduce offspring with existing rules. Exact
   rank gain is the acceptance test. Avoid unbounded group enumeration or eager
   expensive projection of every high-rank identity.
5. **Targeted ordinary-IBP halos.** For overfull sectors, use finite-field
   scouting, existing exact materialization and boundary-adapted sources; retain
   higher-sector/subsector residuals. Check a matched small cohort before a
   blanket deeper run. More sampled rank alone is never a certificate.
6. **Optional critical-locus guidance.** Use only if cheaper checks do not locate
   productive sources. Do not implement a competing CAS; audit Symbolica's
   facilities before selecting any algebra backend.

The aim is a much smaller practical exact basis, not a proof of absolute
minimality. No new campaign, new master relation count, current939 normalization
gain or completion ETA is claimed by this research.

## Evidence and audit trail

Untracked independent reports:
`TMP/research-five-loop-master-gap-20261022/{literature_counts,implementation_gap,methods,independent_review}.md`.
The local user-supplied TIDE PDF was read directly, particularly Chapters2,8,9
and AppendixB. Downloaded PDFs stay untracked; only this original synthesis is
committed. No confidential notes, numerical catalogues or artifact payloads are
included. The independent review corrected the initial r/D notation and the
mistaken equation of complete numerator transport with complete symmetry
enumeration; those corrections are reflected above.
