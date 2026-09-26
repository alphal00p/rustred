# CP5 sectioned walk checkpoints and the semantics-version resume binding (2026-09-26)

Package C, wave 1 of the [FABLE_5_1 plan](../../FABLE_5_1_five_loop_vacuum_plan.md)
(section 3.C). Scope: the semantics-version binding, the sectioned CP5 store
over today's in-memory state types, and save scheduling. Deferred to wave 2:
the streamed records sidecar, compact domains/summaries, CSR edges and the
scale tests. Nothing here changes admission, matching, routing or publication
semantics; `WALK_SEMANTICS_VERSION` stays at 1.

## Format

Flat directory, generation-suffixed (20 digits), one manifest per generation:

- `latest.json` / `previous.json`: schema 5, `format: "RUSTRED-WALK-CP5"`,
  `kind` (`bootstrap` or `state`), `generation`, `walk_semantics_version`,
  `arity`, `publication_policy` (`ordered`/`ready`), the request/policy
  binding digest (unchanged content from CP3/CP4), owner digests,
  `executable` (writer of this generation) and `executable_first`
  (bootstrap), the `sections` map and the `metadata` block read by the
  Python monitor, `cli/shards` and `cli/progress`. Read with a 16 MiB cap and
  `deny_unknown_fields` after a lenient schema/format pre-check.
- Rewritten each generation: `meta-<G>.json` (the twelve walk counters,
  `route_joint_support_masks_pruned`, queue scalar metadata, closure
  counters, in-flight details/refusals/replay (`progress`), `parallel`,
  `uncommitted`, `inputs`, `input_frontiers`, `streams`, optional counts),
  `nodes-<G>.bin` (one flag byte per node: sealed/inspected/closed),
  `ledger-<G>.bin` (bincode of the stored ledger image), `index-<G>.bin`
  (bincode of the owner buckets sorted by (phase, owner); identical state
  gives identical bytes).
- Append-only segments tiling `[0, total)`: `domains-<S>.bin` (consecutive
  bincode `Domain` records), `edges-<S>.bin` ((u32 source, u32 target) pairs
  in insertion order; the tracker's incoming lists are rebuilt on restore),
  `records-<S>.jsonl` (records committed since the previous save; wave 1
  keeps them in RAM but already writes them as segments).
- Every binary section starts with a 32-byte little-endian header: magic
  `RRW5`, tag (`DOMS`/`EDGE`/`NODE`/`LEDG`/`INDX`), arity u16, flags u16
  (bit0 ready), semantics version u32, count u64, first u64. Fixed-width
  payloads are accepted only when `bytes == 32 + count * record_size`.
- Manifest sections: `{file, bytes, blake3}` per rewritten section and
  `{total, segments: [{generation, file, first, count, bytes, blake3}]}` per
  segmented section; validation requires the segments to tile the total
  exactly, strictly increasing generations, and names of the form
  `<section>-<20 digits>.<ext>` at or below the manifest generation.
- Bootstrap generation 1 keeps `kind: "bootstrap"` with only `meta-<1>.json`
  (`{"bootstrap": true}`) and the `preparation_must_restart` metadata.

## Store

- Save: interval/force check (`effective_interval = max(interval, 20 x last
  save seconds)`, reported as `metadata.effective_interval_seconds`); refuse
  a failed prefix or an unbound owner; unchanged-state skip through
  `State::change_stamp()` (domains, published, events, closure revision,
  ledger reservation scan and transfers, records, uncommitted receipts) which emits
  `checkpoint_skipped_unchanged` and returns nothing, so the forced save right
  after a resume is free; a forced closure refresh so the persisted closed
  counts are current; all sections written inside one `rayon::scope` through
  a hashing writer (blake3 and byte count taken from the bytes as written,
  no re-read) via `write_file_atomically_with`; manifest publication keeps
  the rename+fsync and `previous.json` logic; segment-aware cleanup removes
  only `<section>-<generation>.<ext>` files below the previous generation
  that neither retained manifest references, then syncs the directory;
  nothing is removed on a failed save, and a failed generation's orphans keep
  their number to themselves. `checkpoint_saved` carries every existing
  metadata key plus `new_bytes`, `section_seconds`,
  `effective_interval_seconds`, `executable`,
  `executable_changed_since_bootstrap`, `walk_semantics_version` and `format`.
- Open/resume: schema/format/kind; owners equal; request digest and
  publication policy equal ("checkpoint request or policy differs; refusing
  to restart"); `walk_semantics_version` equal, otherwise "checkpoint walk
  semantics version differs (saved v, executable w); resume requires
  identical admission/matching/routing semantics"; a different executable
  digest is recorded and announced by `checkpoint_executable_changed`, never
  refused. Every referenced file's length and blake3 are verified in parallel
  at open, before any decoding. Restore then validates section headers,
  decodes, and runs the same consistency checks as the CP3/CP4 codec
  (publication counters, `Tracker::restore`, stream validation, the
  record-based closure cross-check, ledger validate/normalize, index
  positions), and emits `checkpoint_restored` with verify/decode/validate
  timings, counts and RSS. CP1-CP4 manifests are refused with "unsupported
  checkpoint generation; complete dependency history requires a fresh CP5
  campaign".
- `Store::open_with_identity(request, executable, semantics)` is the test
  seam behind `Store::open`.

## Semantics binding contract

`WALK_SEMANTICS_VERSION` (`walking/mod.rs`) must be bumped for any change to
admission ordering, the containment predicate or minimum-ID choice, ledger
reservation/transfer/publication rules, replay token hashing, inspection
event emission order or effects, `Ticket` encoding or subdivision, core
matching/routing semantics, or the sidecar/record schema. Transport changes
(file layout, codecs, digests, save scheduling) do not bump it.

## Tests

`checkpoint.rs`: request/policy binding (kept), round trip with ledger
aliases and restart of unfinished native work, bootstrap/generation/owner
binding/checksum rejection, changed executable accepted with the same
semantics version, changed semantics version refused with the same
executable, forced post-resume save skipped when unchanged, effective
interval stretching, every section digest verified before decoding,
segments must tile the total, CP3/CP4 manifests refused, deterministic index
bytes, cleanup of unreferenced files below the previous generation only,
failed save keeping the old authority and its orphans, and semantic (not
checksum) rejection of rewritten nodes/edges/meta/ledger/records/domains/
index sections plus header and length corruption. `manifest.rs` and
`sections.rs` unit-test name parsing, structure validation, headers,
fixed-width length checks and JSONL counts. `descendant_closure.rs` tests
`from_parts`. The closure, ready, subdivision and ready-native integration
tests run through the on-disk `Fixture`/`round_trip_state` seams.

## Measurements (four-loop FG control, CPUs 236-241, `RAYON_NUM_THREADS=1`)

Baseline: `TMP/fable51-controls/baseline-32fdec/fg` (CP3, executable
32fdec): 98,869 completed / 98,909 scheduled nodes, 169,509,549 containment
checks, whole command 18.2 s, final checkpoint `state-...3.bin` 198,117,258
bytes.

CP5 (`TMP/fable51-controls/ckpt-wave1/fg`, same command, this branch's
binary): identical 98,869 / 98,909 nodes, 169,509,549 containment checks,
149,787,346 maintenance checks, 2,083,888 deduplication hits; whole command
17.0 s (traversal 13.76 s vs 13.71 s), peak RSS 1.180 GB (vs 1.181 GB).
Final checkpoint (generation 3) 142,711,580 bytes over twelve files, 28%
below the CP3 image: `records-3.jsonl` 132,118,075 (92.6%),
`domains` 4,880,277 in two segments (`[0,248)` from generation 2 and
`[248,98909)`), `edges-3.bin` 4,323,208 for 540,397 edges (8 bytes each),
`ledger` 692,597, `index` 581,268, `nodes` 98,941, `meta` 17,214. Final
save 0.716 s (records 0.657, domains 0.043, edges 0.006, index 0.005,
ledger 0.004, nodes and meta below 1 ms) against 1.361 s for CP3; the
post-admission generation 2 (248 domains) is 41,230 bytes in 5 ms (CP3:
226,368 bytes). The records section is the remaining cost, which is what
the wave-2 sidecar removes from the save path.

Stop/resume equivalence (`TMP/fable51-controls/ckpt-wave1-resume3`,
`--checkpoint-interval-seconds 5`, stop file written at 6.0 s): the first
process saved generations 2 (admission), 3 (periodic) and 4 (cooperative
stop, 20,735,142 bytes: 19,076 domains, 81,952 edges, 14,075 records and
the receipts of the eight inspections the stop cancelled) and exited with
status 4 at 6.73 s. `--resume` verified and restored generation 4 in
0.238 s (`checkpoint_restored`: verify 0.007 s, decode 0.207 s, validate
0.031 s, RSS 162 MB), skipped the forced post-resume save
(`checkpoint_skipped_unchanged`, no new generation), saved generation 5
periodically and generation 6 at the end (67,074,965 new bytes in 0.372 s;
142,716,646 bytes total) and exited 0 after 11.75 s. Generation 2/3 small
sections were cleaned; the domains/edges/records segments of generations
2-6 tile their totals. The resumed `result.json` matches the uninterrupted
control record for record (98,909 `domains` rows compared after sorting by
id) and in every counter (completed 98,869, scheduled 98,909, containment
checks 169,509,549, events 2,488,138, successors 2,182,549, status
`locally_resolved`, `all_scheduled_domains_resolved` true). The only
differing keys are `uncommitted_inspections` (0 vs 8: the persisted
receipts of the cancelled inspections, which the resumed run re-inspected
and reports as diagnostics) and the allocator-capacity estimate
`descendant_closure.retained_storage_estimate_bytes`; neither is walk state.
An earlier run before the stamp covered `uncommitted` had skipped the
cooperative-stop save as unchanged, which is why that field is part of the
stamp. Ordered policy only; the Ready equivalence run is a wave-2 scale
test.

## Deferred to wave 2

Records sidecar (`RecordSink`, streaming `result.json`), compact
`Domain`/`DomainPowerSummary` storage, CSR edges with heads rebuilt on
restore, the ledger/closure cross-check replacing the record scan, 16-byte
ledger entries, mmap/streamed section decoding (wave 1 verifies by streaming
and then reads each section into memory one at a time), and the scale tests
(timed restore of a copied production checkpoint, FG interrupted-vs-
uninterrupted for Ready, synthetic 20M-domain save/restore benchmark).
