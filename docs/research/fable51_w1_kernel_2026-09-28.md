# W1.1 admission kernel on the legacy lanes (v3 S1): gates and the W2 comparator, 2026-09-27/28

Plan item: `docs/research/fable51_next_push_master_plan_2026-09-27.md` §5 W1 item 1.1 (= v3 S1), branch
`fable_5_1-v3-kernel` from `fable_5_1` e74d31ce. Labels: **[M]** measured (paths given), **[E]** estimate or
interpretation. Nothing here is a closure, termination or ETA claim. No campaign was started, stopped or written.
No algebraic code was written: the change is data layout, prefiltering and telemetry around the existing exact
predicate, so the Symbolica triple-check does not apply.

**Owner decision (2026-09-27 ~21:35 UTC, `3bc4349d`):** the full plan continues only if the W2 epoch skeleton
(S2/S4) beats *legacy engine + SoA kernel* by >= 1.5x (25-min gen-7 resume, matched window vs M1 run2). The binary
of this note is that comparator (§6, §9).

**Fix round (2026-09-28, §9).** A review found that the d9163195 binaries walked every index block twice per
coordinator heartbeat (85-94 ms per walk at gen 7 [M]; 25.6% of the coordinator in the first comparator run [M]), and
that the gate (c) timing and several gate procedures were biased or incomplete. The fix-round binaries
`rustred-36c60fb8` / `rustred-fp-36c60fb8` supersede the d9163195 pair; every gate below was re-run on them.
Sections 2-8 keep the d9163195 record, corrected where the review showed an error; §9 carries the fix round.

## 0. Verdict (fix-round binaries, §9)

| Gate (plan §5 W1.1) | Result |
|---|---|
| (a) strict Ordered identity, records and `containment_checks`, vs 4a17f9c7 on C-4L (FG/BMW/H/X + four-all) and C-5F | **PASS** [M] for the plain and the fp binary (§9.3) |
| (b) >= 1e8 forward and >= 1e8 reverse differential pairs from gen 7, incl. wide, empty, infinite, saturated | **PASS** [M]: 169M / 903M lookup pairs, 1.50e9 / 1.50e9 sweep pairs (every kind swept), 0 mismatches, 3 receipts (§9.2) |
| (c) >= 4x less CPU per candidate in the 0.4 replay | **FAIL** [M]: unbiased 2.92-3.03x forward, 3.72-4.00x reverse single-thread (3 receipts); 2.99-3.13x / 3.82-3.90x on 8 concurrent threads (§9.2) |
| resume across binaries (both directions, FG Ordered strict) + FG Ready oracle gate / multiset | **PASS** [M]; oracle `roots_independently_verified == roots_total` on every Ready and Ordered output (§9.4) |
| full gen-7 CP5 restore (production restore path, no walk) | **PASS** [M]: all manifest counts equal, directory unchanged, 564 s restore, 48.6 GB VmHWM (§9.5) |
| lib suite, `cli_routed_campaign`, Python suite, `cargo fmt --check` | **PASS** [M]: 785/0/7, 6/6, 228 OK / 1 skipped, clean (§9.6) |
| clean C-5F A/B vs 4a17f9c7 (not a W1.1 gate; review item P6) | fp comparator at least on par [M]: traversal 0.965x mean over 2 interleaved rounds (§9.3) |

The comparator binary for the owner's 1.5x gate is `TMP/w1-kernel/bin/rustred-fp-36c60fb8` (plain twin
`rustred-36c60fb8`); `TMP/w1-kernel/bin/COMPARATOR_READY.json` lists the gate evidence.

## 1. What changed

Commits on `fable_5_1-v3-kernel`:

| Commit | Content |
|---|---|
| 933128cc | Struct-of-arrays admission kernel on the legacy lanes |
| d5a0b968 | Gen-7 kernel harness (ignored release test; verbatim historical read paths in `queue/tests/legacy.rs`) |
| f40d1f40 | Immutable per-ID summaries at 92 B (N=15) instead of 176 B |
| d9163195 | Kernel attribution telemetry (exact tests and scan wall per side); harness filter breakdown; CP5 index re-encode check |
| da9d2499 | Test only: pin the new heartbeat keys in the key-set freeze test |

- **Blocks** (`queue/index/blocks.rs`): each group keeps its live candidates in blocks of at most 32, in increasing
  admission-ID order. Per block: contiguous `Meta` (ID bounds, length, the historical coordinate envelope) and a
  boxed `Block` with parallel arrays: u32 IDs, u64 filter words, one u8 lane row per comparison field (2N+6), and
  block OR/AND words.
- **A5 lanes**: a monotone saturating map to u8 per field (coordinate/A/R lowers `min(v,255)`; uppers +inf -> 0,
  finite -> `255 - min(v,254)`; D with bias 128). Inclusion C ⊇ Q implies `lane(C) <= lane(Q)` in every field, so the
  lane test is a necessary condition in both directions. A saturated finite bound never shares a code with an
  infinity, which makes A5's two escape rules hold by construction. Empty summaries and empty queries are escaped.
  **Lanes and words only prefilter; A1 (the exact predicate) decides every candidate that passes.**
- **Historical traversal kept exactly**: min-ID rule, callback order, and the per-block coordinate envelope that
  decides which candidates are charged to `containment_checks` (u16 codes, exact u64 fallback). This is why
  `containment_checks` is identical (§2).
- **`partition_point` skip**: `find_from` skips the blocks below `first_id`.
- **Immutable per-ID summaries** (`queue/compact.rs`): every admitted ID keeps its `CompactSummary` for good (live,
  retired or aliased), 92 B at N=15 (u16 coordinates with u16::MAX = +inf, u32 A/R, i32 D). Anything wider is the
  existing `wide` marker, whose comparisons rebuild the native summaries from the queued domain: no input is
  approximated. The removed slab held 176 B per live candidate plus a 4-B slot per ID.
- **Release WIDE check (A1)**: `CompactSummary::contains` returns `None` for a wide side and the caller compares
  native summaries (was a debug assertion). Every positive re-checks the stored image's phase and owner; the orthant
  shortcut is verified (bucket, full orthant, rank) before it counts.
- **CP5**: restore rebuilds words and lanes from the summaries; the CP5 index image is byte-identical (stale dead slots
  included), now also asserted on the gen-7 checkpoint by the harness (§3).

## 2. Gate (a): strict Ordered identity vs 4a17f9c7 [M]

Reference `TMP/fable51-controls/bin/rustred-4a17f9c7`; new `TMP/w1-kernel/bin/rustred-d9163195` (sha256
`86a7d345f138166a97fff2eece2014bd4ca6905bf868d762a3a9115185f00d98`, built 21:55Z from d9163195 with `crates/` == HEAD,
`vendor/symbolica` 953e26e2 + the heap-pow patch). Runner `TMP/w1-kernel/identity.sh` (`run_control.py`,
`--policy ordered`, same session, same CPUs, ref then new per family), comparator
`examples/python/compare_walk_records.py --mode strict`; `containment_checks` read from each run's metrics.

| Control | Records (each side) | Differing | `containment_checks` ref = new | Workers / CPUs |
|---|---|---|---|---|
| FG | 98,909 | 0 | 169,509,549 | W6 / 72-77 |
| BMW | 158,951 | 0 | 653,022,941 | W6 / 72-77 |
| H | 24,929 | 0 | 15,228,826 | W6 / 72-77 |
| X | 47,193 | 0 | 20,507,017 | W6 / 72-77 |
| C-4L `four-all` | 65,444 | 0 | 43,338,845 | W6 / 72-77 |
| C-5F `five-finite` | 1,273,376 | 0 | 5,307,741,824 | W16 / 72-79,328-335 |

All twelve runs exited 0 with 0 frontiers (967,621 natives on C-5F). Outputs: `TMP/w1-kernel/identity-d916/` and
`identity-d916c5f/` (C-5F), run directories `TMP/fable51-controls/w1k-{ref,new}-d916{,c5f}/`. The first C-5F attempt
in `identity-d916/` is **void**: `--workers 24` on a 16-CPU mask made both binaries refuse to start (exit 4, 0 records),
so its "PASS" compared two empty results; the script now uses W16 (Ordered results do not depend on the worker count)
and flags any nonzero exit. (The "VOID" remark had been typed into the machine log by hand; it now lives in
`identity-d916/ANNOTATIONS.md`, and `compare_walk_records.py` returns INVALID for such a pair, §9.7.) The b458fde0 build (f40d1f40, before the telemetry) also passed FG (`identity-smoke-b458/`).

**Wall times from these runs are not evidence** [M]: a release compile (8 cores, 58 GB RSS) and the C-5F run shared the
lane's 16 CPUs, so every coordinator duty bucket, including JSON serialization that the kernel does not touch, moved
by 2-5x between runs in both directions (e.g. BMW new 46.5 s vs ref 167.5 s; X new 133.4 s vs ref 55.8 s). Clean
timing is §4 and §6.

## 3. Gate (b): gen-7 differential [M]

Harness: the ignored release test `queue::tests::gen7_kernel::gen7_kernel_differential_and_cost` (test binary of
da9d2499), run by `TMP/w1-kernel/harness.sh` on CPUs 72-79 alone. It restores only the queue sections of a read-only
block clone of the v2 generation-7 checkpoint (`TMP/w1-kernel/gen7`) twice: into the new kernel, and into a verbatim
copy of the pre-kernel read paths (`queue/tests/legacy.rs`: index blocks, 176-B summary slab, per-ID callbacks).
Requests: 60,000 traced gen-7 index requests (every 400th of 54.4M in
`TMP/w0/intel/g7-trace-resume/trace/coord-823745.bin`) plus 15,000 each of admitted-domain, empty, infinite,
saturated and wide variants (135,000 in total). Every logical candidate of every forward and reverse lookup is
compared with the historical predicate, and every 20th request also sweeps its whole bucket.

Receipt `TMP/w1-kernel/harness-da9d/receipt.json` (380 s wall, 41.8 GB peak RSS):

| Item | Value |
|---|---|
| admitted IDs / live candidates | 74,156,033 / 37,907,667 |
| kernel image rebuilt from the immutable summaries vs independent derivation | 0 mismatches |
| CP5 index section re-encoded from the restored kernel | byte-identical |
| forward pairs (lookups) / reverse pairs (lookups) | 169,178,584 / 903,179,098 |
| forward / reverse pairs in bucket sweeps (exact) | 1,492,285,478 / 1,492,285,478 |
| pair mismatches / lookup-result mismatches | **0 / 0** |
| contained pairs forward / reverse | 223,091 / 14,466,675 |

**Gate (b): PASS** (≥ 1e8 in each direction: 1.7x the threshold forward and 9x reverse in lookups alone, 15x each in
the sweeps). *Correction (review P9):* the sweep selector (global request index modulo 20) aliased with the period-5
layout of the synthetic variants, so the sweeps covered only traced and `infinite` requests (hence exact = sweep
pairs); the wide, empty, saturated and admitted requests were compared in lookups only. §9.2 sweeps every kind. The three later replicate runs
(§4) repeated the whole differential with 0 mismatches each. Caveat [M]: the gen-7 store holds **0 wide summaries**
(and 0 lossy lane images), so the wide path is exercised only by the 15,000 synthetic wide query variants, not by
stored wide candidates; the unit tests of 933128cc cover stored wide summaries.

## 4. Gate (c): CPU per candidate [M]

Same harness, timing phase: one thread, old and new interleaved per chunk of 250 traced requests (order alternated,
2 repeats), thread on-CPU nanoseconds from `/proc/thread-self/schedstat` divided by logical candidates in
`containment_checks` units (reverse: examined candidates). *Correction (review P8):* the "foreign busy time 0" of
these receipts is not evidence: the timing thread was not pinned to the CPU whose load was read (busy < own time in all
four receipts) and a negative foreign load was clamped to 0. *Correction (review P5):* the timing ran in the test
binary with the test-only index work counters on (atomic adds per group and block, in the new arm only). §9.2
re-measures with both fixed. This is the W0.4 unit (CPU per tested candidate at 74M, same checkpoint, same trace) measured on the production kernel,
not on the W0.4 prototype layouts.

| Run | Timed requests | Forward ns/check old / new | Ratio | Reverse ns/check old / new | Ratio |
|---|---|---|---|---|---|
| `harness-da9d` | 20,000 | 88.4 / 28.7 | 3.08x | 68.8 / 22.1 | 3.11x |
| `harness-da9d-perf` | 60,000 | 89.8 / 30.9 | 2.90x | 69.6 / 18.3 | 3.79x |
| `harness-da9d-perf2` | 60,000 | 92.5 / 32.3 | 2.86x | 66.5 / 18.7 | 3.55x |
| `harness-da9d-perf3` (perf attached) | 60,000 | 78.9 / 27.5 | 2.88x | 57.1 / 16.0 | 3.57x |

(Old-side forward range over the four runs: 78.9-92.5 ns, not the 88-92 ns quoted in the first lane report.)

Per request (first row): forward 106.5 -> 34.6 µs, reverse 75.1 -> 24.2 µs. In the timed arms the new kernel's bit
word rejects 88% of forward and 94% of reverse candidates, the u8 lanes another 12% / 6%, and 0.17% / 0.001% reach
the exact predicate (`new_filter` in the receipts).

**Gate (c): FAIL as specified** [M]: 2.9-3.1x forward and 3.1-3.8x reverse, against >= 4x. The per-outcome
`hit_ratio` / `miss_ratio` rows of the receipts (1.13-1.26x / 2.6-2.9x) are not usable: they time one request at a
time (two `/proc` reads per request inside the window) right after the same request was run to classify it (warm
cache), which biases both towards 1.

**Why the production kernel is ~3x slower per candidate than the W0.4 prototype** (6.6-11 ns SoA-id vs 16-32 ns
here; the old arm, 88-92 ns, matches W0.4's "today" 75.5 ns) [M, `perf record -e cpu-clock:u` on the timing phase only,
`harness-da9d-perf3/perf.data` and `annotate.txt`]:
- The new kernel takes 20.6% of the timing phase's samples and the historical paths 76.1% (3.7x, consistent with the
  table). The exact predicate is 0.2%.
- Every hot spot of the new kernel is a first touch of memory, not arithmetic: in `find_controlled`, 28% of its samples
  sit on loading `meta.len` in the loop that skips empty blocks (`Meta` is 144 B and `len` is at offset 136, so each
  block costs a fresh cache line), 21% on the first load of a boxed block's `ids`; in `fold_le` (SSE2-vectorized),
  73% on loading a lane row; in `Block::forward`, the loads of the u64 word array.
- The layout is inherited [M, `storage` in the receipt]: 2.61M block rows (`index_row_bytes` / 144 B) for 1.99M
  allocated 1,600-B blocks and 37.9M live candidates, i.e. about 19 live candidates per allocated block and about a
  quarter of the rows empty. The block boundaries cannot be repacked: each block's historical envelope decides which
  candidates are charged to `containment_checks` (gate (a)), and CP5 must round-trip byte for byte. The W0.4
  prototype had neither constraint [E].

Fixes that keep gate (a), with estimates [E, not implemented, not measured]:
1. Hot/cold split of `Meta`: `first`, `last`, `len` in their own 12-B array (5 per cache line), envelopes in a parallel
   array read only for non-empty in-range blocks. Targets the 28% meta share of `find_controlled` and the reverse
   scan's `may_be_contained`. Safe code.
2. Software prefetch of the next non-empty eligible block (words, then the first lane rows) while the current block is
   processed, via `_mm_prefetch` inside the kernel: this needs one `unsafe` call on rustc 1.97.1 (checked), which plan
   §3.3 allows only in the confined SIMD kernel. Targets the block first-touch latency (21% plus the fold and word
   loads).
3. `find_controlled` binary-searches each eligible group's rows (`partition_point`) even when `first_id` is 0 (every
   coordinator lookup and every harness request); skip it when `first_id` is at most the group's first ID.
4. `#[inline]` on `fold_le` / `fold_ge` (separate call per fold today; small).
Together these plausibly reach 4x forward [E]; each would need gates (a)-(c) re-run on a new binary (about 2 h of lane
time). Not started without a decision, because it would change the comparator binary of §6.

## 5. Resume and Ready [M]

`TMP/w1-kernel/resume.sh` -> `TMP/w1-kernel/resume-d916/`, runs under `TMP/fable51-controls/w1k-{resume,ready}-*-d916/`.

- **FG Ordered, 4a17f9c7 -> d9163195** (pause requested at >= 40,000 committed; paused at 57,957, exit 4; resumed to
  completion, exit 0): strict vs the same-session reference result, **0 differing of 98,909 records**; the only
  top-level difference is `uncommitted_inspections` (the inspection cancelled by the pause, carried as a receipt).
- **FG Ordered, d9163195 -> 4a17f9c7** (paused at 52,817): same outcome.
- Every earlier resume gate on this host shows the same `uncommitted_inspections` difference (`final-4a17f9c7-resume`,
  `wave2-resume-ord-fix`, `b2-resume-ord-fix`, `c1-head-resume-ord`; the 102adcc3 -> 4a17f9c7 one additionally differs
  in `descendant_closure`), so `resume_control.py` reports `compare_exit` 1 for all of them; the criterion used here,
  as before, is 0 differing records with only that field at top level.
- **FG Ready**, ref and new: `audit_owner_domain_walk.py` **PASS** for both; `compare_walk_records.py --mode multiset`
  **PASS** (98,886 records each). *Correction (review P7):* that audit was the pre-oracle script of this branch and no
  script asserted `verdict == PASS && roots_independently_verified == roots_total` (directive 0.1 item 6); §9.4 re-runs
  gate (d) with the W0.2 oracle.

### 5.1 Full gen-7 CP5 restore under the kernel code [M]

The existing ignored test `checkpoint::scale_tests::restore_copied_production_checkpoint` (test binary of da9d2499;
`TMP/w1-kernel/scale_restore.sh`, receipt `TMP/w1-kernel/scale-restore-da9d/receipt.json`) resumed the gen-7 clone
through the production path: the v2 run's frozen argv (`--checkpoint` -> `--resume`), `Store::open` (request binding,
semantics, every section digest), `Store::resume` (decode and validate every section), ledger/closure cross-check,
owner digest binding (67 owners), no walk and no save. `RAYON_NUM_THREADS=1`, CPUs 72-79, alone.

- **PASS**: every manifest count equal (74,156,033 domains, 45,889,639 committed, 27,465,422 natives, 1,192,281,291
  edges, 6,052,803,060 committed events), 37,907,667 live candidates (as in the harness), checkpoint directory
  unchanged, `checkpoint_executable_changed` emitted (saver 102adcc3's blake3 `4cb4ab28…`).
- 946 s total, 47.5 GB peak RSS, 29.4 GB after restore.
- Phase times vs M1 run2's production restore (102adcc3-equivalent code, W100 pools): phases the kernel does not touch
  ran 1.3-1.7x slower here (`domains_decode` 77.9 vs 59.4 s, `edges_decode` 79.2 vs 54.2 s,
  `ledger_closure_cross_check` 194.5 vs 111.7 s), so the environment factor is about 1.4 [E]. The queue index now
  takes `index_summaries` 64.7 s + the new `index_blocks` 23.1 s against run2's `index_summaries` 43.1 s: about 20-25 s
  more after the environment factor [E].

## 6. The W2 comparator (legacy engine + SoA kernel)

*Superseded by §9 for the binaries, the telemetry keys and the timing evidence.* The d9163195 binaries below carry an
O(blocks) `queue_storage` walk per heartbeat (§9.1) and must not be used as the comparator.

**Binaries** [M] (d9163195, superseded):
- `TMP/w1-kernel/bin/rustred-d9163195`: plain release, sha256 `86a7d345…0d98`, the binary gated in §2 and §5.
- `TMP/w1-kernel/bin/rustred-fp-d9163195`: the same source built like M1 run2's `rustred-fp-7eed68fc` (frame pointers,
  line tables, `CARGO_TARGET_DIR=TMP/w1-kernel/fp-target`), sha256 `d38cb33d3e729123c88ce0911d42c491fec46e0b97da09450e055b6543557ec7`,
  for a matched window with perf attribution. FG Ordered strict vs 4a17f9c7: 0 differing records, `containment_checks`
  169,509,549 on both sides (`TMP/w1-kernel/identity-fpd916/`). Only FG was gated on this twin (review P12); the
  fix-round fp twin passed the full gate (a), §9.3.
- Walk semantics 1, `RUSTRED-WALK-CP5` schema 5, identical to 4a17f9c7 and 102adcc3: it resumes a CP5 checkpoint
  written by either (the FG resumes of §5 go through `checkpoint_executable_changed`).

**Recipe for the owner's gate** (25-min gen-7 resume, matched window vs M1 run2): on `fable_5_1-v3-baseline`,
`tools/research/w0_baseline/m1_run.sh TMP/w1-kernel/bin/rustred-fp-36c60fb8 <OUT> <fresh gen-7 block clone>` (socket-1
lock, CPUs 128-227, W100 = 67 inspectors, 32 helpers, 1 coordinator), then `m1_analyze.py` and `m1_drift.py` on
`[T, T+1,500 s]` as for run2 (`TMP/w0/baseline/RESULTS.md` §7). Run2's reference values: 2.27M obligations/h (5-min
slices 1.76-3.08M/h), 1,179 checks per admission request, commit 8.36 µs per prepared record, coordinator duty
commit 48.1% + preparation 43.6%. This lane did not run it: the checkpoint binding hashes the worker count, so gen 7
resumes only at W100, which needs socket 1.

**Kernel attribution in its telemetry** (heartbeats and `result.json`, session counters, never persisted):
- `parallel.coordinator_duty.admission_kernel`: coordinator forward and reverse scans, candidates, word and lane
  rejections, exact tests, scan seconds, ns per candidate. Nested inside the duty object and inside
  `ordered_commit_seconds`, not a disjoint share (`heartbeat_metrics.py` keeps numeric duty entries only).
  *Correction (review P10):* in d9163195 `reverse_ns_per_candidate` divided the wall of the whole retirement call
  (traversal over every examined candidate, compaction, ledger transfers) by the commit-decided candidates only; the
  fix round reports `reverse_examined_candidates` and `reverse_retire_ns_per_examined_candidate` instead (§9.1). The
  clean BMW runs of §6 had 56-80 ns under the old field, C-5F 93.7 ns (contaminated run).
- `parallel.admission_preparation.speculative_kernel` and `speculative_{forward,reverse}_exact_tests`: helper-side
  scan wall summed over helpers, candidates, ns per candidate.

**Clean A/B on four-loop controls** [M, `TMP/w1-kernel/ab.sh`, `TMP/w1-kernel/ab-d916/`]: Ordered W6 on CPUs 72-77,
nothing else of this lane running, interleaved (round 1 ref -> new, round 2 new -> ref):

| Family | Traversal s, 4a17f9c7 (r1, r2) | Traversal s, d9163195 (r1, r2) | Ordered commit s, ref / new |
|---|---|---|---|
| FG | 17.2, 15.5 | 13.7, 15.5 | 2.37, 2.23 / 1.81, 2.05 |
| BMW | 41.5, 42.8 | 39.4, 42.2 | 10.49, 10.54 / 9.65, 9.82 |

End to end the difference (3-11%) is within the round-to-round spread; at four loops the kernel is a small part of the
work. The kernel itself scans at 7-11 ns per forward candidate on the coordinator and 6.5-13 ns per helper candidate in
these runs (warm, small working set), against 16-32 ns at gen-7 scale (§4). (The reverse side of the same runs was
9.6-10.8 ns on FG and 55.6-79.7 ns on BMW under the misattributed field above.)

**Memory** [M, gen-7 receipt `storage`]: compact queue state 19.79 GB for 74,156,033 admitted domains (266.9 B per
domain): domain images 7.12 GB, immutable per-ID summaries 6.82 GB (92 B each), SoA blocks 3.19 GB, block rows 0.38
GB, exact index 2.28 GB (ledger and closure excluded). The per-ID summaries cost about what the removed slab cost
(176 B x 37.9M live + a 4-B slot per ID ≈ 6.97 GB [E, arithmetic]), while now every admitted ID has one.
*Scope (review P3):* this `queue_storage.total_bytes` includes the candidate index; the pre-kernel binaries'
`total_bytes` (4a17f9c7, 7eed68fc/run2) excluded it and counted a live-only slab and 8-B filter words per ID. The fix
round adds `total_excluding_index_bytes` as the nearest analogue; memory gates compare process RSS (marginal fit), not
these totals.

## 7. Tests [M]

| Suite | Result |
|---|---|
| `cargo test --release -p rustred-app --lib` at da9d2499 | 782 passed, 0 failed, 7 ignored (`TMP/test-lib3.log` in the worktree) |
| same at d9163195 | 781 / 1 / 7: the key-set freeze test did not list the three new helper keys; fixed in da9d2499 (test only) |
| `--test cli_routed_campaign` | 6 passed |
| `python -m unittest discover -s examples/python -p 'test_*.py'` (16 CPUs) | 226 OK, 1 skipped |
| `cargo fmt --all -- --check` | clean |

The ignored tests include the gen-7 harness added in d5a0b968. *Correction (review P11):* the "queue suite 89 / 0 / 2
at f40d1f40" of the first report came from `TMP/test-queue1.log`, a run on a dirty tree before 933128cc was committed;
the f40d1f40-state queue run (`TMP/test-queue2.log`) has no result line. The committed-state evidence is the full lib
run at da9d2499 above (and, for the fix round, §9.6).

## 8. Open items (as of the fix round)

1. **Gate (c)**: FAIL, unbiased 2.92-3.03x forward / 3.72-4.00x reverse (§9.2). Orchestrator decision 1 (2026-09-28)
   accepts about 3x for the legacy comparator and moves the >= 4x requirement to the epoch layers (W2.3); the §4 fixes
   stay recorded, not built. The unbiased reverse ratio is closer to 4x than first reported; forward is unchanged.
2. **The W100 walk from gen 7** was first exercised by the comparator lane's runA (d9163195: restored in 508.6 s vs
   run2's 490.1 s, index_blocks 13.8 s [M, `TMP/progress/comparator.md`]); runA itself is INVALID (§9.1).
3. Stored wide summaries do not occur at gen 7, so gate (b) exercises the wide path only through synthetic wide
   queries (now swept, §9.2); unit tests cover stored wide summaries.
4. The oracle branch (`fable_5_1-v3-oracle`) is merged neither into `fable_5_1` nor into this branch. Gate (d) and
   the Ordered outputs now pass the oracle's own verifier and gate helper (§9.4, tools pinned to 63771a50); directive
   0.1 item 6 still asks the orchestrator to merge the oracle branch before accepting W1 gates (merge order oracle,
   then kernel).
5. `progress_json` on C-5F is 19% higher on the new binaries than on 4a17f9c7 (+7 s of 250 s coordinator wall),
   the same for the plain and the fp build; unattributed (§9.3).
6. Tooling: `resume_control.py` (main tree) still exits 1 on every resume gate (`uncommitted_inspections`); this
   lane's `resume.sh` now compares mechanically with `--ignore-top uncommitted_inspections` (§9.4).

## 9. Fix round after the review (2026-09-28)

A review of this lane (`TMP/progress/review-kernel.md`) raised fourteen items. I checked every one against the code
before acting. All fourteen were confirmed; none was a false positive. Two review agents then read the fix-round
diff before its commit. They found five test-side defects and one tooling rule that was too strict (a false INVALID
for exit-4 walks with status `incomplete`), all fixed before the gates below.

Commits: `9e1c2175` (production fix and its tests), `c7b5b20e` (harness), `36c60fb8` (compare tool); the note is
the commit after them. Binaries built from `36c60fb8` on a clean tree (vendor/symbolica 953e26e2 + heap-pow patch):

| Binary | sha256 | Use |
|---|---|---|
| `TMP/w1-kernel/bin/rustred-36c60fb8` | `f530fc8c128bdad156dbd2a5b4e86ca2d8e0f74e60e5a0c6fe630a45c305ff58` | plain release |
| `TMP/w1-kernel/bin/rustred-fp-36c60fb8` | `3ab745fa338d03eb81b7e72cecc3988b23dd7abc7c077f7f548ce746cd7e8fbe` | **W2 comparator** (frame pointers, as run2's `rustred-fp-7eed68fc`) |

Both supersede the d9163195 pair. Walk semantics 1, CP5 schema 5, unchanged.

| # | Item (severity) | On the code | Fix |
|---|---|---|---|
| P1 | `queue_storage` walks every block per heartbeat (major) | confirmed, and larger than the review's estimate (§9.1) | running O(1) totals |
| P2 | helper orthant shortcut vs commit predicate (minor) | confirmed (only reachable with a CP5 whose orthant is not a full orthant) | one predicate, `OwnerBucket::orthant_hit`; restore refuses such a CP5 |
| P3 | `queue_storage` keys and scope changed (minor) | confirmed | `total_excluding_index_bytes`; scope states the pre-kernel analogue; memory gates use RSS |
| P4 | a stale dead-slot ID is mapped to 0 (minor) | confirmed (unreachable for a legitimate CP5) | restore refuses any dead slot not below the domain count |
| P5 | gate (c) timed with the test-only atomic counters in the new arm (major) | confirmed | counters off before timing; re-measured (§9.2) |
| P6 | no clean C-5F timing (major) | confirmed | clean interleaved C-5F A/B, 2 rounds (§9.3) |
| P7 | directive 0.1.6 (oracle) not met for gate (d) (major) | confirmed | oracle gate on every Ready output and on the Ordered outputs (§9.4); the merge order is the orchestrator's |
| P8 | foreign-load recorder invalid (minor) | confirmed (busy < own in all four receipts) | pinned timing thread; whole-mask accounting; INVALID instead of clamping; run delay |
| P9 | sweep selection aliased with the variant layout (minor) | confirmed | sweeps chosen per kind; pair counts per kind |
| P10 | coordinator reverse telemetry mislabelled (minor) | confirmed | per examined candidate; scope stated |
| P11 | queue suite 89/0/2 misattributed (minor) | confirmed (log of a dirty tree) | §7 corrected |
| P12 | fp twin gated on FG only (minor) | confirmed | full gate (a) on the new fp twin |
| P13 | provenance and tooling gaps (minor) | confirmed | receipts stamped; `compare_walk_records.py` INVALID; hand annotation moved |
| P14 | range and single-thread-only scope of gate (c) (minor) | confirmed | old range corrected (78.9-92.5 ns); concurrent arm added |

### 9.1 P1: `queue_storage` per heartbeat (and P3, P10)

`Queue::storage_json()` summed `AggregateIndex::storage()` over the owner buckets, and `storage()` walked every group's
rows and dereferenced every boxed block (`lossy` bits). `enrich_with(lean = false)` calls it on every non-lean event.
The Ordered coordinator loop calls it twice per 250-ms heartbeat (`observe` and `set_parallel`). The **Ready**
publication path (the production policy) calls `observe` alone (`execution.rs` 1549-1553), so the walk lands in
`progress_json_seconds` there.

- [M] Cost at gen-7 shape (74.2M IDs, 1.99M boxed blocks, 2.61M rows): **85.1-94.0 ms wall per walk** over three
  receipts (`harness-fx1`, `-fx1p`, `-fx2`, key `queue_storage_telemetry_cost`), against 0.11-0.14 ms with the
  running totals.
- [M] In situ: the comparator lane's first run with `rustred-fp-d9163195` (`TMP/w1/comparator/runA`) had
  `progress_json` at 25.6-25.8% of the coordinator wall at coordinator elapsed 329/647/1,228 s, against 0.7-0.9% in
  M1 run2 at 352/713/1,514 s. The unaccounted residual was 3.7-4.5% against 2.1-2.5% (read from both runs'
  `heartbeats.jsonl`). The comparator lane marked runA INVALID.
- Fix (`9e1c2175`): the index keeps running totals: boxed blocks; reserved bytes of the group vector, of each group's
  row and pointer vectors and of the exact envelopes; lossy live slots. They are updated at every reservation, including
  one kept after a later preflight fails, and at every insertion, narrow-to-wide widening, retain, row drop and group
  removal. Restore recounts once. `storage()` is O(1); the full walk remains as the tests' reference.
  Tests: a randomized test checks the totals against the walk after every mutation (5,491 insertions, 162 refused and
  285 abandoned preparations, 103 group removals, wide envelopes, lossy slots). `assert_counts` checks it after every
  index-test insertion, and `same_state` after every serial/prepared queue comparison.
- P3: `queue_storage` adds `total_excluding_index_bytes`, and its scope string names the pre-kernel analogue. The
  final walk event must stay below 8 KiB: it is 7,634 B, and the fix-round telemetry is 116 B smaller than
  d9163195's (exact count).
- P10: the reverse timer spans the whole retirement call. `admission_kernel` now reports
  `reverse_prepared_candidates` (examined IDs a helper set decided; they reach no callback),
  `reverse_examined_candidates` (callbacks plus those) and `reverse_retire_ns_per_examined_candidate`, instead of
  `reverse_ns_per_candidate`. Test: the serial and the prepared path examine the same reverse candidates on every
  commit of the complete-proposal streams and of the prefilter-toggle streams. The comparator lane's `m1_analyze.py`
  reads the new keys (its a1c9d325).

### 9.2 Gates (b) and (c), unbiased (P5, P8, P9, P13, P14)

Harness changes (`c7b5b20e`):

- The test-only work counters are off before anything is timed.
- The single-thread timing runs on a thread pinned to one CPU (taskset on its TID, confirmed from
  `/proc/thread-self/status`). A concurrent arm runs the same interleaved rounds on 8 threads pinned to 72-79, all on
  the same arm between barriers.
- The load recorder (pinned CPU and whole mask, run delay) reports INVALID instead of clamping.
- Sweeps are chosen per request kind, with pair counts reported per kind.
- Receipts carry the test binary's sha256 and blake3, the git HEAD and the tree status.

The three receipts below ran test binary sha256 `45062f7e…` at HEAD `36c60fb8`, clean tree, CPUs 72-79 alone.

| Receipt | Timed | ST forward old / new ns | ratio | ST reverse old / new ns | ratio | MT (8 thr) fwd / rev ratio | ST load |
|---|---|---|---|---|---|---|---|
| `harness-fx1` | 20,000 | 62.3 / 20.5 | 3.03x | 50.2 / 13.5 | 3.72x | 2.99x / 3.90x | VALID, foreign 1.4% |
| `harness-fx1p` (perf) | 60,000 | 63.8 / 21.9 | 2.92x | 47.6 / 12.5 | 3.80x | 3.13x / 3.85x | VALID, foreign 1.9% |
| `harness-fx2` | 60,000 | 65.9 / 22.2 | 2.97x | 49.8 / 12.5 | 4.00x | 3.02x / 3.82x | VALID, foreign 1.2% |

Concurrency: CPU per check on 8 concurrent threads is 1.00-1.17x the single-thread value (both arms, both
directions). The 2.0-2.6x of the W0.4 replay came from whole-socket runs at 18-90 threads [D, handoff §7.7], a
different regime.

**Gate (c) stays FAIL** [M]: unbiased 2.92-3.03x forward and 3.72-4.00x reverse single-thread (3 receipts);
2.99-3.13x and 3.82-3.90x on 8 threads. Both arms got faster: pinning, and no atomics in the new arm (old forward
62-66 ns vs 79-92 ns before, new 20.5-22.2 vs 27.5-32.3). The reverse ratio moved up from 3.1-3.8x; forward did not
move. Per request (forward) that is 72-75 µs → 24.6-25.0 µs.

Profile (`harness-fx1p/perf.data`, samples of the pinned timing thread only, 84.5% of all samples): new-kernel symbols
19.2% against 64.7% for the historical paths. The diagnosis of §4 holds on unbiased data. `find_controlled` spends
32.3% + 5.5% on the instruction after the `Meta.len` load at offset 0x88 of the 144-B rows (the first touch of each
row), 21.4% after the boxed block's ID loads (offset 0x100, the binary search) and 5.2% after the block pointer load.
The fixes of §4 target these points; per orchestrator decision 1 they are recorded, not built.

Gate (b) on the same receipts: 0 mismatches and 0 lookup mismatches. Forward / reverse lookup pairs 169,178,584 /
903,179,098. Sweep pairs 1,495,498,544 in each direction, of which 1,008,057,137 exact. Every kind is swept (750 sweeps
each of admitted, empty, infinite, saturated and wide; 2,941 traced_contained, 60 traced_new). Wide, empty and saturated
queries have no exact pairs: their lanes are lossy or absent, so only necessity is checked for them. The CP5 index
section re-encodes byte for byte. Kernel images 0 mismatches over 74,156,033 IDs.

### 9.3 Gate (a) and the clean C-5F A/B (P6, P12)

`identity.sh` (v2) writes a mechanical verdict per control. PASS requires the compare tool's PASS (not INVALID),
exit 0 on both sides and equal `containment_checks`. It records the load too: busy CPUs on the mask minus own CPU,
and the native's schedstat run delay summed over its threads.

| Control | plain `rustred-36c60fb8` | fp `rustred-fp-36c60fb8` | records | `containment_checks` |
|---|---|---|---|---|
| FG | PASS (`identity-fxp`) | PASS (`identity-fxf`) | 98,909 | 169,509,549 |
| BMW | PASS | PASS | 158,951 | 653,022,941 |
| H | PASS | PASS | 24,929 | 15,228,826 |
| X | PASS | PASS | 47,193 | 20,507,017 |
| four-all W6 | PASS | PASS | 65,444 | 43,338,845 |
| C-5F W16 | PASS (`identity-fxpc`) | PASS twice (`identity-fxc1`, `identity-fxc2`) | 1,273,376 | 5,307,741,824 |

Clean C-5F A/B (P6): Ordered W16 on 72-79,328-335 with nothing else of this lane running. Two interleaved rounds:
round 1 ref → fp (`identity-fxc1`), round 2 fp → ref (`identity-fxc2`); summary `TMP/w1-kernel/ab-c5f-fx.json` [M].

| Run | traversal s | preparation | ordered_commit | dispatch | progress_json | foreign busy CPUs | run delay |
|---|---|---|---|---|---|---|---|
| ref r1 | 271.2 | 46.4 | 58.1 | 3.8 | 38.3 | 3.28 | 8.2% |
| fp r1 | 251.3 | 31.4 | 49.3 | 3.4 | 44.5 | 2.13 | 1.4% |
| fp r2 | 249.9 | 31.1 | 49.8 | 3.2 | 44.2 | 1.47 | 1.2% |
| ref r2 | 247.8 | 34.5 | 53.7 | 3.1 | 36.1 | 1.35 | 1.1% |
| plain (after) | 250.8 | 30.8 | 49.3 | 3.1 | 45.0 | 2.12 | 1.2% |

- The fp comparator is at least on par with 4a17f9c7: traversal 0.965x (mean of 2), 1.008x in the load-matched
  round 2; preparation 0.77x (0.90x in round 2); ordered_commit 0.89x (0.93x).
- The adverse d9163195 C-5F walls of §2 (traversal 821 vs 472 s, preparation 390 vs 149 s) came from the concurrent
  compile. The same control now runs in 250-271 s on either binary.
- `progress_json` is +19% (37.2 → 44.4 s, 14.4% → 17.8% of the coordinator) and equal for the plain and the fp build,
  so frame pointers are not the cause. The per-domain progress path is unchanged code (lean enrich, `CompactDomain`).
  The cause is not attributed [E: working-set or cache effect of the new layout on the event path]. Scaled to run2's
  gen-7 commit rate (~670 per-domain events per s), +5.6 µs per event would be about 0.4% of the coordinator [E].
  The comparator lane's validity rule (`progress_json` < 2%) watches it.

Four-loop clean A/B, plain vs 4a17f9c7 (`TMP/w1-kernel/ab.sh`, `ab-fx/`, Ordered W6 on 72-77, interleaved) [M]:
FG traversal ref 12.49 / 12.33 s, new 12.57 / 12.83 s; BMW ref 29.82 / 30.06 s, new 29.58 / 31.07 s. On par within
the round-to-round spread.

### 9.4 Gate (d) with the W0.2 oracle (P7)

`resume.sh` (v2), with the fp binary: FG Ordered resume, 4a17f9c7 → fp and fp → 4a17f9c7, stop at >= 40,000
committed.

- **Resume legs:** first leg exit 4, resume exit 0. A mechanical strict comparison against the Ordered FG reference
  ignores only `uncommitted_inspections`. Result: PASS both ways, 0 differing of 98,909.
- **Ready runs, ref and fp:** the W0.2 oracle gate PASS on each, `roots_independently_verified == roots_total` =
  248/248. Multiset PASS (`resume-fxr/log`, 5/5 GATE lines).
- **Oracle gate (`oracle.sh`):**
  - The verifier binary is the oracle lane's `rustred-46d4dd28` (sha256 `46d4dd28…`), built from the oracle branch
    with the legacy index, so it is independent of the kernel under test.
  - It runs `walk-verify-closure --require-closure --reinspect all` (reference levers off).
  - `assert_oracle_pass.py` checks `verdict == PASS` and roots verified == total.
  - The oracle version of `audit_owner_domain_walk.py` runs paired with the verifier report.
  - Tools are pinned from `fable_5_1-v3-oracle` 63771a50 in `TMP/w1-kernel/oracle-tools/`.
- **Ordered outputs, all PASS** [M] (`oracle-c4l-fxp`, `oracle-ordered-fp`, `oracle-c5f-fxp`):
  - plain FG 248/248, BMW 268/268, H 628/628, X 656/656, four-all 32/32;
  - fp: the same five;
  - C-5F: fp 1/1 and plain 1/1 (`oracle-c5f-fxp`).
- The earlier d9163195 FG Ready run also passes this gate: 248/248 (`oracle-smoke-d916`).
- Directive 0.1 item 6 also requires merging the oracle branch before W1 gates are accepted. That merge is the
  orchestrator's; order: oracle, then kernel (§8 item 4).

### 9.5 Full gen-7 CP5 restore under the fix-round code

The restore code changed: orthant and stale-slot validation, and one totals recount per index. So the ignored
production-restore test (`checkpoint::scale_tests::restore_copied_production_checkpoint`, `TMP/w1-kernel/scale_restore.sh`)
was re-run with the fix-round test binary at `36c60fb8` (`scale-restore-fx/receipt.json`) [M].

- **PASS:** every manifest count equal and identical to the d9163195 receipt (74,156,033 domains, 45,889,639
  committed, 27,465,422 natives, 1,192,281,291 edges, 37,907,667 live candidates). Directory unchanged.
- Restore 564.1 s (verify 107.1, decode 139.3, validate 424.9); the test took 680 s; VmHWM 48.6 GB.
- `index_blocks` (block rebuild plus the one recount) 15.9 s, against 23.1 s in the earlier receipt. The phases the
  fix does not touch also ran 1.2-1.5x faster this time (environment [E]).
- `queue_storage` at gen 7: `total_bytes` 19,789,141,628 (unchanged), `total_excluding_index_bytes`
  16,223,035,580.

### 9.6 P2, P4 and the suites

- P2: `OwnerBucket::orthant_hit` is the one shortcut predicate of the helper and the ordered commit: the stored image
  in the bucket, a full orthant, and a rank that contains the query's. Restore refuses a CP5 whose bucket orthant is
  not a full orthant (`invalid checkpoint full-orthant ID`). A later orthant of a bucket always dominates the earlier
  one's rank, so a helper skip still implies an orthant (or earlier exact) hit at commit. Tests:
  - a tampered CP5 image is refused;
  - on a queue whose orthant names a finite box (in memory only), the prepared admission finds the same older
    container as the serial lookup, with equal checks. The old helper predicate would have admitted it anew.
- P4: restore refuses a dead block slot that is not below the domain count, or is not a u32
  (`invalid checkpoint stale block slot`). Dead slots of any CP5 written by these binaries hold 0 or a former live ID.
  Test: a real retain leaves stale IDs 12..31; valid stale values re-encode byte for byte; out-of-range, u32::MAX and
  larger values are refused.
- The gen-7 CP5 restores under both checks (harness, §9.2).
- Suites at `36c60fb8` [M]:
  - lib 785 passed / 0 failed / 7 ignored (`TMP/test-lib-fix2.log` in the worktree);
  - `cli_routed_campaign` 6/6;
  - Python 228 OK / 1 skipped (16 CPUs);
  - `cargo fmt --check` clean.
- Before the fixes, the first build of the fix-round tests failed the 5 test-side defects the review agents predicted
  and one real regression, the 8-KiB final-event bound. Both were fixed before `36c60fb8`.

### 9.7 Tooling (P13)

`compare_walk_records.py` returns **INVALID** (exit 3) in three cases:

- a walk with no domain record;
- a walk that reports an `error` or an `*_error` status;
- a nonzero `exit_code` in a sibling `metrics.json`, other than the designed exit 4 of an `incomplete` or `paused`
  walk.

`--allow-empty` and `--allow-failed` waive these. Checked on real runs: the void d9163195 C-5F pair is INVALID, and a
widen-falsifier pair with status `incomplete` and exit 4 PASSes. Receipts carry binary hashes and git state. Gate
scripts are versioned: `*.v1-d916` keeps the first round's scripts.
