# Whole-system IBP architecture review — 24 September 2026

## Recommendation: change the unit of work before expanding the machine

The strongest redesign is to deliver a **checked reduction program for the
required finite entry set**, rather than construct and retain a large history
of overlapping regional requests. That is a change of algorithm and proof
representation, not permission to skip descendants. First try a candidate finite
cover through the existing walker; a new checker may not be needed. In parallel,
investigate composed Apply→Route transfers, which may remove avoidable global
queue work. For future rule generation, optimize the cost of the resulting
guarded reduction program, not merely how quickly the first descending relation
is discovered.

This extends the [23 September review](finite_closure_architecture_review_2026-09-23.md).
It is a ranked research proposal, not implementation approval or a completion
claim; see the [independent critique](ibp_generation_independent_critique_2026-09-24.md).
The [subsequent code-free pilot](finite_cover_pilot_2026-09-24.md) now tests the
first proposal: all local walks finish, but neither initial-cover strategy beats
the direct baseline. Routing anchors do substantially reduce retained work and
memory. This narrows the next experiment to fixed-anchor amortization on a larger
unchanged control, not a claimed speedup or immediate full-run replacement.
No production implementation or saved rules changed for this review; the bounded
diagnostic inputs described below are new. The requested
numerator-filtration prototype was checkpointed as unbuilt, unrun TMP source
when this broader review took priority.

The contract remains all 67 owners of the explicit finite generic starting
envelope, currently the full-jet A≤24, R≤15, A−R≥9 pressure input, with every
descendant retained. Five loops is input data, not an engine specialization.
Finite nonminimal terminals are allowed under the existing explicit policy;
unfinished search, unknown guards and resource stops are not terminals. No
observable-specific QCD catalogue, master minimization or numerical evaluation
is required. Physical-profile assumptions and the distinction from universal
renormalizable-amplitude authentication remain as documented in the
[active domain contract](../finite_starting_domains.md#domain-contract).

## What is expensive, and what is actually mandatory?

There are three different computations:

1. **Discover relations:** prepare sector sources, seed/instantiate equations,
   perform modular discovery and native exact materialization, extract guards,
   and visit exceptional cases.
2. **Establish the requested closure:** inspect ordered saved rules and routed
   successor domains, retain every unresolved responsibility, and establish a
   compatible terminating concrete reduction.
3. **Use/package the result:** cold-load the admitted rules, routes, entry policy
   and finite terminals, then reduce requested integrals. This milestone does
   not require full coefficient back-substitution for every starting tuple.

The stopped expensive campaign was computation 2, using already saved rules.
It performed 8.27 million native inspections, retained 10.41 million pending
obligations, emitted 1.447 billion events and produced a 20.51 GB partial report.
Peak sampled aggregate RSS was about 235 GB. The allocation breakdown is not
known; neither all RSS nor all CPU can be attributed to retained diagnostics.
Zero observed frontiers describes a prefix, not complete rule coverage.

The finite-coverage **guarantee** is mandatory for this delivery. The particular
recursive regional walk, chronological ledger history and giant JSON rendering
are not mathematical requirements. Nor is a universal theorem over every
unbounded owner orthant mandatory. Conversely, a usable on-demand reducer is
not by itself the requested offline finite-coverage result. Keep these claims
separate in APIs and performance reports.

Existing evidence constrains the redesign:

- The native substitution change nearly halved matched A11/A12 traversal time
  with identical logical work. Useful speedup decreased CPU occupancy. This is
  evidence for eliminating work, not for maximizing busy-core counts.
- The default W50 budget is 25 inspectors, 24 admission helpers and one
  coordinator. I40 did not establish a wall-time win; I48 was slower. An audited
  no-crossing projection shortcut also produced no material matched speedup.
  A hotspot's total CPU percentage is not the saving of one call-site change.
- A12 has 24,293 Apply and 38,269 Route inspections, 1,929,785 successors,
  2,046,244 reuse hits and 250,297,311 containment charges, including
  53,654,813 conservative reverse-maintenance charges. Route is about 61% of
  inspections, **not** an established CPU share. Earlier inspection-duration
  sums were overwhelmingly Apply, so eliminating routes must be judged by
  queue/containment cost as well as native route execution.
- One A12 receipt records 8,385,480 attempted rule checks and 7,062,079 predicate
  checks. Admission preparation records 20,088 parallel batches and 1,403,685
  speculative requests. Its 180,502,038 speculative containment checks overlap
  ordinary accounting and must not be added to it. Coordinator commit/preparation
  wall measurements overlap worker execution; they are not exclusive CPU shares.
- The complete structural scan has 17,975 rules and 1,667,335 original RHS
  terms. Shift L1≤8 and nonpositive same-support Δ(A+R) are useful structure,
  not guard coverage. Its 1,958,316 potentially unsupported sign regions are
  not witnessed support changes.

Measurement details and timing boundaries are in the
[native profile record](finite_closure_native_profile_2026-09-23.md) and
[completed-slot study](owner_completed_slot_reuse_2026-09-23.md). Do not compare
timings across their different alternating blocks as if they were one experiment.

## Ranked competing architectures

Ranking concerns the present unfinished delivery, not eventual theoretical reach.
The savings below are mechanisms, not numerical speedup predictions.
Ranks 2–3 are provisional: the new fixed-point evidence strengthens guarded
dispatch refinement, while transfer composition first has to demonstrate that
it does not lose today's shared Route-domain reuse.

| Rank | Architecture | Potential work reduction | Cost / principal risk |
|---|---|---|---|
| 1 | Check a finite invariant cover | Replace repeated reachability waves by a bounded set of coverage/containment obligations | Low-cost falsifier; high mathematical risk that cheap, checkable covers do not exist |
| 2 | Compose transfers and share canonical subtopology services | Avoid materializing intermediate route obligations and repeated downstream work | Medium/high implementation cost; exact preimages and routing correlations are essential |
| 3 | Compile ordered guarded programs at the artifact boundary | Amortize guard partition/algebra over repeated applications | Medium/high cost; partition or decision-graph explosion can exceed the old work |
| 4 | Generate rules for downstream economics | Reduce fan-out, guard branching, rank excursions and later closure work | Modest isolated-case pilot; whole-owner program selection/revalidation is difficult |
| 5 | Separate discovery work from obligation dispatch; portfolio/block discovery | Reduce search critical paths and equation work when new relations are genuinely needed | Existing native building blocks help; no evidence this fixes the current saved-rule campaign |

### 1. Finite invariant cover: change the proof, not the required inputs

Propose a finite union C of native power-bounded cells containing E, the unchanged
entry set. Check every cell's dispatch/source conditions, original-term validity,
successor containment and explicit terminal/zero behavior. Every possible
conditional successor must be included too. Combine this with a separately
checked well-founded concrete transition order. The desired cost is proportional
to the complexity of a checkable C and its transfers, rather than to the number
of ways descendants rediscover overlapping regions. Neither quantity is known
to be small.

**Important simplification:** current inclusion reuse is against an immutable
snapshot's admitted domains, including pending work; it is not limited to
previously solved domains. Ledger retirement tracks inspection responsibility,
while campaign-wide exhaustion and frontier accounting retain successor work.
Therefore a small C may be supplied as initial queries to the **existing walker**.
An abstract self-edge is not automatically a bug, nor is it proof of termination.
Initial obligations must all be inspected, and an escaping successor must remain
real work. Root's source audit identifies this as a possible minimal experiment,
not an already validated replacement delivery path.

P=A+R is the relevant candidate potential, not A alone. Native diagnostics
already show same-support A increasing by one while R falls by one. Fixed-shift
sign cells admit exact affine ΔP bounds; after support/route compatibility is
established, per-edge max-plus bounds can be tighter than paying global L1 for
each support loss. Native slabs can represent P≤B as a finite union of A≤k,
R≤B−k predicates. But a few slabs can contain vastly more integer points and
generate vastly more internal guard pieces. Previous broadening did not prove
useful compression.

The 406 finite-width activation bands from the complete census are one optional
sufficient route to support filtration. So is testing canonical original-term
numerators on the crossing roots with existing native polynomial specialization.
Neither should become a new compulsory universal-certification project. An
eight-support unbounded band already has unresolved dispatch cells, whereas the
tested five-/six-/seven-support cases are easier. All eight uncertain cells
contain explicit geometric entry-envelope points (A=10–12, R=1–2, D=9–11).
Every chosen point selects an existing rule and produces only uniformly nonzero
successors: 353 children in total, no problems, unknowns or refusals. Four points
whose broad boxes were uncertain at rule 232 use later rule 239. This is evidence
for guard/priority refinement before relation regeneration, not whole-box coverage
or proof that broad-domain mixed truth is an implementation bug. The native API
emits only the first original/coalesced refusal records (support seven: two
records for 34 attempts); this is partial provenance, not an incomplete capture.
A correlated finite C may classify differently. Restrict a needed proof to C if the stronger
unbounded statement is difficult. The numerator prototype has no result yet.

**Cheapest falsifier:** for the unchanged required A11 entry set, propose a few native C templates that
contain every entry, seed them into the existing walker, and keep all escaping
descendants. Include synthesis, admission, checking, unresolved guards and peak
memory. This is a same-required-E comparison, not identical native inputs:
adding unreachable points must be reported. Do not first require all 406 bands
or a global potential theorem; the pilot may expose and retain escaping work.
If obligations stay near the initial cover and exhaust,
the next work is global descent/cold-runtime validation and packaging—not a new
walk engine. Reject a template when escaping images, internal splits or guard
unknowns grow to baseline scale. A gap, unknown or cap introduced only by added
points rejects that template, not E. A finite experiment allowance yields
“inconclusive/incomplete,” never a clipped proof.

### 2. Compose Apply→Route and make subtopologies reusable semantic services

The physical operation is “reduce into canonical lower-owner domains,” not
necessarily “publish every intermediate support box.” Prepare immutable
composed transfers that retain the source cell and conditional predicate,
perform native RHS validation, and pass pinched terms through the admitted
route map before global destination admission. Several parents can then use one
canonical lower-owner coverage block. Inspect only exact incoming union
differences when representable, with fallback to ordinary work when subtraction
fragments excessively.

This is **not** the already-tried per-owner FIFO scheduler. Current owners,
shared contexts and verified routes already exist. The new unit would be a
reusable domain-to-domain transfer or checked coverage block, not another ticket
for every path through Apply/Route. Coarse blocks offer independent mathematical
work across subtopologies; cells within the few hot owners still need parallel
inspection. Sixty-seven owners do not guarantee sixty-seven balanced tasks.

Do not flatten source conditions, discard original denominators, cancel before
original-term validity, or replace an affine image by a narrower convenient box.
Keep route multiplicities and constant monomials, exact source preimages,
power/rank predicates, aliases and immutable rule/route epoch identity. A more
precise correlated image may cost more than the artificial descendants it saves.
Raw potential support changes and same-support routing aliases prevent assuming
the entire owner graph is already a strict DAG.

**Cheapest falsifier:** replay one recorded expensive Apply region through the
existing native APIs, comparing unfused and composed one-hop outputs and failure
contexts before altering a campaign. That checks semantics only. First measure a
repeated multi-parent stream: fusion could lose current global Route-domain
deduplication and repeat routing once per parent. Require a net reduction after
charging this lost sharing before preferring fusion to guarded-dispatch reuse.
Then run unchanged A11/A12 with exact
responsibility/counter-equivalence checks where applicable, reporting changed
intermediate accounting explicitly. Count destination regions, containment work,
native CPU, total CPU, wall and retained bytes. Reject if composition only hides
route counters, loses preimages, or shifts more cost into correlation algebra.

### 3. Compile the guarded program, not just individual RHS vectors

Generation already knows case faces and exceptional branches. Saved artifacts
already retain ordered rules, fixed coordinates, affine equations, excluded
conjunctions, denominators and source conditions; there is no demonstrated loss
of this information. The opportunity is to compile its repeated interpretation:
intern authenticated predicates and common tests, retain an ordered decision
graph, and attach native transfer summaries to its leaves. Query evaluation would
restrict an existing program rather than rediscover its entire branch geometry.

A generation checkpoint need not store a globally disjoint exponential partition.
Share common decision prefixes and compile only the pieces justified by measured
reuse. Rule priority, terminal precedence, affine charts and original-term
validation remain part of the program. A transfer proof must name its source
cell/preimage, not merely cache target boxes. Unknown/refused results are not
successful leaves. Symbolica expressions remain native; this is not a new CAS,
general Presburger solver or coefficient-string representation.

Cheap algebraic filtration facts fit here: zero on each inactive positive-shift
crossing root is a sufficient original-term no-activation fact. Cancellation,
poles and guards still matter, and failing this stronger test is only unknown.
The artifact's coverage/provenance claims must not silently improve: present
saved candidate formulas do not automatically carry replayed original-source
IBP provenance.

**Cheapest falsifier:** one immutable owner, cold compile, then the same previously
completed query stream. Record actual inner predicate/term reuse, compiled bytes,
refinement count and complete native result equivalence. Reject if unique
source-cell predicates dominate, graph size expands excessively, or compile cost
does not amortize. A complete-query cache is not supported by the old A11 census:
all 27,806 effective native input domains were distinct. Conversely, recurring
fixed-coordinate layouts are not themselves safe cache keys.

Do not mistake a precomputed RHS permutation for this architecture. It is a
feasible CAS-free cleanup costing about 13 MiB across the current rules, but the
available profile identifies only four of 3,070 leaf samples explicitly in its
sort. It has no established material end-to-end payoff.

### 4. Make rule quality mean cheap downstream reduction, not first local success

The current search can stop at a direct hit or a successful discovered pivot;
`finish_rule` checks descent and extracts exceptional conditions. A relation can
be cheap to discover but expensive to apply because it creates many shift groups,
hard guards, broad routed images or long reduction chains. Optimize the entire
program's amortized cost instead: discovery plus guard/exception construction
plus the required coverage/application workload.

A small portfolio can vary source visit order or existing integral-order choices,
retain a few exact candidates, and compare fan-out, sparse coefficient size,
guard branching, support/potential behavior and actual native application cost.
Choose a Pareto tradeoff, not an assumed universal scalar score. Shared reusable
lower-owner rules may justify a more expensive upper-owner relation. Exploit
existing nonminimal finite-terminal policy where valid; never relabel unfinished
search as a cheap terminal.

There is already an isolated-case source-order API that preserves stored source
indices and exact replay, so an experiment need not replace the solver. However,
alternative rules change exceptional children and precedence: replacing one rule
requires rebuilding/rechecking the affected owner program, not patching an RHS
under the old coverage receipt. Native generated search traces and admitted
saved-formula provenance are distinct authorities.

**Cheapest falsifier:** one genuinely difficult case, two existing source orders,
one common CPU/work allowance. Exact-native validate each candidate and all its
exceptional responsibilities, then measure the same bounded application input.
Reject if extra discovery cost, guard splitting or downstream visits outweighs
application savings. Do not run this merely to occupy cores while the current
walker has no demonstrated missing targets.

External evidence supports changing the *equation set*, not a RustRed speed
prediction: Kira 3 targets seeding and equation selection, including symbolic
reductions. Its finite-system benchmarks do not establish RustRed's integer
guard coverage or authorize clipping descendants. [Kira 3](https://arxiv.org/html/2505.20197v1).

### 5. Separate source/discovery tasks from coverage dispatch

For actual new-rule generation, the cost model is roughly sector preparation
plus, per exceptional case, seed instantiation/modular row discovery, selected
exact materialization, guard extraction and exceptional geometry. Large terms,
equation counts and a few difficult cases can dominate; the number of output
rules alone measures none of these costs. RustRed already has a shared immutable
source system, a sector-local prepared basis, sector-level parallel execution,
directed missing-domain solving, modular discovery and multiple exact backends.
“Parallelize sectors” or “use finite fields” is therefore not a new design.

The further option is a two-level service: immutable family/source structural
templates plus bounded worker-local algebra, with explicit tasks for difficult
cases, independent modular witnesses and exact replay. Speculative local
portfolios can reduce a heavy-tail search critical path. They must retain
every exceptional child of the chosen result; deterministic publication need
not require serial speculation. Bound live coefficient memory and losing
portfolio work, and never count unverified modular candidates as delivered rules.
Across families, reuse structural templates only through authenticated native
maps; a shared topology shape does not make coefficient contexts interchangeable.

Block-triangular systems or spanning-cut discovery are more radical candidates
for reducing equation count before distribution. Blade demonstrates the
block-triangular direction, but also documents cross-sector “magic relations”
and assumptions in its spanning-sector procedure. Thus disjoint cuts cannot
simply declare omitted boundary sectors zero in RustRed. Reassemble and exactly
validate the complete uncut requested relation/domain, retaining overlap and
exceptional conditions. [Blade](https://arxiv.org/html/2405.14621v2).
FiniteFlow's graph-organized finite-field evaluation illustrates reusable
algebra tasks and parallel evaluations; it is inspiration for decomposition,
not a replacement backend or a certificate of guarded closure here.
[FiniteFlow](https://arxiv.org/abs/1905.08019).

**Cheapest falsifier:** profile one actual difficult generation case with existing
phase events; compare shared-basis case tasks or two bounded discovery portfolios
against the same native sequential search. Measure exact accepted coverage per
total CPU/RSS and losing speculative work. Only then consider a block/cut pilot.
Reject this lane as the explanation of current low utilization if search is
absent from the workload—which is the case for the measured saved-rule walk.

## Delivery representation and an explicitly different contract

Every candidate should separate compact operational state from optional detailed
diagnostic history. Intern shared domain/rule identities; stream bounded evidence
and persist pending/failed obligations without pretending an old partial JSON is
a checkpoint. Publishing “complete” before successor effects are durable is
unsafe. A small checking record may replace a huge execution history only when
it retains everything needed to check the actual finite-coverage claim.
This is useful infrastructure, not a proof that reporting dominates runtime.

Representing the same entry E by exact disjoint A/R/support strata, or validating
a larger invariant C, preserves the goal. Tightening E to only encountered QCD
integrals does not. A physics-derived union of sharper graph/forest profiles
could be a separate contract proposal, but must not silently replace the current
explicit envelope or cut descendants.

A different product could ship parametric rules with fail-closed **on-demand**
frontier repair, making actual integrals the parallel work units and dispensing
with exhaustive pre-delivery closure. That may be pragmatic in a streamed
counterterm pipeline. It is not the current all-67 offline guarantee: an unseen
input may require repair or fail. User approval would be needed to change that
contract. The present bottleneck may be the chosen assurance procedure rather
than absent IBP relations; acknowledging that does not discharge the assurance.

## Decision gates

First try the smallest same-scope cover experiment using existing machinery;
otherwise prioritize a measured composed-transfer/guard-program pilot. Keep the
current default scheduler. New discovery architectures need actual generation
profiles, not analogies to the saved-rule walk. Across all lanes, success means
less total work or demonstrably better same-input wall/CPU/memory for unchanged
coverage—not a busier machine, smaller report alone, or sampled agreement.

Safety gates are unchanged: native exact algebra, retained original guards and
nonzero-term child validity, explicit conditional dependencies, all unresolved
obligations, checked routing/descent, immutable artifact identity and cold runtime
compatibility. The framework remains generic Rust plus Symbolica. No topology-
specific engine rule, alternate CAS, new minimal-master requirement or claim of
full five-loop completion is introduced.

### Source anchors

- Discovery and existing reuse: `solver/search.rs`, `solver/sector.rs`,
  `solver/sector/domains.rs`, `solver/execution.rs` under `rustred-core/src`.
- Admission and existing artifact payload: `candidate_bundle/codec.rs` and
  `candidate_bundle/load/owners.rs` under `rustred-app/src/application`.
- Ordered term validity and transfers: `solver/candidate_reduction/owners/domains/`
  and `solver/candidate_reduction/routed/{trace.rs,domain_overcover/}`.
- Pending inclusion and operational responsibilities:
  `rustred-app/src/application/routed_campaign/walking/` and its native queue.
- Raw bounded evidence: `TMP/no-crossing-compare.VaG3Lg/matrix/`,
  `TMP/all67-structural-census.JXPNU6/`, `TMP/activation-band-pilot.of1y5J/`,
  `TMP/higher-support-bands.fZTGG2/`, `TMP/fixed-band-witnesses.ENFXZZ/`,
  `TMP/one-step-applied-probe.avTdVa/`.
  TMP is local evidence, not a portable documentation dependency or new artifact
  provenance authority. Independent review and current objective remain linked
  from the active project documentation.
