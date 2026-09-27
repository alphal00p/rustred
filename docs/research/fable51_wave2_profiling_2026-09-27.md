# Wave-2 profiling: merged binary 53e672fc vs 102adcc3 (five-loop W50, four-loop W6, coordinator perf), 2026-09-27

Everything in this note was measured on the shared host between 2026-09-27 00:22 and 01:30 UTC, except the lines labelled
**estimate** or **interpretation**. Timings are single runs, or pairs where a repeat is shown. They are not portable.
Nothing here is a closure, termination or ETA claim. No campaign was touched: only the frozen 32fdec09 executable was
read and executed.

## 0. Setup and provenance

| Label | Executable | sha256 |
|---|---|---|
| old | `TMP/fable51-controls/bin/rustred-102adcc3` (binary of the running v2 campaign) | `102adcc345ff3010496861f6057789632718cb86dbb1c7ac52d5107bc4c1f2ae` |
| new | `TMP/fable51-controls/bin/rustred-53e672fc` (merged wave-2 integration) | `53e672fc3c082b2bbe696bd37c5badff346c5191b3cb0b010d80fa70b9534d90` |
| frozen | `campaigns/five-loop-dependency-closure/bin/rustred-32fdec09…` (read/execute only) | `32fdec098a57dd0c51aef71c01c260fb5cf7d0954b0992db08bb0f955aed358a` |

- **Five-loop finite control.** 1,324 tuples, owner `011101110111000`, family `five-finite` of `TMP/fable51-controls/run_control.py`, run through `tools/run_w50.sh`.
  - W50 (25 inspectors, 24 admission helpers, 1 coordinator) on CPUs 200-249, effective nice 5.
  - Sequential, one run at a time.
  - `tools/procmon.py` sampled `/proc` every 2 s: per-thread sched counters, VmHWM, and `/proc/stat` for CPUs 200-249.
- **Four-loop matrix.** `examples/python/walk_control_matrix.py --audit` with `matrix-w6.json`. The matrix copies the historical command lines behind `run_control.py`:
  - `TMP/four-loop-saved-descendants.VaNmUN/{fg,h,x}/command-rank12orthant.json` and `bmw/command-upstream-a19.json`.
  - Same manifest, owner base and queries; allowances equal to the historical ones; lookahead 256; checkpoint interval 3,600 s.
  - W6 on CPUs 200-205. The harness was launched under `nice -n 5` with `--nice 0`, so the effective nice is 5.
  - Order: FG round 1 first, then BMW/H/X, then FG round 2 last (for the repeat spread).
- **Shared host.** Other users' processes float over all cores, and host load average was about 100-110 of 384 CPUs. `procmon` measured the busy time on CPUs 200-249 minus our own process's CPU time. That estimates **6.7-11.8 foreign busy CPUs** on the 50 pinned CPUs during the W50 traversals (`w50-summary.json`, `procmon_traversal.foreign_mean_cpus_estimate`).

## 1. Result identity (what these controls can check)

**Ordered, strict (`compare_walk_records.py --mode strict`), new vs old: 0 differing records in every pair:**
- five-loop W50 in both rounds: 1,273,376 records each; `w50-compare-strict-new-vs-ref102.json` and `…-r2.json`;
- FG, BMW, H and X at W6, plus FG round 2 (`matrix-w6-compares/strict-*-new-vs-ref102.json`);
- the same holds for new vs the frozen 32fdec09 (`strict-*-new-vs-32fdec.json`).

Native inspections and containment checks are identical in all these pairs: five-loop 967,621 and 5,307,741,824.

**The strict verdict is still `FAIL` in every pair, for one reason only.** The top-level `descendant_closure` block differs, and only in its refresh telemetry: `refresh_count`, `refresh_seconds`, `last_refresh_seconds`, `refresh_scratch_estimate_bytes`, `retained_storage_estimate_bytes` and `snapshot_age_seconds` (`w50-closure-diff-*.json`, `matrix-w6-compares/closure-diff-*.json`).
- The semantic keys are identical: `dependency_edges`, `graph_revision`, `snapshot_revision`, `total_closed`, `initial_closed`, `unresolved_domains`, `locally_inspected`.
- `refresh_count` and `refresh_seconds` are persisted in the CP5 closure `Counters`, but they are driven by wall time. The refresh spacing is max(5 s, last scan wall / 0.01).
  - 102adcc3 itself gives 11 vs 10 refreshes in its two Ready W50 runs.
  - Ordered W50 gives 14/14 (old) vs 19/19 (new).
  - **Interpretation:** the faster CSR scans (0.23 s vs 0.14 s mean per scan) shorten the spacing. That predicts about 295 s / 23 s ≈ 13 and 272 s / 14.4 s ≈ 19 periodic refreshes, against 14 and 19 observed; forced refreshes add to the periodic ones.
- Two new-binary Ordered W50 runs compare fully `PASS`, including this block (`w50-compare-strict-new-r1-vs-r2.json`).

**`audit_owner_domain_walk.py`: PASS with 0 violations on all runs:**
- 8/8 five-loop runs (`w50-*/five-finite/audit.json`);
- 30/30 matrix cases (`matrix-w6/*/audit.json`).

**Ready multisets.** These are not expected to be equal and are reported for information only:
- Five-loop: new vs old differ in 46,805 / 56,830 record shapes.
- Four-loop new vs old: FG is equal. H and X have equal native-inspection counts, but their delegated-record counts differ (179 vs 183, 398 vs 400) and 6-10 record shapes differ (different domain boxes). BMW differs by 116 / 251 shapes, and its native inspections also differ (148,352 vs 148,389).
- The same binary also varies run to run: 102adcc3 FG round 1 vs round 2 differ by 5 shapes (`multiset-fg-rdy-ref102-r1-vs-r2.json`).
- **Five-loop Ready native inspections:**

  | Binary | Round 1 | Round 2 |
  |---|---:|---:|
  | old | 977,984 | 977,777 |
  | new | 987,795 | 988,598 |

  The new binary does +1.0% more native inspections in both rounds.
- **Interpretation:** a different readiness order changes the delegated/native mix. The audit passes.

## 2. Five-loop finite control, W50, CPUs 200-249

Directories are `wave2-profile/w50-<tag>/five-finite/` with `command.json`, `result.json`, `events.jsonl`, `metrics.json` and `audit.json`, plus `w50-<tag>/procmon/`. RSS is shown in two ways: *rc* is `run_control.py` polling every 0.5 s, and *HWM* is the maximum VmHWM that procmon sampled.

| Run | Binary | Policy | Whole s | Prep s | Traversal s | Native inspections | Containment checks | Peak RSS GB (rc / HWM) | Audit |
|---|---|---|---:|---:|---:|---:|---:|---:|---|
| ref102-ordered | 102adcc3 | Ordered | 406.8 | 86.5 | 299.75 | 967,621 | 5,307,741,824 | 15.36 / 15.31 | PASS |
| new-ordered | 53e672fc | Ordered | 380.8 | 86.5 | 271.54 | 967,621 | 5,307,741,824 | 6.55 / 6.55 | PASS |
| ref102-ready | 102adcc3 | Ready | 301.2 | 86.6 | 197.76 | 977,984 | 5,356,960,925 | 15.33 / 15.30 | PASS |
| new-ready (perf) | 53e672fc | Ready | 282.2 | 86.3 | 176.55 | 987,795 | 5,410,707,926 | 6.53 / 6.58 | PASS |
| new-ready-r2 | 53e672fc | Ready | 295.0 | 86.7 | 189.36 | 988,598 | 5,543,352,603 | 6.53 / 6.53 | PASS |
| ref102-ready-r2 | 102adcc3 | Ready | 298.8 | 86.8 | 195.54 | 977,777 | 5,302,428,915 | 15.34 / 15.31 | PASS |
| new-ordered-r2 | 53e672fc | Ordered | 379.3 | 86.2 | 275.00 | 967,621 | 5,307,741,824 | 6.54 / 6.54 | PASS |
| ref102-ordered-r2 | 102adcc3 | Ordered | 395.3 | 87.1 | 291.78 | 967,621 | 5,307,741,824 | 15.35 / 15.31 | PASS |

The rounds are listed in execution order; round 2 ran in reverse order. For reference only, the 2026-09-26 frozen-binary baseline ran on CPUs 192-241 under different load: Ordered traversal 321.5 s, Ready 236.2 s.

**Spread vs difference.**
- **Ordered traversal.** Repeat spread: old 2.7%, new 1.3%.
  - New vs old: −9.4% and −5.8%; mean 273.3 s vs 295.8 s is −7.6%.
  - The two binaries' ranges do not overlap, so the difference exceeds the repeat spread.
  - Whole-command: −6.4% and −4.0%.
- **Ready traversal.** Repeat spread: old 1.1%, **new 7.0%** (176.6 s vs 189.4 s).
  - New vs old: −10.7% and −3.2%; mean −7.0%.
  - The ranges do not overlap, but the new binary's own spread is as large as the mean difference. **Not established** at this precision.
  - The runs also do different work (+1% native inspections for new).
- **Peak RSS: −57%** (15.3 GB to 6.5 GB). Spread is ≤0.02 GB, so this is far outside noise.
- **Preparation** (owner load, about 86-87 s) is unchanged.

### 2.1 Coordinator duty and worker activity

Source: final `parallel.coordinator_duty` in `result.json`. Values are shares of the coordinator wall: `coordinator_elapsed_seconds`, which spans the traversal. *Untimed* is 1 minus the sum of all buckets.

| Run | Coord wall s | Commit | Prep | Dispatch | Publication | Progress JSON | Poll | Closure | Ready service | Checkpoint | Wait | Untimed |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| ref102-ordered | 290.1 | 27.5 | 14.8 | 6.8 | 4.5 | 13.8 | 0.5 | 1.2 | 0.0 | 0.1 | 23.6 | 7.3 |
| new-ordered | 270.8 | 23.8 | 14.8 | 6.9 | 6.2 | 13.7 | 0.5 | 1.1 | 0.0 | 0.1 | 25.5 | 7.4 |
| ref102-ordered-r2 | 282.6 | 27.1 | 14.8 | 6.1 | 4.5 | 13.8 | 0.5 | 1.2 | 0.0 | 0.1 | 24.6 | 7.3 |
| new-ordered-r2 | 274.3 | 23.0 | 17.1 | 7.1 | 6.1 | 13.7 | 0.5 | 1.2 | 0.0 | 0.1 | 24.0 | 7.3 |
| ref102-ready | 188.3 | 38.6 | 22.0 | 10.0 | 8.5 | 5.3 | 4.8 | 1.3 | 0.2 | 0.1 | 0.2 | 9.0 |
| new-ready | 175.9 | 33.8 | 23.5 | 11.6 | 9.8 | 4.9 | 4.6 | 1.2 | 0.3 | 0.1 | 0.2 | 9.9 |
| ref102-ready-r2 | 186.5 | 39.1 | 22.5 | 8.6 | 8.6 | 5.3 | 5.0 | 1.2 | 0.2 | 0.1 | 0.2 | 9.1 |
| new-ready-r2 | 188.7 | 33.8 | 24.5 | 11.1 | 9.9 | 4.7 | 4.3 | 1.2 | 0.3 | 0.1 | 0.2 | 9.8 |

**Commit per prepared record** (`admission_preparation.ordered_commit_wall_seconds / prepared_batch_records`):

| Policy | old (µs) | new (µs) | Change |
|---|---|---|---:|
| Ordered | 3.36 / 3.23 | 2.72 / 2.65 | −18% |
| Ready | 3.04 / 3.05 | 2.46 / 2.64 | −16% |

The repeat spread is ≤7.1%, so this difference exceeds it. In absolute terms, Ready commit went from 72.8 s / 73.0 s to 59.5 s / 63.8 s.

**Preparation per parallel batch:**

| Policy | old (µs) | new (µs) |
|---|---|---|
| Ordered | 177 / 174 | 166 / 195 |
| Ready | 171 / 173 | 169 / 189 |

No difference is established. Prep is the helper barrier and moves with load.

**Publication share rose in the new binary:**
- Ordered: 4.5% to 6.1-6.2% (13.1 s to 16.7 s).
- Ready: 8.5-8.6% to 9.8-9.9%.
- **Interpretation (unverified):** the C1 records sidecar now writes at publication/commit time.

**Ready is coordinator-bound in both binaries.** `wait` is 0.2% of the coordinator wall.

Inspector and thread activity over the traversal:

| Measure | Source | Ready old | Ready new | Ordered old | Ordered new |
|---|---|---:|---:|---:|---:|
| Busy inspector slots (mean, of 25) | `slot_busy_seconds` sum / coordinator wall | 4.34-4.40 | 4.68-4.80 | 2.74-2.80 | 2.82-2.93 |
| Inspector on-CPU (mean) | procmon `sum_exec_runtime` | 4.9-5.0 | 5.5-5.7 | 3.1 | 3.3-3.4 |
| Admission helpers busy (CPUs) | procmon | 4.1-4.2 | 4.5-4.6 | 2.7-2.8 | 2.9-3.0 |
| Coordinator on-CPU (share of wall) | procmon | 74% | 74-75% | 59% | 58-59% |

- The heartbeat `computing_workers` field is only present in the sparse full `parallel` blocks: 6 samples per Ready traversal, 19-24 per Ordered traversal. Its values ranged 0-24. It is too sparse to average; the procmon numbers above are the time-weighted figures.
- Each cell above gives the range over both rounds.

## 3. Coordinator perf profile (new binary, Ready W50)

**Setup.**
- Command: `perf record -F 499 -e cpu-clock:u -t <TID>`, where TID = PID 1000330 (the main thread, i.e. the coordinator).
- Window: native elapsed 151.95-214.53 s, which is 62.2 s wall in the middle of the 86.3-262.9 s traversal.
- Result: 16,690 samples, 0 lost.
- A concurrent `perf stat` ran on the same thread.
- perf is perf 7.2.0 via `nix shell nixpkgs#perf`, with `perf_event_paranoid=2`, so user space only. Kernel time does not appear in the profile; it is covered by the sched counters below.
- Raw data is in `w50-new-ready/procmon/`:
  - `perf-coordinator.data`
  - `perf-report-symbol.txt` (full) and `perf-report-symbol-head80.txt`
  - `perf-report-dso.txt`
  - `perf-stat-coordinator.txt`
  - `sched-perf-{start,end}.{txt,json}`
  - `perf-window.json`

**Duty over the window.** Only two full heartbeat blocks fall inside it (160 s and 172 s), so this covers just that 12 s:

| Bucket | Share |
|---|---:|
| Commit | 35.6% |
| Prep | 24.9% |
| Dispatch | 10.7% |
| Publication | 8.8% |
| Progress JSON | 5.4% |
| Poll | 4.3% |
| Wait | 0.1% |

These match the session totals in §2.1.

**DSO split:** rustred 66.5%, libc 32.7%, vdso 0.8%.

**Top symbols** (flat, `--no-children --sort symbol`, share of the coordinator's user-mode samples):

| # | Share | Symbol (abridged) |
|---:|---:|---|
| 1 | 13.04% | `__memmove_avx512_unaligned_erms` (libc) |
| 2 | 6.85% | `queue::index::AggregateIndex::find` (from `Queue<15>::admit_with_lookup`) |
| 3 | 5.20% | `_int_free_chunk` (libc) |
| 4 | 3.53% | `queue::compact::Stored<15>::contains` |
| 5 | 3.49% | `_int_malloc` |
| 6 | 3.23% | `BTreeMap<String, serde_json::Value>` `VacantEntry::insert_entry` |
| 7 | 3.02% | `_blake3_compress_in_place_avx512` |
| 8 | 2.99% | `cfree` |
| 9 | 2.97% | `queue::compact::ExactIndex<15>::get` |
| 10 | 2.82% | `__memcmp_evex_movbe` |
| 11 | 2.70% | btree `Handle<…String, serde_json::Value…>::insert_recursing` |
| 12 | 2.68% | `Queue<15>::admit_prepared` |
| 13 | 2.65% | `BTreeMap<String, serde_json::Value>::insert` |
| 14 | 2.52% | `AggregateIndex::retire_prepared` |
| 15 | 2.25% | `blake3::ChunkState::update` |
| 16 | 1.77% | `queue::compact::CompactSummary<15>::contains` |
| 17 | 1.63% | `descendant_closure::Tracker::scan` edge iteration (`Chain<Copied<Iter<u32>>, edges::Chain>::try_fold`) |
| 18 | 1.45% | `std::sync::Mutex::lock_contended` |
| 19 | 1.38% | `malloc_consolidate` |
| 20 | 1.37% | `blake3::Hasher::update` |
| 21 | 1.25% | `admission::Engine::commit_chunk` |
| 22 | 1.25% | `AggregateIndex::retire` (from `admit_with_lookup`) |
| 23 | 1.05% | `AggregateIndex::is_live` |
| 24 | 1.02% | SipHash-1-3 `Hasher::write` |
| 25 | 0.98% | `parallel::Pool<15>::snapshot_tier` |
| 26 | 0.96% | `malloc` |
| 27 | 0.92% | `Queue<15>::admit_with_lookup` |
| 28 | 0.88% | `parallel::Pool<15>::poll` |
| 29 | 0.81% | `HashMap<index::Signature, usize>::get` |
| 30 | 0.78% | `__vdso_clock_gettime` |

**Categories.** Built by symbol-name regex, first match wins (`tools/perf_categories.py`, output `perf-categories.json`). This is a flat profile: libc memmove and malloc/free samples cannot be assigned to their Rust callers without call graphs.

| Category | Share |
|---|---:|
| Admission index / containment (`queue::index`, `queue::compact`, `Queue::admit*`, `commit_chunk`) | 29.9% |
| malloc/free (libc and Rust alloc shims) | 16.7% |
| memmove/memcpy/memcmp (libc) | 15.9% |
| serde_json `Value` / `BTreeMap<String, Value>` build, serialize, drop | 15.1% |
| blake3 / replay hashing | 7.2% |
| Pool sync / dispatch / poll / escrow (`parallel.rs`, mutex, condvar, syscall, clock) | 6.5% |
| SipHash / hashbrown (other) | 3.4% |
| Descendant-closure scan / edges | 2.9% |
| Execution state | 0.9% |
| Unclassified | 1.5% |

**perf stat (coordinator, 60 s):**

| Counter | Value | Derived |
|---|---:|---|
| cycles:u | 108.7 G | |
| instructions:u | 168.2 G | **IPC 1.55** |
| cache-misses:u | 1.087 G | 6.5 per 1k instructions |
| dTLB-load-misses:u | 94.1 M | 0.56 per 1k instructions |
| task-clock | 45.2 s | |

**Sched counters** (`/proc/<pid>/task/*/sched`, from the perf-start and perf-end snapshots, 62.24 s apart):

| Thread(s) | Δ `se.sum_exec_runtime` | Δ voluntary | Δ involuntary | Δ migrations | utime / stime |
|---|---:|---:|---:|---:|---|
| Coordinator (TID = PID) | 45.87 s (73.7% of wall) | 236,758 (3.8k/s) | 106 | 8,780 (141/s) | 36.7 s / 9.1 s |
| Other 51 threads, summed | 662.5 s (10.6 CPUs) | 11,527,610 | 6,411,556 | 319,549 | 550.6 s / 111.9 s |
| of which 25 inspectors | 365.8 s (5.9 CPUs) | 8,739,436 (140k/s) | 5,750 | 136,650 | 331.0 s / 34.9 s |
| of which 24 admission helpers | 296.1 s (4.8 CPUs) | 2,788,112 | 6,405,799 (103k/s) | 182,876 | 219.7 s / 76.5 s |

**Interpretation**, relative to the coordinator relief design note (`docs/research/fable51_coordinator_relief_design_2026-09-26.md`):
- **E1** (commit is DRAM/TLB-latency bound, IPC < 0.5) is **not** supported at this control's scale. Here the queue holds about 1.29M domains; production holds more than 20M, so E1 is still untested there.
- **The item-2 gate cannot be decided from a flat profile.** The gate needs at least 25% of coordinator samples in the "removable" hit-path set.
  - Admission-index symbols alone total 29.9%.
  - The libc copy and allocator time (32.6% together) has unknown callers.
  - The M1 call-graph run (frame-pointer build, `--call-graph fp`) is still required.
- **serde_json `Value` work is 15.1% of user samples.** That is larger than the `progress_json` duty share (4.9%). This suggests record and telemetry `Value` construction on the commit/publication path is a separate, sizeable cost, which is relevant to items 1a and 5. The call sites are unverified.
- **blake3 replay hashing is 7.2%.**
- **Wake-ups.** Inspectors take about 140k voluntary switches/s while about 5.9 of 25 are on-CPU. This is consistent with the `notify_all` thundering-herd hypothesis (E3, item 1b), but it is not proof.
- **Admission helpers** show 103k involuntary switches/s and 26% system time. This is consistent with rayon's spin/yield idle loop between batches (relevant to 3a).
- **Coordinator migrations** run at 141/s across two NUMA nodes (relevant to H9 pinning).

## 4. Four-loop matrix, W6, CPUs 200-205

Source: `matrix-w6/` (per case: `command.json`, `run/`, `summary.json`, `audit.json`), plus the harness `RESULTS.md` and `matrix-receipt.json`.
- Column *whole s* is the supervisor's `wait4` wall. It is quantized by the supervisor's 2 s sampling loop, so treat it as ±2 s.
- Column *native elapsed* is preparation plus traversal, from `result.json`.
- Column *RSS* is the `wait4` `ru_maxrss` of the supervisor tree, i.e. the native process.

| Case | Policy | Binary | Whole s | Native elapsed s | Traversal s | Native inspections | Containment checks | Peak RSS GiB | Audit |
|---|---|---|---:|---:|---:|---:|---:|---:|---|
| FG | Ordered | 32fdec09 | 18.10 | 14.63 | 13.52 | 98,869 | 169,509,549 | 1.09 | PASS |
| FG | Ordered | 102adcc3 | 16.09 | 13.96 | 12.89 | 98,869 | 169,509,549 | 1.09 | PASS |
| FG | Ordered | 53e672fc | 16.09 | 13.09 | 12.00 | 98,869 | 169,509,549 | 0.24 | PASS |
| FG | Ready | 32fdec09 | 18.09 | 16.13 | 15.11 | 98,841 | 169,069,040 | 1.08 | PASS |
| FG | Ready | 102adcc3 | 16.10 | 13.15 | 12.02 | 98,846 | 169,315,799 | 1.08 | PASS |
| FG | Ready | 53e672fc | 16.09 | 12.32 | 11.24 | 98,846 | 169,416,501 | 0.23 | PASS |
| BMW | Ordered | 32fdec09 | 40.12 | 35.40 | 33.83 | 147,233 | 653,022,941 | 1.74 | PASS |
| BMW | Ordered | 102adcc3 | 36.13 | 32.81 | 31.24 | 147,233 | 653,022,941 | 1.74 | PASS |
| BMW | Ordered | 53e672fc | 34.12 | 29.77 | 28.19 | 147,233 | 653,022,941 | 0.42 | PASS |
| BMW | Ready | 32fdec09 | 44.14 | 40.54 | 39.01 | 148,237 | 661,336,874 | 1.71 | PASS |
| BMW | Ready | 102adcc3 | 34.12 | 29.54 | 27.93 | 148,352 | 661,433,302 | 1.71 | PASS |
| BMW | Ready | 53e672fc | 32.13 | 28.13 | 26.55 | 148,389 | 659,678,871 | 0.37 | PASS |
| H | Ordered | 32fdec09 | 16.10 | 15.23 | 12.66 | 24,680 | 15,228,826 | 0.64 | PASS |
| H | Ordered | 102adcc3 | 16.09 | 15.07 | 12.46 | 24,680 | 15,228,826 | 0.65 | PASS |
| H | Ordered | 53e672fc | 16.10 | 14.93 | 12.30 | 24,680 | 15,228,826 | 0.47 | PASS |
| H | Ready | 32fdec09 | 16.09 | 15.34 | 12.85 | 24,777 | 15,258,201 | 0.62 | PASS |
| H | Ready | 102adcc3 | 16.10 | 14.66 | 12.13 | 24,777 | 15,094,067 | 0.62 | PASS |
| H | Ready | 53e672fc | 16.11 | 14.57 | 11.90 | 24,777 | 15,204,105 | 0.45 | PASS |
| X | Ordered | 32fdec09 | 42.14 | 39.02 | 34.82 | 46,826 | 20,507,017 | 1.26 | PASS |
| X | Ordered | 102adcc3 | 40.13 | 37.86 | 33.45 | 46,826 | 20,507,017 | 1.26 | PASS |
| X | Ordered | 53e672fc | 40.14 | 37.57 | 33.22 | 46,826 | 20,507,017 | 0.87 | PASS |
| X | Ready | 32fdec09 | 38.13 | 34.90 | 30.79 | 46,826 | 20,479,664 | 1.16 | PASS |
| X | Ready | 102adcc3 | 36.13 | 33.75 | 29.53 | 46,826 | 20,492,440 | 1.16 | PASS |
| X | Ready | 53e672fc | 36.13 | 33.02 | 28.77 | 46,826 | 20,493,642 | 0.87 | PASS |
| FG (r2) | Ordered | 32fdec09 | 18.11 | 14.81 | 13.67 | 98,869 | 169,509,549 | 1.09 | PASS |
| FG (r2) | Ordered | 102adcc3 | 16.10 | 14.02 | 12.89 | 98,869 | 169,509,549 | 1.09 | PASS |
| FG (r2) | Ordered | 53e672fc | 16.11 | 13.24 | 12.06 | 98,869 | 169,509,549 | 0.25 | PASS |
| FG (r2) | Ready | 32fdec09 | 20.11 | 16.25 | 15.15 | 98,841 | 167,513,948 | 1.08 | PASS |
| FG (r2) | Ready | 102adcc3 | 16.10 | 13.17 | 12.04 | 98,841 | 169,065,065 | 1.08 | PASS |
| FG (r2) | Ready | 53e672fc | 16.11 | 12.33 | 11.18 | 98,846 | 169,240,906 | 0.23 | PASS |

**FG repeat spread** (|r1 − r2| / mean over the 6 (policy, binary) pairs; `matrix-w6-analysis.json`):

| Measure | Max spread | Note |
|---|---:|---|
| Traversal | 1.1% | 32fdec09 Ordered: 13.52 s vs 13.67 s |
| Native elapsed | 1.2% | |
| Supervisor whole-command | 10.6% | one 2 s tick: 18.09 s vs 20.11 s; unusable at this scale |
| Peak RSS | ≤0.01 GiB | |

**New vs 102adcc3, traversal (native elapsed in brackets); multiple of the 1.1% FG spread:**

| Family | Ordered | Ready | Verdict |
|---|---|---|---|
| FG | −6.9% (−6.3%), 6.1x | −6.5% (−6.3%), 5.7x; round 2 −7.1% | **exceeds** |
| BMW | −9.8% (−9.3%), 8.6x | −4.9% (−4.8%), 4.4x | **exceeds** |
| H | −1.3% (−0.9%), 1.1x | −1.9% (−0.6%), 1.7x | marginal; native elapsed within spread; **not established** |
| X | −0.7% (−0.8%), 0.6x | −2.6% (−2.2%), 2.3x | Ordered within spread; Ready marginal, **not established** |

**New vs 102adcc3, peak RSS:**

| Family | Change | Absolute |
|---|---:|---|
| FG | −78% / −79% | 1.09 → 0.24 GiB |
| BMW | −76% / −78% | |
| H | −28% | |
| X | −31% / −25% | |

All RSS changes are far outside the spread.

**Against the frozen 32fdec09** (these include 102adcc3's own gains):

| Family | Ordered traversal | Ready traversal |
|---|---:|---:|
| FG | −11.2% | −25.6% |
| BMW | −16.7% | −31.9% |
| H | −2.8% | −7.4% |
| X | −4.6% | −6.5% |

Every one of these exceeds the spread.

**Caveat on the spread.** The FG spread is the only repeat measured at four loops. BMW, H, X and Ready may be noisier; five-loop Ready showed 7% in §2. The H and X verdicts above are therefore deliberately conservative.

## 5. Caveats

- Single host, shared with other users. The foreign load on the pinned CPUs was 7-12 busy CPUs.
- The five-loop numbers are two rounds each. The Ready traversal difference is not established; the Ordered difference is.
- The perf profile is flat, user-space only, and one 60 s window of one run. Shares are of user-mode samples; the coordinator also spent 9.1 s of its 45.9 s on-CPU in the kernel.
- The heartbeat duty/computing blocks are sparse (6-24 per run). Duty shares come from the final `result.json`; activity comes from procmon.
- The control scale (about 1.3M domains, 15 GB) is 10-20x smaller than the v2 campaign. Scale-dependent costs cannot be extrapolated from it: commit per record grew from 2.6 to 4.3 µs in production.
- Not covered here: `WALK_SEMANTICS_VERSION`, and CP5 resume and rollback against 102adcc3 checkpoints. Those are separate gates.

## 6. Files (all under `TMP/fable51-controls/wave2-profile/`)

- `w50-<tag>/`
  - `five-finite/{command.json,result.json,events.jsonl,metrics.json,audit.json,stderr}`
  - `procmon/{meta.json,samples.jsonl,sched-{first,last}.{txt,json}}`
  - `run_control.stdout`, `loadavg_{start,end}`, `{started,finished}_utc`
- `w50-new-ready/procmon/`: perf files, as listed in §3.
- `w50-summary.json`: every derived number in §2.
- `w50-compare-*.json`, `w50-closure-diff-*.json`.
- `matrix-w6.json`, `matrix-w6/` (harness output), `matrix-w6-table.md`, `matrix-w6-analysis.json`.
- `matrix-w6-compares/`: strict, multiset and closure diffs.
- `tools/`: `run_w50.sh`, `procmon.py`, `analyze_w50.py`, `closure_diff.py`, `perf_categories.py`, `matrix_table.py`, `matrix_compares.sh`.
