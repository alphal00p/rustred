# S1: Ready width scaling, 2026-09-30

The original combined four-loop control rejects naive widening from W16 to W50:
W16 completed native plus full cold proof; W50 was cooperatively stopped as a
performance negative after already doing 52.93 times as many native inspections.
This is a censored performance result, not a correctness failure or a completed
W50 closure comparison. The representative finite five-loop W16 baseline passed;
the planned finite W50 arm was deferred by root. No 200-core utilization or
full-five-loop closure claim follows.

## Frozen inputs, tools and execution

Native source `1b33ad2956d87f29db6ef9987542f3364f443814`, executable
`TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-1b33ad29`, SHA256
`73253922552ef341e3e97522d9481a4e369d388e9f8612c4ac92468c3c584a14`.
Python tools are the separate frozen `python-delivery-62c763cd` tree, commit
`62c763cd974fb662458d5dc2d281e0306aa887b7`. No solver or source implementation
changed in S1.

Registration and exact argv arrays: `TMP/codex-s1-scaling.69wxuY/PLAN.md`,
`matrix.json` (SHA256
`dec7b32e142f73528d89bb8ddf1c56c69370ad42d0a9520abde57efa1667fc60`) and
`commands/{00-four-all-ready-w16,01-four-all-ready-w50,02-five-finite-ready-w16}/`.
The existing Ready runner uses G2 Union, lookahead256, unchanged native levers,
hourly checkpointing and fresh outputs. W16 is CPU32–47 with 8 inspectors,
7 lookup helpers and 1 coordinator; W50 is CPU32–81 with 25+24+1. These are
physical CPUs, outside LC2's protected128–227. W50 also spans an additional NUMA
node, so this is the actual width/partition/placement change, not a pure fixed-work
core-count comparison. LC2 was untouched.

4L preserves all original58 Required/0 Auxiliary rows,16 original programs and
508 routes. Query SHA256:
`c129f38836a1fd60a537cba1e3873a88bb3880b561f4b3a67c8f8aa404bc7921`.
5L preserves all67 programs and8246 route objects but requests exactly the
existing single finite query `five-loop-finite-publication-r2-a11`, owner
`011101110111000`, R<=2/A<=11/D>=9/coordinate upper15, one Required/zero Auxiliary.
Its query SHA256 is
`e5a1986fb5a19322bd26e5101e84699c09ea8541396dea642872a482e5a45063`.
Loading67 programs is not query coverage of all67 owners or the full production
183-input/116-Required scope.

Each arm retained its original1800-second clock including preparation/admission,
shutdown, proof and reporting; native900+300 grace, explicit cold
`--reinspect all --certification-scope all-roots --require-closure`,300+60 seconds.
Start/live host headroom floors were250/150GiB. Every owned group was independently
found absent and both resource locks reacquired/released before handoff.

## Actual outcomes

| Scope / width | Native guarded wall | Cold guarded wall | Native + cold | Discovered domains / native inspections | Outcome |
|---|---:|---:|---:|---:|---|
| Combined4L W16 | 9.578s | 15.163536s | 24.741536s | 66,914 /24,259 | All58 queries,32/32 roots PASS |
| Combined4L W50 | 250.373s at stop | Not run | Not completed | 1,892,833 /1,284,105 | CENSORED_PERFORMANCE_STOP |
| Finite5L W16 | 172.858s | 155.211180s | 328.069180s | 966,384 /760,033 | Exact1 query,1/1 root PASS |
| Finite5L W50 | Not run | Not run | Not measured | — | Deferred by root |

Native wall is the existing runner's launcher-through-owned-group-drain boundary;
lock admission and recorder teardown are separately retained. Cold wall is the
existing guard's child-through-drain boundary. Complete cold proofs re-inspected
all24,259 and760,033 native candidates respectively, had zero violations and
left every checkpoint file unchanged. Four-loop W16's paired Python audit also
passed in6.153525s. Five-loop secondary Python audit was explicitly NOT RUN under
root's retained prior-censor policy: native+cold PASS is not full downstream
artifact-proof acceptance.

Root requested the W50 stop after clear work amplification. Only the verified
owned ArmGuard runner PID2423735 received SIGINT; it wrote its cooperative stop,
saved a paused checkpoint and drained native PG2423736 without SIGKILL. Raw native
exit4, `censored=true`, `stop_reason=operator_signal_2` remain intact. At stop:
13,974 queued/pending domains, cached recursively closed712,432/1,892,833,
1,180,401 discovered-minus-closed,18/32 initial roots cached closed, zero frontiers.
These cached counts are not an independent cold proof. Its250.373s and1,284,105
inspections are lower bounds for the unfinished arm, not a completed speed ratio.

## Actual utilization and costs

Native CPU seconds / average busy CPUs were32.240274 /3.366 for4L W16,
1683.532864 /6.724 for censored4L W50, and555.213594 /3.212 for finite5L W16.
Peak measured process-tree RSS was241,467,392B,1,342,500,864B and6,234,255,360B.
These are observed work rates, not the reserved16/50 cores. The short W16 4L run
had no10-second recorder sample. W50 had24 samples averaging6.801 own busy CPUs
and3.050 foreign busy CPUs; finiteW16 had17 samples averaging3.221 own and1.008
foreign. Allocation does not establish utilization or justify200-core extrapolation.

4L W16 preparation/traversal were1.618453/5.192874s; W50's unfinished values were
1.609937/246.491302s. W50 coordinator ordered commit consumed109.090546s and
admission preparation42.449314s, versus1.852209/1.246308s at W16. Its final save
was0.873947s versus0.047153s. The changed dynamic traversal is the dominant
observed difference; more worker busyness is not evidence of useful throughput.

Finite W16 preparation/traversal were83.302704/73.195852s. Its recorder observed
about one own busy CPU for the first80 seconds, then roughly6.1–6.6 during the
main traversal. Coordinator ordered commit/admission preparation consumed
26.048085/12.162878s; final save0.371527s. Cold internal preparation/checks/
reinspection were84.413385/29.086184/29.028748s. Cold CPU was581.015882s,
3.743 average busy CPUs, peak single-child RSS6,164,316KiB. The native/cold phase
and all raw counters are retained, without attributing the total to inspection.

The4L pair had no competing experimental native job. FiniteW16 native drained
around05:06:53 UTC before P3 began05:07:45.992. Its cold guard interval was Unix
1790744879.495063–1790745034.706243 and overlapped disjoint CPU64–79 P3 default
generation/walk/cold and sparse generation. Thus the finite cold/combined timing
is explicitly shared-host exploratory, not an isolated speed qualification.
LC2 host contention remained possible throughout.

## Preserved setup and test caveats

The first4L payload checker mistakenly used stale informational source-format
SHA fields from the unchanged selection document. It failed before native work
in1.148382s. All16 actual bytes then matched the previously verified E1 pins and
format6 conversion destination hashes. `preflight-correction.json` and the failed
receipt remain; no input was changed, no clock reset. The native outer allowance
was reduced to790.106s solely to retain the same absolute deadline and full later
proof/drain reserves; it completed in9.578s. Future4L checks used the corrected
destination hashes. All67 original5L checksum pins already matched conversion
destination hashes, and the actual checksum phase passed in3.153531s.

Whole-arm elapsed through result/drain reporting was322.334s,359.894s and496.173s;
these include setup/coordination/reporting, not merely the primary timing sum.
The fourth registered arm has no clock or output and is retained as unexecuted.

The frozen1b33 native app suite remains1139 passed,1 stale diagnostic-string
assertion failed,12 ignored, zero internal skips. All new rolling/cut/replay and
W50-capable tests passed. A separately audited10-phase public-CLI equivalent
regression passed against the same optimized binary; the corrected Rust unit was
not re-executed. This distinction is not erased by S1 cold success.

Small result receipts are `TMP/codex-s1-scaling.69wxuY/result-00-four-all-ready-w16.json`,
`result-01-four-all-ready-w50.json` and `result-02-five-finite-ready-w16.json`.
Raw commands, native metrics/events/results, cold reports, checkpoint hashes,
guard exits and process-group drain evidence are linked from those receipts.
