# C3 CSR dependency edges: implementation record and control measurements (2026-09-26)

Scope:

- Design section 3 of
  [`fable51_design_checkpoint_memory_2026-09-26.md`](fable51_design_checkpoint_memory_2026-09-26.md):
  u32 CSR-by-target edges plus a bounded append log folded after saves.
- Review finding #11 of
  [`fable51_review_checkpoint_branch_2026-09-26.md`](fable51_review_checkpoint_branch_2026-09-26.md):
  the pre-save closure refresh is timed and cancellable. The wave-2 fix
  round withdrew the cancellability (see "Wave-2 fix round").

Branch `fable_5_1-c2-csr` ("c2" in the branch names means package C,
wave 2; this is item C3). Commits `8e4503d3`, `f831f8b4`, `06e9fc0e`,
`45ed5f7b` and `faf696d0` sit on top of `fable_5_1` at `754200a3`. The
review fix round added `d4bb11dd`, `114d0303`, `a73fe146` and `1fb162cb`
(section "Fix round" below). `W/` =
`crates/rustred-app/src/application/routed_campaign/walking/`. The text
below describes the branch head. Where the first version differed, the fix
round says so. Control directories are under `TMP/fable51-controls/`;
worktree logs and the benchmark receipts are under
`.claude/worktrees/fable51-csr/TMP/`.

## What changed

- **Tracker layout.** The dependency tracker (`W/descendant_closure.rs`)
  keeps its node flags as bytes (sealed / inspected / closed). Its edges
  live in the new `W/descendant_closure/edges.rs`, in two parts:
  - Folded edges are a CSR by target: u32 sources and u64 offsets.
  - Edges since the last fold are an append log of (u32, u32) pairs,
    chained per target (u32 heads and next).
  - The former layout was a linked list of 24 B edges with usize links.
- **Refresh.** A refresh walks each popped node's CSR slice and then its
  log chain. Its scratch is a u64 blocked bitset plus a u32 stack, about
  4.1 B per node (was 9 B). `edge()` still deduplicates through
  `open_targets` for unsealed sources only, as before.
- **Fold.** The fold merges the log into the CSR by counting sort by
  target. A stable partition of the new pairs by target bucket (2^15
  targets per bucket, `45ed5f7b`) comes first, so both passes stay cache
  local.
  - Insertion order within a target is unchanged, so the CSR is identical
    to an unpartitioned sort.
  - Extra transient memory: one partitioned copy of the folded pairs (8 B
    per folded edge), next to the old CSR and the log.
- **Save.** Only the log keeps global insertion order, which the
  checkpoint writer needs.
  - `Store::save_cancellable` writes `edges-<S>.bin` from
    `Tracker::edge_segment`: the unpersisted log tail, or a full re-tile
    when the retained tiling ends inside the folded prefix.
  - After the manifest is published, `Tracker::persisted` folds the whole
    log once it exceeds 1/16 of the CSR or 64M edges (`FOLD_LOG_FRACTION`,
    `FOLD_LOG_EDGES`). The fold is amortized O(1) per edge and does not
    bump the revision.
  - Since `d4bb11dd` the fold runs only for `SaveKind::{Periodic, Forced}`
    saves made while the run's cancellation is not set. The final save,
    which serves both the pause and the completion, and every cancelled
    save persist the log without folding it.
- **Restore.** Restore reads the pairs as (u32, u32) and reserves them
  once for the manifest's validated total (`a73fe146`). It rebuilds the
  CSR in `Tracker::from_parts` and validates as before, minus the
  linked-list head check, which segment digests plus endpoint range checks
  replace. It still refuses:
  - duplicate edges from unsealed sources;
  - closed-but-unsealed flags;
  - closed frontiers;
  - counter mismatches.

  The `checkpoint_restored` event gains `closure_seconds`.
- **Review #11 (`f831f8b4`).**
  - `Store::save` now takes its start time before the forced pre-save
    refresh, so the scan counts in `save_seconds` and in the adaptive
    interval.
  - The three save sites in `W/mod.rs` pass the run's cancellation.
  - `Tracker::refresh_before_save` keeps the previous snapshot on
    cancellation or when scratch allocation fails, instead of disabling
    the monitor inside the save. A stale snapshot is valid: closed nodes
    stay closed, and counters match the flags.
  - Superseded in part by the wave-2 fix round (`67c8125d`): the
    pre-save scan is again never cut by cancellation, as in 102adcc3.
    The timing change and the scratch-shortage behaviour stay.
- **Late stop requests (`114d0303`).** `State::report_cancellation` returns
  a never-set flag when the walk exhausted its worklist without pausing or
  failing, and the run's cancellation otherwise. The final save and the
  report refresh use it. A stop request that races the last publication
  therefore no longer leaves `descendant_closed`, `total_closed` or
  `initial_closed` stale in a `locally_resolved` `result.json`.
- **Full log (`1fb162cb`).** A full u32 log (2^32 - 1 edges) is folded
  instead of refusing the edge and disabling the monitor. Only runs
  without a checkpoint store could reach that point.
- **Iteration API.** `dependencies()` keeps its callers, but its order is
  now unspecified (folded edges by target, then the log). `for_each_edge`
  is the internal-iteration form. After the merge with C1, both became
  `#[cfg(test)]` (see "Merge into fable_5_1-wave2").
- **Benchmark.** `csr_edges_scale_benchmark` is an ignored, env-driven
  scale test (`W/descendant_closure/edges_benchmark.rs`) that compares the
  CSR with a `cfg(test)` copy of the former linked list on the same graph.
- **Report.** The JSON report keeps its key set.
  `retained_storage_estimate_bytes` and `refresh_scratch_estimate_bytes`
  now describe the new layout. These are logical-capacity estimates and
  are not persisted.
- **Tests.**
  - `checkpoint_rejects_bad_endpoints_duplicates_closed_frontier_and_counts`
    (replaces the linked-list corruption test).
  - `csr_refresh_matches_linked_list_reference_after_folds` (300 random
    graphs, random folds and restore round trips against
    `reference_closed`).
  - `edge_segments_are_disjoint_prefix_partition_of_the_log`.
  - `binary_closure_sections_reject_endpoint_duplicate_closed_unsealed_and_counts`.
  - `fold_only_after_the_whole_log_was_persisted`.
  - `save_path_refresh_never_disables_and_honours_cancellation`.
  - `cancelled_pre_save_refresh_persists_a_stale_but_valid_snapshot`.
  - `csr_keeps_every_edge_and_its_order_across_buckets`.
  - Fix round: `final_and_cancelled_saves_persist_the_log_without_folding_it`,
    `exhausted_walk_reports_a_current_closure_despite_a_late_stop_request`
    and `a_full_log_folds_instead_of_refusing_an_edge`.

Compatibility:

- **On-disk format.** Unchanged: `edges-<S>.bin` holds (u32, u32) LE pairs
  in insertion order, `nodes-<G>.bin` holds u8 flags, the closure counters
  sit in `meta-<G>.json`, and the manifest is schema 5.
  - Measured: `cmp` of the nine binary sections of `c3-ref102-fix/<F>` and
    `c3-new-fix/<F>` finds them byte-identical for FG, BMW, H and X, with
    the same single edge segment (FG: 540,397 edges, blake3 identical).
  - The meta sections differ only in timing values.
- `WALK_SEMANTICS_VERSION` stays 1. A `rustred-102adcc3` checkpoint resumes
  under this code in both policies (measured below).
- **`result.json`.** Records and all counters are identical, including
  `descendant_closure.{dependency_edges, graph_revision, total_closed,
  initial_closed}`. Besides timing, only the two layout estimates differ.
- **Behaviour changes outside the admission path.** None of these is
  exercised by a control:
  1. A stop request racing the last publication. In checkpointed runs,
     the first version could leave the closure stale; `114d0303` restored
     102adcc3's behaviour. In non-checkpointed runs, which already had
     this race on 102adcc3, the report is now also current, so it equals
     the uncancelled run instead of a timing-dependent stale value.
  2. A cancelled periodic or initial save persists a stale-but-valid
     snapshot (`snapshot_revision < revision`). Restore accepts that, both
     here and, per review #11, in the fable_5_1 restore code.
     - Correction (wave-2 fix round): this item missed the common case.
       The walk's final save after a stop request also ran with the
       run's (set) flag, so every paused generation persisted the last
       throttled snapshot, and every resume control's paused receipt
       shows it (see "Wave-2 fix round"). `67c8125d` removed the whole
       item: no save's pre-save scan is cut any more.
  3. A scratch allocation failure in the save path keeps the monitor on
     (102adcc3 disabled it for good).
  4. `save_seconds` now includes the pre-save scan, so the adaptive
     interval `max(interval, 20 x last save)` can lengthen. Save timing is
     session-dependent anyway.
- **Rollback.** A checkpoint written by this code resumed under 102adcc3
  was not run with the branch binaries. It was run with the merged wave-2
  binary, which contains this branch (see "Merge into fable_5_1-wave2").

## Gates, first version (binary `rustred-0b69fd1a`, sha256 `0b69fd1a7f7362b7de98060732beeff9f1750e5bf132d89c29ed88d526511488`)

Setup:

- Commit `faf696d0`.
- Reference binary: `rustred-102adcc3` (sha256
  `102adcc345ff3010496861f6057789632718cb86dbb1c7ac52d5107bc4c1f2ae`,
  `fable_5_1` at `343a86a7`, the running campaign's binary).
- Controls and the benchmark ran on CPUs 218-223, W6, `nice -n 5`,
  sequentially; builds and tests on 212-223.
- The host was shared, so wall times are informational.

Results:

- **Suite.** 735 passed / 0 failed / 5 ignored, run twice (branch point
  728/0/4; the fifth ignored test is the benchmark).
  - The implementer reports confirming `SYMBOLICA_LICENSE`. No suite log
    of this round was kept in the worktree.
  - `cargo fmt` is clean.
  - `cargo check --release -p rustred-app --lib --tests` passes on the
    intermediate commits `8e4503d3`, `f831f8b4` and `06e9fc0e`
    (`c3-check.log`).
- **Four-loop Ordered.** `c3-ref102/<F>` vs `c3-new-r2/<F>`, strict: 0
  differing records for FG (98,909), BMW (158,951), H (24,929) and X
  (47,193).
  - The only top-level difference is `descendant_closure`: session
    telemetry (BMW `refresh_count` 5 vs 7, `*_seconds`) plus the two
    layout estimates.
  - `dependency_edges` (FG 540,397; BMW 1,915,548; H 110,275; X 260,571),
    `graph_revision`, `total_closed`, `initial_closed` and all counters
    are equal.
  - Audit PASS on all four.
- **FG Ready.** `c3-new-ready-r2/fg` vs `c3-ref102-ready/fg`: multiset PASS
  (98,846 native each, 0 / 0 shapes only on either side), audit PASS.
- **Resume.**

| Label | First -> second | Policy | Committed at stop | first_exit | resume_exit | Compare | Restore |
|---|---|---|---:|---:|---:|---|---|
| `c3-resume-ord-r2` | 102adcc3 -> 0b69fd1a | Ordered | 61,065 | 4 | 0 | strict: 0 differing records | 1.03 s, 345,953 edges |
| `c3-resume-ready-r2` | 102adcc3 -> 0b69fd1a | Ready | 66,316 | 4 | 0 | multiset PASS | 1.24 s, 373,273 edges |
| `c3-resume-self-r2` | 0b69fd1a -> 0b69fd1a | Ordered | 61,357 | 4 | 0 | strict: 0 differing records | 1.00 s |

  The strict resume comparisons differ at top level only in:
  - `uncommitted_inspections`;
  - `descendant_closure` telemetry: the key set of the 102adcc3 -> 102adcc3
    self-test `harness-selftest-102`, plus `refresh_scratch_estimate_bytes`
    (a layout estimate).
- **A defect the controls caught.** A first draft of the bucket partition
  had its bucket cursors off by one bucket.
  - The unit tests never left bucket 0. A fresh four-loop run folds only
    at its final save, after the walk. So the fresh controls `c3-new/<F>`
    finished with the reference counters (no strict comparison was kept
    for them).
  - The resume controls then failed: `c3-resume-ord/fg` and
    `c3-resume-ready/fg` (102adcc3 -> `rustred-44c4dae7`) refused the
    rebuilt graph with "dependency alias or partial-anchor edge missing"
    (resume_exit 4).
  - The implementer's assessment is that in a running process the same
    bug would have corrupted the folded graph silently.
  - The fix and the multi-bucket unit test
    `csr_keeps_every_edge_and_its_order_across_buckets` are in `45ed5f7b`.
    The draft never reached a commit.
  - `TMP/fable51-controls/bin/rustred-44c4dae7` (sha256
    `44c4dae7992f89926171a4ff947c55cdab5192220edcce647bebcc5f3e4ca297`)
    is still in the bin directory and must not be used. Its runs `c3-new`,
    `c3-new-ready` and `c3-resume-{ord,ready,self}` (no suffix) are
    superseded.
- **Design 3.4 measurement gate** (synthetic; see the scale benchmark
  under "Measurements"). All three targets are met on the synthetic graph:
  - 4.60 edge bytes/edge after the fold (target <= 5);
  - a refresh ratio of 0.035 against the former linked list (target
    <= 1/3);
  - a restore rebuild of 8.98 s (target <= 10 s).

## Fix round (review of the first version)

The review ran three reviewers on the branch at `faf696d0` (restore,
semantics, tests/performance). A triage step merged their output into five
findings, and two adversarial verifiers checked each finding (workflow
journal `.../subagents/workflows/wf_6afddf23-7b2/journal.jsonl` of session
`7dfabea8`). Four findings reached the fix round, and all four are
applied. Commits on `fable_5_1-c2-csr` after `faf696d0`:

1. `d4bb11dd` (the pause and completion saves fold the log; minor,
   confirmed by both verifiers; three reviewer findings shared this root
   cause).
   - Every successful publish folded the log once it outgrew 1/16 of the
     CSR, including the final forced save.
   - That fold allocates new sources (4 B/edge), new offsets (8 B/node)
     and a partitioned copy of the log (8 B per log edge) next to the old
     CSR and the log. It never checks cancellation, and it buys nothing:
     after a pause the process exits, and resume rebuilds a folded CSR
     from the segments anyway.
   - At a RAM-guard stop this was an uncancellable spike just below the
     hard limit (est. 6-12 s of coordinator time at campaign scale). A
     SIGKILL at that point would also lose the paused receipt, although
     the checkpoint was already durable.
   - `SaveKind { Periodic, Forced, Final }` replaces the force flag, and
     only saves after which the walk continues fold.
2. `114d0303` (the completion save honours cancellation, so the closure
   can be stale; minor, confirmed by both).
   - Review #11 passed the run's cancellation to the final save as well.
     Before that, the final save's pre-save scan could not be cut and
     guaranteed a current snapshot before the records were annotated.
   - After review #11, a stop request arriving after the last loop-top
     check cut both that scan and the report refresh. A `locally_resolved`
     result could then carry stale `descendant_closed` values (flagged
     `snapshot_stale`, conservative, but different from the uncancelled
     run).
   - A periodic save that captured the final stamp under cancellation
     also made the final save skip as unchanged, so fixing the save alone
     would not have been enough. `State::report_cancellation` covers both
     the save and the report refresh.
3. `a73fe146` (the restore reserve can grow to about twice the pairs; one
   verifier confirmed, one refuted).
   - The restore buffer grew by amortized `try_reserve` per segment while
     `Edges::from_pairs` built the partition, sources and offsets next to
     it.
   - The manifest's segments tile `[0, total)` (`validate_structure`,
     before restore), so one exact reserve is safe.
   - The effect is virtual size only (no RSS effect measured). It matters
     under an address-space cap and for the design 7.1 restore
     measurement.
4. `1fb162cb` (the u32 log caps unfolded edges; one verifier confirmed,
   one refuted).
   - Only saves fold, so a walk without a checkpoint store kept every edge
     in the log. After about 4.29e9 edges it would have disabled the
     monitor, where the former usize list kept going.
   - This is not reachable at current scale. The five-loop W12 control
     `b2-ready-gate` has 12.7M edges, and the live campaign reported
     787,343,714 at its last heartbeat (below).
   - It was the one place where the new layout could have changed
     `result.json` against the old binary.

Refuted by both verifiers and not applied in code:

- **The benchmark's ratio and bytes/edge are best-case synthetic figures.**
  - The benchmark does what design 3.4 specifies, and its receipt says
    "synthetic closure graph only; measurements, not a campaign claim".
  - The factual content is adopted in this note: the figures below are
    labelled synthetic, and the steady-state bytes/edge between saves is
    listed as an estimate.

Skipped parts of applied findings:

- **Finding 3's extension to the domain and record restore buffers:**
  those paths belong to C2 and C1.
- **An end-to-end walk test of the finding-2 race:** there is no
  deterministic hook between the last commit and the loop exit. The unit
  test drives the same `State` predicate, the Final save and the report
  refresh directly.
- **A benchmark rerun:** the fold, CSR build and refresh code are
  unchanged; only when a fold runs changed.

### Fix-round gates (binary `rustred-9f92cdb9`, sha256 `9f92cdb95502465b77808b168acd3e02464df868443720a00e1d3bd885f0eb61`)

Commit `1fb162cb`. Reference: 102adcc3 as above. CPUs 218-223.

- **Suite.** 738 passed / 0 failed / 5 ignored at `1fb162cb`
  (`c3fix-suite-3.log`; `c3fix-suite-1.log` on the pre-rustfmt tree was
  also 738/0/5).
  - `c3fix-suite-2.log` (737/1/5) failed only the pre-existing flake
    `cli::shards::supervisor::tests::orphan_child_retains_campaign_lock_until_exit`
    ("campaign already has a supervisor"). It passes alone and on rerun.
  - The implementer reports confirming `SYMBOLICA_LICENSE`; the logs do
    not record that check.
  - `cargo fmt` is clean.
  - `114d0303` and `a73fe146` pass `cargo check --release --lib --tests`
    (`c3fix-check.log`).
- **Four-loop Ordered.** `c3-ref102-fix/<F>` vs `c3-new-fix/<F>`, strict
  (`c3-new-fix/<F>/compare-strict-vs-ref102-fix.json`): 0 differing records
  for FG, BMW, H and X.
  - The verdict prints FAIL only because of `descendant_closure`. Checked
    for this note, the keys that differ there are `last_refresh_seconds`,
    `refresh_seconds` and `snapshot_age_seconds` (timing), plus
    `refresh_scratch_estimate_bytes` and `retained_storage_estimate_bytes`
    (layout estimates).
  - `refresh_count`, `dependency_edges`, `graph_revision`, `total_closed`
    and `initial_closed` are equal.
  - The nine binary sections are byte-identical per family.
  - Audit PASS on all four (`audit-c3-new-fix-<F>.log`).
- **FG Ready.** `c3-new-fix-ready/fg` vs `c3-ref102-fix-ready/fg`: multiset
  PASS (98,846 native, 40 aliases, 160 partial), audit PASS on both.
- **Resume.**

| Label | First -> second | Policy | Committed at stop | first_exit | resume_exit | Compare | Restore (edges / closure s / total s) |
|---|---|---|---:|---:|---:|---|---|
| `c3-resume-ord-fix` | 102adcc3 -> 9f92cdb9 | Ordered | 62,157 | 4 | 0 | strict: 0 differing records | 350,565 / 0.0060 / 1.00 |
| `c3-resume-ready-fix` | 102adcc3 -> 9f92cdb9 | Ready | 65,492 | 4 | 0 | multiset PASS | 368,570 / 0.0067 / 1.19 |
| `c3-resume-self-fix` | 9f92cdb9 -> 9f92cdb9 | Ordered | 60,298 | 4 | 0 | strict: 0 differing records | 340,317 / 0.0057 / 0.98 |

  - In `c3-resume-self-fix` the paused checkpoint was written by the new
    Final save, which persists the log without folding it. Restore rebuilt
    the CSR from those segments.
  - The harness exits 1 for the strict resumes because of the telemetry
    and estimate keys listed above, as for the 102 -> 102 self-test.

## Measurements

Scale benchmark (design 3.4 gate, synthetic):

- **Setup.** `csr_edges_scale_benchmark` ran on CPUs 218-223 on the shared
  host. Test binary sha256
  `474cda2d20498ea3c311ee5b990863b35c472074fdb92392c0d9fb4cc42baa85`
  (`c3-bench/test-binary-sha256-run3.txt`), built from the tree committed
  as `faf696d0`. The run log notes the worktree was dirty at run time, so
  the binary-to-commit mapping rests on that commit's message. Receipt:
  `.claude/worktrees/fable51-csr/TMP/c3-bench/receipt-20M-400M-final.json`.
- **Graph.**
  - 20,000,000 nodes and 399,982,633 edges.
  - A committed prefix of 7,000,000 sources (35%) owns all edges and is
    sealed after its edges are accepted; the pending rest is unsealed.
  - Targets are drawn uniformly at random over all nodes.
  - Every tenth committed node depends only on earlier members of its
    class and so stays closed: 700,000 closed.
- **Assertions.** Closure flags are equal between the two layouts. An
  order-independent edge-multiset fingerprint matches after the fold and
  after the restore rebuild.

| Quantity | CSR (this branch) | Former linked list | Target |
|---|---:|---:|---:|
| Edge storage after fold | 1,839,930,540 B = 4.60 B/edge | 9,919,583,192 B = 24.8 B/edge | <= 5 B/edge |
| Whole tracker after fold | 1,859,930,708 B = 4.65 B/edge | - | - |
| Log before fold (amortized capacities) | 6,542,451,112 B = 16.4 B/edge | - | - |
| Forced refresh | 2.92 s (CSR); 58.4 s on the unfolded log | 82.8 s | ratio <= 1/3 (measured 0.035) |
| Fold | 6.07 s | - | - |
| Restore rebuild | 8.98 s = CSR 6.75 s + validation 2.23 s | - | <= 10 s |
| Build through `edge()` | 85.5 s | - | - |

- Peak RSS for the whole benchmark was 13,305,171,968 B, which includes
  the former layout kept for comparison (9.9 GB). RSS after the fold was
  5,068,853,248 B.
- Intermediate receipts in the same directory show:
  - the plain counting sort (`receipt-20M-400M-pre.json`, test binary
    digest not recorded): fold 12.27 s, restore rebuild 15.6 s, above the
    10 s target;
  - the first bucketed run (`receipt-20M-400M-bucketed.json`, before the
    multiset assertion): fold 5.87 s, restore rebuild 8.65 s.
- The restore rebuild figure builds the CSR from an in-memory pair vector
  of exact capacity. It does not include reading and digesting the
  segments from disk.
- Per edge, the refresh cost 207 ns on the former list and 7.3 ns on the
  CSR in this graph. Uniform random targets are the worst case for the
  former list (see the production figure below).

Four-loop controls (peak RSS from `run_control.py`, W6, single runs,
102adcc3 -> 9f92cdb9):

| Family | Peak RSS | `retained_storage_estimate_bytes`: 102adcc3 / 0b69fd1a (final fold) / 9f92cdb9 (no final fold) | `refresh_scratch_estimate_bytes` |
|---|---:|---:|---:|
| FG | 1,183,522,816 -> 1,173,299,200 | 27,197,608 / 3,587,916 / 13,217,960 | 890,181 -> 408,004 |
| BMW | 1,876,480,000 -> 1,852,542,976 | 54,722,728 / 10,306,136 / 26,538,152 | 1,430,559 -> 655,676 |
| H | 696,950,784 -> 671,281,152 | 3,788,968 / 841,668 / 1,773,992 | 224,361 -> 102,836 |
| X | 1,369,477,120 -> 1,362,980,864 | 7,635,112 / 1,839,844 / 3,565,736 | 424,737 -> 194,676 |
| FG Ready | 1,172,082,688 -> 1,165,254,656 | - | - |

- At 0.1-1.9M edges the closure is a small part of four-loop RSS. The
  peaks move by -0.5% to -3.7%, within what these single runs can
  resolve.
- The retained estimate at the end of a `9f92cdb9` run counts the unfolded
  log. In these fresh runs the log is never folded (the initial save has
  no edges yet, and the periodic interval of 3,600 s is never reached), so
  the whole walk runs on the log.
- Wall time (informational, whole command / traversal s, 102adcc3 ->
  9f92cdb9): FG 16.5 / 12.86 -> 17.0 / 13.00, BMW 35.0 / 30.12 -> 36.0 /
  30.73, H 16.5 / 12.36 -> 16.5 / 12.24, X 40.5 / 34.00 -> 40.0 / 33.72,
  FG Ready 16.0 / 12.15 -> 16.0 / 12.20.

Production reference (102adcc3, measured, read-only): the last heartbeat
of the live v2 campaign
(`campaigns/five-loop-qcd-feynman-d9d10-v2/runs/20260926T151353.794886Z/events.jsonl`,
run elapsed 32,878 s, file time 2026-09-27 00:22 UTC) shows:

- 787,343,714 dependency edges over 48,582,681 discovered domains;
- `last_refresh_seconds` 57.3 (72.8 ns per edge), `refresh_count` 31;
- `retained_storage_estimate_bytes` 26,893,929,488 (34.2 B per edge,
  node arrays included).

## Estimates (not measured)

- **Closure storage at the campaign's current size**, from the layout:
  - After a fold: sources 4 B x 787M = 3.15 GB, plus offsets 8 B x 48.6M
    = 0.39 GB, heads 0.19 GB and flags 0.05 GB, about 3.8 GB in total.
  - The log adds up to 1/16 of the CSR between saves: about 49M edges, or
    0.59 GB at 12 B/edge logical and 0.81 GB at the benchmark's 16.4
    B/edge amortized.
  - Steady state is therefore about 4.4-4.6 GB (about 5.6-5.9 B/edge),
    against the 26.9 GB estimate 102adcc3 reports now. These are
    logical-capacity estimates, not RSS.
- **Transient memory.**
  - A fold allocates new sources and offsets plus the partitioned log
    copy next to the old CSR and the log.
  - A restore holds about 20 B per edge at its peak (pairs 8 +
    partition 8 + sources 4), about 16 GB at 787M edges, on top of the
    rest of the restored state.
- **Refresh at campaign scale.** No figure is measured for the new layout.
  If the CSR's synthetic 7.3 ns/edge carried over, a refresh at 787M edges
  would take about 5.7 s, against the measured 57.3 s. The production
  graph has more locality than the uniform synthetic graph: the former
  list ran at 72.8 ns/edge there against 207 ns/edge in the benchmark. So
  the synthetic ratio of 0.035 is not a production expectation.

## Merge into fable_5_1-wave2

- **Merge `60ba796e`.** Conflicts and their resolutions:
  - `Store::save_cancellable`: C1's sidecar-aware records plan is kept
    verbatim, and C3's `edges_total` is added before it.
  - `validate_ledger_closure`: it now scans edges through a new
    `Tracker::try_for_each_edge` over the CSR slices and the log, with an
    early exit. The check is order-independent.
  - `execution/closure_tests.rs`: the union of both sides' tests.
- **Follow-up `2a965c4b`.** `Tracker::dependencies` and
  `Tracker::for_each_edge` became `#[cfg(test)]`, because the merged
  release build had no caller left. No behaviour change.
- **Merged binary.** `rustred-53e672fc` (sha256
  `53e672fc3c082b2bbe696bd37c5badff346c5191b3cb0b010d80fa70b9534d90`)
  resumed a 102adcc3 checkpoint:
  - `wave2-resume-ord/fg`: strict, 0 differing records.
  - `wave2-resume-ready/fg`: multiset PASS.

  A checkpoint it wrote also resumed under 102adcc3:
  - `wave2-resume-rollback-ord/fg`: first_exit 4, resume_exit 0, 0
    differing records.
  - `wave2-resume-rollback-ready/fg`: multiset PASS.

  The other integration gates are recorded separately (`wave2-*`).

## Wave-2 fix round

Review of the merged `fable_5_1-wave2` head, finding "paused checkpoints
persist a stale closure snapshot, while 102adcc3 always refreshed before
saving" (confirmed by both verifiers).

- **Cause.** Review #11 let the run's cancellation cut the pre-save scan
  (`Tracker::scan` checks the flag at node 0). The walk's final save
  passed `State::report_cancellation`, which is the run's (set) flag
  whenever `checkpoint_paused`. Every paused generation written after a
  stop request therefore persisted the last throttled in-loop snapshot:
  the `nodes-<G>.bin` CLOSED bits and the meta closure `total_closed`,
  `initial_closed` and `snapshot_revision` differed from what 102adcc3
  writes for the same state. The exhausted-run gates could not see it,
  because an exhausted walk gets the never-set flag.
- **Seen in the merged binary's controls** (`run1/result.json`, the
  paused receipt, which reports what the paused save persisted):
  - `rustred-53e672fc` (sha256
    `53e672fc3c082b2bbe696bd37c5badff346c5191b3cb0b010d80fa70b9534d90`),
    `wave2-resume-rollback-ord/fg`: graph_revision 478,996,
    snapshot_revision 113,121, `snapshot_stale` true, total_closed 2,056.
    `wave2-resume-self/fg` and `wave2-resume-rollback-ready/fg` are
    stale in the same way.
  - `rustred-102adcc3`, `wave2-resume-ord/fg`: 475,142 = 475,142,
    `snapshot_stale` false, total_closed 39,730.
  - The stop points differ, so these are not same-state pairs.
- **Fix (`67c8125d`).** `Tracker::refresh_before_save` takes no flag and
  always scans with a never-set one, for every save kind, as 102adcc3
  did. Periodic and Forced are included: on an exhausted walk, a
  periodic save cut after a stop request would capture the final stamp,
  and the final save would then skip as unchanged. The run's
  cancellation still suppresses the post-save fold (Final never folds).
  A scratch shortage still keeps the previous, stale-but-valid snapshot
  and the monitor on (item 3 above). Compatibility item 2 is withdrawn.
  - Cost: one uncancellable scan per stop, as in 102adcc3. In the paused
    flow this is no slower than before the fix: the stop's final save
    scans once, and a periodic scan that completed leaves it nothing to
    do. The measured 102adcc3 production scan is 57.3 s at 787M edges
    (above). The CSR scan at that scale is not measured.
- **Diagnostic pause (`1d98b623`).** The same finding's twin, on
  `diagnostic_checkpoint`, is resolved by this fix. Its save now persists
  the triggering state's closure even with a pending stop request. The
  comment says so; the gate note corrects the `f919e9be` merge note.
- **Tests.**
  - `saves_under_a_stop_request_persist_a_current_closure_snapshot`:
    Forced and paused Final saves with the flag set persist current
    closed counts. Restore still accepts a stale-but-valid snapshot,
    rewritten into the meta section.
  - `save_path_refresh_is_never_throttled_and_keeps_the_monitor`.
  - The Ready multi-prefix gate test now resumes a real paused
    generation and requires a non-stale closure (`001c4278`).

### Fix-round gates (binary `rustred-c3d83cf2`, sha256 `c3d83cf2a87ac768706c869e58cedcc55c5cef95ec6486bb42595a1856c23e0a`)

Head `001c4278`: `fable_5_1-wave2` with `fable_5_1-scale-restore`
merged (`7f1bf35d`) plus the three fix commits. Built with
`cargo build --release --locked --offline` (the test build produced the
same sha256). Worktree logs are under
`.claude/worktrees/agent-ade877816b107b1cf/TMP/wave2fix/`. Controls ran on
CPUs 244-249 (102adcc3) and 250-255 (new), the suites on 212-243.

- **(a) fmt.** `cargo fmt --all -- --check` is clean (`fmt.log`).
- **(b) Rust suites.** `SYMBOLICA_LICENSE` confirmed (`license-set` in
  each log).
  - Lib: 777 passed / 0 failed / 6 ignored, twice (`suite-run1.log`,
    `suite-run2.log`). The sixth ignored test is the scale-restore
    branch's `restore_copied_production_checkpoint`.
  - `cli_routed_campaign`: 6 passed (`cli-routed-campaign.log`).
  - The gate test ran its licensed path with `--nocapture`
    (`gate-test-nocapture.log`): `ready_multi_prefix_gate domains=6
    events=2097168 completed=6 paused_published=1 paused_watermark=0`.
- **(d) Python.** 225 tests OK, 1 skipped (`python-suite.log`).
- **(e) Four-loop Ordered, strict.** `wave2-ref102-fix/<F>` (102adcc3) vs
  `wave2-new-fix/<F>` (`compare-strict-vs-ref102-fix.json`): 0 differing
  records for FG (98,909) and BMW (158,951).
  - The verdict prints FAIL only for `descendant_closure`. The keys that
    differ there are timing (`last_refresh_seconds`, `refresh_seconds`,
    `snapshot_age_seconds`), the two layout estimates and, for BMW only,
    `refresh_count` (8 vs 7). That count is wall-clock dependent: it
    counts throttled periodic scans. `dependency_edges`, `graph_revision`,
    `total_closed` and `initial_closed` are equal.
  - Checkpoints (`section-digests-vs-ref102-fix.json`, script
    `TMP/wave2fix/section_digests.py`): both end at generation 3. The
    domains, edges, index, ledger and nodes digests are identical. The
    records files differ in bytes only through timing keys: 0 of 98,909
    (FG) and 0 of 158,951 (BMW) records differ once `seconds`,
    `*_seconds` and `*_unix_time` are removed. The meta closure counters
    are equal except `refresh_count`.
  - Audit PASS on both new runs.
  - FG Ready: `wave2-new-fix-ready/fg` vs `wave2-ref102-fix-ready/fg`,
    multiset PASS (98,846 native, 40 delegated, 160 partial on both
    sides); audit PASS on both.
  - Wall time (informational; the two binaries ran concurrently on
    separate CPUs), whole command / traversal s, 102adcc3 -> new: FG
    17.5 / 13.3 -> 15.5 / 12.2, BMW 41.0 / 36.0 -> 33.0 / 28.1, FG Ready
    16.5 / 13.0 -> 15.0 / 11.1.
- **(f) Resume** (FG, stop at 40,000 committed; `report.json`,
  `compare.json`):

| Label | First -> second | Policy | Committed at stop | first_exit | resume_exit | Compare | Paused receipt (graph / snapshot revision, stale, total_closed) |
|---|---|---|---:|---:|---:|---|---|
| `wave2-resume-ord-fix` | 102adcc3 -> c3d83cf2 | Ordered | 60,758 | 4 | 0 | strict: 0 differing records | 471,398 / 471,398, false, 39,506 |
| `wave2-resume-ready-fix` | 102adcc3 -> c3d83cf2 | Ready | 65,279 | 4 | 0 | multiset PASS vs `wave2-ref102-fix-ready` | 504,014 / 504,014, false, 44,291 |
| `wave2-resume-rollback-ord-fix` | c3d83cf2 -> 102adcc3 | Ordered | 63,091 | 4 | 0 | strict: 0 differing records | 487,564 / 487,564, false, 42,591 |

  - The strict comparisons print FAIL only for the same keys as the
    pre-fix `wave2-resume-*` controls: the timing and estimate keys and
    `refresh_count` in `descendant_closure`, plus
    `uncommitted_inspections` (162 and 213 entries carried from the
    paused session on the resumed side, 0 in the uninterrupted
    reference).
  - The rollback row is the fix, seen in a fresh process. The paused
    generation written by the new binary now carries a current snapshot
    (`snapshot_stale` false, snapshot_revision = graph_revision), as
    102adcc3's paused generations do. 102adcc3 restored it (generation 3)
    and finished with 0 differing records.
  - Inference, not a same-state measurement: the scan is a deterministic
    function of the sealed flags and the edges. A current snapshot
    therefore has the CLOSED bits and closed counts that 102adcc3's
    never-cut scan computes for the same state. No control compared a
    paused generation byte for byte with 102adcc3 at an identical stop
    point, because the stop file makes the stop point timing-dependent.

## Open issues

- **Fold-then-continue on real data.** No control ran a periodic save in
  the middle of a walk: the interval is 3,600 s and the runs last about
  17 s. On real walk data, the CSR refresh path is exercised only after a
  restore (resume controls). The "fold, then keep walking with CSR plus
  log" path is covered by the random-graph unit test only. A control with
  a short `--checkpoint-interval-seconds` would exercise it.
- **No five-loop control.** No five-loop (N=15) control was run with a C3
  branch binary. The largest graph checked against 102adcc3 is BMW, with
  1.9M edges.
- **Missing scale measurements.** The benchmark is synthetic.
  - The production refresh time with the CSR is unmeasured.
  - The restore of a copied production CP5 checkpoint (design 7.1) has
    not been run. It should include the segment read path, which
    `a73fe146` changed.
  - The steady-state bytes/edge between saves is an estimate.
- **Re-tiled segments.** A full re-tile writes the edge segment in
  target-major (CSR) order. That happens only when a folded state is saved
  into another store, or after the u32-cap fold of `1fb162cb`. Whether
  102adcc3 accepts such a segment has not been exercised. Neither case is
  reachable in the production save flow at current scale.
- **Rollback.** Rollback was measured only with the merged wave-2 binary,
  not with `9f92cdb9` alone.
  - Correction: a stale-but-valid snapshot has been restored by 102adcc3.
    `wave2-resume-rollback-ord/fg` resumed generation 3, written by
    `rustred-53e672fc` with `snapshot_stale` true (`run1/result.json`:
    graph_revision 478,996, snapshot_revision 113,121), and exited 0 with
    0 differing records. Since `67c8125d`, only a save short of scratch
    memory can write such a snapshot.
- **Stale binaries.** `rustred-44c4dae7` (defective draft) must not be
  used. `rustred-0b69fd1a` is superseded by `rustred-9f92cdb9`, and the
  merged binary supersedes both.
- **Pre-existing flakes.** Two timing flakes pass in isolation and touch
  no C3 code:
  - `cli::shards::supervisor::tests::orphan_child_retains_campaign_lock_until_exit`
  - `walking::execution::ready_tests::ready_late_native_fault_after_cancellation_disallows_pause`

  Each failed once under host load in the first round, and the first one
  failed once more in the fix round.
