# Bounded-routing five-loop pilot

## Outcome

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
No end-to-end speedup is claimed before a matched rerun.

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

## Pause and resume

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
