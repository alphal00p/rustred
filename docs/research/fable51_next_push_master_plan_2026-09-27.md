# fable_5_1 next push: master plan (v3 epoch engine, work-volume levers, launch criterion)

> **Errata (2026-09-28):** §11 lists the Fable 5.1 audit's corrections and the owner's answers of 2026-09-27/28;
> where they conflict with the body below, §11 wins. The body is kept as written.

Date 2026-09-27. Base: branch `fable_5_1` @ 4be9fdf6 (the root-blocker note, on top of the v3 design note 260b49cc and 8ea26917). Status: plan only; nothing here is implemented. Both five-loop campaigns are stopped; no `rustred` process was running at 11:45 UTC or at this revision (`pgrep`).

Labels: **[M]** measured (receipt or note cited); **[M-r]** measured by a reviewer or lens from campaign data, script in the session scratchpad; **[src]** checked in source; **[E]** estimate. Nothing here is an ETA or a closure claim. Termination of the five-loop walk is not established.

Paths: `W` = `crates/rustred-app/src/application/routed_campaign/walking`; `C` = `crates/rustred-core/src/solver/candidate_reduction`; `v2` = `campaigns/five-loop-qcd-feynman-d9d10-v2` (run 20260926T151353.794886Z, binary 102adcc3, inputs plan-v3); `interim` = `campaigns/five-loop-qcd-feynman-d9d10` (run 20260926T122852.894005Z, binary 32fdec09, inputs plan-v2); `$S` = the session scratchpad `/tmp/claude-1125/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/scratchpad` (ephemeral; W0.1 persists its tools).

Inputs to this plan:
- eight survey lenses (engine state, measurements, early archive, late archive, algorithmic, native inspection, physics inputs, systems/literature);
- four judged strategies with three skeptic votes each: CWE coordinator-free engine 6/5/6; Measure-Rebuild-Collapse 7/6/6; v3-cells moonshot 6/5/5; Cells First 3/3/3, whose submission was empty, so it was never judged on its merits;
- the v3 engine design panel (`docs/research/fable51_v3_engine_design_2026-09-27.md`);
- the committed root-blocker note `docs/research/fable51_root_blockers_2026-09-27.md` (receipt `TMP/root-blockers-gen6/receipt.json`, one analyst, two verifiers);
- a completeness critique of the draft (§10).

Governance: W0.0 makes this plan the goal of the push, with a dated `GOAL.md` directive and a decision-log entry in `FABLE_5_1_five_loop_vacuum_plan.md` §6. For this push it replaces that plan's §3-§4. It carries that plan's §3.F in-flight thresholds into §7 and keeps its §G per-commit gates (§7).

## 0. The decision in brief

1. **Engine.** Build the v3 `epoch` engine (walk semantics 3, checkpoint CP6).
   - Each native inspection is handled whole. The inspecting thread resolves its successors against immutable snapshots.
   - One coordinator merges whole inspections and is the only thread that changes state.
   - This removes the serial per-successor path that bound v2. Of the four engine designs judged, it has the smallest concurrency surface.
   - The lock-free CAS engine (CWE) is deferred (§9).
2. **Work volume runs as a parallel lane from W0, not as a final wave.**
   - Termination is not established, and pending grows by 0.3-2 per completion. A 5-20x faster engine only reaches the RAM wall sooner (≈10-250 h [E], derivation in §3.8).
   - Offline oracles on C-HOT and gen 7 start in W0 (W0.11). W0 ends with one batched decision session (D1-D8, §4); unanswered items take conservative defaults.
   - Any lever that is signed off and that W0 projects at ≥2x less work becomes launch-blocking.
   - Levers with measured headroom: closure-directed priority dispatch; residual inspection against merged native anchors; conservative input coarsening (hybrid and envelope helpers, route witnesses).
3. **Native algebra comes third.** Modular zero certificates and allocation-free geometry are built on the legacy engine from day 1, in parallel, and gated by identical successor sets.
4. **Not adopted as the core design:**
   - cells-first and hull widening. The refusal rests only on the per-owner cost regression (points^0.74-0.90), and the W0.9 falsifier tests it.
   - bucket actors and independent owner shards.

   Union-cover aliasing, symmetry canonicalisation and piece-level certification are not refused. They are decisions for Valentin (D2, D6, D1), with offline bounds measured in W0.11.
5. **Every lever is gated by pilots of at most 1 h**, restore included, on fixed controls. The metrics keep their meaning when a lever changes the unit of work (§7).

## 1. State of play (measured facts only)

**Campaigns.**
- **v2** (W100 = 67 inspectors / 32 helpers / 1 coordinator, Ready, lookahead 256) stopped at 67,788 s [M: `v2/runs/.../result.json`, `checkpoints/main/meta-...07.json`]:
  - 27,465,422 natives; 74,156,033 discovered domains; 45,889,639 committed; 28,266,394 pending.
  - 18,424,217 delegated publications; 26,118,532 transfers; 1,192,281,291 dependency edges.
  - 0 frontiers; 158,552 optional coefficient refusals.
  - 8/67 initial roots closed, all 8 by 1.45 h and none after.
  - Max rank 18 from 2.92 h on; peak RSS 297 GB.
- **Interim** (W50, 60 of 67 helpers with unbounded A): 17.41M natives, 53.2M scheduled, 12/67 roots, 1,299 frontiers at its last heartbeat. The first frontier came at 538 s [M-r: `interim/.../events.jsonl`].

**Where the time went (v2).**
- Coordinator wall 66,896 s: ordered_commit 51.4%, preparation 38.0%, dispatch 2.7%, poll 1.4%, progress_json 0.95%, checkpoint 0.90%, closure refresh 0.87%, publication 0.76%. Total wait was 0.4 s [M: `result.json` `parallel.coordinator_duty`].
- On average 2.69 of 67 inspectors were computing (4.0%), falling to 0.37-0.67 after 9 h. Inspectors were busy 180,248 s net, i.e. 6.56 ms per native [M].
- Process CPU averaged 10.8 of the 100 reserved cores (`resources.jsonl`) [M].

**Growth law (v2 windows; `$S/meas/fits.py`) [M-r].**
- Helper forward checks per request rose from 136 to 2,000-2,850; checks ∝ live^1.05 (R² 0.61).
- Wall µs per request rose from 5.8 to 30-56; commit µs per record from 2.8 to 16.7-31.7; prep µs per batch from 199 to 1,866-2,852.
- Maintenance per new domain ∝ live^1.26. Cumulative wall ∝ N^1.59.
- Completions per hour fell from 4.51M in the first half hour to 0.64-0.85M after 14 h.
- W100 v2 and W50 interim reached the same late throughput: ~0.75M/h, with ~4.8 ms of coordinator time per completion at ~16.5 h.

**Admission outcomes and locality.**
- 5.53G admission requests (201 per native). Outcomes: containment hits 96.15% (72% of them semantic), exact 1.65%, orthant 0.86%, new domains 1.34% [M].
- 4.232T speculative checks (774 per request); the bit word rejected 89.6% of them [M].
- About 85% of Apply successor events hit a container the same inspection had already hit (0.114 distinct targets per successor) [M-r: `$S/join7.py`, `$S/edges_age.py`, `$S/fanin.py`].
- 86.3% of first-hit containers are at least 4.2M admissions old; 2.53% are younger than 4,096 [M-r: same scripts].

**Native work.**
- Apply: 5.95M inspections, 241,437 record-seconds. Route: 21.52M inspections, 1,951 s. Route is 78% of natives and 74% of nodes but under 1% of native seconds [M-r: `$S/records_scan.py`].
- Owner 011101110111000 carries 63.6-64.1% of record-seconds and 36.5% of events [M-r].
- The top 1% of natives carry 77.7% of gen-7 native seconds. The longest native took 178.9 s including backpressure; the longest in the clean pilot took 25.4 s [M-r].
- Coefficients: 21.6% of 6.78G term visits are zero; coalescing_additions = cancelled_groups = 0. The walk reads only the zero/nonzero decision [M-r; src `W/inspection.rs` `inspect_native`].
- Inspector profiles (older binaries; truncated inclusive shares, so lower bounds): specialization 19-31%, guards 15-25%, heuristic GCD 12-16%; allocator 24-28% exclusive [M-r: `TMP/five-loop-*-profile*/report-*.txt`].
- **Per-owner cost law** [M-r: `$S/perfskeptic/scale_owner.txt`, `scale_v2g6.txt`]:
  - Within an owner, cost ∝ points^0.74-0.90 and successors ∝ points^0.7-1.0.
  - The two hottest owners are superlinear (exponent 1.4-1.7) in the top size decade.
  - The pooled points^0.2-0.6 law is a mix effect across owners.

**Index at gen 7.**
- 8,040 buckets (67 Apply, 7,973 Route) hold 37.9M live candidates in 459,075 groups and 1,993,524 blocks, with fill 59.4% [M-r: `$S/index_census.py`].
- IDs inside a block average 696,417 apart. The largest bucket holds 2.9% of live candidates [M-r].
- Offline replay [M-r: `$S/perfskeptic_cwe/scan2.rs`, `scan3.rs`]:
  - u8 lanes suffice: 0 of 74.16M tight summaries escape.
  - First-found cuts candidates per hit by 51% (7,374 → 3,633).
  - A miss tests about 11.2k candidates (p99 70-142k), scaling as N^0.52-0.66 under thinning.
  - Clustering by Σ lower bound is worse (+47% per miss, 2.9x per hit). Ordering by finite-upper pattern, then lexicographic lower corner, is best (9.9k per miss, 3.2k per hit).
  - Gathers on the real layout cost 53-60 ns per tested candidate. The top bucket holds 4.8% of gen-7 new IDs.
- Host membench: a dependent random load costs 143 ns on the local socket and 240 ns cross-socket; streaming costs 2.2-2.7 ns per candidate [M-r: `$S/membench`].

**Waste.**
- 10,129,834 of 27,465,422 inspected domains (36.9%) were later contained by a newer admission [M-r: `$S/indexscan/src/doms.rs`]. This is a loose upper bound on avoidable work, because some inspections create their own later container.
- Drained hot-owner pilot [M-r: `$S/pilot_cover2.txt`, `$S/rtool/src/main.rs` mode `cover`]:
  - Apply points overlap 19.5x; only 3.4% of admitted Apply points were new (9.50e6 of 2.76e8).
  - 71.1% of Apply inspections, carrying 69.3% of Apply native seconds, ran on domains already covered by the union of earlier-ID records. That count includes pending and aliased records.
- 82% of Apply domains in the 47 owners bounded only in v2 lie inside the interim helper box, i.e. they are A-escapes above the plan-v3 A_max [M-r: `$S/inputs_lens/an2.py`].

**Root blockers (gen 6) [M: `docs/research/fable51_root_blockers_2026-09-27.md`].**
- Roots 25/35/42/43 closed in the interim but not in v2. Their regions are small (10.7K-47K nodes) and are blocked only by pending nodes that were never inspected:
  - 35/42: 20 blockers at ID 61.73M, with 13.6M obligations ahead of them in the ID-ordered Ready queue;
  - 25/43: 4,227 blockers at IDs 48.4-62.8M, with 3.8-14.4M obligations ahead.
- Their regions escape both inputs' helpers through rank (13-17 against helper ranks 11-13) and through the Route phase, not through A.
- Root 42 only: each dependency level was admitted at the queue tail, one pass after its parent (largest ID per level 0.21M → 61.73M over 9 levels). For 25/43 many levels sit within ~1M IDs, so depth does not equal queue passes there.
- Absorption by interim helpers:
  - No node in the four regions lies inside an interim initial helper box, so the narrow absorption hypothesis is refuted.
  - Absorption into interim non-initial representatives was not tested.
  - Globally, 12.8% of v2 nodes are interim-contained. Adding everything reachable only through them gives 43.5% of the graph and 54% of the unreserved queue (an upper bound).
- Not established (verifier findings): why the interim reached these chain ends sooner. Untested contributors are the shorter queue, W50 vs W100, binary 32fdec vs 102adcc3, and non-initial absorption. The interim queue was not measured.
- Root 8's gen-6 cone holds 63.05M of 68.87M nodes (92%) [M: receipt `rows`].

**Memory and checkpoints.**
- Restoring gen 3 loads 28.8M domains into 11.0 GB (382 B/domain) in 164-177 s. Restoring gen 6 takes 337 s and 26.8 GB [M: `fable51_restore_at_scale_2026-09-27.md`; root-blocker receipt].
- W50 control marginal RSS: 0.70-0.75 KB/domain on the new binary vs 7.5 on 102adcc3 [M-r].
- Gen-7 CP5: 46.1 GB, of which records JSONL are 30.3 GB; the final save took 22.5 s [M]. Records cost ≈620 B each [M: v3 note §8].

**Host.**
- 2x EPYC 9754 (Zen 4c, AVX-512), 8 NUMA nodes. Socket 1 (CPUs 128-255, nodes 4-7) has no online SMT siblings and 564-594 GB of local memory [M-r: node meminfo vs lscpu].
- MemAvailable 651-692 GB (651 GB at this revision, `free -g`, no rustred process). ZFS ARC 226-349 GB observed; zram swap 500 GB. THP is effectively unavailable (compaction fails 99% of the time).
- `/`, `/common` and `/tmp` are all datasets of `zroot`, one NVMe vdev with 2 permanent data errors (`zpool status`, `zfs list`). No off-pool target exists today.
- About 50 foreign busy CPUs were seen on socket 1 at 11:21-11:35 [M-r].

**Termination signals.**
- Native pending per completion: 0.82 in the first half of v2, 0.63 in the second [M-r].
- Total pending growth per completion is 0.31-1.96 per 30 min window, with no trend and no pending peak [M-r].
- The rank cap stayed flat at 18 for 15.7 h; A ≤ 25 over all descendants [M-r].

## 2. Earlier notes: what holds and what is outdated

**Still right.**
- Admission decisions are bucket-local, and ~95% of requests are hits (`fable51_coordinator_relief_design_2026-09-26.md` §0-1; v2: 96.15%).
- Cut admission cost before adding workers (`finite_domain_parallel_audit_2026-09-22.md`).
- Rule generation is not the lever: v2 had 0 frontiers and 0 problems. Independent owner shards were 2x slower (`five_loop_completion_levers_2026-09-26.md` §3; `joint_pruning_independent_campaigns_2026-09-25.md`).
- Narrowed A19 helpers slow the four-loop walk 3.9-15.7x; R-free helpers leave 85-556 frontiers (`four_loop_helper_bounds_2026-09-25.md`).
- Without anchors the four-loop finite envelope fragments: FG stopped at 1.35M natives with 764,122 pending, against 12.9 s with rank-only anchors (`four_loop_saved_cover_control_2026-09-24.md`).
- Guard obstructions are genuine affine diagonals, so A stays finite on guard-reachable owners (`guard_obstruction_triage_2026-09-22.md`).
- Same-support Apply has ΔP ≤ 0 (`finite_closure_architecture_review_2026-09-23.md`).
- The D residual was 9-24x cheaper locally (`five_loop_utilization_2026-09-23.md`).
- Measure selectivity offline before restructuring the index (completion levers §2.5). Frontiers must be 0 early (`FABLE_HANDOFF.md` §7.4).
- On-demand repair, or a smaller workload, changes the delivery contract and needs the user's approval (`radical_parallel_architecture_2026-09-24.md`); hence D8.
- The epoch design and its F1-F20 (`fable51_v3_engine_design_2026-09-27.md`).
- The `FABLE_5_1_five_loop_vacuum_plan.md` §3.F thresholds (rank ≤ helper + 7, pending growth against the old run, roots closed increasing). They are carried into §7, with merge duty in place of coordinator duty.

**Outdated or refuted.**
- "Ordered-head serialization is the bottleneck" (memory note; levers §1). The bottleneck is the single coordinator's admission: 97% duty, 89.5% of it prep plus commit.
- "~4.5 KB/domain, RAM wall ~160M". The figure is now 0.38-0.75 KB/domain. The 3.7 KB/domain in `status.json` is an artefact of the RSS plateau after 11 h.
- Relief items 1a/1b first: their combined ceiling is below ~6% at production scale. The wave-2 profiling note's §3 "E1 not supported" holds only at 1.3M domains.
- Handoff §8.1 "commit 3.8-4.3 µs" and the levers note's "~250 checks per lookup": the late values are 16.7-31.7 µs and 2,000-2,850 checks.
- Levers §2.1 "Ready at more workers gives ~2x": W100 and W50 reached the same late throughput.
- "Successor coalescing: nothing to merge": true for coefficients, but misleading for containers (85% repetition).
- "Heavy heads hold a slot for hours": the maximum was 179 s. The scheduler design's §6 claim of 3-4.5x from inner parallelism caps at 2.07x at k=4.
- Memory note "wider helpers refuted": a misreading. Narrowing was slow; unrestricted helpers were fast but produced frontiers.
- RESULTS v3 "bound A on every t ≥ 8 helper": over-conservative on the observed graph (27 such owners never reach a guard owner), but this rests only on observation (§4).
- "99% of pending lies above the anchors": a tautology under aliasing.
- "The interim's extra roots come from helper absorption": the narrow version (initial helper boxes) is refuted for the four roots' regions. The blocking fits ID-ordered reservation, but causality is not established (root-blocker note).
- The pooled points^0.2-0.6 law is refuted per owner (§1). The v3 note's W150 split on CPUs 28-177 is superseded by the 100-core launch on socket 1.
- "Flat bucketed lists are the right shape": its quantitative premise is outdated (checks ∝ live^1.05; misses ∝ N^0.52-0.66). The rejection of trees stays open (§3.3).
- CP5 checkpoint-review follow-ups: #1-#6 were applied to CP5 (handoff §7). The open items are not fixed in CP5, which is deleted after launch; they become CP6 requirements (§3.6).

## 3. Target architecture and algorithms

**3.1 Engine: `epoch` as specified in the v3 note, plus amendments.**

Base design:
- `EpochState` holds:
  - an append-only `CompactDomain` arena and per-ID immutable summaries;
  - `ExactIndex` with overflow;
  - `ledger6`, 8 B per ID: Pending | Reserved | Native{frontiers,error} | Alias{to};
  - a `live` bitset, the protected prefix, the anchor map and a u64 epoch.
- The snapshot is an `Arc`, swapped at each merge. A merge has four phases:
  - P1: checks;
  - P2: parallel work per bucket;
  - P3: serial apply, preflighted so it cannot fail midway;
  - P4: layers.
- Controls use `Lockstep`, a canonical min-ID mode that is byte-identical across worker counts and serves as the oracle. Production uses `Rolling`.
- Epoch code is `#![forbid(unsafe_code)]`, except the confined SIMD kernel (A5).

Amendments:
- **A1, verify chokepoint in production.**
  - Every positive (exact, self, Local, MRU, orthant or helper, layer, delta, transfer) passes `verify(t,q)`. It checks the published length, then the stored image's phase and owner, then the native predicate.
  - This is needed because `CompactSummary::contains` ignores phase and accepts EMPTY first [src `W/queue/compact.rs` 513-543].
  - The debug-only WIDE assert becomes a release check. Exact hits compare digest and image.
  - Orthant and helper shortcuts test the full power bounds. Today's rank-only test is valid only because `is_full_orthant` requires unconstrained powers [src `W/queue.rs:77`, `compact.rs:183`, `W/initial_orthants.rs`].
- **A2, failure paths.** The draft's "errored results never merge" contradicted the `ledger6` Native{error} state, and it would re-poison every resume after a deterministic error. The rules are:
  - Transient failures (cancellation, consumer stop, allocation or resource exhaustion) are discarded. The parent stays Reserved and is dispatched again (F7).
  - Deterministic failures are merged as unsealed Native{error}, with their record and any successors already emitted; those successors only add obligations. Deterministic failures are F7's resolver errors (coordinate > 65534, summary error, overflow) and native algebra errors.
  - An error of unknown class is retried once. If the same error recurs on re-inspection, it is treated as deterministic.
  - After a deterministic error the run saves and stops with a distinct exit code. On resume, Native{error} entries are not dispatched again, so resumes do not loop. The error stays explicit and blocks certification of every ancestor. Whether to continue past it is D7.
  - A result with frontiers is merged as Native{frontiers}, unsealed, with its frontier records persisted; then A10 applies.
- **A3, bucket keys.** Buckets are interned dynamically, because Route admits arbitrary masks that are not installed [src `W/inspection.rs`; `W/routing.rs` `missing_route_cover`].
- **A4, full reverse retirement.**
  - It stays exact and complete. In v2 it avoided about half of all inspections (26.1M transfers vs 27.5M natives).
  - P2 builds reverse sets block-parallel inside a bucket, because late reverse work was 27-51k checks per native.
  - A Reserved entry never transfers.
- **A5, kernel encoding.**
  - Forward (candidate = container): a query value above 254 saturates to 255 = +inf.
  - Reverse (query = container): a finite container bound above 254 goes to the scalar path.
  - Signed D lanes use a bias, with escape lists on both sides.
  - SIMD is only a prefilter; A1 decides.
- **A6, snapshot refresh.** A long inspection refreshes its snapshot every N events and re-resolves its outstanding misses. This matters for heads of up to 261k successors.
- **A7, binding (a change to implement).**
  - Today `binding()` [src `W/checkpoint.rs:104-119`] hashes `workers`, `inspection_workers`, the `Debug` form of `scheduling_policy` (lookahead included) and `max_frontiers`. A re-split therefore needs a fresh run, and W0.5 must resume at W100.
  - CP6's binding drops workers, split, enqueue depth and schedule. Performance-only binaries and re-splits then resume via `--upgrade-executable`.
  - Every semantic flag (§3.12), `--frontier-policy` included, is bound into the request digest.
- **A8, restore validators.** F4, F8, F16, the enqueue-depth check of F17, and merge-epoch stamps for multiple anchors (§3.11).
- **A9, certification reporting.** Report the 67 helper roots and the 116 physics queries separately. Physics roots are admitted as hits on their helpers (v2 `initial_total` is 67 for 183 queries).
- **A10, frontier policy.** `--frontier-policy stop` is the default and is bound per A7: save and stop on the first frontier. Today `--unbounded-work` makes frontiers non-halting (`max_frontiers = usize::MAX`) [src `W/work_policy.rs` `disable_work_limits`]. Recovery options are D7.

**3.2 Resolution pipeline on inspectors**, in order:
1. A byte-bounded job-local exact set. It replaces `MAX_KEYS = 4096` [src `W/reuse.rs:13`], which binds for inspections that carry 75.8% of successors.
2. Self-scope: S ⊆ the inspected scope (or its residual for partial jobs). No edge.
3. Local sibling containment.
4. Per-job verified targets per (phase, owner): by default 64 most recently used (MRU) targets plus a hash of exact targets. 81% of gen-7 edges come from jobs with ≥32 distinct targets [M-r: `$S/perfskeptic_cwe/deg.rs`].
5. Helpers and orthants, verified against the store.
6. Layers, newest first; the first verified container wins.
7. Otherwise `Miss{image, v}`. The miss buffer spills to disk and never fails (F19).
- `--epoch-resolve merge` remains the fallback if lookups on inspectors prove bandwidth-bound.

**3.3 Layers and kernel.**
- 32-entry SoA blocks: u32 ids, u64 words, u8 lanes for the 36 comparison fields with escape lists, and block OR/AND words.
- Blocks are ordered by finite-upper pattern, then by lexicographic lower corner.
- Layers are immutable, with size-tiered compaction.
- If the W0.4 exponent projects the admission share at N = 1G above 25% of worker CPU, W3.2 adds a sublinear miss structure. Candidates: per-group dominance pruning on sorted lower corners, or the 2N+6 feature trie of `domain_admission_index_next_step_2026-09-22.md`.

**3.4 Dispatch: priority, not FIFO.** Dispatch order is performance-only. The class is persisted per ID as a u8 or rebuilt on restore.
- Classes, highest first:
  1. Reserved after resume.
  2. Closure boost: Pending IDs in the unresolved cone of roots whose blocker set is small (§3.5).
  3. Default: owner support t descending, then box volume descending.
- An age bound prevents starvation. Variants to A/B: pure depth-first, and the Herbreteau-Tran boost (FORMATS 2015).
- Targets: blocker chains that span many queue passes (root 42's pattern, not 25/43's), and the 36.9% of inspections later contained (a loose bound).
- **Enqueue depth.**
  - F11 sets Reserved at enqueue, and a Reserved entry never transfers. Every ID sitting in a deep queue therefore loses the transfer path (26.1M transfers in v2) and is inspected even if a container arrives meanwhile.
  - Priority therefore acts on Pending before enqueue.
  - The enqueue depth is a fixed small multiple of the inspector count, the analogue of lookahead 256. It is not bound (A7) and is validated on restore (F17).
  - W3.1 gates the depth with M-mist.
- Route micro-batching: k Route IDs per job. Route is 75-78% of natives, at a median of 27 µs each.

**3.5 Closure monitor.**
- Parallel refresh on a frozen prefix, off the coordinator, using u64 CSR offsets with no `LOG_CAP` (F14/F15) [src `W/descendant_closure/edges.rs:14`].
- Per-root cone sizes and blocker counts feed §3.4.
- Edges with a closed endpoint can be evicted, because reverse reachability from unsealed nodes never traverses them.

**3.6 CP6, stops and resume.** As in v3 §6:
- Domains, edges and records are append-only; `ledger6`, nodes, `live`, anchors and meta are rewritten; layers, the exact map and summaries are rebuilt in parallel.
- Cooperative stop: 60 s grace, then unmerged results are discarded. RAM-guard stop: no grace (F18).
- Background saves write a frozen cut at a merge boundary, with watermarks for the append-only sections.
- The latest generation is copied to an off-pool target once one exists (W0.10).
- The RAM guard watches process RSS, host MemAvailable and swap-in.

Requirements carried over from `fable51_review_checkpoint_branch_2026-09-26.md`; each gets a test in W2.2:
- #1: the manifest has a self-digest, and each section's presence is bound to the policy.
- #2/#13: the segment count per section is bounded, and the supervisor reads with the store's manifest cap.
- #3-#5: the change stamp covers every persisted field (paused state, partial progress), and a skipped save resets its timer.
- #6: cleanup errors after a durable publish are reported, not fatal.
- #7: inventory checks do not depend on closure availability.
- #8: leftover staging temp files are swept on open.
- #9: `latest` is installed before `previous`.
- #10: manifest and event carry the same `effective_interval_seconds` value.
- #11: the pre-save refresh is cancellable and counted inside the save timer.
- #12: the stretch of the save interval is capped.

**3.7 Records, telemetry and deliverable.**
- Inspectors serialize typed binary records. A decoder CLI and Rust streaming audit and verify tools ship with them; their runtime per 10^8 records is measured in W2.6.
- The Python `audit_owner_domain_walk.py` stays for controls only.
- The deliverable is the final CP6 generation plus the typed records and a compact summary JSON (roots, counters, digests). There is no `domains` JSON array: at 10^9 records and ≈620 B per record it would be ≈620 GB [E].
- Counters are atomics sampled at 1 Hz. Per-domain progress JSON is removed; it built about 50 keys per event [src `W/execution.rs` `State::progress`].

**3.8 Memory budget, RAM wall and placement.**

Budget per ID [E]:

| Structure | Bytes per ID |
|---|---|
| domain | 96 |
| ledger | 8 |
| exact map | 24-31 |
| CSR edges (16.35 edges/domain [M]) | 65-130 |
| live bitset and nodes | ~1 |
| layer lanes | ~50 per live ID |
| summaries | 176 per live ID only |
| **total** | **0.3-0.45 KB** |

At any feasible RAM this stays below the u32 cap of 4.29G IDs.

**RAM wall [E].**
- Working budget: 600 GB. The 700 GB guard exceeds today's 651-692 GB MemAvailable, and 750 GB is infeasible on this host (D5).
- At 0.3-0.8 KB/domain, 600 GB holds 0.75-2.0G domains. The range combines this budget, the v3 note's §8 estimate of 0.45-0.8, and the measured 0.38 after restore.
- Discovery rate: 2.70 domains per native [M: 74.16M/27.47M] × 5-20x v2's natives per hour (0.61-0.76M/h late [M-r], 1.48M/h over the whole run [M]) = 8-80M domains/h.
- The wall is therefore 10-250 h away if pending keeps growing. The v3 note's §8 gives 1-7 days for 0.9-1.5G domains.

**Placement.**
- Socket 1 has only 564-594 GB of local memory, less than the guard.
- Hot, randomly accessed structures stay on nodes 4-7: layers and lanes, live summaries, the exact map, the ledger and the live bitset.
- Cold or streamed structures go cross-socket or to disk: record segments (disk only), sealed domain segments (mmap), edges with a closed endpoint (evicted), and images of low-priority Pending IDs.
- The split is decided in W3.3, using the W0.3 bandwidth numbers.
- GMP allocates through C malloc (`gmp-mpfr-sys`, and no `set_memory_functions` call exists in the tree [src grep]), so a Rust `#[global_allocator]` misses those allocations. N3 therefore uses mimalloc's malloc override, and W0.3 measures the share of C allocations.

**3.9 Native levers.**
- **N1, modular zero certificates.**
  - Scope: only the check "coalesced group numerator not identically zero".
  - Evaluation happens at a lattice point inside the cell that respects the rank limit, with random base symbols. A nonzero denominator residue is required; a zero residue falls back to the exact path. Memo keys are the exact restricted cell.
  - These stay exact: the source-condition zero-locus, InvalidChildRoot, zero sectors and descent checks. Classification reports Conditional/Unknown, never Uniform.
  - Soundness checks out: `Zero::Yes` requires an identically zero value, and `ZeroDenominator` fires only on an identically zero specialized denominator [src `algebra/indexed/specialization.rs:133,345`].
  - Expected record difference: optional coefficient refusals disappear, because the exact path emits them only when the zero-locus preflight refuses [src `C/.../applied/algebra.rs` `coefficient`, `applied/engine.rs` `classify_coefficient`].
- **N2, allocation-free applied geometry** [src `C/.../applied/engine.rs` 312-320]. The prepared RHS permutation is dropped: it appeared in 4 of 3,070 samples (0.13%) (`finite_closure_architecture_review_2026-09-23.md`).
- **N3, allocator and build.** mimalloc with malloc override, so C allocations are included (cached offline; symbolica `faster_alloc` is off). Add a `[profile.campaign]` with fat LTO, codegen-units = 1 and target-cpu = znver4; the workspace has no release profile today [src `Cargo.toml`].
- **N4, cover-first (after the engine).**
  - Skip numerator specialization on covered images, but keep denominator specialization (ZeroDenominator is raised in `algebra/indexed/specialization.rs`).
  - Validity problems on a covered image re-run the exact path.

**3.10 Work-volume algorithms.** Bounded offline in W0.11; implemented in W4 after sign-off (§4).
- **G2', residual inspection against merged native anchors.**
  - This generalizes `W/initial_overlap.rs` from initial D-bands to any anchor.
  - A newly admitted Apply domain Q keeps its identity, but its job inspects only Q minus the union of the anchors in its bucket, with an edge to each anchor.
  - Anchors must be Native, merged strictly before the job's dispatch snapshot, and carry a persisted merge-epoch stamp. This rules out mutual subtraction, a false-closure path found in the moonshot's version.
  - With an empty residual, G2' *is* union-cover aliasing restricted to merged Native anchors (D2).
  - Residual cuts need lower bounds on A and R (C2).
- **G2'-C2, power vocabulary.**
  - Add `min_positive_power` and a rank lower bound (P = A+R is optional) to `DomainPowerBounds`.
  - Carry them through every representation: extrema, projection with lattice parity, compact layouts and flags, bit words, signatures, digests, the codec, and orthant or overlap shortcuts.
  - Route transport sets every lower bound to None, since only D' ≥ D is proved [src `routed/domain_overcover/power.rs` `mapped_bounds`].
- **Conditional on W0 evidence:**
  - Apply-only hull widening on an allowlist of (owner, rank, A cap), never tightening an axis;
  - adaptive dense/sparse level cells (needs C2; W0.9 oracle);
  - census P-anchors with a per-ID protected flag;
  - parallelism inside a heavy head, committed as one logical native.

**3.11 Anchor bookkeeping.** CP6 stores multiple anchors per node, each with its merge epoch. The restore validator checks that anchor.merge_epoch < node.dispatch_epoch, that each anchor edge is present, and that each anchor is Native.

**3.12 Semantics changes and the version bump.**
- `WALK_SEMANTICS_VERSION` [src `W/mod.rs:56`] becomes a per-policy report {ordered: 1, ready: 1, epoch: 3}. After launch the legacy lanes are deleted and a single constant 3 remains.
- The bump contract in `W/mod.rs` is rewritten:
  - **Semantics:** whole-inspection commit; the ledger, seal, edge, error and transfer rules; the record schema; CP6.
  - **Performance-only:** the choice among verified containers, scan and dispatch order, enqueue depth, resolution caches, layout, split, allocator, snapshot cadence.
- How epoch semantics differ from v1:
  - No partial streams, replay tokens or Ready credits.
  - The first verified container wins; min-ID applies only in canonical mode.
  - IDs are assigned at merge, in (first parent, ordinal) order.
  - A transfer goes to the smallest containing survivor with to > id, in the same bucket, from a Pending, unprotected, live source.
  - Self-scope and Local resolution are added; `containment_checks` is redefined, and new resolution counters are added.
  - Allowances apply per inspection. Finite containment caps, subdivision and OwnerBatched are refused.
  - Frontiers stop the run; deterministic errors are persisted and stop it too (A2).
- Semantic flags are bound in the digest rather than bumping the version: `frontier-policy`; `zero-certificate` (changes only conditional counters and optional refusals); `residual-anchors` with the C2 vocabulary; a `widening-allowlist` digest; `p-anchors`; and any D1/D6 option that is signed off.

## 4. Input changes (v4 inputs; physics scope frozen)

- **I1, hybrid helpers (conservative).** Owners in L* = L_static ∩ L_obs ∩ L_clean get helpers with the interim shape: rank-bounded, A unbounded. All other owners keep the plan-v3 A_max helpers (`TMP/qcd-feynman-d9d10-input.dcgP73/RESULTS.md`).
  - L_static: a static over-approximation, built from rule shift supports and the `selection.json` route maps, of the owners that cannot reach the 7 guard-sensitive owners.
  - L_obs: the observed L40 (`$S/inputs_lens/an3.py`; identical at gen 3 and gen 7).
  - L_clean: owners with 0 unresolved, exact_gap or invalid pieces in the matching diagnostic, and not an L ancestor of any interim `local_dispatch_frontier` node.
  - R-free helpers only where the diagnostic is clean and a drained control proves it (four-loop: 85-556 frontiers).
  - Its counterfactual (−21..−32% nodes, `$S/inputs_lens/hybrid.rs`) treats absorbed helpers as sinks and is not claimed as a gain.
- **I1b, envelope-sized bounded helpers** (no code change).
  - For U/G owners outside L*, raise A_max and rank to the observed envelope plus a margin. A stays finite, so guard owners stay safe.
  - Evidence: 82% of Apply domains in the v2-only-bounded owners are A-escapes from the plan-v3 box, and the blocked roots escape by rank (13-17 vs 11-13).
  - Gated in W0.6.
- **I2, route witnesses.**
  - Compose each `source_to_representative` with the unimodular owner automorphism that minimizes the traffic-weighted cancellation support (7.56 → 5.56 [M-r: `witness6.tsv`]).
  - All 8,179 transported routes are re-verified at load [src `routed_campaign/prepare.rs`: `symmetry::verify`, `integral_transport::compile`].
- **I3, physics scope frozen.** 183 queries: 116 physics plus 67 helpers (`five_loop_qcd_feynman_entry_class_2026-09-26.md`). Trimming would save at most 1.2%.
- **I4, frontier policy `stop`** at launch (A10; recovery is D7).

Decisions for Valentin, taken in one batched session at the end of W0 with the W0.11 bounds in hand. Unanswered items take the default.
- **D1, what certifies a physics root.**
  - (a) With I1/I1b, roots alias into larger helpers.
  - (b) Piece-level certification: unbounded helpers everywhere, and a root certifies if its cone avoids helper pieces that carry frontiers. The physics lens's counterfactual is −71% nodes. This is only considered if the W0.11 falsifier shows the physics roots certify at piece granularity.
  - Default: (a) for owners that pass the W0.6 gates; (b) no.
- **D2, union coverage.**
  - G2': merged Native anchors from before dispatch plus a retained residual. This is pointwise coverage, an extension of the accepted D-band mechanism.
  - General union-cover aliasing: covers may be pending, so they may later be retired or aliased, and cycles are possible.
  - Measured bound on C-HOT: 71.1% of Apply inspections and 69.3% of Apply native seconds are fully covered by all earlier IDs. The bound for merged natives only comes from W0.7.
  - Default: neither.
- **D3, the C2 vocabulary.** Default: follows D2.
- **D4, widening allowlist or P-anchors.** Default: no.
- **D5, host.**
  - 750 GB is infeasible (651-692 GB available).
  - Needed: an ARC cap, cpuset exclusivity for socket 1, and an off-pool target.
  - Default: a 600 GB working budget, with the guard at min(700 GB RSS, a host MemAvailable floor).
- **D6, symmetry canonicalisation.**
  - Verified all-slot automorphisms give genuine identities I(n) = I(σn).
  - Bounds from the physics lens: −20.5% Apply domains and 1.21x route masks. W0.11 measures the merges actually realized.
  - Default: no.
- **D7, frontier recovery.** Inputs are bound into the checkpoint, so a frontier caused by I1 forces a fresh start. Options:
  - (a) A resume-time input amendment that appends protected helpers or anchors. This is sound because it only adds obligations. But it cannot remove a frontier-bearing helper that is already admitted, so it does not cure an I1 frontier. The cure is a per-owner fallback to plan-v3 A_max helpers and a fresh start.
  - (b) Stop on frontiers during the first N hours, then record and continue. Frontiers and errors stay explicit, and the affected roots stay uncertified.
  - Default: always stop.
- **D8, deliverable scope.** Symbolic closure, or on-demand concrete reduction of the physics integrals with the saved rules. The latter is a change to the delivery contract (`radical_parallel_architecture_2026-09-24.md`). Default: symbolic closure.

## 5. The programme in waves

Controls and pilots (each ≤1 h including restore):

| Id | Definition | Baseline |
|---|---|---|
| C-4L | four-loop FG/BMW/H/X, W6 | 12-35 s each (`TMP/fable51-controls/RESULTS.md`) |
| C-4L-comb-O | combined four-loop `four-all` (all 16 four-loop class owners in Luthe's A4 basis, 508 routes; planner physics class plus 16 rank-12 orthants), deterministic schedule (legacy Ordered, v3 Lockstep with canonical resolution) at W6, W24 and W96; an ADDITION to C-4L. Record-keeping changes: strict record identity. Record-changing engines/levers: byte identity across widths plus natives <= 33,175 (one-sided) or a pre-registered expectation. Always: drained, audit PASS, 0 frontiers, verify-closure gate PASS (32/32 roots) | 30,159 natives, 65,444 records, identical at W6/W24/W96; verify-closure 46d4dd28 PASS 32/32 (`examples/input/four_loop_combined/README.md` §5, branch `fable_5_1-c4l-combined`) |
| C-4L-comb-R | `four-all` in the non-deterministic schedule (legacy Ready, v3 Rolling) at W6, W24 and W96: n >= 13 runs per arm and width, interleaved with legacy 4a17f9c7 Ready in one session on one CPU set, 15-min cap or stop at 60,000 natives. Pre-registered one-sided Fisher test at alpha 0.05 per width. BLOCK on a regression, a frontier/violation, or a non-drain without the known signature (max anchor+1 Apply natives on `0111110010`/`0111111001` >= 1,000); VOID on unequal n/CPUs. Never a binary drain gate (`tools/comb_r_test.py`) | legacy Ready drained 5/5 (W6), 17/18 (W24), 7/13 (W96) (context only; host-confounded) |
| C-5F | five-loop 1,324-tuple finite control (hot owner, A≤11 R≤2 D≥9), W50 | 236-339 s; 1.29M domains |
| C-HOT | single hot-owner physics box | Drained: 5,790,994 natives in 13,416 s (3.7 h), Ordered W6 (`TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec`). Its `audit.json` says FAIL: re-audit helper-aware in W0.2 and use only a PASS baseline. Used in a gate only once epoch at W96 is measured to drain it in ≤45 min; otherwise use C-HOT-sub |
| C-HOT-sub | a hot-owner sub-box that drains in ≤20 min on the legacy engine | chosen in W0.9 |
| P-IMP | gen-7 CP5→CP6 import, 40 min at W100 | same-clone 4a17f9c7 resume (W0.5) |
| P-FRESH-v3 | fresh W100 run on the v2 campaign's plan-v3 inputs, 45-60 min; isolates the engine effect (v3 note S7(a)) | v2 events at matched discovered domains |
| P-FRESH-v4 | fresh W100 run on the candidate v4 inputs, 45-60 min | P-FRESH-v3 at matched discovered domains |

Engine-affecting gates below that name C-4L also require C-4L-comb-O, and C-4L-comb-R whenever the change affects the non-deterministic schedule (amendment of 2026-09-28, fix round of W0 lane c4l; `four-all` is an addition to C-4L, never a replacement; orchestrator decision 5).

I1, I1b, I2, G2' and priority order can be judged only through P-FRESH runs or drained controls, and only at the domain count one hour reaches. Their long-horizon effect is projected and labelled [E].

CPU plan:
- Socket 1 (CPUs 128-227) runs one job of ≥W48 at a time.
  - W0 order: 0.5 (gen-7 restore ≈6 min + 25 min), then the 0.3 sweeps at K = 48/96, then the W50 controls of 0.8.
  - Later: P-IMP, the two P-FRESH arms, W5.2, and the launch.
- Controls up to W24 run on CPUs 232-255. W50 controls run on socket 1 when it is free; otherwise they run on CPUs 28-77, with 4a17f9c7 re-baselined on the same CPUs.
- Every A/B runs in the same session on the same CPUs, with ≥2 interleaved repeats, recording foreign load and `schedstat` run delay.

**W0: intel, oracles, harnesses, work-volume bounds.** No engine change is merged. About 20-28 agent-days, about 1-1.5 wall weeks in parallel [E].

- **0.0 Governance** (main tree): a dated `GOAL.md` directive and a decision-log entry in `FABLE_5_1_five_loop_vacuum_plan.md` §6, recording this plan as the goal.
- **0.1 Persist the lens tools** (`fable_5_1-v3-intel`): `indexscan`, `rtool` (including `cover`/`union`), `perfskeptic*`, `meas/fits.py`, `inputs_lens/*` and `membench` go under `tools/research/`, with the outputs they reproduce.
- **0.2 Oracles** (`fable_5_1-v3-oracle`):
  - Audit: `--require-closure`; per-record `accepted_events == stats.events`; alias containment; frontier/error/refusal parity; helper vs physics certification.
  - `walk verify-closure` (edge-based F10), using the exact reducer with every lever off. Roots come from the `queries.json` digest. Small boxes get a brute-force lattice membership check.
  - Mutations that must FAIL: dropped edge, retargeted alias, dropped frontier record, seal with a frontier, seal with an error, injected false hit.
  - Calibrate PASS on C-4L, C-5F and C-HOT.
- **0.3 Rust re-inspection harness** for Apply and Route (`fable_5_1-v3-harness`):
  - Restore a gen-6/7 clone and inspect ~10^4 stratified Pending IDs with a counting sink, at K = 1/8/24/48/96 on socket 1.
  - Compare first-touch vs interleaved memory vs 4 node-bound processes, and glibc vs mimalloc (with malloc override). Record IPC, bandwidth and the share of C allocations.
  - Record futex and lock wait (Symbolica global state) at K = 96.
  - Take a frame-pointer inclusive profile of heavy heads (Q7).
  - Record a cancellation-latency histogram. The harness also supplies real successor streams to 0.4 and to F10.
- **0.4 Offline gen-7 index replay** on real streams (`fable_5_1-v3-intel`):
  - Layouts: today's, SoA u8 (pattern + lexicographic), SoA in ID order.
  - Policies: first-found vs min-ID; MRU k = 1/4/16/64; Local; self.
  - Units: CPU ns per candidate and per bit-test; candidates per hit, per miss and per reverse check.
  - Thinning to 25/50/100% for the exponents; 1/48/90 threads; stale-miss rate against lag.
- **0.5 Production baseline M1** (`fable_5_1-v3-intel`):
  - Block-clone gen 7 and resume a frame-pointer build of 4a17f9c7 at W100 (the binding requires it) for 25 min on 100 exclusive CPUs.
  - `perf record` (short dwarf window) and `perf stat`; RSS per domain above 74M.
- **0.6 Input intel** (`fable_5_1-v3-inputs`):
  - L_static; the ancestors of interim frontiers.
  - Matching-only diagnostic on the candidate helper sets (~150-160 s each).
  - I1b: a 10-min matching diagnostic on census-sized anchors, plus a 60-min closure probe with anchors only.
  - Witness rewrite and load verification.
- **0.7 Work-volume census on gen 7** (`fable_5_1-v3-intel`):
  - Union coverage of a pending sample by merged natives (this is the G2' bound).
  - Residual piece counts with D-only cuts vs with A/R cuts.
  - Per-owner cost exponents; pending vs committed envelope in (A, R, P, D).
  - Global-potential check (`five_loop_auxiliary_scope_2026-09-25.md`).
  - Composition of the 20.57M native-pending.
  - Point-space saturation per owner at gen 3/6/7 (`rtool union`).
  - Guard/coefficient factor census (Q3).
- **0.8 Legacy knobs** on C-5F and C-HOT-sub (`fable_5_1-v3-knobs`):
  - A dispatch-order env knob in `W/delegation/ledger.rs` `reserve_available`: FIFO / support+volume / closure boost / depth-first.
  - Build A/B: GLIBC_TUNABLES, mimalloc, `[profile.campaign]`, znver4.
- **0.9 Falsifiers**, throwaway code (`fable_5_1-v3-widen`):
  - G1 widening at the Apply Successor arm only, on C-4L, C-5F and C-HOT-sub. Pre-registered prediction: it fails on the hot owners.
  - Adaptive dense/sparse level cells as an offline oracle on the drained C-HOT closure. The algorithmic lens gives 1.25x distinct Apply points for this scheme vs 19.5x today.
- **0.10 Host request** (user/admin):
  - cpuset exclusivity for socket 1, an ARC cap and a swap policy;
  - an off-pool target: a device or host outside `zroot`;
  - `zpool status -v` run as root, to confirm that the 2 data errors are not in the inputs or binaries.
- **0.11 Work-volume lane**, offline and in parallel with W1-W2 (`fable_5_1-v3-wv`):
  - G2' and union-cover bounds from 0.7; dense cells from 0.9; I1b from 0.6.
  - Symmetry: count of realized merges (physics lens P4), for D6.
  - Piece-level falsifier: a ≤1 h four-loop run with unrestricted helpers (1.6-6.3 s with 85-556 frontiers per `four_loop_helper_bounds_2026-09-25.md`), then an offline check of whether the physics roots certify at piece granularity.
  - Hot-owner closure import: overlap of the pilot's ~8.0M domains with v2's hot-owner domains (≤1 h).
  - Output: a projected, labelled work factor per lever, for the D-session.

W0 gates [thresholds E]:
- **0.2:** PASS on all current drained outputs, and FAIL on every mutation.
- **0.3:** CPU per native at 96 threads is ≤1.3x the single-thread value. If not, NUMA replication and the allocator change move before the W2 exit, and every throughput band is rescaled.
- **0.4:**
  - Adopt the SoA kernel if it needs ≥4x less CPU per tested candidate than today's layout at 74M. Choose the layout by candidates per hit and per miss.
  - If the projected admission share at N = 1G exceeds 25% of worker CPU, W3.2 moves into W2.
  - If the pipeline tiers resolve <50% of requests (request-weighted), 3.2 is redesigned.
- **0.5:** the baseline is recorded, and ≥80% of coordinator time is attributed to named callers.
- **0.6:** I1 ships only for L* owners. I1b ships only for owners whose diagnostic is clean and whose 60-min probe shows 0 frontiers and no domain in a guard mask.
- **0.7:** W4.2 proceeds if ≥30% of Apply CPU sits in domains whose residual against merged natives is ≤10% of their points in ≤8 pieces.
- **0.8:**
  - Adopt an order class if mistakes per native fall by ≥10%, or roots certified per native CPU-hour rise by ≥20%, with peak pending within +10%.
  - Keep a build change if inspector CPU per native falls by ≥5% with strict Ordered identity.
- **0.9:** go only if C-4L closes with 0 frontiers and C-5F and C-HOT-sub drain at ≤0.5x the inspector-seconds. Otherwise record the rejection.
- **End of W0:** the D-session (§4). A signed-off lever projected at ≥2x less work becomes launch-blocking.

**W1: legacy-lane primitives and inputs, in parallel with W2** (about 12-16 agent-days [E]).
- **1.1 Kernel** (`fable_5_1-v3-kernel`, = v3 S1): SoA compare kernel, immutable per-ID summaries, A5 encoding, release WIDE check, `partition_point` skip.
  - Gate: strict Ordered identity (records and `containment_checks`) vs 4a17f9c7 on C-4L, C-4L-comb-O and C-5F.
  - Gate: ≥1e8 forward and ≥1e8 reverse differential pairs from gen 7, including wide, empty and infinite cases.
  - Gate: ≥4x less CPU per candidate in the 0.4 replay.
- **1.2 Native class A** (`fable_5_1-v3-native`): N1, N2 (geometry only), N3.
  - Gate: identical successor, frontier and problem multisets on C-4L, C-4L-comb-O, C-5F and ≥10^6 sampled term visits.
  - Gate: records identical except for the named differences: conditional-classification counters, `attempted_optional_coefficient_refusals` together with the OptionalCoefficientRefusal events, and timing.
  - Gate: inspector CPU per native ≥20% lower in the 0.3 harness, at K = 1 and K = 96.
- **1.3 Inputs v4** (`fable_5_1-v3-inputs`): I1, I1b and I2, shipped as `examples/input/five_loop_qcd_feynman_d9d10/queries.json` v4 plus a rewritten selection.
  - Gate: the matching diagnostic is clean.
  - Gate: a 1-h walk on L* owners only, on the legacy engine, shows 0 frontiers and no domain in a guard mask.
  - Gate: the witness A/B on C-5F gives ≥20% fewer Route→Route edges per route native, with audit PASS.
  - Before the inputs are frozen, a time-boxed 1-h L*-only walk on the epoch engine (after 2.5) must show 0 frontiers, no domain in a guard mask, and partial closure metrics (roots certified, point saturation).
  - A drain is not expected: the region reachable from the L owners is ~24.8M nodes, and only 7 of the 40 L roots closed in v2's 18.8 h.
- **1.4 Operations** (`fable_5_1-v3-ops`):
  - A10 frontier stop.
  - A host-aware RAM guard in `shared_owner_campaign.py`.
  - The off-pool copy, once W0.10 names a target.
  - Harden the two timing-flaky tests: `orphan_child_retains_campaign_lock_until_exit` and `ready_late_native_fault_after_cancellation_disallows_pause` (`FABLE_HANDOFF.md` §8.6).
  - Review #17: license-gated tests print a skip marker, and epoch soundness tests fail without `SYMBOLICA_LICENSE`.
  - CP5 review follow-ups are not applied (§3.6).

**W2: the epoch engine.** v3 S2-S6 plus amendments. About 45-55 agent-days including reviews, importer and tooling; critical path about 5-6 wall weeks [E].
- **2.0 Protocol note**, written before any code. It answers every hazard in v3 §9 and fixes A1-A10, the A2 error classes and the §3.4 enqueue depth. It states the termination condition: Pending = Reserved = ∅ and no unmerged result.
- **2.1 S2 skeleton, lockstep** (`fable_5_1-v3-epoch`).
  - Gate: audit, closure and F10 PASS on C-4L, C-4L-comb-O (Lockstep, canonical) and C-5F.
  - Gate: byte-identical across W6, W12 and W24 (C-4L-comb-O also at W96).
- **2.2 S3 CP6 and stops** (`fable_5_1-v3-cp6`).
  - Gate: a pause at epoch k is byte-identical.
  - Gate: these all PASS: `kill -9` resume; RAM-guard exit 4 and resume; frontier stop; deterministic-error stop, then resume without re-dispatch; double restore with D-bands; re-split resume.
  - Gate: injected corruption is refused.
  - Gate: each review requirement in §3.6 has a passing test.
- **2.3 S4 inspector resolution**, with the kernel from 1.1 and the layout from 0.4.
  - Gate: verify-all shows 0 mismatches.
  - Gate: canonical mode is identical across worker counts.
  - Gate: first-found vs canonical keeps natives within ±5% and edges per domain within ±10%, with a closure trajectory no worse.
- **2.4 S5 parallel merge and diet** (`fable_5_1-v3-merge`): typed records, bulk edges, `ledger6`, F14, F15, and block-parallel P2 reverse sets.
  - Gate: serial and parallel merges are byte-identical.
  - Gate: the frozen refresh equals the forced scan.
- **2.5 S6 rolling schedule.**
  - Gate: 3 pause points per control PASS.
  - Gate: natives needed to drain C-5F and C-HOT-sub at W96 rolling are within +10% of W96 lockstep in canonical mode. Lockstep is byte-identical across worker counts, and C-HOT took 3.7 h at W6, so it is not used here.
- **2.6 Importer, deliverable and tooling** (`fable_5_1-v3-import`).
  - CP5→CP6 import rules:
    - Native → Native;
    - published Delegate → Alias (re-verified);
    - unpublished Delegate → Pending;
    - Started, Reserved and Unreserved → Pending, dropping their partial edges;
    - the anchor map is imported.
  - Records are not imported, and the import is exempt from audit: it is used for performance measurement only. It is built once and block-cloned for each gate.
  - Also in 2.6: the §3.7 deliverable, and the Rust streaming audit and verify-closure with runtime per 10^8 records measured.
  - Ported: `compare_walk_records.py`, `campaign_monitor.py`, and the `--upgrade-executable` probe in `production_saved_owner_campaign.py`.
- **W2 exit gate, P-IMP.** Restore within 10 min, then run 40 min at W100. Required:
  - ≥60% of inspector threads in native work;
  - merge duty ≤50%;
  - lookup CPU per native ≤1x native CPU;
  - ≥5x the 0.5 baseline in obligations discharged per hour;
  - RSS ≤0.6 KB/domain;
  - verify sample PASS.

  Below 3x: stop adding features and fix the measured limiter (lookup, native scaling or merge).

**W3: scale, order and memory** (after 2.3; about 15-20 agent-days [E]).
- **3.1 Priority dispatch and enqueue depth** (`fable_5_1-v3-order`).
  - Gate: A/B on P-IMP, C-5F and C-HOT-sub (C-HOT only under its rule), measuring roots certified per native CPU-hour, M-mist and natives to drain.
  - The enqueue depth is chosen by M-mist and transfers per native.
  - Peak pending and RSS stay within +10%.
- **3.2 Miss path.** Only if the 0.4 projection requires it. Gated by the exponent on the replay and on P-IMP.
- **3.3 Memory tiering and placement** (`fable_5_1-v3-ram`).
  - Gate: RSS ≤0.45 KB/domain on P-IMP.
  - Gate: a synthetic save/restore of 2-3e8 domains within 10 min.
  - Gate: save stall ≤2% of wall.
  - Gate: at the guard, the RSS local to socket 1 fits within the local node capacity.
- **3.4 Refresher with per-root cones.** Gate: ≤15 s at 1.2G edges; the cost at 10G edges is projected.
- **3.5 NUMA and Route batching.** Owner programs (~1.28 GB) are replicated or interleaved per node; layers are interleaved over nodes 4-7; Route IDs are micro-batched.

**W4: collapsing work volume.** Covers the levers signed off in the D-session; launch-blocking when projected at ≥2x. About 25-35 agent-days [E]. Gates use C-HOT-sub unless C-HOT meets its rule.
- **4.1 C2 vocabulary.** Property tests on ≥10^6 random boxes against point enumeration, covering containment, emptiness, parity, sign-cell translation and route transport.
- **4.2 G2' residual anchors**, plus general union cover if D2 signs it off.
  - Gate: drains at ≤0.5x inspector-seconds with 0 frontiers.
  - Gate: verify-closure is extended so that the anchor scopes plus the residual cover each Q exactly.
- **4.3 N4 cover-first.**
  - Gate: identical frontier and problem multisets.
  - Gate: ≥20% less inspector CPU per native.
- **4.4 Conditional levers.** Each has its own ≤1 h gate on C-4L, C-4L-comb-O (plus C-4L-comb-R if it affects scheduling), C-5F and C-HOT-sub.
  - Widening allowlist, if 0.9 passes.
  - Dense cells, if the 0.9 oracle and D2 allow.
  - P-anchors, if 0.7 shows escape shells dominating.
  - Head parallelism, if the tail sets stop latency or makespan.
  - Factor atlas, if Q3 shows mostly linear factors.
  - D1(b) and D6, if signed off.

**W5: iterate until dry, rehearse, launch** (5-8 agent-days plus the loop).
- **5.1 Loop.** Run P-IMP and both P-FRESH arms with a worker profile, fix the largest non-native cost, and repeat.
- **5.2 Rehearsal.** A 60-min P-FRESH-v4 on the frozen binary and inputs. It exercises:
  - pause and resume;
  - a performance-only binary swap;
  - a re-split;
  - a RAM-guard stop;
  - `kill -9`;
  - a frontier-stop drill on a control;
  - the off-pool copy.

  The receipt lists every restore's time. If restores exceed 20 min in total, split the rehearsal into two ≤1 h runs.
- **5.3 Freeze.** A receipt with the commit, semantics 3, input digests, split and D-decisions. Launch fresh on socket 1 with the D5 guard and 2-4 h checkpoints.

Expected gains [all E; not ETAs; they multiply only up to the RAM wall]:
- **W1:**
  - Kernel: 3-10x less CPU per candidate.
  - N1+N2+N3: −20..−45% inspector CPU per native.
  - I1: at most its unclaimed −21..−32% node counterfactual over L* ⊆ L40. The 43.5% projection assumes all 60 interim helpers unbounded and does not apply here.
  - I2: −20..−30% route domains (modelled).
- **W2:** 5-20x obligations per hour at matched D, with a floor of 3-5x (v3 §8).
- **W3:** earlier certification for roots whose blocker chains span many queue passes. The 36.9% of inspections later contained is only a loose upper bound on savings. About 0.45 KB/domain.
- **W4:** on the pilot, skipping only the fully union-covered Apply inspections saves at most 69.3% of Apply native seconds (≤3.3x). Getting nearer 10x also requires cutting the partials with ≤10% new points down to their residual. Both counts use coverage by all earlier IDs, so the G2' bound (merged natives at dispatch only) must come from W0.7.
- **Total:** about 125-165 agent-days, about 8-10 wall weeks with 4-5 parallel lanes.

## 6. Soundness and validation strategy

Invariants, from the engine lens and v3 §5:
- **S1:** every alias is a verified containment in the same (phase, owner), with its edge.
- **S2:** each ID is published exactly once: as Native, or as an Alias to a strictly newer containing survivor.
- **S3:** Reserved never transfers.
- **S4:** a node seals only after its complete, deduplicated edge set is appended, and only as (Native ∧ 0 frontiers ∧ no error) ∨ Alias.
- **S5:** saves are consistent cuts at merge boundaries. Unmerged work is re-inspected, which is idempotent.
- **S6:** frontiers and deterministic errors are explicit and persisted, are never sealed, and stop the run. Transient failures are re-inspected (A2).
- **S7 (G2'):** a residual job's anchors are Native and merged before its dispatch snapshot, and the anchor scopes plus the residual cover Q exactly.

Checks:
1. **Chokepoint.** In production, `verify` runs for every positive, followed by a second check against the canonical store, and the merge re-verifies a sample of ≥1/64. Controls verify every hit.
2. **Kernels.** Differential tests forward and reverse (≥1e8 gen-7 pairs), property tests against point enumeration, and a brute-force lattice check in `verify-closure`.
3. **Oracles.**
   - The W=1 serial path.
   - Canonical-mode byte identity across W6/W12/W24 and across pause points.
   - The frozen 4a17f9c7 on drained controls, comparing closed roots and frontiers only where both runs drain with 0 frontiers.
   - Legacy Ordered/Ready and CP5 stay read-only until after launch.
4. **verify-closure** (edge-based; reference reducer with every lever off).
   - Full coverage on C-4L, C-4L-comb-O, C-5F and C-HOT-sub (C-HOT under its rule).
   - In production: ≥10^4 newly sealed natives per generation, on spare cores.
   - Two claim levels:
     - *engine-certified*: invariants, chokepoint, canary and validators;
     - *independently verified*: every native in the root's cone is re-inspected.
   - Root 8's cone is 92% of the gen-6 graph, so independently verifying the large roots costs about as much native CPU as the campaign itself. It is a budgeted post-campaign job, run from the final CP6 with the Rust streaming verifier, and it is required before any published closure claim.
5. **Restore validators.**
   - Alias containment, checked in parallel; F8.
   - Out-degree against each record's edge count.
   - Enqueue depth (F17); the anchor and merge-epoch map.
   - Digests on every referenced segment, including the off-pool copy.
6. **Stop paths.** Pause, `kill -9`, RAM-guard exit 4, frontier stop, deterministic-error stop and re-split, each drilled on controls (W2.2) and once at scale (W5.2). Restore at scale takes ≤10 min on P-IMP.
7. **Lever gates.**
   - N1 and N2: identical successor, frontier and problem multisets, with the named record differences.
   - I1, I1b and I2: early frontier checks plus drained audits.
   - W4 levers: drained C-HOT-sub/C-5F plus verify-closure.
8. **Test discipline.** Epoch soundness tests run with `SYMBOLICA_LICENSE` and fail, rather than skip, without it. Review #17 found five license-gated tests that pass vacuously today.
9. **Lost by design.** Byte identity outside canonical mode, the legacy `containment_checks` trajectory, and cross-binary replay determinism.

## 7. Measurement protocol and stopping criterion

Metrics (granularity-invariant):
- **M-drain:** wall time and inspector CPU-seconds to drain C-4L, C-4L-comb-O, C-5F, C-HOT-sub and C-HOT; for C-4L-comb-R the drain fraction per width with its test.
- **M-obl:** obligations discharged per hour (native + alias), split into Apply and Route, with CPU per class.
- **M-cert:** helper roots and physics queries certified, per native CPU-hour and against wall time.
- **M-util:** per-thread CPU split into native, resolve, merge and idle, plus run delay.
- **M-adm:** CPU ns and candidates per request by class (hit by source, miss, reverse), with the fitted exponent against bucket live size.
- **M-mist:** inspections later contained; transfers per native.
- **M-pts:** distinct lattice points per owner in the union of admitted domains, and new points per native (saturation). Pending counts measure fragments, not unexplored closure: only 3.4% of admitted Apply points were new in the pilot.
- **M-ram:** RSS per discovered domain, socket-local share, save peak, and host MemAvailable.

Completions per hour is never the sole decision metric: Route carries 75-78% of natives but <1% of native CPU.

Protocol:
- Every pilot is at most 1 h including restore.
- A/B runs are in the same session on the same CPUs, with ≥2 interleaved repeats. A run is void if foreign load on its CPUs exceeds 10% (`/proc/stat`, `schedstat`).
- Receipts go in `TMP/<pilot>/RESULTS.md`, labelled [M] or [E]. Each finished wave gets a research note under `docs/research/` and a decision-log entry.
- Per-commit gates (plan §G), all run with the license set [`FABLE_HANDOFF.md` §8.4 for baselines]:
  - `cargo fmt --all -- --check`;
  - the release `rustred-app` lib suite (baseline 777/0/6 at 66ede259) and `cli_routed_campaign`;
  - the Python suite (225 OK, 1 opt-in skip);
  - new tests with each package, and a push after each validated milestone.

**Launch** of the ultimate campaign: W100 on socket 1 (about 90 inspectors, 6-8 merge helpers, 1 coordinator and 1 refresher [E, tuned in W5]), with the D5 guard. It requires all of (A)-(G):
- **(A) Soundness battery green:**
  - §6 checks 1-8 pass, and the mutation tests FAIL as intended.
  - 0 mismatches in verify-all on controls and in the ≥1/64 canary, over ≥1e9 verified hits accumulated across W2-W5.
- **(B) Inspection-bound at scale**, on P-IMP and both P-FRESH arms:
  - ≥70% of inspector-thread CPU in native work;
  - merge duty ≤50%;
  - lookup CPU ≤0.5x native CPU;
  - projected admission share at N = 1G ≤25%, from the exponent.
- **(C) Memory:**
  - RSS ≤0.5 KB/domain at 74M+ domains;
  - the save peak stays within the guard;
  - time-to-guard projected at the measured rate and written into the receipt (§3.8 derivation);
  - the at-guard action decided in advance: save-stop, then a scope decision by Valentin.
- **(D) Out of ideas:**
  - two consecutive W5 iterations each change M-obl, or inspector-seconds per native, by <10%;
  - no non-native symbol group above 15% of worker CPU;
  - no open lever projected at ≥15% that fits in ≤3 days;
  - the remaining levers need semantics decisions that have not been signed off.
- **(E) Rehearsal:** W5.2 passes with 0 frontiers at 10, 30 and 45 min.
- **(F) Host:** cpuset exclusivity, the ARC cap and a named off-pool target are confirmed (none exists today), and `zpool status -v` has been checked.
- **(G) Work volume:** every signed-off lever projected at ≥2x is implemented and gated, and the D-decisions are in the receipt.

**In-flight monitoring and escalation** (carried from `FABLE_5_1_five_loop_vacuum_plan.md` §3.F, adapted):
- Frontiers and errors stay at 0; otherwise the A2/A10 stop applies, under the D7 policy.
- Max scheduled rank ≤ helper rank + 7. If exceeded: cooperative stop and scope review.
- Pending growth per completion above v2's at matched discovered domains for 3 h: cooperative stop and scope review.
- Certified roots never decrease. If no new root is certified over two checkpoint intervals while pending grows: review order and scope.
- Watch the M-pts saturation trend. Keep merge duty ≤50% and save duty <10%. Keep time-to-guard above 2 checkpoint intervals; otherwise plan the save-stop.
- "Better odds" is declared only on certified roots, falling pending per completion, point saturation and a flat rank cap, never on completions per hour.
- A 10x target over v2 is aspirational, not a gate.

## 8. Risk register

| # | Risk | Detection | Mitigation / fallback |
|---|---|---|---|
| R1 | Termination not established; pending may keep growing | pending/completion trend; M-pts; potential check (0.7) | work-volume lane from W0; RAM-guard save-stop; scope decision by Valentin (D8). No ETA |
| R2 | RAM wall in 10-250 h [E, §3.8]; host offers 651-692 GB, socket 1 has 564-594 GB local | M-ram; time-to-guard projection | W3.3 tiering and placement; ARC cap; guard on host MemAvailable; u32 hard stop |
| R3 | Native scaling to ~90 threads is unmeasured (never beyond ~6.5); Symbolica global state or C malloc may serialize | 0.3 harness (futex wait, C-allocation share) | NUMA replication, mimalloc override, fewer inspectors; rescale gains |
| R4 | Miss and reverse cost keeps growing (N^0.52-0.66) | 0.4 exponents; M-adm on P-IMP | block-parallel P2; W3.2; `--epoch-resolve merge` |
| R5 | An engine bug causes false closure | verify-all, canary, verify-closure, validators | single mutator; F1-F20 and A1-A10; protocol note; legacy lane kept |
| R6 | First-found picks young containers and slows certification | closure-trajectory gate in 2.3 | hints prefer sealed containers; closure boost (3.4) |
| R7 | Priority order raises peak pending or RAM, or starves roots | 3.1 A/B | age bound; revert to support+volume or FIFO |
| R8 | I1/I1b miss a transition into a guard owner and produce frontiers | frontier stop; early checks | per-owner fallback to plan-v3 A_max helpers (fresh start; D7) |
| R9 | G2' fragments residual scopes into many pieces | 0.7 piece census; M-drain | D-only cuts first; minimum piece volume; per-owner allowlist |
| R10 | N1 or N4 hides a Problem or a frontier | multiset-identity gates | exact path on any doubt; flags bound in the digest |
| R11 | Heavy heads stretch stop latency and resume re-work | cancellation histogram | 60 s grace; head parallelism (4.4) if needed |
| R12 | Measurement noise on a shared host (~7% Ready spread; ~50 foreign CPUs) | repeats; foreign-load logs | cpuset exclusivity; count-based metrics |
| R13 | Storage: a single vdev with 2 data errors and no off-pool target | `zpool status -v` | W0.10 admin request; launch gate (F) |
| R14 | Effort overrun (reviewers call 23-40 d optimistic) | wave gates | parallel lanes; W2 abandon rule; W4 optional only for levers projected <2x or not signed off |
| R15 | Two engines in one tree diverge | review | legacy lanes frozen read-only; deleted after launch |
| R16 | The import harness misrepresents fresh runs (edges derived under min-ID) | P-FRESH vs P-IMP | P-IMP measures the engine only; launch gates also use both P-FRESH arms |
| R17 | A deep enqueue turns transfers into native re-inspections | M-mist; transfers per native | fixed small enqueue depth (§3.4), gated in 3.1 |
| R18 | A deterministic error, or a license-skipped test, hides a defect | error-stop drill; test discipline (§6.8) | A2 error split; tests fail without the license |
| R19 | Deliverable and audit do not scale (≈620 GB of JSON; Python audit; cone re-inspection costs about the campaign's CPU) | W2.6 runtime per 10^8 records | binary deliverable; Rust streaming audit; budgeted post-campaign verification |

## 9. Deferred and rejected ideas

| Idea | Status | Reason (source) |
|---|---|---|
| Lock-free CAS engine (CWE), bucket actors | deferred | wider concurrency surface; false-closure paths found (v3 §9; CWE votes); epoch's P3 ceiling of about 65-200k natives/s [E] does not bind |
| Cells First / G1 hull cells as the core | not adopted as core | the judged submission was empty; the refusal rests on the per-owner regression (points^0.74-0.90, superlinear for hot owners) and on guard frontiers on unbounded cells; tested by the W0.9 falsifier |
| General union-cover aliasing | decision D2 | G2' is its restriction to merged Native anchors; the general form lets pending covers be retired or aliased and can form cycles; bound 71.1% / 69.3% on C-HOT (all earlier IDs) |
| Symmetry canonicalisation | decision D6 | genuine identities I(n) = I(σn), but a new certification rule; bounds −20.5% Apply domains, 1.21x route masks; realized merges measured in W0.11 |
| Piece-level certification | decision D1(b) | −71% node counterfactual; W0.11 falsifier first |
| Min-ID in production; replay tokens | rejected | they serve reproducibility only; canonical mode keeps an oracle |
| Relief items 1a/1b/2/3a/3b, left-right replica, B5 pipelining | superseded | they optimize a barrier that epoch removes; ceiling below about 6% at scale |
| CP5 checkpoint-review follow-ups applied to CP5 | superseded | CP5 is deleted after launch; carried over as CP6 requirements (§3.6) |
| Σ-lower block clustering; 64-wide ID-ordered blocks | rejected | measured worse on gen 7 (`$S/perfskeptic_cwe/scan2_7.out`, `scan3_7.out`) |
| Lossy or bounded reverse retirement | rejected | 26.1M transfers vs 27.5M natives |
| Independent owner shards; exact entry shards | rejected | 2x slower; cross-owner dedup is lost |
| Inline Route as specified in MRC 3.3 | rejected | saves no IDs or RAM; Route is <1% of CPU; micro-batching instead |
| Physical subdivision (`W/physical_parts.rs`) | refused in epoch | a sequential two-part cut; conflicts with whole-inspection commit |
| Prepared RHS permutation | rejected | 4 of 3,070 samples (0.13%) (`finite_closure_architecture_review_2026-09-23.md`) |
| Inner parallelism for heavy heads | conditional (4.4) | Amdahl ≤2.07x at k=4 (`five_loop_slow_inspection_parallelism_2026-09-24.md`) |
| Factor atlas for guards | conditional | needs the Q3 census (W0.7); exact root proofs required |
| Hot-owner closure import | deferred until W0.11 | its "12 d" is the rule-regeneration estimate; the overlap of the pilot's ~8.0M domains with v2 is measured first (≤1 h) |
| Rule regeneration for the hot owner | deferred | 12 d; revisit if the hot owner stays dominant after W4 |
| Zone (difference-constraint) vocabulary | research lane | 20 d; covers only unit-slope diagonals |
| Scope trimming (factorized, non-entry, D=9) | rejected | ≤1.2% of the explored graph (`$S/inputs_lens/hybrid.rs`) |
| Blanket unbounded helpers | rejected | interim: 1,299 frontiers; no gain in discovered domains per completion; the piece-level variant is D1(b) |
| Physics roots without helpers | rejected | four-loop finite envelope without anchors: FG stopped at 1.35M natives with 764,122 pending, vs 12.9 s with rank-only anchors (`four_loop_saved_cover_control_2026-09-24.md`) |
| On-demand concrete reduction instead of symbolic closure | decision D8 | a change to the delivery contract (`radical_parallel_architecture_2026-09-24.md`) |
| Huge pages | deferred | unavailable without admin compaction |
| Telemetry diet alone | subsumed | 0.95% + 0.76% of v2 coordinator wall; delivered by typed records |

## 10. Revision record (completeness critique, 2026-09-27)

All 32 required revisions were checked against the sources named above and applied; none was rejected. Qualifications:
- **#3:** review items #1-#6 were already applied to CP5 (`FABLE_HANDOFF.md` §7 table). They are carried as CP6 invariants together with the open items.
- **#5:** confirmed in `W/checkpoint.rs` `binding()`; A7 is now a change to implement.
- **#6:** confirmed from `$S/pilot_cover2.txt` (fully covered: 7,095 of 10,232 Apply native seconds) and `$S/rtool/src/main.rs` (the cover counts all earlier IDs).
- **#9:** `free -g` showed 651 GB available; the critique's 661 GB was an earlier sample. Both put 750 GB out of reach.
- **#11:** option (a) is sound but does not cure an I1 frontier; stated in D7.
- **#12:** confirmed: 158,552 refusals in v2 `result.json`, emitted only on a preflight refusal.
- **#15:** root 8's 92% re-derived from the receipt rows (63.05M of 68.87M).
- **#16:** confirmed; `/tmp` is on `zroot` too.
- **#20:** "7 of 40" re-derived from `$S/inputs_lens/L.txt` and the receipt's closed roots. The 24.8M reach is the critique's figure and was not re-derived.
- **#23-#27:** the lens figures (A-escape margins, 1.25x, −71%, −20.5%, 1.21x) are cited from the lenses, not re-derived here.

## 11. ERRATA (2026-09-28)

Added by the integrator on 2026-09-28, when the W0/W1 lane branches were merged into `fable_5_1`. The body above is
kept as written; where it conflicts with an entry below, the entry wins. Sources: `FABLE_5_1_CRITIQUE.md` §5 (numeric
errata, re-derived from receipts), `HANDOFF_opus_5_5.md` §0.1 item 4 (audit errata) and items 11-12 (owner answers).
Later W0 corrections and the graded W0 gates: `docs/research/fable51_w0_results_2026-09-27.md` (§1, §5); binding
orchestrator decisions: `TMP/progress/orchestrator_decisions.md`.

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

### 11.5 Where this plan's body is affected

- §0 and §3.8, risk R2 ("5-20x", "≈10-250 h", 0.3 KB/domain): read with §11.1 items 3 and 9; the RAM-wall bracket
  and the time-to-guard are restated in `docs/research/fable51_w0_results_2026-09-27.md` §2.3 [E].
- §3.8 and §4 D5 ("700 GB guard"; "Needed: an ARC cap, cpuset exclusivity for socket 1, and an off-pool target";
  default "min(700 GB RSS, a host MemAvailable floor)"): settled by the owner (§11.3): no ARC cap, no cpuset (socket 1
  shared), no off-pool target (zroot errors accepted), a 600 GB cap and a 50 GB host MemAvailable floor.
- §4 D1/D6 defaults: D1(b) no; D6 off at launch and deferred, not settled "no" (§11.4).
- §3.11 and §6 S7 ("each anchor is Native"): amended by the owner (§11.4); an anchor may also be a validated merged
  G2' residual record, resolved in merge order.
- §5 control table and gates naming "C-HOT-sub": name the box (r1a12 or s2/r2a12; §11.1 item 6). C-HOT is audit-only.
- §5 W2 exit ("5-20x", floor 3-5x): the governing gate is the owner's >= 1.5x against the legacy + SoA-kernel
  comparator (§11.3), scored against runC after the `AggregateIndex::retire` swap fix (orchestrator decision 11).
- §4 D2 ("neither" default): settled as dispatch-time G2' in union form (§11.4).
