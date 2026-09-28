# fable_5_1 v3 walker engine: recommended design

> **Errata (2026-09-28):** §11 lists the Fable 5.1 audit's corrections and the owner's answers of 2026-09-27/28;
> where they conflict with the body below, §11 wins. The body is kept as written.

Date 2026-09-27. Base: branch `fable_5_1` @ 8ea26917. Status: a plan only; nothing is implemented. It combines four judged proposals and two skeptic reviews of each: a single coordinator with a replica, bucket actors, concurrent admission on inspectors, and an epoch merge. W below means `crates/rustred-app/src/application/routed_campaign/walking/`.

Labels: **[M]** measured (v2 campaign 102adcc3/4cb4ab28, final paused `result.json` at 67,788 s, or a cited note); **[M-r]** measured by a reviewer from v2 heartbeats, not re-derived here (S0 re-derives it with a committed script); **[src]** checked in source at 8ea26917; **[E]** estimate.

Nothing in this note is an ETA or a closure claim. The user has allowed a fresh campaign, a semantics bump, a new checkpoint format and a changed `containment_checks` meaning.

## 1. Decision

Add a publication policy `epoch`. It works as follows:
- Inspectors run whole native inspections.
- Each successor is resolved on the inspector against an immutable, Arc-published index snapshot. The first verified container wins.
- One coordinator merges finished inspections in bulk and stays the only mutator of persisted state.

With this, serial work scales with inspections and misses (≈2.7 admissions and 43 edges per native [M]), not with successors (193 per native [M]).

Staging: shared-index levers on the legacy engine under byte identity (S1); the epoch engine in deterministic lockstep for controls (S2-S5); a rolling schedule for production (S6).

`epoch` gets walk semantics version 3 and checkpoint format CP6. Ordered and Ready stay on version 1 and CP5, unchanged, as reference lanes. Bucket-actor sharding and lock-free concurrent admission are deferred (§9).

Why the epoch design:
- It has the smallest concurrency surface. Inspectors read only immutable `Arc`s, so there is no `unsafe` code, no CAS ledger, no save barrier inside native callbacks and no racing edge logs. The reviews found false-closure hazards in exactly those mechanisms in the other proposals.
- Committing whole inspections removes replay tokens, stream contexts and rollback.
- Lockstep keeps a byte-identity oracle for controls.

## 2. Measured facts vs estimates

| Quantity (v2 unless stated) | Value | Label |
|---|---|---|
| Coordinator wall; commit + prep share | 66,896 s; 34,394 + 25,448 s = 89.5% | M |
| Admission requests / successors / events | 5.47G / 5.30G / 6.05G | M |
| Coordinator admission per request | 10.9 µs averaged over the run; 20-36 µs in hours 17-18.6 | M; M-r |
| Forward checks per request | 774 averaged (89.6% rejected by the bit test); 1.8-2.4k late; ≈21 per request per million discovered domains | M; M-r |
| Reverse checks per discovered domain | 4.4k averaged; 10-19k late | M; M-r |
| Natives / delegated publications / transfers | 27.47M / 18.42M / 26.12M | M |
| Discovered / pending | 74.16M / 28.27M | M |
| Inspector net busy per native (slot busy − backpressure) | 6.56 ms, i.e. 34 µs per successor (includes emit-side work) | M |
| Successor tail | 0.2% of inspections carry 60% of successors; the largest in a 57k-record sample had 261k; heads of 0.8-1M in the first campaign | M |
| Dependency edges; closure | 1.19G (16 per domain, 43 per native); total_closed 4.73M of 74.16M; initial 8 of 67; last refresh 126 s | M |
| Bucket skew | 1,166 buckets; the hottest takes 17-30% of admissions; 55-61% of successors land in the parent's bucket | M |
| Memory | 0.38 KB per domain after restore (gen 3: 11 GB for 28.8M); 3.7 KB per domain live on the old binary | M |
| Save / restore | paused save 22.5 s at 74M; restore 164 s at 28.8M; summary rebuild 14.8 s single-threaded | M |
| Pending growth per completion | 0.6-1.9 | M |
| Computing inspectors | ≈0.5 late; 2.57 time-weighted mean over the session | M; M-r |

Corrections to the proposals' inputs:
- Native cost is about 6-6.6 ms per production inspection. It is not 1.0-1.2 ms (that is the W50 control, at 26 successors per native), and not 3 µs per event.
- Late forward checks are 1.8-2.4k per request, not 86-309.

Interpretation [E]:
- v2 was coordinator-bound, and its cost per request grew with the queue (checks per request grew roughly as N^1.3).
- v3 removes the serial ceiling but not that growth. At 10^8 domains, lookup CPU per completion becomes first-order next to native work.
- Native work has never been measured with more than about 6.5 concurrent inspectors.

## 3. Architecture

**3.1 State.** The coordinator owns `EpochState` and is its only mutator. It holds:
- the `domains` array of `CompactDomain`, append-only;
- a canonical store of per-ID `CompactSummary` and word. It is append-only and slots are never reused. Readers see it as immutable Arc'd chunks, and only the tail chunk is copied per merge;
- `ExactIndex`, primary plus overflow, which is the authority for S4;
- the ledger, 8 B per ID: Pending | Reserved | Native{frontiers, error} | Alias{to};
- the protected prefix and the partial-anchor map;
- a `live` bitset, which is the authority for retirement;
- the closure Tracker, the records sidecar, the counters, the u64 epoch k and the watermark W_k.

The snapshot S_k is immutable and is swapped once per merge. For each (phase, owner) bucket it holds:
- the orthant (id, rank);
- up to 16 deterministic hints;
- layers. A layer is struct-of-arrays: u32 ids, u64 words, inline summaries, and a u64 [min, max] version. It is sorted by (signature, word, lower-bound key) into 32-entry blocks with exact envelopes and block OR/AND words.

Layer `retired_at` values (u64) are only a cache; the authority is `live`. The epoch module is `#![forbid(unsafe_code)]`.

**3.2 Inspector.** A job is (id, domain, Arc<Snapshot>). The inspector runs `inspection::inspect`/`routing::inspect` unchanged, with a Resolver sink. For each Admit q:
- The compact image is built from the child slices. Summary, word, digest and signature all come from that image through one function (F1).
- Resolution tries these in order:
  1. job-local KnownReuse (image equality);
  2. **Local**: q ⊆ an earlier miss q1 of this inspection in the same bucket → `Local{ordinal(q1)}`;
  3. a recent-hit list keyed by (phase, owner);
  4. the store-verified orthant;
  5. hints, then layers newest-first; the first verified container wins;
  6. otherwise `Miss{image, version v, ordinal}`. The buffer is byte-bounded and spills when full; it never fails the walk (F19).
- Every positive goes through `verify(t, q)` (F2). The winner is then checked a second time against the canonical store, not the layer copy.
- A long inspection refreshes its snapshot every N events or T ms. Each miss carries its own v.
- The result holds: parent, outcome, emitted-event count, counters, sorted unique targets, misses, locals, frontiers, refusals, anchor scope, and record bytes serialized on the inspector.
- The knob `--epoch-resolve merge` sends every Admit as a Miss. It is the A/B fallback in case lookups on inspectors turn out to be limited by DRAM bandwidth.

**3.3 Merge.** Merges run strictly one at a time: P2 of merge k+1 starts only after P3 of merge k.
- **P1, checks.** The parent is Reserved, the outcome is Ok, the event count equals the count emitted, and allowances hold. Results that were cancelled, consumer-stopped or errored are never merged (F7).
- **P2, parallel per bucket, read-only.**
  - Probe exact.
  - Delta-check against every entry with version > v, retired or not.
  - Form the antichain of the epoch's misses, independent of order: a miss survives iff no other miss strictly contains it and no miss with an equal summary has a smaller canonical key (digest, then image bytes).
  - Compute each survivor's reverse set, filtered by `live`.
  - In controls, also verify every shipped hit here.
- **P3, serial apply.** Every reservation is preflighted before the first mutation (F9).
  - Assign IDs from W_k in (first parent, ordinal) order. Push domains, summaries, exact entries and Pending ledger entries.
  - Retire: clear `live`. Then transfer each Pending, unprotected candidate to its smallest containing survivor, after fully re-verifying containment on canonical domains. Publish the alias in the order record, then edge old→new, then seal.
  - For each result, in parent order:
    1. Sort and dedup the final edge set: targets, resolutions of misses and Locals, pre-admitted reuse, and the anchor.
    2. Bulk-append the edges.
    3. Set the entry Native.
    4. Call `finish`, which seals iff the outcome is Ok with 0 frontiers.
    5. Append the record bytes.
- **P4, parallel.** Build layers for the survivors, recompute hints and publish S_{k+1}. Size-tiered compaction runs in the background and lands in a later snapshot. It never drops an entry that a live snapshot can still see.

**3.4 Schedules.**
- `Lockstep{B, depth 1|2}`: deterministic, for controls. `--canonical-resolution` picks the minimum ID among the snapshot's verified containers; the identity gates use it.
- `Rolling{D, cut R results or T ms}`: for production.
- Controls per schedule (amendment of 2026-09-28, c4l fix round). Lockstep with canonical resolution is gated on C-4L-comb-O: the combined four-loop `four-all` gives byte-identical records across W6/W24/W96, audit and verify-closure gate PASS, natives <= 33,175 one-sided or a pre-registered expectation. Rolling is gated on C-4L-comb-R: a pre-registered one-sided Fisher test of the drain fraction against interleaved legacy 4a17f9c7 Ready runs, n >= 13 per arm and width. Both are defined in `examples/input/four_loop_combined/README.md` §5 and are additions to the per-family C-4L.
- Dispatch is FIFO over Pending. An entry is set Reserved *at enqueue*. After a resume, Reserved entries go first.

**3.5 Closure monitor.** The refresher snapshots the sealed bits and the edge length at a merge boundary. That boundary is quiescent: every sealed node already has all its edges below that length.
- The refresher computes reachability off the coordinator, and the result is OR-ed in at a later boundary.
- The edge log gets u64-capable offsets (or a segmented CSR) and folds independently of saves (F14).

**3.6 Split** [E, tuned in S7]. W150 on CPUs 28-177: 1 coordinator, 1 refresher, 12 merge helpers, 136 inspectors. Shared read-mostly data uses `numactl --interleave` over nodes 0-5. The SMT siblings 284-383 of CPUs 28-127 can carry foreign load [M-r].

## 4. Semantics changes (`epoch`, WALK_SEMANTICS_VERSION 3)

- **Per-policy versions.** `walk-semantics-version` prints {ordered: 1, ready: 1, epoch: 3}.
  - The legacy lanes stay byte-identical on CP5 (S1 gate), so this tree can still resume the frozen v2 checkpoints.
  - The number is 3, not 2, to avoid confusion with the "v2 campaign".
  - The legacy lanes can be deleted after launch.
- **Bump contract** (`W/mod.rs`).
  - Semantics consist of: S1-S4; native event emission and the effect of each event; whole-inspection commit; the ledger, seal, edge and transfer rules; the record schema; and the meaning of CP6.
  - Performance-only, needing no bump: the choice among verified containers, scan order, the recent-hit, hint and Local policies, layer layout and compaction, snapshot refresh, B, D and cut sizes, the worker split, pinning and the allocator.
- **Whole-inspection commit.** No partial streams exist. `epoch` does not use `execution/replay.rs`, stream contexts or Ready credits.
- **Containment winner.** Any verified container in the same (phase, owner), whether live, retired or aliased, found first in locality order. The minimum ID is used only in canonical mode.
- **Assignment and retirement.**
  - IDs are assigned at merge in (first parent, ordinal) order, and retirement happens only at merge.
  - A transfer goes to the smallest containing survivor under the rule "to > id, same key". The old `representative + 1 == len` check [src ledger.rs:276] is dropped.
  - Transfer and alias publication are one step.
- **Allowances and refused lanes.** Allowances are enforced per inspection or per merge. Crossing one is a walk error, not a capped prefix. `epoch` refuses finite `max_containment_checks`, subdivision and OwnerBatched.
- **Counters.**
  - `containment_checks` becomes the forward comparisons made on inspectors plus those made in the merge. This breaks the legacy metric, as allowed.
  - `containment_maintenance_checks` becomes the actual reverse comparisons, and the `maintenance_len` preflight goes away.
  - New counters: `resolution_by_source`, stale misses, merge P1-P4 wall, verify counts and mismatches, and snapshot lag.
- **Records and events.**
  - Records are typed, serialized on inspectors, and keep the keys the audit reads.
  - `accepted_events` equals the inspection's events. Records carry an `epoch` field, and the sidecar follows merge order.
  - Per-domain progress events are dropped; heartbeats stay.
- **Resume binding.** The binding drops `workers`, `inspection_workers`, the schedule, D and merge helpers. Today it hashes `workers` and `format!("{:?}", scheduling_policy)`, which includes the lookahead [src checkpoint.rs:104-118].

## 5. Soundness argument

- **Obligations.** Each admitted ID has one ledger entry. It leaves Pending only in one of two ways: through its own merged native result (Reserved → Native), or through a verified transfer published in the same step (Pending → Alias). Exhaustion requires Pending = Reserved = ∅ and no unmerged result.
- **S1: every alias is a checked containment.** Every positive (Local, recent-hit, orthant, hint, layer, delta, transfer) is decided by `verify(t, q)`, which checks in order:
  1. t < the published length (for a Local, t is instead q1's image from the same inspection);
  2. the stored `CompactDomain` of t has q's (phase, owner);
  3. the native predicate holds.

  Step 2 is needed because `CompactSummary::contains` compares owner but never phase, and returns true for an EMPTY candidate before the owner test [src compact.rs:513-543]. A stale or incomplete lookup yields only a Miss, which the merge re-checks. A retired or aliased t is still an obligation.
- **Completeness of the miss decision.** This is a work property, not needed for soundness. The inspector scanned the live entries at version v. Every entry retired at or before v has a live container at v, by transitivity. The merge checks every entry newer than v, plus the epoch antichain.
- **S2: every edge is recorded before its source seals.**
  - Each parent's full, deduplicated edge set is appended in P3 before `finish`.
  - An alias appends old→new before it seals.
  - Frontiers and errors keep the parent unsealed.
  - The Tracker guard against an edge from a sealed source stays.
- **S3: each responsibility is published once.**
  - A result merges only if its parent is Reserved, and a Reserved entry never transfers.
  - Transfers go only from entries that are Pending, unprotected, live and not an Alias, to a survivor in the same bucket with ID ≥ W_k > old.
  - So chains stay acyclic even when two summaries contain each other.
- **S4: domain images are unique.** The exact probe in P2 and the insert in P3 are the only writers. Misses are deduplicated by digest plus image equality. Restore refuses duplicates.
- **No clipping.** Every successor ends as one of: a verified target; a Local that resolves to q1's resolution; a survivor; an intra-epoch container; or a retained frontier.
- **Consistent cut.** Only P3 mutates persisted state, and saves happen only between merges. In-flight results are never persisted.
- **Closure.** The refresh snapshot is taken at a quiescent boundary. Targets outside the snapshot count as unsealed and are never dropped. Closed sets only grow.

Gaps from the reviews that must be fixed. Each is gated in the stage listed in §7:

| # | Fix | From |
|---|---|---|
| F1 | One geometry source. Summary, word, digest and signature are derived from `CompactDomain`. P3 recomputes digest and summary for every survivor instead of trusting inspector geometry. | P1-G1 |
| F2 | A single `verify` with an explicit phase+owner check. Orthant hits are store-verified (full orthant + key + `rank_contains`). Caches and hints are keyed by (phase, owner). | P4-G1, P1-G2/G3 |
| F3 | `ExactIndex` with overflow everywhere. Miss dedup (per inspection and per epoch) uses digest plus image equality. Tested with the forced-collision seam, including save/restore. | P4-G2, P3 |
| F4 | Every transfer is re-verified against canonical domains, and allowed only if the entry is live, Pending, unprotected and not an Alias. Restore checks old ⊆ to for every Alias, in parallel. | P4-G4/G5, P1-G10 |
| F5 | Hits: controls verify every hit at merge. Production double-checks each winner on the inspector against the canonical store, and the merge re-verifies a sample of at least 1/64. The 1/1024 sample is not used. | P1-G4 |
| F6 | Each parent's final edge set is sorted and deduplicated in P3, and the bulk API asserts it is strictly increasing. Add a control with `--reuse-initial-d-bands` that restores twice. | P4-G3, P1-G6 |
| F7 | Merge only when the outcome is Ok and the emitted-event count equals the result's count. Resolver errors (coordinate > 65534, summary error, overflow) are non-cancellation failures. The audit checks `accepted_events == stats.events` per record. | P4-G7, P1-G5/G12 |
| F8 | sealed ⇔ (Native ∧ 0 frontiers ∧ no error) ∨ Alias, in both the restore validator and the audit. | P2-G10 |
| F9 | P3 cannot fail once it starts mutating: every reservation is preflighted, and ID allocation fails hard before u32::MAX. A failure poisons the run and blocks any save. | P2-G7, P3 |
| F10 | An independent edge-coverage audit. Re-inspect sealed natives with the reference reducer: all of them on FG and X, and at least 10^4 on five-loop. Every Admit and pre-admitted successor must be contained, in the same phase and owner, in a recorded target or its alias chain. Each record stores its distinct-edge count, and restore checks the out-degrees. | P4-G12, P2-G9, P3 |
| F11 | Reserved is set at enqueue. Merges are serialized. P3 re-reads the ledger. | P4-G6 |
| F12 | The delta check covers every entry newer than v. Compaction keeps accurate version bounds. The digest tie-break for mutual containment is the same in the antichain, the transfer target and the to > id rule. These properties are tested against a brute-force reference. | P4-R2 |
| F13 | Epoch and version counters are u64. | P4-G10 |
| F14 | The edge log has no u32 `LOG_CAP` [src edges.rs:14]. Folds are independent of saves, avoid a full CSR copy, and have their peak memory budgeted. | P4-G13/R2 |
| F15 | The forced refresh before a save or stop is parallel, on a frozen prefix. If an allocation fails, the monitor is marked unavailable before any later seal. | P3, P4-R2 |
| F16 | CP6 persists the partial-anchor map, so the anchor-edge checks survive restore. | P4-G8 |
| F17 | D is not bound. Restore validates the Reserved count against the saved D, never the new one. | P1-G8, P4-G11 |
| F18 | A RAM-guard stop has no grace period: unmerged results are discarded and the save happens at once. Record counts come from the manifests. | P4-G11, P3 |
| F19 | The miss buffer spills when its byte bound is reached; it never fails the walk. | P4-R2 |
| F20 | Refused lanes (see §4). The audit reads `workers` from the last session's command. | P1-G13, P2-G11 |

## 6. Checkpoint and resume (CP6)

- **Sections.**
  - Append-only: `domains` (existing codec), `edges` (insertion order), `records` (sealed sidecar).
  - Rewritten each generation:
    - `meta`: counters, k, W_k, protected prefix, uncommitted receipts, inputs and input frontiers, closure counters, and the schedule (informational);
    - `ledger6`: 8 B per ID, written by a parallel chunked writer, ≈0.6 GB at 74M [E];
    - `anchors`;
    - `nodes`: 1 B per ID;
    - `live`: a bitset.
  - Not saved: layers, hints, the exact map, summaries and in-flight results. All of them are rebuilt, so save cost stays proportional to new state plus ≈10 B per ID (C7).
  - Writers stream from memory without building JSON `Value` trees, so a save fits in the RAM left under the guard. `latest.json`/`previous.json` stay atomic (C6).
- **When saves happen.** Only between merges.
  - Lockstep saves at quiescent points. A pause plus resume in canonical mode is then byte-identical to the uninterrupted run.
  - Rolling saves while inspectors keep computing. Dispatched but unmerged IDs are saved as Reserved, and results that finish during the save wait in a byte-bounded queue.
- **Stop paths (C6).**
  - Cooperative stop (stop file or SIGINT):
    1. Stop dispatch.
    2. Merge the results that have already finished.
    3. Allow at most 60 s of grace, then cancel.
    4. Record the discarded work as `uncommitted` receipts with `resume_reinspects_unfinished_part:true`.
    5. Run the parallel forced refresh (F15), save, and exit 4.
  - RAM-guard soft limit: the same path, with no grace and no merging (F18).
  - Hard kill: the previous generation stays valid.
  - A heavy head that was in flight is re-inspected from scratch. That is the cost of C3 option (b).
- **Restore (C4).**
  1. Check digests in parallel and decode domains. Rebuild the exact index sharded by digest, keeping overflow and refusing duplicates.
  2. Validate `ledger6` and `anchors`:
     - only allowed states occur;
     - each Alias has to > id, the same key and an admitted target, and is not protected;
     - counts equal meta, and F8 holds.

     Then check Alias containment in parallel (F4).
  3. Run the Tracker's `from_parts`/`restore` checks: closed ⇒ sealed, and no edge from a closed to an unclosed node.
  4. Run the equivalent of `validate_ledger_closure`: every alias and anchor edge is present, and each out-degree is at least the record's edge count. Check that records equal publications and that events equal Σ `accepted_events`.
  5. Rebuild summaries for every ID, so `verify` never reads an absent summary even for a retired t. Rebuild live layers and hints in parallel. Dispatch Reserved entries first.
- **Binding (C5).**
  - Bound: the request/policy digest minus transport, the owner digests, the `epoch` semantics version, and CP6.
  - The executable is recorded only.
  - `production_saved_owner_campaign.py --upgrade-executable` learns the per-policy probe and CP6, and allows a re-split at resume.

## 7. Staged implementation plan

Controls run on CPUs 200-255: four-loop FG/BMW/H/X at W6 and five-loop finite at W50. Pilots run on CPUs 128-177. The reference binary is `4a17f9c7`. Every stage ends in a review round.

- **S0: oracles and measurement (≈3 d; no engine change).**
  - Audit additions: policy `epoch`; `--require-closure` (available, `unresolved_domains == 0`, `initial_closed == initial_total`); the per-record events check; alias containment; the F10 coverage tool.
  - `compare_walk_records --mode invariants`.
  - Offline replay on a TMP clone of the v2 gen-3 checkpoint (28.8M domains), or gen-7 if memory allows:
    - re-inspect about 10k Pending IDs and replay their Admits;
    - compare minimum ID, first-found, the recent-hit list (k = 1-16), Local, and struct-of-arrays compacted layers;
    - measure callbacks, ns per request and blocks touched, split by final outcome, with 1 thread and with 48 threads.
  - Native scaling of five-loop at W24 and W48 (µs per successor, with interleave).
  - M1 call graphs.
  - Gate: projected lookup cost is at most 2× native CPU per completion at 28.8M. Otherwise `--epoch-resolve merge` becomes the default path in S4.
- **S1: shared index primitives on the legacy lanes (≈4 d).**
  - Changes: struct-of-arrays blocks with inline ids, words, summaries and envelopes; per-ID immutable summaries (the slab free list goes); block OR/AND words; a `partition_point` skip in `find_from`. The minimum-ID rule and the callback order are unchanged.
  - Gate:
    - Ordered strict: 0 differing records and identical `containment_checks` on all five controls against `4a17f9c7`;
    - Ready audit PASS;
    - CP5 resume works in both directions;
    - report ns per check (target ≤15-20 ns per check-equivalent [E]).
- **S2: epoch skeleton (≈5 d).**
  - Lockstep depth 1. Serial merge through `Queue::admit` (minimum ID), with retirement in the merge. No lookups on inspectors, no checkpoints. Covers F6-F9, F11 and F20.
  - Gate:
    - audit, `--require-closure` and F10 PASS on all five controls;
    - byte-identical across W6, W12 and W24, and for five-loop W24 against W50;
    - native count against Ready reported, with differences above 5% flagged.
- **S3: CP6 and the stop paths (≈5 d).** Covers F8 and F16-F18.
  - Gate on FG, X and five-loop:
    - a pause at epoch k plus resume is byte-identical;
    - `kill -9` after a periodic save, then resume, passes;
    - a RAM-guard soft stop through `shared_owner_campaign.py` exits 4, and the resume passes;
    - a double restore with d-bands passes;
    - injected corruption is refused;
    - a rebuilt binary with the same version resumes under a new split.
- **S4: layered snapshots and resolution on inspectors (≈6 d).** Covers F1-F5, F12, F13 and F19, plus Local, snapshot refresh and canonical mode.
  - Gate:
    - verify-all shows 0 mismatches;
    - audit, closure and F10 PASS;
    - canonical mode is identical across worker counts and across pause/resume;
    - first-found against canonical keeps the native count within ±5%, distinct edges per domain within 10%, and a closure trajectory no worse;
    - property tests against brute force pass;
    - the S0 replay is re-run with the real layers.
- **S5: parallel merge and diet (≈5 d).** Parallel P2, typed records on inspectors, bulk edges, `ledger6`, F14 and F15.
  - Gate:
    - serial and parallel merges are byte-identical on every control;
    - the frozen-prefix refresh equals the forced scan at quiescence;
    - a coordinator perf profile;
    - five-loop W50 traversal against `53e672fc` Ready, two rounds each.
- **S6: lockstep-2 and rolling (≈4 d).** Saves run concurrently with inspection.
  - Gate: audit, closure and F10 PASS, with 3 pause points per control. Report the stale-miss rate, merge duty and inspector utilization. Pick rolling unless lockstep-2 reaches at least 85% of rolling's utilization.
  - Gate: C-4L-comb-R PASS for the chosen non-deterministic schedule (no regression of the `four-all` drain fraction against interleaved legacy Ready at W6/W24/W96).
- **S7: pilots on CPUs 128-177 (≈3 d plus wall time).**
  - (a) The engine on the **v2 inputs** at W50 for ≥6 h, compared with v2 at matched discovered domains (µs per event against D). This separates the engine gain from the input change. The run includes one RAM-guard save-and-stop plus resume, one performance-only binary swap and one re-split resume.
  - (b) The v3 inputs for ≥6 h, with a 100-150 GB guard.
  - Launch gates:
    - every resume passes the audit and the canary shows 0 mismatches;
    - completions per hour are at least 3× v2 at matched D;
    - RSS per domain is at most 0.6 KB;
    - save stalls stay under 2%.
  - Then launch fresh on CPUs 28-177 with 4 h checkpoints and a 700 GB guard. First confirm that the interim `32fdec09` process has exited: it was seen at 339 GB RSS [M-r].

Total effort: ≈35 d of stage work, ≈40 d with review rounds [E]. The reviewers judged the proposals' 23-27 d optimistic.

## 8. Expected gains (all [E]; not ETAs)

- **Serial apply:** ≈5-15 µs per native (one result, 43 edge appends, 2.7 survivors, 0.67 aliases). That is a ceiling of about 65-200k natives/s, so it does not bind.
- **Inspector time per native:** p_nat ≈ 6-6.6 ms [M] plus 193 × c_lookup.
  - With struct-of-arrays layers, first-found and hits, c_lookup ≈ 2-25 µs at 30-74M domains. That gives 7-11 ms per native, i.e. about 12-19k natives/s on 136 inspectors, if native work scales.
  - With today's per-check cost (30-200 ns and 1.8-2.4k checks), c_lookup is 50-480 µs and the ceiling falls to about 1.4-8k natives/s.
- **Planning band:** **5-20× the completions per hour of v2 at matched D.** The floor is 3-5× if lookup stays near v2 cost or memory bandwidth binds. For reference, v2 ran 0.61-0.76M/h late [M-r] and 1.48M/h averaged over the run [M].
- **What bounds it, in order:**
  1. native scaling across 136 threads and both sockets (unmeasured beyond ≈6.5);
  2. lookup CPU and random DRAM traffic, both of which grow with D;
  3. hot-bucket miss scans and the merge antichain;
  4. the closure refresh (O(nodes + edges)) and save stalls;
  5. memory.
- **Memory:** 0.45-0.8 KB per domain [E], against 0.38 for the restored state [M]. That puts the 700 GB wall at about 0.9-1.5G domains. At the planning band (≈2.7 discovered domains per native [M]) it would be reached in roughly 1-7 days if pending keeps growing.
- **What the engine does not change:** pending growth per completion (0.6-1.9 [M]). A faster walk gives an earlier verdict on the inputs; it is not convergence.

## 9. Deferred and rejected

- **Deferred: bucket actors on K shards, with CAS state words and no coordinator.**
  - It has the widest concurrency surface. The reviews found false-closure paths: EdgeLog prefix publication, an alias edge written after the seal, publication of a cancelled stream, and a termination race.
  - It also risks lock-order deadlocks and write-lock convoys in the hot bucket, and skew caps per-bucket commit at 1.3-2.3× (relief note E6).
  - Revisit only if P3 apply becomes the measured limit.
- **Deferred: concurrent admission on inspectors with a CAS ledger and a Dekker barrier.** This is the fallback if merge latency hurts.
  - Its barrier parks inside native callbacks, and nobody has measured whether they tolerate that.
  - The refresher and sidecar sit outside the barrier.
  - Tombstoned IDs lack summaries after restore.
  - Adopted from it: first-found, the hit cache, immutable summaries and idempotent re-inspection.
- **Rejected: a single coordinator with a left-right replica and Delta rollback.**
  - Serial work per successor keeps its ceiling at about ×5-15, the replica needs unsafe code, and the rollback breaks the Ordered resume gate.
  - Adopted from it: one geometry source, Local, a live bitmap instead of an index section, typed records, and JSON only at heartbeat cadence.
- **Rejected:**
  - sub-sharding a bucket by coordinate hash, because containment is not hash-local;
  - retirement that is asynchronous or can be dropped (26.1M transfers against 27.5M natives [M]);
  - verifying only 1/1024 of hits in production;
  - minimum ID as the production winner (kept only in canonical mode);
  - replay tokens.
- **Deferred to after the pilot:** per-NUMA copies of hot layers; mimalloc or jemalloc (performance-only); per-signature-group locks.
- **Out of scope:** changes to the physics envelope and helper bounds are independent of the engine. S7(a) A/B-tests the engine on the v2 inputs first.

## 10. Open risks

- Lookup cost at 10^8-10^9 domains is unmeasured. S0 and S4 gate it, and `--epoch-resolve merge` is the fallback.
- First-found may pick younger, unsealed containers, which could slow root certification. The S4 closure-trajectory gate covers this, and hints can prefer sealed containers when costs tie.
- Heavy heads lengthen stops and lockstep epochs; rolling is the production schedule.
- Faster discovery reaches the RAM wall and disk limits sooner. At 20× the records sidecar grows by up to about 15 GB per hour [E] (≈620 B per record [M]).
- Keeping two engines in one tree adds review load, and fixes can diverge between them. Decide after launch whether to delete the legacy lanes.

## 11. ERRATA (2026-09-28)

Added by the integrator on 2026-09-28, when the W0/W1 lane branches were merged into `fable_5_1`. The body above is
kept as written; where it conflicts with an entry below, the entry wins. Sources: `FABLE_5_1_CRITIQUE.md` §5 (numeric
errata, re-derived from receipts) and §3 (epoch-design findings), `HANDOFF_opus_5_5.md` §0.1 item 4 (audit errata) and
items 11-12 (owner answers). The W2.0 protocol note rev 2 (`docs/research/fable51_w2_epoch_protocol_2026-09-28.md`)
settles the §0.1 item 8 design items (F7 vs A2, attempt counter, rolling replay oracle, CP6 cut, summaries, first-found
bias, merge helpers, W3.2 inside W2). "Plan" below is `docs/research/fable51_next_push_master_plan_2026-09-27.md`.

### 11.1 Audit corrections (handoff §0.1 item 4)

1. Union-only residual inspections (W0.9 level-cell oracle): C-5F 0.521x, C-HOT-sub r1a12 0.423x, r2a11 0.394x,
   C-HOT 0.328x. The 0.346x / 0.288x quoted elsewhere are the theta = 0.4 dense-cell rows.
2. Gen-7 restore: 567 s / 598 s including the 108 s verify phase (launch to restored 570 / 600 s), not 458.8 s.
3. RAM: 0.3 KB/domain is unsupported. Measured legacy marginals are 0.5-1.1 KB/domain (late window ~0.5); restore
   VmHWM 0.64 KB/domain; 600 GB holds ~0.75-1.2G domains.
4. Native scaling WAS measured at K = 96 (3.75x CPU per native vs K = 1, quiet session C); "never measured beyond
   ~6.5" is stale.
5. C-HOT is audit-only (CP3 state). About 1e8 positive inclusions (3.8e9 counts `contains()` calls). Successor
   events 25.08M / 25.46M (28.4M counts admitted successors).
6. There are two C-HOT-sub boxes: r1a12 (falsify lane; 1.02M natives; drains at W12) and s2/r2a12 (knobs lane;
   2.12M natives; drains at W48). Every gate names its box.
7. mimalloc compiled ratios 0.897 / 0.929 / 1.002 / 0.900 (not "x0.87-0.93").
8. The restored-baseline "denser index" reading is refuted.
9. Stale in the governing notes: the 700 GB guard, CPUs 28-177, W150 / 136 inspectors and the unrescaled 5-20x band.
   Read them as: a 600 GB cap with a 50 GB host MemAvailable save-and-stop floor (§11.3), the 100-core launch on
   (shared) socket 1, and bands rescaled to the measured native scaling (W0.3: per-CCX replicas 1.14-1.17x [E, loaded,
   provisional], projection bracket 1.42-3.75x per `docs/research/fable51_w0_harness_2026-09-27.md`).

### 11.2 Numeric and consistency errata (critique §5, re-derived from receipts by the auditor)

Locations refer to the handoff (`HANDOFF_opus_5_5.md`) unless marked "plan" or "v3".

| # | Handoff / plan text | Correct or missing | Location |
|---|---|---|---|
| 1 | gen-7 restore 458.8 s (490.1 s run2) | verify 108 s excluded: 567 s / 598 s; launch→restored 570 / 600 s; at the P-IMP 10-min limit | 5.5, 7.5, W3.3 |
| 2 | RAM wall 10-250 h at 0.3-0.8 KB/domain | 0.3 KB unsupported; measured marginals 0.5-1.1 KB (late window ~0.5); restore VmHWM 0.64 KB/domain | 5.2, 5.5, §3.8 |
| 3 | §11: native scaling "never measured beyond ~6.5" | measured at K=96 (7.4); stale R3 text | 11, plan R3 |
| 4 | "both oracles pass on every drained output"; "3.8e9 inclusions"; "successors covered 28.4M" | C-HOT audit-only; ~1e8 positive inclusions; successor events 25.08M/25.46M | 5.7, 7.1 |
| 5 | pure union residual C-5F 0.346x, C-HOT-sub 0.288x | those are dense-cell rows; union-only 0.521x / 0.423x | 5.2, 7.3 |
| 6 | "C-HOT-sub" | two boxes: r1a12 (falsify, 1.02M natives) and s2/r2a12 (knobs, 2.12M) | 7.3 vs 7.6, 10.2 |
| 7 | mimalloc x0.87-0.93 | tables: 0.897/0.929/1.002/0.900 (compiled), 0.899-0.971 (env) | 5.4, 9.2 |
| 8 | smoke-fifo "reproduces 98,841 natives" | a Ready run (non-deterministic); canonical Ordered FG is 98,869 | 7.6 |
| 9 | dispatch order "natives up to 10.6-14.5%" | W48 BMW SV x0.508 and C-HOT-sub(s2) SV x0.650 omitted from the summary | 5.3 vs 7.6 |
| 10 | 27.34M admissions; 6.4-7.9x; 4.6-7.6x | 26,748,231; 7.9-9.5x (SoA-id) / 4.8-7.5x (SoA-pattern); 4.0-7.1x | 7.7 |
| 11 | gate 0.7 PASS [M] | receipt binary predates the uncommitted conservative cover.rs fix; four-loop cover uninterpretable (15-70% unevaluated); cost exponents unreconciled with plan §1 | 5.7, 7.8 |
| 12 | gate 0.5 PASS | scored on one 60-s window inside a 12-min run cut by a crash, 43 foreign CPUs | 7.5 |
| 13 | governing notes | v3 note: 700 GB guard, CPUs 28-177, W150; plan: 700 GB guard beside 600 GB, per-family C-4L | v3 §3.6/§7, plan §3.8/§4/§5 |
| 14 | labels | [M-r], [M-off], [src], "derived here" used but undefined; §3.3 refers to 7.2 for gaps in 7.1 | header, 3.3 |
| 15 | test counts 777/783/784, Python 226/230 | reconcile exactly (+6 oracle tests; +9 order tests -2 failures; +4) | 3.2, 7.1, 7.6 |

### 11.3 Owner answers 2026-09-27 ~21:35 UTC (handoff §0.1 item 11)

- Socket 1 stays SHARED at launch: no cpuset; foreign load is recorded in every receipt.
- NO ZFS ARC cap.
- Host MemAvailable save-and-stop floor: 50 GB (was 20 GB).
- FULL PLAN, GATED: continue W2 only if the S2/S4 epoch skeleton beats the legacy + SoA-kernel comparator (25-min
  gen-7 resume vs M1 run2, matched window) by >= 1.5x; otherwise fall back to MVP-B, then MVP-A, and report.
- Launch criterion (F) is thereby settled: shared host, no ARC cap, 600 GB cap, 50 GB host floor.

### 11.4 Owner answers 2026-09-28 ~09:40 UTC (handoff §0.1 item 12; binding, listed for completeness)

- G2' residual anchors ALLOWED. Plan §6 S7 (G2') and plan §3.11 are amended: an anchor is any merged record whose domain is fully
  discharged (Native, or a validated merged G2' residual record = its residual plus its own anchors), resolved in merge
  order (well-founded); CP6 validators, the audit and the G2' mutations check it. D2 is settled as dispatch-time G2'
  in union form (no admission-time general union).
- Symbolica: per-CCX replicas only, no Symbolica patch (the owner may raise the shared `Arc<PolynomialContext>`
  refcount design upstream).
- I2: rebuild with one coordinate frame per owner and re-gate with a total-work cap (natives, Apply natives and domains
  no worse) in addition to >= 20% fewer Route->Route edges with audit PASS; ship only if it passes.
- D7: always stop at the first frontier (fresh start after an input fix). D8: the deliverable is the symbolic closure.
  D6: off at launch, not launch-blocking, deferred until its admission cost is measured on the epoch path. D1(b): no
  (four-loop evidence). I1b dropped.

### 11.5 Where this note's body is affected

- §3.6 split (W150 on CPUs 28-177, 136 inspectors, `numactl --interleave` over nodes 0-5): superseded by the 100-core
  launch on shared socket 1 (§11.3). Interleaving is the wrong remedy for the measured native-scaling loss (critique
  §2.1: remote fills 63% at K = 96, 81% interleaved); owner programs are replicated per CCX (§11.4); layer and index
  placement is an open W3.5 measurement.
- §7 S7 launch line ("launch fresh on CPUs 28-177 with 4 h checkpoints and a 700 GB guard"): the owner launches; 100
  cores, a 600 GB cap and a 50 GB host floor (§11.3).
- §8 expected gains (136 inspectors, the 5-20x band, "native scaling ... unmeasured beyond ≈6.5", the 700 GB wall at
  0.9-1.5G domains): read with §11.1 items 3, 4 and 9. These remain [E].
- §5 soundness and §6 CP6 anchors: CP6 must carry G2' residual-anchor kinds from day one (§11.4; plan §3.11 as amended).
- §10 "`--epoch-resolve merge` is the fallback": a diagnostic, not a production fallback (critique §3 EPOCH-7; handoff
  §0.1 item 8).
