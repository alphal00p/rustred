# Independent audit: ordered publication and parallel work

Historical design baseline, before the experimental `Ready` policy was added.
The [implementation and matched controls](five_loop_ready_publication_2026-09-24.md)
supersede statements below about the available policy flags and single-prefix
checkpoint support. The mathematical dependency analysis and within-inspection
work-distribution proposals remain relevant. Ordered still drives the unchanged
live campaign; Ready has not demonstrated a production speedup.

This is a source/architecture assessment with one small diagnostic control
reported below, not an implemented engine optimization. No running campaign was changed, resumed, signalled or
reconfigured. The existing five-loop campaign remains incomplete. Its large
pending count is neither a completion fraction nor a count of equally costly,
immediately dispatchable native jobs.

## Conclusion

Global FIFO publication is **not a mathematical requirement** of this symbolic
successor walk. Each native inspection uses immutable programs and a specified
source domain; it can discover successors before other independent inspections
finish. Reusing an admitted containing obligation does not require it to have
finished first, provided that responsibility is retained and the final ledger
cannot discharge a failed, unfinished or cancelled representative.

However, FIFO is a real invariant of the current **implementation and checkpoint
format**. Replacing the current-head poll with an arbitrary ready-ticket poll
would be incorrect. The viable alternatives require changes to ownership,
publication and persistence, not merely another thread count or queue ordering.

The existing owner-batched path already demonstrates a narrower relaxation:
different `(phase, owner)` queues publish ready streams independently. It still
serializes the head of each owner, and currently does not support checkpointing
or physical subdivision. A single expensive owner can therefore remain a
bottleneck even under owner-batched scheduling.

## What the current source actually enforces

| Component | Current contract | Why a naive change is unsafe |
|---|---|---|
| Ordered coordinator | Polls only the ticket for `queue.next`; later finished results may enter bounded escrow | Later chunks would be charged to the wrong active source record |
| Responsibility ledger | `cursor`, fixed `cursor + H` reservation fence, publication only at `id == cursor` | Merely increasing dispatch can enter transferable/unreserved work or invalidate deterministic transfers |
| Source-local progress | One frontier list, refusal accumulator, accepted-prefix digest and physical-parent progress | Interleaving sources needs separately keyed ownership and replay cursors |
| Destination admission | Exact/inclusion lookup, retirement, ledger admission and domain publication form a serialized transaction | Concurrent stale lookup misses must not create duplicate IDs or erase obligations |
| Checkpoint | Completed prefix plus one partially published stream/part; uncommitted speculative work may be replayed | Out-of-order completed holes or multiple accepted source prefixes are not represented |
| Resource control | Bounded producer chunks and completed escrow; workers may block on publication | An unbounded out-of-order staging reservoir is not a correctness or RAM solution |

Source anchors:

- [`execution.rs`](../../crates/rustred-app/src/application/routed_campaign/walking/execution.rs):
  `State` fields around lines 30–59; dispatch fence and escrow around 928–995;
  current-ticket polling at 1009. Publication is distinct from dispatch and
  from a worker returning `Finished`.
- [`delegation/ledger.rs`](../../crates/rustred-app/src/application/routed_campaign/walking/delegation/ledger.rs):
  `transfer_retired`, `native_started`, `publish_native`, `check_publisher`.
  Transfer is only from an unreserved old ID to a newer containing ID with the
  same phase/owner. Reserved/started/initial obligations cannot be stolen.
- [`queue/prepared.rs`](../../crates/rustred-app/src/application/routed_campaign/walking/queue/prepared.rs):
  immutable speculative lookup is evidence, not admission. Commit revalidates
  exact/orthant lookup and subsequent changes before publishing state.
- [`checkpoint/codec.rs`](../../crates/rustred-app/src/application/routed_campaign/walking/checkpoint/codec.rs):
  persisted state includes the queue/ledger, records, one `details`/`refusals`
  accumulator and current replay/physical progress.
- [`parallel.rs`](../../crates/rustred-app/src/application/routed_campaign/walking/parallel.rs):
  `publish` blocks while a slot's prior chunk is occupied; `poll` releases it.
  [`parallel/escrow.rs`](../../crates/rustred-app/src/application/routed_campaign/walking/parallel/escrow.rs)
  retains successful returned jobs within an explicit entry/byte allowance,
  without treating them as published coverage.

These restrictions make deterministic domain IDs, representative selection,
capped failure prefixes and single-prefix restart substantially simpler. They
do not constitute an independent IBP identity or global termination proof.

## Can the millions of pending obligations run independently?

Many could run concurrently in a suitable scheduler; they are not waiting for
the mathematical value of the current integral. This walk classifies domains
and discovers obligations rather than recursively waiting for numerical RHS
values. But not every pending record needs, or is eligible for, a native call.
Some are delegated obligations. Others are protected by the dispatch fence or
already represented by running/finished jobs. Overlapping distinct domains can
both have been reserved before a broader cover arrives.

There are two different meanings of “no double work”:

1. Exactly one owner of each native ticket: achievable with an atomic
   unreserved/reserved/started/finished state transition and a nontransferable
   reservation. This is a correctness requirement.
2. Never inspect overlapping mathematical points twice: not guaranteed by the
   present representation, and not obtained merely by out-of-order scheduling.
   Two already reserved boxes may overlap or later be contained in a new box.
   Cancelling one safely would require a separate complete-stream/dependency
   protocol; index retirement alone is not permission to drop its work.

Raising the lookahead can expose more jobs, but reserves more obligations before
later covers can absorb them. It can therefore increase native work, retained
buffers and RAM. More workers cannot parallelize a single unsplit native call,
nor remove a single-publisher admission bottleneck. The measured live profile
showed a substantial inspector component as well as admission work, so neither
“all serial admission” nor “just allocate 50 inspectors” is established.

Zero redundant computation is **not** an acceptance requirement. Repeated
matching, overlapping inspections, or explicitly managed replicated attempts
can be useful if they reduce completed-coverage wall time within the RAM
allowance. The correctness requirement is unambiguous ownership and exactly-once
logical publication of each accepted effect, not exactly one CPU attempt at
each mathematical point. A replicated attempt needs an explicit winner/discard
protocol; its extra CPU must not become duplicate logical completion.

Assess bounded lookahead, independent source parts and ready-source publication
on the wall-time/CPU/RAM tradeoff, not by rejecting CPU inflation in isolation.
For example, a larger bounded window may keep more inspectors productive at the
cost of weaker later absorption, while geometric parts may repeat matching to
shorten the longest call. Neither is automatically preferable to doing less
work serially. Report the actual work inflation, slowest-part time, retained
buffers and checkpoint cost alongside wall time; retain a useful faster point
even when it does not minimize total CPU.

## Existing ready-stream path and its limit

[`owner_batches/mod.rs`](../../crates/rustred-app/src/application/routed_campaign/walking/execution/owner_batches/mod.rs)
explicitly identifies publication order as scheduling rather than mathematical
authority. Its coordinator polls eligible FIFO heads from different keys,
delivers ready chunks without waiting for quiet peers, and admits to destination
queues exclusively per key. Its selector fairly offers each ready key an
inspector before adding more inspectors to a hot key. Later jobs of that same
key remain FIFO-held.

“Ready stream” is an internal scheduling mechanism here, not a third public
publication-policy flag. The public policies are `Ordered` and `OwnerBatched`.
[`walking/mod.rs`](../../crates/rustred-app/src/application/routed_campaign/walking/mod.rs)
rejects checkpointing and subdivision with `OwnerBatched` at preflight. It is
therefore not a drop-in resumable replacement for the live campaign.

## Sound alternatives to assess, not yet implemented

**Ready-source publication with exclusive destination admission.** Maintain a
per-ticket stream accumulator (frontiers, refusals, accepted event prefix,
completion/error state). Preserve each source's event order: counted local
reuse is valid only after that source's preceding geometric admission has been
accepted. Admit chunks from any ready source using one linearized destination
transaction, or independent locked owner/phase transactions. Publish a source
as complete only after all its events and its genuine native `Finished` are
accepted. Keep failure and cancellation visible globally.

The ledger then needs explicit completed/pending sets rather than interpreting
one cursor as all publication state. Forward-only delegation IDs can still make
alias resolution acyclic independently of completion order. Checkpoints must
persist every accepted partial prefix and out-of-order completion, with source
digests and reservation state. On restart, only unfinished streams may replay;
previously admitted effects must be suppressed exactly once. A consistent
snapshot must include in-flight cross-owner delivery transactions or quiesce
them. This requires an explicit format/binding transition, not silently loading
an old ordered checkpoint under a new policy.

**Within-inspection subdivision.** An exact partition of an expensive domain
can parallelize its native bottleneck even when most pending work belongs to
one owner. Preserve the full coupled rank/power predicates in every part,
prove their union covers the parent, and retain every successor. Each part
needs its own source stream; one logical parent is discharged only when all
parts genuinely finish. Duplicate outgoing obligations are handled by ordinary
admission, not by dropping source regions or conditional branches.

The current two-part option is narrower: `State::parts` applies only to initial
IDs, and `ApplySubdivision::parts` requires an explicit finite upper bound on
the selected local axis. It does not split the currently expensive descendant
head. A fixed two-way cut can also be badly imbalanced or duplicate matcher/
guard work; the existing small integrated control was slightly slower. Neither
that negative result nor the earlier native-only speedup predicts an adaptive
partition of the actual expensive descendant workload.

There is a second integration restriction: the ordered coordinator polls the
current `publisher_raw` ticket, so physical part0 is admitted before part1.
An unfinished nonpublisher part can block on its bounded output chunk long
before returning `Finished`; completed-job escrow does not drain that stream.
The isolated pilot's independent stats-only sinks therefore have more drain
parallelism than the present recursive walker. A native-only split gain cannot
establish production gain. A useful integration needs sound interleaved part
admission and per-part checkpoint prefixes, or another explicitly bounded drain
mechanism, while retaining one logical parent's completion responsibility.

The explicit-finite-upper condition is not intrinsically necessary for a sound
partition: for a finite cut `c >= lower` with both `c` and `c+1` representable,
an unbounded axis can be split exactly into `[lower,c]` and `[c+1,infinity)`.
Intersecting both parts with the unchanged coupled A/R/D predicates preserves
the parent union. This
does not provide a balanced cost split or justify dropping a part that merely
looks empty; use existing exact geometry/native-empty handling. Extending the
code to descendants still requires the ticket/checkpoint contracts above.

**Matched-piece work distribution is a different possible seam.** The current
native application engine performs matching and immediately calls private
`apply_piece` on the same reducer. A zero-rematch parallel path would need an
opaque, immutable capability bound to that exact reducer/program snapshot and
its selected rule, source geometry, coupled predicates and matching authority.
An owned public piece plus owner/batch/rule ordinals is not sufficient authority
to apply it through an arbitrary other reducer. Keep the existing native
application/coefficient operations; do not duplicate them in Python or expose
unchecked reconstruction of a selected rule. A global parent budget must still
account for all pieces, and interruption must preserve partial pieces without
losing the matching coverage inventory.

This does not require a cryptographic identity or new authentication framework.
A privately constructed task can retain a direct borrowed reference to the
actual immutable program/rule and its selected source. Lifetimes keep the data
alive, but a lifetime parameter alone does not distinguish two program instances
that happen to live equally long; the actual reference provides the binding.

If distributing RHS work below the matched-piece boundary, equal-shift terms
must retain their original grouping/coalescing and original-denominator checks.
Naively farming individual terms can change cancellations, conditionality and
optional-refusal behavior. Partitioning whole source pieces or whole validated
shift groups is conceptually different from dropping/independently summing
arbitrary terms. Exact typed native APIs and per-source completion receipts are
prerequisites, not a reason to reimplement coefficient algebra.

## First actual descendant split: limited gain and severe imbalance

The [companion inspection study](five_loop_slow_inspection_parallelism_2026-09-24.md)
now records a completed one-hop pilot on saved queue ID3,889,485. Its whole
parent was partitioned exactly at axis0/cut1, preserving rank16/Amax11/Dmax11;
this is not asserted to be the live overlap-trimmed inspection. Native broad,
serial-part and parallel-part spans were 48.866, 49.309 and 48.069 seconds,
with 48.39, 48.81 and 50.83 seconds of process CPU. Serial parts cost 46.131
and 3.178 seconds despite a near-even 48.05%/51.95% count of concrete points.

Independent audit verifies actual completion and all non-timing per-part
serial/parallel fields, including every counter and retained refusal example.
All modes produce 885,723 successors, 28,401 conditional successors and zero
gaps, unresolved classifications, problems or unsupported transitions. The
296 optional original-coefficient refusals remain explicit. Broad-versus-split
work differs only by one callback event, two matching cells, 60 coordinate cells
and one terminal check; no successor coefficient-stream equality is claimed.

This first cut does not provide substantial scaling. It is evidence that
concrete tuple counts are not a reliable cost balance, not evidence against all
source subdivision. The root stopped the proposed fixed rotations before
launch; a later cost-informed cut is an exploratory strategy, not a selected
favorable repeat or campaign-wide speedup. Raw evidence and the independent
audit are under `TMP/five-loop-head-split.EcUaJy/`. The live campaign is unchanged.

That separately predeclared axis4/cut5 trial also completed and passed independent
raw audit. It measured 49.224 seconds broad, 50.350 serial and 43.642 parallel,
with 48.76/49.87/49.56 seconds process CPU. The observed single-order 1.128x
native wall ratio costs 1.64% more CPU, while one serial part still consumes
87.3% of part time. Native operations rise 3.61%, successor descriptors become
918,016 instead of 885,723, and optional original refusals become 330 instead
of 296; every part completes without gaps, problems or unsupported transitions.
These genuine fragmentation/work differences remain reported. The exact source
union does not constitute a successor coefficient-payload equality proof, and
the independent-sink versus production-buffer distinction above still applies.
No further cut or rotation was run. See the companion study for the full
predeclared hypothesis, timing boundaries and retained negative first result.

## Smallest checkpoint-compatible ready-publication proposal

Prefer one global queue/index/ledger and one serialized admission coordinator,
with ready-ticket polling allowed across sources **including the same owner**.
This is an opt-in design recommendation, not an implemented or measured change.
It preserves the existing native visitor and avoids introducing distributed
destination transactions merely to remove global head-of-line waiting.

Use `H` as outstanding native-parent credits rather than the numeric interval
`[lowest_unfinished, lowest_unfinished + H)`. Keep an independent reservation
scan, sticky reserved/started ownership, and a lowest-unfinished watermark.
When all events and a genuine successful `Finished` have been accepted, the
source becomes a published completion hole and releases its credit. That hole
must not continue consuming unpublished capacity: otherwise a slow early source
still stops replenishment after at most H later completions. Active and
finished-but-not-yet-admitted jobs remain bounded; ordinary published records
and ledger entries grow with actual admitted work under the RAM policy.

Retain per-ticket replay progress, frontier details and optional-refusal state
for bounded active contexts. A smallest first version can explicitly reject
combining the new policy with physical subdivision; if supported, physical-parent
progress also belongs to its ticket context. Reuse `Local::Published`
for native completion holes and make delegated publication explicit rather than
inferring it solely from the cursor. Replace fence-based transfer/dispatch tests
with the actual reservation state; preserve forward-only aliases and protected
initial-anchor dependencies. Poll ready contexts fairly and keep each source's
event order. Native-returned escrow is not the same as published completion.

At checkpoints, freeze the coordinator transaction boundary while workers may
backpressure. Persist the global admission effects, reservations, published
holes and every accepted source prefix together. An event's effects and its
replay cursor must never be saved on opposite sides of a transaction. Resume
redispatches only unfinished duties and suppresses their accepted prefixes,
including rebuilding source-local reuse in stream order. Do not silently load
this state through the old single-prefix CP1 contract: bind a new policy/format
explicitly. Keep non-cancellation failures sticky across cancellation and never
turn a cancelled prefix into successful source completion.

Counters currently derived from the cursor must become actual published counts;
the cursor is only a watermark. Domain IDs, representative choices and record
publication order may legitimately differ between runs. Preserve exact source
coverage, failure/frontier meaning and final obligations, not artificial
bit-identical IDs or work counts. Completion still requires all duties and
delivery/pool buffers to be discharged, with no hidden error.

A decisive regression test holds an early source open while **more than H**
later sources of that same owner finish and publish. It must show replenishment
and bounded active contexts, then checkpoint with multiple accepted partial
prefixes and completed holes, restart, and finish without lost or duplicated
logical effects. Pair this with reservation/retirement races, non-head failures
and initial-anchor overlap; test both physical-part interruption boundaries if
the combined policy is supported. Measure
the wall-time/CPU/RAM tradeoff before recommending a live campaign change.

## Acceptance conditions for any proposed change

Keep this separate from the running campaign until controlled evidence exists.
Required tests include interleaved same-owner sources, duplicate admissions,
stale prepared lookups, transfer racing reservation, failed later results after
an earlier cancellation, cross-owner cycles, and bounded-buffer progress. Real
multiworker interruption/restart must cover several partially accepted sources
and completed holes, without fabricated completion or repeated logical effects.
Subdivision tests must establish exact source coverage and honest part failure.

Compare complete required coverage, frontier/error semantics and discharged
responsibilities; do not require identical IDs or work counters after changing
admission order. Different valid covers can cause different optional-resource
refusals or extra out-of-scope frontiers, so success is not automatically
scheduling-invariant under finite per-operation guards. Measure total work,
native/admission CPU, ready/blocked/held jobs, RAM and checkpoint cost on the
same workload; higher worker occupancy alone is not evidence of faster closure.

No scheduler change by itself proves that the five-loop abstract worklist is
finite, guarantees a speedup, or promotes candidate exhaustion to a family
closure certificate.
