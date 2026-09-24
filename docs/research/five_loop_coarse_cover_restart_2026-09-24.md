# Five-loop coarse-cover restart, 24 September 2026

## Outcome and unchanged mathematical scope

The analogous four-loop entry-and-descendant controls completed twice. The new
five-loop campaign was then actually launched in Zellij session `rustred`, tab
`five_loop_vacuum`, pane `terminal_0`, at 21:06:56 UTC. This note records launch
and initial progress, **not five-loop closure**.

All 67 original five-loop A<=24, R<=15, D=A-R>=9 entry queries remain required.
The same 67 saved owner programs are copied byte-exact; no IBPs, ordering,
coefficient representations or rule scope changed. Additional ordinary native
query obligations provide a coarser reusable cover. Every escaping RHS and
Route dependency remains required; anchor bounds never clip descendants.

The fresh ignored input directory is
`campaigns/five-loop-saved-coarse-cover/inputs`. It contains 1,280,854,595 owner
payload bytes, the original query bytes in `queries-original.json`, and 134
queries in `queries.json` (91,246 bytes):

- the unchanged original 67 entries, in their original order;
- 67 rank-15 nonnegative orthant anchors, with no D bounds;
- of those anchors, 54 owners with at least eight active lines also receive
  A<=24; the other 13 have no positive-power bound.

The native query input is SHA256
`94a6941b585e559b567281fa02c9ec043604f8efe1aaf97f965468bdcd6ff4e8`.
The selection is unchanged, SHA256
`d2667dc9c761d1dcd171408452eea030c0b1eae23421e55407745d046deec3f0`.
Both old and new campaigns use the exact frozen executable SHA256
`5746feb1630341b5d4cd8a7c211c823b0c8d96c6813d28f164f4ae0b91c248bb`.

## Evidence for the input change

The four-loop full-UV-jet analogue is A<=19, R<=12, D>=7, over all 900 saved
H/X/BMW/FG owners, not merely four top-level roots. Finite entries alone caused
a growing fragmented-domain admission queue. Coarse auxiliary inputs completed
all four families without dropping descendants. FG/H/X used R12 orthants; BMW
needed A19 on auxiliary owners with support>=6. Two complete W6 passes had
whole-command sums 112.06 and 109.94 seconds. Independent strict comparison
matched every native/domain geometry, mathematical counter, guard-refusal
record and ledger obligation. Every result had zero frontiers/errors/pending
work and a discharged ledger. These are shared-host controls, not a claim of
optimal ordering, isolated speedup, or five-loop termination.

The five-loop source diagnostic first classified the 67 original entries and
67 R15/no-A orthants without following RHSs. All original entries were locally
clear. Seven broad anchors had 62 unresolved guard pieces (3 equalities and
59 excluded conjunctions), with minimum support cardinality eight. There were
no exact gaps, invalid source conditions, native errors or materialization
refusals. Current Apply/Route contracts do not increase support cardinality,
which motivates the >=8 upstream input fallback; this is not an A/R invariant
or a proof of global routed termination.

A second diagnostic checked the actual staged 54 modified anchors: all 54 were
locally applicable, with 175,082 pieces and zero unresolved/gap/invalid/error
records. Independent audits matched the staged payload hashes, original input
bytes and every changed query. Together the two diagnostics establish local
dispatch coverage of all 134 final inputs, **not RHS/descent/routed closure**.
Their existing matching-only result-materialization allowance was one million
pieces; neither hit it. The production recursive walk has uncapped cumulative
work, not this diagnostic output allowance.

Raw evidence is retained locally under
`TMP/four-loop-saved-descendants.VaNmUN/RESULTS.md`,
`TMP/four-loop-saved-descendants.VaNmUN/REPEAT2_INDEPENDENT_AUDIT.md`, and
`TMP/five-loop-anchor-diagnostic.2rBzX9/{RESULTS,INDEPENDENT_AUDIT}.md`.

## Old campaign remains paused and recoverable

The original `campaigns/five-loop-saved` process received a user-authorized
cooperative stop, not deletion or a change to its input. Its final receipt is
`runs/20260924T092912.681203Z`, state `paused`, exit 4 at 20:46:26 UTC.
Checkpoint generation 13 retains:

- 15,030,002 completed native inspections;
- 33,380,033 committed logical domains and 27,816,081 pending domains;
- 3,198,178,283 committed events and zero reported frontiers;
- 39,286,209,131 checkpoint bytes, saved in 398.735 seconds.

The original checkpoint directory is
`campaigns/five-loop-saved/checkpoints/main`. The old supervisor/native PIDs
1360082/1360164 exited. This state is not silently resumed with changed inputs:
the new coarse-cover campaign has its own input, frozen executable copy,
receipts and checkpoint directory. The old all-67 initial-entry publication
was not descendant closure.

## Actual launch, resource policy and recovery

From the repository root, with the license inherited only in the environment,
the exact outer command already issued in the Zellij pane was:

```sh
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/five-loop-saved-coarse-cover \
  --executable campaigns/five-loop-saved/bin/rustred-5746feb1630341b5d4cd8a7c211c823b0c8d96c6813d28f164f4ae0b91c248bb \
  --workers 50 --max-memory-bytes 500000000000 \
  --ram-guard-margin-percent 5 --checkpoint-interval-seconds 3600 --start
```

Do not issue a second start while this campaign is active. The persistent
`active-run.json` records the exact inner command and hashes. The actual run is
`campaigns/five-loop-saved-coarse-cover/runs/20260924T210656.815896Z`.
It uses W50 on CPU0-49 (25 inspectors, 24 admission helpers and one coordinator),
Ordered/H256 publication, initial-D reuse, route overcover, finite-axis
refinement and degree64 guards. Physical subdivision remains off. All nested
thread-pool settings were independently observed as one; every sampled native
thread had affinity CPU0-49.

Requested and admitted RSS limits are 500 GB hard and 475 GB cooperative
save-and-stop, with host/cgroup protection. No address-space limit or elapsed
deadline is installed. The monitor's 15-hour objective is informational, not a
timeout. `--unbounded-work` removes cumulative enumeration cutoffs; bounded
buffers and native scratch/algebra admission remain explicit.

Checkpoint interval is 3,600 seconds. Ctrl-C in the owning pane requests a
cooperative saved pause; wait for that terminal state before resuming. The
new checkpoint directory is `campaigns/five-loop-saved-coarse-cover/checkpoints/main`.
After a pause, resume the exact frozen binary and input policy with:

```sh
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/five-loop-saved-coarse-cover --resume --start
```

Read-only monitoring, without signalling or restarting the solver:

```sh
nix develop --command python examples/python/campaign_monitor.py \
  campaigns/five-loop-saved-coarse-cover/runs/20260924T210656.815896Z --once
nix develop --command python examples/python/campaign_monitor.py \
  campaigns/five-loop-saved-coarse-cover/runs/20260924T210656.815896Z --json
```

Each resume creates a fresh receipt directory; obtain its path from
`campaigns/five-loop-saved-coarse-cover/active-run.json`. Omit `--once`/`--json`
for the live read-only display. See [driver instructions](../shared_owner_campaign_driver.md)
for exact RAM override and checkpoint integrity semantics.

## Independently observed initial state, not closure

Actual same-boot identities, boot ID
`02fd9278-273f-4b64-852c-0e9e9ac339fa`:

| Process | PID | Start ticks |
| --- | ---: | ---: |
| Supervisor | 3520232 | 177123899 |
| Native | 3521352 | 177124309 |

At 21:08:30 UTC (94.108 seconds after supervisor start), preparation had
finished and traversal was active: one of 134 initial responsibilities was
published, 78,612 logical domains were scheduled, 78,611 were pending, and no
frontiers were reported. Aggregate RSS was 6,155,378,688 bytes. Generation 2
was a real initial-state checkpoint; generation 1 was only preparation bootstrap.
Subsequent queue growth is expected to change this snapshot. Worker reservation
is not measured CPU utilization, and an initial-entry bar is not a closure
fraction or a basis for an ETA.

At approximately 128 seconds, the monitor showed eight of 134 initial
responsibilities published, roughly 598,000 queued domains, zero frontiers,
about 6.8 GB RSS and 2.7 sampled busy cores. The growing backlog means the
four-loop improvement is not yet an established five-loop speedup.

Final acceptance requires actual terminal native receipts: zero errors, failed
or unresolved frontiers, zero pending/uncommitted work, recursive worklist
exhaustion, all scheduled obligations and ledger dependencies discharged, and
drained worker/escrow state. In particular, every native
`unsupported_support_successors` and `conditional_unsupported_support_successors`
count must be zero. A matching-only pass, zero currently observed frontiers or
fully published initial entries does not establish those terminal conditions.
