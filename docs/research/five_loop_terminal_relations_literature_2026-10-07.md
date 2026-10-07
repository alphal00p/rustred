# Relations worth testing on the 829 five-loop terminals

Research-only literature lane, 7 October 2026. No solver implementation,
production campaign, terminal declaration, or artifact was changed. This note
separates published results, existing RustRed evidence, and proposed experiments.

## 1. What the current count actually establishes

At inspection, `campaigns/five-loop-rank-ladder-THE-ONE/artifacts/latest.json`
selected scope
`52afacba4abc580fae79caf552cc7fc29b500071b7b7d46bc399ca6d1a06f025`.
Its publication receipt, `runs/20261007T204806.377630Z/stdout.json`, records:

| Field | Value |
| --- | ---: |
| Raw terminal keys | 829 |
| Normalized terminal keys | 829 |
| Remaining algebraic terminal labels | 829 |
| Completed refinement seeds | 0 |
| Independent refinement rows | 0 |
| Eliminated terminals | 0 |
| Status | `published_unrefined` |

Thus **829 is not the outcome of an unsuccessful master-minimization search**.
It is the current published nonminimal terminal list. The receipt explicitly
denies a minimality proof. Before attributing the count to missing dimensional
identities, the native refinement must actually search relations, and its
normalization must be audited on this particular terminal union.

The existing [saved-campaign interface](../campaign_master_reduction.md) supports
explicit finite ordinary-IBP refinement. Its `seed_depth` is a search-radius
control, not the starting physical numerator rank.

## 2. Recommendation, in priority order

| Priority | Mechanism | Why it could help | First falsifiable test |
| --- | --- | --- | --- |
| 1 | Global exact symmetry/factorization normalization, including generated columns | Family-local labels and numerator images need not be independent integrals | Count exact aliases and weighted rows in the frozen 829-key union |
| 2 | Target-directed ordinary Laporta with a generation margin | Finite residuals can reflect missing nearby seeds, not new masters | Compare depth 0 with selected one-dot/one-numerator boundary extensions |
| 3 | Higher-sector cancellation and symmetry relations | Sector-local searches can miss relations whose generating sector cancels completely | Seed a small set of containing sectors and project exactly onto the old terminal space |
| 4 | Sparse subgraph dimension recurrences | Can expose short reductions for suitable massive insertions | Compare support, coefficient size, and terminal rank with ordinary IBP on one identified subgraph |
| 5 | Parametric divergence-free syzygies | A more targeted source of higher-sector cancellations | Search low-degree candidates with Symbolica, then verify full boundary identities |

This ranking does not assume a reduction to 110 or any other predetermined
number. Success is an exact smaller spanning list at tolerable runtime; proving
independence is unnecessary.

## 3. TIDE gives a directly relevant answer

The supplied [Luthe thesis](https://noah.nrw/ubbihs/download/pdf/5131192),
*Fully massive vacuum integrals at 5 loops* (2015), was inspected locally,
particularly §§2.3–2.4 and 8.1.1–8.1.4. Its notation `r` means dots, while `s`
means numerator powers: do not confuse its `r` with RustRed's numerator cap `R`.

TIDE distinguishes wanted bounds `(r_max,s_max)` from larger generation bounds
`(r_gen,s_gen)`. It typically admits a one-step margin in both directions, orders
unwanted integrals first, and retains only results useful for wanted integrals.
For its symbolic difference-equation construction it used wanted bounds 2,2
and generation bounds 3,3. That is an empirical setup for that task, not a
theorem that one extra layer suffices for our 829 keys.

It normalizes every generated integral by momentum shifts before elimination,
including weighted numerator transformations. Its boundary supplements are:

- ordinary momentum IBPs everywhere;
- combinations that do not increase numerator power, including common-mass
  differentiation, at the numerator boundary;
- no-dot syzygies at the dot boundary.

These supplements reorganize ordinary IBP information. The thesis explicitly
says dimensional-shift identities were **not needed** for its reductions.
This is strong evidence against treating dimension shifts as the presumed
missing ingredient before ordinary finite-target refinement is exhausted.

### A concrete ordinary relation

For Euclidean denominators `q_i^2 + m^2`, homogeneity gives

\[
 m^2\sum_i a_i I_d(a+e_i)
 =\left(\sum_i a_i-\frac{Ld}{2}\right)I_d(a).
\]

This follows by differentiating the common mass, and equals the sum of diagonal
momentum IBPs. It is not an extra independent law. For a symmetric five-loop
six-line banana corner, permutations identify all six singly dotted integrals:

\[
 I_{\rm one\ dot}=\frac{6-5d/2}{6m^2}I_{\rm corner}.
\]

This is a derived illustrative identity, not a claim that the current 829
contains both representatives. Native `q_i^2-m^2` conventions reverse the
corresponding coefficient sign and must be handled by native source replay.

RustRed already has an especially pertinent negative/positive control:
[the four-loop finite-terminal study](finite_terminal_relations.md) found no
terminal identities in its initial radius-zero systems, nor its FG radius-one
system. Quotienting **generated auxiliary columns**, not merely original
terminals, by exact parameter equivalence then revealed nine identities without
adding any radius-zero seeds. That is a measured reason to prioritize global
column normalization over indiscriminate larger shells.

## 4. Symmetries must include weighted numerator images

FIRE4's §5 treats a union of terminal lists from several families. It combines
parameter-space canonicalization, auxiliary-family symmetries, and mass
differentiation to uncover extra relations. One instructive example finds no
new information from the first dotted symmetry difference, but does find it
after raising another index. Therefore a failed shallow identity probe is not
evidence of independence. [Smirnov and Smirnov, 1302.5885](https://arxiv.org/pdf/1302.5885).

The modern symmetry treatment identifies affine momentum-space sector maps
through permutations of Lee–Pomeransky polynomials. Its graph discussion
includes cycle-matroid equivalences and Whitney twists: graph isomorphism alone
is not the complete equivalence test. Numerators must be transported along the
actual momentum map, potentially producing a sum of integrals.
[Duhr et al., 2604.08332v2, §§3–4](https://arxiv.org/html/2604.08332v2).

A direct illustration requiring no analytic bubble integration is

\[
 \int\!d^dk\,\frac{k\cdot p}{(k^2+m^2)((p-k)^2+m^2)}
 =\frac{p^2}{2}\int\!d^dk\,
 \frac{1}{(k^2+m^2)((p-k)^2+m^2)}.
\]

It follows simply by adding the integrand after `k -> p-k`. Inside a vacuum
graph, `p` can be another loop momentum; a remaining `p^2+m^2` denominator
allows conversion of `p^2` into a pinched term minus a mass-weighted term.
Thus a numerator terminal can reduce to a weighted combination, even though
it is not a unit-weight alias of any other terminal.

The [existing weighted-normalization delivery](weighted_terminal_normalization.md)
already implements a restricted, loop-generic circuit case of this idea and
reduced the old four-loop effective list from 179 to 74 family-local keys.
Audit whether that path and its prerequisites cover the saved-campaign union;
do not infer applicability from its existence elsewhere in the library.

## 5. A small global Laporta problem, not another closure campaign

Proposed RustRed-specific workflow:

1. Freeze the 829 targets and their exact family/routing context. Build a global
   weighted normalization map and retain product structures when factorization
   is proved.
2. Generate the 25 ordinary five-loop IBPs at selected fixed-index seeds. Leave
   `d` symbolic; after unit-mass specialization this is univariate rational
   algebra, not the multivariate symbolic-index problem of rule generation.
3. Include a deliberate margin outside the target set. Do not delete auxiliary
   columns merely because they exceed the physical starting rank or D cap.
4. Normalize all generated columns, then eliminate auxiliaries before targets.
   Keep only rows supported entirely on the original terminal span, or admit a
   changed basis only through a fully invertible, separately accounted map.
5. Use finite-field discovery to identify useful row support. Rebuild and
   replay the resulting exact rows over `Q(d)` with Symbolica.
6. Expand only sectors where target pivots remain missing or target count is
   unexpectedly high. Record source counts and cost per removed terminal.

Kira 3 supplies relevant engineering precedents: smaller sector-dependent seeds,
IBPs even in symmetry-related sectors, forward-elimination dependency selection
that avoids redundant hidden-zero work, and explicit checks against a requested
basis. Its cutoffs require adjustment when extra masters appear; its benchmark
speedups are not evidence of a speedup on our vacuum corpus.
[Lange, Usovitsch and Wu, 2505.20197, §§3.1–3.2,3.6](https://arxiv.org/pdf/2505.20197).

Finite-field row selection is an established discovery method, but a sampled
rank deficit can be unlucky. Multiple primes/samples are diagnostics; exact
regeneration supplies the final identity, and no failed finite search proves
master independence. [Kant, 1309.7287](https://arxiv.org/abs/1309.7287).

## 6. Higher-sector or “magic” relations

Kira 1.2 documents relations that require seeding sectors above the requested
ones, even when those sectors are absent from the original physical problem.
Its concrete example lowers a sector's count from five to four by including a
higher sector and its symmetries. It also warns that a poor ordering can instead
introduce additional unwanted masters.
[Maierhöfer and Usovitsch, 1812.01491, §3.13](https://arxiv.org/pdf/1812.01491).

For RustRed this suggests explicitly solving for combinations of parent-sector
IBPs whose parent columns cancel, leaving a row among descendant terminal
columns. Auxiliary-sector use is local to this finite post-processing; it need
not enlarge the published campaign's physical input scope or keep extra master
labels. A zero result at a bounded margin is inconclusive.

The new paper by Crisanti, Frellesvig, Pokraka and Smith studies this mechanism
through divergence-free syzygies. With `G=U+F`, a polynomial vector satisfying

\[
 \sum_i\phi_i\partial_iG=0,\qquad
 \sum_i\partial_i\phi_i=0
\]

cancels the bulk parametric contribution, leaving relations between boundary
sectors. Some are already symmetry identities. Its critical-variety detection
criterion has stated assumptions and open questions, so it is a candidate
selector, not a completeness oracle. The paper's explicit relation (89) comes
from a non-vacuum example; no five-loop equal-mass gain is demonstrated there.
[2605.29789v2, §§4.4–4.5](https://arxiv.org/html/2605.29789v2).

Our proposed pilot is a bounded low-degree ansatz on one stubborn actual sector,
with Symbolica doing polynomial and linear algebra. First audit its current
syzygy/Groebner APIs; do not implement a competing CAS. Retain boundary terms,
normalization factors, and exact replay. Counting critical points alone cannot
install a terminal reduction.

## 7. Dimensional shifts: useful, but not a free reduction of rank

For Euclidean normalized vacuum integrals define
`A_i f(a)=a_i f(a+e_i)`, `B_i f(a)=f(a-e_i)`, and let
`P=det(k_i.k_j)` expressed in the complete denominator basis. The normalized
vacuum recurrences are

\[
 J_{d-2}=U(A)J_d,\qquad
 J_{d+2}=\frac{2^L}{d(d-1)\cdots(d-L+1)}P(B)J_d.
\]

Independent-mass derivatives are formed before setting all masses equal.
[Tarasov, hep-th/9606018, §2](https://arxiv.org/pdf/hep-th/9606018);
[Lee, 0911.0252, §2](https://arxiv.org/pdf/0911.0252).

Composing the two gives a fixed-d candidate
`[2^L U(A)P(B)-d(d-1)...(d-L+1)]J_d=0`. The operators do not commute. At
one loop this becomes precisely the ordinary tadpole IBP, illustrating why a
dimensional recurrence does not automatically lower the number of independent
fixed-d masters. At five loops the denominator includes `d-4`: a Gram numerator
cannot be set to zero at `d=4-2epsilon`. Doing so can lose finite contributions
after multiplication by poles.

The [previous paired-dimension pilot](dimensional_recurrence_shortcuts_2026-10-01.md)
found dense, poorly oriented rows at its tested target and was parked. Reopening
it should require a sparse subgraph opportunity, not simply a new terminal
count. FMFT supplies an actual example of this route: it uses one-/two-loop
subintegrals, dimension recurrences, and extra connector-mass denominators.
Its paper also notes that large numerator-shift expansions can become less
efficient than ordinary negative-index recurrences.
[Pikelner, 1707.01710, §§2.2–2.4](https://arxiv.org/pdf/1707.01710).

The [EPSILON note](../../EPSILON.md) remains applicable: a massless bubble can
produce an epsilon-dependent propagator power; a generic fully massive bubble
cannot be replaced by a single such power. This is not the automatic cure for
the fully massive terminal count.

## 8. What published master counts do and do not promise

| Source | Reported count and precise scope |
| --- | --- |
| Luthe thesis, §9.1 | 131 chosen masters over its 67 five-loop topologies, including factorized cases; 109 excluding factorized cases |
| Luthe–Schröder 2016 | 103 masters in 63 sectors with at most 11 lines; another nine in four 12-line sectors whose numerical evaluation was then missing |
| Five-loop QCD renormalization paper, 2017 | 110 five-loop masters in that calculation's reduction; only three combinations of the unevaluated 12-line masters entered its results |

Sources: the supplied thesis;
[Five-loop massive tadpoles, §3](https://arxiv.org/pdf/1609.06786);
[Complete renormalization of QCD at five loops, §2.2](https://arxiv.org/pdf/1701.07068).

These are different reported bases and workloads. They are not interchangeable
proofs of a universal minimal count. The 829-key inventory must first be mapped
to comparable sectors, dimensions, masses, factorized products and index
conventions. In particular, reducing a list of product integrals to products of
lower-loop masters is not the same count as the number of independent new
five-loop numerical constants.

## 9. Scope of the conclusion

The literature identifies credible ways to reduce this list without any new
physical assumptions: first full weighted/global symmetry information and
boundary-aware ordinary IBPs, then higher-sector cancellations. The leading
unexplored relation type is not “another 25 IBPs at the same corner”; it is
information that becomes visible only after a larger *but targeted* source
space is quotiented and auxiliary columns are eliminated.

No terminal was removed by this research lane, and no achieved terminal count
or runtime is claimed. A decisive next result would be an exact relation on
the frozen 829-key set, with its source trace, followed by a cold application
check and a measured before/after basis count. Numerical PSLQ, evaluation at
one epsilon value, minimality assumptions, and unconditional four-dimensional
Gram zeros are not substitutes.
