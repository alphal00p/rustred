# C1 records sidecar: implementation record and control measurements (2026-09-26)

Scope: design section 1 of
[`fable51_design_checkpoint_memory_2026-09-26.md`](fable51_design_checkpoint_memory_2026-09-26.md)
plus review finding #7 of
[`fable51_review_checkpoint_branch_2026-09-26.md`](fable51_review_checkpoint_branch_2026-09-26.md).
Branch `fable_5_1-c2-sidecar`, commit `553feb3e` on top of `fable_5_1` at
`754200a3`, then the review fix round (commits `e98414e2`..`f5fe5c85`, section
"Fix round" below). `W/` = `crates/rustred-app/src/application/routed_campaign/walking/`.
The text below describes the branch head; where the first version differed,
the fix round says so.

## What changed

- Checkpointed walks keep no committed record in RAM beyond one unwritten
  64 KiB batch of serialized lines.
  `W/execution/records.rs`: `RecordSink::{Memory, Sidecar}`; the sidecar
  serializes each record when it is committed and appends it to the open
  `records-<G>.jsonl` of the generation the store reserved (file created at
  the first push; whole lines written in 64 KiB batches, blake3 while
  writing; the unwritten batch stays readable in RAM). `Store::save` writes
  the batch and seals that segment (fsync, digest, directory sync) instead of writing records
  from RAM. Non-checkpointed runs keep the in-memory sink.
- Crash consistency: an unsealed segment is referenced by no manifest;
  resume ignores it, `next_free_generation` never reuses its generation and
  segment-aware cleanup removes it once it is below the previous generation.
  A segment sealed by a save that then failed is listed by the next
  generation.
- Restore reads no record. `validate_closure_records` is replaced by
  `validate_ledger_closure` (design 1.4): the record/native inventory
  (`records.total == published`, natives against the ledger or the Ordered
  watermark) runs first and also when the closure monitor is unavailable
  (review #7); then per-ID `(inspected, sealed)` and the required alias and
  partial-anchor edges from `Ledger::closure_expectation`; without a ledger
  only inspection is per ID and unsealed inspections are bounded by the
  frontier count.
- Ready accepted events: running aggregate `State::records_accepted_events`,
  persisted as the key `records_accepted_events` inside the free-form meta
  `progress` value (Ready only; the top-level meta key set and the Ordered
  progress value are exactly the old ones). A checkpoint without it (every
  checkpoint of the fable_5_1 binaries) derives it once on restore by
  streaming the record segments in parallel (one line per reader retained);
  the `checkpoint_restored` event reports `records_accepted_events_derived`.
- Finalization: `finalize_delegation` returns the per-ID resolutions;
  `Annotations` applies them and `descendant_closed` while the records are
  read back. `OwnerDomainWalkResult` gains `records: OwnerDomainWalkRecords`
  with `write_json` / `into_document`; the CLI streams `domains` into
  `result.json` inside the existing atomic writer. For checkpointed walks
  `document["domains"]` is `null` in the library result.

Compatibility: section encodings, file names, JSONL bytes and manifest
segment entries are unchanged; the only new persisted field is the Ready
progress key, which the fable_5_1 binaries ignore. Compatibility therefore
holds both ways: a `rustred-102adcc3` checkpoint resumes under this binary,
and a checkpoint this binary wrote resumes under `rustred-102adcc3` (an
executable-history rollback; measured below). `WALK_SEMANTICS_VERSION`
stays 1.

## Gates (binary `rustred-83a78d90`, sha256 `83a78d9040976087ff340c25f1cd519161284949f625419cbc6f1f68f2011a7c`)

Reference binary: `rustred-102adcc3` (sha256
`102adcc345ff3010496861f6057789632718cb86dbb1c7ac52d5107bc4c1f2ae`, fable_5_1 at
`343a86a7`, the running campaign's binary). Both in
`TMP/fable51-controls/bin/`. All runs on CPUs 250-255, W6, `nice -n 5`,
sequential; neighbouring cores were busy with other builds, so wall times
are informational.

- Suite: `cargo test --release --locked --offline -p rustred-app --lib`
  739 passed / 0 failed / 4 ignored (branch point 728/0/4; 11 new tests),
  `SYMBOLICA_LICENSE` set; `cargo fmt --all -- --check` clean.
- Four-loop Ordered controls (`TMP/fable51-controls/c1-ref102/<F>` vs
  `c1-new/<F>`): `compare_walk_records.py --mode strict` 0 differing records
  for FG (98,909), BMW (158,951), H (24,929), X (47,193); FG/H/X PASS, BMW
  lists `descendant_closure.refresh_count` 7 vs 8 (the time-throttled periodic
  refresh; session telemetry). A positional line diff of the pretty
  `result.json` files, timing values masked, differs only in 20-21 lines per
  family: checkpoint bookkeeping (directory, executable, state_path, bytes,
  new_bytes, per-section seconds), `parallel` scheduling telemetry and per-slot
  timing arrays, and BMW's `refresh_count`. `audit_owner_domain_walk.py`
  PASS on all four new runs.
- Checkpoint sections of the same runs: nodes, ledger, index, domains and
  edges digests identical to the reference, same generations (2, 3) and
  segment tiling; the record segment is byte-identical after masking the
  `seconds` values (FG 132,130,733 vs 132,130,797 bytes, 98,909 lines).
- FG Ready (`c1-ref102-ready/fg` vs `c1-new-ready/fg`): multiset PASS
  (98,886 logical, 98,846 native, 40 aliases, 160 partial), audit PASS.
- Resume (`resume_control.py`, stop at 40,000 committed, FG):

| Label | First -> second | Policy | first_exit | resume_exit | Compare | Restore |
|---|---|---|---:|---:|---|---|
| `c1-resume-ord` | 102adcc3 -> 83a78d90 | Ordered | 4 | 0 | strict: 0 differing records | 57,804 records, 0.13 s, RSS 89.7 MB |
| `c1-resume-ready` | 102adcc3 -> 83a78d90 | Ready | 4 | 0 | multiset PASS | 66,029 records, aggregate derived, 0.37 s |
| `c1-resume-self` | 83a78d90 -> 83a78d90 | Ordered | 4 | 0 | strict: 0 differing records | 58,694 records, 0.13 s |
| `c1-resume-self-ready` | 83a78d90 -> 83a78d90 | Ready | 4 | 0 | multiset PASS | 64,941 records, 0.14 s |

  The strict comparisons list exactly the top-level keys of the
  102adcc3 -> 102adcc3 self-test (`harness-selftest-102`):
  `uncommitted_inspections` (interrupted receipts of the paused session) and
  `descendant_closure` {last_refresh_seconds, refresh_count, refresh_seconds,
  retained_storage_estimate_bytes, snapshot_age_seconds}.

Re-check with the branch head binary (`8ccdc676`, a behaviour-neutral
refactor of the sealing code; `rustred-f2e00db9`, sha256
`f2e00db968de69a353d13ffd1a6dcc962e5fe5254a84e52e99c5960f1c2c1c89`):
`c1-head/fg` strict PASS (0 differing), `c1-head/bmw` 0 differing records
with only `descendant_closure.refresh_count` 7 vs 8; `c1-head-resume-ord`
(102adcc3 -> f2e00db9) first_exit 4, resume_exit 0, 0 differing records,
same telemetry keys as above; `c1-head-resume-ready` multiset PASS with the
accepted-events aggregate derived. Peak RSS FG 302,571,520, BMW 529,825,792;
BMW simple RSS slope 989 B per committed domain.

Suite flakes seen while gating (both pre-existing: the same two tests fail
in `TMP/fable51-merge/gate4.log` before this change, 727/2/4):
`cli::shards::supervisor::tests::orphan_child_retains_campaign_lock_until_exit`
and `walking::execution::ready_tests::ready_late_native_fault_after_cancellation_disallows_pause`
each failed once in 7 full runs of the final tree under load and pass in
isolation; the other runs were 739/0/4.

## Memory measurements

Peak RSS from `run_control.py` (`/proc` polling, `metrics.json
peak_rss_bytes`):

| Case | 102adcc3 | 83a78d90 | Change |
|---|---:|---:|---:|
| FG Ordered | 1,178,738,688 | 305,569,792 | -74.1% |
| BMW Ordered | 1,880,399,872 | 531,574,784 | -71.7% |
| H Ordered | 693,063,680 | 513,429,504 | -25.9% |
| X Ordered | 1,367,154,688 | 935,956,480 | -31.5% |
| FG Ready | 1,170,984,960 | 296,841,216 | -74.7% |

RSS against committed domains from the heartbeat `process_rss_bytes` in
`events.jsonl` (least squares over the heartbeats with committed > 0):

| Case | Points | Simple slope (B / committed), ref -> new | Two-variable fit, B / committed (ref -> new) |
|---|---:|---:|---:|
| BMW Ordered | 29 / 31 | 9,443 -> 999 | 7,873 -> -1,804 |
| FG Ordered | 13 / 13 | 9,580 -> 708 | 7,353 -> -2,500 |
| FG Ready | 12 / 12 | 9,680 -> 662 | 6,411 -> -1,610 |

The two-variable fit (RSS = a + b x discovered + c x committed) is poorly
conditioned because discovered and committed domains grow together; its
per-committed coefficient is consistent with zero, the simple slope bounds
what is left (discovered-domain state plus allocator noise). The
reference's ~9.5 KB per committed domain was the retained `serde_json::Value`
record (1.34 KB of JSON on FG).

Finalization and `result.json` write: peak RSS minus the largest heartbeat
RSS bounds what the process added after its last heartbeat (finalization,
annotation, report write): FG 305,569,792 - 300,138,496 = 5.4 MB and BMW
531,574,784 - 529,260,544 = 2.3 MB for the new binary (reference: FG
21.4 MB, BMW 52.0 MB). This is an upper-bound indicator at four-loop size,
not the design 1.6 measurement at 7M records.

Wall time (informational, whole command / traversal seconds, ref -> new):
FG 17.0 / 13.30 -> 16.5 / 13.04; BMW 36.0 / 30.34 -> 36.0 / 30.98; H
17.0 / 12.48 -> 16.0 / 12.14; X 40.5 / 34.16 -> 39.5 / 33.16; FG Ready
16.0 / 12.11 -> 15.5 / 11.67. Wall time does not show the coordinator cost
of moving serialization to the commit: see "Coordinator cost per record" in
the fix round, which measured it and replaced the first version's
per-record write.

## Fix round (review of the first version)

Five review findings, each checked by two adversarial verifiers; all five
were confirmed and all five are applied. Commits on `fable_5_1-c2-sidecar`
after `b9187010`:

1. `e98414e2` (major). The first version persisted the Ready aggregate as a
   top-level meta key on every Ready save. `rustred-102adcc3` denies unknown
   meta keys, so after one save on the new binary a Ready campaign could no
   longer roll back to it through the executable history
   (`production_saved_owner_campaign.py --upgrade-executable`, "Rolling
   back" in `docs/shared_owner_campaign_driver.md`), although the semantics
   version is the same and neither the probe nor the tooling could tell.
   The aggregate now lives in `progress` (see "What changed"); restore
   takes it out before the progress parser, refuses a malformed value and
   derives an absent one. Test
   `meta_section_keeps_the_key_set_the_fable_5_1_binaries_accept` pins the
   meta and progress key sets to the 343a86a7 lists; the rollback controls
   below run the old binary on a checkpoint this binary wrote.
2. `5edad80b` (two minor findings). A failed seal (segment or directory
   fsync) consumed the open segment, so the failed run's `result.json`
   silently lacked the records since the previous save; the seal now
   borrows the segment and the tail stays readable. A record write failure
   in `commit_delegated_id` returned before the publisher's cursor followed
   the ledger, which then reported a secondary "delegation final cursor
   mismatch"; the cursor now follows the ledger first, as on the native
   path. The failure tests no longer use directory permissions (skipped as
   root): ENOTDIR, EBADF and ENOENT inject the failures.
3. `0f87171c` (filed major, rated minor by both verifiers). Negative tests
   for `validate_ledger_closure`: a sealed ledger native that kept a
   frontier; a no-ledger native without frontiers left unsealed, plus the
   exact frontier bound; a missing partial-anchor edge; an anchor outside
   the initial prefix (constructible by shrinking the prefix recorded in
   meta). The comment now says that the no-ledger path cannot detect a
   sealed native that kept frontiers; the former record scan could.
4. `f5fe5c85` (minor). The coordinator cost of the per-record write was
   measured and was visible, so records are now written in 64 KiB batches
   (next subsection).

Not applied: a verifier's optional `domains_complete` marker in a failed
run's `result.json`. With the fixes a record-count gap remains only when a
record write itself failed; the run is then `incomplete` with the sidecar
error, and `audit_owner_domain_walk.py` already checks `processed_nodes`
against the record count. No `result.json` key was added.

### Coordinator cost per record

`parallel.coordinator_duty` of `result.json`, divided by committed domains.
In `rustred-102adcc3` the records are serialized during the saves, on the
coordinator thread (it waits in the save's `rayon::scope`); here they are
serialized at commit. The final save is outside the duty buckets, so it is
listed separately; the periodic saves did not fire in these runs
(`checkpoint_seconds`, the initial forced save, is at most 0.2 us per
record for every binary). CPUs 250-255, W6, sequential, neighbouring cores
busy: timings are indicative.

| Run (label) | Binary | publication (us/rec) | final save (s) | publication + final save (us/rec) |
|---|---|---:|---:|---:|
| FG Ordered `c1-ref102` | 102adcc3 | 15.0 | 0.75 | 22.6 |
| FG Ordered `c1-new` | 83a78d90 (per-record write) | 29.4 | 0.30 | 32.5 |
| FG Ordered `c1-perf-ref102` | 102adcc3 | 14.9 | 0.73 | 22.3 |
| FG Ordered `c1-perf-batch` | 66609723 (batched) | 19.0 | 0.07 | 19.7 |
| FG Ordered `c1-ref102-fix` / `c1-new-fix` | 102adcc3 / 66609723 | 14.9 / 19.6 | 0.76 / 0.08 | 22.7 / 20.4 |
| BMW Ordered `c1-ref102-fix` / `c1-new-fix` | 102adcc3 / 66609723 | 14.6 / 19.4 | 1.27 / 0.09 | 22.6 / 20.0 |
| FG Ready `c1-ref102-fix-ready` / `c1-new-fix-ready` | 102adcc3 / 66609723 | 16.1 / 19.3 | 0.73 / 0.06 | 23.5 / 20.0 |
| five-loop finite Ready W6 `c1-perf-ref102-w6` / `c1-perf-new-w6` | 102adcc3 / 66609723 | 10.5 / 13.8 | 6.90 / 0.72 | 15.9 / 14.3 |

The per-record write(2) of the first version cost about 10 us per record on
this ZFS dataset (29.4 -> 19.0 us on FG). With batching the commit path
still carries about 3-5 us per record more than the reference (the
serialization and the drop of the record), and the reference pays about
5-8 us per record in its saves instead; in total the batched binary spends
less coordinator time per record than the reference in every pair above
(the first version, `c1-new`, spent more).

The five-loop finite control (`run_control.py --family five-finite`, the
1,324-tuple input of the W50 baseline) ran at W6 on 250-255, not at W50:
this track's CPU budget is 16 cores. Its coordinator waited 138 s (ref)
and 135 s (new) of about 315 s, so it was not saturated; the per-record
duty above is what transfers to a saturated coordinator. Same run pair:
1,268,583 / 1,268,810 committed domains, traversal 324.1 / 319.6 s,
committed domains per second 3,914 / 3,970 (informational), peak RSS
15,051,309,056 / 7,455,154,176 bytes, heartbeat RSS slope 7,310 / 1,364 B
per committed domain. A W50 comparison on the saturated coordinator has not
been run.

### Fix-round gates (binary `rustred-66609723`, commit `f5fe5c85`)

sha256 `6660972313f768d066e8dd8865633fb09c9cd9b00b108fcf90944254f1a64082`;
reference `rustred-102adcc3` as above; all in `TMP/fable51-controls/`.

- Suite: 746 passed / 0 failed / 4 ignored with `SYMBOLICA_LICENSE` set
  (739 + 7 new tests); of three full runs, the other two each had one of the
  two pre-existing timing flakes named above (745/1/4), and both pass 8/8
  in isolation. `cargo fmt --all -- --check` clean.
- Four-loop Ordered (`c1-ref102-fix/<F>` vs `c1-new-fix/<F>`): strict
  comparison PASS for FG (98,909 records), BMW (158,951), H (24,929) and X
  (47,193): 0 differing records and no top-level difference outside the
  ignored session telemetry. Checkpoint nodes, ledger, index, domains and
  edges digests identical, same generation (3) and record tiling, and the
  record segments identical once the `seconds` values are masked. Audit
  PASS on all four.
- FG Ready (`c1-ref102-fix-ready` vs `c1-new-fix-ready`): multiset PASS
  (98,846 native, 40 aliases, 160 partial); audit PASS.
- Peak RSS (bytes, ref -> new): FG 1,184,120,832 -> 301,391,872; BMW
  1,882,693,632 -> 526,774,272; H 686,370,816 -> 510,976,000; X
  1,363,726,336 -> 934,166,528; FG Ready 1,176,064,000 -> 295,780,352.
  Heartbeat RSS slope, B per committed domain: BMW 9,460 -> 958, FG 9,598
  -> 729, FG Ready 9,668 -> 717.
- Resume controls (FG, stop at 40,000 committed; every first run exit 4,
  every resume exit 0):

| Label | First -> second | Policy | Compare | Restore |
|---|---|---|---|---|
| `c1-resume-ord-fix` | 102adcc3 -> 66609723 | Ordered | strict: 0 differing records | 60,725 records, not derived |
| `c1-resume-ready-fix` | 102adcc3 -> 66609723 | Ready | multiset PASS | 66,096 records, aggregate derived |
| `c1-resume-self-fix` | 66609723 -> 66609723 | Ordered | strict: 0 differing records | 60,345 records |
| `c1-resume-self-ready-fix` | 66609723 -> 66609723 | Ready | multiset PASS | 65,425 records, aggregate read from progress |
| `c1-rollback-ord-fix` | 66609723 -> 102adcc3 | Ordered | strict: 0 differing records | 60,935 records |
| `c1-rollback-ready-fix` | 66609723 -> 102adcc3 | Ready | multiset PASS | 63,157 records |

  The strict comparisons differ at top level only in
  `uncommitted_inspections` and `descendant_closure` {last_refresh_seconds,
  refresh_count, refresh_seconds, retained_storage_estimate_bytes,
  snapshot_age_seconds}, the key set of the 102adcc3 -> 102adcc3 self-test
  `harness-selftest-102`. The Ready meta sections this binary writes carry
  exactly the old top-level keys, with `records_accepted_events` inside
  `progress` (`c1-new-fix-ready/fg/checkpoint/meta-*.json`).

## Resuming the live campaign onto this binary

A CP5 checkpoint of `rustred-102adcc3` resumes under this binary with the
same `WALK_SEMANTICS_VERSION` (both policies measured above;
`checkpoint_executable_changed` is emitted), and a checkpoint this binary
wrote resumes under `rustred-102adcc3` (both policies, `c1-rollback-*-fix`),
so the executable-history rollback stays available. For a Ready campaign
the first restore streams every record segment once to derive the
accepted-events aggregate; its cost is proportional to the record bytes
(restore validation 0.33 s including the derivation for 66,096 FG records
in `c1-resume-ready-fix`, against 0.08 s without it in `c1-resume-ord-fix`;
not measured at campaign scale). The same derivation runs again after any save
by `rustred-102adcc3`. Not yet measured: the synthetic 20M-domain gate of
design 1.6 / 7.3 (scale tests are a separate track) and a W50 run of the
five-loop control.
