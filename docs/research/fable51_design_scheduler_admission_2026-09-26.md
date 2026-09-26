# Design: scheduler / admission / closure-refresh / inner-parallelism package (fable_5_1, 2026-09-26)

File-level implementation plan produced by a planning agent for section 3.B of `FABLE_5_1_five_loop_vacuum_plan.md`. Wave 1 (items 1(d)-(f), 2, 4, 5, 7, 8) is implemented on branch `fable_5_1-sched`; items 1(a) Ready resume gate, 3 (prepared retirement is done), 4 pipelining and 6 inner parallelism remain. Line numbers refer to the `fable_5_1` base at the time of planning.

# Plan: Scheduler / Admission / Closure-refresh / Inner-parallelism package

All paths below are absolute; `W/` = `/common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/`, `CORE/` = `/common/dev/rustred/crates/rustred-core/src/`.

## 0. Findings that determine the sequencing

Verified in code (not just the brief):

1. **The coordinator is the throughput ceiling, and it also gates dispatch.** In `W/execution.rs:975-1302` one thread does everything serially: dispatch (`1068-1161`), poll (`1180-1186`), chunk commit (`1203-1241`, which for a 16,384-record chunk is 64 alternating prep/commit batches inside `admission::Engine::commit_chunk`), Finished publication, `pool.snapshot()` JSON after every poll, `maybe_save`. A finished inspector slot stays occupied until that ticket is polled (`W/parallel.rs:262-266` sets `slot.id = None` only in `poll`; `dispatch` at `:182` needs `id.is_none()`), so while the coordinator commits one heavy chunk, every inspector that finishes a small job idles. That is the mechanism behind "1.8 of 10 cores" and "10 of 50 cores". Nothing in items 6 or 7 pays off until coordinator work per request drops; items 2 and 3 are therefore first.
2. **The reverse retirement runs inside the serial commit** (`W/queue.rs:456-477` → `AggregateIndex::retire` `W/queue/index.rs:404-461`) and calls `DomainPowerSummary::contains` for every ID in every eligible block. The `containment_maintenance_checks` charge (`maintenance_len`, `index.rs:385-399`) is a conservative per-group bound, not the callback count; the plan adds real callback counters.
3. **Prepared lookups are already staleness-tolerant** (`W/queue/prepared.rs:139-147,155-191`): `watermark` + `is_live` for a hit, `find_from(watermark)` for a miss. This is exactly the invariant item 3 and item 4 need; the obstacle for item 4 is borrow/memory safety, not semantics.
4. **Ready round-robin** (`W/execution/publication.rs:20-38`) takes one non-waiting item per pass; each slot holds at most one published chunk (`parallel.rs:216-242`). H (`Ledger::reserve_available`, `W/delegation/ledger.rs:412-430`) bounds Reserved+Started IDs, not buffers; buffers are bounded by inspector slots (2 × 8 MiB logical per slot).
5. Existing deterministic-schedule test pattern for Ready: `W/execution/ready_tests.rs:86-130` (`run_pause` with `maybe_save` trigger and a synthetic `Count{CHUNK_EVENTS}` flush marker); real-native serial Ready resume: `W/execution/ready_native_tests.rs`.

## Interfaces needed from the other two agents

| From | Needed by this package |
|---|---|
| Checkpoint/memory agent | (a) `WALK_SEMANTICS_VERSION` policy: this package makes **no** admission-semantics change if items 2/3 are implemented as specified (proof in §3); items 1(f) and 6 change scheduling only. (b) Codec: `Queue` gains a non-persisted `bits: Vec<u64>` rebuilt in `Queue::deserialize` (`W/queue/checkpoint.rs:129-137`); optional per-block words are `#[serde(skip)]` and rebuilt in `restore_positions`. No format change. (c) A `force`-save + cancel seam reachable from the `maybe_save` closure built at `W/mod.rs:576-588` (already exists via `store.save(..., force, ...)`); manifest metadata should carry an optional `diagnostic_pause` string. (d) Memory model must include: prepared retire sets (bounded, §3), 2 × 8 MiB × inspectors chunk buffers at W>50, inner-pool event buffers (§6, bounded window), closure refresh scratch (`V × 9 B`). (e) If H is ever changed on resume, `validate_checkpoint` (`ledger.rs:441-541`) needs a "pending ≤ new H, then `reserve_available`" normalization; this plan keeps H frozen per campaign so nothing is required now. |
| Physics-scoped inputs agent | Nothing structural. Item 8 telemetry fields are additive JSON; the four-loop/five-loop controls they define are the measurement gates here. |

---

## 1. Ready hardening

### 1(a) Close the multi-prefix resume-to-exhaustion gate

**Goal.** A real native multi-inspector Ready run whose checkpoint holds ≥2 positive unfinished accepted prefixes plus a finished hole, resumed in a fresh process to exhaustion, matching a baseline.

**Files/functions.**
- `W/execution/streams.rs`: add `pub(in super::super) fn ready_multi_prefix_hole(&self) -> bool` = `ready_accepted_source_prefixes >= 2 && published_count() > queue.next` (factor the counting out of `add_ready_progress` at `:26-47`).
- `W/mod.rs:576-588`: in the `run_checkpointed` closure, if `diagnostic_pause == Some(ReadyMultiPrefix)` and `state.ready_multi_prefix_hole()`, call `store.save(state, .., force = true, ..)` then `cancellation.store(true)`; record `"diagnostic_pause":"ready-multi-prefix"` in the save metadata. Source of the flag: environment variable `RUSTRED_WALK_DIAGNOSTIC_PAUSE=ready-multi-prefix` read once in `owner_domain_walk_with_progress` (pattern: `RUSTRED_ADMISSION_GRAIN_*` in `W/execution/admission/grain_replay.rs:16`). An env var rather than a CLI flag keeps the frozen `steering.json` command untouched (`examples/python/production_saved_owner_campaign.py:155-161` validates the frozen argument list).
- New in-process test module `W/execution/ready_native_multi_tests.rs` (registered in `execution.rs` next to `ready_native_tests`).

**Test: `ready_multi_inspector_multi_prefix_disk_resume_matches_gated_baseline`** (license-gated: `LicenseManager::is_licensed()` early return and `ParallelExecution::preflight_requested_core_budget(4)` skip). W=4 Ready (3 inspectors, 0 helpers, 1 coordinator per `worker_budget.rs:67-79`), H=3, seed 4–6 domains from `native_fixture()` (`W/execution/initial_orthants_tests.rs:117`). Inject a visitor into `run_pool` that wraps `inspection::inspect(reducer, …, &InitialOrthants::empty(), &InitialOverlapIndex::empty(), …)` with a schedule script:
- tickets 0 and 2: forward native events, then emit `Count{count: CHUNK_EVENTS}` to force a flush (as `ready_tests::prefix`), then park on `stop` (ticket 2 flushes only after a shared flag says ticket 0's chunk was committed, observed in `maybe_save` via `ready_accepted_source_prefixes >= 1`);
- ticket 1: completes only after the flag says both prefixes are accepted (the finished hole);
- `maybe_save`: when `ready_multi_prefix_hole()` holds, `codec::write` to bytes, set cancellation.
Resume via `codec::read` into a fresh `State` with the same wrapper (no parking) to exhaustion; baseline = same scripted schedule without pausing. Assert exactly what `ready_w1_native_disk_resume_…` asserts (`ready_native_tests.rs:102-129`): domains, records without `seconds`, `(events, successors, completed, deduplicated)`, `finalize_delegation()`; plus `has_two_prefixes` on the restored state (reuse `ready_tests.rs:72-85`) and `resumed.streams.parked.len() >= 1` after restore.

**Fresh-process gate (measurement).** New harness `examples/python/ready_resume_control.py` (or the TMP harness that produced `TMP/ready-five-loop-prefix-v3`): baseline run to exhaustion; run with `RUSTRED_WALK_DIAGNOSTIC_PAUSE` → paused exit 4 whose manifest metadata shows `diagnostic_pause` and whose `progress` shows `ready_accepted_source_prefixes >= 2` and `ready_published_holes > 0`; resume with the production launcher `--resume` in a new process to exhaustion. Pass criteria: the streaming audit (zero pending, zero frontiers, all inputs published, all ledger obligations discharged) and native inspections/events within ±2% of baseline. Exact equality is **not** the criterion for real Ready runs (readiness-dependent admission, documented in `docs/research/five_loop_finite_publication_scaling_2026-09-25.md:86-93`); the in-process test supplies exact equality under a fixed schedule. Controls: four-loop X (restricted, the earlier attempts drained before the trigger) and the restricted five-loop pilot.

**Effort.** 2 days (test 1 day, env seam + harness 1 day).

### 1(b) Stale documentation

Update (all say "experimental / Ordered remains the default"): `/common/dev/rustred/docs/shared_owner_campaign_driver.md:167,175-183`, `/common/dev/rustred/docs/shared_owner_domain_matching.md:364-377`, `examples/python/production_saved_owner_campaign.py:239-240` help text, `examples/python/README.md` if it mentions the policy, `docs/CLI.md` `--publication-policy` line. Add a status header to `docs/research/five_loop_ready_publication_2026-09-24.md` pointing to the new gate record; do not rewrite history.

### 1(c) Launcher default

- `examples/python/production_saved_owner_campaign.py:191`: `args.publication_policy or "ready"`; `:239` help "initial default: ready; ordered remains available"; keep resume-freeze logic unchanged.
- `examples/python/test_campaign_monitor.py:412-437`: `expected = requested or "ready"`.
- Rust `OwnerDomainWalkRequest::new` (`W/mod.rs:83`) and the CLI default stay `Ordered` (Ordered remains available and its deterministic tests remain valid). `shared_owner_campaign.py:501,693` record what was actually passed, so no change.
- New Python test: `test_new_campaign_default_is_ready_and_requires_lookahead_256`.

### 1(d) H for W50–W128

Analysis from `ledger.rs:412-430,356-376`: a credit is held from reservation until `publish_native`, so `H ≥ inspectors + finished-awaiting-publication + reservation slack`. H does not bound memory (buffers are per slot). The cost of a large H is fewer transfers (a Reserved ID cannot be delegated, `transfer_retired` `:291-296`) → measured +1.5–1.7% native work at H=256/I=25. Recommendation: **H = max(256, 4·inspectors)**, which is 256 for all W ≤ 128 (I ≤ 64 with the §7 split). Keep 256 frozen in the production preset; add a launcher check in `production_saved_owner_campaign.py::frozen_policy` that H ≥ 2 × inspectors (warn, not error) and a unit test `worker_budget/tests.rs::ready_lookahead_covers_inspectors_up_to_128_workers`. No ledger/checkpoint change.

### 1(e) Draining policy and chunk depth (confirmation)

Confirmed: with round-robin polling, per-slot depth > 1 cannot raise throughput; total throughput is coordinator-bound, and depth only smooths bursts (inspectors keep computing while their chunk waits). Preferring heavy streams likewise redistributes idle time, it does not create coordinator capacity. Decision: keep depth 1 and round-robin; add the §8 counters (`backpressure_seconds` per slot, coordinator `wait_seconds`). If a pilot shows **both** coordinator wait share > 20% **and** inspector backpressure share > 20% (burstiness), revisit depth 2 (memory +8 MiB × inspectors).

### 1(f) Ready slot recycling (new, small, high leverage)

Under Ordered, `pool.reclaim_finished(publisher)` (`parallel.rs:160-176`) moves later finished jobs into escrow so their slots free up; under Ready it is skipped (`execution.rs:1065-1067`). For Ready, every finished ticket is eligible: call `pool.reclaim_finished(0)`-equivalent (new `reclaim_all_finished`) at the top of the loop, and additionally run a cheap "service" step between the 256-batches inside `commit_chunk` (via the existing `heartbeat` callback slot, extended to `FnMut(&mut State)`): reclaim finished slots and dispatch reserved work (dispatch touches `ledger.native_started` and `ready_streams.dispatched` only, never the mounted replay context, so it is safe mid-chunk; Finished publication stays in the main loop because `commit_physical` uses the active context). Escrow limits for Ready: entries ≤ 2 × inspectors, bytes ≤ 2 × CHUNK_BYTES × inspectors (`parallel/escrow.rs:9-17` `Limits`). Not a semantics-version change (Ready results are already schedule-dependent; checkpoint validity unaffected). Test: `ready_finished_slots_are_recycled_before_the_current_chunk_commit_ends` in `ready_tests.rs` (synthetic inspectors: one long chunk-heavy ticket, many tiny tickets; assert `completed_slots_reclaimed > 0` and that tiny tickets dispatch while the heavy chunk is mid-commit). Gate: five-loop W50 control `active_workers` mean and `native_busy_cores` rise; no change in audited results. Effort: 1 day.

---

## 2. Bit-signature pre-filter tier

**Goal.** Cut full `DomainPowerSummary::contains` evaluations in forward lookups and reverse retirement with an 8-byte necessary test; results and persisted counters byte-identical.

**Word layout** (`u64`, new module `W/queue/bits.rs`), chosen so a single subset test `candidate_word & !container_word == 0` encodes every implication:

| bits | meaning | implication used (C ⊇ Q) |
|---|---|---|
| 0–15 | `upper[i].is_none()` per axis | Q unbounded ⇒ C unbounded |
| 16–31 | `lower[i] == 0` per axis | C.lower>0 ⇒ Q.lower>0, contrapositive Q.lower==0 ⇒ C.lower==0 |
| 32–35 | A upper None, R upper None, D lower None, D upper None | Q infinite ⇒ C infinite |
| 36–37 | A lower == 0, R lower == 0 | Q zero ⇒ C zero (C.lower ≤ Q.lower) |
| 38 | D lower is None or ≤ 0 | Q.diff_lower ≤ 0 ⇒ C.diff_lower ≤ 0 or None |

Empty summary → word 0 (passes as candidate, correct since `contains(_, empty)` is true; as container it only passes all-zero candidates and then `contains` returns false — filter stays merely necessary). Arity ≤ 16 (`W/mod.rs` `dispatch!(1..=16)`), so the masks fit.

**Option decision: (a) `Vec<u64>` parallel to `summaries`, rebuilt on restore.** Reasons: zero checkpoint-format impact; per-ID rejection is exact; the reverse path visits IDs block by block anyway. Option (b) (per-block `any_word` OR-aggregate for forward, `all_word` AND-aggregate for reverse, maintained in `Block::insert`, left conservatively stale by `retain` exactly like `AxisEnvelope`, `#[serde(skip)]` and rebuilt in `restore_positions` given a `word_of(id)` closure) is a measured follow-up: implement only if post-(a) counters show `blocks_visited × 32` still ≫ full compares.

**Files/functions.**
- `W/queue.rs`: `bits: Vec<u64>` (unlimited lane only) pushed at `:501-503`; forward closure at `:315-321` and `retire` closure at `:463-474` become `bits::may_contain(bits[id], q) && summaries[id].contains(..)` (reverse: roles swapped). New session counters (not persisted; `#[cfg(not(test))]` irrelevant): `forward_callbacks`, `forward_bit_rejections`, `reverse_callbacks`, `reverse_bit_rejections` exposed via `progress()` (`execution.rs:173-208`) and the final document (`W/mod.rs:713-720`). Doc comment on `containment_checks` (`:110-112`) amended: "candidate comparisons attempted; a bit-tier rejection is still one attempted comparison".
- `W/queue/prepared.rs:111-114` and `:183-186`: same closure change.
- `W/queue/checkpoint.rs:129-137`: push `bits::word(&summary)` in the same loop.
- Test-only toggle `Queue::disable_bit_prefilter()` mirroring `disable_index_work_counters` (`index.rs:186-191`).

**Invariants.** Min-ID container unchanged (the traversal, `best` logic and callback order in `find_controlled` are untouched); `containment_checks` charged before the bit test → identical to today; `containment_maintenance_checks` unchanged; replay tokens untouched (admission-side only). **Not a semantics change.**

**Tests** (`W/queue/tests/bits.rs`, no license needed):
- `bit_word_is_a_necessary_condition_for_native_containment`: exhaustive small grid over N=1..3 (upper ∈ {None, 0, 1, 3}, lower ∈ {0, 1, 3}, rank ∈ {None, 0, 2}, power bounds with/without D lower) asserting `contains(C,Q) ⇒ may_contain(W(C), W(Q))`.
- `bit_prefilter_keeps_admission_results_and_counters_identical`: run the `admission/tests.rs::stream()` and `queue/tests/prepared.rs` proposal streams with the prefilter on/off; assert `same_state` plus `containment_checks == ` and `containment_maintenance_checks ==`.
- `restored_queue_rebuilds_bit_words_from_summaries`: via `checkpoint::round_trip_state`.
- Extend the ignored input-driven benchmark `W/queue/tests/replay.rs` with `RUSTRED_DOMAIN_REPLAY_BITS=off` to report callbacks with/without.

**Gate.** Four-loop FG/BMW/H/X Ordered at W6: `result.json` counters (`containment_checks`, `containment_maintenance_checks`, `containment_retired_candidates`, domains, records) byte-equal to the pre-change binary; five-loop 1,324-tuple W50 Ordered: same equality; new counters show `forward_bit_rejections/forward_callbacks ≥ 0.6` and reverse likewise; `ordered_commit_wall_seconds` and `preparation_wall_seconds` per request down ≥ 30%.

**Effort.** 1–1.5 days for (a); +1–2 days for (b).

---

## 3. Reverse retirement off the serial path

**Goal.** Compute each admission's retire set read-only on helpers during preparation; apply serially at commit; identical results, counters and index layout.

**Design.**
- `W/queue/index.rs`: 
  - `collect_contained(&self, signature, coordinates, checkpoint, contains) -> Result<Vec<usize>, &'static str>`: the read-only twin of `retire` (same group test `signature.may_contain(group.signature)`, same `block.may_be_contained`), collecting IDs for which `contains(id)` holds, with a cancellation checkpoint like `find_controlled`. Returns sorted ascending.
  - `retire_prepared(&mut self, insertion, coordinates, prepared: &[usize], first_new: usize, contains_new, on_retire) -> usize`: **the same loop as `retire`** (positions, eligibility, `pin_tail`, `blocks.retain`, `swap_remove` + `positions` fixup) with the per-ID predicate replaced by `prepared.binary_search(&id).is_ok() || (id >= first_new && contains_new(id))`, calling `on_retire(id)` for each removal. Keeping the loop identical guarantees the same block/group layout and traversal order afterwards (which is what future forward `containment_checks` counts depend on) and identical test-only work counters.
- `W/queue/prepared.rs`: `PreparedLookup` gains `retire: Option<Vec<usize>>` and `reverse_checks: usize`; computed in `prepare_lookup` only on a forward miss and when not cancelled; capped (e.g. 65,536 IDs → `None`, counted as a fallback) to bound memory. `revalidate` returns the set on a miss (a snapshot hit that is still live never retires; a retired winner returns `None` → full serial path, as today).
- `W/queue.rs:456-477`: if the revalidated lookup carries `Some(set)`, use `retire_prepared` with `first_new = lookup.watermark`, else `retire`. `maintenance_len` charge, `extra_retired` (raw `domain.contains`) and `ledger.transfer_retired(old, id)` are performed exactly where they are today.
- `W/execution/admission.rs` `Metrics`: `speculative_reverse_checks`, `prepared_retirements_applied`, `prepared_retire_fallbacks`.

**Proof of identical results (to be written into the module doc).** Let S be the snapshot, L(S) its live index set, W = |S.domains|. At commit the live set is L = (L(S) − R_batch) ∪ N_batch where R_batch are IDs retired by earlier commits and N_batch ⊆ [W, id) the IDs admitted since. Serial `retire` computes {x ∈ L : N ⊇ x}. Since containment is pure geometry on immutable summaries, {x ∈ L(S) − R_batch : N ⊇ x} = prepared − R_batch, and members of R_batch are absent from every block so `retain` never sees them (no callback, no transfer) — identical to serial; the remainder {x ∈ N_batch : N ⊇ x} is exactly what `contains_new` evaluates for `id ≥ first_new`. `transfer_retired` is evaluated at apply time after `admit_reserved`/`reserve_available`, so representative-newest and `old`-Unreserved checks see the same state serial would; transfers are per-`old` independent so callback order is irrelevant. Cleanup order replicated ⇒ identical layout. ⇒ **not a semantics change; counters identical** (`containment_checks` = forward callbacks + unchanged maintenance charge).

**Ready interaction.** `reserve_available` may reserve an `old` between snapshot and apply → transfer refused with `ReservedOrStarted` exactly as serial would at that instant. No new refusal class.

**Tests** (`W/queue/tests/prepared.rs` additions; unit, no license):
- `prepared_retirement_matches_serial_retire_set_layout_and_transfers`: overlapping streams (containers admitted after contained; in-batch containment chains), Ordered and Ready ledgers (`Ledger::new_ready` with H small so some olds are Reserved); compare `same_state`, `containment_checks`, `containment_maintenance_checks`, `transfer_count`, `resolve().summary`, and a test-only `AggregateIndex::layout()` (groups → blocks → ids).
- `prepared_retirement_scans_in_batch_admissions_above_watermark`.
- `prepared_retirement_falls_back_when_snapshot_winner_retired_or_near_counter_exhaustion` (extend the two existing fallback tests).
- `cancelled_reverse_preparation_publishes_nothing` (checkpoint fault injection at each block).
- `W/execution/admission/tests.rs`: extend `parallel_preparation_preserves_every_event_frontier_and_domain_cap_prefix` with `prepared_retirements_applied > 0` and `assert_same`; `grain_tests::canonical` gains `containment_checks`.

**Gate.** Same equality checks as §2 on FG/BMW/H/X and the five-loop W50 control; `ordered_commit_wall_seconds` per request ≤ half of post-§2 value; `preparation_wall_seconds` growth ≤ +30%; `(prep+commit)/coordinator_elapsed` down ≥ 40% overall (from ~31–48%).

**Effort.** 3–4 days.

---

## 4. Preparation/commit pipelining (double buffer)

**Invalidation analysis (answer to the brief's question).** Helpers never need to re-check against IDs admitted by an in-flight commit. A preparation against any consistent snapshot S_j is valid at any later commit because (i) `revalidate` uses `is_live` for a hit and `find_from(watermark)` for a miss, (ii) §3 applies `contains_new` for `id ≥ watermark`, (iii) retirements only remove. The only requirement is that `watermark` equals the snapshot's `domains.len()` and the snapshot is not torn. Staleness costs O(#same-bucket IDs admitted since the watermark) per admission, ≤ 256 per batch of lag.

**What actually blocks it.** `prepare_with_replay` borrows `&state.queue` while `commit_prepared` needs `&mut State` (`admission.rs:305-313`); `Vec` growth and `HashMap` rehash make concurrent reads unsound. Two safe designs:

- **A. Lagging read replica, double-buffered (recommended if built).** `LookupReplica<N>` = `{ domains_len, bits: Vec<u64>, index: per-bucket groups/blocks (IDs + envelopes + words), orthant per bucket, summaries: Vec<Arc<[DomainPowerSummary<N>; 4096]>> sealed segments + a copied tail }`. The coordinator logs its index mutations per batch (`IndexOp::{Insert{bucket,sig,id}, Retire{bucket,sig,id}}`) and applies each batch's delta to the replica not currently in use by helpers; helpers prepare batch k+1 on replica R (state S_{k-1}) while the coordinator commits batch k; then swap. Each delta is applied twice in total (once per replica), O(delta). Memory: ~31 B/ID per replica for index+bits (≈1.2 GB at 20M IDs for both) + one 4096-entry tail copy per batch (≈2 MB); summaries shared. Exact-key dedup stays serial (a prepared lookup for an exact-hit domain is simply wasted). Pipelining is limited to batches **within one chunk** (same ticket, same replay context); cross-chunk pipelining would require mounting the target ticket's `Replay` before `filter_replay`, which is not worth the complexity.
- **B. Persistent CoW queue snapshot** (`Arc` chunks + `Arc::make_mut`): cleaner API, touches all index code, higher risk. Not recommended before launch.

**Recommendation.** Do **not** build before measuring §2/§3. Gate: on the five-loop W50 control after §2+§3, if `(preparation_wall + ordered_commit_wall)/coordinator_elapsed > 15%` or `preparation_wall` alone > 8%, implement A. Expected gain ≤ the remaining prep-wall share (prep and commit alternate; overlap hides the smaller of the two).

**Tests if built.** `pipelined_preparation_matches_serial_commit_at_every_batch_boundary` (fault-inject a cancel/abort at each batch; compare state with the non-pipelined engine), `replica_delta_application_reproduces_index_layout`, `checkpoint_between_pipelined_batches_restores_without_replicas` (replicas rebuilt on resume), Ready and Ordered variants, helper panic ordering (mirror `helper_panic_propagates_only_after_backpressured_native_worker_is_joined`).

**Effort.** 5–8 days (A).

---

## 5. Closure-refresh duty

**Goal.** Bound refresh duty to ~1% without changing monitoring semantics.

- `W/descendant_closure.rs:181`: `interval = max(5 s, REFRESH_DUTY_DIVISOR × last)` with `REFRESH_DUTY_DIVISOR = 100.0`; `json()` (`:241-262`) adds `refresh_duty_bound: 0.01` and `next_refresh_seconds`. Doc comment `:175-176` updated ("≤ ~1% duty"). `docs/research/dependency_closure_monitoring_2026-09-25.md` and `docs/shared_owner_campaign_driver.md:25-45` note the new cadence (snapshot ages up to 100× the last refresh time; the monitor already prefixes conservative counts with ≥/≤).
- Test `refresh_interval_bounds_duty_to_one_percent_and_force_bypasses_it` in `descendant_closure.rs` tests (set `last_refresh_seconds`, assert skipped/allowed; `force=true` unaffected).
- **Off-coordinator refresh (deferred design).** Edge store as sealed `Arc<[Edge; 65_536]>` segments + mutable tail; per refresh the coordinator copies `sealed` flags and `incoming` heads (V × 9 B, ≈180 MB at 20M nodes, ~50 ms) and hands them plus the sealed segments to one admission helper; helper clamps head pointers to the snapshot edge count and computes the `closed` bitmap; coordinator applies it. Correct because `closed ⇒ sealed` and sealed nodes never gain outgoing edges, so a snapshot's closed set remains valid later (matches the `restore` invariants at `:340-353`). Gate: `refresh_seconds/coordinator_elapsed > 1%` or snapshot age > 10 min at live scale. Effort 3 days.

**Effort now.** 0.5 day.

---

## 6. Inner parallelism for heavy Apply heads

**Design (core).** Task grain = (rule, shift group, normalized sign cell) inside `apply_piece` (`CORE/solver/candidate_reduction/owners/domains/applied/engine.rs:325-426`). The inspector thread keeps the serial prefix per shift group (`charge(shift_groups)`, `sign_cells`, `normalize`); each normalized sign cell becomes a task executing `Boundaries::new` → `next` → refinement → `apply_group` (`:476-763`, the algebra-heavy part) with:
- a **private `Budget`** (limits = remaining allowance at task creation, so a task cannot outrun what sequential could still afford by more than one task's work), and
- a **recording visitor** producing a per-task log `Vec<Recorded<N>>` = `Charge(kind, amount)` | `SuccessorCharge{source,target,conditional}` | `Refusal{original, owned event}` | `Event(OwnedAppliedEvent<N>)` | `Failure(OwnerAppliedFailure)`. Owned events clone `IndexedCoefficient`, coordinates and shifts (the public `OwnerAppliedEvent` borrows them, `model.rs:126-193`).

**Replay on the inspector thread in canonical order** (group, sign cell, boundary, refinement child, term ordinal): `Charge` → the same `charge()` against the real budget (identical `ResourceLimit{requested,limit}` at the identical point); `Refusal` → `budget.optional_refusal(original)` decides "first in query" at replay (sequential semantics, since the query-global first-refusal state lives only in the real budget); `Event` → `budget.emit(visit, borrowed view)`; `Failure` → returned at its position. Replay stops at the first failure ⇒ lowest task ordinal wins, as in `ParallelExecution::map_ordered` (`CORE/campaign/execution.rs:191-234`). Consumer `Break` discards the buffered tail. Cancellation: tasks observe the shared `AtomicBool`; the replay's `emit` checks it too; only the stop *position* of a cancelled inspection differs, and any emitted prefix is a prefix of the canonical order — sufficient for `replay.rs` tokens (hash of the accepted callback sequence).

**Streaming/bounded memory.** Tasks run on a private rayon pool with an ordered reassembly buffer (window = 2 × pool threads); the inspector replays task t once tasks < t are replayed. Task logs above a byte cap (64 MiB) mark themselves `Sequential` and are executed inline at replay. Small pieces (few sign cells × few terms) skip the pool entirely (threshold parameter).

**Pool/licensing/budget.** New `CORE/solver/candidate_reduction/owners/domains/applied/executor.rs::InnerExecutor` built like `SectorExecutor::new` (`CORE/solver/execution.rs:161-196`: `LicenseManager::max_threads` broadcast on every pool thread). API: `visit_power_bounded_owner_applied_successors_with_executor(.., Option<&InnerExecutor>)`; the existing entry point calls it with `None`. App side: `W/worker_budget.rs` gains `inner: usize` (default 0 = off; explicit `--inner-apply-workers k` carved from inspector slots so `inspection + helpers + inner + coordinator == workers`); `W/execution.rs::run_pool` builds one shared `InnerExecutor` and `W/inspection.rs::inspect_native` passes it. `RAYON_NUM_THREADS=1` (launcher) only affects the global pool; private pools are unaffected, and CPU affinity still bounds total cores to W, which is why `inner` must come out of the budget.

**Speedup estimate and interaction with Ready.** Per head: if ≥85% of head time is in `apply_group` tasks, k=4 → ~3×, k=8 → ~4.5× (Amdahl; replay, matching and geometry stay serial). Campaign-level: heads already overlap across inspectors under Ready, and a 1M-successor head still needs ~13 s of coordinator time at today's per-request cost; inner parallelism only shortens wall when the coordinator has headroom or when few heads remain runnable (campaign tail). It also raises peak memory (owned coefficients in the window).

**Recommendation.** Implement the mechanism and its equivalence tests, ship **off by default** (`inner = 0`), sequence last, and enable in pilots only after §2/§3 telemetry shows `coordinator duty < 60%` with idle inspectors. Honest expectation: useful for the tail, not for the current bottleneck.

**Tests** (core; license-gated by `LicenseManager::is_licensed()`; skip when `max_threads(k) < k`): `inner_parallel_apply_emits_identical_event_transcript_and_stats` (sequential vs k ∈ {1,2,4} over the applied fixtures, comparing serialized owned transcripts and `OwnerAppliedStats`), `inner_parallel_reproduces_every_resource_limit_failure_point` (each limit swept over 1..M), `inner_parallel_first_refusal_only_is_emitted`, `consumer_stop_mid_replay_discards_buffered_tail`, `inner_pool_requires_license_on_every_thread`. App: `inner_apply_workers_keep_replay_prefix_digest` (digest of `replay::token` runs identical) in `W/execution/ready_native_tests.rs`.

**Effort.** 6–9 days.

---

## 7. Worker scaling

- **Cap 64 → 256**: `crates/rustred-app/src/cli/args/owner_match.rs:405-407` and test `:761` (`--workers 65` → `257`); `W/mod.rs:219` `(1..=64)` → `(1..=256)`; `W/worker_budget/tests.rs` loops `1..=64` → `1..=256`; Python `examples/python/shared_owner_campaign.py:402-403` (`1..50` and aggregate ≤ 50 → ≤ 256 and ≤ `len(sched_getaffinity)`), `production_saved_owner_campaign.py:236,259`. `preflight_requested_core_budget` already rejects W > available cores.
- **Split heuristic (Ready, W = 50..128).** Keep today's default for W ≤ 64 (existing expectations untouched); for W > 64 under Ready: `helpers = min((W−1)/2, 32)`, rest inspectors. Rationale: prep parallelism is bounded by the 256-record batch (8 admissions/helper at 32 helpers; the min-eight-task-grain experiment found no gain from finer grain), so extra cores are better spent on inspectors — but only once the coordinator has headroom. Table: W=50 → 25/24; W=64 → 40/23; W=96 → 63/32; W=128 → 95/32. Implement in `WorkerBudget::new` (`worker_budget.rs:73-79`); test `ready_large_budgets_cap_helpers_at_32_and_preserve_smaller_defaults`.
- **Gate.** On a restricted five-loop pilot at W=50 then W=96: `native_busy_cores` (supervisor `/proc` sampling, `shared_owner_campaign.py:655`) ≥ 0.6 × W and coordinator duty < 60% (§8). If busy cores do not rise with W, stop raising W: the coordinator is saturated and §2–§4 are the lever. Memory: +2 × 8 MiB × extra inspectors (interface to the memory agent).
- **Effort.** 0.5 day + pilot time.

---

## 8. Live-campaign monitoring additions

Rust (`W/execution.rs::progress` `:173-208`, `W/parallel.rs::snapshot` `:370-407`, `W/execution/admission.rs::Metrics::json`):
- `coordinator_duty` object: session wall seconds for `dispatch`, `poll`, `preparation` (exists), `ordered_commit` (exists), `publication` (Finished commits), `wait` (`ready_streams.wait`/`pool.wait`), `closure_refresh` (exists), `checkpoint`, `progress_json`, plus `coordinator_elapsed_seconds`; monitor derives shares. Timing hooks around the existing call sites in `run_pool`; cheap `Instant` reads only.
- Pool: per-slot cumulative `backpressure_seconds`, `busy_seconds`, `idle_seconds`; snapshot adds `computing_workers` (running and not blocked), `finished_awaiting_poll`, `heaviest_active_stream {id, seconds, attempted_events}`, `stream_stall_share` (= Σ backpressure / (elapsed × slots)).
- Admission: `forward_callbacks`, `forward_bit_rejections`, `reverse_callbacks`, `reverse_bit_rejections`, `prepared_retirements_applied`, `prepared_retire_fallbacks`, `speculative_reverse_checks`.
- Python `examples/python/campaign_monitor.py::progress_summary` (`:215-270`): new optional `coordinator` and `inspectors` sub-objects (missing → `None`, never inferred); `dashboard` (`:296-360`): a `Coordinator commit 41% · prep 12% · publish 6% · wait 20% · closure 1%` line and `computing N` on the Workers line. Tests in `test_campaign_monitor.py`: `test_coordinator_duty_and_computing_inspectors_are_optional_and_bounded`, `test_old_status_without_duty_fields_renders_unknown`. Existing `native_busy_cores` remains the ground truth.
- Effort: 1.5 days.

---

## Semantics-change flags (for the WALK_SEMANTICS_VERSION owner)

| Item | Admission results / records / edges | Persisted counters | Verdict |
|---|---|---|---|
| 2 bit tier (a) | identical | identical | none |
| 2 (b) block words | identical | identical | none (fields `#[serde(skip)]`) |
| 3 prepared retirement | identical (proof §3) | identical | none |
| 4 pipelining | identical by construction | identical | none |
| 5 closure cadence | monitoring only | `refresh_count/seconds` differ (telemetry) | none |
| 6 inner parallelism | identical transcripts (tests enforce) | none | none; scheduling only |
| 1(f) slot recycling, 7 split | Ready schedule-dependent as before | none | none |
| 1(c) launcher default | new campaigns only | — | policy, frozen per campaign |

---

## Sequencing and dependencies

1. **Week 1 (no dependencies, all measurable on FG/BMW/H/X):** §2(a) → §3 → §5 → §8 counters → §1(b,c,d) → §7 cap/validation. Run the release suite (`nix develop --command cargo test -p rustred-app --release`) and the four controls with exact-counter comparison against the pre-change binary.
2. **Week 2:** §1(a) in-process test + env seam + fresh-process harness (needs §8 fields for the trigger evidence); §1(f) slot recycling; five-loop 1,324-tuple W50 control (Ordered exact equality; Ready audit + timing); decide §2(b) and §4 from the gates.
3. **Week 3:** §6 mechanism off-by-default with equivalence tests; §4 only if its gate fires; restricted five-loop pilots at W=50 and W=96 for §7's gate.
4. Launch configuration: Ready, H=256, W chosen from the §7 gate, `inner=0`, closure divisor 100, no subdivision.

Total: ~12–16 engineering days excluding pilot wall time, of which §4 and §6 (5–8 + 6–9 days) are gate-conditional.

### Critical Files for Implementation
- /common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/queue.rs
- /common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/queue/index.rs
- /common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/queue/prepared.rs
- /common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/execution.rs
- /common/dev/rustred/crates/rustred-core/src/solver/candidate_reduction/owners/domains/applied/engine.rs
