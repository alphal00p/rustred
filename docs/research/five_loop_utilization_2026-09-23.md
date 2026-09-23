# Five-loop full-owner utilization investigation — September 23

## Latest active run: progress without an exhaustion estimate

This snapshot refers to the **renewed initial-overlap run**, receipt
`TMP/initial-overlap-gate.4Tj82V/shared-owner-campaign.alc7oe24/`, with executable
and input hashes recorded under "Renewed run launched" below. Historical sections
below describe separate, already stopped attempts. The current process remains live;
no solver replacement, rule regeneration or restart accompanied this audit.

At heartbeat **12628.923 s** (about 3.5 hours), native publications reached
**5,094,503**, with **6,473,505 pending native obligations**, zero observed
frontiers and maximum scheduled descendant rank bound 19. Nearby aggregate RSS was **79.71 GB**;
the preceding half-hour sampled peak was **82.59 GB**. RSS fluctuates, so the
peak is not the current value and a linear memory forecast is inappropriate.

| Last two complete 15-minute windows | Earlier | Latest |
|---|---:|---:|
| Native publications/s | 365.86 | 333.31 |
| Net native demand/s | 584.32 | 536.03 |
| Pending-native increase | 196,473 | 182,304 |
| Mean busy cores | 7.64 | 8.54 |

All six five-minute bins still grew pending work. Actual utilization in the
latest window is about **17% of the 50-core budget**, not fifty busy workers.
Admission preparation/helper-wait plus ordered commit occupied about 67% of
the coordinator wall interval; this is not a per-thread CPU profile. The
longest observed publisher span in that window was about two seconds, rather
than the earlier heavy-Apply tails. Different work regions are being visited;
these observations do not establish an optimization speedup.

The same full attempt continues without an elapsed deadline or a defensible
under-ten-hour ETA. The existing two-billion-event allowance has 1.253 billion
events left at this snapshot. At the latest event rate that would last about
3.63 additional hours (7.14 hours total); this is a **conditional time to a
resource limit, not an estimated completion time**. The limit cannot be changed
in the live immutable request. A future invocation can already request the
largest supported event allowance without a code change or cap-sized allocation;
that is not a reason to discard this run's progress now. Memory headroom remains
ample. Continue monitoring useful work and the event allowance separately.

Independent receipt:
`TMP/initial-overlap-gate.4Tj82V/INDEPENDENT_UTILIZATION_HEADROOM_AUDIT_12629S.md`.

### Fourth creation wave reached, not a queue drain

The unchanged publisher entered first-creation wave four between heartbeats
14182.886 and 14197.456 s. Original-admission ancestry brackets establish
wave-five admissions by 14198.461 s. During the first 185.032 s after the
proven transition, native service averaged 453.82/s and net demand 1141.28/s:
pending native work increased by **127,202**. Observed frontiers remained zero.

These waves count original admission hops, not concrete reduction length;
historical IDs include subsequently delegated work. Another wave neither
proves an infinite traversal nor predicts its eventual depth. It does rule out
assuming that entering wave four itself ends new admissions. There is still
no exhaustion ETA. The separate one-core diagnostic below overlapped this
interval, so it is not an isolated-host performance measurement. Continue the
same full attempt. Independent evidence is
`TMP/initial-overlap-gate.4Tj82V/INDEPENDENT_WAVE4_CROSSING_AUDIT_14383S.md`.

### Four-hour follow-up: continuing growth and shared-host paging

The complete 14,439.839--15,340.562 s observation window services 275.64 native
inspections/s against 399.28/s net native demand. Pending native obligations
increase by 111,360 to 7,234,523. Average utilization is 6.041 physical-core
equivalents out of 50 configured workers. Zero observed frontiers still
describes inspected work only; neither exhaustion nor a sub-ten-hour ETA is
established.

The nearby RSS fall from 70.76 to 26.92 GB must **not** be interpreted as a
corresponding reduction in retained memory. Read-only process checks find
approximately 81.60 GB swapped out.
A separate 30-second sample at 16:00:36--16:01:06 UTC observes 108,474 major
faults and approximately 444 MB additional process read I/O, while the host
is actively paging under memory pressure. It uses 7.36 core equivalents:
5.73 in admission lookup, 1.27 in inspection, and 0.35 in the coordinator
(plus minor other-thread work and rounding). Relevant cgroup memory and swap
limits are unlimited, with no recorded high/max/OOM events. The configured
500-GB ceiling does not reserve that much resident RAM on this shared host.

Paging is therefore a measured performance confound, not proof that it alone
explains the admission bottleneck. Do not present the lower RSS as a reduced
working set or extrapolate isolated-host performance from this interval.
The full attempt remains unchanged; no other process or host setting was
modified. Additional heavy builds are deferred while the bounded index patch
undergoes source review.
Independent receipt:
`TMP/initial-overlap-gate.4Tj82V/INDEPENDENT_UTILIZATION_AND_SWAP_AUDIT_15340S.md`.

### Small input-only work-compression control

A separate two-loop sunset control tests pre-admitting a finite staircase
`A<=a, R<=M-a`, whose union is `A+R<=M`, using existing input geometry and the
unchanged release CLI. It is an enlarged initial workload, never a descendant
cutoff. Every escaping child remains ordinary work. No new CAS primitive,
rule-application policy or five-loop IBP generation was introduced.

For M=6, the baseline top-sector input produces 13 Apply and 13 Route
inspections. Adding five canonical two-line staircase inputs gives six Apply
and the same 13 Route inspections: **new Apply work falls from 12 to zero**.
Both walks exhaust their worklists with zero frontiers/errors and discharged
reuse obligations. Two runs reproduce the same non-timing counts. The four
pure steering tests pass; independent source review precedes native execution.
Independent result review confirms both nonidentity routing maps are exercised
(ten `110` and three `101` Route records), with identical Route geometry and
top-sector native statistics across the two input policies. Those routed
domains have rank zero; this is not a test of numerator transport expansion.

This is evidence that broader initial anchors can absorb recursive Apply work
with the current engine, not a matched-workload speedup or a five-loop result.
The additional inputs contain numerator cases absent from the baseline, and
native algebra-operation counts actually rise from 274 to 499, despite fewer
scheduled jobs. Five-loop analogues could introduce expensive, unnecessary
regions. The live five-loop input and process remain unchanged. Local protocol, receipts and
review are under `TMP/two-loop-staircase-control.gAcMTD/`. Both tiny native
controls used CPU49, one worker and a 4-GiB address-space bound; exclude their
brief overlaps around 15:16:59 and 15:17:34 UTC from isolated performance claims.

### Five-loop adjacent-band control: no demonstrated improvement

The follow-up uses four existing owners, selected from the documented diagonal
owner and its downward owner/routing dependencies: 2,766,659 bytes of unchanged
bundles and 86 existing routes. Native zero classification remains authoritative;
an absent route in this reduced selection is not assumed zero. No rules were
generated and the full 67-owner attempt was not modified.

Compare the original `A<=24,R<=15,D>=9` inputs with coalesced
`A<=24,R<=16,D>=8` inputs. The latter are exactly the original domains plus
their adjacent D=8 bands, since A<=24 and D>=9 already imply R<=15. Neither
input bounds descendants. All eight local-applicability queries pass with
9,256 rule pieces and two **existing** fixed terminals, no unresolved guards,
gaps, invalid sources or truncation. Matching takes 1.637 s after 2.783 s
preparation; this is not recursive traversal.

The two subsequent recursive walks have identical native settings except
query/output paths, including one worker on CPU49, initial-D reuse, H256,
unlimited containment, 6/8-GB soft/hard memory thresholds, 200,000-domain and
ten-million-event diagnostic allowances, and no elapsed deadline. Both stop
at the domain allowance, not a missing-rule frontier:

| Censored diagnostic | Original D>=9 | Coalesced D>=8 |
|---|---:|---:|
| Complete native inspections | 74,855 | 72,689 |
| Native Apply records, including partial work | 22,803 | 22,849 |
| Native Route records | 52,053 | 49,841 |
| Pending native obligations | 50,123 | 51,401 |
| General containment checks | 3,035,566,804 | 2,925,253,308 |
| Native elapsed seconds | 300.393 | 291.084 |
| Sampled peak process-tree RSS, GB | 1.613 | 1.594 |
| Frontiers | 0 | 0 |

Each has one interrupted native record caused by its allowance. Both fail
worklist exhaustion and dependency discharge; neither is a completed solve.
The elapsed values cover different incomplete workloads and do not establish
a speedup. Maximum scheduled rank bounds are 16 and 17, not proofs that
concrete integrals of those ranks were reached. The native processes share
CPU49 with the main run; their supervisor placement differs slightly.

The enlarged input does not demonstrate useful work compression: new native
Apply records increase from 22,799 to 22,845. Of the baseline's post-input
Apply records, 9,942 fit the proposed anchors, but 12,521 advertise lower D
bounds and 336 advertise A25. A saved domain admits an A25 point, so the
latter cannot all be dismissed as redundant labels; its concrete reachability
has not been established. Moving only the first D boundary leaves further
work and adds extra starting cases.

**Decision:** do not replace the full input on this evidence. Keep the main
attempt running. A separate bounded index-filter implementation is being
prepared for a future binary, retaining native containment authority and
exact admission decisions. It needs independent review, release gates and
measurements including reused requests; the earlier new-admission-only
prototype is not evidence of a campaign speedup.

Independent input, outcome and domain-pattern reviews pass. Full receipts
and the paired report are under `TMP/five-loop-staircase-control.lOhlBm/`;
the baseline is `shared-owner-campaign.jd8xf34i/`, the coalesced attempt is
`walk-coalesced/shared-owner-campaign.no1stu6x/`. Both are terminal, incomplete
diagnostics; no auxiliary solver remains from this comparison.

### Earlier 96-minute observation

At heartbeat **5742.888 s** (about 96 minutes), it had published **3,185,426
native inspections**, with **3,930,281 native obligations still pending** and
**zero observed frontiers**. The nearby sample was **61.64 GB aggregate
process-tree RSS**; the preceding five-minute resource interval averaged
**6.07 supervisor-measured busy cores**.
All 67 starting owners participate. Entry bounds remain A24/R15/D9; descendants
have reached rank **19**, and are not clipped to the starting rank. No
unbounded-rank domain has been reported. These are partial observations, not
a new closing artifact or proof that every remaining region is covered.

An independent analysis of two adjacent fifteen-minute intervals makes the
remaining scheduling problem explicit:

| Interval since launch | Native publications/s | Net native demand/s | Pending increase | Mean busy cores | Ending RSS |
|---|---:|---:|---:|---:|---:|
| 3832.797–4732.788 s | 266.76 | 458.25 | 172,343 | 4.03 | 55.99 GB |
| 4732.788–5633.138 s | 302.86 | 689.76 | 348,347 | 4.63 | 61.44 GB |

Net native demand is the change in historical admissions minus responsibility
transfers; it includes transferred-away older obligations, not just newly born
children. Every approximately 150-second bin in both intervals grew pending
native work. The latest interval's service rates varied from about 111 to
747 publications/s. Retained containment candidates also grew, from 5.779 to
6.309 million. Neither remaining work nor index size has yet plateaued.
These intervals visit different regions and are not matched-work benchmarks.

Parallel admission is implemented: expensive lookup preparation uses immutable
batches, while revalidation and ownership publication remain deterministic.
The 50-worker budget reserves 25 inspectors, 24 admission helpers and one
coordinator. Initial-domain overlap reuse avoids repeated inspection where
native containment proves it safe. Nevertheless, finite lookahead, bounded
stream buffers and canonical publication still allow head-of-line waiting.
Recent heartbeats show Apply publishers lasting 18–25 seconds while emitting
hundreds of thousands of events, and one Route publisher spanning 19.112
seconds without a committed event. These spans locate expensive work; they
are not full standalone timings or evidence of deadlock.

A separate read-only OS audit found CPU0–49 affinity on every native thread,
no cgroup CPU-bandwidth quota and no observed throttling. Thus no hidden
six-core cap explains the low utilization. Own-thread snapshots instead show
bursty lookup work and many sleeping inspectors. Some run-queue delay exists,
so the shared host is not assumed contention-free; changing the Rust scheduler
alone is not a demonstrated route to 50 continuously busy cores.

**Decision:** continue the same full attempt, with no elapsed deadline and
the existing 300/500-GB cooperative/hard RSS limits. There is no defensible
under-ten-hour ETA while the total future work is unknown and the queue is
growing; the measurements also do not prove that such a finish is impossible.
Zero observed gaps is encouraging but does not discharge pending obligations.
Completion still requires an exhausted worklist, resolved dependencies, the
narrow existing-consumer compatibility check, and packaging/cold loading of
the finite-domain program. Terminal minimization, evaluation and five-loop
Vakint integration remain subsequent work, not substitutes for this result.

Independent receipts are `INDEPENDENT_TWO_PERIOD_TREND_5633S.md` and
`INDEPENDENT_SEVERE_TAIL_AUDIT_4868S.md` under
`TMP/initial-overlap-gate.4Tj82V/`, and `CGROUP_CHECK.md` / `RUNQUEUE_CHECK.md`
under `TMP/dispatch-fence-audit.Tl4yhT/`. No production code or live-run
configuration changed during these audits.

### Validated identity-specialization optimization, not in the live binary

The Apply hot-path review found that fixed-index specialization reconstructed
an already canonical coefficient with Symbolica's polynomial GCD enabled even
when the binding list was empty. Identity substitution preserves coprimality.
The narrow follow-up now passes `do_gcd=false` to Symbolica in that case only;
nonempty substitutions still normalize. No CAS kernel, cache, schema, search
policy or resource-limit change is introduced. Context validation, both
preflights, the original denominator guard, output checks and logical/native
accounting remain unchanged. Independent source and mathematical review pass.

Release validation passes **66 indexed-algebra tests**, including five new
identity/guard/limit/context/nonempty-cancellation regressions, and the full
core suite passes **2,792 tests**, with **32 existing ignored diagnostics** and
zero failures. Indexed test execution took 0.02 s; the full core harness took
135.65 s. The separate one-CPU optimized compilation took about 22 minutes;
that is not solver execution time. Gate receipts and independent review are
under `TMP/identity-specialization-gate.r16nMn/`.

This controlled build/test gate used CPU49, one Cargo job, single-threaded
default compute pools and a 32-GiB address-space bound. Exclude
**13:29:25–13:53:45 UTC** from clean campaign-performance comparisons. The live
CLI remains byte-identical to its recorded executable hash: it was not rebuilt,
replaced or restarted. The optimization is for a subsequent binary; no actual
campaign speedup or frequency of empty substitutions has yet been measured.

### First short drainage interval and existing-log creation waves

A fresh 120-second observer ran after the build/test overlap, from
14:05:24 UTC, and exited normally by 14:07:33 UTC. The unchanged solver averaged
**7.16 busy cores**: 5.69 in admission lookup, 1.11 in inspection and 0.35 in
the coordinator. Across nearby heartbeat endpoints 8524.721–8644.675 s, it
published 29,630 native inspections (247/s), while net native demand rose by
25,275 (211/s). Pending native work therefore **fell by 4,355** to 4,996,404.
The preceding complete 300.320-second window also drained: **15,942 fewer
native obligations**, and only 680 additional retained containment candidates
despite 307,081 historical admissions and 306,401 candidate retirements.

This is a genuine short stabilization/drain interval, not an exhaustion claim
or a linear completion forecast. Thirty-second slopes still alternate, and
later regions can create more work. At the frozen endpoint, 3,783,308 native
inspections had been published, with zero observed frontiers, descendant rank
19 and about 70.69 GB sampled aggregate RSS. The raw queued-ID count still
grew because it includes unpublished aliases; it must not replace the pending
native count. Admission preparation plus commit occupied about 71.8% of the
heartbeat interval's coordinator wall time. The longest sampled publisher
dwell was about two seconds, rather than the earlier 25-second Apply tail.
These visit different regions and do not measure a speedup from the newer
identity-specialization code, which is not in this running executable.

An independent read-only source audit also extracted a conservative
**first-creation wave** diagnostic without changing the logger. After initial
admission, every new ID appends during the current canonical publisher's event
stream. Between
coherent snapshots `(admitted=A0, cursor=Q0)` and `(A1,Q1)`, a new ID `j` in
`[A0,A1)` therefore has an original creator in `[Q0,min(Q1,j-1)]`. Root IDs
have depth zero; recursively bounded creator intervals can sometimes identify
a depth exactly. Use `committed_domains` for the cursor, not the displayed
`commit_domain` of a just-published alias. Sparse logging widens the brackets;
heartbeat timestamps are reporting times rather than exact admission times.

At heartbeat **8464.332 s**, those brackets establish that current ID 8,684,231
was first created in wave **3**, while newest ID 17,965,762 was created in wave
**4**. This reveals very broad, still shallow first admission. It is not
concrete reduction length, proof depth, an estimate of the remaining waves, or
evidence that all breadth is geometrically redundant. Transfers alter the
responsibility graph without changing original creation; the 9,281,532
uncommitted IDs at that snapshot are not its 4,985,311 pending native jobs.

Continue the same full attempt. The new drainage is encouraging, but neither
50-core scaling nor a sub-ten-hour ETA is established. Exact receipts and
caveats are in `TMP/initial-overlap-gate.4Tj82V/INDEPENDENT_POST_BUILD_AUDIT_8645S.md`
and `TMP/finite-closing-handoff-audit.9S1OaK/CREATION_WAVE_MONITORING_AUDIT.md`.
No production algorithm, saved rule, input region or runtime setting changed.

## Scope and reproducibility

The resumed investigation uses all 67 prepared starting regions together,
with `A<=24`, `R<=15`, and `A-R>=9`, keeping the saved owner programs and
8,246 admitted routes unchanged. These bounds describe the documented
conditional full-jet envelope, not a catalogue of QCD diagrams. Its exact
planning count is 3,258,551,484,224 labelled starting keys; the run processes
symbolic regions, not that many individual integrals.

The release executable is the previously gated combined-tree binary:
`TMP/parallel-admission-gate.53B924/rustred`, SHA-256
`6e3791ad6a4aee482b5486ddf8946bee101da4fd2992c91364b550455c8a713a`.
It includes the same pre-existing uncommitted scheduler/escrow changes as the
September 22 pilot. No solver implementation or saved IBP was changed for
these runs. The query SHA-256 is
`c80d332a20bf9ed5b957106d96c3e8db1c255ac873d51dbc6f18827a3a94e570`;
the manifest SHA-256 is
`d2de73414a0cc5652d9124a32dd978759daf75abc305b705b53d759a3aad7f7f`.

The Python supervisor pins the native process to CPUs 0–49, sets inner pools
to one, and monitors its process tree. The 50-slot compute budget reserves
25 inspectors, 24 admission helpers and one coordinator. The memory policy
is 450 GB cooperative / 500 GB sampled hard RSS with a 480 GB child address-
space bound; sampled RSS is not an exact high-water measurement. A separate
read-only thread sampler runs on CPU 50. The 15-hour objective is telemetry,
not a timeout. Other host workloads remain active, so these are observational
measurements, not controlled scaling experiments.

All receipts are under `TMP/full-jet-symbolic67.CwBp4L/` and are not shipped.
The input is broader than the earlier 980-point diagonal: comparing their
elapsed times would not measure equal work.

## Completed diagnostic attempts

| Measurement | Original per-query allowances | Raised per-query allowances |
| --- | ---: | ---: |
| Receipt suffix | `yiunlg31` | `v88q1vsx` |
| Preparation | 104.727 s | 103.683 s |
| Traversal including failure/drain | 6.107 s | 61.646 s |
| Native total | 110.834 s | 165.330 s |
| Supervisor lifetime | 114.047 s | 170.097 s |
| Sampled peak process-tree RSS | 6.046 GB | 6.591 GB |
| Completed regions | 0 | 7 |
| Partial failed region | 1 | 1 |
| Scheduled / pending | 13,753 / 13,752 | 500,000 / 499,992 |
| Stop reason | 100,000 predicates per query | 500,000 scheduled regions |
| Observed frontiers | 0 | 0 |

Both attempts exited with status 4, explicitly incomplete. Neither exhausted
memory, hit an elapsed-time deadline, or proved the family closed. Pending
obligations cannot be dismissed because no frontier has yet been reported.

The first attempt's default predicate allowance was too small for a broad
root. Related allowances were raised together through existing public CLI
options: ten million rules/predicates per query, 100 million geometry cells,
3.2 billion coordinate entries, ten million RHS boundary cells, and 400 million
native operations. Exact geometry, ordering, rank and source conditions did
not change. These are work budgets, not mathematical restrictions.

With those allowances, root 0 finishes its local inspection: 342,309
predicates, 9,157 selected rule pieces and 202,895 successors in 10.348 s
worker wall time. Only three bounded-refinement steps (39 faces) occur, with
no reported problem. The original stop therefore did not identify an
exceptional unsolved integral. By root 6, a single local inspection emits
649,259 successors. Worker wall times include waiting for output publication
and are not standalone serial solver timings.

## What limits utilization now

The initial native-heavy phase briefly reaches about 24.8 busy cores: it can
fill the 25 inspection slots, but not the 24 helper slots reserved for later
admission work. Once output buffers fill, 24 inspectors repeatedly wait behind
the currently published inspection. Completed-result escrow cannot unblock a
producer that is still streaming a large result: it stores only finished jobs.

For the raised-budget run, 25 two-second sample intervals aligned with
heartbeat labels 115–164 s cover 50.003 s. Mean measured utilization is
**2.696 cores**: 1.060 native inspection, 1.302 admission helpers and 0.330
coordinator, with a small non-atomic sampling difference. This is not 50-core
saturation. Preparation takes 3.224 coordinator-wall seconds and ordered
commit 17.605 s out of the 61.646 s traversal. Those timers exclude native
waits, dispatch, observation and drain; they are not CPU-time partitions.

The raised-budget run admits 1,833,022 successor events and reuses 1,333,088
scheduling requests. It still reaches half a million distinct obligations
before the eighth initial root finishes. There are 326,261 live containment
candidates and 173,739 retired lookup candidates; retirement does not delete
the corresponding pending obligations. No Route-phase region has committed
yet. Rank-16 descendants are retained even though the starting rank is 15,
as required: entry bounds never clip intermediate work.

The earlier observation that 99.699% of *previously seen* Apply regions fit
the new broader roots was useful motivation, not a prediction for this much
larger input. The output retains processed and attempted inspections, not the
entire pending queue. In particular, these receipts do not establish the
phase/mask distribution of the 499,992 pending regions.

## Larger diagnostic: cooperatively stopped, not a completed workload

Receipt `shared-owner-campaign.lgvisscc` used the same input, ordering,
executable and expanded per-query policy, with aggregate allowances increased
to ten million domains and 200 million events. All 67 initial regions have
finished local inspection and the worklist progressed to descendants.
This is neither a new artifact nor closure of those regions under recursion.

The operator subsequently stopped it for a measured optimization checkpoint,
not because of a deadline, memory exhaustion or mathematical failure. Native
and supervisor processes have exited; the durable result reports:

| Measurement | Final incomplete result |
| --- | ---: |
| Preparation | 104.028 s |
| Traversal including cancellation/drain | 1,865.666 s |
| Native logical total | 1,969.694 s |
| Supervisor lifetime | 2,029.718 s |
| Completed regions | 2,522,397 |
| Cancelled partial / pending | 1 / 6,029,467 |
| Scheduled regions | 8,551,865 |
| Committed logical events | 146,807,714 |
| General containment comparisons | 126,279,058,490 |
| Reverse maintenance comparisons, included above | 9,755,315,281 |
| Sampled peak process-tree RSS | 50.119 GB |
| Observed missing-rule frontiers | 0 |

The final population balances exactly: scheduled = completed + partial +
pending. The partial is cancellation fallout, not a discovered missing rule.
The roughly 60 s between native logical reporting and supervisor exit includes
output serialization, teardown and supervision; it is not a separately timed
serialization benchmark. RSS rose during final output from roughly 36 GB.
The final observed native CPU counter was 17,628.86 s, not fifty cores times
wall time. No hard kill or resource-ceiling failure occurred.

The measured completion rate fell from approximately 3,036 regions/s in an
earlier window to 60/s in the late 1,799.51–1,926.15 s window, while admissions
still exceeded completions. Even the artificial no-new-work drain estimate
for roughly six million pending regions at that late rate exceeds 27 hours.
This is a reason to investigate the bottleneck, **not** a reliable completion
forecast: region costs are heterogeneous and future descendants remain unknown.

All fields except top-level per-record `seconds` match for the first seven
completed regions shared with `v88q1vsx`. Independent streaming extraction also
retains the first 60,000 complete records for a subsequent same-prefix test.
Audit scripts, hashes and compact evidence are in
`TMP/five-loop-utilization-readonly.vV9E5N/`. The result does not persist a
durable pending-work resume or all rejected admission proposals.

The earlier live observations below describe intermediate phases of this
now-terminal diagnostic, not its final state.

At the 778.47 s heartbeat, 1,287,561 regions are complete, 5,591,880 scheduled
and 4,304,319 pending, with zero observed frontiers and approximately 22.8 GB
native-process RSS. These are **live** counters, not a completed workload or
maximum memory measurement. Rank-17 descendants are retained. No input or
saved rule was changed between attempts.

Weighted post-root utilization is increasing, but remains below 50 cores:

| Heartbeat window (seconds after launch) | Native inspection cores | Lookup helper cores | Coordinator cores | Process cores |
| --- | ---: | ---: | ---: | ---: |
| 421–480 | 3.74 | 5.22 | 0.62 | 9.59 |
| 540–600 | 3.86 | 3.34 | 0.76 | 7.97 |
| 601–660 | 5.94 | 5.67 | 0.70 | 12.32 |
| 662–718 | 6.15 | 5.84 | 0.69 | 12.69 |
| 720–777 | 7.31 | 6.39 | 0.67 | 14.37 |

Each entry is summed sampled CPU time divided by summed interval duration,
not a count of reserved or apparently active threads. Small discrepancies
between the component sum and process total reflect non-atomic snapshots.
Pending work grew in every complete minute window in this early selection.
Memory headroom alone did not predict completion. The ten-million-domain
allowance was not a completion denominator.

The separate two-loop test target was compiled/tested on CPUs 74–77 during
part of preparation and early traversal, and an isolated geometry prototype
used CPUs 74–75 later. These were outside the native process's 0–49 affinity;
their CPU is not included in the process-tree solver timings. Neither these
measurements nor the shared host should be described as an exclusive-host
50-core benchmark.

## Implementation implications

The present implementation can split an input region into disjoint coordinate
boxes while retaining its A/R/D predicates. That can shorten indivisible
jobs, but removes the broad parent from the single-region containment index:
a child crossing a partition boundary fits the union, not necessarily one
shard. Adding both parent and shards simply deduplicates the shards under the
parent. A principled scheduler improvement would retain the broad parent's
reuse obligation while dispatching exact partitions and require **every**
partition to finish before resolving the parent. An isolated native-geometry
prototype passes six tests covering 111,132 configurations and 4,000,752
integer-membership checks (`TMP/parent-partition-design.xkm6A5/`). This validates
the partition geometry, not a production scheduler or a performance gain.
Since initial roots already finish, partitioning cannot be assumed to cure
the later admission/Route fanout and could create more fragments.

A smaller experiment tested exact-summary hashing before general containment
searches. It preserved domain geometry and queue obligations and passed its
correctness gates, but the matched full-owner prefix was slower. The experiment
has therefore been removed from production; the evidence below records why.

## Exact-summary equality lookup: measured and rejected

The experimental unlimited comparison lane used an optional phase/owner-local
digest lookup over live candidate summaries. A hit had to pass full native
`DomainPowerSummary` equality and current live-index membership. Since live
candidates are an inclusion antichain, this identifies the same unique
containing candidate as the original scan, not an arbitrary alternative ID.
Digest collisions, stale entries and optional allocation failure fell back to
the normal search. The cache stored only digest/ID entries, not duplicated
geometry. Retirement dropped lookup entries, never pending obligations or old
exact keys. Finite-cap mode and near-counter-overflow behavior were unchanged.

The experimental `containment_summary_equality_hits` counter measured successful
committed shortcut reuse; speculative hits did not increment it. This was not
a new admission count or a mathematical coverage claim. Independent
implementation/mathematical review passed. The actual-source optimized queue
gate passed 56 tests, with one existing replay diagnostic ignored, including
twelve new tests and the existing full-proposal-stream
comparisons against an independent linear semantic index and prepared workers.
The candidate's full application gates also passed: 400 application-library
tests plus 82 integration tests (482 total), and 72 Python tests. These gates
establish tested correctness, not a performance benefit.

A synthetic complete-proposal benchmark compared the prepatch queue and new
queue against the same unchanged native geometry. Six alternating-order pairs,
batched to avoid sub-millisecond CPU-counter noise, ran on CPU 70, outside the
application build's 74–77 affinity. All returned IDs and admission decisions
match. These are shared-host observations, not campaign speedups:

| Synthetic stream | Equality hits | Median wall before / after | Paired speedup |
| --- | ---: | ---: | ---: |
| Large mixed set, 12,928 proposals | 7,680 | 40.216 / 23.817 ms | 1.688x |
| Same size, strict subsets instead of equal descriptions | 0 | 40.167 / 42.416 ms | 0.947x |
| Small mixed set, 808 proposals | 480 | 0.717 / 0.877 ms | 0.817x |

The negative controls matter: hashing adds cost for small sets or rare equality.
The production decision therefore used the instrumented full-owner prefix
test, not just the favorable synthetic case. The synthetic timing includes
fresh queue admission, input cloning, destruction and intra-batch identity checks; it
excludes input construction and cross-version postchecks. These timing streams
do not exercise retirement, which is covered by separate correctness tests.
Evidence: `TMP/equality-queue-audit.Hv3jrC/RESULTS.md` and
`compare-isolated.log`. Integration evidence is under
`TMP/summary-equality-gate.eIgC5f/`.

The real comparison used the identical 67-owner input, ordering, work budgets
and CPU 0–49 affinity. Both binaries were actual release CLI builds with
opt-level 3 and LTO off. Compilation was outside the timing boundary. The
first 60,000 completed records match in **every field except the top-level
per-inspection `seconds`**; their complete non-timing payload hash is
`c4f051665c8910ccd86bcb3827d043b1d01b814bff4953f54a20dab21323246a`.

| Matched first-60,000-record boundary | Baseline | Equality experiment |
| --- | ---: | ---: |
| Traversal-time bracket, preparation excluded | 251.3576–252.3632 s | 275.7406–276.7462 s |
| Sampled CPU envelope near boundary, preparation included | 1,007.51–1,050.48 CPU-s | 1,090.90–1,123.33 CPU-s |
| Peak sampled process-tree RSS through the envelope | 9.7656 GB | 9.7557 GB |

The candidate was **9.3–10.1% slower** over this exact common prefix. The
brackets include heartbeat age; CPU/RSS samples bracket nearby observations
rather than an exactly synchronized record boundary. This was one observational
shared-host pair, not a repeated controlled scaling study. The candidate was
cooperatively stopped after the comparison point; neither run completed the
five-loop workload.

Near that boundary, only about **0.39% of summary constructions** produced a
committed equality shortcut (31,649–31,661 hits for 8.048–8.109 million summary
builds). This counts successful cache shortcuts, not a full equality census:
collisions and optional fallback can send equal summaries through the ordinary
index. Its complement therefore cannot be called a measured strict-inclusion
fraction. The ordinary semantic-hit counter is also a different population.

Decision: remove the production experiment and its extra telemetry rather than
retain a measured regression. Its source/tests are preserved only under the
ignored `TMP/equality-queue-audit.Hv3jrC/rejected-experiment/` directory; the
standalone comparison in the parent evidence directory now uses that archive.
The complete
matched audit is
`TMP/five-loop-utilization-readonly.vV9E5N/MATCHED_SUMMARY_EQUALITY.md`.
These negative results do not prove hashing is universally unhelpful, but they
do not justify enabling this implementation on the target workload.

## Pending-obligation transfer: initial protocol investigation

This subsection records the initial proposal. The production implementation
and its subsequent release gates are described below.

Under the default inspect-all policy, lookup retirement leaves every admitted
inspection obligation alive.
The final baseline had 5,427,770 retired lookup candidates but only 2,522,398
processed IDs, including its partial cancellation. Thus **at least 2,905,372**
retired IDs were still among the 6,029,467 pending obligations (48.19%). This
is a counting lower bound, not measured avoidable CPU time or proof that those
IDs were undispatched.

A prospective opt-in scheduler policy could transfer an untouched obligation
`A` to a later containing obligation `B`, for the same owner, phase and frozen
rule snapshot, using existing exact native containment. It must retain `A`'s
raw identity/provenance, explicitly require `B` to finish, and distinguish
delegation from actual inspection. Forward-ID aliases are acyclic, but that
does not prove descending IBP reductions or rule closure. Started work, failures
and partial output cannot simply be discarded.

Worker-count-independent decisions require a fixed logical dispatch fence and
sticky reservations/aliases, not a timing-dependent test for whichever job is
currently undispatched. New report semantics would be necessary: skipping an
inspection intentionally cannot preserve the previous complete event prefix.
The design and required tests are in the ignored
`TMP/parent-partition-design.xkm6A5/RETIREMENT_TRANSFER.md`. **No production
transfer policy was enabled at that prototype checkpoint, and no performance
gain was claimed.** It would optimize
the local domain worklist, not replace missing finite-root admission or closure
requirements.

The isolated optimized Rust protocol prototype passes twelve tests, including
96 successful and 60 failure-prefix comparisons across simulated one/two/six/
fifty-worker schedules. Independent source/mathematical review passes within
the model's scope. These are simulated dispatch orders, not fifty native
threads or a solver benchmark. Fixed logical reservations make delegation
independent of physical worker completion; aliases advance the publication
cursor without being falsely reported as inspected. Forward responsibility
chains resolve only after the representative's successful publication.

The production integration audit identified two obligations absent from that
small model. A native inspection can finish without an error while still
reporting unresolved source/guard frontiers; these must prevent a delegated
obligation from being discharged. Cancellation cleanup and failure accounting
must count actual native jobs separately from cursor movement over aliases.
The existing native asynchronous-failure behavior does not promise identical
failure prefixes across worker counts. All source obligations, successor
publication and final unresolved-frontier checks remain mandatory.

The resulting implementation requirements were to retain default inspect-all behavior and
introduce an explicit opt-in policy with native differential tests before a
timed pilot. Evidence and the integration map are in
`TMP/parent-partition-design.xkm6A5/TRANSFER_LEDGER_RESULTS.md` and
`TRANSFER_PRODUCTION_INTEGRATION.md`. A reduction in retired pending IDs is
not yet a measured reduction in campaign time or an exhaustive five-loop result.

There is still no evidence-backed five-loop completion ETA. Dividing pending
regions by processed regions per second is invalid while the pending set grows;
extrapolating seven heterogeneous roots to 67 is also unjustified. Even a
completed symbolic walk would not by itself implement the explicit finite-root
admission, reachable-witness feedback and final closure contract described in
[the active plan](../finite_starting_domains.md). Finite-root admission is now
integrated and release-tested as a separate implementation track. Its repeated
67-root control matches every non-timing graph counter, with 4.6687s traversal,
110.9826s application time and 116.0519s supervisor time. These are the original
rank-zero roots, not exhaustive R15 coverage. The full finite starting envelope
remains unfinished; neither this control nor the protocol tests provide a
five-loop completion-time forecast.

## Connected delegation and entry witnesses: release-gated pilot

Opt-in `--transfer-unreserved-lookahead 256` now enables a separate delegation
ledger with a deterministic logical reservation fence. Exact same-owner/phase
containment can transfer an untouched pending obligation to a later ID; reserved
or started work is never cancelled by containment. The default inspect-all
behavior remains unchanged. Forward-only aliases retain responsibility until
their representative publishes without error or unresolved frontiers. Alias
publication is not native inspection or proof of discharge. Cleanup counts
actual native records independently of the logical publication cursor.

The separate typed entry-witness interface intersects native match pieces with
the original finite starting union, selects admitted concrete points and feeds
only actual traced missing rules to the existing fixed-target search. Successful
points never establish regional coverage. Neither the witness interface nor
delegation changes saved rule provenance or clips intermediate descendants.

Independent implementation/mathematical audits pass. Release gates pass
**3,319 Rust tests (36 ignored) and 73 Python/API/steering tests**. A separate
actual-source gate passes 120 tests (one ignored) against the committed scheduler
without inherited escrow work. That eight-CPU compatibility gate exercises
serial/two/six-worker controls, not fifty workers. The full application gate
exercises the fifty-worker control at lookahead one: protocol correctness,
not a throughput result. No new CAS primitive is introduced.

The frozen release binary is
`TMP/witness-delegation-gate.9qXc3m/rustred`, SHA-256
`ce792425c3fa96c19df5c0d774cf27a6b998a44be7150d298c5b6552ec31fbcc`.
It includes the same inherited escrow work as the baseline; clean-source
compatibility does not make this a benchmark of an isolated commit.
The full-67-owner diagnostic is now running as
`TMP/witness-delegation-gate.9qXc3m/shared-owner-campaign.zx0rs62j/`, with the
same saved inputs, rules, routes, per-query budgets and aggregate diagnostic
allowances. CPU affinity is 0–49, with 50 configured compute slots and the
450/500 GB memory policy. There is no elapsed deadline or concurrent owned
build. It reuses saved IBPs and performs no terminal evaluation.

Delegation changes the native inspection stream, so equal job IDs or counts
are not a matched-work speedup boundary. Report native publications, transferred
obligations and their remaining native work separately. In particular, for
admitted A, transferred T, native-published N and alias-published P,
`pending_native=A-T-N` and `queued=A-N-P`. A shrinking logical queue caused by
alias publication alone is not evidence that native work is draining.
Actual CPU-time deltas and pending-native growth will determine whether a
longer attempt is justified. At launch, there is no new five-loop performance,
closure or completion-time claim.

## Live delegation pilot: frozen slowdown observations

The observations in this section are frozen at native heartbeat
**2,074.852853168 s** on September 23, from
`TMP/witness-delegation-gate.9qXc3m/shared-owner-campaign.zx0rs62j/`.
The run was **still live** at this boundary; these are not final results.
Independent read-only evidence is retained in
`TMP/witness-delegation-gate.9qXc3m/INDEPENDENT_SLOWDOWN_AUDIT_2000S.md`.
No build, profiling attachment, process interruption or solver restart was
performed for that audit.

All 67 initial regions had passed local inspection. The frozen counters were:

| Quantity | Observed value |
|---|---:|
| Historical admitted obligations A | 7,044,946 |
| Transferred obligations T | 3,772,729 |
| Native publications N | 1,325,896 |
| Alias publications P | 1,253,309 |
| Logical publications N+P | 2,579,205 |
| Pending native obligations A-T-N | 1,946,321 |
| Observed frontiers | 0 |
| Committed events | 88,516,559 |
| Charged containment comparisons | 81,920,164,582 |
| Native heartbeat RSS | 27,380,068,352 bytes (27.38 GB) |

There was no reported worker failure or resource-limit stop, and maximum
scheduled finite rank had reached 18. Intermediate ranks are not clipped to the
entry rank. A transfer is still a responsibility; publishing its alias does
not inspect it or prove that its representative has completed. Historical
admissions, pending native work and logical queue length must remain separate.
Zero observed frontiers concern the inspected prefix, not the unprocessed
domain or absence of missing rules everywhere.

The post-root workload became substantially more expensive. Three complete
approximately two-minute windows show the change:

| Native heartbeat interval (s) | Native publications/s | Apply publications/s | Pending-native change/s | Actual busy CPU cores |
|---|---:|---:|---:|---:|
| 1,560.640–1,679.369 | 62.01 | 12.37 | +101.11 | 3.63 |
| 1,680.374–1,799.106 | 47.98 | 10.12 | +137.89 | 3.48 |
| 1,800.112–1,919.885 | 28.50 | 5.27 | +88.69 | 4.56 |

These CPU figures sum thread CPU deltas over sampler intervals; assigning
samples to heartbeat windows permits approximately two seconds of edge
misalignment. They are actual usage, not the 50 reserved compute slots.
Inspectors used about 1.33–1.40 cores, lookup helpers 2.02–3.07 and the
coordinator only 0.086–0.104 in these windows. The first post-root window had
1,653 native publications/s and about 5.02 busy cores, but its different
domain costs and phase mix prohibit a like-for-like speedup/regression claim.

The newer evidence changes the bottleneck diagnosis: **repeated expensive
Apply-domain streaming with ordered head-of-line waiting is now prominent**,
alongside substantial admission lookup work when chunks arrive. It is not
adequately explained by a saturated serial commit thread. Preparation occupied
15.82–21.39% of wall time and ordered commit 8.48–9.51%. The remaining roughly
70–76% includes waiting, polling, dispatch and reporting; it cannot all be
assigned to one operation without more direct timing.

For example, Apply ID 2,577,768, owner `011101110111000`, stayed at the publisher
for at least 26.17 sampled seconds (1,888.684–1,914.855 s), admitting only 130
new domains while committing 220,283 events and charging 439,063,137 containment
comparisons. ID 2,578,058 similarly streamed 148,818 events over at least
23.14 seconds without a new admission. The cursor subsequently advanced:
these are expensive progressing jobs, not evidence of deadlock. Owner labels
alone do not identify their complete coordinate/rank/power-bounded workload.

At 1,895.733 s, 140 finished jobs awaited publication and 11 inspectors were
active, ten of them backpressured. Other samples had only two active inspectors.
The logical H256 fence, aliases within that window and ordered publication can
leave fewer dispatchable native jobs than physical cores. Raising H might help
overlap, but also protects more eventually redundant work from transfer; it is
not an automatic utilization fix. The current run has not been changed.

Containment counts are charged comparisons, not an instruction profile.
Reverse maintenance preflights its comparison allowance; speculative lookup
counts overlap committed counts and must not be added to them. Likewise,
native-operation/predicate totals are credited when an inspection returns,
so they do not locate the active unfinished job's inner-loop cost.

Pending native work grew in all three complete windows. A nearly flat short
fragment near the frozen endpoint is not sustained drain. There is therefore
**no evidence-backed completion ETA, fifteen-hour success forecast or closure
claim**. Memory remains far below the configured 500 GB ceiling; the present
problem is useful throughput and repeated work, not demonstrated RAM exhaustion.

The isolated containment measurement harness under
`TMP/parent-partition-design.xkm6A5/containment-measurement/` is prepared and
independently source/mathematically reviewed, but **unbuilt and unrun** at this
checkpoint. It proposes comparing aggregate and coordinate-block filters
against frozen native containment on bounded saved-descriptor prefixes, with
every representative/retirement decision checked. Such prefixes omit reused
proposals and cannot establish a campaign speedup. Given the newer Apply-job
evidence, this is a measurement option, not a decision that a containment-only
optimization will solve the slowdown. No new index, CAS algorithm or rule
generation was introduced by this documentation update.

## Delegation pilot terminal outcome

The same H256 diagnostic subsequently stopped cooperatively for targeted
optimization. The operator stop requested examination of repeated heavy Apply
regions and ordered-publication waiting; it was not an elapsed deadline,
resource-limit failure or observed missing-rule diagnosis. The supervisor and
native process both exited, and the read-only thread sampler finished normally.

| Final quantity | Value |
|---|---:|
| Historical admitted obligations | 7,077,607 |
| Complete native inspections | 1,335,458 |
| Cancelled partial native inspections | 1 |
| Alias publications | 1,261,941 |
| Transferred obligations | 3,783,574 |
| Remaining native obligations | 1,958,574 |
| Remaining logical queue | 4,480,207 |
| Observed frontiers | 0 |
| Preparation timer | 104.041 s |
| Reported traversal timer | 2,530.416 s |
| Application timer | 2,634.456 s |
| Whole supervisor time | 2,690.417 s |
| Sampled peak process-tree RSS, including shutdown/reporting | 40.535 GB |

The reported traversal timer includes final ledger resolution and result
construction; it is not pure worker CPU time. The application timer ends before
CLI JSON serialization and supervisor reaping.

The cancelled partial is stop fallout, not a failed IBP. The final local
delegation ledger resolves 917,044 transferred obligations, leaves 2,866,521
pending, and blocks nine on cancellation. Its native and alias counts must not
be combined into a claim of closed physical regions. The saved report explicitly
states `recursive_worklist_exhausted=false` and `family_closure_claim=false`;
the supervisor reports exit 4, `hard_stopped=false`, and `work_checkpoint=false`.
This preserves diagnostic evidence, not a resumable worklist.

Completed expensive-region records show substantial matching and RHS work. For
example, ID 2,577,768 visits 938,040 matching cells, 323,066 predicates, 3,238
selected pieces and 298,250 RHS terms. Its native wall timer includes producer
backpressure and must not be reported as an isolated algebra benchmark.

The next bounded measurement calls the existing native application visitor on
those exact original inputs and on their exact residual outside an initial
region. For an initial lower-D bound d, test the partition `D>=d` / `D<=d-1`;
only the existing native summary can establish that the upper slice is
contained in the initial region. This is input-derived and topology-generic.
Any later implementation must preserve the original initial inspections and
their dependency obligations, avoid self-reuse/delegation cycles, and report
partial reuse honestly. No performance gain or production implementation of
this proposal is claimed here.

## Follow-up: isolated containment-index measurements

After the full diagnostic stopped, the prepared index experiment completed
four alternating-order passes over 100,000 saved descriptors, with eight modes
per pass. All 32 measurements reproduced every representative ID, admission
decision and retirement set from the frozen production queue. An independent
audit recomputed the timings and checked the native-containment boundary.

| Diagnostic model | Median CPU time |
|---|---:|
| Current three aggregate keys | 3.692 s |
| Six aggregate keys | 2.760 s |
| Three keys with 32-ID coordinate-envelope blocks | 0.943 s |
| Six keys with 32-ID coordinate-envelope blocks | 1.031 s |

The three-key/block32 model reduces native inclusion calls from 191,666,431
to 26,289,307. Peak whole-process RSS was 480.43 MiB, including input, reference
replay and correctness checks. The optimized diagnostic ran on CPU70; four
measurements per mode are not a confidence-bound performance study.

This is **not a production implementation or a campaign speedup**. All
100,000 replayed descriptors were newly admitted; the input omits reused
requests, which are common in the expensive late Apply stream. Timings also
exclude the production maintenance-preflight group pass and parallel
preparation/publication. The results identify a promising secondary lookup
optimization, not evidence that it alone will make 50 cores useful or finish
the five-loop campaign. Full receipts and limitations are in
`TMP/parent-partition-design.xkm6A5/containment-measurement/PAIRED_100K_RESULTS.md`.

The primary next measurement is exact-overlap removal using the unchanged
native Apply API. The first completed case is recorded below.

## Follow-up: first native original/residual comparison

The optimized external driver under `TMP/heavy-apply-native.0BiQ3d/` loads the
same 67 saved owners once and calls the existing public native Apply visitor
with the saved limits. It introduces no new application, algebra or rule-search
implementation. Its source and exact partition passed independent review.
The first case, ID 2,577,768, completed twice on CPU0 with all compute pools
limited to one. Compilation, loading, route verification and queue publication
are outside the visitor timing; a lightweight geometric-event fingerprint is
inside it. The host is shared, not an isolated benchmark machine.

For this query, the exact initial-region containment check proves that the
slice `D>=9` lies in initial anchor 66. Retaining that anchor and all of its
dependencies leaves only `D=8` for new inspection. All original coordinate,
rank and positive-power constraints remain in both halves. The split threshold
comes from the input, not the owner label or loop count.

| Native input | First wall / CPU (s) | Repeat wall / CPU (s) |
|---|---:|---:|
| Original region | 27.309 / 27.088 | 27.408 / 27.211 |
| Exact residual D band | 2.986 / 2.964 | 2.990 / 2.968 |

Both original runs reproduce the saved native counters exactly. Each input's
repeat reproduces its own counters and ordered geometric-event fingerprint.
There are no local problems or observed support-activation events in this
case. Optional coefficient-classification refusals remain recorded; they are
not suppressed by the driver. Original and residual outputs are deliberately
different because they describe different domains.

The original visits 938,040 matching cells and 298,250 RHS terms; the residual
visits 548,060 and 132,312 respectively. Successor events decrease from 272,747
to 109,749. Loading took 14.964 s separately, and the process high-water RSS was
5,819,672 KiB including the all-owner load.

This is approximately **9.1x less local inspection time**, conditional on
retaining the anchor work. It is not a five-loop campaign speedup, a closure
claim or a matched full-workload replacement. Runs use original-then-residual
order in one process, with possible cache/order effects; the warmed repeat
supports the local direction but does not remove all measurement caveats.
Receipts are `first-heavy.jsonl` and `first-heavy.stderr` in the driver
directory; the driver exited successfully.

### Remaining saved heavy regions

The other three regions subsequently completed in one invocation, with a
separate 14.583 s all-owner load and the same one-CPU visitor boundary:

| Saved region ID | Residual D | Original wall, two repeats (s) | Residual wall, two repeats (s) |
|---|---|---:|---:|
| 2,514,890 | 8 | 19.131 / 21.564 | 1.011 / 1.013 |
| 2,557,621 | 8 | 20.076 / 21.995 | 1.027 / 1.008 |
| 2,578,058 | 6–8 | 22.960 / 22.226 | 0.951 / 0.948 |

Together these are 16 completed inspections: four original regions and their
residuals, each twice. Every original matches its saved native statistics;
every same-input repeat matches its own statistics and event fingerprint.
All have zero local problems and zero observed support-activation events.
Both benchmark processes exited successfully, and independent receipt audits
pass for all four cases. Full wall/CPU/work counters remain in the JSONL
receipts, including `remaining-heavy.jsonl` for the latter three cases.

Observed local time reductions range from approximately 9x to 24x. This does
not establish how often the optimization applies in the full campaign, its
queue impact, or improved core utilization. No full run with this optimization
has completed. The measured direction justifies a narrow production slice:
default-off initial D-band reuse alongside the existing transfer policy,
protected initial obligations, one unchanged residual visitor, and explicit
partial-scope/dependency accounting. The subsequent implementation and
independent gates are recorded below, before the renewed 50-worker full campaign.

## Parallel coefficient-path source check

A separate read-only audit traced the current native Apply restriction and
guard-classification path into the pinned Symbolica 3.0 checkout. The inspector
runs directly on an ordinary Rust worker thread. Polynomial substitution,
rational-polynomial normalization, GCD, factorization and base-coefficient
splitting on this path are synchronous; they do not dispatch through Symbolica's
explicit parallel term-mapping or sparse back-substitution APIs. Consequently,
`RAYON_NUM_THREADS=1` does not funnel these independent Apply inspections through
one global CAS worker. The successful licensed check is an atomic fast path,
not a worker-serialization queue.

Relevant entry points are `walking/parallel.rs`, `owners/domains/applied/`
and `algebra/indexed/{specialization,base_coefficients}.rs`; the traced native
implementations are Symbolica's polynomial substitution, rational-polynomial,
GCD and factorization services. This is source evidence for the current path,
not a scaling benchmark or a claim that allocator contention, memory bandwidth
or scheduler stalls are absent. No CAS implementation or thread-pool policy
was changed as part of this audit.

## Integrated initial-domain overlap reuse

The opt-in `reuse_initial_d_bands` Rust request field and
`--reuse-initial-d-bands` CLI/Python-steering flag now implement the measured
optimization. Defaults remain unchanged. The policy requires recursive walking
with `--transfer-unreserved-lookahead` and unlimited aggregate containment.
It uses no new CAS operation and recognizes neither topology names nor loop
counts.

For a successor region Q and a same-owner initial region C, the native domain
summary supplies C's finite minimum D=d. When both pieces are nonempty and
native containment proves `Q intersect D>=d` lies in C, the worker inspects
only `Q intersect D<=d-1`. All original coordinate, A, R and D constraints
remain. The original initial region is protected from responsibility transfer;
its inspection and all descendants remain required. Final resolution combines
the residual result with that initial region's actual status. A pending,
failed, cancelled or frontier-bearing initial region cannot discharge the
overlap. Optional-index limits or failed applicability checks fall back to the
unchanged full inspection.

This is partial reuse, not a whole-region inspection or a new terminal. Both
published and uncommitted partial reports retain original and residual scope.
TTY and plain progress distinguish native jobs, aliases and partial initial
reuse. The implementation leaves the existing exact coefficient path and all
per-job limits in place.

Independent implementation/mathematical review passes. The clean-owned release
compatibility gate passes **135 tests, one existing diagnostic ignored**, in
6.27 s. It excludes the preserved unrelated completed-result escrow work.
The first full shared-tree gate passed 454 application unit tests and 66
integration tests before finding a new test's incorrect cancellation-status
expectation. Recursive walking already reports `incomplete` with a cancellation
reason, unlike the match-only status string. The test was corrected to check
that contract and additionally require zero admitted/processed work and no
resolution claim; production cancellation behavior was unchanged. The full
rerun passes **536 application/CLI tests**, one existing diagnostic ignored,
and **74 Python/API/steering tests** (50 public API, 12 matcher-steering,
12 supervisor tests). The coordinated gate exits successfully. Receipts and
independent audit are under
`TMP/initial-overlap-gate.4Tj82V/`.

The newly gated shared-tree release CLI is
`TMP/initial-overlap-gate.4Tj82V/rustred`, SHA-256
`3be828134ca05f6d08222656277841c0cfcfb2d6ff8d06a2c38b65d501bca5db`.
Like the preceding campaign binary, it includes preserved unrelated escrow
work; its measurements must not be described as an isolated-commit benchmark.
The additional clean-owned gate verifies that this implementation does not
depend on that work. The two starting-input hashes above are unchanged.

The renewed all-67-owner run retains the same A24/R15/D9 input, saved rules,
per-query budgets, 50 physical-core affinity and lookahead 256. Lifetime
allowances rise to 100 million historical domains and two billion committed
events; these are incrementally used resource ceilings, not mathematical
restrictions or preallocations. A 300-GB cooperative memory threshold reserves
drain/reporting headroom below the 500-GB hard ceiling. There is no elapsed
timeout. Initial pinning and residual reuse change the work stream, so equal
node counts or IDs must not be used as matched-work speedup denominators.
Actual CPU use, remaining native obligations, queue growth, frontier count and
memory must be measured before forecasting completion. No full campaign with
this policy has yet completed.

### Renewed run launched

Implementation milestone `02f03d63` is pushed to `main`. The full run is now
live, with receipts at
`TMP/initial-overlap-gate.4Tj82V/shared-owner-campaign.alc7oe24/`.
The launch record confirms 50 workers on CPUs 0–49, the above input and
executable hashes, lookahead 256, enabled initial-band reuse, 300/500-GB
cooperative/hard RSS limits, a 480-GB child address-space ceiling and
`hard_timeout=null`. The 15-hour objective is telemetry, not a supervisor
deadline. At launch, no owned build or separate solver experiment was running
concurrently. The later handoff-diagnostic build overlap is recorded separately.

At the initial 68.077-s heartbeat it was still verifying saved routing maps,
with 4,832 of 8,246 preparation items processed. Setup was using approximately
one core and 5.19 GB aggregate RSS; sampled peak was 5.72 GB. These are startup
observations, **not** post-preparation parallel utilization or a completion
forecast. Subsequent monitoring must distinguish native residual inspections,
delegated obligations and worklist growth; no closure is claimed at launch.

### First post-initial-region measurement

The run advanced beyond all 67 initial publications by heartbeat 336.462 s.
An independent, bounded 120-s `/proc` thread sampler subsequently observed the
actual reuse phase. It ran on CPU49 within the configured affinity and exited
normally; it did not stop or restart the campaign. Stable thread identities and
CPU deltas distinguish inspectors, admission helpers and the coordinator.

| Counter | Heartbeat 397.892 s | Heartbeat 517.568 s |
|---|---:|---:|
| Native publications | 136,123 | 279,430 |
| Partial initial-overlap inspections, included above | 12,000 | 26,593 |
| Pending native obligations | 1,244,276 | 1,361,125 |
| Observed frontiers | 0 | 0 |

The native process averaged **4.81 busy cores**: 3.25 in admission helpers,
1.01 in inspection and 0.55 in the coordinator. Native publication throughput
was about 1,197/s, but pending native work grew by about 976/s. One early
30-s interval drained slightly; this did not persist over the complete window.
RSS at the sampler's end was 11.47 GB. Reuse is genuinely active, but these
observations do not demonstrate good 50-core scaling, sustained drainage,
full-run speedup or a completion ETA.

The run remains active to test the later expensive-Apply regime that motivated
the optimization. An additional heartbeat at 642.306 s had 547,249 native
publications, 38,984 partial inspections, 1,463,276 native obligations pending
and zero frontiers; the latest corresponding resource sample was 14.47 GB.
These are frozen live observations, not final results. Full per-thread windows
and counter definitions are recorded in
`TMP/initial-overlap-gate.4Tj82V/INDEPENDENT_POST_ROOT_AUDIT_518S.md`.

An independent scheduler review also confirms why the fixed lookahead can
leave cores idle: it counts logical IDs ahead of the publication cursor, not
only unfinished computations. Completed but unpublished jobs still reserve
their positions. A larger fixed lookahead is already configurable, but can
increase unnecessary inspections by preventing later responsibility transfers;
it is not a guaranteed speedup. Changing the fence to depend on worker finish
times would undermine the present worker-count-independent transfer policy.
No scheduler or source was changed during these observations.

### Narrow runtime handoff check, without repeating queue traversal

The separate runtime-compatibility audit found an economical route for the
eventual successful result. The symbolic visitor permits locally descending
support changes within a saved root, whereas the current concrete tracer
requires the same support or a strict subset. No actual incompatible selected
edge has been observed. A prior conservative all-rule sign scan does report
potential activations, but does not resolve guards, zeros or dispatch priority;
those counts are neither actual failing reductions nor proof of compatibility.

If the full walk completes, its existing final report retains every native
inspection's owner, coordinates, rank, power bounds and complete native stats.
Partial inspections additionally retain their exact residual power bounds and
initial-anchor link. A narrow post-pass can therefore replay only those actual
Apply inputs through the unchanged native visitor, count non-subset support
transitions, and compare native stats. Route and alias records need not be
re-inspected; their original resolved responsibilities remain required. This
avoids repeating queue admission and routing, which dominate current CPU use.

Such a pass must stream the report and use bounded dispatch with one shared
owner load: the previous result is 2.796 GB logically despite its much smaller
filesystem-compressed size. It must require a complete final receipt and
unresolved-free ledger, preserve partial-residual scope, and fail closed on
missing/mismatched records or any unsupported edge. This is a narrow
consumer-compatibility check, not universal certification, a master-minimality
test or coefficient back-substitution. It does not require interrupting the
active run or regenerating saved rules.

A local standalone diagnostic now implements that protocol without production
code, schema or API changes. Independent source and focused-gate audits pass.
All **nine focused tests** pass, including streamed receipt validation, exact
partial-scope/anchor reconstruction, alias/Route count reconciliation, malformed
and incomplete inputs, and actual native K1 full/residual replay. The K1 control
agrees for pools configured with 1, 6 and 50 workers, all pinned to CPU49; that
is configuration parity, not a 50-core scaling measurement. Algebraic
`cancelled_groups` are correctly distinguished from execution cancellation.
No full-five-loop or live-incomplete-report replay has been attempted.

The diagnostic and its receipts remain under
`TMP/finite-support-replay.AjEBHn/`. Its optimized standalone build links the
existing gated native libraries; it did not rebuild or replace the campaign
solver. The two-binary build took 434.62 s and peaked at 1.52 GiB RSS. Tests
took 0.02 s in the harness, 0.03 s whole-command wall time. This controlled
validation used CPU49 and a 16-GiB address-space allowance. Exclude the conservative
overlap window, 12:21:23–12:28:59 UTC (roughly campaign seconds 2285–2740), from clean
utilization comparisons. The campaign stayed live and unchanged throughout.
The diagnostic is ready for an eventual genuinely completed receipt; its small
controls establish neither five-loop compatibility nor closure.

The accompanying read-only packaging inventory finds no need for a new format:
retain native owner bundles and routing witnesses, relocate only the required
owner paths relative to `--owner-base`, and preserve the exact existing
`--entry-domains` union. Existing per-bundle `CandidateReducer::terminals()` can
export the declared finite terminal union without search or minimization. A
fresh-load concrete canary follows completion; neither packaging nor that
canary substitutes for exhausting the original regions. The inventory is
`TMP/finite-closing-handoff-audit.9S1OaK/DELIVERABLE_INVENTORY.md`.

### Later live measurements: admission throughput, not a new rule gap

Two additional independent 120-second thread samples keep the same campaign,
binary and inputs. Both observers exited normally without stopping the solver.
The later observer finished by 12:19:03 UTC, before the separately authorized
standalone handoff-diagnostic build. These are different traversal intervals,
not matched-work scaling comparisons.

| Native heartbeat interval | Mean busy cores | Native publications/s | Pending native change/s | Ending RSS |
|---|---:|---:|---:|---:|
| 1170.454–1290.183 s | 4.26 | 254 | +466 | 26.36 GB |
| 2012.450–2133.150 s | 5.33 | 770 | +1,446 | 32.94 GB |

At the latter boundary, 1,643,766 native regions had been published, including
95,637 partial initial-overlap inspections; 2,523,090 native obligations
remained pending. There were zero observed frontiers and maximum descendant
rank was 18. The original A24/R15/D9 entry domain never clips those descendants.
Historical admissions, transfers and publications still satisfy the ledger
identities; neither aliases nor partial reuse are additional native inspections.

In the later window, admission helpers used 3.91 cores, inspectors 0.91 and the
coordinator 0.50. The 121 heartbeat samples had no repeated current publisher
ID, so this particular interval does not exhibit the older run's prolonged
same-publisher Apply stalls. Finished-unpublished jobs nevertheless reached
255. This supports a remaining admission/publication throughput bottleneck,
not the claim that every interval is dominated by one expensive symbolic job.
Snapshots alone cannot distinguish logical-horizon holes from all other
dispatch limits: the H256 window counts delegated IDs as well as native work.

Continue the unchanged full attempt: it is making real progress, no algebraic
frontier has appeared, and memory remains far below its configured envelope.
All four 30-second subwindows in the later sample grew pending native work,
so no completion percentage or under-ten-hour forecast is justified. There is
no automatic thirty-minute cutoff. The native-counted lookahead alternative
has a read-only design audit, but is neither implemented nor benchmarked; do
not attribute an unmeasured scheduling improvement to this run.

Detailed frozen receipts are
`TMP/initial-overlap-gate.4Tj82V/INDEPENDENT_LATE_AUDIT_1290S.md` and
`INDEPENDENT_LATE_AUDIT_2133S.md` beside it. These observations establish neither
finite-domain exhaustion nor a new closing five-loop artifact.
