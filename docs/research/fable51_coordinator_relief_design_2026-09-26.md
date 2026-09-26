# Coordinator relief for the Ready walk: design note (2026-09-26)

Branch `fable_5_1` @ 88ae6fbd. W = `crates/rustred-app/src/application/routed_campaign/walking`.

This note is a design only. Nothing has been implemented and no live campaign was touched. Inputs:
- the per-request coordinator path map;
- four proposals, each reviewed by two equivalence/gain skeptics (scores 7/7, 6/5, 4/4, 7/6);
- read-only checks of W/queue/prepared.rs and W/execution/admission.rs.

## 0. Short answer: can the coordinator itself be parallelized?

**The decisions are not inherently serial.** Every admission decision reads and writes only its own `(phase, owner)` bucket: exact entries, index, orthant, and the immutable summaries, bits and domains. IDs enter a decision only through their relative order.

**Only ordered bookkeeping is inherently serial:**
- global ID assignment;
- ledger op order;
- closure edge order;
- per-stream replay chains;
- stream context switching.

With unlimited caps, none of these feeds back into a decision.

**Sharding the commit by bucket is not the right first lever.**
- Skew bounds it. Within a chunk, 55-61% of successors land in the parent's own bucket. In a commit-weighted sample, one bucket carries 30%.
- The proposed protocol also breaks pause/resume as written (section 6).

**Better: take the decisions off the coordinator.**
- About 95% of requests are snapshot containment hits. The helper's verdict for them is already final after O(1) "unchanged since snapshot" checks.
- Step 1 turns their commit into bookkeeping. It is byte-identical.
- Step 2 removes the per-batch helper barrier, which is 30% or more of wall and rising. It can be done byte-identically for a small gain, or by moving lookups to the idle inspectors. The inspector route is large but changes the `containment_checks` trajectory (section 4).
- Estimated coordinator throughput: about x1.15-1.4 from the byte-identical items, and about x2.1-3.1 if inspector-side resolution passes its gates.

## 1. Measured facts

Sources: live `events.jsonl` heartbeats (binary 102adcc3), `/proc` for the main thread, and the five-loop W50 Ordered control `result.json`.

The numbers were read by different reviewers at different times (t = 1 ks to 10.2 ks), which is why ranges appear. I did not re-read the live events for this note.

**Coordinator duty shares (% of coordinator wall)**

| Window | commit | prep | dispatch | progress_json | poll | publication | closure | untimed |
|---|---|---|---|---|---|---|---|---|
| first 17 min | 47.1 | 22.2 | 9.5 | 6.8 | 3.7 | 2.7 | 1.1 | – |
| session to 7,286 s | 52.9 | 29.0 | 5.0 | 2.3 | 2.3 | 1.4 | 1.2 | 5.7 |
| 8.5-10.2 ks | 52.2-52.3 | 30.0-30.2 | 5.0 | 2.2 | 2.3 | 1.4 | 1.05-1.2 | 5.4-5.6 |

The prep share reached 35.8% in the latest 10-min window.

**Admit outcomes:** containment hit 94.5-94.7%, exact 2.7%, orthant 1.25-1.3%, miss 1.5-1.56%. 98-99% of Admits go through prepared batches.

**Per prepared batch:**
- 174.6 records, about 168 admissions, about 2 new domains, about 1.3 retirements.
- prep 302 us (session) to 334 us (later); commit 549-578 us; inter-batch gap about 195 us.
- 1.18-1.22 prepared batches per returned inspection, so most batches are the first batch of their chunk.

**Growth over time:**
- Commit per record: 2.6-2.9 us below 10M domains, 3.8-4.3 us above 20M.
- Prep per batch: 165 us to 450 us, as mean forward checks per request rose from about 86 to about 309 (718-921 in heavy windows).
- Admission in the late window: about 6.9-7.3 us per request.

**Coordinator-bound state:**
- 4-12 of 67 inspectors computing.
- 185-200 finished inspections awaiting commit.
- Escrow full at 134/134.

**Main thread (`/proc`):**
- on-CPU about 67%;
- stime 17% of its CPU time;
- 48M voluntary switches (about 6.4 per batch);
- 1.46M migrations across CPUs 28-127 (NUMA 0-3).

**progress_json:** 189-213 s total over about 6.6M delegated publications, about 30 us each (R² 0.997). RoutedProgress keeps only the latest event and samples once per second.

**Skew:**
- Published-domain sample (7,005 events): 1,166 buckets; top bucket 16.8%, top 12 together 43.6%.
- Heartbeat `domain_progress` sample (4,918, commit-weighted parent): top 30.1%, top 2 41%, top 4 56%.
- W50 control: 60.6% of successors are in the parent's bucket (54.4% for inspections with at least 256 successors).

**Code facts (read):**
- `exact` is append-only.
- `bucket.orthant` has a single write site.
- `on_retire` is the only index removal.
- `revalidate`'s overflow guard is unreachable (about 5e11 against 1.8e19).
- `commit_chunk` takes 256 raw records, then filters replay (admission.rs:414-429).
- The prepared `checks` field counts forward callbacks only. Reverse work is charged through the `maintenance_len` bound on both paths (prepared.rs:29-31).
- `containment_checks` already depends on batch boundaries, the MIN_* gates, helper presence and snapshot lag.
- The table in section 4 of the scheduler design doc claims that lag-1 pipelining keeps persisted counters identical. That claim is false for `containment_checks`.

## 2. Estimates and hypotheses (not measured)

**E1: hit commit cost.** The per-hit commit is dominated by DRAM/TLB latency plus hashing, not by index size. Each hit repeats:
- the exact SipHash and table probe;
- the `by_owner` hash;
- the summary clone and word recompute;
- `is_live` (SipHash plus a binary search over 288 B blocks);
- `domains[f].contains`;
- two cross-arena `free` calls.

Removable per hit: proposer 0.8-1.6 us on the compact base; skeptics 0.45-1.4 us, citing memory-level parallelism and an L1-hot clone. Commit time rising with domain count is consistent with this, but no profile exists yet.

**E2: prep cost model.** A regression fit gives about 101 us fixed per batch (rayon wake-up plus latch sleep), plus 0.25 us per request, plus about 2.9 ns wall per speculative check, plus straggler hot-bucket misses. Helper CPU per batch is only about 20-40 us, so the latency is set by the slowest request, not by throughput.

**E3: thundering herd (H5, not reviewed by a skeptic).** `work.notify_all()` in `Pool::dispatch` and `Pool::poll` wakes all 67 inspectors. That fits dispatch at about 68.5 us per native publication (R² 0.996) and 564M inspector voluntary switches. Suspected share: 5-9% of wall across dispatch, poll and part of untimed.

**E4: resolve-to-commit lag.** By Little's law, 92k-135k buffered events at 135k-189k events/s gives about 0.7 s mean and about 6 s spikes. That is roughly 1.3-2.6k admissions and 0.5-1.4k retirements per window. This lag, not replica publish lag, governs any inspector-side scheme.

**E5: residue floor after decision offload.** About 0.3-0.9 us per request remains: closure edge (3 SipHash, plus a push for about 24% of requests), replay append, event move and counters.

**E6: skew bound on sharding.** Commit speedup is at most about 1/c_max:
- Stage A (one chunk): 1.3-1.5x.
- Stage B (multi-chunk Ready windows): 1.4-2.3x, depending on window formation.

## 3. Ranked plan

Gains are **estimates** as a share of coordinator wall, or of throughput while coordinator-bound. Classes are defined in section 4.

| # | Item | Class | Effort | Est. gain | Base |
|---|---|---|---|---|---|
| 0 | Measurement protocol (section 5) plus session-only telemetry patch | A | 1.5-2 d | – | fable_5_1 |
| 1a | Telemetry diet: throttle `domain_delegated` to 250 ms; cadence-gate `set_parallel_lean` | A | 1 d | 2-3.5% | fable_5_1 |
| 1b | Pool sync: per-slot wakeups instead of `notify_all`; atomic `failure()`; escrow totals from the Emitter | A | 1.5-2 d | 4-8% (E3, unreviewed) | fable_5_1 |
| 2 | Helper-final hit verdicts (O(1) certified commit), plus prefetch and binary-search `find_from` | A | 3.5-4.5 d | 7-20% (skeptics), 12-24% (proposer) | after compact |
| 3a | Barrier cheapening: spin barrier; pin coordinator and helpers to one NUMA node; lag-0 cross-chunk prep-ahead | A | 3-5 d | ≤15-20% combined, unverified | after 2 |
| 3b | Inspector-side resolution with a left-right lookup replica (includes 2's certified commit) | B | 10-12 d | x2.1-3.1 total if stale-miss rate x ≤ 5-10% | after 2 |
| 4 | Miss path: sparse `retire_prepared`; `maintenance_len` without the full scan | A | 2-3 d | ≤15% of commit today; 45-60% of admission after 3b | any |
| 5 | Residue: closure op log on its own thread; records serialized by the sidecar writer; graveyard frees | A | 3-5 d | 5-15% after 3b | sidecar, CSR |
| 6 | Bucket-sharded commit | A only after the must-fixes | 21-28 d | 1.3-1.7x on top | deferred |

### Item details and gates

**1a (proposal 1, telemetry part).**
- Ship it as its own small PR on fable_5_1.
- Keep the first `domain_delegated` event and the first `set_parallel_lean` call ungated.
- Nest `progress_events_throttled` under the duty JSON so `heartbeat_metrics.py` does not read it as a share.
- Recheck `ready_tests.rs:391`, which polls `s.parallel` inside `maybe_save`.
- Gate: Ordered byte-equal; `progress_json` below 0.3% of coordinator wall.

**1b.** Pure scheduling change.
- Gate: Ordered byte-equal and the Ready drain audit.
- Session counters: `notify_all` count and pool-lock wait. These should fall by more than 10x.
- Expect less helper preemption as a side benefit, which also shortens prep.

**2 (proposal 1, hit fast path).** `final_verdict` returns early only when it can prove that `admit_with_lookup` would take its exact, orthant or containment early return. It then applies the same increments in the same order.

Preconditions:
- same queue identity and `max_checks == None`;
- `len - W ≤ 64`;
- no equal domain in `domains[W..len]`;
- no same-bucket full-orthant domain in `domains[W..len]` (this replaces the proposed global `orthant_epoch`, per a skeptic);
- the verbatim overflow guard;
- `!is_released(f)` (compact) or a live bitset (pre-compact).

The helper also returns the exact-hit ID, the semantic flag, and drops the transport Domain.

This is also step 1 of 3b, so it is not throwaway work.

Gates:
- A flat perf profile (M1): the removed symbols cover at least 25% of coordinator samples.
- A differential test with every fallback class above 0.
- Ordered byte-equal including `containment_checks`.
- W50: commit per prepared admission down at least 25% (compact) or 35% (pre-compact), and fast verdicts at least 90%.
- Fail if the commit drop is under 10% or any fallback class exceeds 5%.

Composes with (from skeptics):
- a software prefetch of `slots[f]` and `nodes[target]` for request i+k;
- the `partition_point` block skip in `find_from`/`find_controlled` (same callbacks, cheaper).

Follow-up, class A if its claim holds: a strict small-batch mode. Prepare batches below MIN_ADMISSIONS but use the evidence only while `domains.len() == watermark`. After the first in-batch admission, fall back to the serial admit. A skeptic claims this is byte-identical including `containment_checks`; this is plausible per prepared.rs:1-33 but unverified. Estimated 3-6% of wall (7.87G forward and 2.27G reverse serial callbacks), and only once the wake-up is cheap.

**3. Decision point: re-measure after 2.** After 2, prep becomes about 40-45% of the smaller wall.

*3a (byte-identical).*
- Spin barrier on the reserved helpers, removing the ~101 us fixed wake per batch.
- Pin the coordinator plus its helper group to one CCD/NUMA node (H9).
- Lag-0 prep-ahead: prepare the next chunk's first batch while the coordinator does between-chunk work (dispatch, publication, JSON: about 195-231 us per inspection).

This is a simplification of proposal 2's "exact mode", derived for this note and not reviewed. That work never touches `exact`, `by_owner`, `summaries`, `bits` or `domains`. Helpers can therefore read the live queue through a split borrow (queue admission state shared; ledger, closure and records mutable), with no replica. The evidence is computed at exactly today's state, so `containment_checks` is identical. Proposal 2 estimated this mode at 10-13%.

It still needs the Ready lookahead FIFO fixes listed in section 6.

*3b (proposal 4).* Structure:
- Inspectors resolve each Admit in `Emitter` against a left-right replica fed by a per-miss op log.
- The coordinator runs an O(1) certified commit (item 2's logic).
- Misses go through the unchanged `admit_prepared`/`find_from`/`retire_prepared` path.

Effect: the prep barrier disappears, and hot-bucket skew does not bound the result.

Gates:
- **Before building:** the container-age histogram (M3) must show a stale Miss→hit rate x ≤ 5-10%, since x decides 2.1x versus 3.1x.
- **After building:** prep at most 3% of wall; commit at most 1.2 us per request; fallbacks at most 1%; zero canary mismatches; at least 2x committed events/s on the W50 Ready control.
- Owner sign-off on the `containment_checks` trajectory.
- Off by default for Ordered: the head stream would lose its 32 helpers.
- Memory: +7-10 GB. The compact work must land first given RSS growth (a reviewer read about 120 GB, growing about 35 GB/h, against the 700 GB cap).

*Proposal 2 (lagged replica on helpers): not recommended as a separate build.*
- It carries the same `containment_checks` caveat as 3b.
- Its effort is similar or larger (13-16 d per a skeptic).
- Its ceiling is lower: 1/(1 − prep share) ≈ 1.43x, realistically 1.2-1.3x.
- Its commit may slow under concurrent helper DRAM traffic.

Build it only if M3 shows x too high for 3b.

**After any coordinator win**, watch whether the next limit becomes the Ready credit H=256 (reported saturated), inspector supply, or dispatch/poll.

## 4. Compatibility classes

**A. Result-identical, byte-equal including `containment_checks`.** The running campaign can resume with WALK_SEMANTICS_VERSION 1 and no sign-off.
- Items 1a, 1b, 2, 3a, 4 and 5.
- Prefetch; spin barrier; pinning; graveyard frees.
- A non-SipHash hasher for `exact`, `by_owner`, `positions`, `open_targets` and escrow. The path map says none of these is iterated into output; verify that for `open_targets` first.
- Strict small-batch mode, if its claim holds.
- Telemetry changes touch only `parallel` and progress, which the comparisons already exclude. The only difference is that `parallel` can be up to 250 ms staler at an interval save.

**B. Decisions identical; `containment_checks` trajectory changes.**
- Admission IDs, records, edges, ledger, replay tokens and every other counter are identical.
- No decision reads `containment_checks` in the unlimited lane, and resume already varies it (grain_tests.rs:450-462).
- Under the section 5 contract this needs no WALK_SEMANTICS_VERSION bump. It does break the strict "persisted counters identical" rule, so it needs:
  - owner sign-off;
  - a per-session pipeline-mode marker in manifest or checkpoint metadata;
  - Ordered controls that compare with the key deleted (including `walk_control_matrix.py`).
- Members: item 3b; proposal 2 lagged mode (and the design doc's section 4 lag-1 design); any change to BATCH_RECORDS, MIN_ADMISSIONS or MIN_CANDIDATES, or tail merging.

**C. Semantics change: fresh campaign plus a version bump. Not recommended.**
- A first-found containment winner instead of the minimum ID.
- IDs assigned in shard-completion order.
- Any new replay-token encoding.

**Item 6 (bucket sharding)** is intended as class A. As written it fails, both skeptics agree (section 6).

## 5. Measurement protocol (run first; controls only, never the live campaign)

**Setup.** Five-loop finite control (1,324 tuples) at W50, pinned to **CPUs 200-249**, in Ordered and Ready. Rebuild once with `-C force-frame-pointers=yes` and `debug=line-tables-only`; this is transport-only. The coordinator is the main thread (TID = PID).

**M1: coordinator perf.** Three 60-s windows (early, mid, late), each with:
- `perf record -F 999 -e cpu-clock -t <TID> --call-graph fp`
- `perf stat -t <TID> -e cycles,instructions,cache-misses,dTLB-load-misses,ls_dmnd_fills_from_sys.*` (IPC and DRAM fills)
- `perf trace -s -t <TID>` (futex counts and time)
- `perf c2c` on the pool mutex
- `/proc/<pid>/task/*/sched` snapshots (switches, migrations, `sum_exec_runtime`) for coordinator, helpers and inspectors.

Attribute samples to:
- the SipHash/hashbrown probe;
- `is_live`/`partition_point`;
- `Domain`/`DomainPowerSummary::contains`;
- memcpy (clone);
- `_int_free`;
- `Tracker::edge`;
- blake3/`Replay::append`;
- `find_from`/`retire`;
- `serde_json`;
- futex.

Decision rules:
- The item 2 gate is at least 25% of samples in the removable set.
- IPC below about 0.5 with high DRAM fills confirms E1.
- Repeat one window with the coordinator pinned to test H9.

**M2: batch-size sweep.** This is diagnostic only: it changes `containment_checks`, so it runs through an env-gated knob and never ships.
- BATCH_RECORDS ∈ {64, 128, 256, 512, 1024} × MIN_ADMISSIONS ∈ {16, 1}.
- Per setting, measure:
  - the prep intercept (fixed us per batch);
  - per-batch max versus mean helper task time (straggler share);
  - commit us per request, which should be flat;
  - serial-path batches, records and admissions.
- This decides spin barrier versus prep-ahead versus 3b, and sizes the strict small-batch mode. Prior: the cap binds on at most 19% of batches, so BATCH_RECORDS itself is worth at most about 1.8%.

**M3: bucket-skew and lag histograms.** Session-only counters recorded in `prepare_with_replay` and at commit, never persisted.
- Per batch: admission requests per `(phase, owner)`, giving max-share by count, by sampled commit time (1 in 64) and by helper prep time. Also distinct buckets per batch.
- Simulated window share for K = 2/4/8, using the actual window rule (with Finished items interleaved), not the published-domain proxy.
- Container age at commit: the hit target's distance from `domains.len()` (256/1k/4k/16k). This sizes 3b's x.
- `would_be_stale_lag1`; prepared batches per chunk; lookahead availability at chunk end; resolve-to-commit lag.
- Commit time and count by outcome (exact, orthant, prepared-live, `find_from` hit, miss, serial).
- Blocks visited versus skipped in `find_from` and `retire`.

**M4: A/B gate for every item.**
- Ordered `result.json` byte-equal. Class B items delete `containment_checks` and say so.
- Ready passes `audit.py`.
- Watch `coordinator_duty` shares, `computing_workers`, `finished_awaiting_commit` and escrow occupancy.
- Resume both ways: an old checkpoint on the new binary and a new checkpoint on the old binary.

## 6. Unresolved equivalence gaps (from the skeptics)

**Item 2 (proposal 1; both skeptics: equivalence holds):**
- Evaluate every precondition before any mutation. Otherwise a fallback double-counts `containment_summary_builds`.
- After the helper-side Domain drop, rebuild the Domain from `key.0.expand()` on every slow path, including the `!same_queue` key discard and the `cfg(test)` trace mode. Never reuse a digest across queues.
- Pre-compact: set the live bit in the `bucket.indexed.insert` branch (queue.rs:569, not :572). The restore rebuild needs an index ID iterator, which exists only on compact.
- The differential test must cover:
  - an in-batch full orthant in another bucket;
  - an in-batch miss equal to a later request;
  - a retired snapshot winner whose slab slot is reused within the batch;
  - a debug `is_live` agreement check.
- The "composes with section 4 pipelining" claim is false: design A's replica lacks the exact index.
- The Query-clone saving is overstated unless the verdict header is split from the moved event.

**Item 3a / proposal 2 (1 of 2 skeptics: equivalence fails as written):**
- The lookahead FIFO loses polled Finished items on pause, cancel or failure. They must return to leftovers and the uncommitted receipts, and a non-empty FIFO counts as pending.
- Lookahead tokens must use the run's checkpointing flag and the target stream's filter, not the mounted stream's `state.replay`. Otherwise `record_accepted` skips silently and resume is refused.
- The replica fingerprint does not cover summaries, bit words or seal boundaries.
- The Ordered exact-mode control never exercises replica evidence; a forced replica-at-lag-0 mode is needed.
- SegVec sharing conflicts with compact slot reuse (a data race).

**Item 3b (proposal 4; both skeptics: equivalence holds):**
- The replica retire lookup must sort the retired set, because `on_retire` order is group order.
- The canary must re-derive the full serial decision (exact, orthant, find) and also sample Orthant resolutions: exact-map completeness decides the verdict and is never re-checked.
- The coordinator must compute a missing token when `state.replay` is `Some`.
- Check identity and bucket slot for every certified variant.
- Resolve after the slot wait in `Pool::publish`, and measure staleness at commit.
- Route `ResolvedAdmit` to `PreparedEvent::Certified` on both `prepare_with_replay` branches, and send sparse unresolved Admits to helpers, not to serial finds.
- Box the payload so `Event` does not grow.
- Keep escrow and `buffered_bytes` balanced.
- Pre-reserve the replica maps (rehash stalls).
- Verify replica against queue (watermark, live count, digest) before each checkpoint.
- Keep resolution time out of the record `seconds` field.

**Item 6 / proposal 3 (both skeptics: equivalence fails):**
- Pause saves: `resumable_cancellation` leads to the forced save in mod.rs:620. A mid-window pause therefore persists provisional IDs or phantom retirements, so windows must be atomic under cancellation.
- `Block` serializes all 32 slots, so stale provisional IDs would change CP5 bytes.
- The proposal filters before splitting, which contradicts split-then-filter.
- Argument (9) is wrong: delegates publish even when the pool is full. The proof must argue commutation instead, and edge and record order can then differ.
- Poison the queue on any non-cancellation error mid-window.
- Stopping at the first Finished degenerates windows to about 2 chunks.
- Rayon LIFO scheduling starves the hot-bucket critical chain.

**Design docs:** correct the section 4 counter claim in `fable51_design_scheduler_admission_2026-09-26.md`. Record any class B adoption in the campaign notes.

## 7. Deferred or rejected, with numbers (estimates)

- **Bucket-sharded commit (item 6).** Stage A is about 1.2-1.37x on the coordinator and does not pay for 3-4 weeks of work. Stage B is 1.3-1.8x only with fixed window formation and critical-path priority. Revisit only after items 1-5, and only if M3 shows a cost-weighted K=8 window share of at most 0.35 at the median and at most 0.5 at p90.
- **BATCH_RECORDS change:** at most about 1.8%. **Off-thread record JSON:** 1-1.7%; fold it into the sidecar branch. **Non-Admit replay tokens on helpers:** about 0.13%. **Standalone `Replay::append` offload:** 40-70 s, eaten by handoff cost.
- **Checkpoint writer from an immutable snapshot:** out of scope (4 h interval, not a decision input).
- **Worker split.** `workers` and `inspection_workers` are frozen in `binding()`, so the 32 helpers stay stranded in the live campaign after 3b. For new campaigns, re-split (for example 91 inspectors / 8 helpers / 1 coordinator).