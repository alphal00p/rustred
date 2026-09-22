# Bounded-routing five-loop pilot

## Outcome

**Support-aware routing update:** exact numerator-support caps now remove
81.90% of routed descendants on 5,910 identical source queries shared with the
semantic-only pilot. However, the recursive run still suffers serial admission
pressure and shows worse queue/throughput trends: tighter conservative boxes do
not automatically make global subsumption cheaper. It stops incomplete with
63,913 complete and 113,500 pending domains. See
[the matched support pilot](#support-aware-routing-pilot-local-pruning-global-admission-regression).

**Correlated-domain update:** A/R/D predicates now survive matching, shifts,
routing and queue/cache reuse. The 980-point constrained diagonal agrees with
the saved singleton oracle locally, but recursive traversal still develops
severe serial admission pressure. It was stopped cooperatively with 49,686
complete and 145,431 queued domains, after 106.956 s traversal. This is not
five-loop closure or a speedup comparison with the larger rectangular input.
See [the constrained pilot](#correlated-domain-integration-and-pilot).

**Resumed audit update:** the matched maximal-index rerun is now measured: its
identical 74,326-domain completed prefix takes 108.625–109.632 s traversal,
versus 315.671–316.676 s previously. This is a 2.88–2.92× prefix improvement,
not complete five-loop closure. A second all-owner control exposes serial
admission pressure again. Both runs stop incomplete at explicit work allowances;
see [resumed measurements and recommendations](#resumed-measurements-and-recommendations).

The first release pilot preserves finite positive-power bounds through real
five-loop IBP and momentum-routing paths, but does **not** finish the requested
domain. It was stopped cooperatively to optimize a measured serial
containment-index bottleneck. No missing-rule frontier had been reported at
the stop. That observation is not coverage of the unfinished queue.

The workload is one 2,560-point diagonal box in the saved 67-owner family.
Only 980 of those points satisfy the conditional marginal physical envelope;
the box deliberately overcovers it. It contains the earlier 50 concrete stress
roots, but is a different workload and cannot establish a speedup over their
completed concrete traversal. Rules were loaded unchanged, not regenerated.

## Reproducible boundary

- Bounded-routing implementation: `ac74dc74`, branch `feynkit`.
- Frozen optimized binary SHA256:
  `279b20a76b4c166d51ad740ebdd4ef731357da483d4cd070076d918efb705c0c`.
- The tested working tree also includes separate, uncommitted scheduler/escrow
  changes. These measurements are of that combined binary, not the isolated
  bounded-routing commit.
- 67 unchanged owner programs, 8,246 admitted route records; 50 workers pinned
  to CPUs 0–49, nested compute pools set to one.
- 500 GB sampled aggregate RSS ceiling, 450 GB cooperative soft stop and a
  480 GB child address-space limit. No elapsed deadline.
- Explicit diagnostic allowances: 500,000 domains, 50 million successor events,
  100,000 route-mask candidates per query; unlimited containment comparisons.
- Finite-axis refinement enabled, 65,536 faces per query and native guard
  univariate-degree allowance 64.

Local receipts, input and supervisor command are under
`TMP/bounded-routing-pilot.4kY19R/shared-owner-campaign.ksc985pr/`.
The release binary and gate logs are in `TMP/bounded-route-gate.clR7HK/`.
Compilation is outside the measurements below. The full release gate passes
2,720 core tests, 422 application/integration tests and 72 Python/steering tests;
32 existing core diagnostics remain ignored. Independent implementation and
mathematical audits passed.

## Measured incomplete run

| Quantity | Result |
|---|---:|
| Preparation, including route verification | 106.254 s |
| Reported traversal interval | 321.095 s |
| Application total | 427.349 s |
| Supervisor lifetime | 434.428 s |
| Sampled aggregate CPU time | 6,858.92 s |
| Sampled peak aggregate RSS | 15.301 GB |
| Scheduled domains | 159,344 |
| Completed domains | 74,326 |
| Queued domains remaining | 85,017 |
| Cancelled/failed domain | 1 |
| Observed unresolved frontiers | 0 |
| Committed events | 26,367,682 |
| Successors, including conditional ones | 21,988,073 |
| Conditional successors | 471,952 |
| General containment comparisons | 6,090,772,383 |
| Route inspections / mask candidates | 56,530 / 2,166,887 |
| Maximum admitted descendant rank | 12 |

Exit status is 4 with explicit cancellation; the supervisor did not hard-kill
the child. Neither memory nor the diagnostic event/domain allowance was
exhausted. The saved result is an incomplete diagnostic receipt, not a durable
resume of every pending obligation or a new closing artifact.

All 74,327 committed domain records retain finite upper bounds on positive
axes, including the last cancelled record. Descendant rank 12 is legitimate:
the rank-10 starting restriction is not reapplied to descendants. No terminal
minimization, master evaluation or coefficient back-substitution was performed.

## Why the run was stopped

The initial traversal used roughly 36–42 busy cores. Later, the ordered
publication buffer reached its approximately 8.590 GB accounted-byte ceiling.
During a representative interval around total time 280–283 s, returned native
work counters stayed fixed while the coordinator advanced through millions of
containment comparisons. A process-thread snapshot showed one running main
thread and 51 threads waiting in futex calls. Work still progressed: this was
a throughput bottleneck, not deadlock or a missing-IBP diagnosis.

One owner accounts for 15,057 of 17,797 committed Apply records. The current
per-owner index retains historical admitted boxes and scans them linearly.
Increasing the completed-result buffer would postpone the stall and consume
more memory, without fixing serial admission throughput.

The next narrow optimization is an independently audited maximal-box index,
now implemented with passing release validation:
retire dominated boxes **only from containment lookup**, retaining every exact
key, original domain and pending work obligation. A retained containing domain
can suppress duplicate scheduling while pending, but cannot count that work
as solved. Reverse-dominance maintenance must be counted, deterministic and
transactional. The implementation keeps the old lookup for explicitly capped
comparison budgets. Progress and final results report maintenance comparisons,
retired candidates, current candidate count and the selected index policy.
The final checkpoint gate passes 346 application-library tests plus 82
integration tests, and all 72 Python API/CLI/steering tests. The native core is
unchanged from the earlier passing 2,720-test gate. Gate logs and a frozen
updated binary are in `TMP/maximal-index-gate.Z4gd01/`.
At that checkpoint no end-to-end speedup was claimed. The resumed measurements
below establish an equal-prefix improvement, not a complete-run speedup.

Separately, compact total-power and A-R constraints remain necessary candidates
for reducing conservative overcoverage. A faster containment index alone does
not establish coverage of the full finite renormalizable input domain.

## Independent offline index experiment

A separate optimized, std-only Rust probe replayed the 74,327 committed domain
descriptors. It performs no IBP solving or algebra. Every admission decision
agrees with the historical-list algorithm, and every historical descriptor
remains contained by the resulting index. Four standalone probe tests pass.

| Committed-prefix replay | Historical list | Maximal candidates |
|---|---:|---:|
| Retained lookup descriptors | 74,327 | 9,666 |
| Search comparisons | 179,375,023 | 29,283,205 |
| Reverse maintenance comparisons | 0 | 29,283,205 |
| Single-core index replay | 3.538 s | 0.715 s |

64,661 descriptors retire from lookup only; the peak live maximal index has
10,462 entries. The largest Apply bucket shrinks from 15,057 to 1,830 entries.
Parsing took 9.118 s and is excluded equally from both replay intervals.
This is a single shared-host experiment, not a statistical performance claim.
It excludes the original campaign's rejected scheduling requests and unfinished
queue, so it does **not** replay all 6.091 billion campaign comparisons or
demonstrate an end-to-end speedup. Evidence:
`TMP/domain-antichain-replay.XNGKBi/{replay.rs,results.tsv}`.

## Resumed measurements and recommendations

Three independent audit lanes reviewed runtime receipts, native/application
correctness, and primary literature plus local SpIRed/TIDE references. No saved
IBP programs were regenerated for either run. Both use the frozen release binary
`TMP/maximal-index-gate.Z4gd01/rustred`, SHA256
`4004d685ff1bf656a163a59674e99d9e905b80ac81648a78651c3b1951919ef3`.
It includes the same separate, uncommitted scheduler/escrow changes as the
baseline; do not attribute these results to an isolated clean commit.

The diagonal request matches the baseline after normalizing only the executable,
receipt paths and PIDs. Owner-manifest SHA256 is
`d2de73414a0cc5652d9124a32dd978759daf75abc305b705b53d759a3aad7f7f`;
diagonal input SHA256 is
`1a40b8e5f045d01ce774991f9acd5c0561a2531bcb3b4973555880f24587956d`.
The all-owner input SHA256 is
`f1e71f265e2816fe5528429cf1f11bdc615bc1da7cc4ea166986efff6d8caf60`.
Both runs configure 50 workers pinned to CPUs 0–49, nested pools of one and the
same 500/450 GB hard/soft RSS allowances. There is no elapsed deadline.

| Measurement | Diagonal rerun | All-67-owner binary-dot/R0 control |
|---|---:|---:|
| Finite starting points | 2,560 | 58,400 |
| Preparation | 105.182 s | 104.997 s |
| Traversal until incomplete stop | 241.852 s | 81.587 s |
| Application total | 347.034 s | 186.584 s |
| Supervisor lifetime | 354.456 s | 194.128 s |
| Sampled aggregate CPU | 10,359.34 s | 936.04 s |
| Sampled peak aggregate RSS | 11.054 GB | 8.404 GB |
| Fully completed domains | 137,684 | 90,428 |
| Scheduled domains | 264,765 | 500,000 |
| Pending domains | 127,080 | 409,571 |
| Committed events | 50,000,000 | 2,513,390 |
| Observed unresolved frontiers | 0 | 0 |
| Total containment comparisons | 2,925,702,289 | 2,229,579,098 |
| Of those, reverse maintenance | 695,282,830 | 804,132,331 |
| Live / retired lookup candidates | 32,163 / 232,602 | 358,405 / 141,595 |
| Maximum admitted descendant rank bound | 12 | 3 |
| Stopping reason | 50-million-event allowance | 500,000 scheduled-domain allowance |

Each result also contains one partial failed record caused by the named
coordinator allowance, not by a missing IBP. Both exit 4 without a hard kill.
The workloads differ, so comparing their final times is not a speedup estimate.
Neither covers the full physical envelope or provides a durable saved queue.

### Matched-prefix gain

The first 74,326 completed domain records are identical after removing only
their per-record `seconds` fields, including all bounds, ranks, native counters
and optional refusal details. Their normalized SHA256 is
`a0482095adc545331f212bd8ba3616ec8f160fd6c882cf083400d8b48aee4c33`.
Heartbeats bracket that prefix at 108.625–109.632 s traversal with the maximal
index, versus 315.671–316.676 s previously: **2.88–2.92× faster** in these
single shared-host runs. This is not a repeated statistical estimate.
Near that prefix, total containment comparisons fall from about 6.08 billion
to 0.85 billion; maintenance is already included and must not be added twice.

The resumed diagonal averages 43.11 busy cores over its 110 samples from total
time 110–330 s. Its queue nevertheless continues growing. The gain repairs
the earlier measured prefix bottleneck, not the total amount of required work.

### Why the all-owner control still stalls

At total time 174.408 s, 65,536 completed results fill the return-buffer entry
limit, using only 0.757 GB of its byte allowance. The main thread is running
while all 50 compute workers are waiting; a nearby resource sample shows 1.084
busy cores. Publication still progresses, so this is neither deadlock nor
memory exhaustion. The large set of mutually non-containing boxes defeats
simple retirement: 358,405 lookup candidates remain at the stop.

This supports a serial admission/index bottleneck. It is not a stack-sampled
profile proving an exact percentage of time in one function. Increasing the
result buffer or compute worker count would postpone the symptom without
addressing its cause. The domain and event ceilings are inherited diagnostic
allowances, not physical limits or closure criteria.

### Audit recommendations, in implementation order

1. **Preserve total-power correlations.** Carry positive-power sum A, numerator
   rank R and D=A-R through matching, exact IBP shifts, routing, cache keys and
   inclusion tests. Independent coordinate caps alone solve an enlarged input
   problem. The geometry service and campaign propagation are now integrated;
   the 11 standalone optimized tests include 442,368 exhaustive small-domain
   comparisons, and the full release core gate passes 2,748 tests (32 ignored).
   Application/integration gates pass 435 tests and Python/steering gates pass
   72; the constrained recursive pilot below remains incomplete. None of the historical
   timings above includes this integration. Retain predicates after
   projection, including in guarded pullbacks; do not overwrite sibling-cell
   rank limits or lose the existing rank-orthant fast path during normalization.
2. **Remove serial lookup work.** Profile per-owner/phase admission and redundant
   domain representations before introducing a complex index. Test semantic
   normalization, conservative containment filters and deterministic batches.
   Preserve exact keys and every pending obligation. Lookup subsumption is not
   proof of completion. Compare the same saved prefixes before widening runs.
3. **Measure routing expansion.** Score already verified alternative momentum
   maps by numerator expansion and successor growth. Kira 3 and TIDE support
   considering native-sector reduction when symmetry routing is more costly;
   this is a proposed experiment, not a demonstrated speedup here.
4. **Generate only for a real gap.** Current domain walking does not invoke IBP
   generation. If a conservative frontier appears, first obtain a concrete
   reachable missing target, then use fixed-target feedback. For genuine gaps,
   test adaptive small source neighborhoods/tubes before broad source sweeps.

The independent source audit finds no confirmed mathematical defect in the
reviewed bounded-routing/maximal-index paths, and no evidence that these stops
demonstrate an IBP cycle or exceptional infinite ray. Native concrete descent
checks remain in place, but do not establish convergence of the unfinished
conservative region traversal. No defensible full-envelope ETA follows.
Do not launch the enormous independent-axis A24 envelope or merely increase
all allowances on the all-owner control before addressing this evidence.

Primary-source findings, caveats and concrete experiments are in the
[literature audit](finite_domain_campaign_literature_audit_2026-09-22.md).
Local audit evidence is in `TMP/bounded-routing-pilot.4kY19R/`:
`RESUMED_PERFORMANCE_AUDIT.md`, `IMPLEMENTATION_REVIEW_2026-09-22.md` and
`CORRELATIONS_REAUDIT_2026-09-22.md`. Run receipts are
`shared-owner-campaign.gltu7ezq/` and `shared-owner-campaign.9jj5ev7y/` there.

## Correlated-domain integration and pilot

The implementation retains an A upper bound and D=A-R lower/upper bounds in
native matching, exact sign-cell IBP shifts, admitted affine routing, local
reuse, queue identity/containment and diagnostics. Coordinate tightening never
replaces these predicates. Constrained optional guarded pullbacks fail closed;
the default unconstrained APIs retain their prior behavior. No CAS kernel or
new IBP search is introduced. Query/match/walk schemas are v2, documented in
[the input/API contract](../shared_owner_domain_matching.md).

Release validation passes 2,748 core tests, 353 application-library tests,
82 integration tests and 72 Python/API/steering tests; 32 existing core
diagnostics remain ignored. Independent implementation/math review passes.
An additional orthant shortcut initially failed the existing admission
differential test; it was removed without weakening that test. The corrected
focused campaign gate passes all 134 tests. Final post-build source edits
were comments and indentation only. Logs and the frozen CLI are under
`TMP/power-domain-gate.UpQa87/`; CLI SHA256:
`32a49e46147d443bbc77329a0f1a38b19bebb080ab43bf84cff7f5f0212b9ed8`.
The tested tree includes the same separate, uncommitted scheduler/escrow work;
these are not measurements of the isolated correlation commit.

### Local first-rule classification

Intersecting the earlier diagonal with A<=24 and D>=10 retains 980 of its
2,560 points, including all 50 earlier concrete stress roots. All 980 match
the saved singleton oracle, with zero partition/selection mismatches and no
excluded point admitted. The 28 selected-rule regions take 0.008475 s matching,
0.702373 s preparation and 0.710908 s application total. These are **local
first-rule lookup** times, not IBP generation, recursive reduction or closure.

Input, verifier and independent audit:
`TMP/correlated-routing-pilots.jw6Bwo/{diagonal-a24-d10-r10-v2.json,verify-match.sh,LOCAL_MATCH_AUDIT.md}`.
Local result: `match-probe.hQCWzw/result.json` in that directory. The D>=9
variant and schema-only controls were prepared but not run in this slice.
Neither small diagonal covers the complete marginal R14 or full-jet R15 domain.

### Recursive run: incomplete, stopped for optimization

The unchanged 67 owners and 8,246 admitted route records were used with 50
workers, fixed CPU affinity, nested pools of one, and the previous 500/450 GB
hard/soft RSS allowances. No elapsed deadline was imposed. Python/interface
gates overlapped only serial preparation and ended before 50-worker traversal.
This is a shared-host single run, not an exclusive-host statistical comparison.

| Measurement | Constrained diagonal |
|---|---:|
| Starting points | 980 |
| Preparation | 102.701 s |
| Traversal before cooperative stop | 106.956 s |
| Application total | 209.657 s |
| Supervisor lifetime | 216.138 s |
| Sampled aggregate CPU | 650.32 s |
| Sampled peak aggregate RSS | 9.589 GB |
| Completed / scheduled domains | 49,686 / 195,118 |
| Queued / partial cancelled domains | 145,431 / 1 |
| Committed events | 3,518,508 |
| Successors / conditional successors | 2,431,056 / 39,600 |
| Observed unresolved frontiers | 0 |
| Containment comparisons, including maintenance | 4,971,600,936 |
| Reverse maintenance comparisons | 987,619,360 |
| Live / retired containment candidates | 58,169 / 136,949 |
| Route masks / routed domains | 1,440,040 / 36,302 |
| Maximum admitted descendant rank bound | 11 |

At approximately 125 s total, the completed-result buffer reaches its 65,536
entry ceiling, using about 2.50 GB of its 8.59 GB byte allowance. Over 37 samples
between 125 and 200 s, mean observed utilization is only 2.084 cores. A thread
snapshot shows the coordinator running while most compute workers wait. The
queue keeps growing and comparisons dominate admission activity. Root stops
cooperatively to investigate this bottleneck; no memory/event/domain cap or
short timeout triggers the stop. Exit is 4, without a hard kill. These facts
justify an optimization experiment, not an assertion that completion exceeds
fifteen hours. There is still no defensible whole-envelope ETA.

Every committed domain retains power metadata and finite positive coordinate
uppers. Descendant A bounds range from 9 to 25 and D lower bounds from 8 to 13;
817 records have A upper>24 and 144 have rank bound>10. These legitimate
translated images demonstrate that entry restrictions are not reapplied to
children. The 4,326 correlation-empty cells and 682,241 pruned routing masks
are geometric exclusions, not missing IBPs.

### Recommendation after the run

Measure cached semantic containment and selective index lookup next, rather
than merely raising caps. Although 44,847 records carry a redundant D upper
bound at least as large as A upper, dropping only that field reduces distinct
descriptors by just 168 (about 0.34%). Thus field cleanup alone is not a demonstrated
solution. The stronger proposal uses existing exact coordinate/A/R/D extrema
to test implication without repeated geometry work per comparison. Preserve
every admitted/pending obligation regardless of index retirement.

Also census full unit-permutation routes before implementing an exact
permutation image; the existing generic overcover can introduce spurious
pinches even for such maps. No performance win is claimed for either proposal.
Do not widen the physical campaign or regenerate rules without a concrete
reachable rule gap. The changed workload prevents a speedup claim relative to
the larger rectangular runs above.

Recursive receipts: `TMP/correlated-routing-pilots.jw6Bwo/shared-owner-campaign.jtsg2q0t/`.
The result is diagnostic, not a durable pending-work checkpoint or a closing
artifact. No solver remains running from this pilot; the project goal is active.

## Semantic-only correlated rerun: useful reuse, still incomplete

The integrated release gate now passes 361 application tests (one replay
diagnostic ignored), including both actual serial/parallel equivalence test
loops. The prior two failures were corrected fixed-count fixture expectations;
explicit equivalent-domain reuse assertions were added. Core passes 2,758
tests (32 ignored), the isolated queue harness 27 (one ignored), and the
separately run integration suite all 82 tests. This resolves the gate status
recorded in the earlier performance-audit snapshot.

The next run uses the **same 980-point A24/D10/R10 input**, saved 67-owner
manifest, 8,246 routes, 50 workers, CPUs 0–49 and 500/450 GB hard/soft RSS
allowances. Normalized supervisor requests compare identically; no elapsed
deadline or changed diagnostic cap is introduced. The frozen release CLI is
`TMP/semantic-containment-gate.qn9MTE/rustred`, SHA-256
`891cf5ae621760c653645e3ae17e9246111f803f2d8684d8cad942eb85648d10`.
The audit verified the hash through the running process executable and checked
its actual affinity and process start identity. This is the semantic-index-only
baseline: pending affine-support routing edits were **not** compiled into it.
No root-owned build overlaps the measured traversal.

| Measurement | Earlier correlated pilot | Semantic-only rerun |
|---|---:|---:|
| Preparation | 102.701 s | 104.124 s |
| Traversal including cooperative stop | 106.956 s | 169.163 s |
| Application / supervisor time | 209.657 / 216.138 s | 273.287 / 282.206 s |
| Sampled aggregate CPU | 650.32 s | 1,219.85 s |
| Sampled peak aggregate RSS | 9.589 GB | 11.515 GB |
| Completed / scheduled domains | 49,686 / 195,118 | 125,503 / 229,130 |
| Queued / partial cancelled domains | 145,431 / 1 | 103,626 / 1 |
| Committed events | 3,518,508 | 10,656,512 |
| Comparisons, including maintenance | 4,971,600,936 | 8,157,472,074 |
| Included reverse maintenance | 987,619,360 | 1,407,523,725 |
| Additional semantic reuse hits | — | 2,790,937 |
| Additional semantic retirements | — | 6,136 |
| Live / retired lookup candidates | 58,169 / 136,949 | 65,855 / 163,275 |
| Observed unresolved frontiers | 0 | 0 |

The new semantic index performs useful extra reuse, but these are **different
partial prefixes** with different admission histories. The table does not
establish a complete-campaign speedup. The approximately 4.27-fold speedup
measured by replaying only 49,686 saved completed descriptors is a separate
index microbenchmark and must not be extrapolated into a closure ETA.

Early traversal uses the worker pool effectively: at 116 s total the rerun has
19,112 completed, 41,101 pending and 29.10 busy cores. Serial admission pressure
then returns. At total times 140.295, 180.456 and 257.766 s, the pending queue
grows from 67,230 to 81,538 to 100,947, while comparisons rise from 1.365 to
3.490 to 7.518 billion. The completed-result escrow remains at its 65,536-entry
ceiling; its peak accounted memory is only 3.878 GB of the 8.590-GB byte limit.
Across 28 samples from 212–266 s, mean utilization is 2.075 busy cores (range
1.279–3.287). A thread snapshot finds the coordinator runnable and all 51 other
threads waiting in futex calls. No draining phase has yet appeared.

Root therefore requests a **cooperative optimization stop**, not a timeout,
missing-rule stop or resource-cap stop. The cancellation is acknowledged;
the first finished-result heartbeat follows that acknowledgement by 5.026 s.
The result file is written 9.799 s after stop-file creation and the supervisor
receipt 12.919 s after it. Both owned PIDs are confirmed absent. Exit is 4,
`hard_stopped=false`; no support build starts until after termination.
This decision does not prove the pilot could never finish or would exceed
fifteen hours. It provides a measured reason to reduce the conservative work
feeding serial admission before another attempt.

All 125,504 committed records, including the cancelled partial record, retain
power metadata and finite positive coordinate uppers. A upper ranges 9–25,
D lower 7–15, and the maximum admitted descendant rank cap is 11. There are
2,895 records with A upper>24 and 1,328 with rank cap>10. These are legitimate
translated domain bounds, not reapplication of the starting caps or proof
that every point in a conservative overcover is concretely reachable.
Node accounting is exact:
229,130 scheduled = 125,503 complete + 103,626 pending + one partial. Zero
observed frontiers does not discharge the pending obligations. Neither a
closed five-loop artifact nor a durable pending-work checkpoint is produced.

The next isolated slice is **support-aware affine routing**, not full-family
permutation specialization. The native census found that none of the 32
actually used nonliteral maps is a full denominator permutation. By contrast,
support-aware numerator degree budgets strictly tighten 17,304 of 27,807
observed root covers after native projection. That diagnostic is not an
end-to-end speedup or an exact count of removable successors. The next run must
measure actual route fanout, queue growth and admission cost after its own
endpoint tests and independent audit. Do not regenerate rules without a
reachable concrete rule gap.

Receipts: `TMP/correlated-routing-pilots.jw6Bwo/shared-owner-campaign.r8txq1a3/`.
Independent launch/protocol/final audit:
`TMP/semantic-d10-pilot-audit.X9KVDj/`. The semantic-only pilot has stopped;
the goal remains active and implementation proceeds to the separate routing
slice.

## Support-aware routing pilot: local pruning, global admission regression

The new release gate passes 2,768 core tests (32 ignored), 361 application
tests (one ignored), all 82 application integration tests and all 72 freshly
built Python/API/steering tests. The focused route-cover gate passes 40 tests;
an independent source/mathematical audit passes. This slice derives numerator
degree caps from Symbolica-verified affine-map support and the source power
domain. It tightens surviving denominator lower bounds and rejects impossible
pinches; it neither expands numerators nor regenerates IBPs.

The frozen executable is `TMP/affine-support-gate.cv24Y0/rustred`, SHA-256
`e870efc3a51f8ec515b7efffcee087e98620c4c5e4dc2639325af0cb6d163a0a`.
The audit verified that hash through the actual running executable, actual
affinity on 50 distinct physical CPUs 0–49, nested pools of one and the 480 GB
child address-space limit. The request matches the semantic-only baseline
after normalizing only executable, receipt paths and process identities.
Query and manifest hashes remain `541d14fb...220085` and `d2de7341...7f7f`.
There is no elapsed deadline, changed diagnostic allowance or root-owned build
overlapping traversal. Both binaries include the same separate scheduler/escrow
work; these are combined-tree measurements, not isolated clean-commit timings.

| Measurement | Semantic-only baseline | Support-aware routing |
|---|---:|---:|
| Preparation | 104.124 s | 103.737 s |
| Traversal including cooperative stop | 169.163 s | 121.100 s |
| Application / supervisor lifetime | 273.287 / 282.206 s | 224.836 / 232.155 s |
| Sampled aggregate CPU | 1,219.85 s | 591.78 s |
| Sampled peak aggregate RSS | 11.515 GB | 8.666 GB |
| Complete / scheduled domains | 125,503 / 229,130 | 63,913 / 177,414 |
| Pending / cancelled partial domains | 103,626 / 1 | 113,500 / 1 |
| Committed events | 10,656,512 | 3,020,915 |
| Containment comparisons, including maintenance | 8,157,472,074 | 6,886,904,922 |
| Included maintenance comparisons | 1,407,523,725 | 1,469,956,434 |
| Live / retired lookup candidates | 65,855 / 163,275 | 83,753 / 93,661 |
| Semantic hits / retirements | 2,790,937 / 6,136 | 1,012,817 / 7,058 |
| Committed Route inspections | 71,749 | 40,637 |
| Route masks examined / pruned | 2,355,356 / 1,161,021 | 2,006,778 / 1,696,703 |
| Route-emitted descendants | 1,122,586 | 269,438 |
| Observed unresolved frontiers | 0 | 0 |

These totals are different stopped prefixes. Their smaller time, event count
or memory does **not** establish a full-campaign improvement. At approximately
20, 40 and 60 seconds after each first nonzero traversal heartbeat, the support
run has only 16,794 / 26,588 / 37,551 completed domains, versus the baseline's
30,037 / 51,871 / 65,634. Its pending queue grows to 59,623 / 81,410 / 97,450,
versus 60,975 / 69,106 / 75,794. Live candidates are also higher at each window:
42,250 / 55,308 / 67,585 versus 30,690 / 39,265 / 43,202. These observations
show an unfavorable admission/queue trend, not an equal-work speedup ratio.
Tighter boxes can reduce subsumption and change the traversal history; this is
a plausible explanation supported by index growth, not a causal profile of
every extra comparison.

### Equal-source comparison: the local routing improvement is real

Match completed Route records by the entire source descriptor: owner, phase,
coordinate lower/upper bounds, rank cap and every A/D predicate. There are
5,910 identical descriptors in both results. On this common set:

| Committed native routing statistic | Baseline | Support-aware |
|---|---:|---:|
| Masks examined | 221,692 | 221,692 |
| Masks pruned | 113,977 | 197,353 |
| Emitted Apply domains | 4,367 | 4,367 |
| Emitted Route descendants | 101,805 | 18,429 |
| Emitted zero sectors | 1,543 | 1,543 |
| Total emitted events | 107,715 | 24,339 |

Thus 2,867 common queries emit strictly fewer events and none emits more;
routed descendants fall **81.90%**, total events **77.40%**. For example, the
same first Route descriptor (owner `111010100100100`, rank cap 9, A<=23,
D>=10) examines 128 masks in both implementations. Its old 63 Route children
and one Apply image become just the Apply image. This is removal of a
conservative overcover, not deletion of 63 established reachable integrals.
Equal-source pruning is demonstrated, but it does not cure global admission.

### Stop, integrity and next step

At total time 204.505 s the support pilot has 56,500 complete and 111,573
pending domains, 5.866 billion comparisons and 79,704 live candidates. The
65,536-entry result escrow is full while its peak accounted memory remains
only 2.149 GB against an 8.590 GB byte allowance. Across 38 resource samples
between 140 and 216 s, mean busy-core use is 2.231 (range 1.174–5.828).
Snapshots show a runnable coordinator with most workers waiting. Root requests
a cooperative optimization stop; no resource allowance or elapsed deadline
triggers it. The sample does not prove a fifteen-hour completion impossible.

The stop file is created at 19:39:01.287 UTC; the result is written 7.751 s
later and supervisor completion follows after 10.539 s. On the separate
heartbeat clock, first cancellation acknowledgement to first finished result
takes 3.015 s. Both owned processes have exited, exit status is 4 and
`hard_stopped=false`. Accounting is exact:
177,414 scheduled = 63,913 complete + 113,500 pending + one partial.
No pending-work checkpoint or closing artifact is produced.

All 63,914 committed records retain finite coordinate uppers and A/D metadata.
A upper ranges 9–25, D lower 8–14 and maximum descendant rank cap is 11;
975 records have A upper>24 and 270 rank cap>10. These are legitimate translated
bounds, not proof that every point in each conservative cover is reachable.
Zero frontiers does not discharge unfinished obligations.

The immediate next performance target is the serial containment lookup and
its growing candidate sets: add cheap sound filters or a measured selective
index before exact semantic inclusion, while preserving deterministic order
and all pending obligations. Keep the support bounds for their demonstrated
mathematical precision, but do not advertise an end-to-end improvement yet.
Do not widen the physical envelope or regenerate rules without an actual
reachable missing target. Receipt:
`TMP/correlated-routing-pilots.jw6Bwo/shared-owner-campaign.ujyk56ua/`.

## Historical pause and resumed sequence

The user explicitly resumed work from this report. The following pause is
historical and does not direct current execution. Steps 1 and the R0 portion
of step 2 have now run, and step 3 is implemented and piloted. The measured
growth defers the larger guard-box run until the remaining index/routing work
above. The remaining scope restrictions apply.

The user requested a quota-driven checkpoint and stop. No further five-loop
campaign is being launched at this checkpoint. All research/implementation
subagents and the final release gate have finished. Work stops after the
checkpoint commit/push; no long-running solver is left active.
Do not resume implementation or a campaign without a new user instruction.

When resumed:

1. Use the tested maximal-index release binary for the same diagonal input,
   recording its checksum and exact working-tree provenance. Keep the saved
   owner manifest and ordering unchanged. Compare completed work and index
   maintenance honestly; the preceding run was cancelled, not completed.
2. Run the prepared all-67-owner binary-dot/R0 pipeline control, then the 57
   bounded R10 guard boxes if the measured growth is reasonable. Do not blindly
   launch the enormous independent-axis A24 overcover.
3. Preserve compact total-A and A-R constraints if conservative domains still
   grow excessively. The audited design is in the input-contract documentation
   and local `CORRELATIONS{,_AUDIT,_IMPLEMENTATION}.md` notes beside the pilot.
4. Keep new-root admission above saved R10 separate from actual descendant rank,
   which already exceeds R10 legitimately. Do not rewrite saved provenance.
5. Continue toward the full finite physical input envelope; do not start terminal
   minimization or numerical evaluation. Require an actual reachable missing
   key before declaring a conservative-domain frontier a missing IBP.

The unrelated scheduler/escrow work and other pre-existing files remain
untouched and must not be silently attributed to these commits. Native owner
programs and local receipts remain available; the stopped queue itself is not
a durable resumable work checkpoint.
