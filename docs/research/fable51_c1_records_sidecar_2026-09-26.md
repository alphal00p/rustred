# C1 records sidecar: implementation record and control measurements (2026-09-26)

Scope: design section 1 of
[`fable51_design_checkpoint_memory_2026-09-26.md`](fable51_design_checkpoint_memory_2026-09-26.md)
plus review finding #7 of
[`fable51_review_checkpoint_branch_2026-09-26.md`](fable51_review_checkpoint_branch_2026-09-26.md).
Branch `fable_5_1-c2-sidecar`, commit `553feb3e` on top of `fable_5_1` at
`754200a3`. `W/` = `crates/rustred-app/src/application/routed_campaign/walking/`.

## What changed

- Checkpointed walks keep no committed record in RAM.
  `W/execution/records.rs`: `RecordSink::{Memory, Sidecar}`; the sidecar
  appends each record, when it is committed, to the open `records-<G>.jsonl`
  of the generation the store reserved (file created at the first push, one
  unbuffered write per record, blake3 while writing). `Store::save` seals
  that segment (fsync, digest, directory sync) instead of writing records
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
  persisted as the optional meta key `records_accepted_events` (Ready only; an
  Ordered meta section keeps the old key set). A checkpoint without it (every
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
segment entries are unchanged; the only new persisted field is the optional
Ready meta key. `WALK_SEMANTICS_VERSION` stays 1.

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

Wall time (informational, whole command / traversal seconds, ref -> new):
FG 17.0 / 13.30 -> 16.5 / 13.04; BMW 36.0 / 30.34 -> 36.0 / 30.98; H
17.0 / 12.48 -> 16.0 / 12.14; X 40.5 / 34.16 -> 39.5 / 33.16; FG Ready
16.0 / 12.11 -> 15.5 / 11.67. The record serialization moved from the save
to the commit (one write per record); no slowdown is visible at this size.

## Resuming the live campaign onto this binary

A CP5 checkpoint of `rustred-102adcc3` resumes under this binary with the
same `WALK_SEMANTICS_VERSION` (both policies measured above;
`checkpoint_executable_changed` is emitted). For a Ready campaign the first
restore streams every record segment once to derive the accepted-events
aggregate; its cost is proportional to the record bytes (0.37 s for 66,029
FG records here; not measured at campaign scale). Not yet measured: the
synthetic 20M-domain gate of design 1.6 / 7.3 (scale tests are a separate
track).
