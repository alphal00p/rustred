# Rethinking finite IBP closure for multicore execution

## Executive recommendation

The user requested an independent, deliberately broad redesign study and a
separate adversarial assessment. Both are complete. They agree on a crucial
distinction: the presently stalled workload is **following saved IBP rules over
overlapping integer domains**, not discovering new rules by elimination. Better
parallel elimination or reconstruction would not directly fix this bottleneck.

The highest-upside proposal is to check a compact, closed collection of domains
instead of repeatedly constructing the whole overlapping reachability history.
The lower-risk first production experiment is to share immutable rule-transfer
plans and, where sound, reuse their checked domain restrictions. Neither has a
demonstrated speedup yet. Keep the default scheduler and full starting-domain
contract unchanged until complete matched pilots justify changing them.

This is a research agenda, not additional universal certification work. The
objective remains the finite generic starting envelope in
[the active plan](../finite_starting_domains.md), with all descendants retained,
finite nonminimal terminals, concrete-runtime compatibility and a portable
package. Minimal masters and numerical evaluation remain deferred.

## What is actually demonstrated

The stopped all-67-owner run completed 8,274,060 native inspections but retained
10,413,595 pending native obligations and one cancelled partial. It emitted
1,447,411,449 events, peaked at about 234.76 GB sampled aggregate RSS, and produced
a 20.51-GB partial JSON report. Zero observed frontiers is only a fact about the
inspected prefix. That report is neither closure nor a resumable checkpoint.

The following **historical, pre-reclamation controls** used saved rules, not
regenerated ones. Their frozen executable included bounded completed-result
reclamation in Ordered only. They did not isolate sector concurrency with
otherwise identical slot handling, and no longer describe the latest scheduler.

| Historical completed control, 50-worker budget | Ordered | Concurrent owner |
|---|---:|---:|
| A10/R1/D9: traversal seconds | 1.403919 | 2.950783 |
| A10: native inspections | 7,432 | 8,797 |
| A10: sampled busy cores | 15.24 | 10.27 |
| A11/R2/D9: traversal seconds | 7.777882 | 9.593036 |
| A11: native inspections | 27,806 | 32,333 |
| A11: events | 695,918 | 744,009 |
| A11: sampled busy cores | 9.36 | 7.64 |
| A11: peak process RSS, KiB | 584,360 | 628,172 |

A11 represents 45,342 starting tuples using four symbolic regions, four saved
owners and 86 routes. Its descendants are uncut; scheduled region rank bounds
reach three, which does not establish that concrete rank-three keys are reached.
Both controls finish without unresolved responsibilities. Their starting-domain
and publication accounting pass the raw-receipt checker. These are single
50-worker pairs on a shared host, not confidence intervals or full-family solves.
CPU samples are heartbeat-labelled windows, not precisely isolated traversal CPU.

In those controls the concurrent lane permitted 25 inspections within one owner
but was slower and did more work. Both policies reported zero full-buffer
producer backpressure. Later completed jobs occupying physical slots were a
distinct measured limitation. That limitation has since been addressed: both
policies now share bounded completed-result reclamation. Six unchanged A11
controls give median traversal 7.484 s concurrent owner versus 8.284 s Ordered,
9.65% lower wall time, with about 18.8% more native visits and 24.8% more
whole-process CPU. This is a useful bounded improvement, not a full-envelope ETA.
See [the completed-slot results and corrected CPU analysis](owner_completed_slot_reuse_2026-09-23.md).
The historical busy-core values above use their original sampling windows and
must not be directly compared with the later corrected estimator. The fixed
25-inspector/24-helper/one-coordinator budget remains a separate measurement
question, not evidence that all helpers do useful work. Fifty requested workers
mean at most 25 native inspectors in this configuration. If admission helpers
remain lightly used, native slot reclamation alone cannot occupy fifty cores.
Test a measured inspector/helper ratio or a work-conserving common pool as a
separate hypothesis, retaining admission progress and bounded buffers; do not
attribute the entire shortfall to insufficient same-owner dispatch.

See [the measurement and gate record](owner_concurrent_inspection_2026-09-23.md).
The larger A11 receipts are in `TMP/owner-larger-pilot.zP4XkJ/run-a11/`; the prior
controls are in `TMP/owner-concurrent-controls.tD1zsL/`. No live full campaign is
being restarted on the strength of these measurements.

The subsequent single A12/R3/D9 pair also passes independent raw review:
357,192 entry tuples over the same four owners and 86 routes, with every
descendant retained. Concurrent-owner traversal is 26.746 s versus 28.427 s
Ordered (5.91% lower), with 24.13% more native visits, 11.84% more events and
9.45% more peak process RSS; whole-command CPU is 5.59% lower in this pair.
Corrected sampled activity is about nine busy cores for both. The maximum
scheduled finite rank bound is four, not evidence of concrete rank-four keys;
there are no unresolved frontiers, pending obligations or resource stops.
This one pair supports another bounded wall-time improvement, not a replicated
scaling conclusion or full-envelope estimate. Details remain in the
[completed-slot measurement record](owner_completed_slot_reuse_2026-09-23.md).

## Competing redesigns

| Proposal | Where substantial savings could come from | Main failure mode |
|---|---|---|
| Check a finite closed cover | Eliminate recursive waves and much of their historical bookkeeping | No compact checkable cover exists in the chosen representation |
| Reuse immutable rule-transfer proofs | Avoid repeating algebra, guard and geometric work across overlapping queries | Safe reuse is rare or synchronization costs more than recomputation |
| Inspect only exact union differences | Avoid inspecting points already handled by several overlapping regions | Differences fragment into too many pieces |
| Retain correlations in routing images | Avoid artificial descendants created by independent coordinate bounds | More precise image operations cost more than the work they save |

These are alternatives to evaluate separately. Their prospective gains cannot
be multiplied or assumed to combine positively.

### 1. Check a finite closed cover rather than construct the smallest one

Let E be the starting set. Propose a finite union C of bounded native regions
for each owner and routing phase. C may include unreachable points. Check that:

1. E is contained in C.
2. Every point of C selects a valid saved rule, explicit terminal, admitted zero
   or valid route; all relevant guards and source conditions are accounted for.
3. Every possible successor emitted by those inspections remains in the
   destination C or reaches an explicit terminal/zero.
4. Every inspection and delivery completes without an unresolved obligation.
5. Concrete transitions strictly decrease a checked well-founded order.

This would let independent workers check immutable cells against an immutable
destination cover. The computation need not repeatedly admit the same overlap
through many different paths. It also decouples mathematical completion from
the chronological order of diagnostic publication.

For example, a rule I(n) -> I(n-1) for n>1 with I(1) terminal can reduce every
entry 1<=n<=100. Checking the complete interval's boundary and translated image
can establish that result without storing one hundred successively overlapping
requests. The example illustrates the mechanism, not a model of measured IBP
cost. A coarse region for x -> x-1 with a missing rule at x=0 must fail: an
abstract self-loop or strongly connected component cannot discharge that gap.

The critic's strongest objection is synthesis: how do we find C without doing
the same expensive traversal? Try a bounded collection of native templates
suggested by rule boundaries and observed domains, then check them exhaustively.
An escaping successor or exposed exceptional guard forces refinement or explicit
failure. Observations can suggest a cover; samples cannot establish it. Broader
regions can expose unreachable difficult guards and make this approach worse.
Earlier experiments merely adding wider initial bands did not establish a gain.

Global descent must be checked explicitly. A possible order combines support
cardinality, routing phase and owner-local integral order. The existing concrete
tracer's support-decrease checks cannot simply be assumed to cover every symbolic
visitor transition. Same-support routing cycles must not be hidden.

This proves operational coverage relative to the admitted saved formulas. The
candidate-reduction model explicitly does not automatically attach regenerated
original-IBP provenance to every saved candidate formula. A new domain checker
cannot silently strengthen that artifact's provenance or establish master-basis
minimality. Preserve the current milestone's authority boundary.

First radical pilot: construct an explicit small C for the completed A11 input
and check it in a separate, non-authoritative mode. Include synthesis time,
all escaping descendants, exceptional cases, cover inclusion work and memory.
Reject missing boundaries, singular source conditions and non-descending cycles.
Stop this experiment if templates or native checking work expand to the old
scale. Its experimental budget is not a reduction cutoff.

The conceptual precedent is sound abstract fixed-point approximation and
counterexample-guided refinement, not an IBP-specific guarantee of a small cover:
[Cousot and Cousot](https://www.di.ens.fr/~cousot/COUSOTpapers/POPL77.shtml),
[Clarke et al.](https://www.cs.cmu.edu/~emc/papers/Conference%20Papers/Counterexample-guided%20Abstraction%20Refinement.pdf).

#### A constructive candidate from the saved power order

A subsequent source-level refinement offers a candidate without first computing
reachability. Write `P=A+R=sum_i|a_i|` and let m be support cardinality. The saved
uncut candidate order compares, within a fixed sector, the corner distance
`C=A+R-m` before degree and coordinate ties. C is the sum of nonnegative local
coordinates, so its integer sublevel sets are finite. Every admitted same-sector
Apply therefore has `delta P<=0`; exact saved-order ties still establish strict
local descent. Dominant lexicographic weights are unnecessary just to bound
these phases. This concerns the persisted uncut ordering, not arbitrary cut
priorities in the source solver. See the
[native ordering](../../crates/rustred-core/src/sector/ordering.rs) and
[saved-order reconstruction](../../crates/rustred-app/src/application/candidate_bundle/load.rs).

The admitted route geometry is favorable too: active denominators map by a unit
bijection and inactive numerator substitutions are affine. Every monomial
endpoint satisfies `A'<=A` and `R'<=R`. The native power-bounded route overcover
preserves those caps, not merely its concrete endpoints. Affine constants can
increase D, so an old upper D bound must not be retained without proof.

Suppose L bounds the L1 norm of every potentially live saved Apply shift. If
every support-changing Apply strictly **decreases cardinality**, then from an
entry bound `P<=B0` at support m0 the layer bound
`B_m=B0+(m0-m)L` is inductive: same-sector steps do not increase P, each
support-loss step increases it by at most L, and routes do not increase it.
Take the maximum over eligible initial owners when necessary. The full generic
entry gives B0<=39; the A11 entry gives B0<=13. This is conditional candidate
synthesis, not a completed invariant check or a new descendant cutoff.

There is an important current gap. The
[symbolic Apply engine](../../crates/rustred-core/src/solver/candidate_reduction/owners/domains/applied/engine.rs)
checks child support against the saved root and proves local descent; a changed
support's mask is compared before C. Those tests alone do not exclude same-size
support exchange followed by a canonical route reset. A structural example is
`(1,0,-t) -> (0,1,-t-1)`: support changes from `100` to lexicographically smaller
`010`, yet P increases. A coordinate-swap route could reset the mask. This is a
counterexample to inferring global descent from those tests, **not an observed
rule in the saved workload**. The concrete tracer rejects this transition.
Cardinality decrease suffices for the mathematical bound, but compatibility with
the [current concrete tracer](../../crates/rustred-core/src/solver/candidate_reduction/routed/trace.rs)
requires the stronger strict-subset condition, with no activation. Neither
condition has been exhaustively established here for the proposed enlarged cover.

The bound needs no general inequality solver. On fixed support m,

`{A+R<=B} = union_{k=m..B} {A<=k, R<=B-k}`.

These O(B) overlapping slabs use existing positive-power and actual-rank caps.
Choose k=A to prove coverage. Native sign partitioning fixes crossing coordinates
and translates exact delta A and delta R, so a successor has a direct destination
slab witness; retain aggregate predicates rather than only their projected boxes.
Original-term source validity must still precede cancellation. The packed
candidate-bundle format supplies a coarse L<=127N, but that is impractically loose
and does not apply to arbitrary wide artifacts. A tight saved-shift census or
fully checked candidate parameter remains needed; no custom LP/CAS is proposed.

Breadth may defeat the idea: even P<=13, before any support-loss allowance,
contains 730,626 tuples across the four A11 supports, versus 45,342 entry tuples,
about16.1 times as many. It can expose unreachable guard holes. It is not valid
to retain entry R/D limits on descendants just to make this cover smaller.
The falsifiable pilot is one bounded A11 cover experiment using the existing
native inspectors: count synthesis and all local checking, audit global support
compatibility, and require every successor to have a destination witness. Reject
or explicitly refine escaping successors or unresolved guards; never clip them.
Compare completed useful coverage, total time, native work and retained memory
against the current matched walk. Large L, excessive guard refinements or checking
cost comparable to the old traversal would disprove this candidate's usefulness.
Even success would remain operational coverage relative to admitted formulas,
not original-IBP provenance, minimal-master certification or full-family closure.

### 2. Share compiled transfers and reusable checked restrictions

The current native visitor repeatedly groups RHS shifts, splits boundary cells,
restricts coefficients, checks original-term source conditions and proves local
descent. First prepare immutable structural plans once per loaded rule. Then
measure whether repeated checked restrictions can also be shared.

A more structural variant is to compile a guarded dispatch/transition graph at
load time or alongside generation. This is **not recovery of lost case data**:
the [candidate codec](../../crates/rustred-app/src/application/candidate_bundle/codec.rs)
already preserves ordered rules, fixed coordinate faces, affine equations,
exclusion conjunctions and exact RHS coefficients. It does not persist a checked,
disjoint first-applicable partition. The native matcher repeatedly walks source
conditions, terminals, fixed/equality/exclusion tests and denominator guards;
its guard resolver specializes predicates and reconstructs zero-locus tests
against each query's box/rank. Sharing that decision structure could remove work
by construction, instead of waiting for a post-hoc cache hit.

Start with native coordinate/separable tests and lazy unresolved leaves that
fall back to today's matcher. Preserve batch/rule precedence, original-term
source validity, poles, source-cell/preimage relations and immutable program/order
identity. Do not eagerly materialize all predicate combinations: the number of
disjoint cells can explode. The existing guarded single-rule visitor is not a
complete substitute: it omits first-priority coverage, and its power-bounded
wrapper currently rejects nontrivial A/D bounds. A pilot must compare every
selected classification/guard/refusal and successor obligation, include graph
construction time and retained bytes, and reject a graph whose total cost exceeds
the current dispatch. Reusing generation-side partition work is a hypothesis;
saved seeds/cases still do not confer replayed original-IBP provenance.

Distinguish three objects: an exact restricted expression, a zero/nonzero proof
on a domain, and a complete selected-rule transfer proof. An expression-cache
hit alone proves neither guard applicability nor coverage. Proof keys need the
immutable program, selected-rule precedence, owner, complete source geometry,
source-condition policy and any affine chart. Hash collisions require equality
checks. Unknown/refused work remains unknown/refused.

A wider valid proof may restrict to a smaller source after native containment.
Clipping only its target boxes does not in general recover the image of that
smaller source. Keep source cells and transport information. Preserve source
conditions for every original term before coalescing equal shifts.

This can reduce real native work while leaving queue responsibility semantics
unchanged. Cap bytes and in-flight construction; eviction falls back to native
inspection. The decisive experiment replays an actual complete A11 input stream
with cold caches and compares every classification and successor obligation,
then repeats the complete walk. Count cold preparation, locking and retained
memory. Merely timing pre-grouped shifts is not evidence for proof reuse.

### 3. Subtract the union already inspected

Currently, containment in one representative can reuse work, but several
representatives can jointly cover a new domain without any one containing it.
For D and already inspected union U, inspect only D minus U.

For instance, inspected intervals [1,5] and [4,8] jointly cover [2,7]. Neither
alone contains it. In several dimensions, however, the exact residual can split
into many pieces, and the native A/R/D geometry is not closed under arbitrary
complement. Start only with exactly representable D-bands or coordinate slabs;
on a fragmentation limit, inspect the original domain conservatively.

Reuse of a completed local inspection does not mean its descendants are already
solved: they must remain in the global ledger. Initially exclude reserved work
from subtraction. Later reservation-based reuse would require explicit retained
dependencies and failure handling; circular reservations must not certify each
other. This is the central objection to a naive distributed work-stealing set.

Incremental fixed-point methods motivate propagating new facts, but do not supply
the required integer geometry or closure proof:
[Differential Dataflow](https://www.cidrdb.org/cidr2013/Papers/CIDR13_Paper111.pdf).
Symbolic saturation also has important representation and parallelization
trade-offs: [Ciardo, Zhao and Jin](https://arxiv.org/html/0912.2785v1).

### 4. Avoid unnecessarily loose routed successor domains

An affine numerator substitution can produce correlated target powers. Separate
upper bounds may admit combinations that cannot occur together. Preserve selected
joint support bounds, or retain a lazy source-to-image relation until needed.

This must never omit actual monomials, including constant terms and cancellations
against denominators. Exact low-rank Symbolica substitution can measure how loose
current covers are; it is not a proposal to expand all high-rank numerators.
Lazy representations may only postpone the same explosion. Prioritize routes
whose overcovered points create expensive new obligations, not points immediately
reused elsewhere. Fall back conservatively when refinement costs too much.

## Storage, termination and the generation algorithm

Compact typed proof records and a separate obligation ledger could also replace
large retained JSON histories. The current code stores `Vec<serde_json::Value>`
records and later pretty-serializes the assembled report. This identifies a
measurement target, not an explanation of the entire 235-GB peak. Profile queues,
indexes, dependencies and reports separately before making a memory claim.

Stream optional diagnostics independently from authoritative successor effects.
A durable restart must retain every pending/partial/failed responsibility and
validate program and input identity. Completion cannot be persisted before its
successor effects are durable. The old partial JSON is not retroactively a
checkpoint. Distributed termination accounting detects completion only when a
computation actually finishes; it does not make an infinite expansion finite.
[Dijkstra and Scholten](https://www.cs.utexas.edu/~EWD/transcriptions/EWD06xx/EWD687a.html).

There is also a broader generation idea: select valid rules for low *downstream*
closure cost, not merely the first locally descending rule. Smaller fan-out,
simpler guards and tighter routing might beat a cheaper individual coefficient.
Any composed rule must retain shifted guards, original source validity and exact
replay. The current campaign repairs genuine fixed missing targets. A measured
repeated transfer pattern may motivate a separate future rule-generation
experiment. Block-triangular discovery and reconstruction work
such as [Blade](https://arxiv.org/html/2405.14621v2) is relevant inspiration, but
its speedup factors do not apply to this regional traversal by analogy.

The critic also checked the generating-function paper previously suggested by
the user. Selected-path consistency checks in section 3.5.2 are not exhaustive
guarded closure; predicting a minimal master count is a different, unnecessary
requirement here. [Feng et al.](https://arxiv.org/html/2605.09541v1).
The existence of finitely many masters is likewise not a constructive campaign
bound or completion estimate. [Smirnov and Petukhov](https://arxiv.org/pdf/1004.4199).

## Symbolica and implementation boundaries

Both reviews inspected the current Symbolica 3.0 manifests, implementations and
RustRed call sites. Native polynomial substitution, variable shifts, rational
polynomial arithmetic and reconstruction already exist. RustRed already uses
Symbolica reconstruction in `solver/discovery/semi_numerical.rs`. Historical
2.2-era statements that reconstruction is unavailable are not current guidance.
Modular discovery/reconstruction remains a proposal mechanism, not integer-domain
guard authority. [Official polynomial documentation](https://symbolica.io/docs/guide/polynomials.html).

Neither review established a ready-made general integer-region union/difference,
Presburger entailment or invariant-synthesis API. An integer solution-domain
filter is not such an API. Begin with existing RustRed geometry and Symbolica
algebra; audit any new algebra requirement precisely before implementation.
No custom CAS, topology dispatch or loop-count-specific algorithm is proposed.

## Agreed next gates and independent review

1. The narrow completed-slot fix and independent A11 measurement are complete;
   retain that baseline while testing larger inputs. More occupied workers
   without lower elapsed time is not a win.
2. The [native CPU profile](finite_closure_native_profile_2026-09-23.md) now
   identifies substitution as a concrete hotspot; test the small native-API
   execution-order change before implementing a broader reuse cache. Cold
   compiled-transfer/decision-graph experiments remain separate and must count
   preparation, retained bytes and exact native-result equivalence.
3. Separately test a small proposed finite closed cover without replacing the
   production traversal. This is the highest-upside research experiment.
4. Pursue exact union differences or routing correlations only when measured
   repeated work identifies their opportunity. Keep failed pilots as evidence.
5. Require a complete larger matched control and correctness audit before
   restarting the full 67-owner envelope. There is still no credible full-run ETA.

The independent critic required explicit global descent, precise cache keys,
original-term source validity, finite-cover synthesis failure handling,
reservation-cycle safety and an unchanged artifact-provenance boundary. The
architect incorporated these objections. Both distinguish strategic potential
from implementation readiness. Neither claims a new architecture is already
faster or that fifty-core saturation alone would complete five loops.

Full working reports remain locally available in
`TMP/finite-closure-architecture.972XZb/ARCHITECTURES.md` and
`TMP/radical-ibp-critic.Mv4pvT/independent_critique.md`. They were prepared by separate
agents; neither ran a solver or changed production code. This synthesis records
their recommendations, objections, primary references and falsifiable pilots.
