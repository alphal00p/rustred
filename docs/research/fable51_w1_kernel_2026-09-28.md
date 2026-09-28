# W1.1 admission kernel on the legacy lanes (v3 S1): gates and the W2 comparator, 2026-09-27/28

Plan item: `docs/research/fable51_next_push_master_plan_2026-09-27.md` §5 W1 item 1.1 (= v3 S1), branch
`fable_5_1-v3-kernel` from `fable_5_1` e74d31ce. Labels: **[M]** measured (paths given), **[E]** estimate or
interpretation. Nothing here is a closure, termination or ETA claim. No campaign was started, stopped or written.
No algebraic code was written: the change is data layout, prefiltering and telemetry around the existing exact
predicate, so the Symbolica triple-check does not apply.

**Owner decision (2026-09-27 ~21:35 UTC, `3bc4349d`):** the full plan continues only if the W2 epoch skeleton
(S2/S4) beats *legacy engine + SoA kernel* by >= 1.5x (25-min gen-7 resume, matched window vs M1 run2). The binary
of this note is that comparator (§6).

## 0. Verdict

| Gate (plan §5 W1.1) | Result |
|---|---|
| (a) strict Ordered identity, records and `containment_checks`, vs 4a17f9c7 on C-4L and C-5F | **PASS** [M] (§2) |
| (b) >= 1e8 forward and >= 1e8 reverse differential pairs from gen 7, incl. wide, empty, infinite | **PASS** [M]: 169M / 903M lookup pairs + 1.49e9 / 1.49e9 sweep pairs, 0 mismatches (§3) |
| (c) >= 4x less CPU per candidate in the 0.4 replay | **FAIL** [M]: 2.9-3.1x forward, 3.1-3.8x reverse, 4 replicates; memory-bound on the inherited block layout; fixes in §4 |
| resume across binaries (both directions, FG Ordered strict) + FG Ready audit/multiset | **PASS** [M] (§5) |
| full gen-7 CP5 restore under the kernel code (production restore path, no walk) | **PASS** [M]: all manifest counts equal, 946 s single-threaded, 47.5 GB peak (§5.1) |
| lib suite, `cli_routed_campaign`, Python suite, `cargo fmt --check` | **PASS** [M]: 782/0/7, 6/6, 226 OK, clean (§7) |

The comparator binary for the owner's 1.5x gate is `TMP/w1-kernel/bin/rustred-d9163195` (frame-pointer twin
`rustred-fp-d9163195`); §6 gives the recipe and the telemetry that attributes the kernel's share.

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
and flags any nonzero exit. The b458fde0 build (f40d1f40, before the telemetry) also passed FG (`identity-smoke-b458/`).

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
the sweeps). The three later replicate runs
(§4) repeated the whole differential with 0 mismatches each. Caveat [M]: the gen-7 store holds **0 wide summaries**
(and 0 lossy lane images), so the wide path is exercised only by the 15,000 synthetic wide query variants, not by
stored wide candidates; the unit tests of 933128cc cover stored wide summaries.

## 4. Gate (c): CPU per candidate [M]

Same harness, timing phase: one thread, old and new interleaved per chunk of 250 traced requests (order alternated,
2 repeats), thread on-CPU nanoseconds from `/proc/thread-self/schedstat` divided by logical candidates in
`containment_checks` units (reverse: examined candidates). Foreign busy time on the timing CPU was 0 in every run.
This is the W0.4 unit (CPU per tested candidate at 74M, same checkpoint, same trace) measured on the production kernel,
not on the W0.4 prototype layouts.

| Run | Timed requests | Forward ns/check old / new | Ratio | Reverse ns/check old / new | Ratio |
|---|---|---|---|---|---|
| `harness-da9d` | 20,000 | 88.4 / 28.7 | 3.08x | 68.8 / 22.1 | 3.11x |
| `harness-da9d-perf` | 60,000 | 89.8 / 30.9 | 2.90x | 69.6 / 18.3 | 3.79x |
| `harness-da9d-perf2` | 60,000 | 92.5 / 32.3 | 2.86x | 66.5 / 18.7 | 3.55x |
| `harness-da9d-perf3` (perf attached) | 60,000 | 78.9 / 27.5 | 2.88x | 57.1 / 16.0 | 3.57x |

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
  **PASS** (98,886 records each).

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

**Binaries** [M]:
- `TMP/w1-kernel/bin/rustred-d9163195`: plain release, sha256 `86a7d345…0d98`, the binary gated in §2 and §5.
- `TMP/w1-kernel/bin/rustred-fp-d9163195`: the same source built like M1 run2's `rustred-fp-7eed68fc` (frame pointers,
  line tables, `CARGO_TARGET_DIR=TMP/w1-kernel/fp-target`), sha256 `d38cb33d3e729123c88ce0911d42c491fec46e0b97da09450e055b6543557ec7`,
  for a matched window with perf attribution. FG Ordered strict vs 4a17f9c7: 0 differing records, `containment_checks`
  169,509,549 on both sides (`TMP/w1-kernel/identity-fpd916/`).
- Walk semantics 1, `RUSTRED-WALK-CP5` schema 5, identical to 4a17f9c7 and 102adcc3: it resumes a CP5 checkpoint
  written by either (the FG resumes of §5 go through `checkpoint_executable_changed`).

**Recipe for the owner's gate** (25-min gen-7 resume, matched window vs M1 run2): on `fable_5_1-v3-baseline`,
`tools/research/w0_baseline/m1_run.sh TMP/w1-kernel/bin/rustred-fp-d9163195 <OUT> <fresh gen-7 block clone>` (socket-1
lock, CPUs 128-227, W100 = 67 inspectors, 32 helpers, 1 coordinator), then `m1_analyze.py` and `m1_drift.py` on
`[T, T+1,500 s]` as for run2 (`TMP/w0/baseline/RESULTS.md` §7). Run2's reference values: 2.27M obligations/h (5-min
slices 1.76-3.08M/h), 1,179 checks per admission request, commit 8.36 µs per prepared record, coordinator duty
commit 48.1% + preparation 43.6%. This lane did not run it: the checkpoint binding hashes the worker count, so gen 7
resumes only at W100, which needs socket 1.

**Kernel attribution in its telemetry** (heartbeats and `result.json`, session counters, never persisted):
- `parallel.coordinator_duty.admission_kernel`: coordinator forward and reverse scans, candidates, word and lane
  rejections, exact tests, scan seconds, ns per candidate. Nested inside the duty object and inside
  `ordered_commit_seconds`, not a disjoint share (`heartbeat_metrics.py` keeps numeric duty entries only).
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
these runs (warm, small working set), against 16-32 ns at gen-7 scale (§4).

**Memory** [M, gen-7 receipt `storage`]: compact queue state 19.79 GB for 74,156,033 admitted domains (266.9 B per
domain): domain images 7.12 GB, immutable per-ID summaries 6.82 GB (92 B each), SoA blocks 3.19 GB, block rows 0.38
GB, exact index 2.28 GB (ledger and closure excluded). The per-ID summaries cost about what the removed slab cost
(176 B x 37.9M live + a 4-B slot per ID ≈ 6.97 GB [E, arithmetic]), while now every admitted ID has one.

## 7. Tests [M]

| Suite | Result |
|---|---|
| `cargo test --release -p rustred-app --lib` at da9d2499 | 782 passed, 0 failed, 7 ignored (`TMP/test-lib3.log` in the worktree) |
| same at d9163195 | 781 / 1 / 7: the key-set freeze test did not list the three new helper keys; fixed in da9d2499 (test only) |
| `--test cli_routed_campaign` | 6 passed |
| `python -m unittest discover -s examples/python -p 'test_*.py'` (16 CPUs) | 226 OK, 1 skipped |
| `cargo fmt --all -- --check` | clean |

The ignored tests include the gen-7 harness added in d5a0b968. The queue suite (`queue::`) was 89 / 0 / 2 at f40d1f40.

## 8. Open items

1. **Gate (c)**: FAIL at 2.9-3.1x forward / 3.1-3.8x reverse; fixes and estimates in §4. Needs a decision: implementing
   them changes the comparator binary and requires gates (a)-(c) again (about 2 h).
2. **The W100 walk start from gen 7 is untested** (socket 1). The full restore is tested (§5.1); what remains is the
   walk itself after `checkpoint_restored`. The first comparator run covers it; compare its restore time with run2's
   490 s (the kernel adds about 20-25 s of index rebuild [E], §5.1).
3. Stored wide summaries do not occur at gen 7, so gate (b) exercises the wide path only through synthetic wide
   queries (§3).
4. Timing from the gate (a) runs is void (shared CPUs with a compile); the only timing evidence is §4 and the A/B in §6.
5. Tooling: `resume_control.py` exits 1 on every resume gate on this host (`uncommitted_inspections` is a top-level
   difference by design). Teaching `compare_walk_records.py` to ignore that field for resume comparisons would make
   the exit code usable.
