# Restore-at-scale: production CP5 generation 3 under the merged code (fable_5_1, C §7.1)

Status: implemented and measured on branch `fable_5_1-scale-restore` (from the
integration head `971bf34c`). Design: `fable51_design_checkpoint_memory_2026-09-26.md`
§7.1. Two measured runs on a shared host, pinned to CPUs 250-255 with `nice -n 5`.
Timings are informational. Nothing here is a closure, termination or ETA claim.

## What the test does

`walking/checkpoint/scale_tests.rs`, `restore_copied_production_checkpoint`
(`#[ignore]`, `cfg(all(test, feature = "cli"))`), driven by environment variables:

| Variable | Meaning |
|---|---|
| `RUSTRED_CHECKPOINT_RESTORE_DIRECTORY` | a copy of a CP5 `checkpoints/main` directory (never the live one) |
| `RUSTRED_CHECKPOINT_RESTORE_REQUEST` | the campaign run's `request.json` (its frozen native `command` argv), read only |
| `RUSTRED_CHECKPOINT_RESTORE_RECEIPT` | the JSON receipt to create (`create_new`; defaults to `TMP/checkpoint-restore-receipt-<unix>.json`) |
| `RUSTRED_CHECKPOINT_RESTORE_SAVER_EXECUTABLE` | optional: the binary that wrote the checkpoint; its blake3 is compared with the manifest's `executable` |

Steps, all through production code:

1. The campaign argv is turned into its resume the way the supervisors do it:
   `--checkpoint <dir>` becomes `--resume <copy>`. `--output`, `--events` and
   `--stop-file` move to scratch paths that are never created. They are transport
   and not part of the request binding. Every other option stays verbatim.
2. `cli::walk_request_from_argv` runs the command's own parser
   (`parse_args`), `validate_query_allowances`, `preflight_checkpoint_paths` and
   request construction. This commit extracts `match_request`/`walk_request` from
   `run_admitted`; the command runs the same functions, so its behavior is unchanged.
   `walking::admit_request` then runs the walk's request admission. It was
   extracted from `owner_domain_walk_with_progress` in the same order. The host
   core-budget preflight (100 workers under a 6-CPU affinity) and the CLI
   inner-pool environment preflight are not applied, because they concern the
   host, not the checkpoint.
3. `Store::open` checks the request binding, the policy, the ledger presence,
   `WALK_SEMANTICS_VERSION` and the blake3 digest and length of every referenced
   section, and emits `checkpoint_executable_changed`. `Store::resume` then
   decodes and validates every section. The owner payload digests are bound
   through `prepare_with_fingerprints` → `Store::bind_owners`, exactly as a
   resume does, and the preparation is cancelled right after binding, before the
   native owner import. `Store::attach_records` checks the sidecar ownership.
   No walk step, no save and no bootstrap run.
4. The receipt records per-phase timings, `VmRSS`/`VmHWM`/`VmPeak` at each
   stage, the restored counts compared with the manifest, the executable
   identities and the directory listing (name, length, inode, mtime and ctime
   in ns) before and after. The test fails on any restore error, on any
   count/manifest disagreement or on any listing change.

The per-phase timings come from a new `phase_seconds` object in the
`checkpoint_restored` report (`checkpoint/restore.rs` `Phases`, threaded through
`Queue::restore_from_parts`). The change only adds a key to the report: no
persisted byte, counter, record or edge changes, and the existing
`verify/decode/validate/closure_seconds` keys are unchanged.

## Input and binaries

- Checkpoint: generation 3 of `campaigns/five-loop-qcd-feynman-d9d10-v2`
  (saved 2026-09-26T19:17:54Z, `saved_unix_time` 1790450274, 16,706,223,886 bytes),
  from the retained copy `TMP/v2-checkpoint-copy-gen3`, block-cloned with
  `cp -r` into `TMP/scale-restore-gen3.MKIZ1Y/checkpoint` in 1.4 s.
- Writer: the manifest `executable` blake3 `4cb4ab28…5e55` equals the blake3 of
  `TMP/fable51-controls/bin/rustred-102adcc3` (sha256 `102adcc3…c1f2ae`; the
  receipt checks `saver_matches_saved: true`).
- Under test: `target/release/deps/rustred_app-e6a2c23c0acbce37` (sha256
  `17733523514edd237d18bb6f5d6506890c0ca986afc0ec3d6e7f5e779c8b8a6e`, blake3
  `6e0aecf7…92d6`), release profile, this branch's working tree. The worktree's
  `vendor/symbolica` carries the pre-existing uncommitted `src/poly/polynomial.rs`
  change that the main checkout also carries.
- Environment: `RAYON_NUM_THREADS=1` and the other single-thread pool variables
  that the production CLI requires, so the parallel verify and derivation passes
  run on one thread as they would in the campaign process.

## Result: PASS, both runs

Receipts: `TMP/scale-restore-gen3.MKIZ1Y/receipt-run1.json` and `receipt-run2.json`.

| | run 1 | run 2 |
|---|---:|---:|
| total (request build → drop) | 223.8 s | 208.8 s |
| `Store::open` (includes digest verification) | 42.6 s | 40.2 s |
| digest verification (all sections, 16.7 GB) | 41.6 s | 39.2 s |
| `Store::resume` | 176.5 s | 164.0 s |
| Ready accepted-events derivation (10.5 GB records) | 53.0 s | 43.9 s |
| ledger/closure cross-check | 35.3 s | 33.4 s |
| domains decode (2.0 GB) | 21.7 s | 21.4 s |
| edges decode (3.7 GB) | 20.8 s | 20.5 s |
| domains exact index | 15.7 s | 15.3 s |
| index summaries | 14.8 s | 15.0 s |
| closure CSR build / validate | 6.6 / 2.5 s | 6.6 / 2.1 s |
| index decode / owner buckets | 2.1 / 0.2 s | 2.0 / 0.2 s |
| ledger decode / restore | 1.9 / 1.7 s | 1.9 / 1.7 s |
| nodes, meta, progress+streams | < 0.1 s | < 0.1 s |
| owner digest binding (67 owners) | 3.0 s | 2.9 s |
| VmRSS after restore | 11.008 GB | 11.008 GB |
| VmHWM (peak, restore included) | 18.519 GB | 18.521 GB |
| VmRSS after drop | 1.093 GB | 1.093 GB |
| filesystem reads (`time -v`, 512 B blocks) | 6,333,616 (3.2 GB) | 0 |

Both runs were served mostly or entirely from the ZFS ARC. A cold-disk restore
was not measured.

Restored counts equal the manifest for every compared field: 28,799,586 domains
(and ledger entries), 468,090,672 dependency edges, 16,882,169 records and
committed domains, 11,917,417 pending, watermark 16,881,859, 9,344,009
completed native inspections, 2,229,650,531 committed events. There are
14,713,699 live candidates (live summaries). The dependency monitor is available
with 2,324,860 closed. The derived `records_accepted_events` is 2,229,633,662.
The 16,869 events in unfinished accepted prefixes make up the committed total,
and the stream validator checks this sum.

Executable signal: `checkpoint_executable_changed` was emitted (saved
`4cb4ab28…`, current = the test executable's `6e0aecf7…`,
`walk_semantics_version` 1), and the report says
`executable_changed_since_bootstrap: true`. The executable change is recorded,
not refused, as designed.

Directory unchanged: the in-test listing and an external `stat` listing
(`listing-before.txt` vs `listing-after-run{1,2}.txt`) are identical. The
sha256 of every file under 1 MiB (`latest.json`, `previous.json`, meta, lock,
generation-2 domains) is unchanged. The scratch CLI output directory was never
created.

## Comparison with the live campaign (not like-for-like)

This comparison is labelled because the two numbers measure different things.
The live process (`102adcc3`, run `20260926T151353.794886Z`) reported
`process_rss_bytes` = 142,840,217,600 (142.8 GB) in its last heartbeat before
generation 3's save started (events line 14340). It reported 142.73 GB at most
during the save and 138.0–144.9 GB within ±15 min of `saved_unix_time`
(`TMP/scale-restore-gen3.MKIZ1Y/live-campaign-rss-gen3.json`, read-only from
`events.jsonl`). The merged code's restored state of the same generation is
11.0 GB resident (18.5 GB peak during restore). The restored figure excludes the
native owner programs, inspector and admission-helper buffers, completed-escrow
and replay buffers, and all other transient walk state. This measurement does
not split the live 142.8 GB into these parts, so the difference is not a
predicted campaign RSS for a resumed run.

## Observations (single host, informational)

- The accepted-events derivation and the ledger/closure cross-check together
  take about half of the 164–177 s resume (47–50%). The domain decode, the exact
  index and the summary rebuild take about another 30%. All of them ran single
  threaded here. The derivation runs because the gen-3 meta written by
  `102adcc3` carries no `records_accepted_events` aggregate. The merged code
  writes the aggregate into the Ready progress metadata at every save
  (`State::checkpoint_progress_metadata`), so a resume of a checkpoint that the
  merged code saved reads the aggregate instead of deriving it.
- Later generations (the live campaign had published generation 4 by the time
  of this test) are larger and were not measured.
