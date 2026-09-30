# Bounded ready-result publication rescue

Status: implementation authorized; no new native build, performance result or
production action. The measured executable remains frozen at `3428b519`.

## Mechanism and retained evidence

The final original combined four-loop control is 11.41% slower than Ready;
finite/hot five-loop controls are 25.57%/20.50% slower at W16/W12. The two
reindexed four-loop pairs have mixed signs (mean 1.23% lower for Epoch), not a
robust scheduler win. Cut-one, AllMiss, adaptive dispatch and query-order
experiments did not establish a remedy. W50 four-loop correctness passes;
neither W50 nor W200 five-loop performance has been measured.

The worker pool already pulls jobs from one shared queue. The rolling
controller instead constrains publication: all oldest 16 issued results must
arrive before the next cut, and returned-but-unpublished results still occupy
the bounded window. No global worker join occurs at ordinary publication.
Two leased lookup replicas can additionally delay refill or publication.
The existing `inspect` timer combines receipt collection, prefix waiting and
replica backpressure; it is not an isolated CAS or head-of-line measurement.
The saved finite/hot heartbeat samples often show no queued work and many
returned slots while pending domains remain. Those rate-limited post-poll
samples establish that such states occur, not their fraction of elapsed time.

## Minimal opt-in change

Keep the default `oldest-prefix`, cut 16 and automatic inspection-plus-cut
window. Add explicitly bound fresh-campaign options:

- `--epoch-publication-order oldest-prefix|oldest-ready`;
- `--epoch-cut-size N` and `--epoch-window N`, each bounded by 4096;
- oldest-ready chooses a full cut of the lowest completed sequence numbers,
  rather than waiting for an unfinished lower sequence. Tail/replay drain
  must remain live; it must not turn ordinary work into eager one-job cuts.

Reuse the existing pool and P1/P2/P3 proof checks, exact containment, G2
visibility, checkpoint inventory and stop/restore machinery. Publication
order and effective cut/window must be reported and persisted. Defaults must
retain the old request identity where feasible; incompatible explicit resume
choices are refused, never silently reinterpreted. A saved rolling window
remains fixed even if the resumed worker width changes. No CAS, graph
canonicalizer, physics-domain restriction or new positive proof shortcut.

Bounded draining of already available channel messages may reduce transport
overhead. Small diagnostic counters must distinguish receipt waiting and
publication/refill backpressure without introducing a new monitoring system.
The implementation records invocation-local snapshot-refresh elapsed time,
ready-channel drain time, blocking receipt waits and publication-backpressure
waits separately. These are coordinator observations, not worker CPU or
exclusive whole-run phases; W1 receipt polling executes native work inline.
Oldest-ready sorts the currently returned sequences, but arrival timing can
change which full cut is available. It does not promise cross-run graph identity.

Default CP6 scalars omit the new publication-order field, and an explicit cut
16 preserves the old request hash. A nondefault publication order or explicit
cut is request-bound; saved effective cut and window are authenticated scalars.
Runtime restore requires the same effective cut and publication policy; only
an omitted window inherits. Cold verification compares explicit cut/window and
the publication policy, plus the request digest: historical environment-only
diagnostic cuts remain verifiable without reproducing that environment, while
new public custom cuts are additionally digest-bound. Inline W1 retains the historical
automatic window of one; an explicit window is at least the requested cut.

## Acceptance and falsifier

First test a held oldest sequence with at least 16 later completed jobs:
oldest-ready must publish and refill before that job is released, while the
default retains its prefix rule. Exercise save/stop with that job held,
partial replay, exact inventory, stale snapshots, G2/quarantine and W1.
Author and independent reviewer must check durable policy binding and default
compatibility before a resource-authorized consolidated build.
Prepared source tests cover actual nonprefix P3/refill with held sequence zero
under both lookup modes, durable holes and W1 replay; unchanged prefix waiting;
full-cut/tail selection; policy/cut/window refusal before session adoption;
valid-but-wrong cold scalar binding; and real Union/rescue/quarantine checks in
both default and oldest-ready modes. Formatting passed; native compilation and
execution remain pending independent review and the resource handoff.
A bounded synthetic lifecycle test also creates 199 real inspector threads
plus the coordinator, authenticates all worker callbacks, drains/retire/refills
the automatic 215-slot window, and cancels/joins. This is pool capacity and
safe-stop coverage only, not physical-core licensing or throughput evidence.

Use the same optimized binary for at most three initial full combined-58
cells: default prefix/window, prefix with a larger bounded window, and
oldest-ready with the default window; keep cut 16. Preserve every query/role,
payload, cold-All check, RAM guard and whole-pilot limit. Compare native plus
cold wall, work, CPU/RSS and waiting diagnostics, not just occupied threads.
Only a successful mechanism and scope gate warrants matched finite/hot
follow-up. A lower waiting counter without lower whole verified wall, or
expanded work that loses the gain, rejects the performance hypothesis.
W200 is a separate capacity/scaling question, never a forecast from W12/W16.
LC2 remains untouched; only its owner may pause it for a later allocation.
