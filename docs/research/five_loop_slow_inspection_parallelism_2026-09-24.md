# Five-loop slow inspection: parallelism and repeated work

24 September 2026. Technical study after the refreshed Vakint numerical gates
and the completed four-loop controls. No live campaign, native policy, saved
rule, or solver implementation has been changed. The five-loop workload is
still incomplete; these observations provide neither a closure claim nor an
ETA. The complementary [independent scheduler audit](five_loop_parallel_publication_audit_2026-09-24.md)
covers publication, responsibility and checkpoint constraints.

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

## Smallest credible experiments

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
