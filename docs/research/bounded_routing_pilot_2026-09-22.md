# Bounded-routing five-loop pilot

## Outcome

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
