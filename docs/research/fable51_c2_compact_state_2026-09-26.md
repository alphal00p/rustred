# C2 compact in-RAM queue state: implementation record and control measurements (2026-09-26)

Scope: design section 2 of
[`fable51_design_checkpoint_memory_2026-09-26.md`](fable51_design_checkpoint_memory_2026-09-26.md)
(compact domains, compact summaries with retired-slot reuse, digest exact
index). Branch `fable_5_1-c2-compact` ("c2" in the branch names means
package C, wave 2; this is item C2). Commits `39f46fd6`, `b3037e25` and
`34d9cce9` sit on top of `fable_5_1` at `754200a3`. The review fix round
added `274cdc89`, `fc0e205c` and `d0a73b43` (section "Fix round" below).
`W/` = `crates/rustred-app/src/application/routed_campaign/walking/`. The
text below describes the branch head. Where the first version differed,
the fix round says so. Control directories are under
`TMP/fable51-controls/`; worktree logs are under
`.claude/worktrees/fable51-compact/TMP/`.

## What changed

- **Domains.** The queue keeps one fixed-size `CompactDomain<N>` per
  admitted ID (`W/queue/compact.rs`, new). It replaces an
  `Arc<Domain<N>>` with two heap vectors. The layout is: u16 coordinates
  with a +infinity sentinel, an owner bitmask (u32, so N <= 32), and flags
  for absent rank/power bounds. It is 96 B at N=15 (`size_of` test
  `compact_state_size_of_is_within_budget`). At the four-loop arity N=10
  it is 80 B, inferred from the FG `domain_bytes` of 10,485,760 = 131,072
  slots x 80 B. `Domain<N>` remains the transport type. `domain()`,
  `domain_arc()` and `expand_prefix()` expand it at dispatch, for records,
  for physical splits and for the initial indexes.
- **Summaries.** Each live lookup candidate has one `CompactSummary<N>`
  (176 B at N=15: u32 extrema plus u64/i64 aggregates). The old code kept
  a 560 B `DomainPowerSummary<N>` for every ID. The summaries sit in a
  slab with an ID -> slot map.
  - Retirement from the candidate index puts the slot on an intrusive free
    list, so the infallible retirement phase does not allocate. The next
    admission reuses the slot.
  - `PreparedLookup::revalidate` checks for a released slot before it
    reads a snapshot winner's signature. A released winner falls back
    exactly like `is_live == false`. The overflow guard still counts every
    admitted ID.
- **Wide summaries.** Some summaries have extrema that do not fit the
  compact form (only extreme power or rank bounds do this). They are
  stored as a wide marker. Any comparison involving one rebuilds the
  native summary from the queued domain, so no input is approximated.
  Every production comparison goes through `Stored`, which handles this
  case.
- **Exact index.** A map from a digest of the canonical compact bytes to
  the ID. Every hit is confirmed against the stored domain, and genuine
  digest collisions go to an overflow list.
  - First version: 128-bit blake3 digest, 24 B buckets.
  - Head (`fc0e205c`): a 64-bit prefix, 16 B buckets. A lookup miss now
    tells `try_reserve` whether a different domain holds the primary slot,
    so a published admission probes the table twice instead of three
    times.
  - The digest is never persisted. Restore rebuilds the index from the
    domains.
  - Helpers pass each proposal's compact image and digest with the
    prepared admission, so the coordinator does not hash it again.
- **Restore.** Domain sections are read back as bincode `Domain<N>` and
  range-checked into compact images. Only live candidates get summary
  slots. The domain vector is reserved once.
  - Since `274cdc89`, restore also refuses an indexed candidate ID that
    appears twice ("duplicate checkpoint index ID").
  - It also refuses an indexed ID that belongs to a different
    `(phase, owner)` than its bucket ("invalid checkpoint owner bucket").
  - Both checks run before the summaries are rebuilt. Otherwise a
    duplicated ID would keep a released slab slot reachable, and the walk
    would panic when a later scan reads it.
- **New refusal.** The only new refusal is a finite coordinate above
  65,534: "domain coordinate exceeds the compact queue range (finite
  coordinates must be <= 65534)". It fires at admission and at restore.
- **Telemetry.** A new `parallel.queue_storage` object appears in
  heartbeats, drains, the final report and the checkpoint meta `parallel`
  object (`b3037e25`). It reports the reserved bytes of the compact per-ID
  state:
  - domain images;
  - the digest exact index;
  - the summary slab with its slot map and filter words;
  - admitted IDs, live summaries and bytes per admitted domain.

  The candidate index blocks, the ledger and the closure are not
  included. `parallel` is session telemetry and is ignored by the strict
  result comparison.
- **Not implemented.** Design 2.3 (16 B ledger entry, optional) is not
  implemented. On-disk ledger bytes would not change. The saving is about
  32 B per ID (estimate).
- **Tests.** 8 new tests in `W/queue/tests/compact.rs`:
  - `compact_summary_contains_matches_core_on_random_boxes` (100k random
    pairs for each N=1..16, including more than 100 wide summaries per
    arity);
  - `compact_domain_round_trips_every_field_and_rejects_out_of_range`;
  - `coordinates_above_the_compact_range_are_refused_without_publishing`;
  - `campaign_query_bounds_are_inside_the_compact_range`;
  - `compact_state_size_of_is_within_budget`;
  - `exact_index_survives_injected_digest_collisions`;
  - `retired_summary_slots_are_released_and_never_read_by_revalidate`;
  - `summary_slab_recycles_retired_slots_and_restores_only_live_candidates`.

  `checkpoint::tests::rewritten_sections_fail_semantic_validation_not_checksums`
  gains the coordinate-65535, duplicate-index-ID and wrong-bucket cases.
  Design 2.4's `queue_admission_sequence_is_identical_under_compact_storage`
  was not written. The N=15 strict control below covers that comparison
  at control level instead.

Compatibility:

- Admission results, records, dependency edges and persisted counters are
  unchanged against `rustred-102adcc3`, measured below on four-loop N=10
  and five-loop N=15. `WALK_SEMANTICS_VERSION` stays 1.
- CP5 section encodings are unchanged. `cmp` of the nine binary sections
  (domains, index, ledger, nodes, edges) finds them byte-identical to the
  reference for FG, BMW, H, X and the five-loop control.
- The meta JSON differs from the reference only in timing values and in
  the added `parallel.queue_storage` object. Its top-level key set and its
  `progress` value are unchanged.
- A `rustred-102adcc3` checkpoint resumes under this code, on FG in both
  policies and on the five-loop control in Ordered.
- Rollback (a checkpoint written by this code resumed under 102adcc3) was
  not run with the branch binaries. It was run with the merged wave-2
  binary, which contains this branch; see "Merge into fable_5_1-wave2".
  In that control, 102adcc3 restored a meta carrying
  `parallel.queue_storage` and wrote it through to its next generation.

## Gates, first version (binary `rustred-50abb31f`, sha256 `50abb31fd7bcd254f9aa73b3c2c0b54f602ac3b665eb8275a2f2a306f304ca77`)

Setup:

- Commit `34d9cce9`.
- An earlier build at `b3037e25`, `rustred-ade72c16` (sha256
  `ade72c16f0a82a1bbfd0814f2377e90bb6f7815cf53f103e7b607ba02259cb98`),
  ran the `c2-new*` and `c2-resume-{ord,ready,self}` controls with the
  same outcomes.
- Reference binary: `rustred-102adcc3` (sha256
  `102adcc345ff3010496861f6057789632718cb86dbb1c7ac52d5107bc4c1f2ae`,
  `fable_5_1` at `343a86a7`, the running campaign's binary).
- All binaries are in `TMP/fable51-controls/bin/`.
- Controls ran on CPUs 234-239, W6, `nice -n 5`, sequentially; builds and
  tests on 224-239.
- Neighbouring cores were busy, so wall times are informational.

Results:

- **Suite.** 736 passed / 0 failed / 4 ignored at `34d9cce9`
  (`c2-test-4.log`, `SYMBOLICA_LICENSE` confirmed), against 728/0/4 at the
  branch point. `cargo fmt --all -- --check` is clean.
  - Two earlier full runs during development (`c2-test-1.log`,
    `c2-test-3.log`, 733/3/4 each) failed the two pre-existing timing
    flakes named below.
  - The same runs also each failed one test on the in-progress tree:
    `queue::tests::bits::restored_queue_rebuilds_bit_words_and_keeps_admitting_identically`
    in run 1 and `rewritten_sections_fail_semantic_validation_not_checksums`
    in run 3. Both pass at `34d9cce9` and at the head.
- **Four-loop Ordered.** `c2-ref102/<F>` vs `c2-new-r2/<F>` (and
  `c2-new/<F>`), with `compare_walk_records.py --mode strict`: identical
  (PASS), 0 differing records and no top-level difference for FG (98,909
  records), BMW (158,951), H (24,929) and X (47,193).
  - Counters are equal, e.g. for FG: 169,509,549 containment checks,
    149,787,346 maintenance checks, 308 retired candidates, 2,083,888
    deduplication hits and 292,359 exact hits.
  - The nine binary sections are byte-identical per family.
- **FG Ready.** `c2-new-r2-ready/fg` passes the audit and passes the
  multiset comparison against `c2-ref102-ready/fg` (98,846 native).
- **Resume.** All runs use FG, `resume_control.py`, and a stop at 40,000
  committed.

| Label | First -> second | Policy | Committed at stop | first_exit | resume_exit | Compare |
|---|---|---|---:|---:|---:|---|
| `c2-resume-ord-r2` | 102adcc3 -> 50abb31f | Ordered | 60,478 | 4 | 0 | strict: 0 differing records |
| `c2-resume-ready-r2` | 102adcc3 -> 50abb31f | Ready | 65,717 | 4 | 0 | multiset PASS |
| `c2-resume-self-r2` | 50abb31f -> 50abb31f | Ordered | 62,155 | 4 | 0 | strict: 0 differing records |

  The strict resume comparisons print FAIL only because of top-level keys
  that the 102adcc3 -> 102adcc3 self-test (`harness-selftest-102`) also
  lists:
  - `uncommitted_inspections` (the interrupted receipts of the paused
    session);
  - `descendant_closure` refresh telemetry: `last_refresh_seconds`,
    `refresh_count`, `refresh_seconds`, `retained_storage_estimate_bytes`,
    `snapshot_age_seconds`.

## Fix round (review of the first version)

The review ran three reviewers on the branch at `34d9cce9` (restore,
semantics, tests/performance). A triage step merged their output into
seven findings, and two adversarial verifiers checked each finding
(workflow journal `.../subagents/workflows/wf_6afddf23-7b2/journal.jsonl`
of session `7dfabea8`). Findings that at least one verifier confirmed were
handed to the fix round.

Commits on `fable_5_1-c2-compact` after `34d9cce9`:

1. `274cdc89` (duplicate index ID; minor, confirmed by both verifiers).
   - `restore_summaries` marked every indexed ID live and silently ignored
     repeats. `AggregateIndex::restore_positions` checks only per-group
     order and live counts.
   - So an ID listed in two groups or two buckets restored with one slab
     slot. Its first retirement released the slot, and a later scan read
     `entries[u32::MAX]`. The result is a deterministic index panic on the
     coordinator in release builds, where the old per-ID summary vector
     never freed anything.
   - Restore now refuses the repeated ID and the wrong-bucket ID. Two
     corruption cases were added.
   - Correct writers never produce either case, so 102adcc3 checkpoints
     are unaffected (the resume controls below).
2. `fc0e205c` (128-bit exact-index key, three probes; minor, confirmed by
   both).
   - Exactness never depended on the digest width, because every hit is
     confirmed and collisions overflow.
   - The key is now one u64: buckets shrink from 24 B to 16 B, and
     admission probes twice instead of three times.
   - Expected colliding pairs at 38M domains: about 4e-5 (estimate).
   - Measured on FG: `exact_index_bytes` 3,276,800 -> 2,228,224 and
     `bytes_per_admitted_domain` 335.3 -> 324.7.
   - IDs stay `usize`: a u32 ID would not shrink the (u64, id) bucket, and
     it would add a refusal.
3. `d0a73b43` (collision test bypassed production restore and prepared
   paths; one verifier confirmed, one refuted).
   - The test had rebuilt the index with a hand-copied loop.
   - `Queue::restore_from_parts` now delegates to `restore_with_index`,
     and the JSON image restores through `restore_image`.
   - The test now drives the production restore under injected
     collisions, refuses a genuinely repeated domain, and continues the
     restored queue serially.
   - It also runs `parallel_prepare` + `admit_prepared` against
     uncolliding serial and prepared references.
4. No code change (no N=15 comparison had been run; one verifier
   confirmed, one refuted).
   - All controls so far were four-loop, N=10. The finding was closed with
     measured five-loop N=15 controls (next subsection).

Refuted by both verifiers and not applied:

- **The 65,534 refusal is unreachable only by inference.**
  - The verifiers found no path to a coordinate of 65,535. The observed
    maxima are about 15-16, and every live domain has a finite rank.
  - The factual part stands: the bound is empirical, and nothing reports
    the largest admitted coordinate (see "Open issues").
- **The design 2 sub-target of about 250 B is missed, and the 0.68 live
  fraction does not hold.**
  - The live fractions quoted came from small or early runs. The verifiers
    read 0.518 from the live campaign's `events.jsonl`, below the design's
    0.68.
  - `queue_storage` reports reserved capacities, not logical bytes.
- **No queue-level wide-summary test.**
  - Every production comparison goes through `Stored`, and the
    differential test drives `Stored` with wide summaries at every arity.
  - The finding describes a possible future misuse, not a defect.

Skipped parts of applied findings:

- The replay diff at branch point vs head (finding 4): the full N=15
  strict control was run instead.
- A per-ID summary-signature check at restore (finding 1): a mismatch can
  only trigger the `is_live` fallback, and the gap predates C2.
- Avoiding the insert probe (finding 2): not possible with std `HashMap`.

### Fix-round gates (binary `rustred-dfe4396d`, sha256 `dfe4396dc58c617e93fc43343489ebdb7c9fe48380a9e41f3a7029048920bbfc`)

The binary was built before the last test-only amend and rebuilt at
`d0a73b43` with the same sha256 (`c2-progress.md`). Reference: 102adcc3 as
above. CPUs 234-239.

- **Suite.** 736/0/4 at `d0a73b43` (`c2-fix-test-2.log`, license-set in
  the log). `cargo fmt` is clean.
  - The first fix-round run (`c2-fix-test-1.log`, 735/1/4) failed the
    implementer's own new assertion. It compared a prepared colliding
    queue's containment checks with a serial reference (14,496 vs 14,486).
  - A revalidated prepared scan may do more or less work than a serial
    one, so the test now compares against an uncolliding prepared
    reference.
- **Four-loop Ordered.** `c2-ref102-fix/<F>` vs `c2-new-fix/<F>`: strict
  PASS, 0 differing records, no top-level difference for FG, BMW, H and X.
  The nine binary sections are byte-identical per family (`cmp`, checked
  again for this note). `audit_owner_domain_walk.py` passes.
- **FG Ready.** `c2-ref102-fix-ready/fg` vs `c2-new-fix-ready/fg`:
  multiset PASS (98,846 native, 40 aliases, 160 partial) and audit PASS.
- **Five-loop N=15 Ordered.** `c2-ref102-fix/five-finite` vs
  `c2-new-fix/five-finite`. This is the 1,324-tuple W50 command of the
  `baseline-32fdec-w50-*` controls, run at `--workers 6` on 234-239.
  - 0 differing records of 1,273,376.
  - The only top-level difference is `descendant_closure` refresh
    telemetry: `refresh_count` 17 vs 15 and the `*_seconds` values (the
    refresh is time-throttled).
  - `dependency_edges` (12,518,693), `graph_revision`, `total_closed` and
    every counter are equal.
  - Both runs also equal the 32fdec W50 baseline (`baseline-32fdec-w50-ordered`):
    - 967,621 native inspections;
    - 1,273,376 scheduled;
    - 5,307,741,824 containment checks;
    - 1,147,804,267 maintenance checks;
    - 551,365 retired candidates.
  - Audit PASS. The nine binary sections are byte-identical.
- **Five-loop N=15 Ready.** `c2-new-fix-ready/five-finite` passes the
  audit. The multiset comparison is not an equality gate at N=15:
  - Two 102adcc3 runs of the same command already disagree
    (`c2-ref102-fix-ready` vs `c2-ref102-fix-ready-r2`): 972,880 vs
    972,179 native, with 4,608 / 3,991 shapes only on either side.
  - 102adcc3 vs dfe4396d gives 972,880 vs 973,314 native and 4,512 / 4,797
    shapes.
- **Resume.**

| Label | First -> second | Policy | Committed at stop | first_exit | resume_exit | Compare | Restore |
|---|---|---|---:|---:|---:|---|---|
| `c2-resume-ord-fix/fg` | 102adcc3 -> dfe4396d | Ordered | 59,599 | 4 | 0 | strict: 0 differing records | 0.97 s, RSS 593 MB |
| `c2-resume-ready-fix/fg` | 102adcc3 -> dfe4396d | Ready | 63,921 | 4 | 0 | multiset PASS | 1.15 s |
| `c2-resume-self-fix/fg` | dfe4396d -> dfe4396d | Ordered | 61,342 | 4 | 0 | strict: 0 differing records | 1.00 s |
| `c2-resume-ord-fix/five-finite` | 102adcc3 -> dfe4396d | Ordered | 488,161 | 4 | 0 | strict: 0 differing records of 1,273,376 | see below |

  The N=15 resume paused at 488,161 committed domains with 146,985
  pending:
  - The CP5 checkpoint was 411,495,738 bytes.
  - Restore took 5.66 s, including 4.39 s decode and 1.27 s validation.
    RSS after restore was 3,137,589,248.
  - `checkpoint_executable_changed` reports `walk_semantics_version` 1.
  - The resumed run took 330.8 s.

  The strict resume comparisons differ at top level only in
  `uncommitted_inspections` and the five `descendant_closure` telemetry
  keys, the key set of `harness-selftest-102`.

Pre-existing timing flakes seen while gating (they pass in isolation and
touch no C2 code):

- `cli::shards::supervisor::tests::orphan_child_retains_campaign_lock_until_exit`
- `walking::execution::ready_tests::ready_late_native_fault_after_cancellation_disallows_pause`

Neither failed in the fix-round runs.

## Memory measurements

Peak RSS comes from `run_control.py` (`/proc` polling, `metrics.json
peak_rss_bytes`). All runs are single, at W6.

| Case | 102adcc3 | dfe4396d | Change |
|---|---:|---:|---:|
| FG Ordered | 1,184,448,512 | 1,141,506,048 | -3.6% |
| BMW Ordered | 1,879,244,800 | 1,785,294,848 | -5.0% |
| H Ordered | 668,577,792 | 669,769,728 | +0.2% |
| X Ordered | 1,363,079,168 | 1,334,358,016 | -2.1% |
| FG Ready | 1,171,898,368 | 1,124,335,616 | -4.1% |
| five-loop Ordered | 15,081,582,592 | 13,996,363,776 | -7.2% |
| five-loop Ready | 15,052,464,128 | 13,974,257,664 | -7.2% |

At four-loop size the per-committed-record JSON dominates RSS on both
binaries; C1 removes that. So the four-loop peaks move by only a few
percent.

Same final state, last heartbeat `process_rss_bytes` (`events.jsonl`, all
domains discovered and committed):

- Five-loop Ordered: 13,955,051,520 -> 13,319,475,200, which is 635.6 MB,
  or 499 B per discovered domain (1,273,376).
- Five-loop Ready: 13,936,328,704 -> 13,295,681,536, which is 640.6 MB.
- The peak difference of the Ordered pair is 852 B per discovered domain.
  The peak includes finalization and the report write, so the
  last-heartbeat figure is the cleaner one.
- Four-loop last-heartbeat differences are 232-616 B per domain (FG 232,
  BMW 447, H 616, X 534, FG Ready 281). At about 10^5 domains these
  figures are noisy.

`parallel.queue_storage` (reserved capacities, including Vec/HashMap
growth slack; the candidate index blocks, ledger and closure are
excluded):

| Run | Admitted | Live summaries | B / admitted, first version -> head |
|---|---:|---:|---:|
| FG Ordered (`c2-new-r2` -> `c2-new-fix`) | 98,909 | 98,601 | 335.3 -> 324.7 |
| BMW Ordered | 158,951 | 130,934 | 305.1 -> 291.9 |
| H Ordered | 24,929 | 24,029 | 332.6 -> 322.0 |
| X Ordered | 47,193 | 45,722 | 351.3 -> 340.2 |
| five-loop Ordered (`c2-new-fix/five-finite`) | 1,273,376 | 722,011 | - -> 350.8 |
| five-loop Ready (`c2-new-fix-ready/five-finite`) | 1,269,303 | 724,821 | - -> 351.9 |

- The five-loop Ordered breakdown is: domains 201,326,592 (a
  2,097,152-slot vector for 1,273,376 domains), summary slab 192,937,984,
  exact index 35,651,584 and filter words 16,777,216. Total 446,693,376.
- Final live fractions are 0.567 on the five-loop control and 0.997 / 0.824
  / 0.964 / 0.969 on FG / BMW / H / X.
- The ignored replay `replay_committed_domain_admissions` covered the
  first 100k descriptors of `baseline-32fdec-w50-ordered/five-finite`. It
  reported 405.0 B per admitted domain, with 96,189 live of 100,000
  (`c2-replay-five-100k.log`, first version).

Coordinator duty (`parallel.coordinator_duty`, five-loop pair, W6):

| Run | ordered_commit (s) | per committed domain | preparation (s) | wait (s) | coordinator elapsed (s) |
|---|---:|---:|---:|---:|---:|
| Ordered 102adcc3 / dfe4396d | 65.9 / 54.0 | 51.7 / 42.4 us | 76.3 / 77.7 | 125.9 / 120.0 | 338.2 / 322.7 |
| Ready 102adcc3 / dfe4396d | 64.1 / 51.7 | 50.5 / 40.8 us | 78.1 / 79.4 | 137.3 / 131.8 | 322.6 / 307.0 |

- Commit time per domain fell by about 18-19% and preparation rose by
  about 2%, in one run pair each.
- The coordinator was not saturated here (it waited about 120-137 s of
  about 320 s), unlike the live W100 campaign. How this transfers to a
  saturated coordinator has not been measured.
- Wall time (informational, whole command / traversal s): five-loop
  Ordered 453.0 / 347.8 -> 436.3 / 331.0, five-loop Ready 437.9 / 332.6
  -> 421.8 / 315.3, FG 16.5 / 12.87 -> 16.5 / 12.74, BMW 36.0 / 30.77 ->
  35.0 / 29.98.

## Estimates (not measured)

- **Per-ID payload by `size_of`**, before allocator and hash-table
  overhead:
  - Old: about 1,088 B, the implementer's figure for the Arc block, the
    two coordinate vectors, the native summary, the map entry and the
    filter word.
  - New: 300 B (96 + 176 + 4 + 16 + 8) while the summary is live.
  - At the five-loop control's live fraction of 0.567, the summary term
    drops to about 100 B, for about 224 B per ID.
- **Design targets.**
  - Design 2.5's gate (<= 0.45 KB per discovered domain for
    queue + ledger + closure nodes, on the synthetic 20M-domain state of
    design 7.3) has not been run.
  - The <= 0.8 KB whole-state target is therefore not claimed.
- **Campaign scale.** The last heartbeat of the live v2 campaign
  (`campaigns/five-loop-qcd-feynman-d9d10-v2/runs/20260926T151353.794886Z/events.jsonl`,
  run elapsed 32,878 s, file time 2026-09-27 00:22 UTC, read-only) shows:
  - 48,582,681 discovered domains;
  - 24,944,728 containment candidates, a live fraction of 0.513;
  - `process_rss_bytes` 242,982,969,344.

  Scaling linearly by the control's measured 499-852 B per discovered
  domain gives roughly 24-41 GB less RSS at that state. The control is
  38x smaller, and allocator and hash-table behaviour at 48.6M domains is
  not measured.
- Design 2.3 would save about 32 B per ID more, about 1.6 GB at 48.6M.

## Merge into fable_5_1-wave2

- **Merge `aa179fdd`.** It had one conflict, in
  `State::commit_delegated_id` (`execution/delegation.rs`). C1 reserves
  the record through the sink, and C2 expands the compact image. The
  resolution builds the record from `queue.domains[id].expand()` and
  reserves through `records.get_mut().reserve_one()`. The record JSON is
  unchanged.
- **Follow-up `971bf34c` (test only).** The merge broke two B2 tests that
  freeze the per-domain event key sets
  (`per_domain_event_key_sets_are_frozen_and_heartbeats_only_add_pinned_telemetry`
  and `observe_attaches_the_lean_pool_tier_to_per_domain_events_only`): C2
  adds `parallel.queue_storage` to heartbeats and drains. The pinned
  heartbeat lists now include it, and the per-domain lean key sets are
  unchanged.
- **Merged binary rollback.** The merged binary `rustred-53e672fc`
  (sha256 `53e672fc3c082b2bbe696bd37c5badff346c5191b3cb0b010d80fa70b9534d90`;
  `TMP/wave2/build-final.log` in the wave-2 worktree reports the same
  sha256 after `971bf34c`) was rollback-tested:
  - `wave2-resume-rollback-ord/fg` (53e672fc -> 102adcc3): first_exit 4,
    resume_exit 0, 0 differing records.
  - `wave2-resume-rollback-ready/fg`: multiset PASS.
  - In the Ordered rollback, 102adcc3 restored the paused generation 3,
    which the merged binary wrote. It then wrote generation 4 with
    `parallel.queue_storage` still present. 102adcc3 does not produce that
    object itself, so it carried it over from generation 3, which means it
    accepts the key. Generation 5 no longer has it.
  - The other integration gates of the merged binary are recorded
    separately (`wave2-*`).

## Open issues

- **Coordinate headroom.** The live checkpoint's largest finite coordinate
  has not been scanned. The bound of 65,534 is supported by inputs (15),
  the FG control (16) and the five-loop control (15), not by structure.
  Nothing reports the largest admitted coordinate. If a 102adcc3
  checkpoint held a coordinate above 65,534, this code would refuse it at
  restore, before writing anything, so the checkpoint and the rollback
  would be untouched. If such a domain were first proposed after an
  upgrade, the walk would stop with an admission error.
- **Missing gates.** The synthetic 20M-domain gate of design 2.5 / 7.3 and
  the timed restore of a copied production checkpoint (design 7.1) have
  not been run. Design 2.3 is not implemented.
- **Worker count.** The N=15 controls ran the W50 command at W6, not W50.
  Ordered results did not depend on worker count here: the W6 runs
  reproduce the W50 baseline counters. No saturated-coordinator (W50/W100)
  comparison exists.
- **Ready at N=15.** Ready five-loop results are not reproducible
  run-to-run even on 102adcc3, so Ready at N=15 is gated by the audit
  only. The Ordered strict comparison is the semantic evidence.
- **Rollback.** Rollback was measured only with the merged wave-2 binary,
  not with `dfe4396d` alone.
- **Resumed audits.** `audit_owner_domain_walk.py` flags resumed `run2`
  directories ("uncommitted_inspections must be empty", "pool
  returned_inspections != native records"). The 102adcc3 -> 102adcc3
  self-test shows the same two violations. B2's `a6866db5` (merged in
  wave 2) makes the audit accept exactly such carried entries on a
  `--resume` run. It was not rerun on these directories.
- **Stale binaries.** `rustred-ade72c16` and `rustred-50abb31f` are
  superseded by `rustred-dfe4396d`, and the merged binary supersedes all
  three.
