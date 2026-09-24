# All-owner regional sharing: a stopped comparison, not a speedup result

24 September 2026. Two release controls were run sequentially against the same
67 saved owners. Both stopped at their prospective scheduled-work diagnostic
gate. No elapsed deadline, memory limit, missing-rule frontier or crash ended
either run. They provide bottleneck evidence, **not completion, a speed ratio,
or a full-family ETA**. No new rules were generated.

## Required scope and the intentional input difference

The common required entry set E is A≤12, R≤3, A−R≥9, with 2,123,560 distinct
sector-labelled tuples across 67 original regions. This bounded diagnostic is
not the full A≤24, R≤15, A−R≥9 objective. All descendants remain obligations;
entry limits are never descendant cutoffs.

- Direct: the 67 required Apply regions, 18,353 input bytes.
- Anchored: 339 additional Apply anchors and 57,215 Route anchors, followed by
  the identical 67 required regions: 57,621 queries, 16,139,743 input bytes.
  The anchors have P=A+R≤13. Their union with E contains 999,539,725 labelled
  tuples. All 1,770,085 required tuples outside P13 remain in the input.

Thus this compares strategies for the same required E, not identical native
workloads. Checking extra anchors is charged work; an anchor gap would not
automatically refute E. Offline construction/counting and pre/post input hashes
are outside the command timer. Native input parsing, loading, route verification,
admission, traversal and output are inside it.

Both controls use the same frozen release CLI, saved programs and routing
manifest, Ordered publication and W50 default split: 25 inspectors, 24 admission
helpers, one coordinator. Native affinity is CPUs 0–49; the observer uses CPU50;
nested compute pools are one. The cores are not exclusively reserved. Both
request 57,621 queries/32 MiB, 450/500 GB soft/hard supervised memory limits
and a 480 GB owned-child address-space limit. These are allowances, not usage.

Equal cooperative stop gates are 375,000 scheduled responsibilities, 250,000
returned inspections (including uncommitted/cancelled), or 15,000,000 committed
events. Native hard caps
remain 500,000/50,000,000. Polling and cooperative cancellation permit recorded
overshoot; no cap was raised and neither run was retried. There is no elapsed
timeout. A cancelled prefix is not a mathematical failure or an accepted terminal.

## Measurements

| Metric | Direct E | Fixed anchors + E |
|---|---:|---:|
| Outcome | Work-gate incomplete | Work-gate incomplete |
| Native preparation | 102.714 s | 101.967 s |
| Traversal through cancellation/cleanup | 17.767 s | 119.836 s |
| Supervised-command wall, including drain | 124.309 s | 227.617 s |
| Command user + system CPU | 186.46 s | 626.80 s |
| GNU peak RSS | 6,409,600 KiB | 8,054,600 KiB |
| Sampled busy cores in traversal-labelled intervals | 4.16 | 4.29 |
| Scheduled count when stop was requested | 396,999 | 376,973 |
| Final scheduled responsibilities | 457,176 | 382,970 |
| Final queued responsibilities | 457,115 | 268,836 |
| Committed native inspections | 61 | 105,514 |
| Returned inspections, including uncommitted/cancelled | 302 | 105,718 |
| Finished but uncommitted inspections | 241 | 204 |
| Committed events | 2,175,919 | 11,768,009 |
| Mathematical frontiers observed | 0 | 0 |
| Cooperative cancellations | 1 | 1 |
| Initial Apply index entries/usable anchors | 67 / 67 | 406 / 406 |

The anchored index examined all 57,621 inputs and retained every eligible Apply
entry under the unchanged index limits. All original E and extra-anchor handles
were published; the direct prefix stopped with six initial handles still
unpublished. Publication of a root does **not** discharge its descendants. The
anchored ledger still has 49,401 pending delegated obligations and 224,554 pending
native publications. Neither strategy exhausted its worklist.

The anchored prefix contains 834 native Apply records (including 71 partial
initial inspections) and 104,680 Route records, plus delegated publications. Its support census
reports 3,436,885 same-support and 4,623,076 strict-subset successors, with zero
unsupported or conditional-unsupported transitions observed. Direct reports
982,237 and 1,032,219 respectively, likewise zero unsupported. These stopped
prefix counters are not global support/termination proofs or original-IBP replay.
For example, direct's 2,014,456 support observations exceed its 2,009,861 committed
successors because the cancelled inspection includes attempted, unpublished work.
Published domain records reach represented P=15/16; these are geometric
over-cover extrema, not individual concrete reachability witnesses or maxima
over all pending regions. The direct prefix has no published descendants yet.
Descendants are never clipped to P13.

The much larger anchored inspection count at the stop is **not** a speedup
factor: the added domains, cancellation prefixes, phase mix and remaining work
differ. Likewise its longer wall time is not evidence of slower completion of E.
Only completed matched required scope can establish that comparison.

## What the run says about parallelism

Cold preparation is predominantly serial. During traversal, inspectors account
for about 2.31/2.17 sampled busy cores, admission helpers 1.49/1.86, and the
coordinator 0.34/0.26. These are duration-weighted heartbeat-labelled samples,
not an exact exclusive CPU profile. Active worker slots include blocked workers.

In the anchored trace, commit ID1 persists from 118.20 to 129.26 seconds while
events advance from 398,310 to 752,674. Over that interval the measured commit
wall grows only 0.694 s and helper-preparation wall 0.183 s. The corresponding
approximately 12.1-second sample bin averages about 23.9 blocked inspectors.
This is consistent with a productive earlier stream and later producers waiting;
it does not demonstrate coordinator CPU saturation or identify the head's thread.

A different interval at commit ID338 has 255 completed escrow entries, zero
blocked producers and about 1.57 busy cores. There the lookahead/head dependency
persists after later jobs finish. Larger running buffers alone cannot remove
that barrier. Commit/preparation wall timers overlap worker execution and cannot
be added into an Amdahl serial fraction.

Runqueue delay is also present on this shared host, particularly for admission
helpers, so scheduling interference cannot be assumed absent. It does not erase
the explicit publication waits. A post-run read of this session and its ancestor
cgroups reports no CPU quota and no throttling; that read is not a measurement
of exclusive CPU availability during the pair.

## Consequences for the architectural review

1. Keep regional sharing as a candidate, but do not extrapolate the successful
   four-owner A12 speedup to all owners. This broader pilot remains incomplete.
2. Before a large rewrite, consider a matched bounded-outbox experiment. Running
   visitors have one published and one private chunk, independently capped by
   16,384 records, 1,048,576 logical events and 8 MiB. The separate 8 GiB completed
   escrow cannot drain a still-running producer's second flush. The traces do
   not identify which chunk limit binds. Spare RSS allowance alone proves no
   benefit; extra buffering can merely retain more speculative work.
3. Exact entry shards remain a genuine competing decomposition, but the critic
   identifies a concrete problem with the previously suggested experiment:
   splitting E while duplicating all fixed anchors leaves the same expensive
   early anchor in both shards. It would not split that observed bottleneck.
   Do not promote that unchanged experiment on these results. Splitting or
   demand-selecting the anchors needs its own explicit unchanged-E design, with
   every descendant retained and all duplicated work/loading charged under one
   aggregate CPU/RAM budget.
4. Micro-epoch/batched publication and guarded relational reachability remain
   more radical options. A barrier can worsen a long-head problem, and exact
   predicate sharing can replace domain growth with formula growth. Neither is
   justified as a production replacement by these stopped measurements.

The [architecture proposal](radical_parallel_architecture_2026-09-24.md) and
[independent critique](radical_parallel_critique_2026-09-24.md) give the correctness
conditions and counterexamples. No full-envelope restart, additional rule search,
new terminals or scope reduction follows from this pair alone.

The next discriminating measurement is a separate native stack profile of the
expensive initial Apply phase, especially IDs 0–3, using the same frozen input
and executable with the existing resource/stop policy. Charge cold setup again
and do not mix profiling timings into the control table. It should distinguish
productive native matching/projection/coefficient work from helper scheduling
overhead before choosing an outbox or work-decomposition change.

## Reproduction and evidence

- Source milestone: `9e9e0b24ada7e6b29a162b07ba596c8c9ddb2abb`.
- Frozen CLI SHA256:
  `254d7e1eefaacf0f20d76ce641836df62d47ff7daef7d45c5be0037530da9e8b`.
- Input construction/count audit: `TMP/all67-regional-a12.wXo3pZ/`.
- Runner and authoritative preflight: `TMP/all67-regional-pair.zVU72K/`,
  `prepared-reviewed/`; older preparation drafts are not the frozen run.
- Independent raw review: `INDEPENDENT_RAW_AUDIT.md` in that runner directory;
  separate performance observations: `TMP/all67-performance-observer.wPENvq/OBSERVATIONS.md`.
- Raw outputs: `run/baseline-e/receipts/shared-owner-campaign.h5113epk/` and
  `run/anchored-e/receipts/shared-owner-campaign.jirzv3wy/` beneath that directory.
- Per-control summaries, GNU times, resource/thread samples, gate requests,
  input hashes and `comparison.json` are retained locally. TMP evidence is not
  a portable artifact dependency and is not committed.

The outer runner exited zero after recording the two expected native exit-4
incomplete results. That outer success is not solver success. No native run
remains live; no build or engine change was made for this comparison.
Independent raw review passes for the classification as two censored controls:
all 90 input/source hashes and both before/after sets agree, actual resource
settings match the plan, and the reported counts agree with native receipts.
This is an audit of the measurements and their limits, not a closure certificate.
