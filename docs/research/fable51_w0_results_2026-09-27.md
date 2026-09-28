# W0 results, work-volume synthesis and D-session memo (fable_5_1 next push)

W0 gatekeeper memo, written 2026-09-28 ~11:30 UTC (the file name keeps the W0 date). Read-only on code; committed by the
integrator (see the note below). Owner: Valentin Hirschi. Labels: **[M]** measured (receipt or run dir cited), **[M-off]** measured offline on
saved trajectories, **[E]** estimate or interpretation. No ETA and no closure claim; `family_closure_claim` stays false.
Sources: `TMP/progress/*.report.json` (census, knobs, intel, baseline, inputs, c4l, wv, harness, oracle, g2falsify,
routecensus, kernel, ops, comparator) with their verify/fix/review notes; `FABLE_5_1_CRITIQUE.md` §1-5, §7; handoff §0.1
items 1-12; `TMP/progress/orchestrator_decisions.md` 1-11; W2.0 protocol note rev 2
(`TMP/w2/fable51_w2_epoch_protocol_2026-09-28.md`, now committed as
`docs/research/fable51_w2_epoch_protocol_2026-09-28.md`). The audit's errata are applied throughout (§5).

> Integrator note (2026-09-28): committed unchanged apart from this note and the two file-status phrases above.
> The merge state in §0 item 1 and the "fix before the merge" items of gate 0.2 are those of ~11:20Z. The oracle
> gate-helper fixes of §4.1 (1) landed with the oracle merge; the merge list, gate counts and the merged binary are in
> the `fable_5_1` history and in `TMP/progress/integrator.md`.

## 0. Bottom line

1. W0 is complete in substance, not in form: **no lane branch is merged into `fable_5_1` (998d1b06) and none has an
   upstream** [M, git 11:20Z]. Under directive 0.1.6 every gate that rests on the oracle stays PROVISIONAL until
   `fable_5_1-v3-oracle` (f1895f64) is merged and the merged verifier re-runs.
2. Gates: 0.2 PASS (pending merge); 0.3 FAIL for today's shared design, remedy provisional; 0.4 (a) PASS provisional,
   (b) INCOMPLETE, (c) PASS; 0.5 PASS; 0.6 I1 PROVISIONAL, I1b dropped, I2 as built fails on total work; 0.7 PASS;
   0.8 orders FAIL, build PASS in instructions; 0.9 G1 FAIL (rejected), G2' union FAIL narrowly as registered.
3. One lever measured live changes work volume: **G2' in union form** (scheduled domains 0.79-0.80x, peak pending
   0.82-0.86x, useful native work 0.42-0.53x) on two drained five-loop controls whose cost mix inverts gen 7. D6 is
   similar in domains offline (0.85-0.88x) but 1.03-1.10x in CPU. Every other lever is ~1.05x or less in domains, or
   negative.
4. **Termination is not established.** At gen 7 the queue grows by ~0.59 pending per completion, with 2.8-4.0 discovered
   domains per native [M]. A faster engine reaches the 600 GB wall sooner. A launch criterion (H) is proposed (§2.4).
5. Owner rulings (09-27, 09-28) and evidence settle D2, D3, D4, D5, D6 (at launch), D1(b), D7, D8, D9, D-I2 and I1b (§3.1).
   **Seven items are genuinely open** (§3.2). Each has a default.
6. Comparator: runB is valid but provisional: 2.849M obligations/h, 1.255x run2 over the time window, 1.417x over the
   same ID range [M]. The >= 1.5x owner gate is scored against runC, after the `AggregateIndex::retire` swap fix.

## 1. W0 gates 0.2-0.9

| Gate | Verdict | Evidence | Caveats |
|---|---|---|---|
| 0.2 oracles: PASS on drained outputs, FAIL on every mutation | **PASS, provisional until merged** | Round-3 verifier `rustred-b0835196` (tip f1895f64): 10/10 calibration reports gate-PASS with full re-inspection and `roots_independently_verified == roots_total` (C-4L Ordered and Ready 248/268/628/656; C-5F 1/1 each). Paired audits 10/10 PASS. Frontier fixture refused (60/124). Mutation matrices: FG 48/48, C-5F 23/23 + 11/11 all_ok. Partial anchors are now well-founded. Route-native negative controls give exact classes. `covered_by_union` has 0 disagreements over 84,525 real (25,376 union-only) + 341,124 synthetic decisions, plus gen 7 16,381 + 66,916 [M `TMP/w0/oracle/r3/`] | C-HOT is audit-only (CP3). The verifier cannot read G2' residual records (a W4.2 precondition). At f1895f64, `assert_oracle_pass.py` still accepts reports made with `--mutate` or `--reference-levers as-run`, and `verifier_pairing.paired` is set unconditionally (audit line 574) [M]: fix before the merge |
| 0.3 CPU per native at K=96 <= 1.3x K=1 | **FAIL** (shared copy); remedy **INCOMPLETE** | Shared copy 3.754/3.753x on a quiet run set [M session C]. Node-bound processes p4 1.417x quiet [M]. Per-CCX private copies 1.14-1.17x, measured only in session E at 63-86% foreign load. Same-session contrast: shared 2.06-2.14 -> per-CCX 1.14-1.17 -> per-CCX processes 1.155 [M] | 1.17 is [E, loaded, provisional], for the gen-7 pending mix only. Projection bracket is 1.42-3.75. The quiet session F2/G/H (pid 2999184) was still load-gated at 11:17Z (nodes 4-6 27-46% busy). Replicas cost 5.46 GB each (65.7 GB for 12). Cause: shared written refcount lines, context count and/or variable-map count; the split is unresolved |
| 0.4 SoA kernel | (a) **PASS provisional**; (b) **INCOMPLETE**; (c) **PASS** | (a) Replay: SoA-id is 5.2-10.4x cheaper per candidate in 11/11 cells [M counts; timings void]. The realised legacy kernel reaches 2.92-3.03x forward and 3.72-4.00x reverse, unbiased (W1.1 gate (c) FAIL-accepted, orch. 1). (b) 13.9-27.4% admission share at 1G, borderline; W3.2 is moved into W2 by choice. (c) Cheap tiers resolve 76.6-88.9% of gen-7 requests | Legacy block boundaries cap the kernel at ~3x; the >= 4x target moves to the epoch layers |
| 0.5 baseline recorded, >= 80% attributed | **PASS** | 100% named; duty buckets 97.0/95.4% (95.3/92.4% strictly timed). Baseline of record [T, T+1,500.9 s]: 2.27M obligations/h, 1.30M natives/h, slice CV 25%, 33.4 foreign busy CPUs [M run2] | "Exclusive CPUs" FAIL is moot (the owner keeps socket 1 shared) |
| 0.6 inputs | I1 **PROVISIONAL**; I1b **DROPPED**; I2 load **PASS**; I2 witness gate **FAIL on total work** | I1 meets every criterion, and the round-2 verifier gives 268/268 on BMW I1. I2 on C-5F: Route->Route edges per Route native -61%, but natives 1.52x, Apply natives 2.13x, domains 1.45x, edges 1.30x, instructions 1.246x [M, 8 arms] | Needs the merged-verifier re-run on 18 arms. The 1-h L*-only walk (W1.3) has not run. The static proof covers rank <= 32 |
| 0.7 G2' bound: >= 30% of Apply CPU with a residual <= 10% in <= 8 pieces | **PASS** | D-only 70.1%, hull 89.0%; fully covered 61.6% of gen-7 Apply CPU [M census receipt-v4] | The 0.06-0.10x cost projection leaves out residual computation and the coordinator [E]. Gens 3 and 6 are the same run's history, not replicates. At-resume shares are [E]-weighted |
| 0.8 knobs | orders **FAIL**; build **PASS in instructions** | Orders: SV peak pending x1.84-2.10, DF natives x1.49-3.16; FIFO is kept [M counts]. `[profile.campaign]`: 0.927-0.944x instructions per native, with strict identity and oracle PASS on C-4L, four-all and C-5F. campaign+mimalloc: 0.78-0.83x instructions; RSS +0.50-0.67 GB as a fixed offset at W48; marginal 411 vs 509 B/domain [M ops S3] | The registered W1.2 harness bar at K=1/K=96 and the aged gen-7 RSS pilot are open. znver4, jemalloc and glibc tunables FAIL |
| 0.9 falsifiers | G1 **FAIL** (rejected); G2' union **FAIL narrowly** (not relaxed) | G1: guard frontier on H; C-5F 0.774x; C-HOT-sub 0.820x/0.696x. Dense cells add 3.47M out-of-closure points. Union residuals alone need 0.328x inspections on C-HOT, 0.521x on C-5F, 0.423x on r1a12 and 0.394x on r2a11 [M-off, corrected]. G2' live, arm u: record-s 0.496-0.537x (n=3 per arm; one cell nominally 0.496); arm n: 0.68-0.77x [M `TMP/w0/g2falsify/gate-table-2.md`] | Record-s is wall time under load (flag-off CV 0.011-0.085). Useful work and instructions are the better units (§2) |

Also touched by the W0 close: kernel gates (a), (b), (d) PASS and (c) FAIL-accepted. The ops lane passes C-4L 36/36, four-all
6/6 and C-5F 6/6 (strict plus oracle), plus stop, pause and resume and cross-binary resumes [M]. Comparator runB is valid
but provisional.

## 2. Work-volume synthesis (plan item 0.11)

**Units.** The domain factor is flag-off / lever for scheduled or discovered domains, at drain or at matched natives (> 1
means fewer). The CPU factor uses useful native work U (natives x per-owner cost) where it exists; otherwise instructions,
then record-seconds (named in each row). Campaign projections are [E].

### 2.1 Per lever

| Lever | Domain-volume factor | CPU factor | Pending dynamics | Basis | Status |
|---|---|---|---|---|---|
| **G2' union, arm u** (anchors: merged Native or validated G2' residual) | **1.25-1.26x** (scheduled 0.794-0.802) | **1.9-2.4x U** (0.418-0.525); 1.7-1.8x instructions; 1.9-2.0x record-s | Peak pending 0.82-0.86x. Pending/completion at matched discovered is lower (C-5F 0.36 -> 0.20 at 750k). Discovered per native unchanged (1.297 vs 1.316) | [M] C-5F Ordered and Ready W24, C-HOT-sub r1a12 W12, n=3 each, drained, 0 frontiers, audits and g2verify PASS. At the gen-7 mix: 61.6% of Apply CPU fully covered; ~29% of domains avoided at first level [E]; Apply CPU up to ~10x [E, upper bound] | Promoted W4 lever (owner). The registered <= 0.5x gate fails narrowly |
| G2', arm n (S7 as written) | 1.14-1.16x | 1.5-1.8x U | Peak 0.91-0.94x | [M] same sessions | Fallback only |
| G2', 1-2 anchors | 1.08-1.12x | 0.9-1.3x record-s | Peak 0.92-0.98x | [M] B1/B3 | Dropped |
| General union D2 (admission time, pending covers) | 2.21 / 4.35 / 5.16x at first level (by anchor rule) [E] | Apply CPU fully covered 76.7% vs 61.6%; union tests 2.2-7.7x today's 74.2M misses, on the serial path [E] | Net RSS 0.06-3.1x (k' unmeasured) | routecensus, census | Not pursued (owner chose G2') |
| Route union cover (dispatch time, merged natives) | ~1.05x beyond Apply G2' (3.4M of 74M at first level) [E, from 20.7% of 16.31M]. At resume, a Route pending alias avoids 2.0-3.9M creations (1.0-3.2 GB) [E] | ~1.0x (Route is 0.8% of native seconds) | D-only residuals are 1 piece | routecensus | Fold into W4.2 as a Route arm |
| Dense cells (θ=0.4) | Worse: 1.62x distinct points, 3.47M outside the closure | 0.228x inspections vs 0.328x for the union alone; the hot owner costs superlinearly per point | - | W0.9 oracle | Dropped |
| I1 (40 L* owners) | 1.02-1.05x at matched natives (within the ±3-5% noise) | **0.79-0.92x** (Apply native ops +9-26% at matched natives). Ceiling if all L* Apply work vanished: 1.3-1.7x in K=1 units | Pending 0.86-0.96x; peak 0.79x at the stop; P/n 1.37 vs 1.54; roots close earlier; edges 1.32-1.70x; peak RSS per discovered domain 1.01 vs 0.96 KB (+6%) | [M] 54-min fresh probes, n=1 per arm | Owner Q1 |
| I2 as built | 0.69x (1.45x domains) | 0.80x (instructions 1.246x) | Peak pending 1.09x | [M] C-5F, 8 arms | Not shipped |
| I2 rebuilt (one frame per owner) | ~1.2x modelled [E] | unmeasured | unmeasured | critique, plan | Rebuild, then re-gate with a total-work cap (owner) |
| Symmetry D6 | 1.18x gen 7, 1.13x C-5F [E first order; direct part M-off] | 1.03x gen 7; 1.09-1.10x C-5F | Pending 1.16x [E] | wv rev 2 | Off at launch; admission cost unmeasured |
| Piece-level D1(b) | n/a: unbounded helpers certify 206/900 | 0.64-0.79x (the idealised rule needs 1.27-1.57x more native seconds) | - | [M] four-loop, 33 runs, verify PASS | No |
| Priority order (SV, age bound) | 1.1-2.0x fewer natives on some controls | ~same | Peak pending **1.12-2.93x** | W0.8 counts | FIFO; redesign in W3.1 with a pending cap |
| Hot-owner closure import | ~1.0x (the pilot covers 0.4% of v2 gen-7 Apply natives and 0.0% of pending cost) | ~1.0x | - | census | Dropped |

**Compounded** [E; independence untested]: G2' u x I1 gives ~1.3x fewer domains per closure; adding D6 later gives ~1.5x. No
lever reaches >= 2x in domains. G2' reaches >= 2x only in useful-work units: on C-HOT-sub (0.418x), and at the gen-7 mix by
the census bound. It is signed off (D2) and treated as launch-relevant (Q2).

### 2.2 CPU levers, for context
- SoA kernel (legacy): runB gives 1.255x obligations/h over the time window and 1.417x over the same ID range, 0.657x
  admission CPU per obligation, and identical work volume (within 0.1%) [M]. The `retire` swap fix is worth +1.3-1.4x more
  [E] (runC).
- Per-CCX replicas: 3.75 -> 1.14-1.17 [E, loaded] gives up to 3.2x native throughput per inspector CPU; the quiet p4
  bound gives 2.6x [M/E].
- `[profile.campaign]`: 1.06-1.08x instructions. Adding mimalloc: 1.20-1.28x [M counts].
- Epoch engine: the P-IMP target is 5x run2 = 11.4M obligations/h. The owner gate is 1.5x runC, about 5.6-6.0M/h if runC
  gains 1.3-1.4x [E].

### 2.3 Termination and RAM-wall dynamics
- **v2**: pending grew in every hour, by 0.35-1.76 per completion (1.03 overall); 8 of 67 roots closed, flat after ~1.5 h
  [M, critique §2.2]. Native-pending at gen 7 is 20.57M, of which Route 14.72M [M census].
- **Gen-7 resumes, common ID range**: run2 and runB give 0.593 / 0.592 pending per completion and 2.785 / 2.783 discovered
  per native [M]. runB's later stretch: 3.95 discovered per native, and 0.784 pending per completion over its window [M].
- **Saturation**: gen-7 Apply natives add 7.3 new lattice points each (361k at g3); Route natives add 7.2-8.4 [M]. With
  C-HOT at 19.5x overlap, the walk is fragmenting a nearly saturated point set: domains keep growing without new points
  [E]. G2' union residuals attack this directly; engine speed does not.
- **Drained controls** show a turnover the campaign has not shown. On C-5F, pending/completions at matched discovered falls
  0.98 -> 0.17 over 250k -> 1M, and pending peaks at ~12% of total domains. G2' u lowers the curve (0.36 -> 0.20 at 750k)
  and the peak (0.82-0.86x) without changing discovered per native [M].
- **The wall** [E]. Legacy marginal RSS is 0.5-0.85 KB/domain (run2 610-847 B depending on fit start; runB 527-692 B;
  C-HOT W48 509 B). So 600 GB holds ~0.7-1.2G domains, or ~0.64-1.1G with 11-12 replicas. The epoch budget (protocol
  §3.6) is 0.23-0.30 KB/ID, i.e. ~1.66-1.88G IDs [E, design, unmeasured]. At the discovery rates:
  - legacy runB, 5.1M domains/h: ~120-220 h;
  - epoch, 50-156M domains/h (κ 3.75-1.17): ~10-37 h after a gen-7-sized start.
- **Reading** [E]: unless pending growth per completion turns negative, the campaign ends at the RAM guard with a subset of
  roots certified. Speed decides how much of the cone is explored before the wall. Work-volume levers decide how much
  closure fits under it (~1.3-1.5x from G2' + I1 + D6 later).

### 2.4 Proposed launch criterion (H)
This adds to protocol §15.5 (H), which it sharpens; it replaces nothing. Each part is necessary, and none is a termination claim.
- **(H1) Engine neutrality.** Compare P-IMP with runC from the same gen-7 clone, over the discovered-domain range both
  reach, in 0.5M-domain slices:
  - pending growth per completion <= 1.10x runC in the median slice, and <= runC's slice maximum in every slice;
  - discovered per native <= 1.05x;
  - noise floor from runC's repeats.
- **(H2) Fresh-walk dynamics.** Compare P-FRESH-v4 with P-FRESH-v3 (and v3 with v2's events) at matched discovered
  domains in 5M-domain slices, per owner class, with the same thresholds.
- **(H3) Lever delivery**, for every signed-off volume lever (G2' u; I1 if Q1 = a):
  - on C-5F and C-HOT-sub r1a12 (n >= 3): scheduled domains <= 0.85x and peak pending <= 0.90x of the flag-off arm;
  - pending per completion below flag-off at matched discovered domains, in every slice past 25% of the drain;
  - one aged gen-7 resume pilot with pending growth per completion <= runC's.
- **(H4) Receipt, not a threshold.** Record time-to-guard [E] at the measured discovered/h and marginal RSS, the last hour's
  pending growth per completion and discovered per native, the admitted cap (Q3) and the owner's at-wall action (Q4).

## 3. D-session memo

### 3.1 Settled (no question needed)

| Item | Ruling | By |
|---|---|---|
| D5 host | 600 GB cap. Socket 1 shared (no cpuset), foreign load recorded. No ZFS ARC cap. 50 GB MemAvailable save-and-stop floor. No off-pool target (zroot errors accepted) | owner 09-27 (0.1.11) |
| D9 governance | Full plan, gated: continue W2 only if the S2/S4 skeleton beats the legacy+SoA comparator by >= 1.5x (gen-7 resume, matched window). Otherwise fall back to MVP-B, then MVP-A, and report | owner 09-27 |
| Computer algebra | No own CAS; Symbolica vendored 953e26e2 + heap-pow. Per-CCX replicas only, no Symbolica patch. The owner may raise the shared `Arc<PolynomialContext>` refcount upstream (lane symbolica-mre prepares an MRE and a diff) | owner rules, 09-28 (b) |
| D2 | G2' in union form at dispatch. An anchor is any merged, fully discharged record (Native, or a validated G2' residual record), resolved well-founded in merge order. No admission-time general union | owner 09-28 (a) |
| D3 | No C2 vocabulary for G2': D-only residuals are 1 piece in 2,284/2,298 gen-7 partials, and Route residuals are 1 piece. The hull is optional (~15% lower residual cost [E]) | evidence |
| D4 | No widening (0.9 FAIL). P-anchors are moot: escape shells are 8.2e-5 of pending points | evidence |
| D6 | Off at launch. Not launch-blocking; deferred until its admission cost is measured on the epoch path; resolver hook only | owner 09-28, orch. 4 |
| D1(b) | No. Four-loop falsified: 206/900 certify, and the idealised rule costs 1.27-1.57x more native seconds. The five-loop factor is unmeasured | owner + evidence |
| D7 | Always stop at the first frontier; fresh start after an input fix | owner 09-28 (d) |
| D8 | Symbolic closure | owner 09-28 (d) |
| D-I2 | Rebuild with one coordinate frame per owner. Ship only with >= 20% fewer Route->Route edges AND natives, Apply natives and domains no worse, with audit PASS; otherwise keep the original witnesses | owner 09-28 (c) |
| Dropped by evidence | I1b; dense cells; hot-owner import; 1-2-anchor G2'; znver4, jemalloc, glibc tunables. Dispatch order stays FIFO in legacy | directive 0.1.5, lanes |
| Gates | The W4.2 <= 0.5x gate is not relaxed. runC scores the owner gate | orch. 7, 11 |
| Process model | One process with per-CCX replicas (60-66 GB of the cap). Per-CCX processes are the pre-registered fallback (protocol §2.1 switch rule) | owner (b), protocol |

### 3.2 Open decisions (default first; reply with a letter per question)

**Q1. D1(a): should roots of the 40 L* owners certify through I1 helpers (rank-bounded, A unbounded)?**
- (a) [default] Yes, with the L*-escape guard (stop on any edge that leaves R* or reaches rank > 32 or unbounded rank),
  once the merged-oracle re-verification and the guard's mutation test pass.
- (b) No: keep the plan-v3 A_max helpers for every owner.
- *Recommendation: (a).* Evidence [M; 54-min fresh probes, n=1 per arm]. For: peak pending 21% lower at the stop;
  pending growth per completion 1.37 vs 1.54; first roots closed at half the natives (3 vs 1 at 1M, 6 vs 3 at 2M);
  0 frontiers; 0 guard hits over 6 replays of up to 4.2G edges. Against: 9-26% more Apply work per native; 1.3-1.7x
  dependency edges (peak RSS per discovered domain +6%). The safety proof covers rank <= 32 (observed maximum 18).
  This is a pending and roots lever, not a CPU lever.

**Q2. Is G2' launch-blocking on every rung of the MVP ladder?**
- (a) [default] G2' is built only in the full plan (W4.2); MVP-A and MVP-B launch without it.
- (b) Yes: if the ladder falls back, first make the legacy G2' prototype production-grade.
- (c) Never launch-blocking: launch whatever is ready; G2' is bound in the digest, so it enters only a later fresh campaign.
- *Recommendation: (b).* Evidence [M]: the env-gated legacy prototype already drains C-5F and C-HOT-sub with 0
  frontiers, audits and pointwise g2verify PASS, at 0.79-0.80x domains, 0.82-0.86x peak pending and 0.42-0.53x useful
  work. It is the only measured lever that changes how much closure fits under 600 GB. Cost [E], ~1-2 agent-weeks:
  verifier support and G2' mutations (W4.2 needs them anyway), an indexed anchor search (the prototype scans 1.7-3.1e9
  anchors per control), and anchor records in the checkpoint.

**Q3. Memory admission at each start and resume.**
- (a) [default, as coded] Hard cap = min(600 GB, MemAvailable - 50 GB) at every (re)start, recorded in the receipt.
- (b) Runtime floor only: the cap is always 600 GB; the 50 GB floor acts only while the campaign runs.
- (c) As (a), but ZFS ARC above c_min counts as available.
- *Recommendation: (c).* Evidence [M]: MemAvailable was 629-692 GB on this host, with ARC at 204-349 GB (c_min 38 GB).
  At 636 GB, (a) caps the campaign at 586 GB (-2.3%). Two campaigns once held 646 GB of joint RSS with 155 GB
  MemAvailable to spare, because the ARC shrinks under pressure. (c) keeps 600 GB in every observed state and keeps
  the floor.

**Q4. At the wall: what happens when the RAM guard fires with roots uncertified?**
- (a) [default] Save and stop. Deliver the symbolic closure of every certified root plus the open cones of the rest.
  Continue only through a performance-only binary swap with a smaller footprint; anything else is a new owner decision.
- (b) Pre-authorise one scope stage: a fresh start of the uncertified roots only, with an agent-chosen helper-shape
  change, keeping the certified results.
- *Recommendation: (a) now; revisit (b) once the first P-FRESH (H) data exist.* Evidence [E]: the wall is at ~0.7-1.9G
  domains depending on the engine, 10-37 h (epoch) or 120-220 h (legacy) after a gen-7-sized start (§2.3). Root
  trimming saves <= 1.2%, so any scope stage must be a helper-shape stage.

**Q5. How is the >= 1.5x gate scored, and when is MVP-B preferred to MVP-A?**
- (a) [default, protocol §15.2] Interleave C S C S (n >= 2 per arm; C = runC's binary). The mean obligations/h ratio
  over load-matched 5-min slice pairs (<= 10 foreign CPUs apart; a run with < 3 pairs is repeated) decides. MVP-B beats
  MVP-A only if it passes the soundness battery and runs >= 1.2x MVP-A on the same protocol.
- (b) Stricter: n >= 3 per arm, and the lower end of the 80% bootstrap interval must be >= 1.5.
- (c) One run each, point estimate over [T, T+1,500 s] (as runB was scored).
- *Recommendation: (a).* Evidence [M]: runB's 5-min slices have CV 35% (run2 25%); same-state replicates spread ~10% in
  natives/h. The time-window ratio (1.255x) and the same-ID-range ratio (1.417x) differ by 13%, because the work mix
  drifts along the ID sequence.

**Q6. D-C4L: which combined four-loop control is canonical?**
- (a) [default] four-all (Vakint generation policy; planner physics class + 16 rank-12 anchors), beside the unchanged
  per-family FG/BMW/H/X.
- (b) Switch to four-all-p5 (five-loop generation policy).
- (c) Keep (a) and add four-all-p5 Ordered as a second identity row on every engine-affecting merge.
- *Recommendation: (c).* Evidence [M]: four-all has 30,159 natives, identical at W6/W24/W96, verify 32/32. four-all-p5
  has 31,717 natives, identical at W6/W24/W96, verify PASS; Ready drains W24 2/2 and W96 2/3. The A <= 19 envelope floods
  even under Ordered (3/7 drained). Neither variant has A-bounded helpers; 54 of the campaign's 67 do. (c) costs about
  one minute per merge.

**Q7. How fast must a restore be at scale (launch criterion (C))?**
- (a) [default, protocol proposal] <= 10 min on a >= 3e8-ID state, extrapolating to <= 20 min at 1G.
- (b) <= 30 min at 1G, with less W3.3 restore work.
- (c) Report only.
- *Recommendation: (a).* Evidence [M]: a gen-7 CP5 restore takes 564-600 s at 74M (600-615 s from launch to restored);
  linear CP5 extrapolation gives 27-40 min at 2-3e8 [E]. Every RAM-guard stop and binary swap pays one restore, and
  1-h pilots need restores under 10 min.

## 4. Plan changes W1-W5

**4.1 Close W0 first** (Track A; write-up before any new run).
- **Merge train** into `fable_5_1` (fmt, lib, `cli_routed_campaign` and the Python suite at each step, then push):
  (1) oracle, after two small fixes: the gate helper rejects `mutation != null`, `reinspection.complete != true` and
  undeclared as-run reference levers, and `paired` becomes the conjunction of the pairing checks; (2) kernel with the
  retire-swap fix; (3) ops; (4) harness tools; (5) inputs; (6) c4l (README item per Q6); (7) census, routecensus, wv
  (its example moves out of `rustred-core`), intel, baseline; (8) knobs, docs and tools only (`[profile.campaign]` comes
  through ops); (9) g2falsify, tools and note only (the env-gated engine stays unmerged unless Q2 = b).
- **Re-verify** with the merged verifier: the 18 inputs arms and the kernel Ready resume audits.
- **Commit and correct**: commit the protocol note rev 2 and this memo under `docs/research/`; ratify amendments 2a and
  3a; fix the handoff 0.1 item 5 wording and the stale "700 GB / CPUs 28-177 / 136 inspectors / ±10% comb-O" text.
- **Harness F2/G/H**: keep them load-gated until ~15:05Z. Without a quiet window, the in-engine κ at S4 (protocol §2.1)
  is the confirmation of record. Session H (c4l-x, c4l-fg, C-5F; g1 vs g12) runs before any launch-(B) projection uses a
  per-CCX factor.

**4.2 W1**
- **W1.1 kernel**: at `queue/index.rs:893`, swap `Meta<15>` rows only when `kept != read`; re-run strict identity
  (a)-(e); then runC, n=2 in one session. The four optional kernel fixes stay recorded, not built.
- **W1.2 native**:
  - per-CCX replicas in the legacy inspector pool (MVP-A path), with the full differential (FG/BMW/H/X, four-all, C-5F
    digest sinks, five-loop controls) and a replica-RSS growth check;
  - an A/B removing RustRed's hot-path `Arc<Vec<PolyVariable>>` clones (`model.rs:19-20`, `context.rs:25`), to split
    the cause;
  - N1 (modular zero certificates), then N2;
  - N3: adopt `[profile.campaign]` now; adopt mimalloc only after harness K=1/K=96 with replicas and an aged gen-7 W100
    RSS pilot that includes restore VmHWM.
- **W1.3 inputs v4**: I1 per Q1, with the guard (mask + rank; O(1) at `fn dependency`; persisted as a
  certification-blocking checkpoint item; mutation tests) and the 1-h L*-only walk. I2 rebuild, re-gated on C-5F and on
  a campaign-like mix (C-HOT-sub r1a12 or an aged gen-7 pilot), per owner class, under the total-work cap.
- **W1.4 ops**: done. Open: Q3, an "incomplete" supervisor state, and `independent_owner_campaign.py`'s 20 GB reserve.

**4.3 W2** (protocol note rev 2)
- **Order**: S2 -> S4 (+ per-CCX replicas, CP5 import, RollingLite) -> owner gate scored per Q5; S3 in parallel.
- **Scope**: W3.2 miss path inside W2; CP6 carries G2' residual anchor kinds from day one; D6 gets a hook only;
  first-found prefers sealed and oldest containers.
- **Gate builds**: both arms release + frame pointers + glibc; `[profile.campaign]` and mimalloc gains reported apart.
- **Gate units**: U with a frozen c_K1^ref table; κ, concurrency-qualified; IPC in native frames; marginal RSS over the
  second half of each pilot; (H) as in §2.4.

**4.4 W3**
- **3.1 dispatch**: priority order with a bounded enqueue depth plus a pending cap (SV-like orders raised peak pending
  1.1-2.9x); scored by roots per native CPU-hour and by pending at matched discovered domains.
- **3.5 placement**: owner programs replicated per CCX, never interleaved. Layer and index placement measured with
  per-successor lookups and numa_maps (numa_balancing = 1 on this host). Coordinator and helpers on the heap's node
  (89.4% of the heap on node 4 [M run2]).

**4.5 W4** (re-ordered)
- **4.2 G2' union comes first**, before N4 and the conditional levers, with a Route arm (dispatch-time, D-only).
  Preconditions: the verifier reads multi-anchor residual records (well-foundedness, anchor kinds, `covered_by_union` as
  the primary predicate); G2' mutations; an indexed anchor search.
- **Gate**: <= 0.5x U (record-s beside it), 0 frontiers, the (H3) domain clauses, one aged gen-7 pilot per owner class;
  C-HOT-sub box r1a12.
- **Dropped**: dense cells, widening allowlist, P-anchors, D1(b), hot-owner import, general union. D6: admission-cost
  measurement only. N4 follows the engine, unchanged.

**4.6 W5 and launch**: the rehearsal adds an at-wall drill (Q4), a guard trip with a restore under the cap (restore VmHWM
<= 1.00x RSS at save) and the Q3 admission receipt. The launch receipt carries the (H4) figures, foreign load and replica
RSS.

**4.7 Controls**
- **Per-family C-4L** (FG/BMW/H/X) is unchanged; four-all is an addition:
  - comb-O: strict identity for record-keeping changes; record-changing levers need drained + audit + verify gate +
    width invariance + natives <= 33,175;
  - comb-R: a one-sided Fisher test against interleaved 4a17f9c7, n >= 13 per arm and width; it BLOCKs on a regression,
    a violation, or the signature max(N993, N1009) >= 1,000;
  - a p5 identity row per Q6.
- **Other controls**: C-5F; the C-HOT-sub box named in every gate (r1a12 or s2/r2a12); C-HOT audit-only.
- **Every lever gate** adds aged gen-7 resume pilots, writes the mandatory recorder into every `metrics.json`, and
  asserts `verdict == PASS` and `roots_independently_verified == roots_total` in its gate script.

## 5. Errata (use the corrected form wherever quoted)
1. Handoff 0.1.5 / orch. 3: I1 is not "16.5% of gen-7 Apply CPU". It is 16-26% of contended seconds, 28-44% of op/term
   counts and 22-35% of K=1 Apply work (cumulative -> gen-7 append). Its ceiling is <= 1.3-1.7x, and it raises Apply ops
   1.09-1.26x.
2. Orch. 2 original: "1.14-1.17x vs 3.75x" should read: same-session 2.06-2.14 -> 1.14-1.17 [E, loaded, provisional],
   bracket 1.42-3.75. Replicas cost 5.46 GB each (65.7 GB for 12), not "~61 GB". The cause split is unresolved.
3. Handoff 0.1.12 / orch. 10(d): "D6 settled no by measurement" means off at launch, not launch-blocking, deferred (~1.18x
   domains [E]). "D1(b) settled no" is four-loop evidence only, at 1.27-1.57x, not 1.57x.
4. Orch. 7 original: G2' is the strongest measured work lever and the only one measured live, not "the only domain-volume
   lever".
5. Gate 0.4(a): the realised legacy kernel is 2.92-3.03x forward / 3.72-4.00x reverse (earlier gate-(c) ratios were biased
   by cfg(test) counters); 5.2-10.4x is the timing-void replay.
6. Gate 0.8 build "PASS" (knobs) rested on void timings. It now rests on instructions per native (ops). mimalloc RSS is a
   fixed +0.50-0.67 GB at W48, not "+3..+72%". N3 is "implemented, gate open", not "done".
7. c4l: the comb-O ±10% band is withdrawn, and comb-R needs n >= 13 (not n >= 5). The four-all W96 history is session 1 1/2
   (no load record), session A 2/3, session B2 4/8.
8. Census: at-resume shares are measured coverage with [E] weights. v2 covers 99.98% (not 100%) of the pilot's natives.
   Gens 3 and 6 are not independent runs. The four-loop "corrected" uncovered values are conservative bounds.
9. Baseline: the marginal is 610-847 B/domain, depending on fit start. Average RSS crosses 0.5 KB/domain at 80-90M. The
   replicate spread is ~10% in natives/h and 18% in ns/check [E, n=1 pair]. Strictly timed duty share is 92.4-95.3%.
10. Routecensus: the resume Route alias avoids 2.0-3.9M creations (1.0-3.2 GB), not 4.2-6.0M. Admission-time union net
    RSS is 0.06-3.1x.
11. I2: not "~0 CPU". As built it costs 1.246x instructions and 1.52x natives on C-5F.
12. Comparator: runA (d9163195) is invalid (progress_json 25.9%). runB is 1.255x over the time window vs 1.417x over the
    same range, with 40% of coordinator on-CPU time in memmove from `queue/index.rs:893`.
13. Oracle at f1895f64: `paired` is unconditional, and the helper accepts mutated or as-run reports. The old 3,580/3,580 was
    a D-cut identity check; the r3 union sample now validates multi-target covers.
14. Gen-7 restore is 564-600 s (600-615 s launch to restored), not 458.8 s. RAM: 600 GB holds ~0.7-1.2G legacy domains; the epoch's 0.23-0.30
    KB/ID is a design estimate.
15. Harness fills tables D/E: only the cross-CCX column is valid. mimalloc gives 2-4% at K=96 and is unresolved at K=1.
16. G2' W4.2 gate: B4 C-5F Ordered u at 0.496 is nominally below 0.5 in one cell; overall it FAILs narrowly (0.496-0.537).
17. The task's example questions (D2 three-way, D-I2, D7, D8) were already ruled at 09-28 ~09:40Z and are not re-asked.
