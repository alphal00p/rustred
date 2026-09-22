# Finite-domain five-loop campaign: focused literature audit

Date: 2026-09-22. Scope: practical completion of the finite renormalizable
starting domain in `docs/finite_starting_domains.md`, not terminal minimization
or numerical master evaluation. This is a research recommendation, not a report
of a completed five-loop solve. No external solver was run for this audit.

## Executive recommendation

The most promising next step is **less unnecessary work**, not a new CAS kernel:

1. Retain the physical total-power correlations through matching and routing.
2. Reduce costly numerator-bearing momentum transformations: choose among
   admitted routes by predicted expansion, and investigate retaining a local
   owner when canonical routing is more expensive than native IBP reduction.
3. Keep the maximal-domain index, but improve its lookup/publication mechanics
   only after measuring the remaining admission bottleneck.
4. Repair an actual reached missing target with a small, adaptive source system.
   A recent tube-seeding paper supplies a particularly relevant experiment.

The literature does not establish a universal cheap finite-source cutoff for
arbitrary five-loop vacuum reductions. Nor does it justify promoting a modular
success, exhausted abstract worklist or unvisited conservative region to a
closed physical-domain result. Those are separate correctness questions.

## What problem the current measurements expose

The checkpoint pilot in `bounded_routing_pilot_2026-09-22.md` stopped after
321.095 seconds of traversal: 74,326 domains completed, 85,017 queued,
6.091 billion general containment comparisons and no observed missing-rule
frontier. It preserved finite positive coordinate bounds. The maximal-candidate
index subsequently reduced standalone replay cost; the resumed matched run
below measures its campaign-prefix effect. That replay is not an IBP benchmark.

These observations distinguish three possible costs:

- **Too many represented points:** separate coordinate maxima discard A/R/D
  correlations, enlarging the mathematical problem before any search begins.
- **Too many representations of related points:** overlapping boxes and repeated
  route/IBP images overwhelm the scheduling/index machinery.
- **Genuinely absent rules:** a concrete reached integral fails exact dispatch.

Only the third calls for generating a new IBP. The first two can be severe even
if every concrete integral tested reduces with the saved rules. The current
evidence does not measure an irreducible five-loop algebraic obstruction.

The resumed runs strengthen that diagnosis without establishing closure. Root's
measured diagonal rerun, receipt `shared-owner-campaign.gltu7ezq`, stopped at
the 50-million-event allowance after 241.852 s traversal, with 137,684 completed
of 264,765 scheduled domains and 127,080 queued. The all-67-owner R0 control,
receipt `shared-owner-campaign.9jj5ev7y`, stopped at its 500,000-domain allowance,
with 90,428 completed and 409,571 queued. Neither had reported a missing-rule
frontier. The latter again showed a running main thread with all 50 compute
workers waiting and the 65,536-entry publication limit reached. These are
resource-incomplete runs, not successful campaign timings; no whole-envelope
ETA follows. Final reproducible timings belong to the main pilot report.

## Primary literature: relevant mechanisms and limits

### TIDE: separate wanted targets, economical routing and delayed lower work

Luthe's thesis, Sections 8.1.1–8.1.4, distinguishes the wanted reduction region
from a larger region used to generate equations. It selects momentum routes
to limit expansion of the highest-power numerator factor. Finite-field pilot
runs identify needed equations, while delayed subsector execution avoids
repeated lower-sector manipulation and permits independent later work. The
thesis explicitly discusses exceptional information flowing from higher to
lower sectors; a strict sector-only information model is not universal.
These are methods for the thesis's reductions, not timings for RustRed's
current symbolic-domain traversal.
[TIDE thesis, Chapter 8](https://noah.nrw/ubbihs/download/pdf/5131192).

**RustRed inference:** existing verified maps should be scored before use, and
immutable lower-owner work should be reused. Source-generation halos must not
be confused with cuts on required descendants. Master evaluation and difference
equation numerics are outside the present task.

### Kira 3: numerator symmetries can be more expensive than IBPs

Kira 3, Sections 3.1–3.2, improves seeding in lower sectors and equation selection
for requested targets. Importantly, it also generates IBPs in sectors mapped
away by symmetries: numerator-bearing symmetry equations can become more
expensive than IBPs. It discourages blindly selecting constant-rank targets in
every subsector when using decreasing-rank seeding, and describes adjusting
seed bounds where reductions remain insufficient. Its improved selection uses
finite-field elimination to discard irrelevant contributions before the final
solve. The reported gains concern particular reduction workloads, not a
general theorem that lower-sector rank can always be truncated.
[Kira 3](https://arxiv.org/html/2505.20197v1).

**RustRed inference:** sharing canonical subtopologies is valuable, but forcing
every high-rank numerator through one canonical routing is not necessarily the
fastest execution plan. Keep canonical identity separate from execution choice.

### Blade: compact systems and lower-sector seeding, with explicit caveats

Blade uses block-triangular relations, adjustable lower-sector seed ranks and
target-based trimming. Its discussion of mapped-sector IBPs likewise identifies
a tradeoff between extra seed equations and expensive symmetry relations.
Its spanning-sector algorithm accounts for relations connecting sectors through
higher-sector equations; the paper explicitly labels complete discovery of
those connections an assumption checked for failures during execution. This
is not an unconditional certificate from isolated sector cuts.
[Blade, Sections II.4–II.5 and III](https://arxiv.org/html/2405.14621v1).

**RustRed inference:** small block/source proposals are attractive for genuine
repair, but replacing the current saved rule application with a new global
block-triangular solver would target the wrong measured bottleneck first.

### FIRE 7: symbolic presolve and ordering remain orthogonal levers

FIRE 7 performs forward and backward elimination on symbolic IBPs before
integer seeding, and exposes configurable integral orderings. It also separates
modular reduction from reconstruction and supports parallel probe workloads.
These improvements primarily affect equation generation and solving; they do
not directly remove conservative-domain scheduling fanout.
[FIRE 7, Sections 3.2–3.3](https://arxiv.org/html/2510.07150v1).

**RustRed inference:** retain the existing ordinary-source presolve and ordering
portfolio. Compare alternative rules by expected downstream routing cost as
well as local coefficient complexity; do not assume the first exact rule found
minimizes end-to-end application work.

### A new concrete candidate: tube seeding, June 2026

Berman et al. place ordinary IBP seeds in thin zigzag tubes between selected
targets and low-complexity integrals. Multiple target chunks cover a larger
finite target set. Their complete 66-target rank-10 double-pentagon example
uses all eleven spanning cuts: Tables 9–10 report summed solve time 4,731 s
versus 26,525 s and maximum solve RSS 20.9 versus 220.5 GiB for their
decreasing-rank comparator. These are finite-field, numerical-kinematics
workloads, not exact five-loop vacuum artifact generation. Some initially
unreduced central targets require added diagonal paths. The paper therefore
supplies a useful adaptive heuristic, not a topology-independent closure proof.
[Tube seeding, Sections 4.3.3 and 5](https://arxiv.org/pdf/2606.10698).

**RustRed inference:** try path-local ordinary-source proposals inside existing
fixed-target feedback, widening only when the target remains unresolved. This
does not require reproducing the authors' machine-learning discovery procedure.
No performance factor above should be transferred to RustRed without testing.

### Antichains: useful scheduling representation, not automatic closure

Doyen and Raskin formulate antichain algorithms using simulation-compatible
preorders and fixed-point operators. Keeping extremal representatives can
replace much larger represented state sets when the needed structural
properties hold; arbitrary dominance without those properties is not enough.
[Antichain Algorithms for Finite Automata, Sections 2–3](https://lsv.ens-paris-saclay.fr/~doyen/papers/Antichains_Algorithms_Finite_Automata.pdf).

**RustRed inference:** the current index-only retirement is the conservative
application: retain exact keys and pending obligations, remove only redundant
lookup candidates. A further optimization that actually deletes pending work
requires a separate covering-obligation argument. It must not rely on an
unproved monotonicity of polynomial guard selection.

### Fixed templates and counterexample-guided refinement

Template numerical domains retain selected linear forms instead of general
polyhedra. The original general construction uses optimization queries; it
does not imply RustRed needs to implement an LP solver.
[Sankaranarayanan, Sipma and Manna](https://home.cs.colorado.edu/~srirams/papers/vmcai05.html).

Counterexample-guided abstraction refinement separates a genuine concrete
failure from a spurious path introduced by abstraction, then refines the latter.
[Clarke et al.](https://www.cs.cmu.edu/~emc/papers/Papers%20In%20Refereed%20Journals/Counterexample-guided%20abstraction%20refinement.pdf).

**RustRed inference:** the relevant fixed forms are A, R and D=A-R, alongside
coordinate bounds. Their special disjoint-group structure permits direct
checked integer interval formulas. A conservative routing frontier should
trigger a reachable-witness check before requesting a new rule. This is a
design analogy, not an existing model-checking theorem for RustRed.

## Local SpIRed inspection

The local reference code was inspected rather than treated as a black box:

- `vendor/spired/src/solver.tpp`: sector pre-reduction, case-directed seed
  search, modular pivot discovery, dependency pruning before exact solving,
  and recursion into inapplicability cases;
- `vendor/spired/src/seedRunner.tpp`: signed-depth source progression and
  separately bounded numerical-case search;
- `vendor/spired/src/reducer.tpp`: requested-row solving and back-substitution;
- `vendor/spired/src/diophantine.tpp`: exceptional-case simplification.

This inspection found no ready-made replacement for RustRed's present campaign
containment index or compact physical input-envelope traversal. Its useful
role remains a narrowly matched case/ordering/source-search oracle. The notes
PDF was neither copied nor added to the repository. No confidential source
text is reproduced here, and no C++ code was changed or executed.

## Proposed practical architecture

The following is our proposal, not a claim that a cited package implements it.

### 1. Preserve the input problem before optimizing its traversal

Attach `A<=Amax` and `Dmin<=A-R<=Dmax` to each existing coordinate box and rank
cap. For a fixed sector, the active and inactive coordinates form disjoint
integer interval sums, so emptiness and coordinate projections are O(N).
Keep the predicates after projection: individual maxima are not equivalent.

For example, two positive powers in 1..3 with A<=4 exclude (3,3), although
projecting either coordinate independently still gives 1..3. Repeating that
loss across many propagators creates enormous nonphysical work. The current
all-owner independent-axis A24 diagnostic permits A up to 156; it is not the
intended A24 envelope.

For a fixed IBP shift after sign-crossing coordinates are fixed, translate
current constraints using constant delta_R, delta_D=sum(shift), and
delta_A=delta_R+delta_D. Do not reapply entry caps to descendants. Admitted
affine routing conservatively preserves A upper and D lower, with weighted
pinches tightening A/R; affine constants may invalidate a previous D upper.
These inequalities have a separate repository-local mathematical audit.

All cache keys, containment tests, emitted pieces and failure receipts must
retain the same predicates. Constrained domains cannot seed rank-only full-
orthant shortcuts. The newly drafted geometry service is not yet evidence
that end-to-end constrained traversal is implemented or verified.

### 2. Make routing a costed plan, not a compulsory normal form

On hot edges, compare a bounded set of already verified maps using structural
statistics: nonzero terms in relevant numerator rows, numerator powers, degree
loss from constants, projected support masks and measured repeated endpoints.
Use Symbolica to inspect/evaluate native algebra; do not add another polynomial
expander. Preserve deterministic tie-breaking and source conditions.

An initial experiment can change only the selected map, with the same target
owner and saved rules. A larger experiment keeps a local native rule program
for an expensive mapped sector and canonicalizes only when that reduces work.
That second option requires source replay and consistent owner/routing descent;
it must not become an unverified topology-specific escape hatch.

### 3. Separate discovery of a real gap from abstract overcoverage

Retain enough provenance to connect a frontier to a starting region and its
IBP/routing path. Refine the responsible coordinate face or test a concrete
candidate with exact native dispatch/transport. If the gap is only in the
overcover, improve that local representation. If a reached key is missing,
nominate it to fixed-target feedback. If witness construction cannot decide,
keep an explicit unresolved obligation.

Do not accept every sampled success as evidence for all remaining points. Also
do not regenerate a family merely because an extra abstract point has no rule.

### 4. Reduce serial admission without broadening algebraic scope

Retain per-owner/phase maximal candidates. Add cheap necessary containment
filters before full coordinate comparison and measure hit rates. Candidate
filters must have no false negative for possible containment; false positives
only cost work. Options include aggregate ranges, a few discriminating axes,
or bucketed finite upper-bound signatures followed by the exact predicate.

Bulk worker-local exact deduplication and deterministic publication batches
can reduce coordinator pressure without changing rule authority. Cross-owner
parallelism should continue sharing immutable programs rather than cloning
CAS expressions. Do not increase the return buffer merely to hide a slow
publisher. Sharding the index by owner is a larger change and should follow
evidence that cheap filtering/batching cannot remove the serial bottleneck.

### 5. If a genuine gap survives, use target-local adaptive source proposals

For a missing key, generate a small deterministic path toward low-complexity
keys under the installed ordering, thicken it by a modest ordinary-source
neighborhood, and share rows across nearby missing targets. Compare axis orders
and diagonal junctions. Use the existing modular solver to select a compact
source trace, followed by the existing exact materializer/replay.

This is a proposal ordering, not a completeness cutoff. Failed neighborhoods
expand or fall back to fair signed-depth enumeration. Descendants outside the
entry domain remain obligations. No terminal is created simply because a tube
or source allowance was exhausted, and no master-minimality test is required.

## Ranked experiments and acceptance boundaries

| Priority | Experiment | Expected leverage | Complexity | Honest success criterion |
|---|---|---|---|---|
| 1 | Complete compact A/R/D propagation | High where rectangular growth dominates | Moderate, cross-cutting metadata | Exact small-domain parity; fewer scheduled regions on matched physical inputs |
| 2 | Profile/select alternative admitted routes | Potentially high on numerator fanout | Low–moderate for map scoring | Same exact endpoints/reductions; lower routing work and queue growth |
| 3 | Filter/batch maximal-candidate admission | Moderate if publication remains serial | Low–moderate | Same admission decisions/obligations; lower publisher CPU/comparisons |
| 4 | Frontier witness/refinement loop | High when false gaps dominate | Moderate | Separate reached missing keys from disproved or unresolved abstract paths |
| 5 | Native-owner bypass of expensive routing | Potentially high, family dependent | Moderate–high | Replayed generic local rules and reduced total work without routing cycles |
| 6 | Adaptive target-local source tubes | High for some true repair systems, unknown at five loops | Moderate | Exact replayed repair of identical targets with fewer rows/time/RSS |

Use the unchanged saved owners first. Controls should progress from the matched
diagonal box to the 67-owner low-rank control and bounded guard boxes, then
faithful marginal/full-jet physical profiles with explicitly admitted root
scope. Do not extrapolate a whole-envelope ETA from preparation time or a local
guard match. Retain the 50-worker/500-GB ceiling and monitored stopping policy.

Report separate preparation, dispatch, RHS inspection, route expansion,
publication/index and repair timers. Include peak aggregate RSS, busy cores,
queue size, maximal candidates, comparisons, actual/implied rank, correlation-
empty pruning and concrete missing keys. Record how many frontier reports are
overcover-only. Reproducibility requires fixed inputs, saved owners, ordering,
worker budget and exact build provenance.

## What not to do next

Do not start terminal minimization or master numerics, brute-force the
trillion-entry envelope, replace the narrow sum geometry by a general
polyhedral/CAS subsystem, or rewrite reconstruction. Do not equate a minimal
master count with coverage, or use literature benchmark numbers as an ETA.
The immediate objective remains a finite, exact, terminating reduction program
for every admitted starting input and its necessary descendants.
