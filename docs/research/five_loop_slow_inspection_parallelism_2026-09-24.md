# Five-loop slow inspection: parallelism and repeated work

24–25 September 2026. Technical study after the refreshed Vakint numerical gates
and the completed four-loop controls. The native probes below do not change the
live campaign or its saved rules. The five-loop workload is still incomplete;
these observations provide neither a closure claim nor an ETA. The complementary
[independent scheduler audit](five_loop_parallel_publication_audit_2026-09-24.md)
and [experimental Ready implementation](five_loop_ready_publication_2026-09-24.md)
cover publication, responsibility and checkpoint constraints. Ordered remains
the default and the live policy.

## Current evidence

All 134 initial obligations finished earlier. The expensive work observed now
is successor Apply inspection, repeatedly involving owner `011101110111000`.
At approximately 81.6 minutes, the run had 2.72 million completed native
inspections, 2.72 million pending logical obligations, zero reported frontiers,
about 50.5 GB RSS and approximately one sampled busy core. Those counts are
changing; they are not equally expensive independent tasks or a completion
fraction. The maximum observed finite rank had reached 19, beyond entry R15.

The earlier 20-second profile at 23 minutes had one inspector accounting for
65.16% of sampled user CPU, admission helpers 30.46%, and coordinator 4.38%.
Its unchanged head emitted approximately 460,000 successors while 170 finished
jobs waited for publication. It was producing work, not stuck.

One additional root-authorized 20-second, 49 Hz user-CPU profile was recorded
at approximately 84.6–85.0 minutes. PID/start/boot/executable identity was
verified before and after; all native threads remained on CPUs0–49 and only
the recorder used CPU62. It produced a 7.176 MB trace with zero lost samples.
This is an instrumented observation, not an isolated timing measurement.

| Exclusive sampled user CPU | Share |
|---|---:|
| Inspector `owner-domain-1` | 43.21% |
| Inspector `owner-domain-6` | 17.15% |
| Admission helpers | 36.41% |
| Coordinator | 3.23% |

During the bracketing native heartbeats the head advanced from logical ID
6,860,693 to 6,860,879, both that owner with Amax=Dmax=25. There were 29 native
and 157 delegated publications, 550,322 accepted events, 543,880 successors and
1,025 newly scheduled obligations. Backpressured workers fell from six to one;
finished held inspections rose from 92 to 122. This window spans multiple
calls, so its CPU cannot honestly be assigned to one specific domain.

Leading exclusive leaves included helper containment summaries/index lookup
(10.29% and 8.44% overall), inspector allocation (5.54% in the leading inspector),
polynomial validation (3.76%), and Symbolica polynomial sorting (3.17%). The
profile does not establish that sorting RHS ordinals is a hotspot: the visible
sorting symbols operate on polynomials. Raw captures and bracketing records are
under `TMP/five-loop-heavy-profile.AmExwp/`; the earlier recording is under
`TMP/five-loop-coarse-live-profile.d8QXjU/`.

Inclusive postprocessing of the same trace recovers both application and
matching ancestors: leading-inspector `apply_group` 16.95% of overall samples,
fixed-index specialization 11.35%, matching guard resolution 8.84%, and base
coefficient-system construction 5.94%. RHS ordinal sorting appears at only
0.13%. These percentages overlap and are not a phase decomposition. In
particular, `apply_piece` is recovered on fewer stacks than its descendant
`apply_group`, exposing bounded-stack/unwinding loss. Optimized generic symbol
merging also names inactive instantiations. Thus the trace supports both kinds
of expensive work, not an Amdahl speedup bound or exact matching/application
fraction. No additional recording was needed for this check.

## Later live observation: the four-million-inspection plateau

A further identity-checked 20-second capture at 23:26:46–23:27:07 UTC used the
same 49 Hz/user-CPU settings, disjoint recorder CPU62, and unchanged live
executable on CPUs0–49. Zero samples were lost. Evidence is retained in
`TMP/five-loop-queue-profile.fIBC46/`; this is profiling, not a benchmark.

| Exclusive sampled user CPU | Share |
|---|---:|
| Inspector `owner-domain-9` | 63.69% |
| Admission helpers | 33.83% |
| Coordinator | 2.49% |

The bracketing native heartbeats span 21.15 seconds. Head ID8,897,540 remains
owner `011101110111000`, Amax=Dmax=12, with no lower D bound. Native completions
remain 4,001,281 and pending logical obligations 4,940,196. Nevertheless, the
head publishes 331,235 additional events and 330,561 successors, including
6,139 conditional successors. No additional obligation is scheduled in this
window: newly emitted destinations find existing responsibility. There are
159 finished inspections held for publication, two occupied native slots and
one backpressured worker. This is useful work with costly repeated checking,
not evidence of a hung process or of queue exhaustion.

Containment checks increase by 105,117,535 and summary constructions by
330,175 during those heartbeats. Leading exclusive leaves are containment
index lookup (14.32% of all sampled user CPU), the summary containment predicate
(8.34%),
allocation in the inspector (7.20%), Symbolica polynomial sorting (5.99%),
RustRed polynomial validation (4.64%), and admission scheduler epoch handling
(4.03%). Inclusive stacks include application groups (26.77%), fixed-index
coefficient specialization (19.91%), and matching guard resolution (15.74%).
These inclusive percentages overlap; bounded-stack unwinding and optimized
symbol merging prevent interpreting them as a phase-time decomposition.

The live supervisor reports approximately 66.8 GB RSS. Its fourth checkpoint
is 9.02 GB and took 89.98 seconds, versus 55.89 seconds for the previous save.
The profiled window did not overlap either write. Descendant ranks have
reached 20 even though the entry envelope has R<=15; no descendant is clipped.

The immediate improvement candidate remains fair ready-ticket publication:
drain completed and partially ready sources, release actual outstanding-work
credits, and refill while a slow earlier source continues. It cannot by itself
remove the containment or coefficient-specialization cost. Later candidates
should measure repeated identical specialization inputs and containment
summary reuse before adding any cache; reuse existing Symbolica operations,
respect immutable context identity, and bound retained memory. The observed
helper scheduling overhead also warrants measuring preparation batch size
against lookup work, rather than assigning all spare threads to tiny batches.
None of these observations establishes a five-loop completion ETA.

### Follow-up Symbolica/API inspection (no implementation yet)

An independent read-only source review identifies three narrow hypotheses to
measure separately from the scheduler change:

1. `algebra/indexed/specialization.rs::execute_fixed_polynomial` deep-clones the
   source before Symbolica's first allocating replacement. Borrow until the
   first actual replacement, retaining the clone for identity substitution;
   preserve preflight, zero-first/descending order and output validation.
2. `owners/domains/applied/geometry.rs::fixed` already emits sorted unique axes,
   while specialization repeatedly canonicalizes those assignments and builds
   numerator/denominator assignment vectors. Consider a small private prepared
   assignment scoped to the same indexed context and source cell/group. Retain
   coefficient-dependent bounds and validation of untrusted public inputs.
3. `owners/domains/applied/algebra.rs` re-authenticates fresh checked results
   before numerator classification. Investigate reusing the existing
   `bind_sealed` / `numerator_condition_from_bound` interface only where the
   previous validation proves the same limits. Do not weaken guard limits,
   optional refusal semantics, cancellation or native-operation accounting.

The relevant public Symbolica API is in `vendor/symbolica/src/poly/polynomial.rs`.
`replace` already selects `replace_last` where legal. `replace_all` yields a
scalar and `replace_except` retains one variable; neither is a drop-in arbitrary
subset specialization replacement. Reusable last-variable workspaces are
private in this checkout. No independent polynomial substitution or CAS kernel
is proposed. These are unmeasured hypotheses, not a claimed speedup, and the
inclusive specialization share cannot be added to its own child costs.

## What existing parallelism can and cannot do

The public
`CandidateOwnerPrograms::visit_power_bounded_owner_applied_successors` composes
priority matching and application in one serial callback chain
(`owners/domains/applied/engine.rs`). For each selected piece it sorts original
RHS indices by shift, keeps equal-shift groups intact, visits sign/boundary
cells, specializes coefficients, checks original nonzero/source conditions and
descent, coalesces equal-shift contributions, then constructs exact images.
The immutable owner programs are already shared among inspector threads.

The existing physical-part implementation has the right parent-responsibility
model, but `State::parts` only admits IDs below `initial_domain_count`.
Therefore enabling its flags would not split any of the observed descendant
heads. It also requires an explicit finite upper bound on the chosen local
axis. A user-specified axis/cut is not an automatic cost-balanced partition.
The finite upper bound is only this implementation's restriction: an unbounded
integer axis also partitions exactly at a representable cut `c` with
representable `c+1`, into `[lower,c]` and `[c+1,infinity)`, retaining every
coupled predicate. Extending that case still needs explicit API/tests and is
not already supported by the current physical-part option.

Its earlier native-only two-part experiment achieved 1.51x median speedup on
one different 11.5-second initial domain, with almost unchanged work. The later
completed recursive controls were 1.58% slower in median traversal with splitting
than without, and lost two of three rotations. Neither result answers how an
exact partition of the present costly descendant behaves. More workers or a
larger reservation window alone cannot split an individual native call.

## Exact source and completed native controls

First recover an **actual recorded domain**, not a box inferred from an owner
mask or the reported A/D bounds. Current progress events omit lower/upper/rank.
Generation 3 is an immutable 6.46 GB checkpoint containing the exact queue;
its saved head is ID6,555,873. It also contains earlier profiled ID3,889,485
and the later sampled IDs6,860,693/6,860,879. A bounded-memory framing/JSON scan
can select those records without restoring the solver or reading coefficients.
Record checkpoint generation, hash, file identity and sampling timestamps.
After the separate fixed Vakint timing matrix finished, that scan completed in
46.75 seconds on low-priority CPU50, with approximately 309 MiB peak RSS. All
6,462,875,246 raw bytes were framed and SHA256-hashed, and file identity/size/time
remained unchanged. The selected queue prefix was JSON-parsed; the remainder
was framed/hashed, not fully deserialized. The manifest's BLAKE3 digest was
recorded, not independently verified. This is diagnostic extraction, not a
restored-state or coverage certificate. Receipts are in
`TMP/five-loop-head-extract.3oCC23/`.

The unchanged head from the earlier 23-minute window, ID3,889,485, has all
lower bounds zero, upper bounds
`[16,1,0,0,16,0,0,0,16,0,0,1,16,16,16]`, rank16, Amax11,
no D minimum and Dmax11. Its six inactive coordinates share the rank bound;
the two varying active coordinates each have offsets 0 or 1. There are 298,452
concrete tuples. The chosen first experiment splits axis0 at **1**, giving
143,412 and 155,040 tuples (48.05%/51.95%). This is a count-balancing hypothesis,
not a prediction of native cost. It measures the whole saved parent: the normal
walker may reuse a high-D initial slice before inspecting its residual.

The temporary public-API helper and frozen first-sequence plan are under
`TMP/five-loop-head-split.EcUaJy/`. The sequence is broad, identical parts
serially, then the same parts in parallel, with current cached release
libraries. The first native sequence started at 22:56:05 UTC under its isolated
resource guard (probe PID825297, guard PID825072; binary SHA256
`41c5ecfe…a0cac0d`) and completed before 23:00 UTC. Receipts are in
`first-native-guard/` and `first.result.json`. It did not overlap the next hourly
checkpoint, but the live campaign continued on disjoint cores: this is a
shared-host measurement, not a quiet-host benchmark.

| Completed mode | Native span (s) | Process CPU (s) | Individual part spans (s) |
|---|---:|---:|---|
| Whole parent | 48.867 | 48.39 | 48.866 |
| Same parts serial | 49.309 | 48.81 | 46.131 + 3.178 |
| Same parts parallel | 48.069 | 50.83 | 48.068 / 3.191 |

This cut **did not yield meaningful scaling**. Its 48.05%/51.95% point-count
split became a 93.55%/6.45% serial-time split. The single observed 1.017x wall
ratio is not a statistical speedup. Planned rotations of this imbalanced cut
were held; no successful sample was selected from repeated attempts.

Independent raw-receipt audit passed. Serial and parallel per-part queries, counters and bounded exceptions match
exactly. All modes produce 885,723 successors (28,401 conditional), 1,041
selected pieces, 948,235 term visits and 3,211,650 native operations, with
zero problems and zero in both unsupported-transition counters. All 296
optional-original refusals remain counted. Splitting changes only events +1
(per-call optional provenance), matching cells +2, coordinate cells +60 and
terminal checks +1. Here the obstacle is cost imbalance, not duplicated work.

Loading/preparation took 15.774 seconds. Whole-command wall was 164.60 seconds,
CPU 166.18 seconds, and sampled aggregate peak RSS 5.991 GB; the resource
supervisor reported no stop/error. Neither the original domain nor any
descendant was narrowed to obtain this result. This remains a one-hop native
inspection diagnostic, not a recursive closure or successor-payload certificate.

Broad selected-piece spans (`Classified` through corresponding `RuleFinished`)
total 33.815 seconds; the remaining 15.051 seconds include matching, nonselected
callbacks, preflight and diagnostic overhead, not purely matching. All eight
slowest retained broad spans have axis0 fixed to zero, with varying fixed
axis4 values. Those spans are only about 1.93 seconds of the total, so they
suggest another partition hypothesis rather than a complete cost map. Exact
bounded source-piece geometry is retained for further API/cost analysis.

### Second, cost-informed exploratory cut

The same frozen helper, whole source and three-mode order were used once more,
with axis4 cut at **5**. The eight longest retained application spans divide
0.953/0.975 seconds across that cut, but represent only 5.7% of total selected
time and are selection-biased. This was explicitly a hypothesis, not measured
whole-work balance; its point counts are deliberately uneven (89.27%/10.73%).
The plan was recorded before execution. Neither trial replaces the other.

| Completed mode | Native span (s) | Process CPU (s) | Individual part spans (s) |
|---|---:|---:|---|
| Whole parent | 49.224 | 48.76 | 49.223 |
| Same parts serial | 50.350 | 49.87 | 43.953 + 6.398 |
| Same parts parallel | 43.642 | 49.56 | 43.642 / 6.403 |

This exploratory observation is **1.128x** faster in native wall time with
1.64% more process CPU than its own whole-parent baseline. It still leaves
87.3% of serial part time in one part. It is not a statistical speedup or
evidence of major core scaling; no further cuts or rotations were run.

Here partitioning changes native geometry and work counts: native operations
increase from 3,211,650 to 3,327,578 (+3.61%), term visits from 948,235 to
987,074 (+4.10%), selected pieces from 1,041 to 1,101, and successor descriptors
from 885,723 to 918,016 (+3.65%). Conditional descriptors increase from 28,401
to 30,109, and optional-original refusals from 296 to 330. These are retained
fragmentation/refinement/provenance differences, not silently normalized away.
The exact same two parts agree between serial and parallel on every native
counter and bounded exception. All dispatch gaps/unresolved/invalid sources,
native problems and both unsupported-transition counts remain zero.

All source points remain covered by the exact partition and each public native
call finishes. The diagnostic does **not** retain successor coefficients or
prove equality of complete successor payload unions; nor does it establish
recursive closure. Additional work is acceptable when it buys useful wall
time, but this trial does not justify promoting the current production option.

Preparation took 16.264 seconds. Whole-command wall was 162.05 seconds and
sampled aggregate peak RSS 5.991 GB, with no resource stop/error. The trial
started at 23:03:07 UTC and ended before 23:06 UTC, before the next hourly
checkpoint. Raw evidence is `axis4.result.json`, `axis4.time`, and
`axis4-guard/` in the same temporary directory. Independent raw audit passed:
all non-timing broad fields match the first trial, every serial/parallel part
matches, all aggregate counters reconcile, and no checkpoint overlap occurred.

Both controls drain their two native callback streams independently. The current
Ordered walker instead polls only its current publisher ticket, including
physical part0 before part1. With hundreds of thousands of successors, a later
part can block on bounded chunks long before completion; finished-result escrow
does not rescue an unfinished blocked producer. Thus even a better native cut
would require sound bounded interleaved part admission and persisted per-part
prefixes, or another proven drain design, before claiming production scaling.

The next implementation study should therefore address dynamically distributed,
program-bound matched pieces and/or a ready-ticket coordinator, with explicit
bounded queues, source responsibilities and checkpoint state. Continuing to
choose geometric cuts from eight biased timing extrema is not a demonstrated
general scheduling strategy. Neither implementation is part of these controls.

For a selected actual Apply source, the current public one-hop visitor permits
three matched modes sharing one immutable all-67 owner snapshot: broad source,
identical disjoint parts serially, and those same parts in parallel. Preserve
all coupled rank/A/D predicates, native guard/descent checks, optional refusals,
conditional images and both unsupported-transition counters. Use a bounded
counter/diagnostic sink, explicitly not recursive closure or a coefficient-event
replay certificate. Measure loading separately, every part's wall time, complete
mode span, total native work, CPU and process RSS. Rotate only after feasibility.

The historical helper is not a drop-in: its finite ten-million event/term
allowances predate production `--unbounded-work`. Any controlled adaptation must
use the current unlimited cumulative convention while retaining the same
per-operation/scratch safeguards and one process RAM allowance. Do not quietly
halve `usize::MAX`, reset finite budgets for every child, or stop the broad call
at an arbitrary elapsed threshold and report a speedup.

There is also a parent-overlap subtlety. Ordinary `inspection::inspect` may first
trim a D-band covered by an existing initial anchor; `inspect_part` deliberately
bypasses this to prevent a source part reusing its own pending broad parent.
A descendant integration must plan overlap once, keep the anchor dependency,
and partition that exact residual, or explicitly measure the extra full-parent
work. Blindly applying the current initial-only seam to descendants could
duplicate an already-covered D-band.

## Avoiding duplicated work before changing the scheduler

The cheapest structural candidate is preparing the immutable RHS ordinal order
once. `PreparedRule` currently stores `rhs` but no sorted ordinal permutation;
`apply_piece` allocates and sorts it for every selected piece. The single
production constructor is `preparation/shared.rs`. A prepared permutation or
group-range table could preserve the original term ordinals and exact
`(shift, ordinal)` ordering, leave serialized artifacts unchanged, and eliminate
that repeated allocation/sort without adding algebra. Per-call scratch admission,
shift-group counters and cancellation/error ordering still need deliberate
preservation. Its memory is paid once per prepared rule. The current profile
does not identify this as a material cost, so no speed claim or implementation
is justified yet.

Coefficient preparation is less simple. Each boundary's fixed-index or affine
restriction can change the coefficient; equal-shift additions invalidate the
single-term classification. The current code already avoids repeated final
classification for an unchanged singleton, and skips empty-substitution GCD
while retaining authentication/resource checks. `numerator_condition_with_limits`
and `base_coefficient_system` then validate/extract native polynomial data; the
root-domain decision still depends on source bounds and rank.

Caching only domain-independent data may eventually save work, but an eager
load-time guard computation could reject an unused rule or turn today's optional
per-call refusal into an input-loading failure. A safe proposal needs exact
context/value/restriction/limit identity, bounded retention, unchanged original
guards and honest work counters; it must not cache a domain decision under only
an original coefficient ID. The existing Symbolica-backed specialization,
GCD/factorization and polynomial APIs remain the authority. No new CAS operation
or replacement arithmetic is proposed here.

## A more ambitious no-rematching split

The matcher publicly emits owned, privately constructed `OwnerDomainMatchPiece`
values. But applying a selected piece is currently private, and a piece carries
no binding to a unique program instance. Simply exposing `apply_piece` would
allow owner/batch/rule ordinals from a different prepared program to be reused
incorrectly. A future opaque lifetime-bound work capability could let one
priority matcher emit disjoint selected pieces to bounded worker tasks without
repeating matching. This is an API/design change, not an available CLI mode.

A minimal prototype can use a privately constructed job borrowing its exact
originating `CandidateOwnerPrograms`, with one ordinal for every match-stream
piece, including nonselected pieces. Native callbacks borrow coefficient/source
data and must not cross threads as borrowed events: project them into the
existing owned walker effects while their native data remain valid. An ordered
merger keeps one parent-level source-reuse cache and the query's first-original/
first-coalesced optional-refusal election. Aggregate work must not silently reset
per piece; an initial prototype may explicitly require unbounded cumulative
policy while preserving finite per-operation/scratch checks. Bound both job and
output queues. The existing canonical parent-prefix checkpoint replay can
initially recompute unfinished internal jobs rather than introducing a new
authority framework or CAS. This preserves a single externally published parent
stream while testing internal work distribution.

Whole selected pieces are the simplest unit. If a single piece is itself costly,
whole prepared **equal-shift groups** are the next boundary: never split raw RHS
terms and classify them independently, because cancellation/coalescing and
original guard provenance belong to the entire group. Budgets, source completion,
deterministic per-source publication, failures, cancellation and checkpoint
progress must remain explicit. The duplicate-work advantage over geometric
splitting is plausible but unmeasured; it cannot fix a serial matcher bottleneck.

The next decision is evidence-driven: measure broad versus serial split work
inflation and slowest-part imbalance on extracted geometry before implementing
descendant subdivision, matched-piece tasks, or relaxed publication. Higher busy
core count is not itself a success criterion.

Some repeated matching or other redundant work is acceptable if it materially
reduces completed-coverage wall time within the shared RAM envelope. Record the
work-inflation versus wall-time tradeoff rather than rejecting a split merely
because CPU increases. Exactly-once ownership of each intended ticket and exact
coverage of valid parts are correctness constraints; overlap between distinct
mathematical inspections is a performance cost, not automatically an invalid
computation. Total CPU and memory remain reported, not optimized at all costs.

## Live trend across midnight, 2026-09-24–25 UTC

A bounded read-only observation of the same unchanged live run covered
23:30:21–00:00:21 UTC (native heartbeat elapsed 8,604.630–10,404.912 seconds).
Sources were the last 12 MiB of `events.jsonl` and 2 MiB of `resources.jsonl`
under `campaigns/five-loop-saved-coarse-cover/runs/20260924T210656.815896Z/`,
plus its small `status.json`. No process was signalled or newly profiled.

| Observed metric | 23:30:21 | 00:00:21 |
|---|---:|---:|
| Native completions | 4,001,705 | 4,030,531 |
| Pending obligations | 4,939,314 | 5,136,993 |
| Native heartbeat RSS | 66.85 GB | 67.52 GB |
| Finished, unpublished inspections | 43 | 156 |

The interval admitted 269,079 new obligations and published 71,400, growing the
backlog by 197,679. Of 28,826 native completions, 27,573 were Route inspections.
Apply heads repeatedly advanced; this was not one unchanged inspection for the
whole thirty minutes. At head `8964580`, owner `011101110111000`, the observed
23:57:33–23:58:30 Apply span produced 992,466 successors and admitted 223,362
new obligations while 189 finished results remained held. The following
approximately one-second Route transition admitted another 15,641.

Coordinator admission preparation and commit increased by 62.304 and 108.265
seconds: together **9.47% of the 1,800.282-second observed wall interval**.
This is measured coordinator admission elapsed time, not a process-CPU share;
it must not be combined with the earlier inclusive CPU-profile percentages.
The final minute's corresponding fraction was 8.84%. Sampled native CPU over
the thirty minutes averaged 1.83 busy cores (median 1.68; brief maximum 8.03).
These observations favor costly Apply inspection and Ordered head blocking,
rather than destination admission dominating this interval. They do not identify
all remaining elapsed time as algebra or establish a whole-campaign bottleneck.

Observed frontiers stayed zero, current pool failure markers were null, and
checkpoint generation 4 remained saved with no active write. Pending work is
the currently discovered backlog, not the remainder of a preknown fixed amount
of work: further inspections may add descendants. No completion ETA, closure
claim, or scheduling-performance gain follows from this observation.

## Independent optimization ranking after the midnight observation

The follow-up source/API audit recommends finishing matched Ready controls
before changing another hot path. In the profiled 21.154-second interval,
coordinator preparation took 0.678 seconds and commit 0.988 seconds: helper CPU
cost must not be mistaken for the current blocking wall-time fraction. The
105.1 million containment calls amount to roughly 318 comparisons per emitted
destination. The sampled `DomainPowerSummary::contains` cost is not summary
construction; admitted summaries and prepared proposed summaries are already
retained.

If Ready makes admission limiting, test these independently:

1. Coarsen Rayon preparation tasks within the existing bounded 256-event batch.
   Preserve indexed output order, cancellation, serial commit and stale-result
   revalidation; first measure task granularity rather than increase threads.
2. Measure selectivity lost by the existing 32-ID index blocks whose envelopes
   remain safely outward-stale after retirement. Recomputing affected blocks
   from surviving summaries may reduce search work. Preserve minimum-ID choice,
   unbounded/empty geometry, allocation preflight and finite-cap behavior.
3. Measure reuse after the job-local cache fills its first 4,096 entries. A
   bounded replacement policy may retain useful recent keys without more RAM.
   Reuse still requires an already accepted same-source admission. Do not
   introduce cross-job caching without evidence and its stronger authority key.

Separately, avoiding the initial deep polynomial clone before Symbolica's first
allocating `replace` is the narrowest inspector optimization identified above.
The audit confirmed the public API and the required identity-substitution,
execution-order, preflight, output-check and guard invariants. None of these
follow-ups is implemented or timed here; they are not part of the Ready
performance comparison.

## Second plateau profile, 25 September 00:07 UTC

An independent raw audit passed for the single later capture in
`TMP/five-loop-later-apply-profile.7oUHHo/`. It used the same native
PID/start/boot/executable identity, native CPUs0–49, recorder CPU62, 49 Hz
user-CPU sampling and bounded 4096-byte DWARF stacks. Perf exited successfully
with zero lost samples. Capture timestamps were 00:07:12.953–00:07:33.966 UTC;
checkpoint generation 4 stayed unchanged with no active write, and the capture
ended 194 seconds before its saved-time-plus-hour checkpoint boundary.

| Measured quantity | 23:26 profile | 00:07 profile |
|---|---:|---:|
| Inspector share of sampled user CPU | 63.69% | 57.86% |
| Admission-helper share | 33.83% | 39.88% |
| Coordinator share | 2.49% | 2.26% |
| Approximate sampled inspector CPU | 19.33 s | 19.31 s |
| Preparation plus commit wall / heartbeat span | 7.87% | 8.62% |

The later native heartbeats span 21.146990 seconds at unchanged Apply head
ID9,023,049, owner `011101110111000`, Amax=Dmax=11 with no D minimum. They add
388,804 events and 387,982 successors, including 18,118 conditional successors;
every successor finds existing responsibility. Scheduled obligations, native
completions and logical publications do not advance. There are 115 finished
results waiting, six occupied native slots and five backpressured workers.
Frontiers and current failure markers remain clear.

Containment comparisons increase by 98,190,859 with no reverse-maintenance
increment. Preparation takes 0.758746 seconds and commit 1.063966 seconds.
Thus more helper CPU still does not establish admission as the elapsed-time
bottleneck. The inspector's smaller CPU percentage accompanies essentially
unchanged sampled CPU seconds, not disappearance of its one-core workload.
These are different live domains, not matched performance controls. Inclusive
application/specialization/guard shares remain overlapping and subject to
unwinding limitations; no speedup, closure or ETA follows from the comparison.

## Input-only coarse-cover hypothesis after the profiles

The observed costly heads also suggest reducing the amount of work, separately
from rescheduling it. The current auxiliary regions retain R<=15, while the
two earlier low-A extracted heads for owner `011101110111000` have A<=11 and
R16–17; the live queue has reached R20. An additional ordinary Apply query with
A<=12/R<=20 and no D bound would contain those two regions, not the other
already-extracted A25/R14 heads. These numbers are experimental
input data, not topology-specific engine logic or a claimed rank invariant.

Read-only implementation review confirms that the existing semantic containment
index could reuse such a query. It would not use the fastest full-orthant path:
`InitialOrthants` rejects any A/D restriction. A rank-only R20 query could use
that path, but would reintroduce the unresolved-positive-guard risk that motivated
the A restriction. Initial D-band reuse also requires actual containment of its
high-D slice; it cannot invent coverage of the added region.

The candidate is a genuine additional obligation to inspect and discharge.
Retain the existing A24/R15 region because these two regions are incomparable,
as well as all original entry queries and every escaping descendant. In
particular, neither R>20 nor A>12 successors may be dropped. Native local
failure, unsupported geometry, or an unresolved descendant prevents success.
A fresh controlled input experiment is needed before any runtime conclusion;
the current live campaign and checkpoint bindings are unchanged.

## Continued monitoring, 25 September 00:50 UTC

The unchanged process has completed 4,386,358 native inspections, with 5,773,401
pending logical obligations, of which 3,925,548 remain native responsibilities.
These are distinct counters, not fractions of a known total. Relative to the
approximately 00:30 snapshot, native completions increase by 103,328 and pending
obligations by 182,605. In the exactly bracketed 00:30:00–00:50:03 heartbeat
window, native completion averages 87.0/s and pending growth 154.1/s. Average
measured CPU usage is 3.11 cores, with 164 finished results waiting on average
and a maximum of 252. The backlog is not draining.

RSS is 69.28 GB, with 70.48 GB peak and roughly 882 GB host memory available.
The configured guard remains 475/500 GB. Checkpoint generation 5 completed at
00:12:13 UTC: 9.785 GB in 85.04 seconds. No write or cancellation is active at
the snapshot; reported frontiers and both failure markers are clear. Bounded
read-only tails were sufficient for this observation; no live signal, new
profile or configuration change was needed.

The same owner dominates 91% of heartbeat samples, and 96% report Apply.
Coordinator preparation plus serial admission commit account for about
240 seconds of the 1,203-second window; this wall telemetry is not a CPU phase
decomposition. Current expensive heads report A25–26, unlike the earlier
extracted A11 heads. Thus the A12/R20 input-only hypothesis above cannot cover
all current expensive work. Before a new coarse-cover pilot, recover the exact
geometry of a representative current head from an immutable checkpoint and
check its guard restrictions. Do not broaden based on the owner mask alone or
clip escaping descendants. These observations still provide no five-loop
completion ETA.

### Exact current-head geometry

The read-only extraction in `TMP/five-loop-current-head-extract.Jtj0wn/` recovers
four current Apply heads from immutable checkpoint generation 5, using the same
bounded-memory framing/parser as the earlier diagnostic. It reads 9.785 GB in
58.98 seconds with 315,428 KiB maximum RSS on low-priority CPU62. File identity,
size and modification time remain unchanged; the complete raw SHA256 is recorded.
This is selected-record inspection, not a full solver restore or independent
BLAKE3 verification.

| Queue ID | A maximum | R maximum | D maximum |
|---|---:|---:|---:|
| 9,499,579 | 26 | 13 | 26 |
| 9,532,265 | 25 | 14 | 25 |
| 9,661,172 | 25 | 14 | 25 |
| 9,694,360 | 25 | 14 | 25 |

All four have owner `011101110111000` and no lower D bound. For example, the
last has zero lower bounds except axis1=13, with upper bounds
`[14,14,0,1,14,1,0,0,14,0,0,0,14,14,14]`.
The current bottleneck therefore crosses the existing auxiliary **positive-power
bound A24**, not its rank bound R15. The earlier R16–17 observations describe
different costly regions; neither explanation should be generalized to all jobs.

The smallest proposed common A/R enlargement for these four samples is an
additional same-owner A26/R15 Apply region with no D restriction. Native guard
feasibility must be tested before a recursive performance pilot. The original
A24/R15 region and every required entry remain ordinary pinned obligations;
all A>26/R>15 descendants remain required too. Geometric containment alone
neither solves the added region nor proves that inspecting it will be faster.
No new candidate input has been launched in the live campaign.

A more selective follow-up proposal is a staircase union: retain A24/R15 and
add A25/R14 plus A26/R13, each with the same owner and no D restriction. These
regions still contain the four observed heads, without introducing the extra
A+R=40–41 combinations admitted by the uniform A26/R15 rectangle. This is only
an input-cover design hypothesis, not an A+R<=39 invariant; every escaping
descendant remains required. The first six-query local feasibility diagnostic
keeps its original uniform-cover candidate and order unchanged. If that larger
candidate adds too much work, test the staircase separately rather than
silently changing the comparison or narrowing the required starting inputs.

The live process subsequently completed periodic checkpoint generation 6 at
01:13:48 UTC: 10,826,119,568 bytes in 94.60 seconds. Its saved queue records
4,471,428 completed native inspections and 5,996,277 pending logical obligations.
The frozen process and 500 GB RAM allowance remain unchanged. The independent
release-profile comparison waits for this actual write to finish before its
timed controls; profiling and checkpoint I/O are not silently included in those
paired windows.

### Completed local test of the larger reusable region

The six-query diagnostic in `TMP/five-loop-a26-feasibility.zvKWRP/` completed
without a deadline or work-count cutoff. It loads the same 67 immutable owner
programs once, then inspects the existing A24/R15 region, the proposed A26/R15
region, and the four exact saved heads. Native applicability, RHS construction
and guard handling are unchanged; the sink retains counters and bounded
diagnostics, not a recursive worklist or an exact coefficient certificate.

| Query | Native wall seconds | Process CPU seconds | Selected pieces | Successor events | Conditional successor events |
|---|---:|---:|---:|---:|---:|
| Existing A24/R15 | 89.262 | 88.38 | 16,850 | 1,800,202 | 33,423 |
| Additional A26/R15 | 90.884 | 90.15 | 16,863 | 1,808,352 | 33,533 |
| Head 9,499,579 | 35.436 | 35.15 | 1,193 | 703,469 | 22,201 |
| Head 9,532,265 | 43.590 | 43.26 | 2,201 | 898,878 | 24,814 |
| Head 9,661,172 | 47.082 | 46.74 | 1,311 | 850,564 | 24,672 |
| Head 9,694,360 | 8.346 | 8.30 | 139 | 36,354 | 461 |

All six inspections report complete local feasibility, with zero gaps,
unresolved/invalid classifications, application problems or unsupported images.
Optional coefficient-refusal attempts are respectively 321/321/223/278/247/3;
each call retains one optional-refusal callback under native filtering. These
distinct counters must not be equated or presented as missing obligations.
Every conditional successor remains counted; its subsequent inspection has not
been performed by this diagnostic.

The larger region adds only 13 selected pieces and 8,150 successor events
(0.453%) in this fixed warm-order observation. Its native wall time is 1.622
seconds higher, but adding it alongside the required original costs its **full
90.884 seconds**, not that difference. Broader outgoing geometry can still
increase recursive work. Local feasibility is therefore encouraging evidence
for a separate reuse experiment, not evidence of closure or net speedup.

Whole-command time is 332.185 seconds, including a 13.619-second owner load;
sampled peak RSS is 5.991 GB. The RAM-only guard did not intervene. The helper's
binary, source, inputs and all 67 program hashes are bound in its receipts.
It uses a distinct explicitly recorded helper/profile: compare these six calls
with each other, not their absolute times with the live CLI or older probes.

The last head also gives a stronger subdivision hypothesis than tuple balance:
its eight most expensive selected pieces account for 6.362 of 6.625 seconds of
selected-piece time (96.03%, or 76.23% of the whole native call). Each has only
axis1 varying between local offsets 13 and 14; every other index is fixed.
A disjoint split at 13 fixes that final index. A separate unchanged-helper
broad/serial/parallel experiment is required to determine whether this actually
reduces algebraic cost. Equal tuple counts alone do not predict speedup.

### Positive-index split of the measured expensive head

The first such experiment completed and independently passed raw-result audit
in `TMP/head9694360-positive-split.TKgkup/`. It uses the original frozen
`EcUaJy` probe, with its own unsplit baseline; its absolute times must not be
compared with the differently built six-query helper above. The query is
exactly saved head 9,694,360. Splitting local axis1 at 13 yields the disjoint
singleton values 13 and 14 (physical positive indices 14 and 15). All other
coordinates and coupled A/R/D restrictions are retained.

| Execution | First-order wall / CPU seconds | Reverse-order wall / CPU seconds | Selected pieces | Successor events |
|---|---:|---:|---:|---:|
| Unsplit | 9.631 / 9.57 | 8.295 / 8.23 | 139 | 36,354 |
| Same two parts, serial | 4.043 / 4.01 | 3.323 / 3.29 | 278 | 72,708 |
| Same two parts, concurrent | 1.903 / 3.79 | 1.672 / 3.32 | 278 | 72,708 |

All parts complete without local gaps, application problems or unsupported
images. Serial and concurrent per-part queries, integer work counters and
bounded diagnostics agree exactly. Conditional successor events change from
461 to 675; optional coefficient-refusal attempts change from three to zero.
Matching and term visits double, rather than disappearing from the accounting.

The first observed 5.06x unsplit/concurrent wall ratio is **not simply two-core
scaling**: splitting already gives a 2.38x serial improvement. Summed selected
application time falls from 7.722 to 2.909 seconds in the serial split, consistent
with cheaper fixed-index specialization and guard handling. The fresh-process
follow-up in `TMP/head9694360-positive-reverse.pDAF0K/` reverses execution to
concurrent/serial/unsplit without changing the executable, query or policies.
It gives 4.96x unsplit/concurrent and 2.50x unsplit/serial ratios. All per-mode
integer work counters, per-part queries and bounded diagnostics also reproduce
across processes. Both runs complete without guard intervention or a concurrent
live checkpoint write. This weakens the simple warm-order explanation; two
short shared-host sequences do not isolate causal effects or establish a
population-wide performance guarantee.
The table excludes the one-time shared owner load: 17.391 seconds in the first
process and 29.924 seconds in the reverse process. Whole-helper times, including
all three modes and loading, are 37.61 and 45.73 seconds respectively; the
roughly fivefold figure is an inspection comparison, not process-launch timing.

The additional outgoing events still require downstream containment and
publication. This native-only result therefore does not establish production
speedup, a coefficient-payload union certificate, or recursive closure. Before
integration, compare whole-domain subdivision with splitting already-selected
expensive pieces: the latter could avoid duplicate matching, but needs its own
ownership, cancellation, publication and checkpoint design. Neither option may
drop a piece or treat a completed part as a completed original obligation.

Independent source review recommends the smaller first implementation experiment:
an **off-default serial refinement of an already normalized application cell**,
immediately before the existing full equal-shift-group application. Admit a
small finite single varying coordinate through a generic geometry policy; use
deterministic exact singleton children and keep the original matched piece,
guards, A/R/D constraints and one parent event stream. This targets the serial
algebraic gain, not parallel scheduling itself, and avoids matching the whole
source twice. Preserve the original shared operation budget and cancellation;
check endpoints and physical-index representability before refinement. Ineligible
cells take the unchanged path, but a failure after emitting a child remains
incomplete rather than falling back and duplicating effects. Bind the policy
into checkpoint requests. Implementation has been authorized, but no production
speedup or live adoption follows from this design review.

### Live monitoring at 01:32 UTC

The unchanged, identity-checked native process has completed 4,524,380 native
inspections, with 6,059,726 pending logical obligations (4,056,198 native).
RSS is 71.19 GB, below the unchanged 475/500 GB guards. Frontiers remain zero
and both failure markers are clear. Checkpoint generation 6 is saved and no
write is active. These observations do not imply closure.

The bounded-tail analysis in `TMP/five-loop-monitor-snapshot.tPrXbj/` covers
01:12:05–01:32:05 UTC, including the 94.60-second checkpoint write. Native
completions increase by 52,952, but pending obligations also increase by 63,449.
Mean measured CPU is 2.37 cores; mean active native slots are 1.92, while mean
finished-held inspections are 175.66 (maximum 254). This is not a checkpoint-free
benchmark or evidence that the configured 50 workers are well utilized.

The window emits 20.16 million successor callbacks but schedules only 151,906
new logical obligations: substantial duplicate suppression already occurs.
Containment-check count increases by 10.99 billion. Coordinator preparation
plus serial commit accumulate 177.12 seconds of wall instrumentation, not an
exclusive CPU decomposition. Queue growth, repeated application and ordered
publication remain distinct concerns.

Owner `011101110111000` appears in 92.2% of heartbeat samples and Apply in 97.6%.
The longest recent head spans again have A12/13 bounds, with rank absent from
the telemetry. Thus the locally feasible A26/R15 candidate must not be assumed
to cover the current wave; exact rank and geometry still matter. Continue
monitoring while testing isolated improvements; leave the live process unchanged
and do not extrapolate a completion ETA from these counters.

### Current-wave profile, September 25 at 02:05 UTC

A further 20-second, 49 Hz user-CPU capture on the unchanged live executable
is retained in `TMP/five-loop-current-wave-profile.O8tkRy/`. It reports no
lost samples, but bounded DWARF unwinding leaves incomplete caller stacks;
zero lost samples must not be described as complete call-stack coverage.
Compilation used disjoint CPUs50–55 while production remained on0–49.
This is observational profiling on a shared host, not an isolated benchmark.

The bracketing native heartbeats span 20.134 seconds. They advance40 native
inspections, emit329,580 successors and schedule468 obligations; pending work
grows428. Apply owner `010011111101011` remains the same while the publication
cursor advances. Only2–3 native slots are active and237–238 finished results
are held for publication. Frontiers and failure markers remain clear.

| Exclusive sampled user CPU | Share |
|---|---:|
| Native inspectors | 62.73% |
| Admission helpers | 35.12% |
| Coordinator | 2.15% |

Within admission, power-summary containment plus index traversal account for
20.55% of all sampled CPU; epoch/deque leaves account for8.06%. These motivate
two separate experiments: coarser Rayon lookup tasks within the unchanged
batch, and tighter block envelopes after index retirement. Neither is yet a
measured improvement. Preserve exact commit order, cancellation and stale-lookup
revalidation. First measure whether tighter envelopes actually avoid enough
false-positive lookups to pay for refreshing them.

Coordinator-instrumented preparation plus commit take1.704 seconds, or8.47%
of this heartbeat interval. This elapsed-time boundary is distinct from the
35.12% sampled-CPU share and cannot be used interchangeably with it. Inclusive
inspector paths overlap; missing caller frames prevent an Amdahl-style bound
from this profile. Inspection, admission and ordered publication all remain
relevant; improving just one is not established to fill50 workers.

At02:22 UTC, production has approximately5.415 million completed native
inspections,8.349 million pending obligations and82.76 GB aggregate RSS.
Checkpoint generation7 saved successfully in112.325 seconds (12.608 GB).
The dynamically growing queue still has no defensible completion ETA.

### Longer mixed-owner window, 02:06–02:31 UTC

The next bounded-tail snapshot, `TMP/five-loop-monitor-snapshot.HQEUkb/`,
covers 1,499.696 seconds and includes the generation7 checkpoint write.
Native completions increase644,398, scheduled obligations2,559,304 and
publications950,585. Pending work therefore grows1,608,719 to9,268,903;
this is expansion, not a remaining-work countdown. RSS reaches86.630 GB.
Frontiers and both failure markers stay clear.

Mean observed process CPU is3.879 cores, not50. Progress samples average1.806
active native slots and175.067 finished-but-held results (maximum255).
Owners are now diverse: the largest accounts for15.81% of sampled heads;
Apply/Route account for77.16%/22.84%. These are samples, not completed-job shares.

Coordinator-instrumented preparation takes178.560 seconds and ordered commit
560.783 seconds: together49.30% of this interval, including checkpoint time in
the denominator. This differs materially from the earlier short profile's
8.47% elapsed fraction. It reinforces admission as a separate optimization
target, but is not a CPU attribution or a matched performance comparison.
The changing workload means neither window should be generalized to the entire
campaign. Continue profiling actual waves and preserve the live configuration.

A second independently reviewed20-second capture at02:32:44–02:33:05 UTC
(`TMP/five-loop-mixed-wave-profile.pfoDMS/`) samples this changed workload.
Admission takes39.69% of sampled user CPU, the coordinator5.42% and inspectors
approximately54.9%. Epoch pin/advance plus deque stealing take15.12% of total
sampled CPU, versus8.06% in the earlier capture. Containment/index leaves take
10.23%, versus20.55%. These are different workload windows, not a speedup test.

Preparation occurs304.54 times/s (earlier66.70), averaging211.87 records/batch
and89.77 comparisons/request (earlier379.89). Preparation plus commit accumulate
4.350 seconds over21.144 heartbeat seconds, or20.57% elapsed time. That is
neither the39.69% CPU share nor the preceding25-minute49.30% elapsed fraction.
The changing ratios reinforce the need to measure several actual waves.
All bracketing frontiers/failure markers remain clear; no checkpoint write
overlaps this short capture. Concurrent builds and the side recovery test use
disjoint cores, but shared-host effects remain possible. Unwinding is still
incomplete despite zero lost samples.

Inspection of the pinned Rayon producer suggests a narrowly scoped next
scheduling experiment: minimum-eight-record tasks within the existing256-record
immutable preparation batch. This would retain the default no-steal split shape
with24 helpers while preventing finer stolen-subtree splits. Eight records do
not mean eight costly lookups or a guaranteed task count; load skew can make
the change worse. Keep batch boundaries, indexed event order, cancellation,
stale-lookup validation and serial commit unchanged. At the time of that capture
this was a source-based proposal, not an implemented or measured optimization;
the later insertion-replay experiment is described below.

### Hour-long admission-heavy window, 02:27–03:27 UTC

The independently collected bounded-tail audit in
`TMP/five-loop-hour-audit.aAxCY7/` retains the unchanged process identity,
raw samples and arithmetic. Native completions increase 1,283,709, publications
2,086,332 and scheduled obligations 4,755,081. Pending work grows 2,668,749 to
11,490,662; every quarter-hour adds pending work. The interval with slower
queue growth also has slower completion throughput, so it is not evidence of
convergence. The maximum scheduled finite rank reaches 21; escaping descendants
are retained rather than clipped to the input rank 15.

Mean measured CPU use is 5.479 cores out of 50. RSS rises from 84.779 to 101.589 GB.
Preparation plus ordered commit account for 2,347.683 seconds, 65.22% of the
coordinator elapsed interval. In the final five minutes the fraction is 87.76%
(29.05% preparation, 58.70% commit). These are elapsed spans, not CPU shares.
Prepared batches average 157.58 records over the hour; speculative comparisons
per request rise across the four quarters from 296 to 369, 504 and 768. Workload
composition changes, so this is a reason to profile destination lookup and
commit, not an attribution to a particular index defect. General containment
counters include conservative reverse-maintenance charges; they are not all
actual native inclusion calls and must not be added twice.

Generation 8 saves 15.899 GB in 137.769 seconds, approximately 3.83% of the hour;
the last-five-minute window excludes that write. Checkpointing cannot explain
the persistent admission fraction. All sampled frontier and failure markers
remain clear. RAM is only 21.39% of the 475 GB cooperative threshold and disk
headroom remains ample; this capture establishes no immediate safety reason to
stop. Positive backlog growth is not a proof of nontermination or an ETA.

The post-match cell-refinement control remains relevant to the earlier costly
head: its eight slowest matched pieces have only one two-valued coordinate.
It avoids repeating matching but can replace one successor with two narrower
successors. A local algebra speedup must therefore be checked against actual
recursive admission/work inflation before any production recommendation.

A further independently analyzed 20-second profile at 03:32:18–03:32:39 UTC
(`TMP/five-loop-commit-wave-profile.ovuHxX/`) confirms that the phase mix changes
quickly. During its 21.150-second heartbeat bracket, 4,805 native inspections
and 13,229 logical publications complete, versus 10,920 new obligations: pending
work temporarily falls by 2,309. Preparation plus commit take 5.799 seconds,
27.42% elapsed time, not the preceding five-minute 87.76%. Neither this short
decrease nor the preceding growth establishes eventual exhaustion.

In this capture, admission helpers account for 51.72% of sampled user CPU,
inspectors 41.98% and the coordinator 6.28%. Admission containment/index leaves
take 19.63%, while epoch pin/advance and deque stealing take 15.96%. Batches
average 190.74 records at 410.88/s. Coordinator containment/index and JSON leaves
are only 0.57% and 0.33% of all sampled user CPU: this sample does not establish
reverse maintenance or serialization as the dominant coordinator cost. Caller
stacks remain incomplete, so forward/reverse lookup and stale-snapshot fallback
cannot be separated reliably. These measurements support controlled task-grain
and lookup experiments, not a speedup claim or an immediate policy change.
Identity and checkpoint 8 remain unchanged, no write overlaps the capture, and
no samples are lost. Concurrent compilation on separate cores still permits
shared-host effects; this is profiling, not an isolated timing comparison.

### Inspector-heavy window, 03:44–04:10 UTC on September 25

The next bounded-tail audit (`TMP/five-loop-admission-grain-snapshot.FQPU3o/`)
finds a different bottleneck again. Over roughly 25 minutes, native completions
increase by 6,116, scheduled obligations by 3,405 and publications by 17,395.
Pending work decreases by 13,990, but remains 11,238,788. RSS is 104.908 GB;
mean measured CPU use is 1.878 cores. Approximately 99.7% of sampled publication
heads belong to one nine-support owner, with repeated roughly 48–51-second
observed head residencies, not measured whole-inspection durations. This is a
workload observation, not topology-specific dispatch.
Preparation plus commit account for only 9.09% of coordinator elapsed time,
versus 65.22% in the preceding admission-heavy hour. Checkpoint 8 is unchanged.

A separate read-only profile at 04:09:47–04:10:08 UTC
(`TMP/five-loop-inspector-wave.PjxrBx/`) brackets one unchanged unfinished head.
Its 20.137-second heartbeat interval has:

| Observation | Delta or measured value |
|---|---:|
| Completed native inspections | 0 |
| Successor scheduling requests | 359,247 |
| Newly scheduled obligations | 13 |
| Deduplication hits | 359,234 |
| Exact-domain hits | 72,260 |
| Job-local exact reuse hits | 375 |
| Domain-summary builds | 286,612 |
| Speculative containment comparisons | 87,181,305 |
| Preparation plus commit elapsed time | 1.883 seconds (9.35%) |
| Process CPU use over the resource bracket | 1.785 cores |

These are descriptor-level requests, not distinct scalar integrals. Almost all
are already covered by other pending or completed domain obligations. Such reuse
does not discharge the covering obligation or establish family closure.
Speculative and committed comparison counts overlap and must not be added.

The single active inspector accounts for 58.74% of sampled user CPU, admission
helpers 38.58% and the coordinator 2.68%. Admission containment/index leaves take
23.83% of total sampled CPU and epoch/deque scheduling leaves 8.28%. Inspector
selected allocator leaves take 12.29%, and repeated polynomial validation 3.92%.
The allocator subset excludes plain `malloc`, `cfree` and `malloc_consolidate`;
it is not the complete allocation cost. These figures are exclusive leaf sums,
not complete phase costs. Zero lost samples
does not fix incomplete stack unwinding. No checkpoint write overlaps this
profile, and the native process is neither signalled nor reconfigured.

At the end of the sample, 177 later inspections have finished but await ordered
publication; only one native slot is occupied. The existing ordered reservation
window can therefore prevent millions of queued obligations from filling the
other inspector slots. Increasing admission task grain alone cannot remove that
head-of-line constraint. The experimental Ready policy addresses that constraint,
but its five-loop full-drain performance/recovery comparison remains open, so
the live policy is unchanged.

The immediate experiment isolates task grain with a **test-only** constructor,
the same immutable 256-record batches and unchanged ordered commit. Its first
workload replays actual retained domain insertions from a smaller saved pilot;
it deliberately does not reproduce the live stream's overwhelmingly reused
proposals or the pilot's interleaved publication lifecycle. Any gain there is an
insertion microbenchmark result, not a live-campaign speedup. Follow-up work
should measure reuse-heavy proposals and repeated inspector algebra separately;
the current data do not justify promoting any optimization or claiming an ETA.

For this fixed short trace, even deleting all 1.883 seconds charged to
preparation/commit would shorten the 20.137-second bracket by only about 9.35%
(roughly 1.10x). This is illustrative fixed-trace arithmetic, not a bound on a
different scheduler: changing publication order can change the work itself.
It explains why the grain experiment must remain separate from the larger
Ready/publication and expensive-inspection investigations. In the earlier
admission-heavy hour the same proportions were very different. Optimizing for
one displayed CPU-utilization number would miss these changing critical paths.
