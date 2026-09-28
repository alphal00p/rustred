# W2 comparator pilot: legacy engine + SoA kernel, 25-min gen-7 resume at W100 vs M1 run2 (lane `comparator`)

Status (2026-09-28 ~10:50Z): **runB is VALID but PROVISIONAL; runA is INVALID (diagnostic only).**
The orchestrator decided at 10:40Z (`TMP/w1/comparator/ORCHESTRATOR.md`) that the >= 1.5x owner gate will **not**
be scored against runB. runB's binary still carries the avoidable compaction-loop copy described in §2.6. The next
workflow will fix it in the W1.1 follow-up (swap only when needed), re-run strict identity (a)-(e), and then run
**runC** on a fresh gen-7 clone with this recipe. The gate is scored against runC. This lane does not launch runC.

Labels: **[M]** measured (paths given), **[E]** estimate or interpretation. No ETA, no closure claim;
`family_closure_claim` stays false. No campaign was started, stopped, signalled or written. Nothing under
`TMP/w0/baseline` was written: run2 is read only, and its figures are re-derived from its raw files by the same code
that analyses the comparator.

## 0. Verdict

**Gate metric and provisional value** [M]. Owner gate: continue W2 only if the S2/S4 epoch skeleton beats the
comparator by >= 1.5x. The final value comes from runC (see Status).

| | comparator runB (`rustred-fp-36c60fb8`) | M1 run2 (`rustred-fp-7eed68fc`) | runB / run2 |
|---|---:|---:|---:|
| **obligations/h (natives + aliases) over [T, T+1,500 s]** | **2.849M** | 2.271M | **1.255x** |
| 5-min slice spread of obligations/h | 1.95-4.56M (mean 2.85M, CV 35%, n=5) | 1.76-3.08M (CV 25%, n=5) | |
| obligations/h over the same committed ID range (947k obligations) | 3.219M | 2.272M | **1.417x** (per run2 slice 1.18-1.66x) |

- **What the gate uses:** comparator obligations/h (natives + aliases) over the matched window [T, T+1,500 s] of a
  fresh gen-7 W100 resume, with its 5-min slice spread. **runB's provisional value is 2.85M obligations/h** (slices
  1.95-4.56M/h, CV 35%), which puts the 1.5x threshold at **4.27M obligations/h**.
  - The protocol is fixed: a fresh gen-7 clone, W100 on CPUs 128-227, the window [T, T+1,500 s] after the first
    traversal heartbeat, and foreign load recorded.
  - The value is not final: the gate is scored against runC. [E] A fixed comparator is likely to be faster, by up to
    ~1.3-1.4x, so the final threshold is likely higher.
- **Validity** [M]: progress_json takes 1.12% of coordinator wall (run2 0.87%; the rule is < 2%). The window is
  full (1,498.3 s on the native clock). Foreign load on CPUs 128-227 averaged 36.9 busy CPUs (run2 33.4).
- **The comparator is conservative in the epoch engine's favour, for two reasons:**
  1. *Kernel speed-up* (orchestrator decision 1). The legacy SoA kernel reached only **~3x less CPU per candidate,
     not 4x** (W1.1 gate (c): forward 2.86-3.08x, reverse 3.11-3.79x). The legacy layout must keep its historical
     block boundaries so that `containment_checks` stay identical.
  2. *Compaction loop* (**new, §2.6**) [M]. `memmove` takes **39.6-40.8% of the coordinator's on-CPU samples**
     (run2: 3.0-4.8%). The DWARF window traces 30% of all coordinator samples to one line: `slice::swap::<Meta<15>>` at
     `queue/index.rs:893` (commit 36c60fb8). That is the compaction loop in `AggregateIndex::retire`, which swaps
     every 144-B row of every eligible group on every retirement, even when no row moves. As a result the commit cost
     per prepared record did not improve (8.42 vs 8.36 µs). [E] Skipping the swap when no block emptied would cut
     coordinator wall by about 22-31%, which would make the comparator up to ~1.3-1.4x faster. This is not measured.
     It is a comparator-binary artefact, like the one that voided runA, but smaller. The orchestrator decided at
     10:40Z to fix it and score the gate against runC (see Status).
- **Work-mix confound in the time window** [M]. runB got 238k obligations past the end of run2's ID range, into a
  slower stretch (1.96M/h there, 3.95 discovered per native). Over the ID range both runs covered, runB is 1.42x;
  over the time window it is 1.26x. A faster engine reaches further along the ID sequence, so a time-matched
  comparison between engines of different speed carries a work-mix term. The epoch pilot should also report
  count-matched figures (§3).
- **Work volume is unchanged by the kernel** [M]. On the common ID range, discovered per native (2.783 vs 2.785),
  queue pending growth per completion (0.592 vs 0.593) and native-pending per native (0.725 vs 0.725) agree within
  0.1%. The time-window work-volume ratios (e.g. pending growth per completion 1.32x) come entirely from the later
  stretch that only runB reached.
- **The walk is still coordinator-bound** [M]. 65.1 of 67 inspector slots sit finished and waiting for commit
  (run2 65.5). The escrow is at its limit in 99.5% of heartbeats. ordered_commit takes 65.0% of coordinator wall
  (run2 48.1%), and the preparation wait 24.1% (43.6%). The whole process uses 8.6 of its 100 CPUs.

## 1. Setup [M]

- Recipe: `tools/research/w0_baseline/m1_run.sh` + `m1_resume_profile.py` (the M1 run2 harness), branch
  `fable_5_1-v3-baseline`, worktree `.claude/worktrees/fable51-fp`. Lane tool commits:
  - 41e9bad7: m1_run.sh harness-CPU and inputs parameters; m1_analyze kernel timers; `w2_compare.py`
  - 4cbf081f: llvm-symbolizer lookup (the pinned llvm-22.1.8 store path was garbage-collected; llvm-21.1.8 is used)
  - 57a99aec: w2_compare validity check (progress_json < 2% and a full window), absolute-committed stretch match,
    `--comparator-end`
  - e4218db6: w2_compare takes an external stop file's time and never ends thread CPU on a sample of the exiting
    process
  - a1c9d325: m1_analyze reads the fix round's reverse attribution (`reverse_examined_candidates`)
- Launcher: `TMP/w1/comparator/launch_run.sh TAG BIN SHA256`. It checks the sha256, the walk-semantics probe and the
  inputs (`diff -rq` against `TMP/w0/baseline/m1/inputs`), makes a fresh gen-7 block clone
  (`cp -r TMP/v2-checkpoint-copy-gen7 checkpoint-TAG`, `latest.json` compared) and runs m1_run.sh. m1_run.sh holds
  `socket1.lock` from before the restore until the native has exited.
- Native: W100 (67 inspectors, 32 admission helpers, 1 coordinator) on CPUs 128-227 at nice 0. Argv: the v2
  `request.json` argv with the executable, output and input paths rewritten and `--resume <clone>`. A dry run showed
  it identical to run2's apart from those paths. Harness and perf ran on CPUs 40-43,296-299 (socket 0; run2 used
  94-99).
- Analysis: `TMP/w1/comparator/analyze.sh TAG` runs `m1_analyze.py` (with perf attribution), `m1_drift.py` (1,500-s
  cap) and `w2_compare.py run2 TAG`. Self-test: `w2_compare.py run2 run2` reproduces run2's `analysis.json` exactly
  (2,270,543 obligations/h, 541,894 natives, 5-min CV 25.4%, 9.43 CPUs).
- Window (both runs): T = first traversal heartbeat; the window is [T, min(stop request, T + 1,500 s + 5 s harness
  slack)]; rates are on the native clock, as for run2 (`TMP/w0/baseline/RESULTS.md` §7).

## 2. runB, the comparator of record: `TMP/w1/comparator/runB/`

### 2.1 Binary, setup and timeline [M]

- Binary: `TMP/w1-kernel/bin/rustred-fp-36c60fb8`, sha256
  `3ab745fa338d03eb81b7e72cecc3988b23dd7abc7c077f7f548ce746cd7e8fbe`, from 36c60fb8 on `fable_5_1-v3-kernel` (the
  W1.1 fix round of d9163195). Build flags match run2's `rustred-fp-7eed68fc` (frame pointers, line tables). Walk
  semantics 1, CP5 schema 5. `TMP/w1-kernel/bin/COMPARATOR_READY.json` records its gates: strict Ordered identity
  on FG/BMW/H/X/four-all and C-5F, oracle PASS, resume, and the suites.
- Checkpoint: fresh clone `TMP/w1/comparator/checkpoint-runB` (09:29:52Z, `latest.json` equal). Inputs:
  `TMP/w1/comparator/inputs`.
- `socket1.lock`: waiting from 09:29:58Z, acquired 09:50:18Z (after the ops lane released it), released 10:29:06Z
  after the native had exited (`runB.launcher.log`).

Timeline, in seconds after launch at 09:50:27Z (`runB/timeline.json`):

| Event | runB | run2 |
|---|---:|---:|
| checkpoint_restored | 615.5 | 600.4 |
| gen-8 rebinding save starts (end of post-restore preparation) | 775.6 | 695.5 |
| T (first traversal heartbeat) | **790.6** (10:03:38Z) | 710.5 |
| stop request, `run_seconds_elapsed` | T+1,501.6 | T+1,500.9 |
| exit (code 4, paused) | stop+25.3 (gen-9 save 16.1 s) | stop+25.0 (13.5 s) |
| launch to exit | 2,318.5 s (38.6 min) | 2,237.4 s |

- Internal restore: runB **503.5 s** (verify 108.2, decode 125.9, validate 377.6; the new `index_blocks` phase
  13.9 s) against run2's 490.1 s (107.9 / 124.3 / 365.8), i.e. +2.7%. RSS after the restore was 30.7 GB (run2
  29.5 GB).
- The post-restore preparation phase (from `checkpoint_restored` to the gen-8 save) took **160.1 s** against run2's
  95.1 s (runA: 140.1 s). [E] Its cause is not diagnosed. It lies outside the measured window, but it adds ~80 s to
  launch-to-T and bears on the P-IMP restore limit. Launch-to-restored is 615.5 s, just over the 10-min P-IMP limit.

### 2.2 Throughput over the matched window [M, `compare-runB.json`]

Window: 1,498.3 s on the native clock (1,501.6 s on the harness clock), 885 heartbeats. Committed 45,889,923 ->
47,075,782; discovered 74,156,784 -> 76,272,707.

| Metric | runB | run2 | runB / run2 |
|---|---:|---:|---:|
| **obligations/h** (committed domains = natives + aliases) | **2,849,366** (1,185,859) | 2,270,543 (947,639) | **1.255** |
| natives/h | 1,672,207 (695,945) | 1,298,378 (541,894) | 1.288 |
| aliases/h | 1,177,174 (489,914) | 972,165 (405,745) | 1.211 |
| discovered/h | 5,084,090 (2,115,923) | 3,614,944 (1,508,741) | 1.406 |
| native pending | +568,965 | +392,837 | |
| initial roots closed | 8 -> 8 | 8 -> 8 | |

### 2.3 5-min slices from T [M]

| Slice (s from T) | runB obl/h | runB nat/h | checks/request | commit µs/record | prep µs/batch | commit / prep / progress_json | kernel scan, % of coord. wall | run2 obl/h |
|---|---:|---:|---:|---:|---:|---|---:|---:|
| 0-301 | 2.59M | 1.53M | 1,088 | 8.49 | 540 | 65.2 / 25.3 / 1.0% | 52.5% | 1.76M |
| 301-602 | 2.80M | 1.62M | 945 | 5.81 | 500 | 61.9 / 27.3 / 1.1% | 44.5% | 1.91M |
| 602-904 | 4.56M | 2.52M | 1,602 | 8.57 | 562 | 60.5 / 25.6 / 1.8% | 48.5% | 1.92M |
| 904-1,205 | 2.34M | 1.42M | 1,064 | 9.57 | 519 | 65.5 / 22.1 / 0.9% | 54.1% | 3.08M |
| 1,205-1,502 (partial) | 1.95M | 1.26M | 1,070 | 11.35 | 528 | 72.1 / 20.2 / 0.7% | 61.3% | 2.67M |
| **full window** | **2.85M** | **1.67M** | **1,133** | **8.42** | **529** | **65.0 / 24.1 / 1.1%** | **52.2%** | **2.27M** |

Spread across the five slices: obligations/h mean 2.85M, SD 1.01M, **CV 35%**, range 1.95-4.56M. natives/h CV 30%,
range 1.26-2.52M. run2: obligations/h CV 25% (1.76-3.08M), natives/h CV 23%. The 60-s bins of `drift/runB.json` span
0.91-2.90M natives/h (run2 0.88-2.54M). Time slices of the two runs cover different ID ranges, so they are not
paired; §2.4 pairs them by ID range.

### 2.4 Same ID range (stretch-matched) [M]

The legacy engine commits in ID order (committed count ≈ contiguous publication watermark), and both runs start from
the same restored state. The first N obligations after T are therefore the same restored-pending ID range. Ready
publication can change the native/alias split, but here it matched within 0.2%.

- **Common range 45,889,923-46,837,278 (947k obligations): run2 took 1,499.5 s (2.272M/h), runB 1,059.6 s
  (3.219M/h), i.e. 1.417x.** Natives in the range: 541,096 vs 541,854.
- Per run2 slice, over run2's ID ranges: 1.36, 1.66, 1.38, 1.61, 1.18x (mean 1.44, n=5).
- Beyond run2's range (46,837,342-47,075,782, 238k obligations, the last 438.6 s of runB): 1.96M obligations/h,
  1.27M natives/h, 3.95 discovered per native, queue pending growth 1.55 per completion. That stretch has much
  heavier discovery than run2's range (2.78 and 0.59), and it pulls the time-matched ratio down from 1.42x to 1.26x.

### 2.5 Coordinator duty, admission rates and kernel timers [M]

| Bucket (share of coordinator wall) | runB | run2 |
|---|---:|---:|
| ordered_commit | **65.0%** | 48.1% |
| preparation (waiting on helper batches) | **24.1%** | 43.6% |
| dispatch / poll / publication | 3.1 / 1.4 / 1.0% | 2.3 / 1.2 / 0.8% |
| progress_json | 1.12% | 0.87% |
| closure_refresh | 0.73% | 0.63% |
| untimed | 3.4% | 2.5% |

- Admission rates: **1,133 speculative checks per request** (run2 1,179, 0.96x); **8.42 µs commit per prepared
  record** (8.36, 1.007x); **529 µs preparation per batch** (1,292, 0.41x); 169.6 records per batch (170.3).
- Coordinator SoA kernel (`coordinator_duty.admission_kernel`, nested in ordered_commit): scans take 781 s, **52.2%
  of coordinator wall and 80.2% of ordered_commit**.
  - Forward: 92.4 s over 2.96G candidates, 31.2 ns per candidate.
  - Reverse: the timer spans the whole retirement call: 689.0 s over 28.1G examined candidates, 24.5 ns per examined
    candidate.
- Helper SoA kernel (`admission_preparation.speculative_kernel`): 3,949 s of scan wall summed over the helpers, i.e.
  **2.64 CPU-equivalents**; 26.6 ns per candidate; 1,403 candidates per request.
- Reading [E]: the kernel shows up on the helper side. Preparation per batch is 0.41x, and its share of coordinator
  wall fell from 43.6% to 24.1%. On the coordinator side, commit per record is unchanged, because the retirement
  call is dominated by the compaction copy of §2.6, not by the kernel's scans.

### 2.6 Threads, perf stat and attribution [M]

| Thread class (/proc over the window) | CPUs runB | CPUs run2 | run delay per thread runB / run2 |
|---|---:|---:|---|
| coordinator | 0.75 | 0.55 | 0.12% / 0.15% |
| admission helpers (32) | 5.63 | 7.21 | 0.79% / 1.43% |
| inspectors (67) | 2.21 | 1.66 | 0.08% / 0.11% |
| **total** | **8.60** | 9.43 | |

- Admission CPU (coordinator + helpers) per obligation: **8.05 ms** (run2 12.26 ms, 0.657x). Inspector CPU per
  native: 4.76 ms (4.57 ms, 1.04x).
- Inspector slots (`parallel` block of 884 heartbeats): finished and awaiting poll, mean 65.1 of 67 (run2 65.5);
  computing, mean 1.50 (1.14); completed escrow at its 134-entry limit in 99.5% of heartbeats (100%).

perf stat, per-thread user mode, 60 s at T+181 s (early) and T+1,265 s (late):

| Class | IPC early / late, runB | IPC early / late, run2 | far-DRAM share early / late, runB | run2 |
|---|---|---|---|---|
| coordinator | 1.02 / 0.99 | 0.86 / 0.91 | 28.0% / 35.2% | 40.7% / 26.8% |
| admission helpers | 0.88 / 0.95 | 0.49 / 0.64 | 42.9% / 51.2% | 57.4% / 55.5% |
| inspectors | 2.76 / 2.71 | 2.74 / 2.78 | 49.5% / 64.0% | 58.3% / 55.1% |

- Early-window admission instructions per obligation (coordinator + helpers): 15.6M vs 17.6M.
- Inspector instructions per native: 22.1M vs 21.1M. [E] With ~98% of inspector slots idle, this counter is
  dominated by idle-wait spinning, not useful work (§4).
- The late-window per-native and per-obligation figures are not used. runB's late perf window covers a
  heavy-discovery stretch that only runB reached.

**Coordinator attribution** (fp windows, `perf record -F 499 --call-graph fp`; runB early 16,329 samples and late
16,798; run2 12,612 and 12,636). Named share is 100% in both runs. Duty buckets cover 95.8% / 95.4% of runB's
samples (run2 97.0% / 95.4%), and ordered_commit 89.9% / 91.6% (run2 92.0% / 86.8%).

| Share of on-CPU samples, early / late | runB | run2 |
|---|---|---|
| `__memmove_avx512_unaligned_erms`, self | **39.6% / 40.8%** | 3.0% / 4.8% |
| leaf in libc or allocator | 45.3% / 46.4% | 7.4% / 12.4% |
| `Block::reverse`, self | 11.7% / 13.2% | n/a |
| `fold_ge` + `Envelope::may_be_contained`, self | 10.9% / 12.0% | n/a |
| find_from + retire, inclusive | 34.7% / 34.4% | 80.7% / 71.1% |
| index block scan, inclusive | 27.6% / 28.6% | 70.0% / 43.4% |

- **Where the memmove comes from.**
  - The fp stacks give nearest Rust caller `Queue::admit_with_lookup` for 35-36% of samples. The return address
    maps to `queue.rs:712`, the call to `bucket.indexed.retire_prepared(...)`.
  - The DWARF window (T+~720 s, 20 s at 99 Hz, `runB/perf-dwarf-coordinator.data`) recovers the direct caller:
    - memmove is the leaf in 413 of 1,243 samples.
    - 373 of those (**30.0% of all samples**) come from the inlined chain `ptr::swap` / `slice::swap::<Meta<15>>`
      -> `AggregateIndex::retire` at **`queue/index.rs:893:36`** (36c60fb8), called from `retire_prepared`
      (index.rs:823).
  - That line is `group.meta.swap(kept, read)` in the compaction loop that runs after each eligible group's scan:
    `for read in 0..old_len { if group.meta[read].len != 0 ... { group.meta.swap(kept, read); group.blocks.swap(kept, read); kept += 1; } }`.
  - The loop visits every row of every eligible group on every retirement call. It swaps a 144-B `Meta` even when
    `kept == read`, i.e. when no block emptied. [E] The swap goes through `ptr::swap`'s overlap-safe copy path,
    hence `memmove`.
  - M1 run2's binary has no such cost (memmove 3.0-4.8%). A guard (compact only if a row emptied, or swap only when
    `kept != read`) keeps the result identical by construction [E]. It is the kernel lane's code; this lane changes
    tools only.
- **Helpers** (4 of 32 threads; runB 16,536 / 15,529 samples): the SoA kernel functions lead (`fold_le` 7.6 / 6.0%,
  `Block::forward` 7.5 / 7.3%, `Envelope::may_contain` 5.9 / 5.0%). crossbeam-epoch `pinned` + `try_advance`
  (rayon idle and steal) rises to 15.7 / 16.5% (run2 8.4 / 9.3%) as the scan work shrinks.

### 2.7 Work volume (criterion (H) units) [M]

| | runB, time window | run2, time window | runB, run2's ID range | run2, same range |
|---|---:|---:|---:|---:|
| queue pending growth per completion | 0.784 | 0.592 | **0.592** | **0.593** |
| discovered per native | 3.04 | 2.78 | **2.783** | **2.785** |
| native-pending growth per native | 0.818 | 0.725 | **0.725** | **0.725** |
| alias share of obligations | 0.413 | 0.428 | 0.428 | 0.428 |

The kernel is a CPU lever with no work-volume effect, as its strict Ordered identity predicts. The time-window
differences come from the later ID stretch that only runB reached.

### 2.8 Memory [M]

- RSS per discovered domain: 470 B at T, 489 B at the end (run2: 459 / 479 B).
- Marginal RSS per new domain:
  - fit after T+60 s: **692 B** (R² 0.956, 1,428 points; run2 847 B, R² 0.941)
  - fit over the second half of the window: **527 B** (R² 0.989, 744 points; run2 542 B, R² 0.962)
  - endpoints: 1,174 B (run2 1,469 B)
- VmHWM, which is the restore peak: 48.58 GB (run2 47.73 GB).
- Host MemAvailable: at least 522 GiB throughout.
- numa_maps at T+1,380 s: node 4 83.4%, node 6 12.9%, node 5 3.1%, node 7 0.6% (run2 89.4 / 7.0 / 3.3 / 0.3%).

### 2.9 Foreign load on CPUs 128-227 [M]

Busy CPUs estimated from /proc/stat minus the process's own CPU, sampled every 5 s:

| | before launch (5 s) | launch -> restored | window mean (p10-p90, max) | per 5-min slice |
|---|---:|---:|---|---|
| runB | 41.5 | 40.4 | **36.9** (32.8-41.0, max 43.8) | 40.2, 39.9, 35.5, 35.9, 32.9 |
| run2 | 32.4 | 36.4 | **33.4** (26.9-40.4, max 68.4) | 31.7, 35.2, 32.2, 39.3, 28.4 |
| runA (invalid) | 67.5 | 62.8 | 58.3 (51.7-64.7) | |

At 09:29Z (20 min before launch), the other load on socket 1 came from lcnbr `gammaloop`, nfink `postgres`/`gammaboard` and a codex-2
oracle verifier. Socket 1 is shared by owner decision. runB ran with +3.5 foreign CPUs on average over run2 and a
lower maximum (43.8 vs 68.4).

## 3. The gate figure and how to use it

1. **Provisional comparator figure (runB): 2.85M obligations/h** (natives + aliases) over [T, T+1,500 s] of a fresh
   gen-7 W100 resume. 5-min slices 1.95-4.56M (CV 35%). Foreign load 36.9 busy CPUs. Binary `rustred-fp-36c60fb8`.
   **1.5x = 4.27M obligations/h.** Relative to M1 run2 the comparator is 1.255x over the time window and 1.417x over
   the same ID range. The gate is scored against runC, not runB (orchestrator decision, 10:40Z). runC uses the same
   metric, window, recipe and reporting: §3 items 2-4 carry over.
2. **Uncertainty** [E].
   - This is one run (n=1).
   - The only same-state replicate pair (run1 vs run2, 12 min, the same binary) differed by ~10% in natives/h and by
     18-24% in time per unit of work.
   - Within this run the 5-min slices vary by CV 35%, driven by the work mix along the ID sequence.
   - A result near the threshold (e.g. 4.0-4.6M/h) is not decisive without replicates.
3. **Conservatism in the epoch engine's favour.** (a) The kernel gives ~3x per candidate, not 4x (orchestrator
   decision 1). (b) The compaction-loop copy of §2.6 costs ~30% of coordinator on-CPU time. [E] Removing it would
   make the comparator up to ~1.3-1.4x faster. That estimate assumes the helper batch time and the batch structure
   stay as measured; the helpers have CPU headroom (5.6 of 32 CPUs used) but are latency-bound. A gate that passes by
   less than ~1.4x may not survive a fixed comparator.
4. **The epoch pilot should report, in addition** (so the work-mix term is visible):
   - obligations/h over [T, T+1,500 s]
   - time to discharge the first 947,639 and the first 1,185,859 obligations from the same gen-7 state (run2's and
     runB's window counts): count-matched views
   - the same work-volume rows as §2.7
   - its foreign load
5. **Restore** [M]. The comparator's launch-to-restored is 615.5 s and launch-to-T 790.6 s. The P-IMP restore limit
   applies to the epoch engine's own import, not to this comparator.

## 4. runA (INVALID, diagnostic only): `TMP/w1/comparator/runA/`

**Why invalid** [M]. Binary `rustred-fp-d9163195`. Its `AggregateIndex::storage` (`queue/index.rs:893` at d9163195)
walks every group and every boxed block, and runs once per owner bucket on every 250-ms `domain_progress`
heartbeat (`execution.rs:1549-1552` -> `observe` -> `progress` -> `enrich_with(lean=false)` ->
`queue.storage_json`).
- In runA, `progress_json` took **25.9%** of coordinator wall (5-min slices 25.4-26.4%), against 0.87% in M1 run2.
- The orchestrator flagged it at 07:17Z, and runA's own profile shows it independently:
  - `AggregateIndex::storage` is 32.6% of the coordinator's on-CPU samples in the early window;
  - the attribution puts 31.2% of samples in the progress_json bucket;
  - the median progress_json time per heartbeat interval is 0.268 s (run2 0.0068 s).
- The fix round measured the walk at 92.3 ms per call; the fixed binary takes 0.135 ms (`COMPARATOR_READY.json`).
- runA was stopped cooperatively at T+1,292 s.

Binary sha256 `d38cb33d3e729123c88ce0911d42c491fec46e0b97da09450e055b6543557ec7` (walk semantics 1, CP5). Launch
06:45:35Z; restored 06:56:01Z; T 06:58:41Z. A cooperative stop file was written at 07:20:13Z. The native exited 4
(paused, gen-9 save 15.0 s) at 07:20:41Z, and the lock was released 07:20:42Z. `runA/INVALID.txt` marks the
directory. Receipts: `runA/analysis.json`, `runA/attr-*.json`, `compare-runA.json` (window cut at the external stop),
`drift/runA.json`. The clone `TMP/w1/comparator/checkpoint` is past gen 7 and is not reusable as a fixture.

| Metric [M] | runA [T, T+1,291 s] (invalid) | run2 [T, T+1,502 s] |
|---|---:|---:|
| obligations/h (natives + aliases) | 2.08M | 2.27M |
| natives/h | 1.19M | 1.30M |
| aliases/h | 0.89M | 0.97M |
| discovered/h | 3.37M | 3.61M |
| **same committed stretch** (45,890,973-46,637,071, 746k obligations): obligations/h, runA / run2 | **0.95x** (per run2 slice 0.91-1.05x, n=3) | 1 |
| coordinator duty: ordered_commit / preparation / **progress_json** / untimed | 39.3% / 23.6% / **25.9%** / 4.6% | 48.1% / 43.6% / **0.87%** / 2.5% |
| SoA kernel scan (coordinator, nested in ordered_commit) | 30.4% of coordinator wall, 77% of ordered_commit; forward 30.8 ns, reverse 242 ns per candidate | n/a |
| helper kernel scan (summed over helpers) | 1.61 CPU-equivalents, 23.9 ns per candidate, 1,337 candidates per request | n/a |
| speculative checks per request | 1,119 | 1,179 |
| commit µs per prepared record / prep µs per batch | 7.10 / 740 | 8.36 / 1,292 |
| threads (CPUs): coordinator / helpers / inspectors / total | 0.74 / 3.61 / 1.34 / 5.7 | 0.55 / 7.21 / 1.66 / 9.4 |
| run delay per thread: coordinator / helpers | 1.7% / 7.0% | 0.15% / 1.4% |
| admission CPU (coordinator + helpers) per obligation | 7.49 ms (0.61x) | 12.26 ms |
| inspector CPU per native | 4.04 ms (0.88x) | 4.57 ms |
| work volume: queue pending growth per completion / discovered per native / native-pending per native | 0.620 / 2.84 / 0.78 | 0.592 / 2.78 / 0.72 |
| initial roots closed | 8 -> 8 | 8 -> 8 |
| **foreign busy CPUs on 128-227**: before launch / restore / window mean (p10-p90) | **67.5 / 62.8 / 58.3 (51.7-64.7)** | 32.4 / 36.4 / 33.4 (26.9-40.4) |
| restore (internal): total / verify / decode / validate | 508.6 / 112.5 / 125.1 / 383.5 s (new phase `index_blocks` 13.8 s) | 490.1 / 107.9 / 124.3 / 365.8 s |
| launch -> restored / launch -> T | 625.8 / 785.9 s | 600.4 / 710.5 s |
| RSS per discovered domain at T / at end | 470 / 490 B | 459 / 479 B |
| marginal RSS: fit after T+60 s / second half | 1,045 B (R² 0.95) / 614 B (R² 0.88) | 847 B (R² 0.94) / 542 B (R² 0.96) |
| VmHWM (restore peak) | 48.6 GB | 47.7 GB |
| perf stat early (T+181 s): coordinator IPC / far-DRAM share | 0.76 / 50.6% | 0.86 / 40.7% |

5-min slices of runA [M] (obligations/h, checks/request, progress_json share):

| Slice | obligations/h | checks/request | progress_json |
|---:|---:|---:|---:|
| 1 | 1.61M | 1,042 | 26.4% |
| 2 | 1.97M | 1,200 | 26.0% |
| 3 | 1.85M | 860 | 25.4% |
| 4 | 2.58M | 1,415 | 25.4% |

Obligations/h CV 21%. runA's late perf window (T+1,260 s) overlaps the stop and the final save, so it is not used.
No numa census was taken (it was scheduled at T+1,380 s). Further diagnostics [M, perf stat early]:
- Helper IPC 1.09 (run2 0.49) at a far-DRAM share of 58.9%.
- Admission instructions per obligation 17.5M vs 17.6M.
- Inspector instructions per native 29.5M vs 21.1M, while inspector CPU per native is lower. [E] Idle-wait spinning
  dominates that counter.
- runA's reverse figure (242 ns per candidate) divides by commit-decided IDs only. It is not comparable with runB's
  24.5 ns per examined candidate.

Reading [E]: even with a quarter of the coordinator's wall in the heartbeat storage walk and 58 foreign busy CPUs,
runA matched run2 within the replicate spread (0.95x on the same stretch). This is consistent with runB: once
progress_json is fixed, the remaining coordinator limiter is the compaction copy.

## 5. Implications and open items

- **For the owner gate.** runB's provisional figure is 2.85M obligations/h (slice CV 35%), so 1.5x would be
  4.27M/h. The orchestrator decided at 10:40Z not to score the gate against runB. Next workflow: guard the
  compaction loop (index.rs:893 at 36c60fb8), re-run identity (a)-(e), then run runC (fresh gen-7 clone,
  `launch_run.sh runC <fp binary> <sha256>`, then `analyze.sh runC`). The gate is scored against runC's
  obligations/h over [T, T+1,500 s], quoted with its slice spread. runB then gives the size of the fix's effect.
  [E] runC's figure is likely higher than runB's, by up to ~1.3-1.4x.
- **For the epoch design.** The legacy engine with a faster kernel stays coordinator-serial. Coordinator on-CPU time
  is ~0.75 CPU; the helpers are latency-bound at 5.6 of 32 CPUs; the inspectors are 98% idle. Per-record commit cost
  is set by retirement and compaction, not by candidate tests. [E] That supports moving retirement and compaction off
  the serial path (epoch merge), and it warns that SoA layouts need compaction that is proportional to removals, not
  to the index.
- **Criterion (H).** The kernel does not change work volume (§2.7). Every later lever gate should report both the
  time-window and the same-range rows, because the ID sequence's work mix moves time-window ratios by up to ~1.4x
  here (1.26x vs 1.42x).
- **Restore path.** The kernel binary's post-restore preparation (restored -> rebinding save) takes 160 s vs 95 s
  (+65 s, undiagnosed). The internal restore takes +13 s (`index_blocks`).
- **Memory.** The second-half marginal is 527 B per domain (run2 542 B) and VmHWM is 48.6 GB, so the kernel adds
  no measurable marginal RSS at 74-76M domains.

## 6. Artifacts and reproduction

- `TMP/w1/comparator/runB/`: raw data (`heartbeats.jsonl`, `events.jsonl`, `samples.jsonl`, `timeline.json`,
  `numa.json`, `perf-stat-{early,late}.csv`, `perf-{early,late,dwarf}-*.data`, `sched-*.json`, `result.json`)
  and `analysis.json`, `attr-*.json`.
- `TMP/w1/comparator/compare-runB.json`: run2 vs runB, all sections of this note. The brief is
  `compare-runB.brief.json`.
- `TMP/w1/comparator/drift/{runB,run2-1500,runA}.json`: admission drift with the 1,500-s cap.
- `TMP/w1/comparator/checkpoint-runB`: the clone, now at gen 9. It is kept for the orchestrator; release it when no
  longer needed.
- runA: `TMP/w1/comparator/runA/` (INVALID.txt), `compare-runA.json`, clone `checkpoint`, now at gen 9.

```
TMP/w1/comparator/launch_run.sh runB TMP/w1-kernel/bin/rustred-fp-36c60fb8 3ab745fa338d03eb81b7e72cecc3988b23dd7abc7c077f7f548ce746cd7e8fbe > TMP/w1/comparator/runB.launcher.log 2>&1
TMP/w1/comparator/analyze.sh runB
# DWARF call-site check of §2.6 (read-only):
perf script -i runB/perf-dwarf-coordinator.data -F ip,sym,srcline --no-inline   # then llvm-symbolizer --inlining on the caller address
```
