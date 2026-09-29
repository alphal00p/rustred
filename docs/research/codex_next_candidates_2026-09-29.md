# Registered follow-on optimization candidates

Date: 2026-09-29. These are proposals, not measured speedups or deployment
recommendations. G2′/rescue integration and its correctness gates take priority.
The governing scope and operational limits are in `CODEX_PROGRESS_PLAN.md`.

## Current priority after the G2 correctness controls

### Same-view epoch misses: narrow follow-up registered

The first private S4 implementation moves published-store lookup to inspectors,
but still repeats the full search in P2 for every reported miss. Root and the
independent reviewer accepted investigating a narrowly guarded bypass:

- Require the same immutable lockstep store version and watermark. No stale
  snapshot or rolling-scheduler inference is permitted.
- Reconstruct the canonical query and native summary independently. Retain
  collision-confirmed exact-image lookup and exact positive verification;
  skip only the already-completed published-store forward search.
- Preserve cut-local duplicate resolution, antichains, reverse retirements,
  Planned tokens and all P3 obligations. A negative result is **not coverage**:
  it enters ordinary admission and cannot discharge a dependency.

Normal deterministic equality depends on the worker actually searching that
same store. A corrupted false negative can create redundant work or change
capacity outcomes; mathematical soundness alone is not record equality. Tests
must include retired exact aliases, contradictory misses, covered-subset false
negatives, missing/stale reports, mixed-hit/miss full-record and graph equality,
and interrupted replay. Charge worker searches, remaining exact checks and
payload handling separately. The expected saving is one coordinator forward
search per true miss, not the whole admission cost. No reduced forward work or
no repeatable net campaign improvement falsifies the performance case.

The five-file implementation was independently source-reviewed and privately
committed as `f128f550`, then combined with the first S5 hash-feed slice at
`fa783fc5`. Four new miss tests and the strengthened native assertions have
not yet executed. No reduced work or speedup has been measured. The running
validation build remains the older frozen `29e30a79`, with source and cache
unchanged; its results cannot validate this later slice.

### S5 edge-feed batching: first bounded source slice

The first private S5 slice (`9dc770a4`, integrated at `fa783fc5`) batches the
same little-endian edge words into existing BLAKE3 with a 1 KiB stack buffer;
record digest inputs use the same nine bytes as before. No new hash, CAS,
schema or record authority is introduced. Independent reviews preserve
append/restore order, exact bytes, mutation/refusal boundaries and failure
behavior. Five scalar-oracle/prefix/restore tests are written, not executed.

Expected benefit is fewer tiny hash-update calls, not fewer mathematical
obligations. Buffer packing/zeroing can instead lose for short edge runs.
Measure zero, 1, 2, 8, 16, 64, 256 and 1024 targets against the old scalar
feed, including prefix restoration; then require representative campaign
evidence. Changed digest bytes or a net slowdown falsifies the slice.
Typed records, bulk dependency updates and parallel merge preparation remain
separate unfinished S5 work; this micro-optimization does not complete them.

### Process-local NUMA: no gross cross-socket-placement opportunity observed

A one-shot read-only check at 10:18–10:20 UTC found LC2's allowed CPUs128–227
on socket1/nodes4–7. Of 42,508,392 KiB resident in the observed NUMA mappings,
only 1,408 KiB was on socket0. Approximately 95.68% was on nodes4 and6. All463
mappings used default policy. The separate status/map reads are not an atomic
RSS measurement. Source: `TMP/codex-s4-source.yAIv3U/NUMA.md`.

Root independently checked CPU/memory masks and repeated the small mapping
aggregation: 42,643,572 KiB total, again 1,408 KiB across sockets. These reads
did not attach a profiler, scan campaign data or change any process settings.
The comparison allocation64–79 lies on one node on socket0 and has SMT siblings;
it cannot directly predict100-worker, multi-node production behavior.

This does not establish memory-access locality, bandwidth or latency costs.
Do not add memory binding merely from the residency picture. Reopen a placement
experiment only with evidence of costly remote accesses and a matched,
process-local test; memory layout and checkpoint costs remain separate open
questions. No speedup or production configuration change follows here.

### Post-G2 disposition of algebraic shortcuts (09:10 UTC)

The later qualified profile is recorded in
[the post-G2 report](codex_post_g2_profile_2026-09-29.md). A separately authored
N1/N4 source/API reassessment was independently reviewed by root and
`joint_support_pruning`; full local evidence is
`TMP/codex-n1-n4-post-g2-reassessment-2026-09-29.md` (SHA256
`10e2b79c11ab11307f591a440c155712bab86a3a3ef3627d3498f0979af13f95`).

Both implementations remain **deferred**, with these explicit reopening tests:

- **N1:** count genuinely expensive matching predicates whose *occurring index
  support* is entirely fixed by the existing checked coordinate bounds. A
  nonzero native finite-field evaluation then proves the restricted polynomial
  is nonzero over formal base parameters; it does not prove nonvanishing at
  every numerical parameter value. Sampling an unfixed index cannot establish
  this uniform domain property. Zero/unlucky evaluations use the exact path.
  Symbolica's pinned and current public `evaluate_with_coeff_map`, exponent
  iterators and finite-field conversion APIs already supply the algebra.
  A capped observational test must return the old exact result and charge
  support scans, evaluation, fallback, and all input/resource admission checks.
  Restricted-result limits cannot be silently replaced by weaker preflight
  checks. No eligible costly calls or no net saving falsifies the optimization.
- **N4:** the finite diagnostic recorded **zero pre-admitted full-orthant hits**;
  its initial request has finite upper/A/D bounds. Its 3,493,943 job-local reuse
  hits, alongside 12,502,912 reported successors, mix Route and Apply and do not attribute
  singleton coefficient cost. Earlier scheduling reuse is not closed coverage.
  Before implementation, a bounded observational join must establish actual
  expensive non-affine singleton Apply reuse, charging added lookup costs.
  Preserve original poles (including cancelled terms), zero/conditional
  semantics, child validity/descent, exact image/guard geometry, dependencies,
  quarantine and cancellation. Never fabricate coefficients for the public
  visitor or cache work before a successful Continue. No useful attributable
  saving falsifies this proposal.

Neither opportunity probe has been implemented or measured. Throttled
instruction-location percentages cannot supply their missing cost attribution,
and their hypothetical savings must not be double-counted. These dispositions
do not delay the independently source-reviewed narrow N2 buffer reuse and
epoch validation, nor authorize a production restart.

Read-only bounded extraction from the completed Ordered finite-five-loop
control changes the next profiling question, not any implementation decision.
Evidence: `TMP/codex-integration/post_g2_profile_priority_2026-09-29.md` and
`TMP/codex-g2-pilot-prep.n7Kd5q/ordered-controls/{off,union}/five-finite/`.
These are not the repeated Ready comparisons or the full production scope.

| Aggregate | Off | Union |
|---|---:|---:|
| Traversal seconds |211.6332|128.7751|
| Apply term/boundary visits |47,998,700|24,504,843|
| Apply selected pieces |1,723,391|892,630|
| Route coordinate cells |99,878,880|83,172,840|

First profile the coordinator/helper critical path on the Ready controls.
Union's coordinator interval128.2664s includes wait36.4869s, commit26.9659s,
progress/observer24.3080s, preparation12.4801s and publication10.5703s. These
listed buckets are not exhaustive. `execution.rs::observe` charges the
observer callback, including possible serialization/I/O, to
`progress_json_seconds`; its roughly19% share is **not** removable lean-JSON
allocation cost. Per-commit lean construction is charged elsewhere. Likewise,
helper summed worker time is not coordinator wall time. Do not infer speedup
from these bucket fractions alone.

N4 remains the leading algebraic *profiling question* only if exact
restriction/GCD on eligible already-covered singleton targets costs enough
after G2. N2 allocation and N1 fully fixed predicates retain their evidence
gates below. G2 already removes about half of Apply visits in this control;
the Route path does not automatically benefit from an Apply-only optimization.
No existing negative experiment is reopened by these aggregate counts.

### Narrower dependencies from borrowed transcripts: deferred

An exact inspection of a broad anchor might ideally let a narrower borrower
depend only on the relevant successor slices. Current successful records do
not retain enough information for that operation: the applied visitor has
source bounds and shifts, but `walking/inspection.rs` drops them from successful
successor events and reduces selected-cell events to counts. The persisted
event/edge stream retains target domains, not a complete source-partition/term
map. Frontier-only provenance is insufficient.

Therefore do not delete broad-anchor dependencies or infer useful narrowing
from current receipts. Reopen only with a positive representative case and
available exact source-to-target evidence (or a small explicitly measured
reinspection), preserving guards, poles and every reachable obligation. A
general authenticated trace/preimage framework is not authorized by this
observation, and it does not reopen the prior negative closed-witness census.

## 1. Lean telemetry: next narrow coordinator slice

Owner: `checkpoint_final_audit` (isolated implementation); independent source
review by `joint_support_pruning` passed, native execution still pending.

Mechanism: capture typed scalar pool statistics under the mutex, serialize
after releasing it, and update/reuse the lean JSON objects rather than
rebuilding their maps and constant strings after every commit. Preserve fresh
attempt counters before every existing checkpoint callback. Simply throttling
`set_parallel_lean` is unsafe: checkpoints persist that state, and a Ready test
uses `completed_slots_reclaimed` in a callback to release a waiting inspector.

Evidence: historical profiles attribute 2.5–5% of coordinator samples to this
path. This predates combined LC2/G2 and is not a predicted whole-walk gain.
Current source is `walking/parallel.rs::snapshot_tier` and
`walking/execution.rs::set_parallel_lean` in `rustred-app`.

Smallest test: compare every snapshot across synthetic commits, resume totals,
overflow, failures and reclaimed slots. Follow with Ready multi-prefix replay,
checkpoint/restore and Ordered exact-result controls. Preserve lean/detailed
key sets and do not retain stale detailed fields or decode failure IDs twice.

Falsifier: any changed non-timing counter or checkpoint semantics; absent
allocation/lock-cost reduction; or no useful matched campaign improvement.

## 2. Helper preparation: preserve the negative result

Preparation's historical 32% coordinator-wall share includes useful containment
work, stragglers and waiting; it is not all removable scheduling cost. Changing
the fixed 256-record batch boundary changes reuse/containment behavior.

The previous minimum-eight grain experiment remains rejected: median paired
ratio 0.99885, preparation 15.48% slower. Source:
`five_loop_admission_grain_2026-09-25.md` in this directory.

Only a narrow allocation cleanup is presently identified: order-preserving,
fallible in-place filtering of the replay suffix, eliminating its extra vector.
Test mixed replay/new/non-admission batches with a mid-batch error, preserving
all replay calls, first-error behavior, cancellation and queue snapshots.
Expected benefit is small and unmeasured. Do not construct another speculative
pool or snapshot architecture without new evidence; that overlaps epoch work.

## 3. N2: redundant geometry copies

Owner: independent `checkpoint_final_audit` read-only registration.

Target the scratch and copied vectors in `owners/domains/applied/geometry.rs`,
using the existing checked `LatticeBox::try_from_preallocated` ownership path
where appropriate. Retain callbacks, iteration order, correlated A/R/D bounds,
unbounded endpoints, checked overflow and logical resource charges. Scratch
must not be mutated while borrowed by a callback. Keep algebra in Symbolica.

Do not rewrite `power_domain::project`: its current implementation already uses
fixed arrays. The historical 10.39% projection CPU share is not an estimate of
removable allocation overhead. Earlier duplicate-projection removal changed
median traversal by only -1.46% and -0.059%, with overlapping ranges; see
`finite_closure_native_profile_2026-09-23.md`.

Smallest test: existing applied-geometry differential tests, including fixed
and absent crossings, correlation, bounds above u32, cancellation, budgets and
overflow prefixes. Then identical event/counter and successor/frontier/problem
records on C-4L/C-5F with G2 enabled in both arms. Park if no useful whole-walk
benefit; G2 may remove much of the native work this would optimize.

## 4. Research: exact pending D-band coalescing

Owner: `work_reduction_research`; separate critique by `checkpoint_final_audit`.
Status: **parked after negative opportunity census**, no engine implementation.

Within a bounded epoch-local miss batch, group domains with exactly equal
program/rescue view, phase, owner, coordinate bounds, numerator-rank bound and
positive-power bound. Only the integer D interval may differ. Overlapping or
adjacent D intervals can then be merged without adding points. For a common
remaining predicate B, `B ∩ [7,9]` union `B ∩ [10,12]` is exactly `B ∩ [7,12]`.

This is inspired by exact symbolic-state merging, not by a claim that timed
automata theorems establish IBP closure. The primary paper explores convex
zone merging and the importance of merge policy/overhead:
[André et al., Efficient Convex Zone Merging in Parametric Timed Automata](https://arxiv.org/abs/2212.04802).

Unlike previously rejected hull widening/dense cells, this adds no input points.
Unlike general admission-time union lookup, it proposes signature grouping and
interval sorting within a bounded batch, not a scan over historical domains.
Unlike G2′, it combines still-uninspected obligations rather than borrowing an
earlier local inspection. No reserved job, initial query, accepted stream,
quarantined node or published history may be rewritten. A union remains a
pending obligation; all original requests need exact dependency accounting.
All already-admitted domain images/IDs, including Pending ones, remain
immutable. Start with unassigned P2 misses; a later extension could append a
new union and transfer genuinely Pending/unprotected originals only through
normal verified-containment aliases. Exclude dead/quarantined work entirely.
The synthesized union needs its own digest/native summary, deterministic first
request position and checked tokens for every original request. It cannot
become a G2 anchor before ordinary native publication. The current invariant
that a survivor uses one original image would need an explicit, audited
geometry-construction extension, not a bypass.

First falsifier: inspect immutable finite-five-loop and r1a12 receipts for
matching signatures in windows of 16/64/256 records, but first establish what
the records actually retain. Publication windows without batch membership are
only descriptive geometry proxies, not actual epoch miss batches or rigorous
opportunity bounds. Record that limitation and use post-G2 data when available.
A weak opportunity count can justify deferral, not disprove opportunity in the
unrecorded P2 stream. If receipts lack the necessary information, defer rather
than infer a reduction from a whole-run histogram.

Only after that: test exact union-input equivalence and compare native output
partitions/downstream work. An exact input union can still lead to more costly
guard partitioning. Count additional union IDs and edges. Preserve interval
overflow/infinity semantics, full-signature checks, replay and quarantine.
Test a feasible common base with D=0 and D=2 requests: never fill the D=1
hole. Separately test exact adjacent union and rejection of Reserved transfer.
Complexity estimate: O(m N) full-signature work plus O(m log m) interval sorting
for a batch of m, dimension N; O(m) auxiliary references (or O(m N) if full
signatures are copied). Durable provenance integration is a separate task.

### Census result (2026-09-29)

The legacy G2-on receipts lack actual miss/admission batches, so no misleading
publication-window histogram was substituted. Instead, an independent source
audit supported reconstruction of historical **G2-off epoch C-5F post-antichain
survivor birth cohorts** from paired record/edge streams. All 1,215,537
records/domains and 12,314,557 edges passed attribution and ordering checks.
Across 48,412 birth merges, the 1,215,536 noninitial survivors had **zero
identical base-signature pairs and zero possible coalescences** in each of the
tested within-cohort windows (16, 64, 256).

Analysis cost: 26.304 s, peak RSS 201,396,224 bytes. Evidence (including source
hashes and exact argv):
`TMP/codex-integration/coalescing-census.Gs3IcM/result-r2.json`.
The first attempt failed closed because the parser omitted the existing
partial-initial-inspection record kind; its receipt is retained as `result.json`.
Source inspection justified that one correction; birth/edge checks were not
weakened. Startup tests covered holes, adjacency, overlap, infinity and unequal
base signatures.

This is negative evidence for this narrow candidate on this workload, not a
theorem about all pending domains and not post-G2 evidence. Reopen only if an
actual post-G2 miss-cohort trace shows useful exact coalescing opportunity.

## 5. Deferred: backward closed-source covers

For an exact rule with targets T_j(n), recursively closed target covers C_j
would give a sufficient source region `G ∩ ⋂_j T_j⁻¹(C_j)`, with G retaining
applicability, original poles and descent. Every RHS obligation is required.
This differs from N4's avoidance of coefficient work on existing forward
images; it would derive a reusable source region before later fragments arrive.

Do not treat merely locally discharged G2 anchors as recursively closed here.
The current guarded pullback engine rejects correlated A/D bounds, directly
relevant to production. Exact preimage geometry, provenance and replay therefore
make this nontrivial. Reopen only if a small sign-stable, shift-only case from a
completed control demonstrably covers useful later work beyond G2. No general
SMT/Presburger service is proposed. No implementation is authorized by this
registration alone.

None of these candidates proves termination from bounded input rank, zero
frontiers, a smaller queue, or higher CPU utilization.

## 6. Read-only study: closed-descendant witnesses for required queries

Owner: `checkpoint_final_audit`; independent critique by
`joint_support_pruning`. **Parked after negative opportunity census**; no
engine implementation.

Mechanism: record an already recursively closed node as the witness for a
required query when that node has the exact same owner/phase and its domain
contains the entire query, including coordinate and correlated A/R/D bounds.
This uses existing exact containment and closure authority, not a new rule,
graph rewrite, union search or inverse-routing service. No locally inspected
but recursively open G2 anchor qualifies. Preserve every required query.

Expected benefit, if the opportunity exists: some narrower physical queries
could be discharged before their broad helper root closes, or without an
unnecessary rescue amendment. Existing root-only query reporting cannot use
those witnesses. Reporting alone does not save traversal: scoped termination
would need its own explicit status and independent verifier support. It must
never report all scheduled domains resolved when unrelated helpers remain
open. Initial orthant reuse often redirects descendants back to the helper,
which may eliminate most opportunity; rescue quarantine may change this.

Smallest test: reconstruct recursive closure at publication prefixes of a
small immutable completed four-loop control, comparing the first input-root
and first any-closed-node containment witnesses for identical physical
queries. Match result/checkpoint generations; do not scan production's live
checkpoint or infer node-level opportunities from aggregate counters.

Falsifiers: no earlier witness; any owner/phase/A/R/D containment mismatch;
using only local sealing; or scan cost outweighing avoided work. A future
implementation must avoid rescanning every domain for each of 116 queries
on every heartbeat—use owner-key filtering or newly closed nodes and retain
only a small witness set. This registration does not reopen parked general
union admission or backward affine-cover services.

### Census result (2026-09-29)

The immutable historical H repair control contains 9,033 nodes and 32,875
edges. Among all 628 original queries and separately the 314 finite full-jet
queries, there were **zero earlier non-input witnesses**. The final required
publication remained 9,033 for root-only and any-closed-node policies. Runtime
was 0.372 s, peak RSS 62,968 KiB. Independent rerun (0.384 s) reproduced every
non-timing/RSS field and input hash. Evidence:
`TMP/codex-closed-witness-census.5P0Cyu/{result-r2.json,census.py}`.

The matching generation-3 CP3 checkpoint had a stale conservative closure
cache (165 nodes versus the final 9,033). The first analysis failed closed;
the corrected analysis recomputed closure from all seals and edges, checked
graph/result consistency and cross-checked eight prefixes and cycle/late-leaf
fixtures. Eligibility begins at complete local record publication. Closure
time is the latest publication reachable through final outgoing edges. Source
audit established that this historical tracker refuses post-seal outgoing
edges, making the prefix calculation exact for this no-rescue Ordered trace.

This is not post-G2/rescue evidence or a universal negative theorem. Reopen
only if such a later immutable control shows useful earlier witnesses. No
additional scanner or scoped early-stop mechanism is justified by this test.

### Scope-alignment source audit

Independent audit at combined integration commit `d12db6cf` confirms a
conservative granularity mismatch: initial containment may represent finite
query Q by broader helper H's ID, and the closure graph then requires all of
H's outgoing obligations. Exact G2 lending restricts local inspection, but
its dependency edge still names the whole anchor. Declaring the 116/67 role
partition does not by itself create a separate demand-restricted Q node.
This delays some possible scoped proofs; it does not authorize false closure.

The current first-owner example pairs rank-only
`owner-anchor-r6-anone-000011001001011` with required
`conv-d10-a16-r6-000011001001011` (A<=16, R<=6, D=10).
Source: `walking/{queue,descendant_closure,execution,rescue,verify_closure}.rs`
under `crates/rustred-app/src/application/routed_campaign/`. The full audit
and precise line references are retained in
`TMP/codex-integration/scope-alignment-audit-2026-09-29.md`.

This finding does **not** reopen the earlier failed helper/piece changes:
all-A-bounded helpers took 3.93–15.73x the traversal time on four-loop controls
(`four_loop_helper_bounds_2026-09-25.md`), I1b increased matched five-loop
pending work by 1.40–1.44x in censored runs
(`fable51_w0_inputs_2026-09-27.md`), and finer piece certification cost
1.57x the native seconds while missing two roots
(`fable51_w0_wv_2026-09-27.md`). Nor does it overturn the negative H census.

Reopening requires a representative unchanged finite Q to close independently
of its still-open H, retaining all generated obligations and exact replay.
Charge that additional solve and bookkeeping. A Q-only run that merely maps
back to H or explores the same open cone fails the falsifier. No such new
experiment has been run; no production input or campaign should be changed
on the strength of this source observation alone.

## 7. Conditional reopening: I1, distinct from rejected I1b

I1's historical status is **provisional, not rejected**. It makes the selected
L* helper owners A-unbounded, to improve absorption; it is not the failed I1b
larger-finite-A experiment. `fable51_w0_inputs_2026-09-27.md` section 7 records
its stated criteria met and independent verifier passes on the old baseline.
The formal fresh L*-only gate and rank/mask escape guard remained open.
`HANDOFF_FOR_ASTRA.md` and `TMP/progress/orchestrator_decisions.md` explicitly
defer shipping until in-run rescue exists **and** a probe shows net runtime
gain. The new rescue integration has passed its corrected native regressions;
representative combined campaign gates remain pending and the second condition
is unmeasured.

Historical single 54-minute probes are not current deployment evidence:
`fable51_w0_results_2026-09-27.md` reports noisy wall/native ratios, lower
pending pressure, and higher edge counts. Do not multiply their effects by
G2's gains or equate matched-native snapshots with whole-campaign improvement.

Smallest follow-on after combined rescue validation: a fresh temporary input
variant retaining all 116 required geometries exactly, explicitly declaring
helper changes, with the necessary rank/mask escape guard and a tested bounded
rescue. Start with BMW/four-loop controls and an appropriate finite five-loop
case; assess the G2-enabled combination separately under the same <=30-minute
pilot ceiling. Measure additional edges, helper/rescue overhead, total work,
wall and memory. Unknown escapes stop explicitly; never truncate descendants.
Absent net benefit, or with an unhandled escape/ambiguous rescue, keep I1 off.
No input variant, guard, pilot or production change is implemented by this note.

## 8. N1: restrict modular witnesses to a useful and sound seam

Independent read-only API/source audit is retained at
`TMP/codex-integration/n1-modular-witness-feasibility-2026-09-29.md`.
Pinned Symbolica already supplies finite fields, coefficient conversion and
`MultivariatePolynomial::evaluate_with_coeff_map`. Existing RustRed modular
sampling evaluates numerator and denominator separately, refusing bad poles.
No new finite-field or reconstruction kernel is needed.

The broad RHS shortcut is not presently justified. In the core's
`owners/domains/applied/engine.rs` the call through `applied/restriction.rs`
to `algebra/indexed/specialization.rs` performs exact substitution and
rational normalization before `applied/algebra.rs` classifies the result.
A witness in that classifier is too late to save this work. Furthermore,
`Zero::No` authorizes uniform nonzero on the integer domain. A nonzero sample
of n-3 on a domain containing n=3 cannot authorize that result; downgrading
all such results to Conditional could instead increase branching. Exact
coefficient payloads and original pole conditions cannot disappear either.

A narrower candidate exists before predicate specialization in
`owners/domains/matching/guards.rs`: when **every occurring index variable
is fixed by the cell**, a nonzero sample proves the restricted polynomial
nonzero in the formal coefficient field. For example, n*d-n+1 at n=1
becomes d; a nonzero residue at d=7 proves nonzero in Q(d), not nonzero at
every numerical dimension. This can preserve the existing Nonzero outcome,
without inventing Conditional branches. Zero/bad samples still take the exact
path; all original denominator, input-admission and guard obligations remain.

Before implementation, a post-G2 profile must establish enough expensive
predicate calls with wholly fixed index support. Charge support scans,
sampling and fallbacks; preserve error ordering, cancellations, source guards
and emitted payloads. Small falsifiers include n-3 versus n uniformity,
fully fixed base-dependent predicates, unlucky primes, sampled zero
denominators, poles on a zero RHS and original-term cancellation. No speed
benefit has been measured.

The simpler identically-zero predicate seam in
`owners/domains/guarded/engine.rs` is **parked for this campaign**: its app
caller is the separate guarded-domain API, not the ordinary walker, and its
power-bounded wrapper rejects the campaign's nontrivial A/D bounds. Optimizing
that path would not demonstrate a benefit for the current five-loop run.

## 9. N4: an opt-in coverage-only visitor, if profiles justify it

Read-only source evidence at combined `d12db6cf` is in
`TMP/codex-integration/n4-coverage-first-feasibility-2026-09-29.md`.
The live Apply path materializes, normalizes and classifies RHS coefficients
before the app tests target reuse, although the walker does not retain the
exact coefficients. G2 already avoids whole inspections, and empty source
geometry already avoids this coefficient work; measure what remains after G2.
Route uses a different geometry visitor, so N4 cannot automatically address
a Route-heavy workload.

A separate internal coverage-only visitor could ask exact target containment
earlier and avoid materializing unneeded payloads. Existing Symbolica polynomial
substitution/GCD/addition suffice. Do not weaken the public visitor's exact
coefficient contract. For the opt-in visitor, a resource failure caused only
by an operation that is safely avoided need not be reproduced, nor must old
operation counts match. Its new cost/event contract must be explicit; flag-off
identity remains required.

Mathematical obligations are different: original poles, source/child validity,
descent, genuine frontiers and Uniform/Conditional support cannot disappear.
Individually zero terms and a cancelling pair of nonzero terms have different
existing source-validity obligations. A naive return-on-covered-target skips
these checks and is not justified. Start with a non-affine singleton case and
exact fallback, only if a post-G2 profile finds substantial eligible cost.

The smallest falsifier uses an admitted target and nonconstant coefficient
under fixed indices, followed by uncovered-target, pole, zero-term,
cancelling-pair, child-condition, guard-zero and optional-refusal variants.
Compare exact images, A/R/D constraints, obligations and replay—not just queue
size. Count covered singleton groups and all fallback/lookup costs. This is
a feasible architectural direction, **not an implemented or measured gain**;
it does not reopen deferred backward covers or previously rejected helper-bound
variants.
