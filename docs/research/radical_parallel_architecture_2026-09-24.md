# Radical parallel architectures: three genuine challengers

24 September 2026. Research/design only: no new solver run, build, input or
production change. This supplements, rather than renames, the
[previous whole-system review](ibp_generation_architecture_review_2026-09-24.md).

## Executive recommendation

**Keep measured regional sharing as the leading path; challenge the ownership
and representation of work before building another scheduler.** The new
alternatives below deliberately change one of three assumptions: that every
entry must share one live closure ledger; that every successor must be admitted
as it arrives; or that reachability should be represented by successive boxes.
None currently has evidence sufficient to replace the saved-rule walker.

For example, suppose ten different entry regions all need the same difficult
lower-sector domain. Today's shared anchors can arrange one inspection. Two
independent entry shards might inspect it twice but finish sooner by avoiding
global coordination. A batched fixed-point engine would instead try to admit it
once per frozen reconciliation epoch. A relational program could represent the
shared guarded transfer without rediscovering its rectangular image per path.
Which wins depends on duplicated algebra, critical-path length and memory—not
on how many processors can be made busy.

The first recommended experiment was the **bounded all-owner anchor control**,
not the full A24/R15 restart. It subsequently stopped at its work gate; the
[post-pair update below](#post-pair-update-two-different-publicationspan-regimes)
supersedes the initial suggestion to test two entry shards with duplicated
anchors unchanged. Do not commission the relational engine before measuring how
much expensive work is actually caused by lost correlations.

## The contract and the evidence

The required set remains the explicit generic finite entry envelope over all
67 owners: currently A≤24, R≤15, A−R≥9, under the documented physical-profile
assumptions. Every descendant remains required. Finite nonminimal terminals
are allowed; an unknown guard, resource stop or unfinished search is not a
terminal. No observable-specific diagram catalogue, sampled closure or new CAS.
The [domain contract](../finite_starting_domains.md#domain-contract) distinguishes
this mathematical input from automatic authentication of every physical model.

Three computations must remain distinct:

1. **IBP discovery:** source equations, case search, modular selection, exact
   replay, guards and exceptional cases.
2. **Saved-program assurance/repair:** establish coverage and terminating usable
   reduction for the finite entry set; generate repairs only for genuine missing
   targets, not merely broad-box uncertainty.
3. **Delivery/use:** cold-load rules, routes, entry policy and explicit terminals.
   A fully back-substituted coefficient table for trillions of entries is not
   required. Existing bundles already carry terminal keys.

The expensive stopped run was computation 2, not demonstrated slow elimination.
It stopped after 8.27 million native inspections with 10.41 million pending,
1.447 billion events, about 235 GB sampled peak RSS and 20.5 GB partial JSON.
No observed frontier does not settle pending work; memory allocation shares are
not known. The partial output is not a resumable checkpoint.

| Evidence | What it establishes—and what it does not |
|---|---|
| A12 fixed anchors: 22.388→11.720 s traversal; CPU 174.73→128.34 s; RSS 1,403,176→771,516 KiB | One audited same-required-E pair. Traversal includes anchor work but excludes cold preparation, which is separately reported and charged in whole-command costs. Inspections fall 62,562→23,863 and containment charges 250.30m→37.95m, while native operations **rise 1.67%**. Not an algebra-cache result or an all-67 forecast. |
| A11 broad anchors are slower than direct E | Bigger reusable domains can introduce more work than they save. A12 does not invalidate this counterexample. |
| A12 represented regions reach P=A+R=17 | P13 anchors are partial reuse opportunities, not an invariant or a descendant cutoff; these are region extrema, not individual reached-integral witnesses. |
| Default 25 inspectors/24 helpers/1 coordinator; 40 inspectors do not establish a wall win, 48 are slower | Configured W50 is not 50 independent inspectors or proof of exploitable span. |
| Exact substitution reordering nearly halves matched traversal; a projection shortcut does not materially help | Per-visit work matters, but a hotspot's percentage is not the saving of a particular patch. |
| All-owner shift census: L1≤8, nonpositive same-support ΔP; narrow support canary passes | Useful structure, not all-owner support filtration, guard coverage, original-IBP provenance or concrete-runtime completion. |

See the [audited cover measurements](finite_cover_pilot_2026-09-24.md) and
[native profiles](finite_closure_native_profile_2026-09-23.md). The local A12
receipts are `TMP/a12-anchor-amortization.qqQTJk/`. Timings from different blocks
must not be multiplied into a projected speedup.

## A cost model that can reject attractive diagrams

Let W be total work after duplication, S the longest necessary dependency chain,
and B the bytes moved through a limiting memory/I/O resource. The basic lower
bounds are W/p, S and B/bandwidth; changing the architecture can change **all
three**, not just p. Native-operation counts are heterogeneous, not calibrated
units of W. Cold loading, final checking and output also count.

We do not know the application's Amdahl serial fraction. Coordinator wall times
overlap worker execution; main-thread CPU includes setup/output; Route visit
counts are not Route CPU shares. Sixty-seven owners are not 67 balanced
independent tasks. A strict-support DAG may organize some transfers, but
same-support reduction, routing aliases and compatibility obligations remain.

## 1. Exact entry shards: deliberately give up global deduplication

**New decomposition.** Partition E exactly into disjoint native domains E1,…,Ek.
Each shard owns its entire descendant closure and local responsibility ledger,
crossing owner boundaries freely. Event publication in one shard cannot fence
another. The intended in-process version shares immutable admitted programs but
not mutable domain indices. This is not per-owner FIFO or work stealing: its
mathematical task is a complete reduction obligation for an entry slice.

**Correctness.** Prove the entry partition is exact; require every shard to close
under the same snapshot, priority, source/pole conditions, routing and terminal
policy, with no descendant restriction. Their union then covers E. Do not treat
a sibling's pending work as solved, and do not omit a shard because training
suggests it is easy. Concrete descent/runtime and delivery gates remain.

**Scaling and memory.** This can remove cross-shard admission/commit dependencies
and reduce S despite increasing W. The strongest counterexample is precisely
the ten-parent example: every shard duplicates the same costly lower-owner
closure. A skewed hard slice still determines makespan. Private ledgers and
coefficient scratch multiply memory; separate-process prototypes also duplicate
loading. Shared rules reduce that latter cost, not duplicated proof work.

**Cheapest falsifier.** Two shards, not fifty. For an already completed finite
control, freeze a native integer split before timing—e.g. A12's D≤10 versus
D≥11 within the original D≥9 input. The existing `power_bounds` fields encode
D∈[9,10] and D≥11 while retaining every original box, rank and A predicate.
Because D is integral this is an exact partition; each side is nonempty for
every installed support size 5–8 (A=9,R=0 and A=11,R=0 give witnesses).
Each CLI invocation follows all descendants across all owners: the entry cut
does not remove downstream coupling. Required E is disjointly partitioned;
duplicated anchor inputs may overlap and are extra checked work.

Use one aggregate W50 CPU/memory budget including both coordinators, helper
pools and loaders—not W50 per shard. Two default W25 processes currently reserve
12 inspectors + 12 helpers + 1 coordinator each: total 24/24/2, compared with
the shared W50 default 25/24/1. These are reservations, not measured activity;
account for any explicit alternative split prospectively. Compare combined cold
makespan, total CPU, tree RSS and complete scope against a fresh fastest shared
anchor baseline, not only the slower unanchored case. Charge any duplicated
anchors and the final union check. Existing-process duplication is a truthful
first falsifier, not a measurement of a future shared-memory implementation.

**Reject** if duplication, imbalance or memory eliminates the wall gain under
that total budget. A stopped shard makes the comparison incomplete. No sampled
keys establish the partition's closure. Implementation cost is low for the
diagnostic, medium for a shared-program multi-ledger application.

## 2. Frozen micro-epochs: make admission a set operation, not an arrival stream

**New publication model.** Freeze a finite admitted frontier and immutable
program snapshot. Inspect its regions independently into bounded worker-local
outboxes; reconcile successor proposals in deterministic batches by phase/owner,
using existing exact equality/containment. The next epoch contains the new
residual obligations. A region completed locally is *inspected*, not globally
solved. Potentially duplicated inspection can be worthwhile if it avoids a
longer coordination path, but the duplicate work must be counted.

The novelty is batch reconciliation of a fixed-point computation, not bigger
queues. Existing pending-region reuse already captures much sharing, so the
unproved benefit is coalescing proposals **before** repeated global admission,
index maintenance and diagnostic publication. Coarse synchronous stages are
inspired by the computation/communication separation in the
[bulk-synchronous model](https://web.mit.edu/6.976/www/handout/valiant2.pdf);
that model is not performance evidence for RustRed.

**Correctness.** Bind every result to its exact snapshot/source query and preserve
original-term checks, conditional edges, Problems and refusals. Commit all
outgoing responsibilities idempotently before discharging their producer's
responsibility; reclaiming a finished physical worker slot is a separate event. Dropping
an included proposal requires a real containing obligation, not an abstract
cycle. A repaired rule creates a new epoch and invalidates affected old proofs;
it cannot mutate underneath inspectors. Preserve global budgets and report every
incomplete producer on cancellation. Different admission order may change work
and cap prefixes; it may not change the completion guarantee.

**Scaling and memory.** Fewer reconciliation rounds may reduce traffic, but an
epoch barrier waits for its slowest guard region. A large final batch can lengthen
the trusted commit path and hold far more successor/coefficient payloads. Hence
bounded micro-epochs and outbox bytes, not an unbounded “finish the whole sector”
barrier. Owner-local asynchronous batches are a related alternative, but they
recover cross-batch causality and termination-detection costs; naming them
barrierless does not remove those costs.

**Cheapest falsifier.** A predeclared bounded frontier from a completed control:
run the same existing native inspectors, comparing streaming admission with
frozen batch reconciliation. Record outgoing proposals, new residuals, duplicate
native work, longest job, reconciliation wall, peak outbox bytes and final
responsibilities. An offline replay of recorded proposals can reject poor
deduplication economics, but cannot demonstrate live wall scaling or successors
absent from the trace. Only a subsequent complete unchanged-E control establishes
end-to-end benefit. Do not infer available serial savings from overlapping
commit timers.

**Reject** if existing reuse already removes nearly all batch redundancy, barrier
stragglers dominate, or reduced queue objects come with more CPU/RSS or no wall
gain. Medium/high implementation cost: changing transactional responsibility
publication is more substantial than replacing a work queue.

## 3. Relational reachability: carry source predicates instead of unfolding boxes

**New mathematical representation.** Represent a reachable set as shared guarded
source-to-target relations and exact disjunctions, not just a new target box at
each path. A constant shift carries n′=n+s together with the source guard and
coefficient-nonzero condition. Joins share predicate/transfer nodes; when native
implication is unavailable, keep an explicit residual or fall back to the
ordinary visitor. This is more radical than caching guard evaluations: it changes
the set being represented and the repeated universal statements being checked.

A toy illustration—not a claimed RustRed bug—is the union {(0,2),(2,0)}.
Replacing it by coordinate ranges, even with the sum fixed at two, admits the
diagonal (1,1). A later u−v guard then has a new branch. Preserving the disjunction
avoids checking that artificial point. Current Apply already retains exact
box/rank/A/D images; do not blame it for discarding those constraints. Actual
possible losses include routed over-covers and omitted conditional-nonzero
predicates in the box-only queue. Their *cost contribution* is not measured.

**Existing seam and hard barrier.** `OwnerGuardedDomain` and its image already
retain native case equations, whole excluded conjunctions, original denominators
and lazy shift pullbacks. However, this API nominates one rule rather than proving
ordered dispatch, is borrowed rather than queue-owned, and rejects nontrivial
A/D bounds. Routing is not generally a single invertible affine exponent map:
numerator transport can expand terms. A general Boolean/polyhedral entailment
engine is not an audited capability of pinned Symbolica 3.0. Existing native
affine charts/polynomial operations may support a restricted seam; no custom CAS
or invented solver is authorized.

**Correctness.** Quantify over the full finite E and retained descendants, with
exact source preimages, first-rule priority, original poles and every conditional
dependency. A compact graph or SCC is not a termination proof. Either check a
closed cover under these relations and concrete descent, or retain explicit
escaping work. P13 is not such a cover. Unknown entailment stays unknown/fallback;
formula sharing and hashes do not establish integer feasibility or coverage.

**Scaling and memory.** Sharing can turn repeated path expansion into a smaller
program DAG, potentially reducing W, memory and coordinator work together. But
the Boolean graph, distinct shifted predicates or exact route images can grow
exponentially; storing correlations can be worse than inspecting their over-cover.
The eight diagonal guard examples establish representational difficulty, not a
dominant hotspot: completed A12 has only 66/68 finite-refinement cells.

**Cheapest falsifier.** First locate one demonstrably expensive join/route whose
over-cover introduces extra obligations. On one small *fully enumerated* finite
cell, compare the existing native concrete one-hop image, ordinary symbolic
over-cover and a restricted guarded relation using existing native algebra.
Include branch/source-validity failures and all conditional terms. This is an
exact small-domain diagnostic, not sampled certification of the campaign. Charge
predicate storage and entailment, then replay a multi-parent stream before
proposing a new queue domain. If no costly artificial workload is found, do not
build this engine merely because the representation is elegant.

**Reject** if the necessary native implication/transport cannot be expressed
without a new solver, if priority or source predicates are lost, or if exact
correlation work exceeds avoided native/coordination work. High implementation
cost and the weakest current evidence among these challengers.

## Actual IBP generation: a separate redesign, not this run's bottleneck

The earlier review already proposed downstream-aware rule choice and bounded
discovery portfolios; they are not new ideas to rename here. The whole-system
implication is to stop optimizing discovery and coverage independently. The
objective for a candidate program is discovery + exceptional-case construction
+ finite-coverage validation + expected repeated-use cost, not time to its first
descending rule or smallest master set.

Independent modular/source-order scouts can expose real parallel work in a hard
case; the winning exact relation still brings all its exceptional children and
new denominators. A few expensive cases limit span; duplicating scouts increases
total work and live coefficient memory. Changing a rule invalidates affected
coverage/priority receipts, so this belongs between immutable program epochs.
RustRed already has modular discovery, shared sources and sector parallelism.

More ambitious equation-set/block redesign has primary precedents:
[Kira 3](https://arxiv.org/html/2505.20197v1) targets seeding and equation selection;
[Blade](https://arxiv.org/html/2405.14621v2) constructs block-triangular reductions.
Their results do not establish RustRed integer-guard coverage or justify dropping
uncut coupling equations, exceptional faces or descendants. Use existing native
exact backends, not another CAS. The discriminatory pilot is one genuinely hard
generation case, fixed aggregate budget, all losing work charged, then exact
replay and the same downstream finite workload. No new search should be launched
to address a saved-rule traversal with no demonstrated missing target.

## Output and assurance: remove accidental requirements, not required guarantees

The desired output is still a cold-loadable admitted reduction program, routes,
entry policy and finite terminal union with complete operational coverage and
runtime compatibility—not a chronology of every request or a giant numerical
reduction table. Compact certificate/checking manifests were already proposed;
that packaging is an enabler for these designs, not a fourth novel algorithm.

An offline structural checker can separate scheduling history from final
responsibility verification only if it retains every native result's scope and
outgoing obligation. It must not trust “solved” worker labels or replace native
mathematics with hashes. Replaying all coefficient algebra as a second full walk
can erase the savings; no such universal recheck is newly required. Operational
closure under admitted saved formulas does not upgrade original-source IBP
provenance. The new support counters are attempted final-successor observations,
not raw-term proofs; zero on an incomplete prefix establishes nothing global.

On-demand repair alone or a smaller physics/diagram workload would change the
delivery contract and requires user approval. Exact partitions of the unchanged
E, alternative supersets with every escape retained, and different proof/task
representations do not inherently change it.

## Initial design-stage decision and adversarial outcome

No new architecture is justified for immediate production replacement. Advance
the measured anchor strategy through its bounded all-owner admission/control
gate. If one further competing pilot is desired, prefer **two exact entry shards**:
it cheaply tests whether reduced coordination can outweigh deliberately lost
sharing. Piggyback work/span and actual inner-repetition measurements on approved
controls; do not create three new campaigns. Micro-epochs come next only with
evidence of profitable batch reconciliation; relational reachability comes last
until a costly correlation-loss witness exists.

The independent critic challenged these proposals before the draft: shared
lower-owner work defeats naive sharding; one heavy region defeats epoch barriers;
exact predicate DAGs can explode; training and cold loading must be charged;
and neither busy-core counts nor pending inclusion imply completed proof.
Those objections are incorporated above. Admission/index implementation proceeded
separately through its own release gate. This design review authorized no run,
scope change or production rewrite.

The [independent adversarial review](radical_parallel_critique_2026-09-24.md)
cross-read this draft and retains its own counterexamples and conditional ranking.
Both reviews favor the measured sharing control before any replacement engine.

## Post-pair update: two different publication/span regimes

The [all-owner regional-sharing controls](all_owner_regional_sharing_2026-09-24.md)
both stopped at the predeclared scheduled-work gate, without mathematical
frontiers. Neither completed the required E. Their different stopped workloads
and remaining obligations support no speedup, slowdown or completion ETA.
Cold preparation alone took about 102 seconds per fresh process; traversal-labelled
samples averaged about 4.16/4.29 busy cores, despite up to 25 active inspector slots.

Two concrete observations sharpen the alternatives. Anchored commit ID1 remains
current from 118.20 to 129.26 seconds while events advance 398,310→752,674.
Its approximately 12.1-second sampled bin averages 23.9 blocked inspectors;
commit/helper-preparation wall grows only 0.694/0.183 seconds between those
heartbeats. This is consistent with a productive native stream and later
producers waiting, not demonstrated coordinator CPU saturation. Exact inspector
thread-to-domain attribution and projection/restriction CPU are not recorded.
Later, ID338 has **255 finished escrow entries, zero blocked producers** and
about 1.57 busy cores: a separate ordered lookahead/head-dependency regime.

Do not launch the earlier E-only shard experiment unchanged: duplicating all
fixed anchors duplicates the identical expensive early head in both shards,
rather than dividing its span. Demand-selecting or partitioning anchors would
be a different, separately specified unchanged-E experiment. Micro-epochs can
likewise turn that head into an epoch barrier; they are not justified merely by
these blocked-worker counts.

A smaller counterfactual is a **bounded running-output allowance** under the
same ordered publication semantics. Present streams have one published and one
private chunk, capped independently by bytes, records and logical events; the
completed-result escrow cannot accept an unfinished stream. More outbox capacity
could let some visitors finish and recycle, or merely postpone the same blockage
while retaining extra speculative work. The actual binding flush limit and
remaining output are unmeasured. Crucially, this cannot fix the later 255-finished,
zero-blocked regime. A separate profile of the expensive initial
Apply phase can discriminate native work before a buffer experiment; none of
these findings justifies a larger campaign, new CAS, rule regeneration or rewrite.

### Local source/evidence anchors

- Contract and output: `docs/finite_starting_domains.md`, Objective, Domain
  contract, Implementation sequence and September 24 checkpoints.
- Native semantics: `crates/rustred-core/src/solver/candidate_reduction/owners/domains/`
  `{applied,matching,guarded}/` and `candidate_reduction/routed/`.
- Representation/ownership: `crates/rustred-app/src/application/routed_campaign/`
  `walking/{queue.rs,inspection.rs,execution.rs,delegation/}`.
- Discovery: `crates/rustred-core/src/solver/{search.rs,sector.rs,execution.rs}`.
- Exact shard predicates and CPU reservation: `candidate_reduction/power_domain/mod.rs`,
  `routed_campaign/matching/input.rs` and `walking/worker_budget.rs` in the
  corresponding core/application source trees above.
- Audited A12 and all-owner plan: `TMP/a12-anchor-amortization.qqQTJk/`
  `{RESULTS.md,INDEPENDENT_AUDIT.md,ALL67_INDEPENDENT_RECOMMENDATION.md}`.
  TMP evidence is local, not a portable artifact dependency.
