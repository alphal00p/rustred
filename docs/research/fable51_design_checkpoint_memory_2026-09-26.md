# Design: CP5 checkpoint / memory redesign (fable_5_1, 2026-09-26)

File-level implementation plan produced by a planning agent for section 3.C of `FABLE_5_1_five_loop_vacuum_plan.md`. Items 4 (sectioned CP5 store with today's types), 5 (semantics binding) and 6 (save scheduling) are implemented on branch `fable_5_1-ckpt`; items 1 (records sidecar), 2 (compact in-RAM state), 3 (CSR edges) and 7 (scale tests) remain. Line numbers refer to the `fable_5_1` base at the time of planning.

# Plan: CP5 checkpoint / memory redesign of the saved-owner walk

Module root `W/` = `/common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/`. All numbers below marked "est." are derived from the measured facts in the brief plus struct layouts I verified in code; they are targets to be confirmed by the measurement gates, not claims.

## 0. Design summary and RAM/disk budget

Current per-discovered-domain RAM (verified layouts): `Arc<Domain<15>>` ~500 B (3 heap allocations: Arc block + 2 Vecs), `DomainPowerSummary<15>` ~560 B retained for every id, ledger `Entry` 48 B, exact-map ~20 B, closure `Node` 16 B, `Edge` 24 B × ~19.5 edges/domain (746M/38.3M) ≈ 470 B, index ~24 B/live id. Plus ~6 KB per committed record (write-only `serde_json::Value`).

| Component | Now (B / discovered) | Target (B / discovered) | Item |
|---|---:|---:|---|
| Record `Value` (per committed) | ~6,000 (×0.35) | 0 (sidecar) | 1 |
| Domain | ~500 | 96 (inline `CompactDomain<15>`) | 2 |
| Summary | ~560 (all ids) | 176 × live-fraction (~0.68) ≈ 120 + 4 slot | 2 |
| Exact map | ~20 | ~28 (`u128` key → `u32`) | 2 |
| Ledger entry | 48 | 16 | 2 (optional) |
| Closure node | 16 | 1 (flags) + 8 (CSR offset) | 3 |
| Edges | 24 × 19.5 = 468 | 4 × 19.5 + log ≈ 90 | 3 |
| Index | ~24 | ~24 (unchanged) | – |
| **Total** | **~1,160 + 2,100 (records)** | **~380–420** | |

At 38.3M domains: est. ~16 GB of walk state instead of the measured 168 GB RSS. Checkpoint per generation becomes O(new state): rewritten sections ~1.3 GB (nodes 38 MB, ledger 0.6 GB, index ~0.6 GB, meta) + new segments (~0.5 GB per 4 h) ≈ 2 GB; target 10–60 s including fsync and digests, versus 496 s at gen 18.

Interfaces this plan needs from the other packages are collected in §8.

---

## 1. Records sidecar

**Goal.** Never hold a committed record in RAM. Stream each record at commit time to an append-only JSONL segment; seal one segment per checkpoint generation; keep only aggregates in RAM; make `result.json` a streaming post-pass.

**Why nothing per-id is needed in RAM (verified).** Every consumer of `state.records` is replaceable:
- `codec.rs:278-359 validate_closure_records` cross-checks record kind/flags against `Tracker::local_status(id)` and the ledger; both sides are already in RAM (ledger `Responsibility` + `initial_anchor` + `delegated_published`; node `inspected`/`sealed`). The check becomes a ledger↔closure invariant (§1.4) with no record scan.
- `streams.rs:231-263` needs `sum(accepted_events)` over published native records: replace with a running counter `State::records_accepted_events`.
- `delegation.rs:125-165 finalize_delegation` and `mod.rs:638-642` annotate records from `ledger.resolve().by_id` and `closure.closed(id)`: both are id-indexed and computable without records; apply them during the streaming post-pass.

### 1.1 Files / structs / functions

New `W/execution/records.rs`:

```rust
pub(in super::super) enum RecordSink {
    Memory(Vec<Value>),           // non-checkpointed runs and unit tests
    Sidecar(SidecarWriter),       // checkpointed runs
}
pub(in super::super) struct SegmentMeta { generation: u64, file: String, first: u64, count: u64, bytes: u64, blake3: [u8; 32] }
pub(in super::super) struct SidecarWriter {
    directory: PathBuf,
    closed: Vec<SegmentMeta>,               // referenced by the last published manifest
    open: Option<OpenSegment>,              // records since the last save
    total: u64,                             // == closed counts + open.count
}
struct OpenSegment { generation: u64, path: PathBuf, writer: HashingWriter<BufWriter<File>>, first: u64, count: u64 }
impl RecordSink {
    fn push(&mut self, record: &Value) -> Result<(), &'static str>;   // serde_json::to_writer + b"\n"
    fn total(&self) -> u64;
    fn seal(&mut self, next_generation: u64) -> Result<SegmentMeta, String>; // flush, sync_all, finalize digest, open next (create_new)
    fn iter(&self) -> RecordIter;          // closed segments in order, then flushed open segment (BufReader lines, bounded line length)
    #[cfg(test)] fn snapshot(&self) -> Vec<Value>;  // test-facing accessor (Memory: clone; Sidecar: flush + read back)
}
```

`HashingWriter<W>` (blake3 + byte count while writing) lives in `W/checkpoint/sections.rs` (shared with §4).

Changes in `W/execution.rs`:
- `State.records: Vec<Value>` → `records: RefCell<RecordSink>` (RefCell mirrors `closure: RefCell<Tracker>`; `save(&State)` must seal the open segment without changing the `maybe_save(&State)` contract owned by the scheduler package). Add `records_accepted_events: usize` (Ready) and keep `native_records`.
- `commit_result` (`:591-663`): build `record` exactly as today, then `if let Err(e) = self.records.borrow_mut().push(&record) { self.error.get_or_insert_with(|| e.into()); }`; under Ready add `self.records_accepted_events += replay.accepted_events()` (checked). Then drop `record`.
- `W/execution/delegation.rs:54-74`: same `push` pattern for delegated records (`try_reserve` becomes the push result).
- `finalize_delegation` (`delegation.rs:109-184`): stop mutating records; return `(summary_json, FinalAnnotations)` where `FinalAnnotations { by_id: Vec<Resolution> }` (24 B × ids, transient ~0.9 GB at 38M; acceptable at finalization only) plus a `fn annotate(&self, record: &mut Value, closure: &Tracker)` that applies `final_representative_id`, `responsibility_status`, `local_classification_discharged`, `descendant_closed` with the exact strings used today.
- `mod.rs:637-642, 675`: replace the in-RAM loop and `take_report_array(&mut state.records)` with `OwnerDomainWalkRecords::Streamed { sink, annotations }`.

`W/mod.rs`:
```rust
pub struct OwnerDomainWalkResult {
    pub all_scheduled_domains_resolved: bool,
    pub document: Value,                 // no "domains" key when records are streamed
    pub records: OwnerDomainWalkRecords, // Inline (already in document) | Streamed
}
impl OwnerDomainWalkResult {
    /// Serialize `document` with "domains" streamed one record at a time (serde_json::Serializer::pretty + SerializeMap).
    pub fn write_json(&self, out: impl Write) -> Result<(), String>;
    pub fn into_document(self) -> Result<Value, String>;  // materializes (small runs, tests)
}
```
`crates/rustred-app/src/cli/owner_match.rs:181-186`: the CLI mutates `document` after the walk, then calls `result.write_json(BufWriter)` inside `write_file_atomically_with`. Peak RAM for result.json = one record. `tests/cli_routed_campaign.rs:649` (`report["domains"] == []`) is the only external consumer of `domains` and still holds (empty array is written).

### 1.2 On-disk layout

`records-<G:020>.jsonl` in the checkpoint directory; one `serde_json::Value` per line, publication order, no compression (no zstd dependency; note as a follow-up). Segment `G` holds records committed after the save of generation `G-1` and before the save of `G`. Manifest lists `sections.records.segments[] = {generation, file, first, count, bytes, blake3}` and `sections.records.total`. The open segment for the next generation is created (`create_new`) at `Store::open`/after each seal; it is not referenced by any manifest until sealed.

### 1.3 Crash consistency

- A crash or failed save leaves an unreferenced `records-<G>.jsonl`. On resume, the sidecar is reconstructed from the manifest's segment list only; the orphan is ignored, and removed by the segment-aware cleanup (§4.5) once `previous.generation` exceeds it. No truncation is ever needed.
- Sidecar write failure sets `state.error` (like ledger errors); `save()` already refuses a failed prefix, so a manifest can never reference a segment whose count disagrees with `published_count()`.
- Records for failed publishers (retained on error) go to the open segment and are only ever consumed by the in-process final `result.json`, never by a manifest.

### 1.4 Invariants and validation (replaces `validate_closure_records`)

New `W/checkpoint/restore.rs::validate_ledger_closure(state)`; for each id:
- No ledger (InspectAll): `id < queue.next` ⇒ `local_status == (true, _)`; else `(false,false)`. Aggregate: `count(inspected && !sealed) <= state.frontiers`.
- `Delegate{to}` & `delegated_published` ⇒ `(false,true)` and required edge `(id,to)`; not published ⇒ `(false,false)`.
- `Local(Published(Completed{f}))` ⇒ `(true, f==0)`; `initial_anchor=Some(a)` ⇒ required edge `(id,a)` and `a < initial_domain_count`.
- Any other `Local` ⇒ `(false,false)`.
- Required edges are collected into a `HashSet<(u32,u32)>` (transient, ~200 MB at 12M aliases) and removed while scanning the edge log; leftover ⇒ error "dependency alias or partial-anchor edge missing" (same wording as today).
- Sidecar aggregates: `records.total == published_count()`; `native_records == ledger.native_publications()` (ledger present) or `== queue.next` (Ordered/InspectAll); Ready: `records_accepted_events + active/parked replay accepted == events` (replaces the record scan in `validate_restored_streams`).

### 1.5 Tests

- `records_sidecar_seals_per_generation_and_ignores_unreferenced_tail` (`W/execution/records.rs`): push 5, seal(2), push 3, drop writer without sealing; reopen from manifest listing only segment 2 → `iter()` yields 5; orphan file present and later removed by cleanup.
- `sidecar_write_failure_sets_publisher_error_not_a_fake_record` (read-only directory): `commit_result` sets `state.error`; `save` refuses.
- `streaming_result_json_equals_materialized_document` (`W/mod.rs` policy_tests): run the `native_fixture` walk with checkpointing; `write_json` output parsed == `into_document()` of an identical non-checkpointed run (minus `seconds`, `checkpoint`).
- `finalization_annotations_from_ledger_and_closure_match_previous_record_mutation`: reuse the fixtures of `execution/delegation/tests.rs:54-85` and `delegation/tests/initial_overlap.rs`; assert the same annotated keys/values via `records.snapshot()`.
- `ledger_closure_cross_check_rejects_fictional_seal_and_missing_alias_edge`: port `closure_tests.rs:242` to the new validator (corrupt nodes/ledger via §4.7 helpers).
- Test migration: replace `state.records[i]` with `state.records.borrow().snapshot()[i]` in `execution/tests.rs:263,404`, `execution/delegation/tests.rs:60-85,232,333`, `delegation/tests/initial_overlap.rs`, `subdivision_tests.rs:114-655`, `ready_native_tests.rs:109`, `initial_orthants_tests.rs:219`, `admission/grain_tests.rs:136`.

### 1.6 Measurement gate

On the synthetic 20M-domain state (§7.3) with 7M records: RSS delta from committing records ≤ 1 MB/1M records (buffer only); `result.json` write of 7M records with peak RSS growth < 50 MB. On the FG control: identical `domains` array vs the pre-change binary (modulo `seconds`).

**Effort:** 3–4 days.

---

## 2. Compact in-RAM state

**Goal.** ≤ 0.8 KB per discovered domain total; here ≤ ~250 B for domain+summary+exact+ledger.

### 2.1 `CompactDomain<N>` (`W/queue/compact.rs`, new)

```rust
#[repr(C)] #[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct CompactDomain<const N: usize> {
    phase: u8, flags: u8 /* bit0 rank none, bit1-3 powers none */, owner: u16 /* bitmask, N<=16 */,
    rank: u32, lower: [u16; N], upper: [u16; N] /* 0xFFFF = +inf */,
    max_positive_power: u64, min_power_difference: i64, max_power_difference: i64,
}
pub(super) const MAX_COMPACT_COORDINATE: u64 = u16::MAX as u64 - 1;
impl CompactDomain<N> { fn try_from_domain(&Domain<N>) -> Result<Self, &'static str>; fn expand(&self) -> Domain<N>; fn key128(&self) -> u128 /* blake3 of canonical bytes, first 16 B */; accessors phase(), owner() -> [bool;N], rank(), powers(), lower(axis), upper(axis), is_full_orthant() }
```
Size 32 + 4N → 92 B (N=15), rounded 96. `Domain<N>` (heap Vecs) remains the event/inspection transport type (`Effect::Admit`, `inspection::inspect(&Domain<N>)`, `physical_parts::parts`, `reuse.rs`, `initial_orthants.rs`, `initial_overlap.rs`); the queue converts at admission and expands at dispatch.

`W/queue.rs` changes:
- `domains: Vec<Arc<Domain<N>>>` → `domains: Vec<CompactDomain<N>>`; add `fn domain(&self, id) -> Domain<N>` (expand), `fn domain_arc(&self, id) -> Arc<Domain<N>>` (for `parallel::Pool::dispatch`, `State::parts`), `fn expand_prefix(&self, n) -> Vec<Arc<Domain<N>>>` (initial prefix for `InitialOrthants::from_initial` / `InitialOverlapIndex::from_initial`, bounded by `MAX_INITIAL_DOMAINS`).
- `exact: HashMap<Arc<Domain<N>>, usize>` → `HashMap<u128, u32>` keyed by `key128` (collision probability ~1e-24 at 1e8 entries; semantic-neutral since the compact encoding is injective on the representable range and `Domain: Eq` compares exactly the encoded fields). Admission converts first; conversion failure (`coordinate > 65534`) is a new explicit refusal `"domain coordinate exceeds compact range"` — the only semantic change, unreachable for A24/R15/D9 inputs (confirm with the physics package, §8).
- Call sites to adapt (all read fields through accessors or `domain(id)`): `execution.rs:74,80,182,255-260,513,591,685,1028,1109,1133,1408,1428,1538,1583`, `execution/delegation.rs:53`, `execution/streams.rs`, `retain_leftovers`, `physical_receipt`, `queue/checkpoint.rs` (rewritten in §4), `positive_reuse_trace` (test), tests constructing `Domain`.

### 2.2 `CompactSummary<N>` and retired-slot reclamation

```rust
#[repr(C)] pub(super) struct CompactSummary<const N: usize> {
    flags: u8 /* bit0 empty; bits for each None aggregate */, lower: [u32; N], upper: [u32; N] /* u32::MAX = +inf */,
    positive_lower: u64, positive_upper: u64, numerator_lower: u64, numerator_upper: u64, difference_lower: i64, difference_upper: i64,
}
impl CompactSummary<N> { fn from_core(&DomainPowerSummary<N>) -> Result<Self,&'static str>; fn contains(&self, other: &Self) -> bool; fn signature(&self) -> Signature; fn coordinates(&self) -> Option<Coordinates<'_>> }
```
176 B at N=15. `contains` replicates `summary.rs:84-104` on compact fields; `Signature::of` / `Coordinates::of` get compact overloads (`Coordinates` currently borrows `&[u64]`; change to `&[u32]` for both stored and query sides, converting the query summary once per admission). Aggregates fit `u64` because coordinates ≤ 65534 and N ≤ 16 (conversion asserts; failure is an explicit error).

Storage: `summaries: Vec<CompactSummary<N>>` slab + `slot: Vec<u32>` (id → slot, `u32::MAX` = retired) + `free: Vec<u32>`. In `admit_with_lookup` the `retire` closure records retired ids into a small Vec; after `bucket.indexed.retire(...)` returns, their slots are released. `PreparedLookup::revalidate` (`prepared.rs:169-175`) must check `slot[id] != RETIRED` before `is_live` (a reused slot must not be read for a retired id); a retired `found` falls back to the fresh scan exactly as an `is_live == false` does today — semantic-neutral. `containment_candidate_count` unchanged.

Semantic-neutral: yes (same predicate on the same values), guarded by the differential test below.

### 2.3 Ledger entry (optional, 48 → 16 B)

`delegation/ledger.rs:23-30 Entry<K>`: drop `key` (re-derive `(phase, owner)` from `queue.domains[id]` via a `KeyLookup` closure passed to `transfer_retired`/`record_initial_overlap`/`validate_checkpoint`/`resolve`), encode `Responsibility` as `tag: u8 + u32 payload`, `initial_anchor: u32`. Keeps the same public methods. Defer if schedule is tight (saves ~1.2 GB at 38M).

### 2.4 Tests

- `compact_domain_round_trips_every_field_and_rejects_out_of_range` (`W/queue/compact.rs`): property over random boxes incl. `None` upper/rank/powers; `expand(try_from(d)) == d`; `key128` equal iff `Domain` equal.
- `compact_summary_contains_matches_core_on_random_boxes`: 100k random pairs (N=1..16): `CompactSummary::contains == DomainPowerSummary::contains`; `signature()`/`coordinates()` agree with `Signature::of`/`Coordinates::of`.
- `retired_summary_slots_are_released_and_never_read_by_revalidate` (`W/queue/tests/maximal_candidates.rs`): admit contained→container→prepare a lookup whose `found` is retired before commit; assert same admission result as serial and `summaries.len()` shrinks/reuses.
- `queue_admission_sequence_is_identical_under_compact_storage`: run the existing `queue/tests/replay.rs` sequences and assert identical `(id, admitted)` results and counters vs the recorded expectations (these tests exist; they become the regression net).
- Storage accounting: extend `positive_reuse_trace` storage estimate and add a `#[test] fn compact_state_size_of_is_within_budget()` asserting `size_of::<CompactDomain<15>>() <= 96`, `size_of::<CompactSummary<15>>() <= 176`.

### 2.5 Measurement gate

Synthetic 20M domains (§7.3): RSS ≤ 0.45 KB × discovered for queue+ledger+closure(nodes) before edges; live run comparison via the heartbeat `process_rss_bytes` slope vs `scheduled_nodes`.

**Effort:** 4–5 days (many call sites; the differential tests are the safety net).

---

## 3. Dependency edges: u32 CSR-by-target + per-generation append log

**Goal.** 24 B → ~4–5 B per edge in RAM; binary append-only segments on disk; heads rebuilt on restore.

`refresh` (`descendant_closure.rs:177-239`) needs incoming lists by target (reverse reachability from unsealed nodes). Insertion order is only needed for the on-disk log and dedup.

### 3.1 Structures (`W/descendant_closure.rs`, plus new `W/descendant_closure/edges.rs`)

```rust
struct Csr { offsets: Vec<u64> /* nodes+1 */, sources: Vec<u32> }         // frozen edges, incoming by target
struct EdgeLog { pairs: Vec<(u32,u32)>, heads: Vec<u32>, next: Vec<u32> } // edges since last fold, linked by target (12 B/edge, bounded)
pub(super) struct Tracker { flags: Vec<u8> /* bit0 sealed, bit1 inspected, bit2 closed */, csr: Csr, log: EdgeLog, persisted_log: usize /* pairs already in a sealed segment */, initial, unavailable, revision, snapshot_revision, initial_closed, total_closed, inspected, refresh_count, refresh_seconds, open_targets: HashMap<u32, HashSet<u32>>, ... }
```
- `edge(source,target)`: dedup via `open_targets` (unsealed sources only, tiny), push to `log`.
- `refresh`: per popped node `id`, iterate `csr.sources[offsets[id]..offsets[id+1]]` then the log chain from `log.heads[id]`. Sequential CSR access makes the 29.6 s refresh substantially cheaper (est. 3–8 s at 746M edges; gate below).
- `fold()`: counting-sort merge of `csr` + `log` into a new `Csr` (O(E), peak 2× sources = est. 6 GB at 746M); called from `save()` after the log segment is persisted when `log.len() > csr.len()/16 || log.len() > 64M`. Amortized O(1)/edge.
- `edge_segment(since: usize) -> &[(u32,u32)]` for the section writer; `restore(total, initial, pairs)` rebuilds `open_targets` for unsealed sources (duplicate ⇒ error, as today), builds the CSR, validates counters exactly as `restore()` does today minus the `next == heads[target]` link check (replaced by the segment digests + endpoint range check).
- `edges.len()` = `csr.sources.len() + log.pairs.len()` for `json()`; `storage_estimate_bytes` updated.

### 3.2 On-disk layout

`edges-<G:020>.bin`: 32-byte section header (§4.2) with `count`, `first` (global edge ordinal), then `count × (u32 source, u32 target)` little-endian, insertion order. Node flags: `nodes-<G>.bin` = header + `total` bytes (rewritten each generation, 38 MB). Closure counters (`initial`, `revision`, `snapshot_revision`, `initial_closed`, `total_closed`, `inspected`, `refresh_count`, `refresh_seconds`, `unavailable`) go to `meta-<G>.json`.

### 3.3 Tests

- Port `descendant_closure.rs:430 interrupted_prefix_roundtrip_keeps_edges_and_deduplicates_replay` and `descendant_closure_tests.rs:109 checkpoint_rejects_corrupt_links_closed_frontier_and_counts` to binary sections using the §4.7 corruption helpers (out-of-range endpoint, duplicate edge from an unsealed source, closed-but-unsealed flag, wrong `total_closed`).
- `csr_refresh_matches_linked_list_reference_after_folds` (`descendant_closure_tests.rs`): random graphs, fold at random points, compare `closed` against `reference_closed` (existing function at `:4`).
- `edge_segments_are_disjoint_prefix_partition_of_the_log`: three saves; segment `first/count` tile `[0, E)`.

### 3.4 Measurement gate

Synthetic 20M nodes / 400M edges: RAM for edges ≤ 5 B/edge after fold (measure Vec capacities via `storage_estimate_bytes`), refresh ≤ 1/3 of the linked-list time on the same graph; restore rebuild ≤ 10 s.

**Effort:** 2–3 days.

---

## 4. CP5 checkpoint format: sectioned, digested, segmented/incremental

### 4.1 Directory layout (flat, generation-suffixed like today's `state-<20 digits>.bin`)

```
checkpoint.lock
latest.json, previous.json                 # Manifest schema 5 (read with a 16 MiB cap; deny_unknown_fields)
meta-<G>.json                               # rewritten: counters, queue metadata, progress, parallel, uncommitted,
                                            #   inputs, input_frontiers, streams (contexts incl. in-flight details/refusals/replay),
                                            #   refusals, optional counts, closure counters, sidecar aggregates
nodes-<G>.bin                               # rewritten: u8 flags per closure node
ledger-<G>.bin                              # rewritten: 16 B per entry (absent under InspectAll)
index-<G>.bin                               # rewritten: owner buckets (bincode 2 Encode/Decode, with_limit)
domains-<S>.bin                             # append segment: CompactDomain records [first, first+count)
edges-<S>.bin                               # append segment: (u32,u32) pairs
records-<S>.jsonl                           # append segment: sidecar (§1)
```
Bootstrap generation 1 keeps `kind: "bootstrap"` with only `meta-<1>.json` (`{"bootstrap":true}`), preserving `preparation_must_restart` semantics and the supervisor's `latest.json` existence check (`cli/shards/supervisor.rs:499-509`).

### 4.2 Binary section header (32 B, little-endian, hand-rolled in `W/checkpoint/sections.rs`)

`magic "RRW5"`, `tag [u8;4]` (`DOMS`,`EDGE`,`NODE`,`LEDG`,`INDX`), `arity u16`, `flags u16` (bit0 ready), `walk_semantics_version u32`, `count u64`, `first u64`. Fixed-width payloads (domains, edges, nodes, ledger) are validated by `bytes == 32 + count × record_size` before decoding; the index payload is bincode with `with_limit::<MAX_INDEX_BYTES>` (bincode 2 native derive is already the repo pattern at `candidate_bundle/codec.rs`; the serde feature is not enabled and is not needed).

Ledger entry (16 B): `tag u8` (Unreserved/Reserved/Started/PublishedCompleted/PublishedFailed/PublishedCancelled/Delegate), `flags u8` (bit0 delegated_published), `pad u16`, `payload u32` (Delegate `to` / `unresolved_frontiers`), `initial_anchor u32` (0 = none, else anchor+1), `pad u32`. Keys re-zipped from domains on restore as today (`ledger/checkpoint.rs:89-127`).

Index (`queue/index.rs` + `blocks.rs`): `Block` serializes only `len` ids (not all 32 slots) and its envelope; `Group{signature, blocks, live}`; `OwnerBucket{phase, owner bitmask, ids, orthant, groups}`; buckets sorted by `(phase, owner)` so the bytes are deterministic (today's HashMap iteration order is not).

### 4.3 Manifest (`latest.json`, schema 5)

```json
{"schema":5,"format":"RUSTRED-WALK-CP5","kind":"state","generation":18,
 "walk_semantics_version":1,"arity":15,"publication_policy":"ready",
 "request":"<blake3 of binding()>","owners":["<blake3>",...],
 "executable":"<blake3 of saving exe>","executable_first":"<blake3 at bootstrap>",
 "sections":{
   "meta":{"file":"meta-00000000000000000018.json","bytes":..,"blake3":".."},
   "nodes":{"file":..,"bytes":..,"blake3":..,"count":38300000},
   "ledger":{"file":..,"bytes":..,"blake3":..,"count":38300000},
   "index":{"file":..,"bytes":..,"blake3":..,"live":25900000},
   "domains":{"total":38300000,"segments":[{"generation":2,"file":"domains-00000000000000000002.bin","first":0,"count":134,"bytes":..,"blake3":".."},...]},
   "edges":{"total":746000000,"segments":[...]},
   "records":{"total":13450000,"segments":[...]}},
 "metadata":{"state":"saved","directory":..,"generation":18,"state_path":"<dir>/meta-...json",
   "started_unix_time":..,"saved_unix_time":..,"save_seconds":..,"duration_seconds":..,
   "committed_domains":..,"pending_domains":..,"contiguous_publication_watermark":..,
   "completed_native_inspections":..,"committed_events":..,"paused":false,"bootstrap":false,
   "bytes":<sum of referenced section bytes>,"new_bytes":<bytes written this generation>,
   "executable":"<digest>","executable_changed_since_bootstrap":false}}
```
`metadata` keeps every key consumed by `examples/python/campaign_monitor.py:135`, `shared_owner_campaign.py:566`, `cli/shards/events.rs`, `cli/progress/routed.rs`, and `supervisor.rs:649-655` (`metadata.state == "saved"`). `state_path` points at the generation's meta file (monitors use it only as a milestone key).

### 4.4 Store rewrite (`W/checkpoint.rs`; codec split into `W/checkpoint/{sections,manifest,domains,edges,ledger,index,meta,restore,test_support}.rs`)

```rust
pub(super) struct Store { options, _lock, manifest: Option<Manifest>, request: String, executable: String, owners: Vec<String>,
    last: Instant, last_save_seconds: f64, saved_stamp: Option<Stamp>, next_generation: u64 }
impl Store {
  fn open(request) -> Result<Option<Self>>;            // manifest checks (§5); reserves next_generation by creating records-<G>.jsonl with create_new (skips existing names)
  fn resume<N>(&mut self) -> Result<Option<Restored<N>>>; // verify all section digests in parallel (rayon), decode, validate (§4.6), hand sealed segment list to the RecordSink
  fn save<N>(&mut self, state:&State<N>, inputs, frontiers, force, observer) -> Result<Option<Value>>;
  fn publish(&mut self, manifest) ; fn cleanup(&self, latest:&Manifest, previous:&Manifest);
}
```
`save()` sequence: (1) timer/force/stamp checks (§6); (2) refuse if `state.error`; (3) seal records segment (`RecordSink::seal`), take `Ref<Tracker>`; (4) `rayon::scope` with one task per section, each writing through `write_file_atomically_with(path, false, |f| HashingWriter …)` and returning `(bytes, blake3)` — no re-read (requirement b); domains/edges segment writers take `&queue.domains[first..]` and `tracker.edge_segment(persisted_log..)`; (5) build manifest, `publish` (unchanged rename+fsync+previous.json logic at `checkpoint.rs:244-259`); (6) fold the edge log (§3.1) and mark it persisted; open the next records segment; (7) emit `checkpoint_saved` with unchanged keys plus `new_bytes`, `section_seconds`. `State`, `Queue`, `Tracker`, `Streams` must remain `Sync` for the scope (they are; the `RefCell`s are borrowed before the scope and only `&Tracker`/`&RecordSink` references cross threads).

### 4.5 Segment-aware cleanup (replaces `checkpoint.rs:266-293`)

After a successful publish: `referenced = files(latest) ∪ files(previous)`. For each entry matching `^(meta|nodes|ledger|index|domains|edges|records)-(\d{20})\.(json|bin|jsonl)$` with `generation < previous.generation` and not in `referenced`: remove. Append segments are always referenced by `latest`, so they survive; rewritten sections older than `previous` and orphans of failed saves/crashed tails are removed; nothing is removed on save failure (existing policy preserved). Then `sync_all` the directory.

### 4.6 Restore validation (`W/checkpoint/restore.rs`)

1. Manifest: schema 5, kind, `walk_semantics_version`, request binding, arity == N, policy consistent with the request's ledger kind, segments tile `[0,total)` contiguously per section, every file name matches its section/generation pattern.
2. Every referenced file: `stat` length == `bytes`, blake3 == recorded (parallel). All digests, always.
3. Sections: header magic/tag/arity/semantics version; fixed-width byte counts; domains: `try_from`-range validity, `powers.validate()`, rebuild `exact` (duplicate ⇒ error) and summaries (rayon-parallel `DomainPowerSummary::try_new` then compact); ledger: `StoredLedger::restore` equivalents + `validate_checkpoint` + `restore_normalize_started` unchanged; index: `restore_positions` plus a new envelope check (recomputed envelope from member summaries must be contained in the stored envelope; ids ordered; `live` counts); nodes/edges: `Tracker::restore` (§3.1); counters: the same inequalities as `codec.rs:222-240`; streams: `validate_restored_streams` with the accepted-events counter; ledger↔closure cross-check (§1.4).
4. Post-restore RSS is recorded in the `checkpoint_restored` event (new), together with restore seconds per phase.

### 4.7 Tests (keeping JSON-key corruption tests meaningful — requirement a)

`W/checkpoint/test_support.rs` (cfg(test)): `rewrite_section(dir, "ledger", |entries: &mut Vec<StoredEntry>| …)`, `rewrite_nodes`, `rewrite_edges`, `rewrite_meta(|Value|)`, `rewrite_domains`, each re-encoding the section and **recomputing the manifest digest** so the semantic validators, not the checksum, must catch the corruption; plus `flip_byte(dir, section, offset)` for the checksum path. Port every negative test listed by the coordinator: `delegation/ledger/ready_tests.rs:265-471`, `descendant_closure_tests.rs:91,109`, `closure_tests.rs:206,242,288` (magic assertions become `format == "RUSTRED-WALK-CP5"` and "CP3/CP4 manifest refused"), `subdivision_tests.rs:725` (corrupt progress), `checkpoint.rs:512` (truncation now on a section file; expect "checksum or length"). New: `every_section_digest_is_verified_before_decoding`, `segments_must_tile_the_total_without_gaps`, `manifest_from_cp4_is_refused_with_fresh_campaign_message`, `bucket_order_is_deterministic_bytes` (two saves of the same state produce identical index bytes), `cleanup_removes_only_unreferenced_files_below_previous`, `failed_save_keeps_old_authority_and_orphans`.

Update `round_trip_state`/`round_trip_state_on_disk` (`checkpoint.rs:380-427`) to the sectioned store (both remain the seam used by ~20 tests); `test_directory` keeps `TMP/`.

### 4.8 Measurement gate

Synthetic state (§7.3): gen-2 save (1M new domains, 20M new edges, 350k new records over a 20M base) ≤ 15 s and `new_bytes` ≤ 1.3 GB + segments; full restore ≤ 3 min; RSS after restore ≤ 1.5× in-RAM estimate. Live campaign: `save_seconds` per generation flat in generation count (slope ≤ 1 s/generation), versus +28 s/gen today.

**Effort:** 4–5 days.

---

## 5. Binding relaxation and `WALK_SEMANTICS_VERSION`

**Where.** `W/mod.rs`, next to `OwnerDomainWalkRequest`:

```rust
/// Contract: bump whenever a resumed walk could produce different admission, matching,
/// routing or replay results from the same checkpoint. This includes: queue containment
/// predicate or index minimum-id semantics (queue.rs, queue/index*.rs, core summary.rs);
/// admission commit order or speculative-lookup equivalence (execution/admission.rs, queue/prepared.rs);
/// ledger reservation/transfer/publication rules (delegation/*); replay token or prefix hashing
/// (execution/replay.rs domain strings "rustred-walk-event-v1"/"rustred-walk-prefix-v1");
/// inspection event emission order or effects (inspection.rs, reuse.rs, initial_orthants.rs,
/// initial_overlap.rs, routing.rs); Ticket encoding and physical subdivision (physical_parts.rs);
/// core RoutedCandidateReducer matching/routing semantics; the sidecar record schema.
/// Transport changes (checkpoint interval, buffer sizes, worker split, logging) do not bump it.
pub const WALK_SEMANTICS_VERSION: u32 = 1;
```
A `docs/shared_owner_campaign_driver.md` paragraph documents the same contract; PR checklist item.

**Manifest records** (§4.3): `schema`, `walk_semantics_version`, `request` (binding digest; `binding()` unchanged in content, plus `arity`), `owners`, `executable` and `executable_first` (informational), `metadata.executable`.

**Resume checks** (`Store::open`): schema == 5 (else "unsupported checkpoint generation; complete dependency history requires a fresh CP5 campaign"); kind; owners equal (`bind_owners`); `request` equal ("checkpoint request or policy differs; refusing to restart"); `walk_semantics_version == WALK_SEMANTICS_VERSION` (else "checkpoint walk semantics version differs (saved {v}, executable {w}); resume requires identical admission/matching/routing semantics"); executable digest differs ⇒ observer event `checkpoint_executable_changed {saved, current}` and `metadata.executable_changed_since_bootstrap = true`, not an error. Ready/Ordered come from the binding (`publication` is inside `binding()`), so no separate schema per policy.

**Test seam.** `Store::open_with_identity(request, executable: String, semantics: u32)`; `open` calls it with `file_digest(current_exe)` and the constant.

**Tests** (`W/checkpoint.rs`): `resume_accepts_changed_executable_digest_with_same_semantics_version` (bootstrap+save with exe "A"; reopen with "B" ⇒ Ok, manifest `executable_first == A`, event emitted); `resume_refuses_changed_semantics_version_even_with_same_executable` (reopen with version+1 ⇒ Err containing "semantics version"); `binding_excludes_transport_and_includes_policy` (existing `application_refinement_is_exact_checkpoint_policy`, `joint_source_support_policy_is_checkpoint_bound` retained).

**Effort:** 1 day.

---

## 6. Save scheduling

- Interval remains transport (`OwnerDomainWalkCheckpointOptions.interval_seconds`, default 3600; campaign uses 14400). Add adaptive stretching in `Store::save`: `effective = max(interval, 20 × last_save_seconds)` (≤ 5% duty), reported as `metadata.effective_interval_seconds`. With O(new-state) saves this rarely engages.
- Unchanged-state skip (requirement c): `State::change_stamp() -> Stamp { domains, published, events, closure_revision, ledger_reserved_through, ledger_transfers, records_total }`; `save()` returns `Ok(None)` and emits `checkpoint_skipped_unchanged` when `force && stamp == saved_stamp` — this makes the forced save right after resume (`mod.rs:566-575`) free, while the fresh-start save after initial admission still runs. The timer starts at `open`, so the first periodic save is `interval` after resume.
- Cooperative stop: the final forced save (`mod.rs:591-598`) and `checkpoint_paused` semantics are unchanged; exit code 4 unchanged.
- Closure refresh multiplier: `descendant_closure.rs:181` becomes `const REFRESH_DUTY_MULTIPLIER: f64` (proposed 100.0; the scheduler package owns the final value). With CSR the refresh itself shrinks, and `force=true` refreshes (`mod.rs:603`, and the one I add before `save()` so the checkpointed `total_closed` is current) are unaffected.

**Tests:** `forced_save_after_resume_is_skipped_when_nothing_changed` (two consecutive forced saves ⇒ one generation); `effective_interval_stretches_after_slow_save` (inject `last_save_seconds`).

**Effort:** 1 day.

---

## 7. Restore-at-scale test plan

All in new `W/checkpoint/scale_tests.rs`, `#[ignore]`, driven by env vars, emitting a JSON receipt to `TMP/`.

7.1 **Timed restore of a copied large checkpoint** — `restore_copied_production_checkpoint`: `RUSTRED_CHECKPOINT_RESTORE_DIRECTORY` (a `cp -r` of a live CP5 directory; the lock is per-directory), opens with `resume=true`, runs `Store::resume`, records phase timings (digests, domains+summaries, index, ledger, closure CSR, cross-checks), `/proc/self/status VmRSS` before/after, and asserts `validate` passes. Run pinned: `taskset -c 192-241 cargo test --release -p rustred-app restore_copied_production_checkpoint -- --ignored`. Expected RAM with the new format ≈ in-RAM state (~16–30 GB at 38M), well under 130 GB.

7.2 **Resume-equivalence on the four-loop FG control** — `fg_control_resume_equivalence`: inputs from `RUSTRED_FG_CONTROL_INPUT` (same shape as `positive_reuse_trace/tests.rs:292` `input.json`: manifest, queries, owner_base, stop_file); run A uninterrupted with checkpointing; runs B/C interrupted at ~30%/~60% `committed_domains` via a `StopWatcher` on the stop file, then resumed (`resume=true`) to completion. Assert equal: `write_json` documents minus `seconds`/`checkpoint`/`parallel` timing keys (records identical in order), counters (`events, successors, conditional_successors, completed_nodes, scheduled_nodes, deduplication_hits, containment_checks, containment_retired_candidates, exact_domain_hits, full_orthant_hits, frontiers`), `descendant_closure.{total_closed, initial_closed, dependency_edges}`, `delegation` summary. Both Ordered and Ready (`publication_policy` env). The old CP3 controls in `TMP/dependency-monitor.EdVwnw` are not resumable by design; the equivalence baseline is the fresh uninterrupted run with the new binary, plus a one-time comparison of run A's `domains` array against the retained CP3-era `result.json` of the same control (modulo `seconds`) to prove the redesign is result-neutral.

7.3 **Synthetic large-state save/restore benchmark** — `synthetic_state_save_cost_is_proportional_to_new_state`: a `cfg(test)` `Queue::admit_synthetic_for_benchmark(domain)` inserts into exact/index/summaries without containment lookups; generate 20M random `N=15` domains (coordinates ≤ 24), a ledger with a realistic published prefix (35%), a closure with 20 edges/node, and 7M template records into the sidecar; save gen 1 (full), add 5% more of everything, save gen 2. Assert `gen2.new_bytes ≤ 0.1 × gen1.new_bytes + rewritten_sections`, `gen2.save_seconds ≤ 0.25 × gen1.save_seconds`, restore ≤ 3 min, restored counters equal, and per-component RAM from `storage_estimate_bytes`/`Vec::capacity` within §0 targets. Parameterized by `RUSTRED_SYNTHETIC_DOMAINS` (default 2M for a 1-minute CI-optional run).

**Effort:** 2–3 days plus machine time.

---

## 8. Ordering, dependencies, interfaces, effort

**Sequence** (arrows = hard dependency):
1. §5 semantics constant + `binding` (1 d) → 2. §4 Store/sections/manifest/cleanup/test_support with today's data types (4–5 d) → then in parallel: 3a. §1 records sidecar + streaming result (3–4 d), 3b. §3 edges CSR + node flags (2–3 d), 3c. §2 compact domains/summaries (4–5 d) → 4. §6 scheduling/skip (1 d) → 5. §7 scale tests and live measurement (2–3 d).

Total ≈ 18–22 engineer-days; ~10 calendar days with three people after step 2. Every step leaves the test suite green because §4 lands first with old structures behind the new codec, and §1–§3 each swap one section.

**Interfaces needed from the scheduler/admission package:**
- Keep `maybe_save: &mut dyn FnMut(&State<N>)` (immutable) and all `State/Queue/Tracker/Streams` fields `Sync`; do not add `Rc`/`Cell`/`RefCell` fields other than the existing `closure` and the new `records` (both borrowed before the parallel section scope).
- Any change to admission ordering, batch equivalence, ledger rules or Ticket encoding must bump `WALK_SEMANTICS_VERSION`; I will add the doc contract, they own compliance.
- Decide `REFRESH_DUTY_MULTIPLIER` (proposed 100) and whether they want the pre-save forced refresh.
- If they move saves off the coordinator (snapshot thread), the sectioned writers are already per-section closures and can be reused; not planned here.

**Interfaces needed from the physics-inputs package:** confirm coordinate/rank/power ranges for the campaign inputs (`MAX_COMPACT_COORDINATE = 65534`, rank < 2^32, |power bounds| < 2^63, N ≤ 16) so the compact refusal is unreachable; confirm Ready is the production policy so `records_accepted_events` is exercised by the control.

**Migration/compat:** none for old formats (CP1/CP3/CP4 manifests are refused with the fresh-campaign message; `docs/shared_owner_campaign_driver.md:41-43` and `docs/CLI.md` updated to say CP5). The same A24/R15/D9 campaign runs fresh with the new binary; the `production_saved_owner_campaign.py` prepare path is unaffected (it checks owner digests and manifest `bytes`, both retained).

**Risks:** (1) `CompactSummary::contains` replication — mitigated by the differential test and by keeping `DomainPowerSummary::try_new` as the source of values; (2) `HashMap<u128,u32>` exact map changes `exact_hits` only if a 128-bit collision occurs (negligible); (3) restore of the index envelope check is new and could reject legitimate stale-outward envelopes — the check is containment, not equality, matching `Block::retain`'s comment; (4) `OwnerDomainWalkResult.document` loses `domains` for streamed runs — only `cli_routed_campaign.rs:649` reads it and still passes; document the change in `docs/interfaces.md`.

### Critical Files for Implementation
- /common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/checkpoint.rs (Store, manifest, publish/cleanup, binding, semantics checks; new `checkpoint/{sections,manifest,restore,test_support}.rs`)
- /common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/checkpoint/codec.rs (replaced by sectioned codecs; `validate_closure_records` → ledger↔closure cross-check)
- /common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/execution.rs (State: `records` sink, `records_accepted_events`, `change_stamp`, record construction at 591-663, `Queue::domain()` call sites)
- /common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/queue.rs (compact domains/summaries slab, `u128` exact map; plus `queue/checkpoint.rs`, `queue/index.rs`, `queue/prepared.rs`)
- /common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/descendant_closure.rs (node flags, CSR + append log, fold, restore, refresh multiplier)
- /common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/mod.rs (`WALK_SEMANTICS_VERSION`, `OwnerDomainWalkResult::write_json`, streaming finalization at 637-675; CLI writer at crates/rustred-app/src/cli/owner_match.rs:181-186)
