# LC1 launch: configuration, rehearsal and the owner's command (2026-09-28)

Owner direction: orchestrator decisions 12-18 (`TMP/progress/orchestrator_decisions.md`), in particular decision 18
(~16:45 UTC), which reduces Launch Candidate 1 (LC1) to what is already stable. The binary and its gates are in
`docs/research/fable51_lc1_ship_2026-09-28.md` (commit 379f9115). Labels: **[M]** measured (receipt cited),
**[E]** estimate or interpretation. Nothing here is an ETA, a finishability claim or a closure claim;
`family_closure_claim` stays false in every receipt. No existing campaign directory was started, stopped,
signalled or written for this note. The new directory `campaigns/five-loop-qcd-feynman-d9d10-lc1` was prepared and
**not started**.

## 0. The launch command

In Zellij session `rustred`, tab `fable_5_1`, in a plain shell (do **not** wrap it in `taskset`; the launcher checks
the shell's affinity against CPUs 128-227):

```sh
cd /common/dev/rustred && env TMPDIR=/common/dev/rustred/TMP nix develop --command python examples/python/production_saved_owner_campaign.py --campaign-directory campaigns/five-loop-qcd-feynman-d9d10-lc1 --start
```

Everything else (binary, inputs, workers, CPUs, policies, RAM guard, checkpoint interval) is frozen in
`campaigns/five-loop-qcd-feynman-d9d10-lc1/bin/steering.json`; the start command takes no other flags. It prints
`Campaign receipts: campaigns/five-loop-qcd-feynman-d9d10-lc1/runs/<UTC timestamp>` (the run directory for the
monitor). If it also prints `RAM admission: effective hard cap ...`, host MemAvailable was below 650 GB at launch
and the admitted cap is lower than 600 GB (see section 4).

Readiness [M]: the prepared directory passes the launcher's validation mode (section 4); the rehearsal on an
identical copy (section 5) started with this command, wrote a periodic and a pause checkpoint, paused on SIGINT
(Ctrl-C) and on the stop file with exit 4, resumed with `--resume --start` in ~110 s, kept 0 frontiers throughout,
peaked at 10.2 GB RSS, and recorded the RAM guard (600 GB cap, 50 GB host floor) in its receipts.

## 1. Scope

In LC1 (all on `fable_5_1`, see the ship note, section 1): the legacy owner-domain engine with the SoA admission
kernel and O(1) storage telemetry, kernel2 (retire compaction fix 71062d77, watermark shortcuts 43269bc2),
`--frontier-policy stop`, the host-aware RAM guard with the 50 GB host floor, the flaky-test fixes and license
markers, and `[profile.campaign]` (fat LTO, one codegen unit, no mimalloc).

Deliberately **not** in LC1 (decision 18): production G2' residual anchors, the automatic resume-time frontier
rescue, the epoch engine (walk semantics 3 / CP6), I1, I2 (its rebuild failed its gate, decision 16), D6, per-CCX
reducer replicas, and the upstream Symbolica fixes (decision 15: LC1 runs on vendored dev 953e26e2 plus the local
heap-pow patch). These continue as upgrades: a performance-only binary swap of the paused campaign
(`--resume --upgrade-executable`) if the checkpoint binding allows it, otherwise the next campaign attempt.

Consequence of the missing rescue: a frontier stops the campaign (exit 4, stop reason `frontier_policy`) instead
of ending it; the checkpoint keeps all certified progress, and `--resume --start` continues to the next new
frontier (driver doc, "frontier policy"). v2 on the same inputs ended its 18.8 h run (paused, exit 4) with 0
frontiers [M, v2 `runs/20260926T151353.794886Z/status.json`, read-only].

## 2. Frozen binary [M]

| | |
|---|---|
| launch binary | `TMP/fable51-controls/bin/rustred-lc1-4606cc4b` (`.meta`, `.provenance.json` beside it) |
| sha256 | `4606cc4b4a5420c54cb1b9afa833d2be2400be831c5628542a3077fb10d53c06` |
| frozen copy | `campaigns/five-loop-qcd-feynman-d9d10-lc1/bin/rustred-4606cc4b...c06` (sha256 re-checked after the copy) |
| source | 43269bc2 (kernel2), merged into `fable_5_1` as 98578d83 with identical build inputs; `[profile.campaign]` |
| Symbolica | gitlink 953e26e2 + working-tree heap-pow patch (diff sha256 629752e7) |
| `walk-semantics-version` (frozen copy) | `{"walk_semantics_version":1,"checkpoint_format":"RUSTRED-WALK-CP5","checkpoint_schema":5}` |
| native checkpoint executable id | `3ae3d9c4...1dfb` (the native's own digest of the executable, recorded in every CP5 generation of the rehearsal) |

## 3. Inputs [M]

Prepared exactly as v2 (`campaigns/five-loop-qcd-feynman-d9d10-v2/inputs/input-receipt.json`, read-only): owners
and selection copied from `campaigns/five-loop-dependency-closure` (read-only source), plan-v3 queries with the
original witnesses, the same three attachments, query order `preserve`. The LC1 input receipt equals v2's in
selection, queries, all 67 owner digests, owner bytes and attachment digests (compared field by field).

| input | sha256 |
|---|---|
| `inputs/queries.json` (from `examples/input/five_loop_qcd_feynman_d9d10/queries.json`; 183 queries, 121,424 B) | `2c7148601436ebd1e50f7c13d922859cdb5d296f5584f33d266f0f866d27204f` |
| `inputs/selection.json` (67 owners, 1,280,854,595 B of owner payloads) | `d2667dc9c761d1dcd171408452eea030c0b1eae23421e55407745d046deec3f0` |
| `inputs/entry-plan-receipt.json` | `a20312ca7105332086eecc56912c7f7daea9ec3a26f1915a5f6c82103c5d2da7` |
| `inputs/skeleton-classification.json` | `6f5a6c5bb8b0ceee0bff412dd45130d07baa002f39c6a1edb83d761ed2bfb70f` |
| `inputs/matching-summary.json` | `98aeba7de1509de043b318985cddad9cddbfcc2f6530dc04cb8c4ccf50904909` |
| `bin/steering.json` (`rustred.production-steering.v3`) | `89dc6c97e1b5deea2cacd086a3c30a08ee7eb0186c19de4311552360274a2ede` |

## 4. Configuration [M]

Preparation command (run at 17:47Z; the rehearsal copy used the same command with
`--campaign-directory TMP/lc1-rehearsal/campaign --checkpoint-interval-seconds 600`):

```sh
cd /common/dev/rustred && env TMPDIR=/common/dev/rustred/TMP nix develop --command python \
  examples/python/production_saved_owner_campaign.py \
  --prepare-from campaigns/five-loop-dependency-closure \
  --queries examples/input/five_loop_qcd_feynman_d9d10/queries.json \
  --attach examples/input/five_loop_qcd_feynman_d9d10/entry-plan-receipt.json \
  --attach examples/input/five_loop_qcd_feynman_d9d10/skeleton-classification.json \
  --attach examples/input/five_loop_qcd_feynman_d9d10/matching-summary.json \
  --campaign-directory campaigns/five-loop-qcd-feynman-d9d10-lc1 \
  --executable TMP/fable51-controls/bin/rustred-lc1-4606cc4b \
  --workers 100 --cpus 128-227 --publication-policy ready --transfer-unreserved-lookahead 256 \
  --checkpoint-interval-seconds 14400 --max-memory-bytes 600000000000 --ram-guard-margin-percent 5 \
  --frontier-policy stop
```

Frozen options: 100 workers on CPUs 128-227 (socket 1, native split 67 inspectors / 32 admission helpers / 1
coordinator), Ready publication, transfer lookahead 256, frontier policy `stop`, checkpoints every 14,400 s (CP5),
RAM request 600 GB with a 5 % margin (save+stop at 570 GB), host MemAvailable floor 50 GB (cooperative save+stop;
hard stop at 12.5 GB), own swap-growth stop at 32 MiB/s sustained for 120 s. The native argv is v2's apart from
these deliberate changes: CPUs 28-127 -> 128-227, 700 GB -> 600 GB, and the v3-steering additions
`--frontier-policy stop`, `--host-memory-reserve-bytes 50000000000`, `--swap-growth-stop-*`. All other solver
flags are v2's (`--unbounded-work --max-queries 183 --max-query-bytes 121424 --bounded-refinement-axes finite-axes
--max-guard-univariate-degree 64 --route-domain-overcover --reuse-initial-d-bands`). The rehearsal and LC1
steering files differ only in `checkpoint_interval_seconds` (600 vs 14400) and paths. No path from v2 or from the
rehearsal appears in LC1's `bin/` or `inputs/*.json`; the input receipt's `source` fields name the read-only
source campaign as provenance only.

RAM admission is re-evaluated at every (re)start: effective hard cap = min(600 GB, MemAvailable - 50 GB), with the
ZFS ARC counted as used. The full 600 GB cap needs MemAvailable >= 650 GB at launch. The validation run at 17:47Z
admitted the full cap (MemAvailable 782.5 GB, ARC 92.3 GB); the rehearsal starts admitted it too.

Validation (dry-run) mode, which prints the frozen plan and changes nothing:

```sh
cd /common/dev/rustred && env TMPDIR=/common/dev/rustred/TMP nix develop --command python examples/python/production_saved_owner_campaign.py --campaign-directory campaigns/five-loop-qcd-feynman-d9d10-lc1 --json
```

Its output at 17:47Z (`TMP/lc1-prepare/validate.json`): rc 0, executable 4606cc4b..., selection d2667dc9...,
queries 2c714860... (183 / 121,424 B, order preserve), 100 workers, `ready`, lookahead 256, frontier `stop`,
requested 600 GB, margin 5 %, host floor 50 GB, swap guard 33,554,432 B/s for 120 s, checkpoint interval 14,400 s,
steering sha256 89dc6c97..., `memory_admission_preview.hard_capped_by_available_memory = false`,
`launch_requested = false`. After it the directory still holds only `bin/` and `inputs/`.

## 5. Rehearsal (socket 1, outside `campaigns/`) [M]

Copy: `TMP/lc1-rehearsal/campaign` (same preparation, 600 s checkpoint interval). Each leg ran under
`flock -w 5400 TMP/locks/socket1.lock` with exactly the owner's start/resume command. Pause 1 was SIGINT to the
supervisor (what Ctrl-C in the pane sends); the final stop used the stop-file path. Receipts:
`TMP/lc1-rehearsal/campaign/runs/{20260928T174627.837158Z,20260928T175827.190076Z}`, launcher logs
`TMP/lc1-rehearsal/run{1,2}.log`, 30 s host samples `TMP/lc1-rehearsal/samples.jsonl`, per-run summaries
`TMP/lc1-rehearsal/run{1,2}.analysis.json` (from `TMP/lc1-rehearsal/analyze_run.py`).

Timeline: leg 1 launcher invoked 17:46:22Z, SIGINT at 17:58:13.4Z; leg 2 (`--resume --start`) invoked 17:58:22.5Z,
stop file written 18:08:23.0Z. "Traversal" starts at the first heartbeat with committed progress; rates are over
[that heartbeat, last heartbeat before the stop].

| | leg 1 (fresh start) | leg 2 (resume) |
|---|---|---|
| launch -> first traversal heartbeat | ~94 s (native preparation 88.8 s) | ~110 s (same preparation; internal restore 13.9 s: decode 4.8 s, validate 9.1 s, 2.1 GB RSS after restore) |
| traversal window | 619.7 s | 495.6 s |
| stop path | SIGINT to the supervisor (= Ctrl-C) | stop file `runs/<RUN>/stop-request.json` |
| stop request -> exit | 5.0 s | 10.0 s |
| exit / state / stop reason | 4 / `paused` / `operator_signal_2` | 4 / `paused` / `existing_operator_stop_file` |
| native stop reason, RAM-guard stop, hard stop | none / none / false | none / none / false |
| frontiers (max over every heartbeat) | **0** | **0** |
| CP5 generations | 1 bootstrap (19 B); 2 end of preparation (25 KB, 0.01 s); **3 periodic at 600 s: 2.36 GB, 4.79 s**, 2.83M committed; **4 pause: 2.40 GB (120 MB new), 0.57 s, `paused: true`** | 5 after restore (2.40 GB, 79 MB new, 0.43 s); **6 pause: 3.72 GB, 2.77 s, `paused: true`** |
| committed domains (obligations) at stop | 2,882,709 | 4,361,089 |
| native inspections at stop | 1,072,221 | 1,734,020 |
| discovered domains at stop | 5,093,991 | 6,958,453 |
| initial roots closed (native descendant closure) | 3 / 67 | 6 / 67 |
| **natives / h** | **6.23M** | **4.79M** |
| **obligations / h** (committed domains) | **16.75M** | **10.71M** |
| **discovered domains / h** | **27.62M** | **13.54M** |
| pending growth per completion (session) | 2.05 | 0.59 |
| coordinator duty (commit + preparation, session) | 62.9 % = 38.1 % + 24.7 %; other coordinator time: dispatch 12.6 %, progress JSON 5.7 %, poll 4.6 %, publication 3.0 % | 70.4 % = 43.4 % + 27.0 %; dispatch 8.8 %, progress JSON 3.3 %, poll 3.6 %, publication 2.3 % |
| inspectors computing (mean of 67) | 6.8 | 7.9 |
| own busy cores (supervised tree, 30 s samples: median, range) | 14.3 (7.7-19.2) | 14.8 (1.0-36.0) |
| **foreign busy CPUs on 128-227** (median, range) | 38.6 (25.3-49.0) | 34.8 (21.1-38.6) |
| **peak RSS** (supervisor sampling) | 10.24 GB | 10.17 GB |
| **RSS per discovered domain** | 2.05 KB at 600 s (9.50 GB, 4.63M); 1.51 KB at stop | 1.11 KB at stop (7.75 GB, 6.96M) |
| RAM guard in `request.json` / `supervisor-result.json` | hard 600 GB, soft 570 GB, host floor 50 GB, emergency 12.5 GB, own swap growth 32 MiB/s for 120 s; admission MemAvailable 780.3 GB (ARC 83.7 GB), not capped; lowest MemAvailable seen 734.4 GB | same policy; admission MemAvailable 799.2 GB (ARC 151.1 GB), not capped; lowest seen 677.4 GB |

Both legs printed `Durable checkpoint: ...` and an exact resume command; `status.json` shows `state: paused`,
`exit_status: 4`, `ram_guard_stop: null`, frontier policy `stop` and publication `ready_ticket_stream`.

**Comparison with v2 [E].** v2 ran the same inputs from scratch with binary 102adcc3 on CPUs 28-127 (W100 Ready,
2026-09-26; its `events.jsonl` read without modification). Windows are matched by committed range:

| window | natives / h | obligations / h | discovered / h | RSS per discovered domain |
|---|---|---|---|---|
| leg 1 (0 -> 2.88M committed) | 6.23M | 16.75M | 27.62M | 2.05 KB at 600 s |
| v2, first traversal heartbeat -> 720 s (0 -> 2.66M committed, 617 s) | 5.56M | 15.51M | 24.93M | 5.81 KB at 720 s (26.8 GB) |
| leg 2 (2.89M -> 4.36M committed) | 4.79M | 10.71M | 13.54M | 1.11 KB |
| v2, 720 -> 1300 s (2.66M -> 4.14M committed, 580 s) | 4.19M | 9.18M | 11.72M | 5.75 KB at 1300 s (37.3 GB) |

The ratios are 1.08-1.17x on throughput and about 3-5x less RSS per discovered domain. This is n = 1, with
different CPUs, different foreign load and a restore in leg 2, so it shows no regression; it is not a speedup
claim. The W0 comparator runs (run2 2.27M obligations/h; runC 3.415M/h for the kernel2 fp build) start from a
generation-7 restore at ~74M domains, a much later phase, and are not comparable to these early-phase numbers.

## 6. Owner operations

**Start**: section 0. The native spends ~90 s in preparation (owner load, map verification; bootstrap
generation 1) before traversal starts; generation 2 (67 pending, a few KB) is saved when preparation ends.

**Monitor (read-only)**, with the run directory printed at start (`ls campaigns/five-loop-qcd-feynman-d9d10-lc1/runs/`
lists them; each resume adds one):

```sh
cd /common/dev/rustred && nix develop --command python examples/python/campaign_monitor.py campaigns/five-loop-qcd-feynman-d9d10-lc1/runs/<RUN> --once
```

`status.json` in the run directory has the same data (`progress.work.frontiers`, `derived`, `resources`).

**Pause**: focus the campaign pane and press Ctrl-C **once**. The supervisor writes `runs/<RUN>/stop-request.json`;
the native saves a paused CP5 generation and exits 4; the pane prints `Durable checkpoint: ...` and the resume
command. Do not press Ctrl-C again and never kill the native process. From another shell the same pause is
`kill -INT <supervisor pid>` (pid in `runs/<RUN>/processes.json`) or creating `runs/<RUN>/stop-request.json`.

**Resume** (same pane or any plain shell):

```sh
cd /common/dev/rustred && env TMPDIR=/common/dev/rustred/TMP nix develop --command python examples/python/production_saved_owner_campaign.py --campaign-directory campaigns/five-loop-qcd-feynman-d9d10-lc1 --resume --start
```

**Performance-only binary swap** (later, e.g. G2' or upstream Symbolica if their gates pass and the binding
allows): pause, dry run `... --resume --upgrade-executable <NEW>` (changes nothing), then the same with `--start`
(driver doc, "Resuming onto a semantics-compatible binary").

**Exit codes**: 4 = paused with a saved checkpoint (operator, frontier stop, RAM guard or host floor; the stop
reason is in the pane and in `status.json`); 0 = native drain (still not a family-closure claim); anything else
= incomplete, keep the checkpoint and report it.

**First 10 minutes** (reference values from the rehearsal, section 5; early-phase only):

- `frontiers 0` in the monitor line (or `progress.work.frontiers == 0` in `status.json`). A frontier stops the
  run with exit 4 and stop reason `frontier_policy`; report it before resuming.
- Initial 67/67 published within ~2 min of the start; phase `Route`/`Apply`.
- RSS: the rehearsal was 9.3-9.5 GB at ~10 min; RSS per discovered domain ~1.5-2 KB. v2 was 26.8 GB at 12 min.
- Throughput over the first ~10 min of traversal: natives ~6.2M/h, obligations ~16.7M/h, discovered domains
  ~27.6M/h (rehearsal). These early rates fall as the queue deepens; they are not a projection.
- Coordinator duty ~60-80 %; own busy cores ~10-20 of the 100 reserved (coordinator-bound, as in every earlier run).
- No `RAM admission: effective hard cap` warning, or note the admitted cap if it appears.
- The first periodic save is at 4 h, not in the first 10 minutes.

## 7. Caveats

- n = 1 rehearsal, 12 + 8 min, early phase only. Nothing here measures the late phase (tens of millions of
  domains, multi-GB checkpoints) that decides the run; v2's generation-3 save (16.7 GB) took 110 s on the old
  binary, and the restore at scale took 164 s (FABLE_HANDOFF 8.1 and 8.4).
- Foreign load: other users' unpinned processes (postgres, gammaboard) kept a median of 35-39 (range 21-49) CPUs
  busy on CPUs 128-227 during the rehearsal. The campaign's own cores are mostly idle waiting on the coordinator,
  so the exposure is the coordinator thread sharing a core [E].
- The comparison with v2 (different binary 102adcc3, CPUs 28-127, other foreign load) is [E] only.
- Pausing costs a restore of about 110 s from launch to traversal at this size (13.9 s of it the checkpoint
  restore, the rest the same ~90 s preparation as a fresh start); at scale it is minutes (restore at scale 164 s
  at 28.8M domains, plus preparation) [M, earlier notes].
