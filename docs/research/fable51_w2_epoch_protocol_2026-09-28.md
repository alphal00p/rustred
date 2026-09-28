# W2.0 protocol note: the v3 epoch engine (walk semantics 3, checkpoint CP6)

Date 2026-09-28, **revision 2** (~08Z). Role w2-protocol (design only; nothing here is implemented, built or run).
Revision 2 answers three independent reviews of revision 1: soundness (SND), performance and scale (PRF), and
implementability and testability (IMP). Each problem is either fixed in place or its rejection is recorded in the
disposition table (§20). Revision 1 (sha256 537f2cff...) is superseded.

Base: branch `fable_5_1` @ 3bc4349d. Every W0/W1 lane branch is still unmerged. Tips at 07:36Z: oracle 63771a50,
ops fc3c07e4, kernel 34861652 (plus an uncommitted fix round), harness b0e9db6e, g2falsify cc2ff75c.
Paths: `W` = `crates/rustred-app/src/application/routed_campaign/walking`; `E` = `W/epoch` (new, this note).

Labels:
- **[M]** measured, with the receipt named.
- **[src]** read in source at 3bc4349d, or at the named branch tip (file:line).
- **[D]** stated in a governing document and not re-derived here.
- **[E]** estimate or design choice.
- **[proposal; owner confirms]** a rule stricter than, or in addition to, an owner decision.

Nothing in this note is an ETA or a closure claim. Termination of the five-loop walk is not established;
`family_closure_claim` stays false.

Binding inputs:
- `HANDOFF_opus_5_5.md` §0, §0.1 (items 1-11), §1-2, §5, §7.
- `FABLE_5_1_CRITIQUE.md` (all).
- `TMP/progress/orchestrator_decisions.md` items 1-9, including amendments 2a (harness fix round) and 3a (inputs fix
  round).
- `docs/research/fable51_next_push_master_plan_2026-09-27.md` §0, §3, §5 (W2), §6, §7.
- `docs/research/fable51_v3_engine_design_2026-09-27.md` (all, F1-F20).
- The lane reports `TMP/progress/*.report.json`, `TMP/progress/comparator.md` (runA INVALID) and `TMP/w0/*/RESULTS.md`.

Where this note and the v3 note or the master plan disagree, this note (as W2.0) amends them, within the limits of
handoff §0.1 and the orchestrator decisions.

---

## 0. Decisions in brief

1. **Process model (§2).** ONE process. Each 8-core CCX runs one inspector group with a private copy of the read-shared
   native state, first-touched by the group: owner programs, `RoutedCandidateReducer`, and the initial orthant and
   overlap indexes.
   - The per-CCX figure is **[E, loaded, provisional]** (amendment 2a). Every throughput projection is bracketed by
     the contention factor κ ∈ [1.42, 3.75]; 1.17 is a named provisional scenario.
   - Replicas cost 5.46 GB each, 60 GB for 11.
   - Snapshot hot data are immutable byte segments with no pointers inside (§3.4). Per-CCX inspector processes that
     map those segments read-only are therefore a real fallback, with a trigger written in advance (§2.1).
2. **Hot-path rule.** No atomic read-modify-write and no refcount write on any cache line that inspectors read per
   lookup, per successor or per polynomial.
   - Per-job shared writes: only the `SnapshotCell` lock, the group queue and the result channel.
   - Every `Arc` that inspectors traverse wraps a `Box`, so its refcount line holds no hot data.
   - At job start the resolver copies the snapshot's scalars and roots into thread-local state.
3. **Numbering (§3.1).** Merge k+1 turns S_k into S_{k+1}, and its natives carry merge epoch k+1. A native is visible
   in S_v iff its merge epoch is <= v.
4. **ledger6 (§4).** One `u64` per ID, 7 states, transitions T1-T10 and T12. T11 is designed for the owner but not
   built. The transition table is exhaustive and release-checked.
5. **The A1 chokepoint covers every edge (§5.3).**
   - `verify` is defined over a `Container`: stored in a snapshot or a merge view, an earlier miss of the same job, or a
     planned survivor.
   - Inspector hits ship as targets of `Verified` tokens.
   - Every merge-side edge and every transfer gets its token in P2.
   - Locals ship their query image.
   - Across the byte boundary, inspector hits are trusted modulo the canary in production and are fully re-verified
     on controls.
6. **Canary (§6.6).** A deterministic >= 1/64 of hits (every hit on controls) ships its query image. P2 re-verifies
   those hits with an independent predicate: the oracle's `lattice::Cell::contains` on expanded domains. Any C5
   writes a sticky `poison-<k>.json`, which voids engine certification in every later generation until an operator
   clears it with a full-F10 receipt.
7. **F1 at the merge boundary (§6.3).**
   - P2 recomputes the digest and the bucket key from every shipped 96-B image; a mismatch is C5.
   - P3 asserts `image.digest() == key` before every exact-index insert.
8. **F7 vs A2 (§9).** P1 classifies results into C0-C5, with an explicit `break_reason` and `panic` flag.
   - C0 and C2 merge, with event parity; the rules for breaks and panics are defined.
   - C1 is discarded and requeued.
   - C3 is retried once.
   - C5 poisons the run and writes no generation.
9. **Liveness (§8.4).**
   - Counters move only for per-job causes. Host-wide, policy and operator stops never penalize a job.
   - A RAM-guard stop attributed to our process tree charges `guard` only to the jobs with the largest metered
     allocation growth.
   - A durable per-session `inflight` journal, plus fatal-signal notes, attributes hard kills, OOM kills and aborts.
   - Suspects run isolated and become Exhausted at guard 3. Guard is derived at restore from the journal, so an
     attributed crash loop ends within <= 3 relaunches. The engine refuses to start after 2 consecutive unattributed
     or zero-progress abnormal exits.
   - `--exhaust-id` marks an ID Exhausted by hand. Deferred IDs have a guaranteed dispatch share.
10. **Rolling replay oracle (§12).** Every resolver input is versioned.
    - Sealed and live hints are frozen bitsets, published at versions ≡ 0 mod H.
    - Compactions are planned at those versions and installed at the next one; young-tier promotion is synchronous
      in P4.
    - Cut membership, discards, stops, refresher installs and operator flags are journaled.
    - The replay unit is one session, from its restore. The replayer is the engine in forced-schedule mode:
      inspections run in parallel, and merges replay serially in recorded order.
11. **CP6 save (§11.2).**
    - Every stop path gets a `MergeBoundary`, through bookkeeping-only merges.
    - The boundary is issued after P3 and holds `&EpochState` for the whole write. This freezes refill, requeue
      bookkeeping and installs.
    - Append-only data live in per-generation sealed segments (blake3); open segments are never referenced by a
      manifest.
    - Results that arrive after a stop's save are dropped unread.
    - The engine parses the stop-file JSON, including the RAM-guard attribution.
12. **Anchors (§7).**
    - G2' never anchors an ID < P0.
    - An initial-D-band native lends only its inspected slice as a G2' anchor.
    - The responsibility relation is well-founded by construction, and it is checked in P1 and at restore.
    - G2' plans anchors from a versioned per-bucket `MergedView` in the snapshot, never from ledger6.
13. **Merge path (§3, §6, §8.2).**
    - A cut holds <= 512 results.
    - P3 costs <= 10 us per native at 74M [E, gated at S4].
    - Refill may happen at any point except a save pause. Queue depth is cost-weighted: >= 20 ms of predicted work
      per group, and D_max >= 3I.
    - Each bucket has <= 12 tiers (a young tier in ID order plus size-tiered pattern tiers), each with an envelope.
    - The exact index is sharded 4,096 ways, and every per-ID array is a chunk tree.
14. **Restore (§11.4).**
    - The edge log is cross-checked against a records digest of (id, tag, distinct edge count) and against per-tag
      counts. This restores the out-degree check of master plan §6, check 5.
    - The roots map is persisted and re-verified.
    - Digests are computed during decode, not as a separate pass. Aliases and records are verified since a
      watermark, with the full pass in the audit tool.
    - The monitor is built in two passes.
    - A per-phase time model is gated on a >= 3e8-ID state.
15. **Resolution modes.**
    - `--epoch-resolve merge` is a diagnostic only and is not bound.
    - Summaries: no per-ID slab. `verify` tries raw inclusion first and recomputes the summary otherwise; the 2%
      recompute gate stands.
    - First-found bias: oldest tier first, sealed-first MRU and a sealed-preference scan, all reading the frozen hint.
16. **Gates (§15).**
    - U (useful native work) uses one frozen, pooled `c_K1^ref` table in cycles:u, with fixed successor bins.
    - κ uses `c_K1^arm` in native-frame cycles:u. It is accepted only when >= 0.9·I inspector threads ran
      concurrently.
    - The IPC clause is measured on native frames.
    - S4 adds >= 4x less CPU per tested candidate on the epoch layers (orchestrator decision 1).
    - The chained-pilot ladder P-CHAIN, up to >= 3e8 real IDs, replaces projections from 74M alone.
17. **Owner gate (§15.2).**
    - Only the owner's criterion decides the ladder: mean obligations/h >= 1.5x the comparator over load-matched
      slices, with build flags matched.
    - Soundness preconditions include RollingLite audit and full F10 at W >= 24.
    - U/h, κ, (H) and the trajectory against the P-IMP floor are pre-registered diagnostics.
    - The comparator is the fixed legacy + SoA frame-pointer binary named in `TMP/w1-kernel/bin/COMPARATOR_READY.json`;
      d9163195 is void as a comparator [M runA].
    - MVP-B = S2-S4 + S3 + kernel with Lockstep depth 2 (handoff item 9).
18. **Policy.** `--frontier-policy stop` is the default and is bound. D7 "always stop" is the default pending the
    D-session. Resumable stops exit 4 with a `stop_reason`; engine-fatal stops exit 70.
19. **Stage order (§17).**
    - S2, then S4 (+ replicas, CP5 import, RollingLite), then the owner gate.
    - S3 runs in parallel from S2: CP6, typed binary records, ledger6 persistence, crash journal, poison.
    - Then S5, S4b, S6, S2.6, P-IMP and P-CHAIN.

---

## 1. Evidence this note rests on

| Quantity | Value | Label and receipt |
|---|---|---|
| CPU per native vs K=1, one shared copy | K=24 1.463x, K=48 2.235x, K=96 3.754x / 3.753x; p4 (4 node-bound processes) 1.417x | [M] `TMP/w0/harness/sessions/C` (quiet) |
| Remote share of fills | 0.1% at K<=24 and p4; 63% at K=96; 81% interleaved | [M] critique §2.1 table |
| Per-CCX in-process copies at K=96 | Same-session contrast: shared 2.06-2.14x -> per-CCX 1.14-1.17x -> per-CCX processes 1.155x; per-node copies 1.221x; K=24 per-CCX 1.109x | **[E, loaded, provisional]** (amendment 2a): session E only, 63-86% foreign load; quiet sessions F2/G/H are queued (`sessionF2.sh`); shared gave 2.94x in session D at similar load |
| Root cause | Shared written lines of the Symbolica `Arc<PolynomialContext>` count (`polynomial.rs:810`) and/or `Arc<Vec<PolyVariable>>` clones; the split is unresolved; ahash `RandomState::new` residual shown cheap only under load | [M] harness fix round, amendment 2a(iii) |
| Replica cost | 5.09 GiB = 5.46 GB per extra copy; 12 copies 65.7 GB (VmHWM 66.4-67.2 GB); parallel prepare 124-143 s; per-copy cache growth over hours unmeasured | [M] amendment 2a(ii) |
| Instructions per native | constant across K, 2.843e8 | [M] session C |
| Cancellation latency | p99 53.8 us, max 1.20 ms | [M] `TMP/w0/harness/cancel` |
| Real gen-7 streams | 225.6 requests per native; at MRU k=16 the cheap tiers resolve 87.5% (exact-job 17.0, self 14.0, Local 0.9, MRU 52.4, exact-store 0.5, helper 2.6); layer hits 11.1%, misses 1.4%; per native r_hit 25.02, r_miss 3.21 | [M] `TMP/w0/intel/replay/streams-g7-v4.jsonl` |
| Layer cost at 74M, SoA-pattern first-found, 1 thread, one static layer | hit 580 candidates / 5.4 us; miss 2,626 / 26.3 us; reverse 2,938 / 34.9 us | [M] intel; timings void under the §7 protocol, counts [M] |
| Admission per native at 74M | hits 135 us, miss scans 85 us, reverse scans 112 us, cheap tiers 20 us (0.351 ms, 6.3% of worker CPU) | [E] intel gate 0.4(b) model on [M] costs |
| Min-ID on pattern layers | loses the early exit: 6,720 candidates per hit vs 2,096 in ID order | [M] intel |
| Stale share of layer hits by snapshot lag | 1.51% (< 1,024 new IDs), 2.60% (< 65,536), 3.50% (< 1M) | [M] intel |
| Kernel lane (legacy + SoA) | strict identity PASS on C-4L, four-all, C-5F; gen-7 differential 0 mismatches; 2.86-3.79x less CPU per candidate (gate c FAIL, accepted by decision 1); 92-B summaries | [M] `kernel.report.json` |
| Comparator binary d9163195 | **void as a comparator**: its Ready heartbeat walks the queue storage (progress_json 25.9% of coordinator wall vs 0.87% in run2); the fixed binary is pending (`COMPARATOR_READY.json`) | [M] `TMP/w1/comparator/runA`, `comparator.md` 07:17Z |
| runA (diagnostic only, INVALID) | 2.08M obligations/h over [T, T+1,291 s] (0.92x run2); 58.3 foreign busy CPUs (run2 33.4); restore 508.6 s, launch to restored 626 s | [M, INVALID] `comparator.md` |
| M1 run2 (production baseline) | W100 gen-7 resume, [T, T+1500.9 s]: 2.27M obligations/h (5-min slices 1.76-3.08M), 1.30M natives/h (1.04-1.72M); coordinator duty: commit 48.1% + prep 43.6%; ~9.4 of 100 CPUs used; marginal 847 B/domain (late 493 B); VmHWM 47.7 GB; launch to restored 600.4 s | [M] `TMP/w0/baseline/m1/run2/analysis.json`, binary 7eed68fc |
| Gen-7 state | 74,156,033 domains, 1,192,281,291 edges (16.07 per domain), 27.47M natives, 37,907,667 live candidates | [M] kernel harness receipt, oracle gen-7 run |
| G2' falsifier, union arm | record-s 0.496-0.537x, run-s 0.53-0.55x, scheduled 0.79-0.80x, peak pending 0.82-0.86x; literal S7 (full natives only) 0.68-0.77x; the union arm used initial-overlap partials as anchors (4 references on C-5F) | [M] `TMP/w0/g2falsify/gate-table-2.md`, `RESULTS.md` §0/§5 |
| Oracle gate contract | `verdict == PASS` and `roots_independently_verified == roots_total >= 1`; INCOMPLETE (exit 9) for partial re-inspection; full F10 at gen 7 ~15 h at 32 threads [E] | [M] `oracle.report.json` |
| RAM guard (ops) | 50 GB host MemAvailable floor. Cooperative reasons: `aggregate_rss_soft_limit`, `host_memory_reserve`, `own_swap_growth_sustained`. Hard reasons (`aggregate_rss_hard_limit`, `host_memory_emergency`) lead to SIGKILL. The stop file holds `{reason, unix_time, family_closure_claim}`; the host-wide attribution `own_memory_signal` exists only in the supervisor's result. Production guard: 2 consecutive attributed zero-progress stops | [src] ops fc3c07e4 `shared_owner_campaign.py:60-90, 476-505, 798-806` |
| Socket-1 memory | nodes 4-7: 141.6-141.7 GiB each, free 82.0 / 19.1 / 64.0 / 51.4 GiB at 07:51Z (41-86 GiB at 07:06Z per PRF review); `numa_balancing = 1`; kernel 6.18.45 | [M] `/sys/devices/system/node/node{4..7}/meminfo`, `/proc/sys/kernel/numa_balancing`, 07:51Z |

Source facts this design depends on [src]:
- `CompactDomain<15>` is 96 B (`W/queue/compact.rs:44-60`).
- `CompactSummary::contains` accepts an EMPTY candidate before the owner test and never compares phase
  (`compact.rs:513-548`).
- `CompactDomain::{contains, is_full_orthant, try_native_summary, native_summary, digest}` are `pub(super)`
  (`compact.rs:173-211`).
- The exact index is a single digest-keyed hashbrown map with overflow, confirmed on the stored image
  (`compact.rs:278-399`).
- The legacy transfer requires `representative + 1 == entries.len()` (`W/delegation/ledger.rs:276`).
- A failed or cancelled native halts the legacy ledger (`ledger.rs:397`), and CP5 refuses to hold it
  (`ledger.rs:513`).
- The closure edge log is u32, with `LOG_CAP = u32::MAX - 1` (`W/descendant_closure/edges.rs:11-14`); refresh is
  throttled by wall time (`W/descendant_closure.rs:43, 185-215`).
- The CP5 binding hashes `workers`, `inspection_workers` and the `Debug` form of the scheduling policy
  (`W/checkpoint.rs:104-119`), and resume refuses any other binding or policy (`checkpoint.rs:277-293`).
- The job-local reuse cache is capped at `MAX_KEYS = 4096` (`W/reuse.rs:13`). `KnownReuse` carries only
  `(successor, conditional)` (`reuse.rs:76-79`).
- Native `error_kind` values are `none`, `cancelled`, `consumer_stop`, `native_failure` and `conversion`
  (`W/inspection.rs:384-402`).
- Initial-orthant short-circuits happen inside the native visitor (`W/inspection.rs:342, 350`).
- `max_events` is an aggregate successor-event allowance (`W/execution.rs:567-581`).
- The engine checks only that the stop file exists (`crates/rustred-app/src/cli/routed.rs:92-94`).
- The semantics probe prints `walk_semantics_version`, `checkpoint_format` and `checkpoint_schema`
  (`cli/mod.rs:334-341`).
- The oracle's `verify_closure::lattice::Cell::contains` exists (oracle 63771a50, `lattice.rs:287`).

---

## 2. Process and placement architecture (handoff §0.1 item 1)

### 2.1 Decision, fallback ladder and switch rule

| Rank | Option | Evidence | Verdict |
|---|---|---|---|
| 1 | In-process per-CCX replicas of the read-shared native state, each built by a thread pinned in its group | 1.14-1.17x at K=96 [E, loaded, provisional]; identical results (FG 98,869/98,869, X 46,826/46,826 digest differential) | **Adopted.** Confirmation of record: the in-engine κ at S4 (§15.1) |
| 1' | In-process per-node replicas (3-4 copies) | 1.221x loaded [M]; ~1.42x expected quiet [E] | Memory fallback only |
| 2 | Per-CCX (or node-bound) inspector processes, each with a private replica, mapping the snapshot segments read-only | p4 1.417x quiet [M]; per-CCX processes 1.155x loaded [M]; also removes process-global residuals (ahash counter, allocator arenas) | **Fallback 1** (switch rule below) |
| 3 | Per-CCX processes with `--epoch-resolve merge` | Moves ~201 requests per native to the merge helpers, with an O(m²) antichain (critique EPOCH-7) | **Fallback 2.** Its helper cost is measured in S4 |
| - | Interleaved placement | Remote share rises to 81% [M] | Removed. No "layers interleaved over nodes 4-7" without measurement |

Rev 1 wrongly rejected shared-memory segments. The harness objection concerns Symbolica objects, whose written
refcount lines live inside the replicas. The snapshot holds no Symbolica objects. Option 2 therefore shares only the
snapshot; replicas stay private per process.

**Snapshot as byte segments** (makes option 2 possible without re-plumbing):
- Store chunks, layer blocks, bucket-table chunks, frozen bitsets and the `MergedView` are immutable byte arrays
  (`Box<[u8]>` in-process).
- They are read through safe `from_le_bytes` views. The SIMD kernel already works on u8 lanes, and ids and words decode
  per access.
- References between segments are `(segment, offset)` pairs, never pointers. The in-process `Arc` tree only owns
  segments.
- In option 2, the coordinator writes each new segment to a memfd (or a file under `TMP/<run>/seg/`), sends its fd to
  the inspector processes at publish, and they map it read-only. The mapping code lives in `W/segmap.rs` (unsafe, outside
  `E`, next to `W/placement.rs`).
- Publish cost [E]: one tail mini-chunk (<= 6 KB), the touched young-tier tail blocks (<= 1.6 KB each) and the touched
  bucket-table chunks. It is measured in S4 as bytes per publish.

**Switch rule** (written before measuring):
- After S4, if the concurrency-qualified in-engine κ at W96 is > 1.3, **and** process-global symbols (ahash
  `RandomState::new`, allocator) cause > 50% of the cross-CCX fills per native, build option 2 (time box 5 working days
  [E]).
- If κ > 1.5 and option 2 does not bring it to <= 1.3 on the same protocol, measure option 3's helper cost and report
  to the owner.
- The `Verified`-token boundary is the same in every option (§5.3).

### 2.2 Thread and CPU split at W100 (CPUs 128-227, socket 1, shared host)

Socket 1 has 128 cores (128-255), no SMT online, and 4 NUMA nodes (4-7) of 32 cores, with 8 cores per L3 (CCX)
[M lscpu, sysfs]. CPUs 128-227 are 12 full CCX (128-223) plus half of CCX 224-231.

| CPUs | Role (default; not bound, A7) | Count |
|---|---|---|
| 128 | coordinator: P1, P3, publish, refill, stop logic | 1 |
| 129 | refresher: closure monitor, cones | 1 |
| 130 | saver, heartbeat, recorder; journal writer; borrows the merge helpers during a save pause | 1 |
| 131-135, 224-227 | merge helpers: P2, P4, bits keeper, compaction, monitor-shard folds, save chunk writers | 9 |
| 136-223 | 11 inspector groups (one per CCX), 8 pinned threads each, one private replica per group | 88 |

S4 A/B, since the split is not bound (PRF-15): arm (a) is the default. In arm (b), the coordinator's CCX 128-135 holds
only low-bandwidth threads: the coordinator, the saver/heartbeat, and 6 CPUs that act as chunk writers during save
pauses. Helpers and the refresher move to 224-227 plus CCX 208-215, with one inspector group fewer. The arm with the
lower P3 µs per native wins. A re-split at resume needs no fresh run. Pinning uses `sched_setaffinity` in
`W/placement.rs`. The socket stays shared (owner answer 11): foreign load is recorded, not excluded.

### 2.3 Shared-data rules on the inspector path

- **Read-only shared: the snapshot segments.** At job start and at each refresh point, the resolver clones
  `Arc<Snapshot>` once. It copies `version`, `published_len`, the store root and the bucket-table root into
  thread-local fields. Lookups never touch the `Snapshot` header again (PRF-12).
- **Refcount lines hold no hot data.** Every `Arc` that inspectors traverse is `Arc<Box<T>>`, or `T` is
  `#[repr(align(128))]` with padding, so a refcount write never shares a line with data read per lookup.
- **Per-publish refcount writes are bounded:** the touched bucket-table chunks (64 buckets each), the store-tree nodes
  on the tail path, and the tail mini-chunk.
- **Per group:** the replica; the job queue (`Mutex<VecDeque<JobBytes>>`, one lock per job); a result channel (one send
  per job).
- **Per thread:** job-local exact set, MRU, Local list, miss buffer, counters (shipped in `ResultBytes`), scratch, and
  an alloc-meter slot. The slot is a padded line written only by its own thread and read by the heartbeat at 1 Hz.
- **Forbidden:**
  - shared statistics atomics;
  - `Arc` clones per lookup or per successor;
  - `Arc<[T]>` for hot data;
  - any lock taken per successor;
  - reading the coordinator's `nodes`, `live` or `ledger` arrays.
- **Residual outside RustRed's control:** ahash in Symbolica. The split with `Arc<Vec<PolyVariable>>` clones is
  unresolved (amendment 2a(iii)); W1.2's hot-path clone audit serves both engines. No CAS code is written here.

### 2.4 Byte-serialized job and result types (S2 onwards, even in-process)

**`JobBytes`** (128 B fixed, plus an optional tail):
- `seq: u64` (session id << 40 | counter; unique across restores), `parent: u32`, `kind: u8` (Apply | Route),
  `attempts: u8`.
- `flags: u16`: isolated, suspect, canary-all.
- `n_batch: u8`, then reserved bytes.
- `image: CompactDomain<N>` (96 B at N = 15).
- Route micro-batching (S4 option, performance-only): a tail of (k-1) x (u32 parent + 96-B image), k <= 8.

**`ResultBytes`**, header (128 B):
- `seq, parent, class (u8), err_class (u8), break_reason (u8), panic (u8)`.
- `emitted_events, accepted_events: u64`; `stats_events: u64`, or `u64::MAX` when stats are unavailable (panic).
- `v0: u64, n_refresh: u16, n_targets, n_misses, n_locals, n_canary, n_frontiers`.
- `native_ns, resolve_ns, serialize_ns, native_cycles, native_instructions, alloc_growth_peak`.

`break_reason` takes one of: none | allowance | resolver_range | resolver_summary | spill_io | alloc | cancel.

**`ResultBytes`**, tail:
- Refresh points: `(u32 event ordinal, u64 version)`.
- Sorted unique target IDs (u32), each backed by a `Verified` token on the inspector.
- Misses: `(96-B image, u64 digest, u32 ordinal, u64 version)`, 116 B each.
- Locals: `(u32 ordinal, u32 ordinal of q1, 96-B q image)`, 104 B each (SND-3).
- Canary entries: `(u32 ordinal, u8 tier, 3 B pad, u32 target, 96-B q image)`, 108 B each (SND-4).
- Anchors (§7), native stats (fixed struct), and the typed record bytes (§11.7), whose 16-B trailer is patched in P3.

Mean size [E]: ~1.1 KB per Apply native (43 edges, 3.2 misses, ~0.9 Locals, ~0.4 canary hits, ~0.3 KB record); ~0.3 KB
per Route native. Encoding: explicit little-endian writer and reader in `E/job.rs`; no new dependency.

### 2.5 Replicas in the memory budget

- 11 replicas x 5.46 GB = **60 GB** [M per copy, amendment 2a(ii)]. This is a fixed term of every budget in §3.6.
- RSS growth of per-copy caches over hours is unmeasured. The S4 pilot and every P-CHAIN link record replica RSS at
  start and end. Growth > 10% per hour is flagged [proposal].
- Preparation (124-143 s for 12 in parallel [M]) runs concurrently with the restore.

### 2.6 Memory placement (PRF-6)

- **Default policy:** EpochState and snapshot segments are allocated under `MPOL_PREFERRED_MANY` over nodes 4-7
  (`set_mempolicy` around their allocations in `W/placement.rs`; kernel 6.18). They fill socket 1 before spilling to
  socket 0.
- Replicas are first-touched by a pinned thread of their group; helper scratch is first-touched by the helper. There is
  no interleave.
- Whether an explicit policy exempts pages from AutoNUMA migration on this kernel is checked in S4, not assumed.
- **Recorded in every pilot:**
  - numa_maps per major section (store, ledger, exact index, edges, layers, replicas);
  - `/proc/vmstat` `numa_hint_faults` and `numa_pages_migrated` deltas;
  - socket-1 free memory at launch.
- **Measured once:** in a P-IMP diagnostic arm (§15.3), the store and layers are bound to socket 0. The measured
  cross-socket penalty on resolve CPU per request and on P2 µs per native is used in launch criterion (C).
- Finer placement (per-section first touch by pinned helpers) remains the open W3.5 measurement (amendment 2a(iv)).

---

## 3. EpochState, snapshot, byte sizes

### 3.1 Types and numbering

- **IDs** are `u32`, with `u32::MAX` as the NONE sentinel. Allocation fails hard at `min(max_domains, u32::MAX - 1)`
  (F9).
- **Version = merge counter** (`u64`, F13). S_0 is the state after initial admission.
  - **Merge k+1** consumes one cut and turns S_k into S_{k+1}.
  - Its P1-P3 run on EpochState at version k.
  - Its T4/T5/T6 stamp merge epoch **k+1**.
  - Its P4 publishes S_{k+1}.
- **Job visibility.** A job dispatched while S_v is current has `v0 = v`, and only a merge k+1 with k >= v0 can merge
  it. A native with merge epoch e is visible in S_v iff e <= v.
- **Epoch storage.** ledger6 stores merge epochs as u48, so P3 preflight refuses k+1 >= 2^48 (`stop_reason
  capacity`). ID ranges:
  - `W_k: u32` is the first unassigned ID in S_k.
  - P0 is the protected initial prefix (IDs < P0 never transfer), as in `ledger.rs:168-175`.
- **Buckets.** A bucket is `(Phase, owner: u32)`, interned to `BucketId: u32` in P3 in ID order of first appearance
  (A3). Arbitrary Route masks are interned the same way. The interner is not saved: restore rebuilds it in ID order, so
  it is identical (test `a3_route_mask_interning_roundtrip`).

### 3.2 `EpochState` (coordinator-owned; the only mutator)

| Field | Type | Bytes per ID |
|---|---|---|
| `store.domains` | chunk tree of `CompactDomain<15>` (§3.3) | 96 |
| `ledger` | ledger6 chunk tree (64 Ki entries per chunk) | 8 |
| `exact` | 4,096 digest-sharded hashbrown tables `u64 -> u32` + overflow (F3); a resize touches one shard | 19-39 (load factor 7/16-7/8) |
| `nodes` | chunk tree of `u8` flags: sealed, inspected, closed, anchored, residual | 1 |
| `live` | chunk-tree bitset: entry still in the lookup index | 0.125 |
| bits keeper (helper-private) | sealed + live bitsets, <= 3 frozen copies alive (§3.4) | 0.25 + <= 0.75 |
| `edges` | `EdgeStore` (§10.1) | 68-84 [E] |
| `anchors` | `AnchorMap` (§7) | 0 in W2 (initial D-band only) |
| `merged_view` | per bucket: `(u32 id, u8 scope)` of merged natives; built only with a bound G2' flag | 0 in W2; ~5 B per native with G2' |
| `buckets` | interner, orthant, <= 16 hints, tier list and envelopes | ~1,166 buckets [M v2] x ~1 KB |
| `layers` | young tier + <= 11 pattern tiers per bucket (§3.4) | ~51.5 per live ID + tiers pinned by long jobs |
| scalars | `k`, `W_k`, `P0`, counters, frontier policy, binding digest | O(1) |

The fields form disjoint borrow groups. `MergeRead {store, exact, live, buckets, layers}` is borrowed shared by the P2
helpers. `Dispatch {ledger, in_flight, lists}` stays with the coordinator, so refill (T2) can run during P2 and P4
(PRF-4).

Not in EpochState: in-flight results, per-job resolver state, records (streamed to disk), summaries of retired IDs.

### 3.3 Canonical store chunk tree

- **Levels.** A level-0 chunk holds 4,096 `CompactDomain` (393 KB) and is immutable once full. Level 1 holds 1,024
  chunks; the root holds level-1 nodes.
- **Tail.** The open tail is a list of immutable **mini-chunks** of 64 IDs (6 KB each).
- **Per publish,** the coordinator copies only the partial mini-chunk (<= 6 KB) and the mini-chunk list (<= 64
  pointers).
- **Chunk assembly.** When 64 mini-chunks are full, a helper assembles the 393-KB chunk off the coordinator, and it is
  swapped in at the next publish.

Coordinator copy volume [E]: <= ~7 KB per publish, i.e. <= 2 MB/s at 80-250 merges/s. Rev 1's 393-KB tail copy is
replaced (PRF-5). A read `domain(t)` is <= 4 dependent loads from the thread-local roots, with no refcount write.

### 3.4 Snapshot, frozen bits, tiers

**`Snapshot`** (immutable, `Arc<Box<SnapshotInner>>`) holds:
- `version`, `published_len` (= W_version), the store root;
- the bucket table (chunks of 64 `BucketView`s: orthant, hints, tier list, tier envelopes);
- the frozen bits `F_{j(v)}`;
- the `MergedView` root (G2' only).

Publication goes through `SnapshotCell = Mutex<Arc<Snapshot>>`. An inspector locks it once per job start and once per
refresh point.

**Frozen bits (versioned resolver hints; SND-5, IMP-1).**
- The **bits keeper** is one helper thread. It applies each merge's `MergeDelta` (sealed IDs, retired IDs) in merge
  order to private sealed and live bitsets.
- For every j ≡ 0 mod H (H = 1,024 [E]; 4-13 s at 80-250 merges/s), it freezes `F_j`: chunked, copy-on-write per
  8-KiB chunk against `F_{j-H}`.
- S_v carries `F_{j(v)}`, with `j(v) = H·⌊v/H⌋ - H`. While `j(v) < k0`, it carries `F_{k0}`, which is built
  synchronously from the restored `nodes` and `live` at restore or import (version k0).
- The publish of S_v with v ≡ 0 mod H waits for `F_{v-H}` if the keeper is late (counter `hint_wait_ns`).
- Hints are therefore <= 2H merges stale, and a deterministic function of the version.
- `sealed_hint` as a live `Vec<AtomicU64>` (rev 1) is removed. Inspectors never read the coordinator's `nodes` or
  `live`.

**Layers per bucket (PRF-10):**
- (i) **A young tier in ID order.** Full 32-entry blocks are immutable; the partial tail block is copied per publish
  (<= 1.6 KB per touched bucket).
- (ii) **Up to 11 pattern-ordered tiers**, size-tiered with a 4x ratio. These use the W0.4 layout: sorted by
  finite-upper pattern, then by lexicographic lower corner.
- **Block format** (32 entries, 1,648 B [E]):
  - `ids [u32; 32]`;
  - `words [u64; 32]`;
  - 36 comparison lanes `[[u8; 32]; 36]`, with the A5 encoding, escape lists and biased signed D lanes;
  - per-field block min/max, block OR/AND words, a version range, and an escape bitmap and offset.
- **Per tier:** `[min_id, max_id]`, per-block ID and version ranges, and a tier envelope (per-field min/max) for an
  O(1) skip.

**Deterministic tier maintenance:**
- **Promotion**, synchronous in P4 of merge k: when a young tier reaches 4,096 entries, it is sorted into a new pattern
  tier. Entries whose `live` bit is 0 at version k are dropped. P4(k) reads `live` under a shared borrow, and
  P3(k+1) cannot run yet.
- **Compaction:**
  - Planned at every v ≡ 0 mod H, as a pure function of the tier layout of S_v: merge 4 tiers of one size class, with
    a cap of 11 pattern tiers.
  - Run on helpers against the live bits of `F_{v-H}`.
  - Installed exactly at the P4 of merge v+H. That publish waits if a compaction is late (counter
    `compaction_wait_ns`).
- **Retired entries** that are not yet compacted stay in their tiers. They are valid containers: a retired entry is
  contained in a newer live survivor of the same bucket (S1), so lookups need no live bit.
- **Old tiers** stay allocated while any snapshot holding them lives. A 13-s heavy head [M max] pins <= 2 compaction
  periods of tiers (a budget term in §3.6).

### 3.5 Coordinator-side transient structures (bounded)

| Structure | Bound |
|---|---|
| dispatch lists: FIFO cursor, boost list, requeue, deferred FIFO, isolated FIFO | requeue <= in-flight at the last stop + D_max; boost <= 4,096; deferred and isolated counts and max ages reported in heartbeats and meta (§8.4) |
| group job queues | cost-weighted: >= 20 ms of predicted work per group (Route 0.061 ms [M intel], Apply by owner class), <= 64 jobs per thread |
| `in_flight: HashMap<u32, JobMeta{seq, v0, group, t_start}>` | <= I + D_max |
| result queue | 1 GiB; this also bounds finished-but-unmerged results; one item larger than the bound is admitted alone into an empty queue |
| cut | <= 512 results and <= the P2 scratch estimate |
| miss buffers | 32 MiB in RAM per inspector, then spill to `TMP/<run>/spill/` (F19) |
| job-local exact sets | 64 MiB per inspector; beyond that, the tier is disabled for the rest of the job (still sound) |
| P2 scratch | 2 GiB; a cut is split before exceeding it; a single result whose plan exceeds it is planned alone, growing with `try_reserve`; failure takes the RAM-guard path |
| refresher scratch | 4.25 B per ID |
| saver buffers | 512 MiB |
| inflight-journal buffer | 64 KiB; records written immediately with write(2), fsync every 1 s |

`D_max` is the depth bound in force (default `max(3I, cost-weighted depth)`). It is saved as `saved_D`.

### 3.6 Memory budget (replicas included)

Per ID [E unless marked]:

| Term | B per ID | Basis |
|---|---|---|
| domains | 96 | [src] |
| ledger6 | 8 | |
| exact index | 19-39 | sharded; load factor 7/16-7/8 |
| nodes | 1 | |
| live | 0.125 | |
| bits keeper | <= 1.0 | |
| monitor edges | 68-84 | 16.07 edges per domain [M] x 4 B, + 4 B offsets, + <= 1/8 delta at 8 B/edge |
| layers | 26 | 51.5 B x 51.1% live [M] |
| **subtotal** | **219-255** | |
| tiers pinned by long jobs | up to +26 | |
| **with chunk and allocator slack** | **~230-295** | |

| Term | 74.16M IDs (P-IMP import) | 1G IDs |
|---|---|---|
| per-ID state | 17-22 GB | 230-295 GB |
| 11 replicas | 60 GB | 60 GB |
| process base, binary, coordinator | ~5 GB | ~5 GB |
| transient (§3.5 maxima) | ~12 GB | ~12 GB + 4.25 GB refresher |
| **total** | **~94-99 GB** | **~311-376 GB** |
| alternative: 92-B live-only summary slab (if the §0 item 15 gate fails) | +2.8 GB | +47 GB |
| alternative: 176-B all-ID summaries (v3 note as written) | +13 GB | +176 GB |

**Capacity.** Take the 600 GB cap and subtract replicas (60), base (5), transients (16) and a 30 GB restore/compaction
margin; the 50 GB host floor is outside the cap. That leaves room for ~1.66-1.88G IDs at 0.26-0.295 KB/ID [E], below
the u32 cap.

**Rates, bracketed** (PRF-2, IMP-3):
- Assume c ≈ 4.6 ms of native CPU per native [E from M1 run2: 1.66 CPUs / 361 natives/s], with 88 inspectors and
  before resolve costs.
- Native rate: **5.1k/s** at κ 3.75, **13.5k/s** at κ 1.42, and 16k/s at κ 1.17 (provisional).
- Discovery at 2.70 domains per native [M v2]: 50 / 131 / 156M domains/h.
- If pending keeps growing, a 1.7-1.9G wall would be ~10-37 h after the 74M import [E].

The engine does not change pending growth; W4 and the inputs do. Time-to-guard is written into every pilot receipt at
the measured rate. P-CHAIN measures the marginal RSS and the placement penalty beyond socket 1's free memory (§2.6).

---

## 4. ledger6

### 4.1 Encoding (one `u64` per ID)

Bits 63..61 hold the tag. Payload by tag:

| Tag | State | Payload (bits) |
|---|---|---|
| 0 | Pending | attempts u8 (0-7), guard u4 (8-11), last_err class u8 (12-19), dispatch class u8 (20-27) |
| 1 | Reserved | same as Pending (the counters survive requeue and restore) |
| 2 | Native | merge epoch u48 (0-47), residual flag (48), initial-D-band flag (49) |
| 3 | NativeFrontier | merge epoch u48 (0-47) |
| 4 | NativeError | merge epoch u48 (0-47), err class u8 (48-55) |
| 5 | Alias | `to: u32` (0-31) |
| 6 | Exhausted | same as Pending |
| 7 | invalid | restore refuses |

Frontier counts, error text and stats live in the typed record, not in the ledger. The ledger6 merge epoch is the
authority for G2' stamps. The planner reads stamps from the snapshot's `MergedView` (§7), and P1 re-checks them against
ledger6.

### 4.2 Meaning

- **Pending**: admitted, no merged result, not queued. It may transfer if it is live and `id >= P0`.
- **Reserved**: queued, in flight, or requeued after a discarded result. It never transfers (S3).
- **Native**: merged C0 result with 0 frontiers. It is sealed after its edge set is appended.
- **NativeFrontier**: merged C0 result with >= 1 frontier. It stays unsealed forever; its frontier records are
  persisted.
- **NativeError**: merged C2 result. It stays unsealed forever and is never re-dispatched (A2); T11 is designed only.
- **Alias{to}**: responsibility transferred to `to > id` in the same bucket, with verified containment. The edge
  `id -> to` is appended before the seal.
- **Exhausted**: no merged result after the liveness limits, or marked by an operator flag. It is unsealed and not
  dispatched unless resumed with `--retry-exhausted`. It may transfer (T10).

### 4.3 Transition table (exhaustive)

`Ledger6::apply(id, t: Transition) -> Result<(), LedgerError>` is the only mutator. It checks the current tag and
refuses every pair not listed, without mutation (a release check, not `debug_assert`).

| # | From -> To | Who / when | Preconditions (all checked) |
|---|---|---|---|
| T1 | (new) -> Pending | P3 step 1; initial admission | `id == W_k + i`; capacity preflighted |
| T2 | Pending -> Reserved | refill, at any point of the coordinator loop except a save pause | state Pending; `id < published_len` of the current snapshot (so self is a stored container); queued + in-flight < I + D_max |
| T3 | Pending -> Alias{to} | P3 retirement | a P2 `Verified` token for `to ⊇ id` on canonical images (F4); `id >= P0`; bucket(to) == bucket(id); `to >= W_k > id`, with `to` a survivor of this merge and the smallest-position containing survivor (P2's choice) |
| T4 | Reserved -> Native | P3, C0 result, 0 frontiers | `in_flight[id].seq == result.seq`; event parity (§9.1); merge epoch = k+1 |
| T5 | Reserved -> NativeFrontier | P3, C0 result, >= 1 frontier | as T4 |
| T6 | Reserved -> NativeError | P3, C2 result or recurring C3 | as T4, with the C2 parity rule (§9.1) |
| T7 | Reserved -> Reserved (requeue) | P3 bookkeeping for a discarded C1/C3 result, or for a stop that cancels in-flight jobs (via a bookkeeping-only merge, §11.2) | counters per the cause table of §8.4 (host-wide, policy and operator causes change nothing); last_err set |
| T8 | Reserved -> Exhausted | T7 would reach attempts >= 8 or guard >= 3; or at restore, when the journal-derived guard reaches 3 (§8.4) | as T7; journaled |
| T9 | Exhausted -> Pending | restore with `--retry-exhausted` | counters reset; journaled; meta |
| T10 | Exhausted -> Alias{to} | P3 retirement | as T3 (sound: nothing is in flight for an Exhausted ID) |
| T11 | NativeError{allowance} -> Pending | restore with `--retry-errors allowance` and a raised bound allowance | **designed, not built in W2** (a D7 option for the owner; needs a second run per retried source and union-edge semantics) |
| T12 | Pending or Reserved -> Exhausted | restore with `--exhaust-id <id>` | not in flight (restore time); journaled; meta |

Within a run, Native, NativeFrontier, NativeError and Alias are terminal. There is no Reserved -> Pending transition,
neither in a run nor at restore: saved Reserved IDs go to the requeue, deferred or isolated lists (§8.4).

### 4.4 Counters and cross-checks (restore and audit)

- **Per-tag counts** in meta (#Pending, #Reserved, #Native, #NativeFrontier, #NativeError, #Alias, #Exhausted) equal a
  recount.
- **Natives vs records.** #Native + #NativeFrontier + #NativeError equals the record count. The records manifest also
  has per-kind counts (native, frontier, error, partial initial overlap, g2 residual), compared kind by kind (SND-9).
- **Aliases.** #Alias = transfers = alias runs.
- **Reserved bound (F17).** #Reserved <= saved_I + saved_D + saved_requeue + saved_deferred + saved_isolated.
- **Seals (F8).** Sealed ⇔ Native ∨ Alias.
- **Alias rules.** Every Alias has `to > id`, the same bucket, `to < W_k`, `id >= P0`.
- **Merge epochs.** Every native's merge epoch is <= k.
- **Records digest:** a blake3 chain over `(id, tag, distinct_edge_count)` of every merged native, in merge order.
  - The records writer computes it and stores it in its segment manifest and in meta.
  - Restore recomputes it from the edge-log run headers and ledger6 (§11.4 step 6). This is the out-degree check of
    master plan §6, check 5.
- **Edge digest:** a blake3 chain over `(source, n, sorted targets)` per run, folded in P3 and stored per generation in
  meta.

---

## 5. Inspector side

### 5.1 Job lifecycle

A thread of group g:
1. Pops `JobBytes`, clones the current `Arc<Snapshot>` (version `v0`), and copies its roots into thread-local state.
2. Expands the parent image and sets its alloc-meter job slot.
3. Calls the **unchanged** visitor `inspection::inspect(&replica.reducer, &domain, &request, &cancel,
   &InitialOrthants::empty(), &replica.overlap, &mut resolver)` inside `catch_unwind(AssertUnwindSafe(...))`.
   - The resolver lives outside the unwinding frame, so its emitted prefix survives a panic.
   - An empty `InitialOrthants` makes every successor arrive as `Admit` with its full box, so the orthant tier can
     store-verify it (A1).
   - The native work is otherwise identical: `inspection.rs:342, 350` only short-circuit emission.

**Panics.** A caught panic produces a C3 result (`panic = 1`, with emitted and accepted counts). The group is then
marked for **replica rebuild**:
- its threads finish their current jobs;
- its queued jobs are requeued without penalty;
- the replica is rebuilt from the owner programs (87-100 s [M]);
- the group resumes.

A `PoisonError` observed on the group queue, the result channel or any lock is C5 (IMP-10, SND-15).

**`KnownReuse`** (IMP-15). The legacy job-local reuse cache inside `inspect()` emits `KnownReuse{successor,
conditional}`. It names a successor already emitted in this job and carries no domain. The resolver counts it as
emitted and accepted and adds nothing: the earlier emission's resolution already produced the edge.

### 5.2 Resolution pipeline (per `Admit q`, in order)

0. **Image (F1).** `CompactDomain::try_from_domain(q)`; a refusal (coordinate > 65534) is C2 with
   `break_reason = resolver_range`. Then the digest and `DomainPowerSummary::try_new`; an error is C2 with
   `resolver_summary`. Then the `Query` (compact summary, word), signature and bucket key. Everything later derives
   from this one image.
1. **Job-local exact set** (byte-bounded; replaces `MAX_KEYS = 4096`). Digest -> first ordinal, confirmed by image
   bytes. A hit takes the earlier request's resolution. If that request was a miss, this one becomes a Local to it
   (with its q image).
2. **Self-scope.** If `q ⊆` the inspected scope (the job domain, or the residual for a residual job), q resolves to the
   parent. The parent is a `Stored` container, because T2 dispatches only IDs < `published_len`.
   - This produces one self-edge per parent, deduplicated like any other target.
   - It is vacuous for closure (coinductive) but keeps F10's rule "every successor is contained in a recorded target"
     literally true.
   - It amends master plan §3.2 ("no edge"). Self-edges are counted separately and excluded from every edges-per-domain
     gate (IMP-16).
3. **Local.** If `q ⊆` an earlier miss q1 of this job in the same bucket (the 64 most recent misses of that bucket
   are scanned), verify `(JobMiss{q1}, q)` and record `Local{ordinal(q), ordinal(q1), q image}`.
4. **MRU** per (phase, owner) of this job: the 16 most recent verified targets, those sealed in F first (§8.3), plus a
   hash of exact targets.
5. **Helpers and orthants**: the bucket's orthant and hints from the snapshot, store-verified (full orthant, key,
   `rank_contains`, full power bounds: A1).
6. **Tiers**, oldest first: the pattern tiers, then the young tier.
   - Tiers whose envelope cannot contain q are skipped.
   - A SIMD/scalar prefilter on words and lanes, then `verify`.
   - The first verified container wins, subject to the sealed-preference extension (§8.3).
7. **Miss**: `Miss{image, digest, version, ordinal}` goes into the miss buffer.

**Canonical mode** (controls, `--canonical-resolution`; IMP-11). The result must be a deterministic function of
(q, job, S_{v0}), independent of layout, worker count and timing. Rules:
- Tiers 0-2 are unchanged; they are deterministic functions of q and the job.
- MRU and hints are off, so there are no F reads.
- The winner is the minimum ID over all verified stored containers of S_{v0} (store-exact, orthant, every tier).
- Only if none exists: tier 3 (the earliest-ordinal containing miss of this job), then Miss.

Every positive of tiers 1-6 is a `Verified` token (A1). `--epoch-resolve merge` (diagnostic, not bound) ships every
request after tier 2 as a miss.

**D6 hook** (symmetry; off at launch; orchestrator decision 4 as amended).
- D6 is deferred, not refused. Its effect is ~1.18x domains / ~1.16x pending [E], and 0.884x domains on C-5F offline
  [M-off]. Its admission cost on the epoch path is unmeasured: 1.35-1.43 sigma-image lookups per Apply domain; 5.5 on
  average and up to 8 per Route domain [M-off].
- The resolver therefore accepts a request as a small list of candidate images: the image itself, plus its sigma-images
  when a bound `symmetry` flag is on. It runs tiers 1-6 per image.
- A positive on a sigma-image needs the sigma-map certificate that the (still unwritten) certification rule defines. No
  D6 edge is added before that rule exists.
- W2 measures the cost without enabling it: an S4 replay arm reports extra lookups and CPU per request. The test
  `d6_hook_off_no_sigma_lookups` pins the off state.

### 5.3 The verify chokepoint (A1) over containers (SND-3)

```rust
enum Container<'a, const N: usize> {
    /// view = the snapshot store (len = published_len) or, in P2 only, a MergeView (len = W_k + n_s)
    Stored { id: u32, view: StoreView<'a, N> },
    /// inspector only: an earlier miss of this job
    JobMiss { ordinal: u32, image: &'a CompactDomain<N> },
    /// P2 only: a survivor of this cut before ID assignment
    Planned { pos: u32, image: &'a CompactDomain<N> },
}
fn verify<const N: usize>(c: Container<'_, N>, q: &QueryImage<N>) -> Option<Verified>;
```

`Verified { container: ContainerRef, q_digest: u64 }` has a private constructor. The steps are identical for every
variant:
1. **Range:** `Stored`: id < view.len. `JobMiss`: ordinal < the current ordinal. `Planned`: pos < n_s.
2. **Phase and owner equal**, compared explicitly, because `CompactSummary::contains` compares owner but not phase and
   accepts EMPTY first. Bucket interning or partitioning is never the authority.
3. **Raw inclusion** `d.contains(&q.image)` (sufficient, `compact.rs:173-181`) -> accept.
4. **Native predicate.** Otherwise, the native predicate on the canonical image: `d.native_summary()` is recomputed
   (counted as `verify_recompute`), then `.contains(&q.core)`.
   - The WIDE path is a release check.
   - Layer lanes are only a prefilter, never the authority.
   - Exact hits compare digest and image bytes.
   - Orthant shortcuts require `d.is_full_orthant()` and `rank_contains(d.rank(), q.rank)`.

**Where tokens are produced** (exhaustive; `every_positive_tier_uses_verify` mutates each call site):

| Edge or transfer | Container | Stage |
|---|---|---|
| tier 1-6 hits (MRU, orthant/hints, tiers, store-exact) | `Stored` (snapshot) | inspector |
| Local (q ⊆ q1) | `JobMiss` | inspector |
| self-edge | `Stored` (the parent) | inspector |
| exact probe of a miss | `Stored` (MergeView), image-equal | P2 |
| delta container | `Stored` (MergeView) | P2 |
| antichain: non-survivor -> survivor | `Planned` | P2 |
| Local final edge: parent -> res(q1) | `Stored` or `Planned` (res(q1)), with q's shipped image | P2 |
| transfer T3/T10: survivor ⊇ old | `Planned`, with q = old's stored image | P2 |
| anchor edges | per §7 (scope verify / exact cover predicate) | inspector, re-checked in P1 |

P3 converts `Planned` tokens into IDs through the provisional-position bijection (infallible) and applies them. P3
makes no containment decision.

**Guarantee** (restated; IMP-15):
- On the inspector, the target list of `ResultBytes` is built only from tokens.
- Every merge-side edge and transfer holds a P2 token.
- Across the byte boundary, the merge trusts shipped inspector targets **modulo the canary**: >= 1/64 with an
  independent predicate in production (§6.6), and every hit on controls.

### 5.4 Snapshot refresh (A6), event-count only

After every `R_ev = 65,536` emitted events, the resolver swaps to the current snapshot (one `Arc` clone) and records
`(event ordinal, new version)` in the result. It then re-resolves buffered misses against the delta of their bucket:
entries with id >= W_old, using the tiers' ID ranges. This is performance-only; the merge re-checks anyway.
- No time-based trigger exists anywhere on the resolution path.
- Heads of 261k successors [M v2] refresh <= 4 times; 1M-event heads refresh <= 16 times.
- Lockstep and RollingLite disable refresh.

### 5.5 Miss buffer (F19) and allocation

- **Miss buffer:** 32 MiB per inspector in RAM, then append-only spill files per job. Spilled misses are streamed back
  into `ResultBytes` at job end, and spill files are swept at restore.
- **Allocation:** every resolver buffer grows with `try_reserve`. A failure is `break_reason = alloc` (C1).
- **Spill I/O error:** `break_reason = spill_io` (C1). A second spill error in one session stops the run with
  `stop_reason io_error` (§8.4).

### 5.6 Result and record

At job end the resolver sorts and dedups targets, assembles `ResultBytes` (§2.4) and serializes the typed record
(§11.7). The record's 16-B trailer holds `merge epoch u64, distinct_edge_count u32, flags u32`, and P3 patches it at a
fixed offset (IMP-9, SND-16).

Per-thread counters go into the header:
- requests by tier, verify counts, recomputes, candidates, tiers scanned;
- spill bytes;
- native/resolve/serialize ns and cycles;
- alloc growth.

Heartbeat aggregation reads them from merged results only.

### 5.7 Cancellation

`cancel: &AtomicBool` per group, written only by the coordinator on stop or for a replica rebuild. Native cancellation
returns within ~1 ms [M]. A cancelled job yields `error_kind = "cancelled"`, which is C1.

### 5.8 Alloc meter and crash notes (SND-1, SND-6)

- **`alloc_meter`** is a `GlobalAlloc` wrapper around the system allocator, in `crates/rustred-app/src/alloc_meter.rs`
  (unsafe, outside `E`).
  - It keeps exact thread-local live-byte counts for allocations >= 4 KiB; smaller ones pass through uncounted.
  - Each inspector thread publishes its job's growth (live bytes minus the value at job start) to its own padded slot
    whenever the growth rises by >= 64 MiB.
  - Gate: <= 1% more instructions per native at K=1 on the harness; otherwise it switches to sampling.
  - It is engine code of arm S and is counted in S's cost at the owner gate.
- **The heartbeat (1 Hz)** journals to `inflight-<session>.bin` (§11.7):
  - `Start{seq, id, t}` for every job running >= 60 s;
  - `Grow{seq, id, bytes}` for every job whose growth is >= 1 GiB;
  - `End{seq}` when such a job returns (any class);
  - `Progress{k, merged natives}` once per second.
- **Isolated jobs** get a `Start` record written before dispatch.
- **Fatal-signal note.**
  - Each inspector thread keeps a preformatted 32-B `(seq, id)` record in a thread-local.
  - A handler for SIGSEGV (chained after std's stack-overflow detection), SIGBUS, SIGILL, SIGABRT and SIGFPE is
    installed on an alternate stack in `W/placement.rs`. It writes `Fatal{thread, seq, id, signo}` to the journal fd
    with write(2) and re-raises.
  - SIGKILL (RAM-guard hard stop, kernel OOM) cannot be caught; `Grow`/`Start` records cover it.
  - Records written with write(2) survive a process death; fsync (1 Hz) only guards against a host crash.

---

## 6. Merge (P1-P4)

### 6.0 Pipeline

- Merges run one at a time: P1(k+2) may overlap P4(k+1), and P2(k+2) starts only after P4(k+1) has published S_{k+1}.
- Refill (T2) may run at any point except a save pause.
- S5 option (PRF-5): P2(k+2) reads S_k plus merge k+1's survivor list as a temporary block, so P4(k+1) does not gate
  it. The test `pipelined_p2_identical` checks byte identity against the unpipelined merge.

### 6.1 Cut

- **Lockstep:** all B = 64 results of the epoch (B constant, independent of W; IMP-11), in ascending parent ID.
- **Rolling:** finished results in completion order, taken when R = 64 are available or 5 ms have passed since the last
  merge. The cut is capped at 512 results and at the P2 scratch estimate, and its membership (seqs) is journaled.

### 6.2 P1: checks and classification (serial, O(m))

`p1_check(state: &EpochState, cut: Vec<ResultBytes>) -> Checked`. Per result:
- `ledger[parent] == Reserved` and `in_flight[parent].seq == seq`. A mismatch is C5: an ID is requeued only after its
  previous job's result was processed, so no stale result can exist. Results that arrive after a stop's final save are
  dropped unread (§11.2).
- F7 parity per §9.1, and the per-inspection allowances.
- `v0 <= k`; refresh versions monotone and `<= k`.
- Anchors (§7):
  - G2' anchors: Native (or G2Residual) in ledger6 with merge epoch <= v0, and present in `MergedView` at v0.
  - InitialDBand results: anchor < P0 <= node, same bucket, and the anchor itself has no anchors.
- The class per §9.1.

### 6.3 P2: plan (parallel on helpers; reads `MergeRead` only)

`p2_plan(state: &MergeRead, snap: &Snapshot, checked: &Checked, helpers: &Helpers) -> MergePlan`. Work items are per
bucket, and their results are gathered in a fixed order, so the plan does not depend on the helper count.
1. **F1 check (SND-7).** Recompute the digest and bucket key from every shipped 96-B image (misses, Locals, canary).
   A mismatch with the shipped digest or bucket is C5.
2. **Exact probe** of every miss in the sharded exact index (image-confirmed) -> an existing ID (`Stored` token, image
   equal).
3. **Delta check (F12; SND-17, IMP-14).** Scan the bucket's tiers in S_k whose `max_id >= W_v`: the young tier, and
   pattern tiers promoted or compacted after v. Only entries with id >= W_v are examined (per-block ID-range skip),
   and the first verified container wins (`Stored` MergeView token).
   - The check is complete by transitivity. An entry retired and dropped before k was contained in a newer live
     survivor of the same bucket, which is itself in [W_v, W_k).
   - The delta length per miss is recorded.
4. **Dedup and antichain** over the remaining misses of the bucket, with `Planned` tokens.
   - Equal images merge (digest + image).
   - A miss survives iff no other miss strictly contains it and no miss with an equal native summary has a smaller
     canonical key (digest, then image bytes).
   - When a bucket has m > 64 misses, containment pairs are found with the SoA kernel over a temporary block of the
     cut's misses.
   - A non-survivor resolves to the containing survivor with the smallest provisional position.
   - The result is independent of miss order (tested).
5. **Provisional survivor order:** (position of the first requesting parent in the cut, ordinal within that parent).
6. **Reverse sets:** every `live` ID of the bucket contained in a survivor (tiers of S_k plus the delta), block-parallel
   inside a bucket (A4).
   - For each such old ID, P2 records the smallest-position containing survivor and its `Planned` token: a T3/T10
     candidate (PRF-5: the F4 re-verify moves from P3 to P2).
7. **Local finals:** for each Local, compute `verify(res(q1), q)` -> token. A failure is C5.
8. **Canary** re-verification (§6.6).
9. **P3 byte estimate** (IDs, edges, record bytes) for the preflight.

### 6.4 P3: preflight, then infallible apply (coordinator)

`p3_preflight(state: &mut EpochState, plan: &MergePlan) -> Result<Reservation, StopRequest>`.
- It reserves capacity in every arena: chunk trees, ledger chunks, exact shards, nodes, live, edge deltas, the bucket
  interner, the record queue and the journal.
- It checks `W_k + n_s <= min(max_domains, u32::MAX - 1)` and `k + 1 < 2^48`.
- A failed preflight changes nothing logical. The cut's results are discarded as C1 **without counter changes**
  (SND-6, IMP-10), and the run stops:
  - on the RAM-guard path (§11.3) for an allocation failure;
  - with `domain_allowance` for the domain cap;
  - with `capacity` for epoch overflow.

`p3_apply(state: &mut EpochState, plan: MergePlan, r: Reservation) -> MergeDelta` has no `Result`. A `poisoned` flag is
set when P3 starts and cleared when it ends. A panic inside P3 is caught at the merge loop and handled as C5, and `save`
refuses while the flag is set, so a half-applied merge can never reach disk. Steps:
1. **IDs:** assign `W_k .. W_k + n_s - 1` to survivors in provisional order. Push the image. Then assert (release)
   `image.digest() == key` and do the exact insert (SND-7). Then ledger T1 (Pending, dispatch class from the policy
   hook), nodes 0, live 1 and bucket interning.
2. **Retirement and transfers (A4).** For each `old` in P2's reverse sets with `live[old]`:
   - clear `live[old]`;
   - if `ledger[old]` is Pending (T3) or Exhausted (T10), and `old >= P0`: apply the transition with P2's token, append
     the alias run `old -> s`, and seal `old`;
   - otherwise (Reserved, terminal, protected) only the lookup entry retires.
3. **Results, in cut order:**
   - the final edge set = inspector targets + P2 tokens (miss resolutions, Local finals) + anchor targets, sorted and
     deduplicated;
   - patch the record trailer (merge epoch k+1, `distinct_edge_count`);
   - `EdgeStore::append_run(source, &sorted_targets)`, which asserts strictly increasing targets and an unsealed
     source;
   - fold `(source, n, targets)` into the edge digest and `(id, tag, n)` into the records digest;
   - ledger T4/T5/T6; nodes `inspected`; seal iff T4; append the record bytes to the record writer (merge order);
     update counters.
4. **Requeue bookkeeping** (T7/T8) and `in_flight` removal; discarded results are journaled with their applied
   counter deltas (`Discard{seq, parent, class, cause, d_attempts, d_guard}`).
5. **Advance:** `k += 1`; `W_{k+1} = W_k + n_s`; write the journal cut header; send the `MergeDelta` (seals,
   retirements, survivors per bucket) to the bits keeper and to P4. **A `MergeBoundary` is available from here** (§11.2).

Budget [E, gated at S4; PRF-5]: <= 10 us per merged native at 74M, covering decode, edge appends, exact inserts,
transfers, record append, tail copy and journal. The microbenchmark `p3_per_native_cost` runs from S2.

### 6.5 P4: layers and publish

Helpers:
- append survivors to young tiers (tail block copy);
- promote young tiers that reached 4,096 entries (synchronous);
- recompute orthants (full orthants only) and hints;
- append to `MergedView` (G2' only).

At v ≡ 0 mod H they install `F_{v-H}` and the compactions planned at v-H, and plan new compactions. The coordinator
then publishes S_{k+1} (new root, touched bucket-table chunks, `published_len = W_{k+1}`).

If P4 or a compaction fails to allocate, the previous snapshot stays published and the run takes the RAM-guard path.
A `MergeBoundary` already exists after P3, so the save proceeds (SND-11).

### 6.6 Canary and poison (F5; SND-4)

**Selection.** Every tier 1-6 hit with `hash(seq, ordinal) & 63 == 0` in production, and every hit on controls (JobBytes
flag canary-all), ships `(ordinal, tier, target, q image)`.

**Check.** P2 re-verifies these hits through an **independent predicate path**: it expands the target's stored image and
q to `Domain`s and decides containment with the oracle's `verify_closure::lattice::Cell::contains` (oracle 63771a50,
`lattice.rs:287`). That path is independent of `compact.rs` and of the layer lanes; phase and owner are compared
explicitly.

**Mismatch = C5.** The run exits 70 with no new generation and a diagnostic dump: the job, the snapshot version and the
target. It also writes a sticky `poison-<k>.json` in the checkpoint directory, outside the generations: `{k, seq, parent,
target, tier, reason, binary sha256}`.

**Every C5 writes a poison file**, not only canary mismatches. Restore reads every poison file and sets
`engine_certification_void: true` in meta and `result.json` of every later generation. The only way to clear it is
`--clear-poison <file> --f10-receipt <path>`, which names a full-F10 PASS receipt (contract of §16) covering the roots
affected. Tests: `canary_catches_injected_false_hit` (a resolver seam forges a hit) and
`canary_poison_sticky_across_restore`.

### 6.7 Merge-helper count and merge critical path [E]

**Helper load.** It is dominated by P2 reverse sets: r_miss 3.21 [M] x 34.9 us per reverse check at 74M = 112 us per
native. With the reverse exponent 0.50 [M thinning], 1G costs (1000 / 74.16)^0.5 = 3.67x, i.e. 411 us per native.
Everything else in P2/P4 is < 20 us per native [E]; this includes the canary at ~0.4 per native and the Local finals.

| Native rate (bracket, §3.6) | 74M, thread factor 1.0 / 2.4 | 1G, thread factor 1.0 / 2.4 |
|---|---|---|
| 5.1k/s (κ 3.75) | 0.6 / 1.4 CPUs | 2.1 / 5.0 CPUs |
| 13.5k/s (κ 1.42) | 1.5 / 3.6 CPUs | 5.5 / 13.3 CPUs |
| 16k/s (κ 1.17, provisional) | 1.8 / 4.3 CPUs | 6.6 / 15.8 CPUs |

The thread factors 2.0-2.9 of W0.4 are contaminated [M].

**Critical path.** Merges at R = 64 run at 80-250/s. P3 at <= 10 us x 64 natives is 0.64 ms per merge. Merge duty
<= 50% at 250 merges/s needs P1 + P3 + publish <= 2 ms per merge.

**Bucket skew** (30% vs 2.9-4.8% are both in play, critique §2.3) is measured offline from `streams-g7-v4.jsonl` before
S4 fixes the P2 granularity. If one bucket takes > 10% of misses, block-parallel P2 inside buckets moves from S5 into
S4.

**Decision:**
- 9 helpers at launch.
- The coordinator raises `helper_saturation` when P2 wall exceeds 50% of merge wall.
- The operator re-splits at the next resume, binding-free, up to 17 helpers (one inspector group fewer).
- S4b exists to push the 1G figure back under 9.
- The recorder writes: merge interval p50/p99, merge latency p99, cut-size distribution, P2 and P3 µs per native, and
  hint and compaction waits.

---

## 7. Anchors (initial D-band now; G2'-ready) (SND-2, IMP-7)

**`AnchorMap` record:** `node: u32`, `kind` (InitialDBand | G2Native | G2Residual), `dispatch_version: u64` (= v0), a
scope descriptor, and the anchor list `(anchor: u32, stamp: u64 or NONE)`.

**Rules:**
- **R1. No anchors for IDs < P0.** G2' never plans anchors for an ID < P0. IDs < P0 are always inspected whole; they
  can never be InitialDBand nodes, since that kind requires node >= P0.
- **R2. InitialDBand** (today's `--reuse-initial-d-bands`, `W/initial_overlap.rs`):
  - Planned on the inspector from the group's replica of `InitialOverlapIndex`, inside the unchanged `inspect()`.
  - The result carries `InitialOverlapScope{anchor_id, cut}`; P3 appends `node -> anchor` and sets the initial-D-band
    flag.
  - Conditions: anchor < P0 <= node; same bucket; the anchor has no anchors (R1); no stamp.
  - P1 re-validates all of this on every result.
- **R3. G2Native / G2Residual** (W4; decision 7 admits merged, validated G2' residual records as anchors):
  - Planned on the inspector from the **`MergedView` of S_{v0}**: per bucket, `(id, scope kind)` of every native with
    merge epoch <= v0, appended in P4. Visibility in S_{v0} implies the stamp bound; ledger6 is never read by the
    planner.
  - Admissible scopes by anchor kind:
    - a plain Native lends its full domain;
    - a G2Residual native lends its full domain, because its own anchors were merged before its dispatch;
    - an InitialDBand native lends **only its inspected low-D slice**, because its high slice is delegated to an anchor
      < P0 that may merge later.
  - Exact cover `Q ⊆ residual ∪ anchor scopes` is decided by `lattice::Cell::covered_by_union` (oracle branch) or by
    the one-point native authority, never by C2 code alone. P3 appends one edge per anchor.

**Well-foundedness** (replaces rev 1's acyclicity text). A node's responsibility (its domain) is discharged by
inspecting its inspected scope plus the admissible scopes of its anchors.
- G2' anchor links strictly decrease the merge epoch: anchor <= v0 < merge epoch of the node.
- InitialDBand links end at an anchor-free ID < P0 that inspects its whole domain (R1).
- A slice delegated by an InitialDBand native is never lent again (R3).

Every delegation chain of a point therefore ends at an inspection within finitely many steps, so there is no
responsibility cycle. Coinductive cycles of closure edges remain allowed. SND-2's construction A0 -> B -> A0 is
excluded twice: by R1 (A0 < P0 is never anchored) and by R3 (B would lend only its low slice).

**Validators** (P1 and restore; §11.4):
- InitialDBand: anchor < P0 <= node, same bucket, anchor anchor-free.
- G2': `stamp == ledger[anchor].merge_epoch <= dispatch_version < ledger[node].merge_epoch`; the anchor is Native or
  G2Residual; if the anchor is an InitialDBand native, the recorded scope is its slice.
- No anchor record has a node < P0.
- Every anchor edge is present.

**Mutations required before any W4.2 code** (handoff §0.1 item 6):
- residual shrunk by one point;
- anchor stamp > dispatch version;
- anchor not Native;
- `g2_anchor_on_dband_partial_of_its_own_anchor` (the planner and the validator must both refuse the A0 -> B -> A0
  construction);
- an anchor record on a node < P0.

In W2 only `MergedView` and the test `anchor_plan_uses_snapshot_epochs_only` land: a native merged after v0 is never
chosen, which is exercised with a test-only planner seam.

---

## 8. Schedules and dispatch

### 8.1 Lockstep, Rolling, RollingLite

- **`Lockstep{B = 64, depth 1 | 2}`** (controls, oracle, MVP-B). B is a constant independent of W.
  - Dispatch the B lowest Pending IDs in dispatch order, wait for all of them, and merge one cut in parent-ID order.
  - Promotion and compaction are deterministic (§3.4) in every mode.
  - Closure refresh is forced only at deterministic points (drain, final). A6 is off.
  - Depth 2 overlaps the dispatch of epoch k+1 with merge k: epoch k+1 jobs see S_k. It is deterministic, because
    every resolver input is versioned (§3.4, §12).
  - `--canonical-resolution` (§5.2) makes the winner layout-independent.
  - The S2/S4 identity oracle is byte identity across W6/W12/W24 and across pause points:
    - CP6 sections byte-identical;
    - records identical except timing fields (`seconds`, ns and cycle counters, as in the strict mode of
      `compare_walk_records.py`).
- **`Rolling{R = 64, T = 5 ms, cut <= 512}`** (production): cuts per §6.1; A6 on; background saves.
- **`RollingLite`** (the owner-gate skeleton, S4): Rolling without A6 refresh and without background saves.
  - It is a rolling schedule. At the owner gate it is soundness-gated by audit and full F10 at W >= 24 (n >= 2).
  - It never runs in production without S6's replay oracle (SND-8).

### 8.2 Enqueue depth and Reserved-at-enqueue (F11, F17)

- Pending -> Reserved happens when an ID enters a group queue (T2).
- `D_max = max(3I, cost-weighted depth)` [E; W3.1 tunes it by M-mist and transfers per native]. Cost-weighted: a group
  queue is refilled until its predicted work is >= 20 ms (Route 0.061 ms, Apply by owner-class mean from the c_K1
  table), with <= 64 jobs per thread.
- Before a scheduled background save, the coordinator tops the queues up to cover the expected pause.
- Route micro-batching (k <= 8) is an S4 option.
- D, I, the split and the schedule are not bound (A7).
- Restore checks the F17 bound with the **saved** values, never the new ones.

### 8.3 Dispatch order, priority hooks, first-found bias

`trait DispatchPolicy { fn refill(&mut self, view: &DispatchView, want: usize, out: &mut Vec<u32>);
fn on_merge(&mut self, delta: &MergeDelta); fn on_refresh(&mut self, cones: &ConeReport); }`. The coordinator calls it at
any point except a save pause.

Default `Fifo` (the W0.8 result: no order adopted [M knobs]):
- (0) **Isolated slot:** if no isolated job is in flight and the isolated FIFO is not empty, dispatch one.
- (1) Requeued Reserved IDs with attempts < 2 and guard == 0.
- (2) The boost list (empty until the W3.4 cones).
- (3) A monotone ID cursor over Pending.
- **Deferred share (SND-6):** at least 1 of every 64 refilled IDs comes from the deferred FIFO (attempts >= 2) while it
  is not empty.
- **Age bound:** at least half of every refill comes from (3).

At restore, and after T9, the cursor is set to the minimum Pending ID (SND-14). **Refill progress assertion:** if
Pending > 0, nothing is in flight and no ID is dispatchable, the run stops with exit 70 (`stop_reason
dispatch_stall`, poison file).

Any other order is a W3.1 A/B designed around peak pending: support-then-volume raised peak pending by +12% to +193%
[M knobs].

**First-found bias** (critique SOUND-2). The resolver prefers sealed and older containers without scanning for them in
general:
- (a) tiers are scanned oldest first;
- (b) MRU tries targets that are sealed in F first;
- (c) when the first verified tier container is not sealed in F, the scan continues for at most 64 more candidates and
  takes the first F-sealed verified one if found.

All three are performance-only: any verified container is sound. Gate (S4): per-root open-cone counts at matched
natives no worse than canonical mode, with newest-first as the negative control.

### 8.4 Liveness: attempts, guard, isolation, crash attribution (SND-1, SND-6, IMP-6)

**Counter effects by cause** (applied by T7 at stop or discard time, or at restore from the journal):

| Cause | attempts | guard | Notes |
|---|---|---|---|
| cooperative or operator stop; frontier, error or exhausted stop; preflight discard; domain or event allowance; replica-rebuild drain | +0 | +0 | never a penalty |
| RAM-guard stop **not attributed to the tree** (`host_memory_reserve` with `own_memory_signal` false or absent) | +0 | +0 | foreign memory pressure |
| C1 `spill_io` | +0 | +0 | a second one in a session stops the run with `io_error` |
| C1 `alloc` (resolver `try_reserve` failure) | +1 | +0 | |
| C3 panic (first) | +1 | +0 | a recurrence is C2 |
| RAM-guard stop **attributed to the tree** (`aggregate_rss_soft_limit`, `own_swap_growth_sustained`, or a host-wide reason with `own_memory_signal` true) | +0 | +1 per suspect | suspects: in-flight jobs whose metered growth is >= 25% of the tree's RSS growth over the guard window, else the top-1 by growth; plus every job not returned within 5 s of the cancel. Applied whether or not the job returned. Queued, never-started jobs are never suspects |
| abnormal exit (SIGKILL, kernel OOM, abort, stack overflow) | +0 | +1 per suspect, derived at restore | suspects in priority order: the `Fatal` note's job; else the isolated job in flight; else jobs with an unreturned `Grow`; else jobs with an unreturned `Start` |

**Journal-derived guard.**
- Restore scans the `inflight-<session>.bin` files of the session that wrote the generation and of every later
  session (meta `accounted_session_watermark`; earlier sessions are already folded into the saved counters). A session
  without `CleanExit` is abnormal.
- For each ID: `guard_eff = saved guard + number of abnormal sessions in which the ID was a suspect`.
- `guard_eff >= 3` gives T8 at restore. The increments reach ledger6 at the next save, which advances the watermark.
- Because the counters are derived from durable per-session files, a crash that happens before any new generation still
  counts.
- Attribution limit: an innocent isolated job can be blamed if a foreign kernel OOM kill hits while it runs. The 50 GB
  host floor normally stops us first, and `--retry-exhausted` exists.

**Limits:**

| Condition | Effect |
|---|---|
| attempts >= 2 | deferred (guaranteed share, §8.3) |
| guard >= 1 | isolated: suspects run one at a time, alongside normal work; an isolated job's `Start` is on disk before dispatch, so a repeat crash names it |
| attempts >= 8 or guard >= 3 | T8 Exhausted, then `stop_reason exhausted_stop` (D7 default) |

**Unattributed loops.**
- If the most recent 2 sessions after the last generation both ended abnormally, **and** each either had no attributed
  suspect or merged 0 natives (`Progress` records), the engine refuses to start.
- The refusal exits 4 with `stop_reason abnormal_exit_loop` and names every journaled suspect. The operator overrides it
  with `--accept-abnormal-exits` or `--exhaust-id <id>`.
- The ops supervisor keeps its own guard (2 consecutive attributed zero-progress RAM-guard stops) as a second line.

**Operator flags at resume** (all journaled and in meta):
- `--exhaust-id <id>` (T12);
- `--retry-exhausted` (T9);
- `--keep-exhausted`: continue past an `exhausted_stop`. Exhausted IDs stay, are never dispatched and may transfer by
  T10. The run stops again only on a new Exhausted ID.

**Bounds** (the rev 1 claim "requeue <= in-flight + D" is replaced):
- queued + in-flight <= I + D_max;
- requeue <= in-flight at the last stop + D_max;
- deferred <= #IDs with attempts >= 2, and isolated <= #IDs with guard >= 1; both counts and their maximum ages in
  merges appear in heartbeats and meta.

**Drills (S3):**
- RAM-guard soft stop (`fast_oom_head_exhausted_within_3_stops`: a seam head allocates past the soft limit within 10 s);
- hard SIGKILL and abort (`crash_loop_head_ends_exhausted`: a seam `RUSTRED_EPOCH_CRASH=<id>:<abort|kill|alloc>`
  makes one head abort, SIGKILL or over-allocate the process; after <= 3 relaunches the ID is Exhausted while other
  work progresses);
- `ram_guard_innocent_jobs_not_penalized`;
- `host_wide_unattributed_stop_no_penalty`;
- `nonreturning_culprit_counter_persisted`;
- `deferred_ids_progress_under_growing_pending`.

All run on FG, X and C-5F.

---

## 9. Error and frontier policy (A2, A10) with F7 reconciled

### 9.1 Result classes (decided in P1)

`consumer_stop` means the resolver returned `Break`. Its class follows `break_reason` and is never inferred from
`error_kind` alone (IMP-10).

| Class | Sources | Merge | Ledger | Run |
|---|---|---|---|---|
| C0 Ok | `error_kind none`, `break_reason none` | yes: record, edges | T4 or T5 | continue; T5 under `stop` -> frontier stop |
| C1 transient | `cancelled` (coordinator cancel for a stop, the RAM guard or a replica rebuild); `break_reason spill_io` or `alloc`; P3 preflight discard | no (discard, journaled) | T7 per the §8.4 table | continue (a second `spill_io` in the session -> `io_error`) |
| C2 deterministic | `native_failure`; `conversion`; `break_reason resolver_range`, `resolver_summary`, `allowance` (per-inspection allowance crossed or counter overflow); a recurring C3 of the same class | yes: record with error class and message, successors of the emitted prefix, edges | T6 | save + stop (`error_stop`), both policies |
| C3 unknown | caught panic (`panic = 1`); any unclassified kind | no (first time) | T7 (attempts +1) with `last_err` | retry once; the same class again -> C2 |
| C4 frontier | a C0 result with frontiers | as C0 | T5 | `stop`: save + stop (`frontier_stop`); `record`: continue |
| C5 engine-fatal | P1 protocol violation; F1 digest or bucket mismatch; canary mismatch; a P2 Local-final or token re-verify failure; a P3 internal failure; `PoisonError`; dispatch stall | no | none | no new generation; poison file; exit 70 |

**Event parity (F7 as amended):**
- **C0:** `emitted == accepted == stats_events`.
- **C2 by break:** `accepted == emitted - 1`; the breaking event is emitted but not accepted. The relation between
  `stats_events` and these counts is pinned by the test `consumer_stop_parity` (whether the solver counts the breaking
  event), and P1 checks exactly that relation.
- **C2 from `native_failure` or `conversion`:** `emitted == accepted == stats_events` over the prefix.
- **C2 from a recurring panic:** the record has `panic = true`, no successors and no edges, and is parity-exempt
  (stats are unavailable after unwinding). The audit accepts panic records as parity-exempt errors. NativeError never
  seals, so a missing or partial successor set cannot produce a false closure.

A2 is kept: deterministic errors are persisted as unsealed NativeError and never re-dispatched. A resume after an error
stop continues the rest of the walk and stops again only on a new error, as the ops frontier-stop drill does (13 stops
for 13 frontier records [M]).

A NativeError caused by the per-inspection allowance is **permanent within semantics 3**, and its ancestors cannot
certify. This is recorded for the owner, and T11 (§4.3) is the D7 option that would lift it.

### 9.2 Stop reasons and exit codes

Every resumable stop exits **4** with `stop_reason` in `result.json` and the journal:

| `stop_reason` | Meaning |
|---|---|
| `paused` | cooperative stop (stop file, SIGINT) |
| `ram_guard` | RAM-guard soft stop |
| `frontier_stop`, `error_stop`, `exhausted_stop` | policy stops |
| `domain_allowance`, `event_allowance`, `capacity` | allowance or capacity reached |
| `io_error` | second spill I/O error in a session |
| `abnormal_exit_loop` | a refusal at start (§8.4) |
| `drained_uncertified` | drained with frontiers, errors or Exhausted IDs, with the monitor unavailable, or with certification void |

- A drained walk with nothing open exits **0** with `drained`.
- Engine-fatal exits **70** (today's `InternalInvariant`, `crates/rustred-app/src/cli/error.rs:31`) with a poison file.

The supervisor auto-resumes nothing. When the operator relaunches without the matching flag, it refuses:
- after `frontier_stop`, `error_stop` or `exhausted_stop` (D7 **default**, pending the D-session; IMP-16);
- after exit 70 without `--accept-poison`; the resume is then allowed, but certification stays void.

### 9.3 Frontier policy

`--frontier-policy stop | record`, default `stop` (A10), bound (A7). Input frontiers at initial admission stop the run
before the first dispatch under `stop`. `--unbounded-work` no longer makes frontiers non-halting for `epoch`.

---

## 10. Closure monitor

### 10.1 EdgeStore

- **Persisted run log** (the `edges` segments, §11.1):
  - Per merged native, one run `(source u32, n u32, targets u32 x n)`.
  - Per alias, one run of length 1.
  - Anchor targets sit inside the anchored node's run.
  - Size: 4 B per edge + 8 B per run. The run header is the out-degree.
  - Written by the saver into the open segment and then dropped from RAM.
- **Monitor graph** (incoming by target, RAM only):
  - Shards of 2^20 target IDs. Each shard has an immutable `Arc<Box<Csr32>>` base (u32 offsets relative to the shard,
    u32 sources) and an append delta of `(u32 target-local, u32 source)` pairs in immutable 64 Ki-pair chunks.
  - A shard whose delta exceeds 1/8 of its base is folded into a new base by a helper and installed at a merge
    boundary; the peak extra memory is one shard (~70 MB at 1G [E]).
  - No u32 edge-log cap (F14): offsets are per shard.
- **Restore** builds the shards in two passes over the log: count per target, then fill. No `(target, source)` pair
  buffer is used (PRF-8).
- There is no per-source dedup map: each source's edges are appended once, sorted and deduplicated, in P3.

### 10.2 Refresher protocol

- **Trigger:** every 2^22 merged results, or when the saver asks at a final save. The trigger counts events; there is no
  wall-time throttle.
- **Input:** at a merge boundary the coordinator hands the refresher a `FrozenPrefix{sealed: bitset copy, shard base
  Arcs, delta chunk Arcs up to the edge watermark, version}`.
- **Computation:** the refresher computes the blocked set by reverse BFS from unsealed nodes over incoming lists and
  returns closed bits.
- **Install:** the coordinator ORs the closed bits into `nodes` at the next merge boundary and journals
  `RefreshInstall{version, refresh id}` (IMP-1). Closed bits only grow.
- **Unavailability:** a refresher allocation failure marks the monitor **unavailable** before any later seal counts as
  closure (F15).
  - Monitor availability is required for exit 0 `drained` and for engine certification (§16; SND-14).
  - An import is refused if the CP5 monitor was ever unavailable (§11.6).
- **Saves** persist the last completed result. Restore accepts stale-but-valid closed bits: closed ⇒ sealed, and there
  is no edge from a closed node to an unclosed one, because sealed sources never gain edges.
- **S2** uses the legacy `descendant_closure::Tracker`, with forced refreshes only at drain and final (IMP-13; test
  `s2_closure_forced_refresh_only`). The off-coordinator refresher arrives in S5.

### 10.3 Cones and priority feed (hook, W3.4)

`ConeReport` holds the open-cone count per root, for roots whose cone is below a work cap. For those roots it also holds
the Pending IDs in the cone (<= 4,096 in total), which feed the boost list. Edges whose target is closed can be dropped
from the monitor graph, never from the persisted log.

---

## 11. CP6

### 11.1 Sections

| Section | Kind | Content |
|---|---|---|
| `latest.json`, `previous.json` | manifest | format `RUSTRED-WALK-CP6`, schema 6, semantics `{epoch: 3}`, self-digest (#1), section presence bound to the policy (#1), segment list with blake3 per segment, segment counts bounded (#2/#13), `effective_interval_seconds` (#10), session id and accounted-session watermark |
| `meta-<G>.json` | rewritten | schema of §11.7 |
| `ledger6-<G>.bin`, `nodes-<G>.bin`, `live-<G>.bin`, `anchors-<G>.bin` | rewritten | layouts in §11.7; blake3 per 64 Ki-entry chunk and in total |
| `domains-<G>.seg`, `edges-<G>.seg`, `records-<G>.seg`, `journal-<G>.seg` | append-only, **sealed per generation** | the data appended between the watermarks of generations G-1 and G; blake3 in the manifest (as CP5 `plan()`, `checkpoint.rs:184-199`) |
| `*-open-<session>.seg` | open segment | never referenced by a manifest; restore ignores and sweeps it (SND-10, IMP-9) |
| `inflight-<session>.bin` | per session, outside generations | crash attribution (§8.4, §11.7) |
| `poison-<k>.json` | sticky, outside generations | §6.6 |

Not saved: layers (rebuilt deterministically), exact index, interner (rebuilt in ID order), hints and frozen bits
(`F_{k0}` rebuilt), replicas, in-flight results.

### 11.2 Save: the cut while merges continue

`cp6::Store::save(&mut self, state: &EpochState, at: MergeBoundary<'_>, kind: SaveKind) -> Result<SaveReceipt, String>`.

**Boundary.** `MergeBoundary<'a>` borrows `&'a EpochState`. It is issued after P3 step 5, or by a **bookkeeping-only
merge**: a merge with n_s = 0 whose cut holds only the discarded results of a stop, which applies T7/T8, journals and
issues a boundary (IMP-8).

**What the token blocks.** While the token lives, no `&mut EpochState` exists. T2, T7, refresher installs and P3 are
blocked; P4 of the last merge may finish, because it only reads (SND-10).

Steps:
1. **Seal the open segments:** fsync, blake3, rename to the generation's names; the watermarks are recorded.
2. **Merge pause:** helpers become chunk writers and stream `ledger6`, `nodes`, `live`, `anchors` and `meta` from the
   live chunk trees.
3. **Install:** fsync; install `latest` before `previous` (#9); resume. Cleanup errors after a durable publish are
   reported, not fatal (#6); staging leftovers are swept on open (#8).
4. **Closed bits** come from the last completed refresh. A final drain save forces a synchronous refresh, which is
   cancellable and counted inside the save timer (#11).

**Inspectors during the pause.** They keep running only on their queued work (>= 20 ms per group, plus the pre-save
top-up of §8.2). Idle inspector time during the pause is **counted in the save duty**.

**Late results.** Results that arrive after a stop's final save are dropped unread; the saved state holds their IDs as
Reserved (test `late_result_after_stop_save`).

**Budget [E].** The rewritten sections are 9.3 B per ID: 9.3 GB at 1G, 0.7 GB at 74M.
- At a write rate of 1-2 GB/s the pause is 5-10 s at 1G; with 2-4 h intervals the duty is <= 0.14%.
- Gate (S3/W3.3): save duty <= 2% of wall, pause measured at the P-IMP scale and on P-CHAIN.
- Fallback if the extrapolation fails: a dirty-chunk ledger, with the caveat that random ledger touches make most
  chunks dirty over hours [E].

The change stamp covers every persisted field; a skipped save resets its timer (#3-#5); the interval stretch is capped
(#12); inventory checks do not depend on closure availability (#7).

### 11.3 Stop paths

| Path | Dispatch | In-flight results | Grace | Save | Exit |
|---|---|---|---|---|---|
| Cooperative (stop file with any non-RAM-guard reason, SIGINT) | stops | finished ones merge; the rest are cancelled after grace and requeued by a bookkeeping-only merge (no counter change); `uncommitted` receipts with `resume_reinspects_unfinished_part: true` | 60 s | yes | 4 `paused` |
| RAM-guard soft (stop-file reason `aggregate_rss_soft_limit`, `own_swap_growth_sustained` or `host_memory_reserve`) | stops | all cancelled at once; bookkeeping-only merge; guard +1 to suspects only if the stop is tree-attributed (§8.4); jobs not returned within 5 s are saved as Reserved (and are suspects if attributed) | none (F18) | yes, streaming writers only | 4 `ram_guard` |
| Frontier / error / exhausted stop | stops after the P3 that merged it | as cooperative | 60 s | yes | 4 with the reason |
| Hard: SIGKILL (RAM-guard hard, kernel OOM), abort, stack overflow | - | lost | - | none | previous generation valid; restore derives suspects from `inflight-<session>.bin` (§8.4) |
| Engine-fatal (C5) | stops | discarded | none | **no**; poison file | 70 |

**Stop-file contract** (IMP-8; a small ops change in its review round):
- The supervisor writes `{"reason", "unix_time", "family_closure_claim": false, "ram_guard": {"host_wide": bool,
  "own_memory_signal": bool}}`, where the `ram_guard` object is present only for RAM-guard reasons.
- The engine reads the file when it appears (bounded read, 4 KiB):
  - RAM-guard reasons take the RAM-guard path. The stop is attributed iff the reason is `aggregate_rss_soft_limit` or
    `own_swap_growth_sustained`, or `own_memory_signal == true`.
  - Every other reason, and a missing or unparseable file, takes the cooperative path.
- Test `ram_guard_stop_reason_parsed`.

### 11.4 Restore and validators

`cp6::restore(dir: &Path, request: &OwnerDomainWalkRequest) -> Result<(EpochState, RestoreReport), String>`. Phases run
in parallel where noted. The report gives each phase's time and VmHWM, and the RSS at the end of restore.
1. **Manifest:** bounded bytes, self-digest, format/schema/semantics, binding digest (§11.5), owner digests.
2. **Poison files** -> `engine_certification_void`.
3. **Inflight journals** of the session that wrote the generation and of every later session -> abnormal sessions, suspects and journal-derived
   guard (§8.4). This applies T8 and T12, and refuses with `abnormal_exit_loop` where §8.4 says so.
4. **Domains:** read, blake3 and decode in one parallel pass into the chunk tree. The exact index is rebuilt sharded;
   duplicates are refused (E3).
5. **ledger6, nodes, live, anchors:** digests; only tags 0-6; per-tag counts equal meta; F8; the Alias rules of §4.4;
   NativeFrontier, NativeError and Exhausted unsealed; F17 with the saved bounds; the kind-specific anchor rules of §7.
6. **Edges:** read and blake3 in one pass, with the two-pass monitor build. Checks:
   - every merged native (Native, NativeFrontier, NativeError) has exactly one run;
   - every Alias has exactly one edge, to `to`;
   - every anchor edge is present;
   - every run target is < W_k;
   - the records digest recomputed from the native runs' `(id, tag, n)` and ledger6 equals meta and the records
     manifest (out-degree check; SND-9);
   - the edge digest equals meta.
7. **Aliases** created since meta's alias-verified watermark are re-verified in parallel (F4). The watermark advances at
   the next save; the full pass is in the audit tool (PRF-8).
8. **Records segments:** blake3 only for segments written since the records-verified watermark. Records are not
   otherwise read at restore; the full pass is in the audit tool.
9. **Tracker checks:** closed ⇒ sealed; no edge from a closed node to an unclosed one.
10. **Roots map** (meta: query digest -> root ID): re-verify each query image ⊆ its root's domain (verify, same bucket)
    and root < W_k (SND-9).
11. **Layers:** young tiers from IDs in ID order and deterministic pattern tiers; interner; orthants; hints; `F_{k0}`.
    Replicas are prepared in parallel (§2.5).
12. **Dispatch lists:** saved Reserved IDs go to requeue (non-suspects), deferred or isolated (suspects); cursor = the
    minimum Pending ID; open-segment and spill-file sweep.

**Restore time model and gate (PRF-8).** Per-phase rates are measured on P-IMP and on every P-CHAIN link and
extrapolated in IDs and edges. Anchors: CP5 gen 7 took 508.6 s (verify 112.5, decode 125.1, validate 383.5) [M runA].
Gate (C): <= 10 min on the >= 3e8-ID link, extrapolated to 1G <= 20 min [proposal; owner confirms].

### 11.5 Binding (A7)

**Bound** (hashed into the CP6 binding digest):
- the owner selection, queries and their digests;
- applied/matching/reduction limits;
- publication = `epoch`, semantics 3;
- `reuse_initial_d_bands`, `route_domain_overcover`, `route_joint_source_support_pruning`;
- `max_route_masks`, `max_queries`, `max_query_bytes`;
- the per-inspection event allowance (and its history if T11 is ever built);
- `frontier-policy`;
- every semantic flag when it lands: `zero-certificate` N1, `residual-anchors` G2' with its mode, the widening
  allowlist digest, p-anchors, any signed-off D1/D6 option.

**Not bound:**
- workers, inspection workers, merge helpers, the split and pinning, D;
- schedule (Lockstep/Rolling, B, cut sizes, H);
- resolution mode (first-found, canonical, sealed-preference, and `--epoch-resolve merge`), MRU k, refresh cadence
  `R_ev`;
- layer layout and compaction, replica count;
- checkpoint interval;
- `max_domains` and `max_events` (the aggregate successor-event allowance, `execution.rs:567-581`), both validated
  `>= current` at resume, with stop reasons `domain_allowance` and `event_allowance`;
- allocator and build profile.

The executable digest is recorded, never refused within semantics 3. Tests (SND-13):
- `a7_bound_flag_change_refused`: changing frontier-policy, `reuse_initial_d_bands` or the allowance at resume is
  refused;
- `a7_unbound_change_accepted`.

**Probe (IMP-15).** It stays backward-compatible. The existing keys stay (`walk_semantics_version` = 1 for the legacy
policies, `checkpoint_format`, `checkpoint_schema`), and two are added: `per_policy: {ordered: 1, ready: 1, epoch: 3}`
and `checkpoint_formats: {cp5, cp6}`. The frozen test `cli/mod.rs:362-382` is extended, not replaced.
`production_saved_owner_campaign.py --upgrade-executable` learns the new keys and CP6, and allows a re-split at resume.

### 11.6 CP5 -> CP6 import (IMP-12)

- **Opening the CP5 state.** `cp5_restore_for_import(dir, legacy_request)` reconstructs the legacy request from the
  saved run's recorded launch arguments (v2: Ready, `TransferUnreserved{lookahead}`, W100 67/32/1).
  - It checks `binding(legacy_request) == manifest.request` and the policy, then runs the legacy restore.
  - The CP5 lock is taken only on the block clone, and nothing else in the clone is written.
  - It refuses a CP5 state whose edge section shrank because the monitor released its graph (`checkpoint.rs:184-199`),
    i.e. whose closure was ever unavailable (SND-9).
- **Conversion rules:**
  - legacy Native `Completed{0}` -> Native (merge epoch 0); `Completed{f > 0}` -> NativeFrontier;
  - published Delegate -> Alias (re-verified); unpublished Delegate -> Pending;
  - Started, Reserved and Unreserved -> Pending;
  - initial anchors -> AnchorMap (InitialDBand);
  - the protected prefix is kept; closure bits are dropped and recomputed.
- **Edge filter:** keep the runs of published sources (completed natives) and the Delegate (alias) edges; drop every
  edge whose source is Started, Reserved or Unreserved.
- **Records and certification.** Records are not imported. `meta.imported_prefix = W_import` marks records below it as
  absent and exempt from audit; no certification relies on an imported native without F10 re-inspection.
- **Two forms:** **in-memory** (needed for the owner-gate pilot before S3) and **on-disk** (CP6 written once and
  block-cloned per gate; W2.6).
- **Tests:** `import_cp5_binding_from_manifest`, `import_drops_partial_edges`, `import_counts_match` (natives, aliases
  and pending vs CP5 meta), `import_refuses_closure_unavailable`.

### 11.7 Formats (little-endian, versioned; IMP-9)

- **ledger6, nodes, live:** §4.1 encodings, in chunks of 64 Ki entries, blake3 per chunk and in total.
- **anchors:**
  - Header `{version u16, count u64}`; records sorted by node.
  - Per record: `node u32, kind u8, n_anchors u8, scope_len u16, dispatch_version u64`, then `(anchor u32, stamp u64 or
    u64::MAX)` x n, then the scope bytes.
  - Scope bytes: InitialDBand `cut i64`; G2Residual a list of boxes, each `lower/upper u16 x N`.
- **Edge run log:** `(source u32, n u32, targets u32 x n)`.
- **records:**
  - Frame: `len u32, schema u16, kind u8, flags u8`.
  - Fixed part: `id u32, phase u8, owner u32, rank u8, lower/upper u16 x N`, power bounds (the fields of today's
    `power_bounds_json`), `class u8, err_class u8, break_reason u8, panic u8, seconds f32, emitted_events u64,
    accepted_events u64`, a stats struct by kind (Apply or Route), `v0 u64, n_refresh u16` and the refresh points.
  - Variable parts: error text, frontiers JSON, optional-refusal block, initial-overlap block, g2 block.
  - Trailer (16 B, patched in P3): `merge_epoch u64, distinct_edge_count u32, flags u32`.
  - Each segment's manifest holds per-kind counts and the records digest.
- **journal records:**
  - `CutHeader{k, n, n_s, W, plan digest, member seqs}`;
  - `Discard{seq, parent, class, cause, d_attempts, d_guard}`: the applied counter deltas are recorded, because RAM-guard
    suspect choice depends on metered growth, which replay cannot recompute;
  - `Stop{reason}`;
  - `RefreshInstall{version, id}`;
  - `Operator{flag, id}`.
  - Size [E, measured in S6]: ~48 B + 8 B per member per merge, plus ~20 B per discard.
- **`inflight-<session>.bin` records:** `SessionStart{session, generation, binary}`, `Start{seq, id, t}`,
  `Grow{seq, id, bytes}`, `End{seq}`, `Fatal{thread, seq, id, signo}`, `Progress{k, merged}`, `CleanExit{stop_reason}`.
- **meta JSON schema v1** (`"schema": 1`):
  - position: `k, W, P0`, per-tag counts;
  - digests: `records_digest, edge_digest`;
  - watermarks: `alias_verified_watermark, records_verified_watermark, accounted_session_watermark`, segment
    watermarks;
  - dispatch: `saved_I, saved_D, saved_requeue, saved_deferred, saved_isolated`, deferred/isolated counts and max ages;
  - stop state: `stop_reason`, uncommitted receipts, input frontiers;
  - `closure {available, refresh_version, counters}`;
  - attempts and guard histograms;
  - `roots_map` (query digest -> root ID);
  - provenance: `imported_prefix`, `engine_certification_void`, poison files, binding digest, operator-flag history.

### 11.8 Audit key mapping (legacy JSON key -> typed field; IMP-9)

| Legacy key (`W/execution.rs:702-777`) | Typed record |
|---|---|
| `id, phase, owner, lower, upper, rank, power_bounds` | fixed part |
| `stats` | stats struct (Apply or Route, by kind) |
| `seconds` | `seconds` (a timing field; excluded from identity) |
| `error` | error text |
| `accepted_events` | `accepted_events` (always present in epoch) |
| `frontiers` | frontiers JSON |
| `record_kind` | kind: `native_inspection`, `partial_initial_overlap_inspection` or `g2_residual_inspection` |
| `local_inspection_finished` | derived: class C0 and kind native |
| `local_classification_discharged` | derived: C0 ∧ 0 frontiers ∧ kind native |
| `residual_inspection_finished`, `native_inspection_scope` (`low_D_residual_only`), `initial_overlap{anchor_id, cut, covered_slice, residual_power_bounds, coordinates_and_rank_unchanged, authority}` | initial-overlap block |
| `optional_refusals`, `optional_refusal_provenance_scope`, `optional_refusal_provenance_truncated` | optional-refusal block (Apply) |
| `conservative_route_overcover` | derived: kind Route |
| `subdivided_native_inspection`, `physical_*`, `stats_scope`, `unreturned_physical_parts`, `part`, `first_per_phase_per_physical_part` | not applicable: subdivision is refused in epoch (F20) |
| `cancelled` | not applicable: cancelled results are never merged |

New epoch keys: `v0`, refresh points, merge epoch, `distinct_edge_count`, `class`, `break_reason`, `panic`. The decoder
CLI emits the JSON view with every key above. `records_roundtrip_audit_keys` (moved to S3) checks every row.

---

## 12. Rolling replay oracle and chaos mode (SND-5, IMP-1, PRF-11)

**Resolver inputs, complete list:**
- the job's `JobBytes`;
- its replica, which is deterministic from the owner programs;
- `S_{v0}` and the recorded refresh snapshots: store prefix, bucket tables, tiers (promotion deterministic in P4;
  compaction planned at H-aligned versions and installed at the next one), `F_{j(v)}`, `MergedView`;
- the resolver's own job-local state.

There are no other inputs: no live coordinator array, no wall-time trigger, no hint outside F. The mutation test
`replay_detects_unversioned_hint` adds a resolver read of the coordinator's `nodes` array and must fail replay.

**Merge inputs:** the cut (membership in completion order), its results, S_k and EpochState at k. Given these, merges
are deterministic.

**Journaled:** cut membership, discards with their cause, stops, refresher installs and operator flags. **Not
journaled, by design:** promotions, compactions, frozen-bit installs and ID assignment, all of which are deterministic.
T2 timing is not journaled either, so ledger6 is compared with Pending and Reserved identified.

**Replay unit = one session**, from its restore (or fresh start or import) to its last save.
`rustred walk-epoch-replay --checkpoint DIR --session S` works as follows:
- It restores the session's base generation with the engine's own restore code. This rebuilds the state bit for bit,
  including tiers and `F_{k0}`.
- It then runs the engine in **forced-schedule mode**. Each cut member's recorded `(seq, parent, v0, refresh points)`
  drives dispatch, and inspections run in parallel on pinned groups as soon as `S_{v0}` exists.
- A job pauses at a recorded refresh ordinal until that `S_v` exists. It always exists before the job's own merge, so
  replay cannot deadlock.
- Merges are applied serially in recorded order.

**Compared:**
- every regenerated `ResultBytes` (timing fields excluded);
- the plan digests and the records and edge digests;
- the next generation's sections, byte for byte, with ledger6 compared with tags 0 and 1 identified;
- the counters, exactly: discards and stops are replayed with their journaled deltas, and P1 re-derives the
  class-determined part (attempts) and checks it.

A replay that crosses a save/restore replays each session separately and compares at the generation boundary. Replay
wall time is gated: <= 1 h for the chosen window. Controls replay whole runs (C-4L, C-5F); pilots replay >= 10^4 merges
of their session.

**Chaos mode.** `RUSTRED_EPOCH_CHAOS=seed` is a test seam, like `RUSTRED_WALK_DIAGNOSTIC_PAUSE`, and the supervisors
remove it from campaign children; the crash seam is `RUSTRED_EPOCH_CRASH` (§8.4). It randomizes:
- inspector delays;
- cut sizes (1..512);
- refresh versions within the allowed range;
- helper counts per merge;
- crashes.

Chaos runs must pass audit, full F10 and replay.

**Tests (S6):**
- `rolling_replay_byte_identical`;
- `chaos_runs_pass_and_replay`;
- `replay_detects_unversioned_hint`;
- `compaction_install_deterministic`;
- `refresher_install_journaled`;
- `discarded_results_replayed_attempts`;
- `replay_crosses_save_restore_and_compaction`;
- `replay_window_starts_at_session_restore`;
- `frozen_hint_versioned`.

---

## 13. Invariants S1-S7 and E1-E5 mapped to code checks

| Invariant | Mechanism | Runtime check | Restore / audit / oracle |
|---|---|---|---|
| S1 every alias is a verified containment in the same (phase, owner), with its edge | T3/T10 require a P2 `Verified` token (canonical images) | `Ledger6::apply` refuses without a token; edge appended before the seal | alias re-verify since the watermark (full in audit); verify-closure `alias_containment` |
| S2 each ID published exactly once: Native, or Alias to a strictly newer containing survivor | transition table; `to >= W_k > id` | `apply` refuses a second terminal transition | per-tag counts; records per kind; one run per native |
| S3 Reserved never transfers | T2 at enqueue; T3 needs Pending (T10 needs Exhausted, never in flight) | `apply(T3)` on Reserved -> `LedgerError::Reserved` | F17 with saved bounds |
| S4 seal only after the complete dedup edge set, only (Native, 0 frontiers, no error) or Alias | P3 order: `append_run`, then T4, then seal | `append_run` asserts; `Nodes::seal` requires Native or Alias | F8; records digest (out-degree); Tracker checks |
| S5 saves are consistent cuts at merge boundaries; unmerged work re-inspected | `MergeBoundary<'_>` (after P3 or a bookkeeping-only merge); in-flight saved as Reserved | `save` requires the token; the token blocks every `&mut` | restore requeues Reserved; pause/resume byte identity in Lockstep |
| S6 frontiers and deterministic errors explicit, persisted, never sealed, stop the run; transients re-inspected | §9.1 classes; §8.4 counters | P1 classifier; `Nodes::seal` refuses F/E/X | parity in audit and F10; stop and crash drills |
| S7 (G2', amended by decision 7) admissible anchor scopes; no node < P0 anchored; anchors ∪ residual cover Q exactly; well-founded responsibility | §7 R1-R3, `MergedView` | P1 re-checks tags, stamps and kinds; cover by `covered_by_union` | kind-specific anchor validator; union coverage in verify-closure; G2' mutations |
| E1 snapshot immutability | byte segments, `forbid(unsafe_code)` in `E` | compile-time | - |
| E2 IDs depend only on the cut order | §6.3-6.4 | - | replay oracle |
| E3 exact uniqueness of images | P2 digest recompute; P3 `image.digest() == key` assert; exact insert asserts absence | C5 on a mismatch | restore refuses duplicates; `shipped_digest_mismatch_is_c5`, `survivor_geometry_single_source` |
| E4 resolver inputs versioned | frozen bits, deterministic tiers, journal | - | replay oracle; `replay_detects_unversioned_hint` |
| E5 every merge-side edge and transfer holds a P2 token | `Container`-based `verify` | P3 has no containment code path | `every_positive_tier_uses_verify` (all sites of §5.3) |

**v3 F1-F20 map:**

| Item | Where / test |
|---|---|
| F1 | §5.2 step 0, §6.3 step 1, §6.4 step 1 |
| F2 | §5.3 |
| F3 | sharded exact index + forced-collision seam test incl. save/restore |
| F4 | T3/T10 tokens + §11.4 step 7 |
| F5 | §6.6 (canary section, independent predicate, sticky poison) |
| F6 | §6.4 step 3 + double restore with D-bands |
| F7 | §9.1 (amended; break and panic rules) |
| F8 | §4.4 |
| F9 | §6.4 preflight |
| F10 | verify-closure extended to epoch; records digest; run headers |
| F11 | §8.2 |
| F12 | §6.3 step 3 + brute-force tests |
| F13 | u64 epochs + u48 preflight (`epoch_u48_preflight`) |
| F14 | §10.1 |
| F15 | §10.2 (amended: saves reuse the last refresh; availability required for certification) |
| F16 | anchors section |
| F17 | §8.2, §4.4 |
| F18 | §11.3 |
| F19 | §5.5 |
| F20 | `f20_refused_lanes`: epoch refuses finite `max_containment_checks`, subdivision and OwnerBatched at request validation; the audit reads workers from the last session's command |

---

## 14. Handoff §0.1 item 8 and related items: settlement

| Item | Decision | Where |
|---|---|---|
| F7 vs A2 | reconciled by P1 result classes with `break_reason` and `panic`; F7 amended | §9.1 |
| Per-ID attempt counter, liveness | per-job causes only; attributed guard; durable crash journal and fatal notes; isolation; Exhausted at 3; `--exhaust-id`; loop refusal; deferred share | §4, §5.8, §8.4 |
| Rolling replay oracle | event-count refresh; frozen bits; deterministic tiers; journal of cuts, discards, installs; session replay unit; forced-schedule parallel replayer; chaos with crash seams | §3.4, §12 |
| CP6 cut while merges continue | `MergeBoundary` after P3 or a bookkeeping-only merge, holding `&EpochState`; per-generation sealed segments; last completed refresher result | §11.2 |
| `--epoch-resolve merge` | diagnostic only; not bound; measured as fallback 2's cost | §2.1, §5.2 |
| Summaries: all IDs vs live-only + recompute | neither slab; raw-first verify, recompute; gate on recompute CPU | §3.6, §5.3 |
| First-found bias | oldest tier first, F-sealed-first MRU, bounded sealed-preference scan; open-cone gate | §8.3 |
| Merge-helper count at 1G | 9 reserved; 2.1-15.8 CPUs across the κ bracket; re-split binding-free; S4b; P-CHAIN measures it | §6.7, §15.4 |
| W3.2 miss path into W2 | stage S4b, gated on P-CHAIN points | §17.2 |
| Process/placement (item 1) | per-CCX in-process replicas [E, provisional]; byte-segment snapshot; option 2/3 fallbacks with a switch rule; placement policy | §2 |
| Gates in useful-work units (item 2) | U (frozen pooled c_K1^ref), κ (cycles, concurrency-qualified), IPC on native frames, matched window, marginal RSS, restore VmHWM and time | §15 |
| Work volume first-class (item 3) | criterion (H); every lever gate reports scheduled domains, peak pending, pending growth per completion, new points per native | §15.5-15.6 |
| Oracles (item 6) | every gate asserts `verdict == PASS` and `roots_independently_verified == roots_total`; F10 reference with N1/N4 off | §15, §16 |
| Governance (item 9, owner answer 11) | owner criterion alone decides; diagnostics pre-registered; MVP-B = Lockstep-2 per item 9; merge train by reviewed tips | §15.2, §17.3 |
| D6 deferred (decision 4, amended) | candidate-image list; sigma lookups off; cost measured in S4 | §5.2 |

---

## 15. Gates in useful-work units

### 15.1 Definitions

- **Classes** for per-native cost: Route; Apply by owner class: {011101110111000 (64% of gen-7 native time [M]),
  000011001001011 (CPU-hot on C-5F / C-HOT-sub [M census]), L* owners, other}. Each class is split by
  successor-count bins that are **fixed now**: the deciles of successor count over the natives of `streams-g7-v4.jsonl`,
  recorded with the table (IMP-5).
- **c_K1^ref(class, bin)** (PRF-13, SND-12, IMP-5):
  - Unit: per-native cycles:u, with instructions:u and CPU ns alongside.
  - Binary: the fixed harness binary (sha256 recorded), at K=1.
  - CPUs: a CCX with no foreign threads during the measurement; per-CPU busy is recorded, and a run with foreign
    threads on that CCX is repeated.
  - Sample: stratified and **pooled across the arms of the gate**, plus run2's natives for the run2 denominator.
  - Fit: per native, as c(class, events).
  - It is frozen before the gate, and its sha256 goes into every `metrics.json`.
- **Useful native work** `U = Σ n x c_K1^ref`. The same table is used for run2, C and S; every U ratio carries a
  bootstrap CI.
- **c_K1^arm**: the same sample re-measured at K=1 with the arm's own binary. It is used for κ only.
- **Contention factor κ** (PRF-3, IMP-5):
  - `κ = native-frame cycles:u per native on inspectors / c_K1^arm`, class-weighted.
  - Native-frame cycles = the per-thread cycles:u counter at job end minus job start, minus the bracketed resolve and
    serialize cycles. Counters are opened per thread by `W/placement.rs` and read at job boundaries.
  - κ is **accepted only if** the mean number of concurrently running inspector threads (schedstat) is >= 0.9·I over the
    window; otherwise the result is "inconclusive".
  - CPU-time κ (`CLOCK_THREAD_CPUTIME_ID`) and wall κ are reported beside it.
- **IPC clause**: IPC in native frames (the same counter windows) >= IPC_K1^arm / κ_max.
- **Useful share**: U / inspector-thread CPU, also reported in cycles.
- **Throughput**: obligations discharged per hour (natives + published aliases) and U per hour, over matched windows.
- **Denominator**: M1 run2's full window [T, T+1500.9 s]: 2.27M obligations/h (5-min slices 1.76-3.08M, CV 25%),
  1.30M natives/h (1.04-1.72M) [M]. `U_run2` comes from run2's records with the same table.
- **Marginal RSS per discovered domain**: least-squares slope of RSS vs discovered domains over the second half of a
  >= 40-min window (R² reported).
- **Pending dynamics**: pending growth per completion and discovered domains per native, per matched discovered-domain
  slice.
- **Recorder** (mandatory in every `metrics.json`; it replaces the 10% void rule):
  - load: foreign busy CPUs per 5-min slice (mean, p90), schedstat run delay;
  - work per native: instructions and cycles, cross-CCX fills (split into engine and process-global symbols);
  - memory: ZFS ARC, host MemAvailable, socket-1 free memory, numa_maps per section, NUMA balancing deltas, replica
    RSS;
  - inspectors: idle fraction;
  - merge: interval p50/p99, latency p99, cut-size distribution, P2 and P3 µs per native, hint and compaction waits;
  - lookup: delta length per miss, tiers scanned and candidates per request;
  - liveness: deferred and isolated counts and ages;
  - provenance: binary sha256 and build flags, c_K1 table sha256, n and the noise floor.

### 15.2 The owner gate (continue W2 past S4?) and the MVP ladder

**Arms** (SND-8):
- **C (comparator)**: the fixed legacy 4a17f9c7 + SoA kernel frame-pointer binary named in
  `TMP/w1-kernel/bin/COMPARATOR_READY.json` (release + frame pointers, glibc). d9163195 is void [M runA].
- **S (skeleton)**: epoch S2 + S4 with per-CCX replicas, RollingLite, P2 on helpers, and the in-memory CP5 import. It
  is built with exactly C's flags (release + frame pointers, glibc, no `[profile.campaign]`, no mimalloc), and both
  sha256 and flag sets go into the receipt. S's gains from `[profile.campaign]` and mimalloc are measured separately
  and reported, never inside the ratio.

**Protocol:**
- Same session, CPUs 128-227 under `socket1.lock`, each run <= 1 h including restore, interleaved C S C S (n >= 2 per
  arm), a fresh gen-7 block clone per run, W100, 25 min after T.
- **T:** for C, the first traversal heartbeat; for S, the first merged result after import and replica preparation.
  Launch-to-T is reported for both.
- **Load matching** (PRF-3):
  - Foreign busy CPUs are recorded per 5-min slice per run.
  - Ratios use slice pairs from adjacent runs whose foreign load differs by <= 10 CPUs.
  - A repeat with fewer than 3 matched pairs is re-run [proposal; owner confirms].
  - The load-normalized S throughput (U per inspector CPU-second x I) is reported beside the raw figure.

**The gate has three parts** (IMP-4, PRF-14):
1. **Owner criterion.** This alone decides the ladder: the mean obligations/h ratio S/C >= 1.5 over the matched slices,
   with the same-session spread reported.
2. **Soundness preconditions.** Without these, S is not admissible. With the same binary, on C-4L (FG, BMW, H, X) and
   C-5F, S passes audit `--require-closure` and walk-verify-closure full F10 (`verdict == PASS`,
   `roots_independently_verified == roots_total`):
   - in Lockstep (with the S2/S4 identity gates);
   - in RollingLite at W >= 24 (n >= 2);
   - with verify-all at 0 mismatches.
3. **Pre-registered diagnostics.** They trigger limiter work, never the ladder:
   - the U/h ratio with a bootstrap CI;
   - min(S) >= max(C);
   - κ(S) (§15.1);
   - (H) over the range of discovered domains that both arms reach (C reaches ~1.4-1.5M new domains in 25 min [E from
     run2's 3.35-3.61M/h]), with its noise floor from C's repeats over that range;
   - the **trajectory**: S/run2 and C/run2 on obligations/h and U/h, against the P-IMP floor (3x run2) and target
     (5x run2).

**Outcome wording, fixed in advance** (PRF-1):
- If S passes but is < 3x run2, the owner is told "gate passed; P-IMP trajectory not shown" before S3/S5 work is
  committed.
- If S is >= 3x run2, work continues.
- Per orchestrator decision 1: the comparator's kernel reached 2.86-3.79x less CPU per candidate, not >= 4x, so on
  lookup cost the comparison is slightly conservative in S's favour. The throughput ratio C/run2 is measured in the
  same session (runA was void, §1). If it confirms that the SoA kernel did not raise legacy throughput materially, the
  report says so.

**Fail -> MVP ladder** (SND-8, IMP-4):
- **MVP-B** = S2 + S4 + S3 (CP6, stops, crash journal) + kernel, with **Lockstep depth 2** (handoff §0.1 item 9), plus
  the S5 soundness items.
  - RollingLite is allowed in MVP-B only if S6's replay oracle (replay + chaos) is built and passes.
  - Time box: 10 working days [E].
  - Recommend MVP-B iff it passes the (A) soundness battery and beats **MVP-A** by >= 1.2x on obligations/h on the
    same protocol [proposal; owner confirms].
- **MVP-A** = 4a17f9c7 + `[profile.campaign]` + mimalloc override + I1(L*) and I4 inputs + L*-escape guard + frontier
  stop + 600 GB cap + 50 GB host floor, plus the W1.2 per-CCX replicas once they land. It is recommended otherwise.
- The owner is told explicitly that the gate's S arm is RollingLite and that MVP-B's schedule is Lockstep-2, measured
  separately.
- The comparator stays the fixed legacy + SoA binary even if the W1.2 legacy replicas land first; any such run is
  reported beside it.

### 15.3 W2 exit gate P-IMP (rewritten)

Protocol: a CP6 import of gen 7, built once and block-cloned per run; launch to first dispatch <= 10 min; then 40 min at
W100; n >= 2; recorder on. Plus one diagnostic arm with the store and layers bound to socket 0 (§2.6). Required (all):

| Clause | Threshold |
|---|---|
| useful share `U / inspector-thread CPU` | >= 0.60 |
| contention factor κ (concurrency-qualified) | <= 1.5; IPC in native frames >= IPC_K1 / 1.5 |
| resolve CPU on inspectors | <= 1.0 x U |
| merge-helper CPU | reported per native; <= 0.25 x U |
| merge duty (coordinator wall in P1 + P3 + publish) | <= 50%; P3 <= 10 us per native |
| throughput over [T, T+1500.9 s] (the full 40 min reported too) | >= 5 x 2.27M = 11.4M obligations/h **and** >= 5 x U_run2/h; every 5-min slice >= 6.8M obligations/h (3 x the run2 mean); band reported against run2's slice spread; the same-session C ratio reported beside run2's (run2 ran under 33 foreign CPUs) |
| marginal RSS (second half of the 40 min) | <= 0.45 KB per discovered domain |
| restore | VmHWM <= 1.10 x RSS at the end of restore; per-phase time and VmHWM recorded; time-to-guard at the measured rate for 600 GB and the 50 GB host floor written |
| work volume | pending growth per completion and discovered domains per native at matched discovered domains no worse than C beyond the noise floor; scheduled domains, peak pending, new points per native reported |
| soundness | canary 0 mismatches, no poison; restore validators PASS; verify-closure sample mode on the end state 0 uncovered / 0 parity (INCOMPLETE is expected, never PASS); the same binary PASSes the full oracle gate on C-4L and C-5F |
| admission | class exponents (hit, miss, reverse) measured (M-adm); the 1G projection is written from P-CHAIN (§15.4) |

Below 3x on obligations/h or U/h: stop adding features and fix the measured limiter (lookup, native scaling, merge).
After two limiter iterations still below 3x: stop and report (abandon rule, §17.4).

### 15.4 P-CHAIN: real states beyond 74M (new; PRF-7)

The P-IMP end state is saved (CP6) and resumed in links of <= 1 h each, until >= 3e8 real IDs (3-5 links at the
projected rate [E]). Each link measures:
- the M-adm exponents by class (hit, miss, reverse) on real states;
- helper CPU per native;
- merge latency p99 and P3 µs per native;
- bucket skew and tiers per bucket;
- restore time per phase and restore VmHWM;
- the marginal RSS and the socket placement.

Launch (B)'s 1G projections and S4b's gate use these >= 3 sizes, not the 74M snapshot alone. The link resumes also
rehearse the restore path at scale.

### 15.5 Launch criteria (B), (C), (H), rewritten

- **(B) inspection-bound at scale**, on P-IMP, P-CHAIN and both P-FRESH arms:
  - `U / inspector-thread CPU >= 0.70`;
  - κ (concurrency-qualified) <= 1.3, and IPC >= IPC_K1 / 1.3;
  - resolve CPU <= 0.5 x U;
  - merge duty <= 50%;
  - projected admission share at N = 1G <= 25% of worker CPU, from the class exponents measured on P-CHAIN with the
    engine's own thread factors;
  - projected merge-helper CPU at 1G <= the helper count of the launch split.
- **(C) memory:**
  - marginal RSS <= 0.40 KB per discovered domain over the second half of each >= 40-min pilot (P-IMP, P-FRESH-v3,
    P-FRESH-v4);
  - restore VmHWM <= 1.00 x RSS at save (a state saved at the guard must restore under the same cap), for every
    rehearsed restore and on the >= 3e8 link;
  - restore time <= 10 min on the >= 3e8 link (or a synthetic 2-3e8 state), extrapolated to 1G <= 20 min [proposal;
    owner confirms];
  - save duty <= 2% of wall;
  - time-to-guard at the measured rate for the 600 GB cap and the 50 GB host floor, **and** the socket-1 free memory at
    launch and the point where socket 1 is exhausted, with the measured cross-socket penalty applied beyond it, written
    into the launch receipt;
  - the at-guard action (save-stop, then a scope decision by the owner) decided in advance.
- **(H) work volume, new:** at matched discovered domains, (i) P-FRESH-v3 vs v2's events in 5M-domain slices up to the
  pilot's reach, and (ii) P-IMP vs the comparator from the same gen-7 clone:
  - pending growth per completion <= reference x 1.10 in the median slice, and in no slice above the reference's slice
    maximum;
  - discovered domains per native <= reference x 1.05;
  - reported per owner class. P-FRESH-v4 is compared with P-FRESH-v3 the same way.

(A), (D), (E) and (G) stand as in the master plan §7, with the oracle contract of §16. (F) is settled by the owner's
answers: shared socket 1 with foreign load recorded, no ARC cap, 600 GB cap, 50 GB host floor.

### 15.6 Protocol rules for every W2 gate

- Same session, same CPUs, interleaved repeats, n >= 2 (Ready-like rolling results n >= 5, with a stated noise floor).
- Load-matched slices (§15.2), and a c_K1 table frozen per gate.
- Lever gates report per owner class and add an aged (gen-7 resume) pilot, because the five-loop controls invert the
  gen-7 cost mix.
- Every lever gate reports scheduled domains, peak pending, pending growth per completion and new points per native.
- C-HOT-sub gates name the box (r1a12: falsify, 1.02M natives, W12; s2/r2a12: knobs, 2.12M natives, W48). C-HOT is
  audit-only.
- Write-up before new runs.

---

## 16. Termination condition and certification

**Termination (drain).** All of the following hold:
- Pending = ∅ and Reserved = ∅;
- no job is in flight;
- the result queue and the miss spill are empty;
- the last merge's P4 has published.

The walk then forces a synchronous refresh, saves, and exits 0 (`drained`) iff every one of these holds; otherwise it
exits 4 (`drained_uncertified`):
- 0 frontiers, 0 NativeError, 0 Exhausted, and no input frontier;
- the closure monitor is available (F15);
- certification is not void (no poison).

Under `--frontier-policy stop`, the walk stops at the first frontier, error or Exhausted ID before draining.

**Engine-certified root.** The query's root record ID `r` comes from the persisted, re-verified roots map (§11.4): its
own ID, or the helper ID it hit at initial admission. `r` is certified when it is closed in the final synchronous
refresh with the monitor available. That means `r` is sealed and every node reachable over edges is sealed (Native with
0 frontiers and no error, or an Alias whose chain ends sealed). It also requires:
- the §13 invariants;
- a 0-mismatch canary and no poison file;
- the restore validators.

Closure is coinductive: sealed cycles count as closed. It is a coverage certificate, not a termination or descent
certificate.

**Independently verified root.** `rustred walk-verify-closure --require-closure` with full F10 re-inspection reports
`verdict == PASS` and `roots_independently_verified == roots_total`. Full F10 re-inspects every native of the root's
cone with the reference reducer and every walk lever off (N1/N4 included when they land). `roots_total` counts distinct
root records, not queries.
- Sample mode is a smoke test: INCOMPLETE, never PASS.
- At gen-7 scale, full F10 costs ~15 h at 32 threads [E]. At the final state it is a budgeted post-campaign job, run
  from the final CP6 with the streaming verifier (W2.6), and it is required before any published closure claim.

**Reporting (A9).** Helper roots (67) and physics queries (116) are reported separately. A physics query admitted as a
hit on a helper is certified iff that helper's root is (test `a9_roots_reported_separately`).
`family_closure_claim` becomes true only when all of the following hold; until then it is false in every artifact:
- all 183 query roots are independently verified under the contract above;
- 0 frontiers, 0 errors, 0 Exhausted;
- the ledger is drained.

---

## 17. Implementation map

### 17.1 Module layout (`W/epoch/`, `#![forbid(unsafe_code)]`)

| Module | Content |
|---|---|
| `mod.rs` | `EpochConfig`, `run_epoch<N>(state, request, replicas, cancellation, observer)`, `EPOCH_WALK_SEMANTICS_VERSION = 3` |
| `state.rs` | `EpochState` borrow groups, chunk trees, `MergeBoundary<'_>`, counters |
| `ledger6.rs` | encoding, `Transition`, `Ledger6::apply`, validators |
| `segments.rs` | byte segments and safe views (store chunks, blocks, bucket tables, bitsets) |
| `store.rs` | canonical chunk tree with tail mini-chunks, `StoreView`, sharded exact index |
| `verify.rs` | `Container`, `verify`, `Verified`, recompute path |
| `canary.rs` | canary selection, independent predicate (oracle `lattice`), poison |
| `snapshot.rs` | `Snapshot`, `SnapshotCell`, bucket tables |
| `bits.rs` | bits keeper, frozen bits `F_j` |
| `layers/{mod,block,kernel,tiers}.rs` | SoA blocks, A5 encoding, scalar kernel, young tier, promotion, compaction |
| `resolve.rs`, `miss_buffer.rs` | resolver sink tiers 0-7, canonical mode, byte bounds, spill |
| `job.rs` | `JobBytes`, `ResultBytes` codecs |
| `inspector.rs` | `InspectorGroup` (replica, queue, pinned threads), A6 refresh, panic handling, replica rebuild |
| `merge/{mod,p1,p2,p3,p4}.rs` | cut driver and phases, `MergePlan`, `MergeDelta`, bookkeeping-only merge |
| `dispatch.rs` | `DispatchPolicy`, `Fifo`, lists, cost-weighted depth |
| `liveness.rs` | cause table, suspects, journal-derived guard, loop refusal |
| `inflight.rs` | per-session crash journal writer and reader |
| `schedule.rs` | Lockstep, Rolling, RollingLite |
| `edges.rs`, `closure.rs` | EdgeStore, two-pass restore build, refresher, cone report |
| `anchors.rs`, `merged_view.rs` | `AnchorMap`, R1-R3 validators, `MergedView` |
| `records.rs` | typed record schema, writer (per-kind counts, records digest), decoder, audit key mapping |
| `cp6/{mod,manifest,sections,segments,restore}.rs` | Store, save, stop paths, stop-file contract, restore, validators |
| `import.rs` | `cp5_restore_for_import`, CP5 -> CP6 (in-memory and on-disk) |
| `replay.rs` | forced-schedule replayer, chaos and crash seams |
| `tests/` | per-stage tests below; soundness tests use `licensed_or_skip` / `workers_or_skip` and run with `RUSTRED_TESTS_REQUIRE_LICENSE=1` |

Outside `E` (unsafe allowed):
- `W/placement.rs`: pinning, mempolicy, per-thread perf counters, fatal-signal handler.
- `W/simd_kernel.rs`.
- `crates/rustred-app/src/alloc_meter.rs`.
- `W/segmap.rs` (option 2 only).

### 17.2 Stages, gates, tests (each pilot <= 1 h)

**S2 lockstep skeleton** (branch `fable_5_1-v3-epoch`)

Scope:
- EpochState and ledger6 (in memory); P1, P2 (serial) and P3.
- Lockstep depth 1 with B = 64.
- Canonical in-merge resolution over the kernel lane's ID-ordered SoA index. The index is reused as code and **mutated
  by P3** (IMP-13).
- Closure through the legacy Tracker, with forced refreshes only.
- Typed records (JSON view); epoch policy in audit and verify-closure.

Tests:
- Ledger and IDs: `ledger6_roundtrip_boundaries`, `ledger6_transition_table_exhaustive`,
  `p3_preflight_failure_is_noop` (injected at every reserve), `id_assignment_first_parent_ordinal`,
  `append_run_strictly_increasing`, `merge_numbering_visibility`.
- Transfers and seals: `reserved_never_transfers`, `protected_prefix_never_transfers`, `alias_to_greater_same_bucket`,
  `seal_rules_f8`.
- Classification: `p1_stale_seq_and_event_parity`, `result_class_matrix` (C0-C5 injected),
  `p1_initial_d_band_revalidation`.
- Verify and F1: `container_verify_matrix` (every `Container` variant x phase/owner/EMPTY/range),
  `antichain_cross_phase_same_owner_refused` (a seam forces Route and Apply misses of one owner into one bucket),
  `local_final_verified_in_p2`, `shipped_digest_mismatch_is_c5`, `survivor_geometry_single_source`.
- Termination and dispatch: `termination_detection`, `termination_requires_monitor_available`,
  `dispatch_cursor_restore_min_pending`, `refill_progress_assertion`.
- Lanes and bindings: `epoch_u48_preflight`, `f20_refused_lanes`, `a3_route_mask_interning_roundtrip`,
  `a9_roots_reported_separately`, `s2_closure_forced_refresh_only`, `semantics_probe` (backward-compatible keys).
- Cost: `p3_per_native_cost` (a microbenchmark, reported).

Gates:
- audit `--require-closure` and verify-closure (full F10, contract) PASS on C-4L and C-5F;
- byte identity across W6/W12/W24 and five-loop W24 vs W50;
- four-all canonical identity across widths (C-4L-comb-O);
- natives vs Ready reported, > 5% flagged.

**S4 inspector resolution** (+ per-CCX replicas, RollingLite, in-memory import, P2 on helpers, frozen bits,
deterministic tiers, canary, `MergedView`)

Scope: tiers 0-7, `verify` over containers, pattern and young tiers, oldest-first order, sealed preference, canonical
mode, A6 wiring (off in RollingLite).

Tests:
- Verify and canary: `verify_phase_owner_empty`, `every_positive_tier_uses_verify` (every call site of §5.3; a mutation
  that bypasses verify must fail a test), `orthant_full_power_bounds`, `canary_catches_injected_false_hit`.
- Kernel: `kernel_forward_reverse_vs_bruteforce` (>= 1e6 random pairs vs `CompactSummary::contains` and
  `lattice::Cell`), `a5_saturation_escape`, `gen7_layer_differential` (ignored; >= 1e8 forward + >= 1e8 reverse pairs),
  `layer_kernel_per_candidate_vs_legacy` (bench).
- Resolver tiers: `local_resolves_to_q1`, `job_exact_set_byte_bound`, `miss_spill_never_fails`,
  `delta_check_complete_vs_bruteforce`, `delta_check_complete_after_compaction`, `antichain_order_independent`,
  `tier_envelope_skip_sound`.
- Determinism: `canonical_identity_workers_and_pause`, `canonical_ignores_hints`, `lockstep_identity_independent_of_W`,
  `frozen_hint_versioned`, `young_tier_promotion_deterministic`, `snapshot_refresh_race`.
- Exact index: `exact_collision_seam_save_restore`, `exact_shards_resize_incremental`.
- Hot-path layout: `arc_lines_no_hot_data` (layout test of the wrappers).
- Failure paths: `inspector_panic_yields_c3_and_rebuilds_replica`, `consumer_stop_parity`,
  `consumer_stop_classified_by_break_reason`, `preflight_discard_no_counter_change`, `oversize_result_admitted_alone`,
  `p2_scratch_single_result`.
- Replicas, anchors, D6: `replica_results_identical` (G = 1, 4, 8), `anchor_plan_uses_snapshot_epochs_only`,
  `d6_hook_off_no_sigma_lookups`.

Prerequisites:
- bucket skew measured offline from `streams-g7-v4.jsonl` (§6.7);
- the comparator binary ready.

Gates:
- verify-all 0 mismatches.
- Audit, closure and the F10 contract PASS on C-4L and C-5F.
- Canonical identity across worker counts and pause points.
- First-found vs canonical:
  - natives within ±5%;
  - distinct edges per domain (**self-edges excluded**) within ±10%;
  - per-root open-cone counts at matched natives no worse (C-5F and C-HOT-sub r1a12, n >= 3 per arm, newest-first as the
    negative control).
- **>= 4x less CPU (and cycles) per tested candidate on the real epoch layers vs the 4a17f9c7 layout at 74M**, forward
  and reverse separately, same session, interleaved, recorder on (IMP-2, orchestrator decision 1).
  - Measured by intel's idxreplay on the S4 multi-tier structure with the recorded publish cadence; candidates and
    tiers scanned per request are reported.
  - If reverse stays < 4x: S4b first, or ID-ordered layers for reverse sets.
- `verify_recompute` CPU <= 2%.
- P3 <= 10 us per native at 74M.
- In-engine κ (concurrency-qualified) <= 1.5 at W96 on the gen-7 sample.
- Replica RSS growth recorded.
- Coordinator-CCX A/B (§2.2) and the D6 cost arm.
- Then the **owner gate** (§15.2).

**S3 CP6, stop paths, crash journal** (branch `fable_5_1-v3-cp6`; developed in parallel from S2; merged after the
gate, or into MVP-B)

Scope: CP6 sections and sealed segments, **typed binary records with the P3 trailer, ledger6 persistence** (moved from
S5; IMP-9), the inflight journal, fatal notes, the alloc meter, poison files, bookkeeping-only merges, the stop-file
contract.

Tests:
- CP5 review items #1-#13.
- Pause, kill and resume: `pause_at_epoch_k_byte_identical`, `kill9_after_save_resume`,
  `ram_guard_exit4_resume` (via `shared_owner_campaign.py`, 50 GB floor), `resplit_resume`.
- Policy stops: `frontier_stop_resume_next`, `error_stop_resume_no_redispatch`, `exhausted_stop_retry_flag`,
  `exhaust_id_flag`, `keep_exhausted_resume`.
- Save mechanics: `double_restore_d_bands`, `save_while_inspecting`, `save_pause_blocks_refill` (and T7 and installs),
  `late_result_after_stop_save`, `restore_previous_with_tail_beyond_watermark`, `result_queue_backpressure_during_pause`,
  `p4_failure_takes_ram_guard_path`, `spill_files_swept_on_restore`.
- Records and anchors: `record_trailer_patched_in_p3`, `records_roundtrip_audit_keys`, `anchors_section_roundtrip`.
- Stop file and RAM guard: `ram_guard_stop_reason_parsed`, `ram_guard_save_without_merge_has_boundary`.
- Liveness drills (§8.4): `fast_oom_head_exhausted_within_3_stops`, `crash_loop_head_ends_exhausted`,
  `ram_guard_innocent_jobs_not_penalized`, `host_wide_unattributed_stop_no_penalty`,
  `nonreturning_culprit_counter_persisted`, `deferred_ids_progress_under_growing_pending`, `abnormal_exit_loop_refused`.
- Poison, binding, import: `canary_poison_sticky_across_restore`, `a7_bound_flag_change_refused`,
  `a7_unbound_change_accepted`, and the four `import_*` tests (§11.6).
- `corruption_refused`, where each case must be refused:
  - a byte flip per section;
  - a wrong digest;
  - an alias to a smaller ID; an alias not contained; an alias from an ID < P0;
  - an anchor stamp > dispatch version; an InitialDBand anchor >= P0; an anchor record on a node < P0;
  - reserved > the saved bound;
  - a sealed NativeError;
  - an edge dropped from a sealed run with meta recomputed;
  - a Native <-> NativeFrontier swap with nodes consistent;
  - a run target >= W_k;
  - a retargeted root.

Gates:
- all PASS on FG, X and C-5F;
- the RAM-guard soft, hard (SIGKILL) and abort drills PASS;
- the gen-7 import restores in <= 10 min with VmHWM recorded.

**S5 parallel merge** (branch `fable_5_1-v3-merge`)

Scope: block-parallel P2 inside buckets, EdgeStore shards, the refresher off the coordinator, and the pipelined-P2
option. "Persisted ledger6" and "typed binary records" moved to S3.

Tests: `serial_vs_parallel_merge_identical` (helpers 1/8/16), `reverse_sets_block_parallel_equal`,
`edge_shards_beyond_u32`, `frozen_refresh_equals_forced_scan`, `refresher_oom_marks_unavailable_before_seal`,
`pipelined_p2_identical`.

Gates: byte identity serial vs parallel on every control; coordinator perf profile; five-loop W50 traversal vs 53e672fc
Ready (two rounds).

**S4b miss path (W3.2 moved into W2)**

Scope: a sublinear forward-miss and reverse structure over the pattern tiers, chosen by replay. Candidates: per-group
dominance pruning on sorted lower corners, or the 2N+6 feature trie of
`domain_admission_index_next_step_2026-09-22.md`.

Gate:
- thinning exponents on the gen-7 replay and on the P-CHAIN states <= 0.30 for miss and reverse candidates;
- projected admission share at 1G <= 25%, and projected helper CPU at 1G <= 9, both from the P-CHAIN points;
- results byte-identical to S5 on controls (performance-only).

**S6 rolling**

Scope: Rolling schedule, A6 refresh, background saves, the replay oracle, chaos.

Tests: §12, plus `save_concurrent_with_inspection`.

Gates:
- 3 pause points per control PASS (audit, closure, F10 contract);
- every rolling control run replays, within the replay wall-time gate;
- stale-miss rate, merge duty and inspector utilization reported;
- natives to drain C-5F and C-HOT-sub r1a12 at W96 rolling within +10% of W96 lockstep canonical;
- four-all rolling judged statistically (n >= 5).

**S2.6 importer, deliverable, tooling** (branch `fable_5_1-v3-import`)
- the on-disk CP5 -> CP6 import;
- the typed record deliverable + compact summary JSON (no `domains` JSON array);
- the Rust streaming audit and verify-closure for epoch, including the full records and alias passes, with runtime per
  10^8 records measured;
- ports of `compare_walk_records.py`, `campaign_monitor.py` and the `--upgrade-executable` probe.

Then **P-IMP** (§15.3) and **P-CHAIN** (§15.4).

### 17.3 Merge train (into `fable_5_1`; each step with fmt, lib suite, `cli_routed_campaign`, Python suite, pushed)

Branches are merged at their **tip after review**, and hashes are recorded at merge time (IMP-16). The current tips are
listed for reference only.
1. `fable_5_1-v3-oracle` (63771a50): gate contract, `assert_oracle_pass.py`, `lattice::Cell::{contains,
   covered_by_union}`. Merged before any W1/W2 gate is accepted (handoff §0.1 item 6).
2. `fable_5_1-v3-ops` (fc3c07e4 at 07:36Z): A10 frontier stop, RAM guard (50 GB, attribution), license gates,
   `[profile.campaign]`, opt-in mimalloc. Its review round adds the stop-file attribution of §11.3. It conflicts with
   step 1 in `W/checkpoint.rs`, `W/mod.rs` and `cli/args.rs`: rebase onto 1.
3. `fable_5_1-v3-kernel` (34861652 + fix round): the legacy SoA kernel (the comparator), 92-B immutable summaries, A1
   checks in the legacy lane. Strict identity vs 4a17f9c7 is re-run after the rebase.
4. `fable_5_1-v3-harness` (b0e9db6e), test-only parts: reinspection harness, replica prototype, root diagnostics. Its
   hunks in `W/checkpoint.rs`, `W/inspection.rs` and `W/mod.rs` are rebased onto 1-3; strict identity is re-run.
5. Tools-only branches (intel `tools/research/idxreplay`, census, routecensus CLI subcommands, baseline, wv, g2falsify
   tools and audit flags): tools and docs only. The g2falsify and widen engine changes and the knobs dispatch-order knob
   are **not** merged.
6. Epoch stages in order: S2, S4, (owner gate), S3, S5, S4b, S6, S2.6. Each stage branch starts from the previous
   merged stage; S3 rebases before its merge.

### 17.4 Time boxes and abandon rules [E]

S2 7 working days, S4 9, owner gate 2 (socket-1 slots), S3 8 (in parallel), S5 5, S4b 5, S6 5, S2.6 6, P-CHAIN 2.
Abandon rules:
- owner gate fails: the §15.2 ladder;
- P-IMP below 3x after two limiter iterations: stop and report, with MVP-B as the candidate;
- any C5 in a control or pilot: stop stage work until it is root-caused and a regression test exists.

---

## 18. Legacy lanes: what must stay (frozen legacy = binary 4a17f9c7 + strict identity)

1. Ordered and Ready (`W/execution.rs`, `W/delegation/`, `W/queue*`), CP5 (`W/checkpoint*`) and
   `WALK_SEMANTICS_VERSION = 1` stay. The probe keeps its keys and adds `per_policy` and `checkpoint_formats` (§11.5).
2. **Allowed shared-code edits** (IMP-15): visibility only.
   - `CompactDomain::{contains, is_full_orthant, try_native_summary, native_summary, digest}` and `rank_contains` move
     from `pub(super)` to `pub(in crate::...::walking)`.
   - The sharded exact index is new code in `E`; it reuses the digest function.
   - Every change to legacy or shared code (`inspection.rs`, `routing.rs`, `initial_orthants.rs`, `initial_overlap.rs`,
     `reuse.rs`, `queue/compact.rs`) must keep strict Ordered identity (records and `containment_checks`) against
     4a17f9c7 on FG, BMW, H, X, four-all and C-5F, plus CP5 resume in both directions on FG.
   - The epoch engine calls the shared visitors unchanged (§5.1) and adds no fields to shared events.
3. **Uses of the legacy lanes during W2-W5:**
   - the Ordered identity oracle for controls;
   - Ready for MVP-A;
   - CP5 restore of the frozen v2 and interim checkpoints (read-only analysis via block clones) and as the importer's
     reader;
   - walk-verify-closure on CP5;
   - the comparator binary;
   - the CP5 `--upgrade-executable` probe;
   - the F10 reference visitor.
4. The legacy lanes are deleted only after the launch, by owner decision.

---

## 19. Open items and risks

**Measurement gaps:**
- The per-CCX verdict is [E, loaded, provisional]. The quiet sessions F2/G/H are queued (amendment 2a(v)). The S4
  in-engine κ (concurrency-qualified) is the confirmation of record, and the switch rule of §2.1 is written in advance.
- The comparator has no valid receipt yet: runA is INVALID, and the fixed binary is pending. The owner gate measures C
  again in the same session anyway.
- The 1G projections still extrapolate until P-CHAIN runs. No hit exponent exists for the chosen layout.
- Unmeasured [E]:
  - c_cheap and the `verify_recompute` rate;
  - the save write rate at 9 GB, and CP6 restore time and VmHWM;
  - the sealed-preference cost and effect;
  - alloc-meter overhead, frozen-hint staleness effects, compaction waits;
  - bucket skew, and whether mempolicy exempts pages from AutoNUMA.

**Design risks:**
- Crash attribution is best-effort for SIGKILL without a `Grow`/`Start` record. The unattributed-loop refusal (§8.4) is
  the backstop, at the cost of an operator decision.
- A faster engine reaches the RAM wall sooner. Termination is not established; (H) and W4 are what act on it.

**Owner and upstream items:**
- Symbolica's refcount and ahash traffic remain upstream. Any change there is an owner decision (no CAS code here).
- D6 needs a sigma-image certification rule and an epoch-path admission cost before it can be proposed (§5.2).
- T11 (retry of allowance errors) and the stricter-than-owner rules marked [proposal] need owner confirmation.

**Follow-ups outside W2:**
- Records at the projected rate grow ~10-15 GB/h on disk [E]; the deliverable size needs a budget in W2.6.
- W1.2's audit of RustRed-side polynomial clones on the hot path changes shared native code. It must pass the §18
  strict-identity gate, and then it benefits both engines.
- The ops branch must add the stop-file attribution (§11.3) in its review round.

---

## 20. Review disposition (revision 2)

Legend: **F** = fixed in the named section; **P** = partly adopted, with the reason; **R** = rejected, with the reason.

### 20.1 Soundness review (SND)

| # | Severity | Problem | Disposition |
|---|---|---|---|
| SND-1 | blocking | Crash/restore loop: hard kills, OOM kills and aborts lose the counters, and "Reserved first" re-dispatches the culprit | **F** §5.8, §8.4, §11.4 step 3, §11.7. A per-session `inflight` journal (`Start` >= 60 s, `Grow` >= 1 GiB, `End`, `Progress`, isolated `Start` written before dispatch) plus `Fatal` notes from a fatal-signal handler. The guard is derived at restore across abnormal sessions; suspects are isolated, not "Reserved first"; guard 3 means T8; `--exhaust-id` (T12); the engine refuses a start after 2 unattributed or zero-progress abnormal exits. Drills for soft, hard and abort, and `crash_loop_head_ends_exhausted` |
| SND-2 | major | G2' + InitialDBand false closure (A0 -> B -> A0) | **F** §7: R1 (no anchors for IDs < P0), R3 (an InitialDBand native lends only its inspected slice), a well-foundedness argument replacing the acyclicity text, kind-specific validators, the new mutation and a corruption test; §13 S7 corrected |
| SND-3 | major | The A1 chokepoint does not cover the antichain, unpublished survivors, Local finals or early self-edges | **F** §5.3 (`Container` enum, token table), §2.4 (Locals ship the q image), §6.3 steps 4, 6, 7, T2 dispatching only IDs < `published_len`, tests. **P**: the re-verify of non-survivors and Locals happens in P2 against `Planned` containers, not in P3 after ID assignment. The position-to-ID map is a bijection fixed before P3, so the authority is the same, and PRF-5 asks to keep containment work out of P3 |
| SND-4 | major | Canary not implementable; C5 not sticky | **F** §2.4 canary section, §6.6: independent predicate `lattice::Cell::contains`, sticky poison files, `engine_certification_void`, cleared only with a full-F10 receipt; tests |
| SND-5 | major | Rolling replay not deterministic | **F** §3.4 (frozen bits; deterministic promotion and compaction), §10.2 (journaled refresher installs), §6.4 step 4 (journaled discards), §12 (replay unit = session from its restore; ledger compared with tags 0 and 1 identified; counters exact). **R**: persisting tier boundaries in meta. The reviewer's alternative, replay windows that start at a restore, is taken instead: the restore itself is deterministic |
| SND-6 | major | Liveness penalizes innocents and starves deferred IDs | **F** §8.4 cause table (no penalty for RAM-guard, preflight, domain-cap or unattributed stops; guard to the top metered growth and unreturned jobs), §8.3 deferred share, bounds restated, heartbeat and meta counts, tests. Spill I/O is also penalty-free (a host cause); a second one stops the run with `io_error` |
| SND-7 | major | F1 violated by trusting shipped digests | **F** §6.3 step 1, §6.4 step 1, §13 E3, tests |
| SND-8 | major | Owner gate not protocol-matched; RollingLite not soundness-gated | **F** §15.2: build parity (C's flags for S; campaign and mimalloc reported separately), RollingLite audit + full F10 at W >= 24, T defined for both arms, load-matched slices, the owner told that the S arm is RollingLite, MVP-B = Lockstep-2 or RollingLite only with the replay oracle |
| SND-9 | major | Restore validators regressed (out-degree, tags, roots, imports) | **F** §4.4 (records and edge digests, per-tag and per-kind counts), §11.4 steps 5, 6, 10, §11.6 (`imported_prefix`, refusal when the CP5 closure was ever unavailable), `corruption_refused` extended |
| SND-10 | minor | Save-cut mechanics | **F** §11.2: the token holds `&EpochState` (refill, T7 and installs blocked); per-generation sealed segments; late results dropped; idle time counted in the duty; tests. **P**: "keep chunks in RAM until covered" is replaced by sealed segments; an open segment is never referenced, so there is nothing to protect in RAM |
| SND-11 | minor | P4, compaction and oversize failure paths | **F** §6.5 (boundary after P3; P4 failure keeps the stale snapshot and takes the RAM-guard path), §3.5 (oversize result admitted alone; single-result P2 plan), tests |
| SND-12 | minor | U vs κ tables; whole-thread IPC | **F** §15.1: `c_K1^ref` for U, `c_K1^arm` for κ, IPC on native frames |
| SND-13 | minor | Hazards without tests (F13, F20, A3, A7, A9, InitialDBand P1) | **F** §17.2 S2/S3 tests; `max_events` and `--epoch-resolve merge` classified as not bound (§11.5) |
| SND-14 | minor | Termination: monitor availability; cursor after T9 | **F** §16, §10.2, §8.3 (cursor = min Pending; refill progress assertion) |
| SND-15 | minor | Error-class edge cases | **F** §5.1 (replica rebuild after a panic; `PoisonError` is C5), §9.1 (panic records parity-exempt, no successors). **P**: the allowance raise is designed as T11 but not built in W2; the permanence is recorded for the owner as the reviewer's alternative allows |
| SND-16 | minor | Record fields known only in P3 | **F** §5.6, §11.7: fixed-offset 16-B trailer patched in P3 and compared in replay |
| SND-17 | minor | F12 delta check has no index or memory term | **F** §3.4 (young tier in ID order, per-tier ID ranges), §6.3 step 3; delta length per miss recorded. **P**: no separate per-bucket ID list, because the young tier plus the tier ID ranges index the delta at no extra memory |

### 20.2 Performance and scale review (PRF)

| # | Severity | Problem | Disposition |
|---|---|---|---|
| PRF-1 | major | Owner gate more lenient than stated | **P** §15.2: the trajectory report against the P-IMP floor and target is pre-registered, with fixed outcome wording. **R** as evidence: the preliminary 0.91x-run2 figure, because runA is INVALID (d9163195 heartbeat storage walk [M comparator.md]). The "slightly conservative" sentence stays because orchestrator decision 1 requires it, now restricted to per-candidate lookup cost |
| PRF-2 | major | Process fallback not implementable; stale replica figures | **F** §2.1: byte-segment snapshot, option 2 maps the segments, `W/segmap.rs`, a written switch rule, per-publish segment cost; κ bracket 1.42-3.75; 5.46 GB per copy |
| PRF-3 | major | κ and ratios load-confounded | **F** §15.1 (cycles:u κ, concurrency qualification, CPU and wall κ beside), §15.2 (load-matched slices, normalized throughput, T for S), §15.3 (same-session C ratio beside run2) |
| PRF-4 | major | Dispatch leaves inspectors idle | **F** §8.2 (cost-weighted depth, D_max >= 3I, pre-save top-up, Route micro-batching option), §3.2 (borrow groups, so refill runs during P2 and P4), §3.5 (result-queue bytes bound unmerged results), recorder (idle fraction, merge interval) |
| PRF-5 | major | Merge critical path not modelled | **F** §6.1 (cut <= 512), §6.3 (SoA kernel for the antichain; delta via tiers), §6.3 step 6 (transfer re-verify moved into P2), §6.4 (P3 budget <= 10 us, gated at S4), §6.7 (critical-path arithmetic, 80-250 merges/s, skew prerequisite), §3.3 (tail copy cut to <= 7 KB). **P**: pipelined P2 is an S5 option with an identity test, not S4 |
| PRF-6 | major | NUMA and socket-1 capacity at scale | **F** §2.6 (MPOL_PREFERRED_MANY 4-7, numa_maps and balancing deltas, socket-0 diagnostic arm), §15.5 (C) |
| PRF-7 | major | Every gate at 74M | **F** §15.4 P-CHAIN to >= 3e8; (B) and S4b use its points |
| PRF-8 | major | Restore at scale ungated | **F** §11.4 (per-phase model and gate; digests fused with decode; aliases and records verified since watermarks, with the full pass in the audit), §10.1 (two-pass monitor), §15.5 (C). **P**: sampling of old domain and edge chunks is rejected; restore reads them anyway, so a fused digest costs no extra pass |
| PRF-9 | minor | Single exact table; unchunked arrays; budget | **F** §3.2 (4,096 shards; every per-ID array a chunk tree), §3.6 (19-39 B/ID, 60 GB replicas, pinned tiers) |
| PRF-10 | minor | Multi-tier lookup cost and unbounded tiers | **F** §3.4 (<= 12 tiers per bucket, envelopes), S4 idxreplay on the multi-tier structure with tiers scanned per request |
| PRF-11 | minor | Single-threaded replayer too slow | **F** §12: forced-schedule mode, parallel inspections, serial merges, wall-time gate |
| PRF-12 | minor | Snapshot header and bucket-view refcount lines | **F** §2.3 (thread-local copies, `Arc<Box<_>>`, chunked bucket tables), test `arc_lines_no_hot_data`, cross-CCX fills attributed in the recorder |
| PRF-13 | minor | U statistically fragile; quiet-socket condition infeasible | **F** §15.1: pooled stratified sample, bootstrap CI, a CCX with no foreign threads, cycles:u with an instructions:u cross-check, one table for run2, C and S |
| PRF-14 | minor | Owner gate adds flipping clauses | **F** §15.2 part 3: κ and (H) are diagnostics; the (H) overlap range and its noise floor are stated |
| PRF-15 | minor | Coordinator shares an L3 with streaming helpers | **F** §2.2: the S4 A/B arm (b); the default stays until measured |

### 20.3 Implementability and testability review (IMP)

| # | Severity | Problem | Disposition |
|---|---|---|---|
| IMP-1 | blocking | Replay nondeterminism: hint, compaction, refresher, discards, old v0 | **F** as SND-5; the window starts at a session restore, so no job has v0 < k0; journal bound [E] with measurement in S6; the tests named in the review, renamed where the design changed (`compaction_install_deterministic`, `replay_window_starts_at_session_restore`) |
| IMP-2 | major | Missing >= 4x per-candidate gate on the epoch layers | **F** §17.2 S4 gate and the bench `layer_kernel_per_candidate_vs_legacy`; the action if reverse stays < 4x is stated |
| IMP-3 | major | Process section stale vs amendment 2a; fallback not drop-in; placement | **F** as PRF-2 and PRF-6; §6.7 and §3.6 bracketed |
| IMP-4 | major | Owner gate stricter than the owner; MVP-B redefined | **F** §15.2 in three parts; stricter rules marked [proposal]; MVP-B = Lockstep-2 per item 9 |
| IMP-5 | major | Wall-time κ; c_K1 provenance and bins | **F** §15.1 (cycles and CPU time; frozen pooled table with sha256; bins fixed from gen-7 streams) |
| IMP-6 | major | Liveness: innocents, unreturned jobs, starvation, drill contradiction | **F** as SND-6; the stop-file attribution is parsed (§11.3); T10 (Exhausted -> Alias) adopted; the drill no longer depends on the 60-s rule (metered growth) |
| IMP-7 | major | G2' planning reads ledger6, which is not in the snapshot | **F** §3.4 and §7 (`MergedView` in the snapshot, option (a)); P1 re-checks against ledger6; test |
| IMP-8 | major | Save pause tears ledger6; RAM-guard path lacks a boundary; stop reason not parsed | **F** §11.2 (the token blocks every `&mut`; bookkeeping-only merge), §11.3 (stop-file contract), tests |
| IMP-9 | major | CP6 formats, P3-only fields, segment tails, S3/S5 split, u48 | **F** §11.7 (layouts, meta schema, journal and inflight records), §11.8 (key mapping), trailer patch, sealed segments, typed records and ledger6 persistence moved into S3, u48 preflight |
| IMP-10 | major | Panic path, `consumer_stop` classification, `try_reserve`, preflight penalty | **F** §2.4 (`break_reason`, `panic`), §5.1 (`catch_unwind`, replica rebuild), §5.5 (`try_reserve`), §9.1 (parity rules), §6.4 (preflight without counter change), tests |
| IMP-11 | major | Lockstep B, canonical definition, merge numbering | **F** §6.1 and §8.1 (B = 64 constant), §5.2 (canonical mode defined), §3.1 (one numbering convention) |
| IMP-12 | major | In-memory CP5 import refused by the binding check; edge filter | **F** §11.6 (`cp5_restore_for_import` with the recorded legacy request; edge filter; tests) |
| IMP-13 | minor | S2 closure monitor; index "read-only" | **F** §10.2 and §17.2 S2 (legacy Tracker with forced refreshes; index mutated by P3) |
| IMP-14 | minor | Delta check vs compaction semantics | **F** §3.4 (compaction drops entries with live = 0 at the frozen version; exact ID and version ranges), §6.3 step 3 (transitivity), test |
| IMP-15 | minor | Shared-code visibility, probe shape, `KnownReuse`, `Verified` claim | **F** §18 item 2, §11.5 (backward-compatible probe), §5.1 (`KnownReuse`), §5.3 (guarantee restated) |
| IMP-16 | minor | Stale merge-train hashes, D7 label, self-edges, restore-time gate | **F** §17.3 (tips after review), §9.2 (D7 default), §5.2 and §17.2 (self-edges counted separately and excluded), §15.5 (C) restore time |
| IMP-17 | minor | Missing tests | **F** §17.2. `sealed_hint_growth_under_concurrent_readers` is replaced by `frozen_hint_versioned`, because the live hint array no longer exists; the others (`snapshot_refresh_race`, `result_queue_backpressure_during_pause`, `spill_files_swept_on_restore`, `epoch_u48_preflight`, `d6_hook_off_no_sigma_lookups`) are added |
