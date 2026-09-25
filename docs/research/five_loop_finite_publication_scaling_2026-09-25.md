# Five-loop publication and work-amplification control

The running full campaign has millions of pending obligations but often uses
only a few of its 50 reserved compute workers. This study separates two
questions: whether Ready publication improves complete recursive traversal,
and how much work is spent repeatedly discovering already-known containment.
Neither a small control nor a published starting region establishes closure
of the full five-loop campaign.

## Workload and boundaries

Use one original required owner, `011101110111000`, from the frozen 67-owner
manifest, with all saved programs and 8,179 nonidentity routing transports.
Keep its original coordinate bounds and D=A-R>=9, but restrict the entry to
R<=2 and A<=11. These bounds contain exactly 1,324 integer starting tuples.
This is an explicit input restriction, not topology-specific engine logic.
All descendants remain required; no descendant is clipped to the entry caps.
No auxiliary initial covers, prior checkpoints or newly generated rules are
used. Stored descendant rank caps are not necessarily tightly attainable:
the observed maximum cap 6 does **not** prove that an R=6 integral occurs.

Each worker-count comparison uses four fresh processes in the fixed order
Ordered / Ready / Ready / Ordered, the same normal-release executable
`8f725507122332a05f68b308d08cd2c5bf9db2932f403cde8ebcba8373cdf106`,
TransferUnreserved lookahead 256, initial-D overlap reuse, finite-axis routing
overcovers and unlimited cumulative work. Application-cell refinement and
physical subdivision are off. There is no elapsed-time or work-count limit.
RAM protection is 100 GB hard / 95 GB cooperative, with 20 GB host reserve.

Six workers reserve three inspectors, two lookup helpers and one coordinator
on CPUs56–61. The subsequent 50-worker comparison reserves 25 inspectors,
24 helpers and one coordinator on 50 distinct physical cores64–113. Both
sets are disjoint from the live campaign's CPUs0–49. This is not an exclusive
host, and worker reservations are not measured CPU utilization. No compiler,
other controlled native workload or profiler overlaps the timing matrix.

Report preparation separately from traversal. Preparation loads the complete
saved manifest and verifies/compiles its routing witnesses before walking the
selected input. Traversal includes final checkpoint writing but excludes final
report output and unloading. Whole-command wall/CPU/RSS includes those costs.
Each process has an independent hourly/final checkpoint; before/after live
status records any production checkpoint overlap. A leg waits for an already
active live save before launch, rather than changing the live process.

## Completion checks and current measurement status

The independent streaming audit visits every final record, using separate
Apply and Route invariants. It resolves alias and pinned-initial dependencies,
reconciles phase-specific counters, and requires zero pending obligations,
frontiers, missing routes and failures with a drained queue, worker pool,
ledger and final checkpoint. Separately check that the Ready experiment actually
publishes out of order; generic record auditing does not require a particular
schedule. Different
publication orders may change IDs and fragmentation; byte equality or identical
native-job counts across policies are not required.

This is a complete traversal check on saved candidate rules, not a new exact
IBP identity proof or a proof that every conceivable entry family terminates.
Fresh full-drain runs also do not replace the separate
interrupted-multiple-prefix-resume-to-exhaustion comparison, which remains open.

All four fixed six-worker processes complete successfully. The independent
phase-aware audit visits all 5,094,173 final records and checks every alias and
initial-anchor dependency. All guards exit cleanly, with no resource stop,
forced termination or supervisor error. Production remains at saved checkpoint10
with no active write in every before/after bracket.

| Six-worker run, in execution order | Ordered 1 | Ready 1 | Ready 2 | Ordered 2 |
|---|---:|---:|---:|---:|
| Preparation (s) | 85.618 | 84.556 | 85.736 | 85.275 |
| Traversal including final checkpoint (s) | 363.557 | 382.723 | 388.450 | 362.790 |
| Final checkpoint (s) | 10.840 | 10.378 | 10.330 | 10.480 |
| Guard elapsed (s, including exit-poll delay) | 464.311 | 484.399 | 490.538 | 464.258 |
| GNU-time whole-native wall (s) | 464.20 | 482.48 | 489.52 | 462.90 |
| Whole-command CPU (s) | 1,155.149 | 1,129.621 | 1,140.988 | 1,149.934 |
| Native inspections | 967,621 | 974,425 | 973,974 | 967,621 |
| Logical obligations | 1,273,376 | 1,273,492 | 1,273,929 | 1,273,376 |
| Events | 31,911,351 | 32,228,635 | 32,210,567 | 31,911,351 |
| Peak sampled RSS (GB, decimal) | 14.686 | 14.703 | 14.702 | 14.676 |
| Pending / frontiers at completion | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |

Ready is slower in both paired traversal comparisons: **+5.27% and +7.07%**.
The ratio of the policy medians is +6.17% (363.174 versus 385.586 seconds).
Ready's median whole-command CPU is 1.50% lower, while its median native work
is 0.68% greater. The Ordered runs reproduce all aggregate work counters;
Ready's small between-run differences reflect readiness-dependent admission
and reservation. These two pairs are descriptive shared-host measurements,
not statistical confidence bounds or an all-workload no-regression proof.

Ready's final publication-ordered reports have 200,125 and 199,894 adjacent
descending-ID transitions, so out-of-order publication was exercised. Those
counts are not the total number of all out-of-order publications. Heartbeat-
weighted mean held completions drop from 97.19/102.47 under Ordered to
0.83/0.76 under Ready, but that does **not** produce a wall-time improvement.
Mean active inspector slots are 2.47/2.35 versus 1.80/1.91. These observations
use approximately one-second heartbeat samples within walking intervals;
active slots are not measured busy CPU cores.

The six-worker resource logs do not retain per-process sampled CPU counters,
so traversal-only mean busy cores cannot be recovered. Do not divide whole-
command CPU by traversal time. The prepared 50-worker guard instead persists
already-read CPU counters and sample timestamps, with no extra process reads
or native-policy change. Its analysis will report the observed walk-window
boundaries and sampling uncertainty separately from complete traversal time.

The subsequent 50-worker matrix, below, uses the same input and completes.
Neither fewer held jobs nor successful fresh-process exhaustion replaces the
outstanding full-resume gate or establishes scaling on the live full campaign.

Raw evidence: `TMP/ready-five-loop-finite-v2.83u7I3/` and
`TMP/ready-five-loop-finite-w50.a6ABXd/`. An earlier harness attempt failed
before native launch because its parent affinity did not include the child's
requested CPUs. It is retained separately; the corrected launch explicitly
sets child affinity before entering the RAM guard. It is not a failed solver
run or a discarded timing.

## Completed 50-worker comparison

All four processes exit successfully, with an independent full-record audit of
5,108,457 logical obligations and zero pending work, frontiers or failures.
Both Ordered runs reproduce the baseline native, logical and event counts.
Ready retains every descendant and resolves all aliases/anchors despite small
readiness-dependent differences in work. All controlled processes are reaped
before the subsequent diagnostic build or checkpoint-recovery control begins.

| Fifty-worker run, in execution order | Ordered 1 | Ready 1 | Ready 2 | Ordered 2 |
|---|---:|---:|---:|---:|
| Preparation (s) | 86.432 | 85.999 | 86.457 | 85.829 |
| Traversal including final checkpoint (s) | 312.792 | 238.349 | 234.753 | 317.517 |
| Guard elapsed (s) | 416.401 | 342.394 | 338.306 | 420.481 |
| Whole-command CPU (s) | 2,197.168 | 2,440.241 | 2,376.788 | 2,212.355 |
| Native inspections | 967,621 | 982,498 | 985,839 | 967,621 |
| Logical obligations | 1,273,376 | 1,278,356 | 1,283,349 | 1,273,376 |
| Events | 31,911,351 | 32,311,156 | 32,441,738 | 31,911,351 |
| Peak sampled RSS (GB, decimal) | 14.887 | 14.821 | 14.848 | 14.884 |
| Sampled interior walking mean busy cores | 6.959 | 10.328 | 10.209 | 6.934 |
| Pending / frontiers at completion | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |

The first clean pair shows **23.80% shorter traversal** for Ready, with
11.06% more whole-command CPU and 1.54% more native inspections. The reverse
pair also favors Ready (26.07%), but has a known shared-host confound: the live
campaign writes checkpoint11 from 06:23:05 to 06:25:48 UTC during Ordered2.
No result is discarded or retried. The direction and size of that overlap's
effect are not established; neither contention nor reduced competing native
work can be isolated from these measurements.

Descriptive medians are 315.154 seconds Ordered and 236.551 seconds Ready:
24.94% shorter traversal, 9.24% more whole-command CPU and 1.71% more native
work. These medians include the confounded leg and are not a confidence-bound
performance guarantee. The Ready 50-worker median is 38.65% below its six-worker
median; Ordered improves by 13.22%. This is useful but far from proportional
scaling across 8.33 times as many reserved slots.

Unlike the earlier W6 logs, these guards retain native PID/start-bound CPU
counters. `cpu-windows.json` uses complete two-second resource samples inside
the initial-save-to-final-save-start interval: approximately 296.331, 222.267,
220.236 and 300.414 seconds respectively. It excludes preparation, final
checkpoint writing, report output and endpoint gaps. The table's busy-core
figures describe only those sampled interior windows, not exact full-traversal
CPU; they must not be compared with W6 active-slot counts. Integer checkpoint
timestamps, process-read brackets and clock ticks limit the boundary precision.

The result supports testing Ready further, not switching the live process.
The separate finite recovery control pauses cleanly, but the actual saved
checkpoint has no positive unfinished source prefixes: they drained after the
telemetry trigger. A genuine later finished hole remains, which does not satisfy
the required multiple-prefix gate. The result is **inconclusive**, and no resume
or automatic retry is launched. The original paused-report optional-field
accessor error is retained separately; fixing that harness error does not make
the saved state qualify. This functional control is not another timing sample.
Evidence: `TMP/ready-five-loop-full-resume.IQlQAN/`. The current production
checkpoint is bound to Ordered, its executable and its request. It cannot
simply be resumed as Ready.

## Live campaign observations during this study

The independently checked 06:11:58 UTC snapshot has 7,814,472 completed native
inspections, 12,640,663 pending obligations and 115.950 GB RSS, with no frontiers
or failures in the retained telemetry. Its checkpoint-free 25-minute window
averages **4.065 measured busy cores**, 2.60 active inspector slots and 161.03
finished-but-held inspections. Admission preparation plus commit occupies
34.10% of coordinator elapsed time, not that share of CPU. Pending work grows
268,865 during that window. These observations identify both head blocking and
admission work; pending obligations are not a fixed denominator or cost estimate.

The next hourly checkpoint completes normally at 06:25:48 UTC: generation11,
18,758,305,527 bytes, 162.579 seconds, preserving 7,884,732 native completions
and 12,769,084 pending obligations. The unchanged campaign resumes at roughly
117 GB RSS. It is still running, not closed, and has no defensible completion
ETA. Neither this control's four-minute Ready traversal nor all 134 published
entry regions means the full five-loop family is solved.

Bounded observation evidence: `TMP/five-loop-mid-w50-monitor.cbtoBf/` and the
timing matrix's retained live before/after status files. No live signals,
profiles, policy changes or large checkpoint reads were used for these snapshots.

## Why 1,324 entry tuples do not mean 1,324 jobs

The audited first Ordered run issues 28,406,787 scheduling requests:

```
1 initial + 25,077,490 Apply successors
          +    653,193 Route-to-Apply covers
          +  2,676,103 Route-to-Route covers
```

Of these, 27,133,411 (95.517%) reuse existing obligations, leaving 1,273,376
logical domains. This is not the 31,911,351 event count: other callbacks account
for rule ends, zero sectors, classifications and diagnostics. The run performs
271,475 native Apply and 696,146 native Route inspections; aliases discharge
the remaining logical responsibilities. Routing is 71.944% of native jobs,
not necessarily that fraction of CPU.

Apply visits 1,723,391 matched pieces, 43,099,305 equal-shift groups and
47,998,700 RHS terms before global admission deduplication. Exact recurrence
branching can produce many intermediate domains. Separately, routing uses a
conservative image under an admitted affine numerator map: it retains necessary
total-power constraints and per-target degree caps but can lose correlations.
Already 9,524,196 of 12,896,445 candidate route masks are pruned (73.851%).
The survivors are not proof that every represented integer endpoint occurs in
an exact expanded numerator. Aggregate counters alone cannot separate necessary
intermediate work from avoidable geometric overcoverage.

Containment reuse is phase/owner-specific and requires one containing admitted
domain, not arbitrary coverage by a union of boxes. Index retirement preserves
historical responsibilities; a pending containing obligation is not completed
coverage. Wider auxiliary regions can improve reuse while also creating extra
obligations. None of these mechanisms gives a fixed remaining-work denominator
or a credible ETA for the full campaign.

## Next optimization targets

1. **Measure repeated successful inclusion queries across jobs.** The exact
   queue map stores newly admitted domains, not contained proposals. The
   job-local cache resets at each inspection. The first Ordered run has
   18,590,984 global non-exact reuse requests, but their repeated-key fraction
   has not yet been measured. A bounded test-only spectator will observe real
   source IDs and full phase/owner/geometry keys without serving hits or changing
   admission. Measure cross-job hits, FIFO eviction, destination concentration
   and original forward comparisons before implementing a production memo.
   A future memo must preserve actual representative responsibility, immutable
   domain inclusion, retired-index/ledger chains, cancellation and checkpoint
   replay; it must not cache misses across insertions or claim pending work done.
2. **Investigate tighter route images.** A joint necessary degree bound for a
   subset of removed propagators could use the union of source numerator rows
   capable of feeding that subset, without double counting a shared source row.
   This is a proposal requiring proof and concrete endpoint tests, not an
   implemented optimization or demonstrated reduction in work. Fusing literal
   or zero-sector Route dispatch must still perform source-validity and geometry
   checks and preserve the original responsibility.
3. **Measure residual overlap sharing.** A domain outside an already responsible
   containing cover may be smaller than the original job. The separate exact
   initial-D cut experiment demonstrates a local opportunity, not a general
   recursive speedup. Arbitrary hull widening can add work; union coverage
   requires real dependencies on every constituent obligation.

In the first completed Ordered control, admission preparation and commit take
99.484 and 75.439 seconds, together 48.114% of traversal coordinator elapsed.
Those are not CPU-profile fractions. Ready changes when work can publish; it
does not remove recurrence evaluation, geometric widening or repeated lookup.
The separate minimum-eight-task-grain experiment found no useful wall-time gain.
These results motivate reducing repeated work as well as testing scheduling.

The expansion analysis and recommendations received an independent source and
interpretation audit; detailed notes are retained in
`TMP/finite-walk-expansion-review.MNc6xR/FINDINGS.md`.
