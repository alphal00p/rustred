# W0.5 production baseline M1: the P-IMP comparison baseline (2026-09-27)

This note is the persisted baseline of record for plan item W0.5 and gate 0.5 of
`docs/research/fable51_next_push_master_plan_2026-09-27.md`. It holds the full-window figures of the 25-min
gen-7 resume (M1 run2) with their spread, the 12-min cross-check (run1), the gate 0.5 scoring, and the answer to
the restored-vs-live question. It also says how P-IMP (W2 exit gate) and later arms should be compared against it.

Labels: **[M]** measured (run directory, file and binary sha256 are cited); **[E]** estimate, derivation or
interpretation. Nothing here is a closure, termination or ETA claim; `family_closure_claim` stays false. No campaign
was started, stopped, signalled or written. The v2 campaign's `request.json` and `events.jsonl` were only read. No
engine code was changed, and no algebraic code was written.

The full working record is `TMP/w0/baseline/RESULTS.md`: §2 C-4L, §3-§4 run1, §7 run2. Raw data are under
`TMP/w0/baseline/m1/`. The tools are `tools/research/w0_baseline/` on branch `fable_5_1-v3-baseline`.

## 1. What was run

| Item | Value |
|---|---|
| Binary | `TMP/w0/baseline/bin/rustred-fp-7eed68fc`, sha256 `7eed68fcafed333afb7370b3c4561c9bb9b20fbc984ebe608b60e2a42213df7f`. Frame pointers plus line tables, built from sources identical to 66ede259, which built the canonical `rustred-4a17f9c7` (sha256 `4a17f9c7…395e`). Walk semantics 1, CP5. |
| Identity to 4a17f9c7 [M] | C-4L (FG/BMW/H/X, A≤19 R≤12 D≥7, W6): Ordered strict 8/8 with 0 differing records; identical native inspections and `containment_checks`; 24/24 audits PASS. fp/ref Ordered traversal ratio 0.99-1.01, below the repeat spread (`TMP/w0/baseline/c4l/summary.json`). |
| State | Block clone of `TMP/v2-checkpoint-copy-gen7`: v2 campaign CP5 gen 7, 74,156,033 domains, 45,889,639 records, 1,192,281,291 edges, 8/67 roots closed |
| Command | v2 `request.json` argv; paths rewritten; `--checkpoint` replaced by `--resume <clone>` |
| Topology | W100 = 67 inspectors, 32 admission helpers, 1 coordinator (required by the checkpoint binding) |
| CPUs | 128-227 (socket 1: nodes 4, 5, 6 and 4 CPUs of node 7), nice 0, under `TMP/locks/socket1.lock`; harness and perf on CPUs 94-99 |
| Harness | `m1_resume_profile.py` / `m1_run.sh` (15cb100c). 5-s `/proc` sampling. 60-s fp `perf record` windows for the coordinator, 4 helpers and 4 inspectors. `perf stat` per thread. A short DWARF window. A `numa_maps` census. |
| run2 (baseline of record) | `TMP/w0/baseline/m1/run2/`. Launch 15:27:10Z; traversal from 15:39:01Z; stop requested at T+1,500.9 s (`run_seconds_elapsed`); exit 4 (paused) at 16:04:27Z |
| run1 (cross-check) | `TMP/w0/baseline/m1/run1/`. Same state and binary. Measured window [T, T+720 s]: the harness crashed at T+720 s (fixed in 15cb100c) |
| Analysis | `m1_analyze.py` (81132285, b5bc9674) → `run2/analysis.json`, `run2/attr-*.json`; `m1_drift.py` (b5bc9674, ffc4d534) → `m1/drift/*.json` |

The host was not exclusive in either run [M]. Foreign busy CPUs on 128-227 (own CPU subtracted):
- run2: mean 33.4 (p90 40.4, max 68.4)
- run1: mean 43.3 (p90 59, max 63)

Neither run is an A/B-grade arm under the plan's 10% rule. Both are baselines with that load recorded.

## 2. Baseline of record: run2, 25.0 min at W100 after the gen-7 restore

The full window is [T, stop], 1,500.9 s on the harness clock [M, `m1/run2/analysis.json`]. "5-min band" is the
min-max over the five 5-min slices from T (the last slice is 297 s). "run1" is [T, T+720 s].

| Metric | run2 full window | 5-min band (median) | run1, 12 min |
|---|---:|---|---:|
| **Obligations discharged per hour** (committed domains, native + alias; M-obl) | **2.27M** | 1.76-3.08M (1.92M) | 2.06M |
| Natives per hour (native inspections) | **1.30M** | 1.04-1.72M (1.12M) | 1.20M |
| Delegated (alias) publications per hour | 0.97M | 0.72-1.36M | 0.86M |
| Discovered domains per hour | 3.61M | 3.32-3.96M | 3.92M |
| Native pending, net change | +392,837 (0.72 per native) | — | +237,934 |
| Initial roots closed | 8 → 8 | — | 8 → 8 |
| Coordinator duty: ordered_commit / preparation | 48.1% / 43.6% | commit+prep 87.8-93.6% | 46.1% / 45.6% |
| Coordinator duty: untimed | 2.5% | 2.1-3.1% | 2.4% |
| **Speculative checks per admission request** (M-adm) | **1,179** | 802-1,742 (1,205) | 1,014 |
| **Commit µs per prepared record** | **8.36** | 5.17-11.87 (9.42) | 6.76 |
| Preparation µs per parallel batch | 1,292 | 996-1,611 (1,316) | 1,195 |
| Commit ns per committed-path containment check | 4.16 | 3.54-5.09 (4.22) | 3.75 |
| Admission requests per committed domain | 83 | 42-156 | 109 |
| RSS per discovered domain (M-ram) | 459 B at T (74.16M) → 479 B at stop (75.66M) | — | 459 → 478 B |
| Marginal RSS per new domain | 847 B (fit after T+60 s, R² 0.94, 1,429 points; endpoints 1,469 B) | — | 1,086 B (fit) |
| VmHWM (restore peak) | 47.7 GB | — | 47.7 GB |
| CPUs used: coordinator / helpers / inspectors | 0.55 / 7.21 / 1.66 (≈9.4 of 100) | — | 0.53 / 6.76 / 1.66 |
| Inspector slots finished and waiting for commit | 65.5 of 67 (mean over 1,062 heartbeats); computing 1.13 | — | — |
| Restore: internal / launch→restored / launch→first traversal | 490 s / 600 s / 710 s | — | 459 / 570 / 680 s |
| Stop request → exit (incl. final save) | 25.0 s (13.5-s gen-9 save) | — | 25.4 s (16.4-s save) |

Finer granularity [M, `m1/drift/run2.json`, 24 bins of 60 s]:
- natives/h 0.88-2.54M (median 1.23M)
- checks/request 553-3,734
- commit per record 4.2-24.0 µs

Across the five 5-min slices, natives/h has CV 23% and obligations/h CV 25%.

Derived for the P-IMP gates [E, arithmetic on the measured values above]:
- **≥5x the baseline in obligations per hour** means ≥ **11.4M/h**. The plan's "below 3x" threshold is 6.8M/h.
- **Merge duty ≤50%**: the baseline is 91.7% (commit plus preparation).
- **≥60% of inspector threads in native work**: the baseline is about 2.3%. Inspectors use 2.5% of a CPU each, and 93-94% of their samples are inside `inspect`.
- **Lookup CPU per native ≤1x native CPU**: the baseline is about 4.3-5.0x.
  - Lookup CPU: helpers 7.21 CPUs (8-9% of their samples are crossbeam-epoch idle/steal) plus the coordinator's ordered commit, ≈0.49 CPU.
  - Native CPU: ≈1.55 CPUs.
- **RSS ≤0.6 KB/domain** (P-IMP), **≤0.45 KB/domain** (W3.3) and **≤0.5 KB/domain at 74M+** (launch (C)): the baseline average is 0.46-0.48 KB/domain.
  - At the fitted marginal 847 B/domain, the average would cross 0.5 KB/domain after about +8.7M domains, near 83M.
  - This extrapolates linearly from a 1.5M-domain fit range.
- **Restore within 10 min**: the baseline reaches restored at 600 s. That is 110 s of owner loading plus the 490-s restore, so it sits at the limit.
- CPU per native over the run [E, thread CPU over 1,495.9 s / 541,894 natives over 1,500.9 s]: inspectors 4.6 ms, helpers 19.9 ms, coordinator 1.5 ms.

## 3. Coordinator attribution and gate 0.5

The 60-s windows used `perf record -F 499 -e cpu-clock:u --call-graph fp` (user mode; `perf_event_paranoid` 2).
Inline chains were expanded with `llvm-symbolizer`, and each sample was assigned to the leaf-most duty-bucket anchor
(`perf_attribution.py`) [M, `m1/run2/attr-*-coordinator.json`, `m1/run1/attr-early2-coordinator.json`].

| Window | Samples | Named | **Duty buckets** | ordered_commit | Loop and `commit_chunk` bodies | Telemetry (`set_parallel`) | progress_json | publication |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| run2 early, T+181 s (74.29M domains) | 12,612 | 100.0% | **97.0%** | 92.0% | 3.0% | 1.7% | 1.3% | 0.9% |
| run2 late, T+1,264 s (75.39M) | 12,636 | 100.0% | **95.4%** | 86.8% | 4.6% | 3.0% | 2.5% | 1.6% |
| run1 early2, T+286 s (74.2M) | 12,283 | 100.0% | **94.8%** | 88.8% | 5.2% | 1.9% | 1.9% | 0.9% |

**Gate 0.5 (baseline recorded, ≥80% of coordinator time attributed to named callers): PASS** [M]. It passes in both
run2 windows, which span the 25-min run, and in run1's window.

Scope of the gate [E]: perf covers the coordinator's on-CPU user time, 0.55 CPU. The off-CPU preparation wait (43.6% of
the wall) is covered by the native duty timers, which leave 2.5% of the wall untimed.

Where the coordinator's CPU goes [M]:
- Forward find plus reverse retire over the aggregate index hold **71-81%** inclusive in every window.
- How it splits moves with the work mix. Early: retire 51%, `find_from` 11%. Late: retire 25%, `find_from` 35%. run1: retire 31%, `find_from` 23%.
- Self time sits in the index block scan (25-40%), summary `contains`/`contained_by` (10-15%) and the bit prefilter (8-11%).
- SipHash/hashbrown, memmove, free, malloc and serde_json/BTreeMap stay ≤ 6% each.

Workers [M, fp, 4 threads each]:
- **Admission helpers.** Nearest named frames: `bits::may_contain` 17-19%, index-block closure 14-16%, `Stored::contains` 10-14%, crossbeam-epoch 8-9% (rayon idle/steal). IPC 0.49-0.64, with 55-57% of DRAM fills from a far node: memory-latency bound.
- **Inspectors.** `inspect` 93-94%, `Matcher::run` 92-93%, `apply_piece` 59-64%, `apply_group` 40-41%. libc is the leaf in 24% of samples (`realloc` nearest in 13%). IPC 2.7-2.8.
- **Coordinator.** IPC 0.86-0.91 in run2 and 1.06 in run1, above the relief note's E1 threshold of 0.5 [E1 verdict E].

The DWARF window (`-F 99 --call-graph dwarf,8192`, one thread per class, 20/10/6 s) is only a cross-check [M]:
- Coordinator: 912 samples. Only 13% unwind to `run_pool` because the 8-KiB stack dumps truncate the stacks. Its leaf-side buckets agree with fp: ordered_commit 92.9% ± 0.9, duty buckets 97.3%.
- Helper: 224 samples, same frame classes as fp.
- Inspector: 3 samples, unusable.

No retry is needed for gate 0.5. A deep inspector-head profile (Q7) belongs to the W0.3 harness.

## 4. Spread, replicate and the restored-vs-live question

**Replicate** [M, `m1/drift/run1.json` and `m1/drift/run2-first720s.json`]: run1 and run2 restore the same state with the same binary. Their first 12 min cover the same stretch of the ID sequence (committed 45.89M → 46.30M / 46.26M).

| First 12 min | run1 | run2 | run2 / run1 |
|---|---:|---:|---:|
| natives/h | 1.20M | 1.09M | 0.90 |
| obligations/h | 2.06M | 1.85M | 0.90 |
| speculative checks/request | 1,013 | 1,028 | 1.015 |
| committed-path checks/request | 1,976 | 2,067 | 1.046 |
| commit ns per check | 3.75 | 4.41 | 1.18 |
| commit µs per record | 6.75 | 8.34 | 1.24 |
| coordinator IPC / far-node share of DRAM fills (T+181 s) | 1.06 / 25.8% | 0.86 / 40.7% | — |
| coordinator samples on the heap's node (node 4) | 90% | 82% | — |

- Admission work per request reproduces within 1.5-4.6%, but time per unit of work differs by 13-24% [M].
- run2 was the slower run even with less foreign load. Its heap sat 89.4% on node 4 (`numa_maps` at T+23 min), and the scheduler placed its coordinator and helpers more often on nodes 5-7 [M].
- [E] Uncontrolled NUMA placement is a plausible cause. It is an input to W0.3 and to the launch configuration: pin the coordinator and helpers to the heap's node, or interleave.
- **Replicate spread of a 12-min timing at the same state: ≈10%.**

**Non-stationarity** [M]:
- The first 15 min are flat at 1.04-1.12M natives/h, so there is no warm-up.
- From T+16 min to T+23 min a work-mix phase runs 1,484-3,734 checks/request and 1.41-2.54M natives/h in 60-s bins, with 16-44 requests per committed domain (68-210 elsewhere).
- The last two minutes fall back to 887-1,005 checks/request.
- Throughput and admission cost follow the stretch of the ID sequence being committed, not the time since restore.

**Restored vs live** (why run1 ran at 1.48x v2's natives/h and 0.41x its checks/request over v2's last 12 min before gen 7):
1. [M] The restore carries the live candidate count over exactly: 37,907,667 in v2's last heartbeat, 37,907,655 in run2's first, 11 admissions later. The restore does not shrink the index.
2. [M] Admission counts are restore-invariant on a control. FG Ordered was paused by 102adcc3 and resumed by 4a17f9c7, and it ends with `containment_checks` 169,509,549 and 98,869 natives, the same as the uninterrupted run (`TMP/fable51-controls/final-4a17f9c7{,-resume}/fg/result.json`).
3. [M] The live process varies as much on its own (`m1/drift/v2-*.json`, v2 `events.jsonl` read only):
   - Consecutive 12-min windows of v2 differ by ≥1.48x in natives/h in 28% of pairs (last 6 h; 22% over 12 h, 30% over the whole run).
   - They differ by ≥2.44x in checks/request in 17% of pairs (last 6 h).
   - v2's last 2 h were a high-checks phase: 5-min median 2,369.
   - run2's slices (802-1,742) lie inside v2's 6-h band (p10-p90 624-2,555).
4. [M] Time per check: run2 4.16 ns against a v2 6-h median of 5.49 ns (p10 3.77). The wave-2 binary alone measured −16..−18% commit time per record on the five-loop W50 control (`fable51_wave2_profiling_2026-09-27.md`). The live RSS gap (272 GB against 34 GB at 74M) is mostly the binary: 3.7-4.5 KB/domain on 102adcc3, 0.38-0.48 KB/domain on the wave-2 code.

Verdict [E]:
- The run1-vs-v2 ratios come from comparing different stretches of the ID sequence, together with the wave-2 binary. No restore effect is needed.
- The "restore rebuilds a denser index" interpretation is refuted for candidate count [M] and unsupported for candidate order [M, control].
- Any restore effect on time per check is below the replicate spread.
- *Correction to the earlier summary (handoff §7.5):* it read run2's series as checks/request "climbing back toward v2's 2,465". The series is not monotone and falls back to 887-1,005 at the end.
- A same-binary live-vs-restored A/B is deferred as not needed for any W1-W5 decision. It would resume the gen-6 clone with 102adcc3 and compare against v2's events over the same committed range; its old-binary restore time is unmeasured.

## 5. How to compare P-IMP and later arms against this baseline

1. **Same state and CPUs.** Use a fresh block clone of `TMP/v2-checkpoint-copy-gen7` (for P-IMP, its CP6 import), W100 on CPUs 128-227 under `socket1.lock`. Record foreign busy CPUs; the harness does this.
2. **Same stretch, full window.** Compare P-IMP's [T, T+25 min] with run2's full window (2.27M obligations/h, 1.30M natives/h) and report P-IMP's 5-min slices next to run2's band. Report P-IMP's 40-min figure separately. If the v3 engine's dispatch order changes which stretch of pending work is processed, say so. The mix noise (5-min CV 23-25%; live adjacent 12-min windows ≥1.48x apart in 22-30% of pairs) means one run resolves only effects well above ~1.5x. The 5x gate is far above that; smaller levers need same-clone, interleaved repeats (replicate spread ≈10%).
3. **Record NUMA placement.** Take a `numa_maps` census and sample per-thread `processor` (the harness does both), or pin explicitly. Placement alone moved time per check by 13-24% between replicates.
4. **Units.** "Obligations discharged" = committed domains (native + alias). Natives = native inspections (`expanded`). Under walk semantics 3 / CP6 the `containment_checks` metric may change (owner rule 3). If candidates-tested-per-request is not defined identically, label M-adm comparisons as not like-for-like.
5. **Not in this baseline:**
   - M-obl split into Apply and Route with CPU per class. The heartbeats only sample the phase of the committing domain: 67-70% Apply.
   - M-mist.
   - Roots certified per native CPU-hour: 0 new roots in 25 min, 8 → 8.
   - Exclusive CPUs.
   - Kernel-mode samples.

## 6. Reproduction

```
# M1 (holds socket1.lock for the native's lifetime; <= 60 min including the ~10-min restore)
tools/research/w0_baseline/m1_run.sh TMP/w0/baseline/bin/rustred-fp-7eed68fc <out-dir> <fresh gen-7 clone>
nix develop --command python tools/research/w0_baseline/m1_analyze.py <out-dir> \
    --perf /nix/store/gyp2si1k1w7jhw8z4xx1bwr2m0pr5445-perf-linux-7.2/bin/perf
# admission drift (stdlib only; a campaign's events.jsonl is read, never written)
python tools/research/w0_baseline/m1_drift.py <out-dir>/events.jsonl --label <label> --out <json> \
    [--max-seconds 720] [--tail-seconds 21600]
```
