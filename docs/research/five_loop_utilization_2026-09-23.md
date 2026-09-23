# Five-loop full-owner utilization investigation — September 23

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
deadline. No owned build or separate solver experiment overlaps this run.

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
missing/mismatched records or any unsupported edge. This is a proposed narrow
consumer-compatibility check, not implemented or passed yet, and not universal
certification, a master-minimality test or coefficient back-substitution.
It does not require interrupting the active run or regenerating saved rules.
