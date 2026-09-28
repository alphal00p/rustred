# W0.9 falsifiers: G1 hull widening and dense/sparse level cells

Lane `falsify`, branch `fable_5_1-v3-widen` (worktree `.claude/worktrees/agent-ade877816b107b1cf`), 2026-09-27.
Plan item: `docs/research/fable51_next_push_master_plan_2026-09-27.md` §5 W0.9 and gate 0.9.

Labels: **[M]** measured (run directory and binary sha256 given); **[M-off]** measured offline on saved walk output by a tool on this branch; **[E]** estimate. Nothing here is an ETA or a closure or termination claim. Both campaigns stayed stopped; nothing was started under `campaigns/` (the C-HOT inputs under `campaigns/five-loop-dependency-closure/inputs` were only read, as in the original pilot).

## 0. Verdicts

| Item | Gate / question | Verdict |
|---|---|---|
| (a) G1 widening, Apply successor arm | Go only if C-4L closes with 0 frontiers **and** C-5F and C-HOT-sub drain at ≤0.5x the inspector-seconds | **REJECTED.** C-4L H leaves 1 frontier and 24 of 628 roots open (2 of 2 repeats). No control reaches 0.5x: four-loop G1 costs 1.48-4.62x the exact record-seconds; C-5F 0.774x; C-HOT-sub 0.820x; C-HOT-sub2 0.696x [M]. An exploratory allowlist (all owners but the hot one) costs 1.204x on C-HOT-sub (§3.1). |
| Pre-registered prediction: "G1 fails on the hot owners" | Hot-owner cost regression | **CONFIRMED.** On the three five-loop controls the hottest owner `000011001001011` gets 109-128x fewer Apply inspections under G1, but its record-seconds change by 1.44x (C-5F), 1.09x (C-HOT-sub) and 0.97x (C-HOT-sub2) [M]. It carries 30-51% of the exact runs' Apply seconds and 57-68% of the G1 runs'. The other owners together fall 1.9-2.3x. |
| (b) Dense/sparse level cells, offline oracle on the drained C-HOT closure | Distinct Apply points and inspections vs today's 19.5x overlap | At θ = 0.4 the lens's static figure reproduces exactly: 590 dense cells + 4.80M sparse points = **1.245x** the distinct Apply points [M-off]. The admission-order (online) version needs **1.624x** the distinct points and **542,005 inspections (0.228x of today's 2,372,220)**. Pure union-cover residuals with no dense cells need 1.000x the points and 778,525 inspections (0.328x) [M-off]. So most of the reduction comes from union coverage, not from the dense cells. The dense cells add 3.47M points outside the exact closure, and G1 showed that such points can carry new guard frontiers. Cost projection: 0.05-0.39x of today's Apply seconds [E, wide; §4.3]. |

## 1. What was run

**Code (throwaway).** Commit `6c672de2` + fix `afdea4d4` adds `inspection::g1`, gated by the environment variable `RUSTRED_W0_G1_WIDEN=finite|all` and off by default. At the `OwnerAppliedEvent::Successor` arm, Apply branch only, after the existing pre-admitted-orthant check on S, the walk admits

W(S) = { same phase and owner, box [0, ∞)^N, R ≤ Rmax(S), A ≤ Amax(S), Dmin(S) ≤ D ≤ Dmax(S) }

instead of S. The extrema are the tight `DomainPowerSummary` extrema (existing `rustred-core` projection, no new algebra):
- An infinite extremum stays infinite, and a value that does not fit u32/u64/i64 becomes None. No axis is ever tightened.
- S ⊆ W(S) is asserted in release through `DomainPowerSummary::contains`. It never fired.
- W(S) is re-checked against the pre-admitted orthants.
- Route successors and routing are unchanged.
- The counters are reported under `w0_g1_widening` in `result.json`.

On every control of this note `skipped_unbounded_a = 0`, i.e. every widened successor had a finite Amax, so modes `finite` and `all` coincide; the runs used `all` [M].

**Binary.** `TMP/w0/falsify/bin/rustred-bce6772f` (sha256 `bce6772f1b3a185dc33f12fbb851f87c64978ecddb683f2b786ad4bf43b2436b`) is built from `afdea4d4` (= `fable_5_1` b15316b9 + the two commits above). Its Rust code equals 66ede259 apart from `g1`, and it uses the worktree's vendored Symbolica (953e26e2 plus the pre-existing uncommitted `src/poly/polynomial.rs` change, byte-identical to the main tree's working copy; not touched here). Both arms of every A/B use this one binary; only the environment variable differs.

Flag off, the counts equal those of `4a17f9c7` [M]:
- four-loop FG/BMW/H/X: 98,869 / 147,233 / 24,680 / 46,826 natives, containment checks 169,509,549 / 653,022,941 / 15,228,826 / 20,507,017 (FG identical to `TMP/fable51-controls/final-4a17f9c7/fg`; the natives of all four equal the `TMP/fable51-controls/RESULTS.md` baselines, and their containment checks agree at that table's precision);
- C-5F: 967,621 natives (the same as the historical Ordered count).

**Controls.** All runs are Ordered, lookahead 256, `--reuse-initial-d-bands --route-domain-overcover --unbounded-work`, nice 5, pinned to CPUs 264-287. The runner is `tools/research/w0_falsify/run_arm.py`, which reuses the historical command lines from `TMP/fable51-controls/run_control.py`.
- **C-4L:** four-loop FG/BMW/H/X at the A≤19, R≤12, D≥7 saved-cover envelope, W6.
  - Repeat 1: the four families concurrently on 264-269 / 270-275 / 276-281 / 282-287. In the G1 arm the oracle of §4 shared CPU 287 with X.
  - Repeat 2: exact and G1 concurrently, pairs of families on 264-275 and 276-287.
- **C-5F:** the 1,324-tuple finite control, hot owner, A≤11 R≤2 D≥9, W24 on 264-287. It was W50 in the plan; only 24 CPUs are this lane's.
- **C-HOT-sub:** defined in `TMP/w0/falsify/CHOTSUB.md`. Hot owner `011101110111000` on the C-HOT inputs, one full-orthant query with R≤1, A≤12, D free. It drains in 846 s whole / 739 s traversal at W12 on the legacy engine [M].
  - **C-HOT-sub2:** R≤2, A≤11, D free. It drains in 993 s [M]. Its closure is the knobs lane's `s1` box (their anchor r2-a11 contains both of their physics boxes).
  - The two sub-box exact arms ran concurrently at W12 on 264-275 and 276-287, and so did the two G1 arms.
- **Rejected sub-boxes** (still growing at ≥17 min on the legacy engine) [M]: r3a12 (W24; binary 4a17f9c7), r2a12 (W24), r3a11 (W12), r1a13 (W12). Runs are under `runs/calib-*` and `runs/exact-bce6772f-hotsub-*`.

**Inspector-seconds.** The gate uses the sum of `seconds` over all native records, Apply and Route, full and partial-initial-overlap, computed by `walkstats`. The sum of `parallel.slot_busy_seconds` is shown as a cross-check; it agrees to within 4%.

## 2. (a) G1 widening: A/B tables [M]

Runs are under `TMP/w0/falsify/runs/<label>/<family>/`, each with `command.json`, `result.json`, `events.jsonl`, `metrics.json` and `walkstats.json`. Binary sha256 bce6772f… for all rows. The table is generated by `tools/research/w0_falsify/summarize.py`.

| Control | Arm | Exit / status | Frontiers | Roots closed | Natives (Apply / Route) | Record-s | Slot-busy s | Traversal s | Max rank | Hot owner insp / s |
|---|---|---|---:|---|---|---:|---:|---:|---:|---|
| C-4L FG r1 | exact | 0 / locally_resolved | 0 | 248/248 | 98,869 / 0 | 33.0 | 33.3 | 12.8 | 14 | - |
| C-4L FG r1 | G1 | 0 / locally_resolved | 0 | 248/248 | 1,196 / 0 | 65.3 | 65.6 | 22.2 | 15 | - |
| C-4L BMW r1 | exact | 0 / locally_resolved | 0 | 268/268 | 147,233 / 0 | 71.2 | 71.9 | 35.3 | 13 | - |
| C-4L BMW r1 | G1 | 0 / locally_resolved | 0 | 268/268 | 2,234 / 0 | 112.3 | 113.6 | 38.5 | 14 | - |
| C-4L H r1 | exact | 0 / locally_resolved | 0 | 628/628 | 24,680 / 0 | 38.7 | 39.2 | 13.7 | 14 | - |
| C-4L H r1 | G1 | **4 / incomplete** | **1** | **604/628** | 2,978 / 0 | 176.5 | 179.2 | 60.4 | 16 | - |
| C-4L X r1 | exact | 0 / locally_resolved | 0 | 656/656 | 46,826 / 0 | 100.4 | 100.6 | 38.1 | 14 | - |
| C-4L X r1 | G1 | 0 / locally_resolved | 0 | 656/656 | 1,496 / 0 | 270.8 | 281.3 | 94.1 | 15 | - |
| C-4L FG r2 | exact | 0 / locally_resolved | 0 | 248/248 | 98,869 / 0 | 39.4 | 40.0 | 16.0 | 14 | - |
| C-4L FG r2 | G1 | 0 / locally_resolved | 0 | 248/248 | 1,196 / 0 | 80.1 | 80.6 | 27.4 | 15 | - |
| C-4L BMW r2 | exact | 0 / locally_resolved | 0 | 268/268 | 147,233 / 0 | 85.7 | 87.4 | 48.3 | 13 | - |
| C-4L BMW r2 | G1 | 0 / locally_resolved | 0 | 268/268 | 2,234 / 0 | 126.5 | 127.9 | 43.3 | 14 | - |
| C-4L H r2 | exact | 0 / locally_resolved | 0 | 628/628 | 24,680 / 0 | 41.0 | 41.6 | 14.5 | 14 | - |
| C-4L H r2 | G1 | **4 / incomplete** | **1** | **604/628** | 2,978 / 0 | 189.4 | 191.7 | 64.6 | 16 | - |
| C-4L X r2 | exact | 0 / locally_resolved | 0 | 656/656 | 46,826 / 0 | 109.3 | 109.6 | 41.0 | 14 | - |
| C-4L X r2 | G1 | 0 / locally_resolved | 0 | 656/656 | 1,496 / 0 | 296.6 | 308.0 | 103.1 | 15 | - |
| C-5F | exact | 0 / locally_resolved | 0 | 1/1 | 271,475 / 696,146 | 1,120.9 | 1,129.1 | 405.3 | 6 | 59,585 / 336.6 |
| C-5F | G1 | 0 / locally_resolved | 0 | 1/1 | 10,900 / 784,023 | 867.5 | 896.6 | 447.2 | 8 | 465 / 484.5 |
| C-HOT-sub | exact | 0 / locally_resolved | 0 | 1/1 | 292,203 / 728,394 | 2,047.0 | 2,060.4 | 738.9 | 6 | 67,208 / 1,029.5 |
| C-HOT-sub | G1 | 0 / locally_resolved | 0 | 1/1 | 10,589 / 744,831 | 1,679.4 | 1,714.0 | 706.2 | 8 | 547 / 1,120.3 |
| C-HOT-sub2 | exact | 0 / locally_resolved | 0 | 1/1 | 327,921 / 839,456 | 2,466.9 | 2,481.2 | 879.5 | 6 | 72,602 / 1,165.3 |
| C-HOT-sub2 | G1 | 0 / locally_resolved | 0 | 1/1 | 11,293 / 848,254 | 1,717.7 | 1,781.0 | 731.7 | 8 | 666 / 1,133.0 |

"Hot owner" = `000011001001011`, the top Apply owner by seconds in every five-loop control and in the drained C-HOT pilot.

| Control | G1/exact record-s | G1/exact slot-busy | G1/exact traversal | Gate (≤0.5x, 0 frontiers) |
|---|---:|---:|---:|---|
| C-4L FG (r1 / r2) | 1.979 / 2.033 | 1.968 / 2.013 | 1.742 / 1.710 | fail (cost) |
| C-4L BMW | 1.577 / 1.476 | 1.580 / 1.463 | 1.089 / 0.896 | fail (cost) |
| C-4L H | 4.563 / 4.618 | 4.565 / 4.612 | 4.418 / 4.459 | **fail (1 frontier, 24 roots open)** |
| C-4L X | 2.698 / 2.715 | 2.797 / 2.810 | 2.467 / 2.513 | fail (cost) |
| C-5F | 0.774 | 0.794 | 1.103 | fail |
| C-HOT-sub | 0.820 | 0.832 | 0.956 | fail |
| C-HOT-sub2 | 0.696 | 0.718 | 0.832 | fail |

Repeat spread [M]: the counts are identical across repeats, because Ordered is deterministic. Record-seconds of the same arm differ by up to +23% between the two sessions (both arms move together; host load), and the G1/exact ratios agree to within 0.1. The five-loop A/Bs have one repeat each. Their ratios (0.70-0.82) are 0.2-0.3 above the gate, far outside the ~7% Ready-spread noise, so a second repeat would not change the verdict [E].

## 3. (a) Mechanism and the prediction

**H frontier [M].** The frontier is a `local_dispatch_frontier`, `Unresolved { predicate: OriginalDenominator { batch: 0, rule: 64, term: 2 } }`, at the single point x = (0,2,0,8,3,0,0,0,0,0) of owner `1010011100` (A = 5, R = 13, D = −8). It lies inside the widened cell of record 1945 (R ≤ 13, A ≤ 5, D ∈ [−8, 3]). None of the 8 exact-closure domains of that owner contains the point (`runs/exact-bce6772f/h/result.json`). It is therefore a new guard obstruction brought in by points that the exact closure never needed. This is the "guard frontiers on unbounded cells" risk of the plan's §9 row, in a finite-A cell. It reproduces in both repeats.

**Closure growth [M].** G1 raises the maximum scheduled rank on every control: four-loop 13-14 → 14-16, five-loop 6 → 8. The Route natives rise by +1 to +13% (C-HOT-sub2 +1.0%, C-HOT-sub +2.3%, C-5F +12.6%), and Route record-seconds double (10-16 s → 22-26 s). Route stays below 2% of the seconds.

**Where the seconds go [M].** Apply inspections collapse 8-83x under G1 (four-loop H 8.3x, FG 83x; five-loop 25-29x), but the cost per widened cell is high where it matters. The table gives the cost per point by size decade for three owners on C-HOT-sub (`runs/*hotsub-r1a12-w12/hot/scale-<owner>.json`, `walkstats --scale`):

| Owner | Arm | ~5 pts | ~30 | ~300 | ~2.5k | ~20k | ~2-5e5 |
|---|---|---:|---:|---:|---:|---:|---:|
| 000011001001011 (hot) | exact µs/pt | 221 | 191 | 177 | 603 | 1,196 | - |
| 000011001001011 (hot) | G1 µs/pt | 533 | 489 | 239 | 440 | 478 (80 cells, 848 of 1,120 s) | 11.4 (21 cells at 5.2e5 pts, 5.9 s each) |
| 010111011000001 | exact / G1 µs/pt | 268 / 516 | 204 / 409 | 170 / 260 | 199 / 132 | - / 37 | - / 20.5 |
| 111000010101001 | exact / G1 µs/pt | 256 / 319 | 158 / 168 | 102 / 82 | 140 / 102 | - / 20 | - / 5.1 |

- Within the hot owner, exact cost per point **rises** above ~10³ points: 177 → 603 → 1,196 µs/pt, which is superlinear, as the pre-registered per-owner law said.
- The G1 cells in the 10⁴-point decade cost ~10.6 s each and carry 76% of that owner's G1 seconds.
- The largest G1 cells (5×10⁵ points) are cheap per point. Cost therefore follows *which* (A, R) levels a cell reaches, i.e. the expensive high-(A+R) tails, not the point count alone.
- Owners without the superlinear decade get about 2x cheaper under G1 (all non-hot owners together: 1.9-2.3x). The hot owner does not, and it is 30-50% of the exact Apply seconds, so no control reaches 0.5x.

**Exploratory allowlist arm [M].** In this arm G1 is applied to every owner except the hot owner (`RUSTRED_W0_G1_EXCLUDE=000011001001011`, C-HOT-sub, run `runs/widen-exclhot-bce6772f-hotsub-r1a12-w12`). It is post-hoc and does not change the gate verdict. The result (§3.1) is 1.204x the exact record-seconds, worse than G1 on all owners.

### 3.1 Allowlist arm result [M]

C-HOT-sub, W12 on 264-275, G1 on every owner except `000011001001011`: 10,013,771 successors widened, 15,323,595 skipped by the owner filter. Run: `runs/widen-exclhot-bce6772f-hotsub-r1a12-w12/hot`.

| Arm | Natives (Apply / Route) | Record-s | Slot-busy s | Hot owner insp / s | Frontiers / roots |
|---|---|---:|---:|---|---|
| exact | 292,203 / 728,394 | 2,047.0 | 2,060.4 | 67,208 / 1,029.5 | 0 / 1 of 1 |
| G1 on all owners | 10,589 / 744,831 | 1,679.4 (0.820x) | 1,714.0 | 547 / 1,120.3 | 0 / 1 of 1 |
| G1 on all owners except the hot owner | 119,687 / 811,234 | **2,464.3 (1.204x)** | 2,546.1 (1.236x) | **109,910 / 1,944.5** | 0 / 1 of 1 |

Excluding the hot owner makes the walk **more** expensive than the exact walk. The widened cells of the other owners feed exact successors into the hot owner: its inspections rise 1.64x and its seconds 1.89x against the exact run. This is closure growth measured directly: the extra points in upstream cells create downstream obligations that the exact closure never had. The allowlist arm ran alone on 264-275 while its exact reference ran next to C-HOT-sub2; a 1.2x ratio is far outside that contention effect [E]. A per-owner allowlist therefore cannot be judged owner by owner. The one allowlist tried here fails the gate by a wider margin than plain G1.

## 4. (b) Dense/sparse level cells: offline oracle [M-off]

**Tool.** `tools/research/w0_falsify/src/bin/levelcells.rs`, read-only. It reads the drained C-HOT closure `TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/hot-owner-physics-ordered/run/result.json` (sha256 `dd3f1dc1…`, per `audit.json`; binary 32fdec09; 7,767,543 domains).
- Lattice points are exact enumerations of box ∩ {A, R, D bounds}, with the rtool convention.
- IDs are monotone, with 0 enumeration failures.
- The C-HOT `audit.json` FAIL concerns physics-query mapping into the helper, not the domains; the oracle does not depend on it.

Outputs are in `TMP/w0/falsify/oracle/levelcells-chot-v2.txt` (C-HOT) and in `levelcells-{c5f,chotsub-r1a12,chotsub-r2a11}.txt` for the exact closures of this lane's controls.

**Scheme (algorithmic lens).**
- For every (owner, A, R) level, keep the covered-point count.
- Once covered/full ≥ θ, promote the level to one exact level cell, inspected once and in full.
- Below θ, each distinct point is inspected once, as the residual of the admitted domain that first reaches it.

Views:
- *Static*: the lens's view, on the final closure.
- *Online*: admission ID order of the 2,372,220 native Apply records. A record with at least one new point in an unpromoted level counts as one residual inspection. A promotion counts as one cell inspection of the full level.

**4.1 C-HOT (drained pilot, 13 Apply owners, 821 touched levels).** Today: 2,372,220 Apply inspections, 2.757e8 inspected points, 1.414e7 distinct points, i.e. **19.50x** overlap, and 10,232.3 Apply record-seconds.

| θ | Static: cells, inspected points / distinct | Online: inspections (residual + cells) / today | Online: points inspected / distinct (vs today) | Dense points outside the exact closure | E Apply s / today (range) |
|---|---|---|---|---:|---|
| 0 (all touched levels full) | 821 cells, 5.816x | 1,321 (500 + 821) / 0.001 | 5.857x (3.33x fewer) | 6.81e7 (83% of cell points) | 0.050-0.449 |
| 0.1 | 676, 2.061x | 203,058 / 0.086 | 2.278x (8.6x fewer) | 1.50e7 | 0.039-0.259 |
| 0.2 | 639, 1.602x | 341,862 / 0.144 | 1.906x | 8.52e6 | 0.043-0.295 |
| 0.3 | 616, 1.408x | 452,070 / 0.191 | 1.774x | 5.77e6 | 0.048-0.360 |
| **0.4** | **590 cells + 4.80M sparse pts, 1.245x** | **542,005 (541,415 + 590) / 0.228** | **1.624x (12.0x fewer)** | **3.47e6 (27% of 12.8M)** | **0.052-0.386** |
| 0.5 | 563, 1.122x | 606,880 / 0.256 | 1.474x | 1.73e6 | 0.054-0.401 |
| 0.8 | 502, 1.023x | 739,248 / 0.312 | 1.336x | 3.26e5 | 0.060-0.445 |
| ∞ (union-cover residual only) | 0, 1.000x | 778,525 / 0.328 | 1.000x (19.5x fewer) | 0 | 0.055-0.434 |

Findings:
- The lens's figures reproduce exactly [M-off]: θ = 0.4 gives 590 dense cells, 12.8M dense points, 4.8M sparse points in 231 levels and 1.25x; θ = 0 gives full level cells with 8.22e7 points, 3.4x fewer than today.
- **Online is worse than static**: at θ = 0.4 it inspects 1.62x the distinct points, not 1.25x, because 10.2M sparse points are inspected before their level is promoted and are then inspected again inside the cell.
- **Most of the collapse is union coverage, not cells.** Without any dense cell (θ = ∞), residual inspection against the union of earlier admissions already removes the 19.5x overlap (1.00x) and 67% of the inspections, with no point outside the exact closure. Dense cells at θ = 0.4 lower the inspections further, 0.328 → 0.228, at the price of 3.47M points (+25% of the distinct closure) that the exact closure never needed. §3 shows that such points can carry new guard frontiers (the H row) and raise the rank. The oracle cannot model that closure growth.
- **Cell sizes lie outside the measured cost range.** At θ = 0.4 the largest cell per owner is 0.46-21.5x the largest exact domain of that owner, and above 1.3x for 12 of the 13 owners: hot owner 228,690 vs 40,040 points; `111000010101001` 110,880 vs 5,148.
- The same pattern holds on the exact closures of this lane's controls [M-off]:
  - C-HOT-sub: 9.41x overlap today; θ = 0.4 gives static 1.245x, online 1.638x, 0.288x inspections; θ = ∞ gives 0.423x inspections.
  - C-HOT-sub2: 10.56x; θ = 0.4 gives 1.244x, 1.633x, 0.270x; θ = ∞ gives 0.394x.
  - C-5F: 3.60x; θ = 0.4 gives 1.236x, 1.586x, 0.346x; θ = ∞ gives 0.521x.

**4.3 Cost projection [E].** Today's Apply record-seconds are known exactly; the scheme's cost is not. Three models bracket it:
1. **Per-owner power law.** OLS of log seconds on log points over each owner's native Apply records, rescaled so that it reproduces the owner's measured total. α comes out at 0.59-0.78, and the rescale factor is 1.4-14, i.e. the log-log fit is poor. It extrapolates to cells up to 21.5x beyond the data.
2. **Linear cell cost** at the owner's top-decade seconds per point.
3. **Residual cost** between the share of new points in the record's seconds (proportional) and the record's full seconds (upper bound).

The range column is min(model 1 or 3-proportional) + min(cells) to 3-full + max(cells).
- At θ = 0.4 this gives 0.05-0.39x of today's Apply seconds.
- The θ = ∞ union residual gives 0.06-0.43x.

The live G1 evidence (§3) says that cells reaching the hot owner's high-(A, R) levels are *not* cheap, so model 1's low end is not credible for the hot owner. Neither bound includes closure growth from the out-of-closure points. **No ≥2x claim for dense cells separate from union coverage is supported.**

## 5. Implications for W1-W4 and the D-session

- **G1 / widening allowlist (plan §3.10, W4.4, D4):** rejected by the 0.9 gate on both conditions. D4's default "no" is confirmed for widening.
  - An allowlist would have to exclude every owner whose cost per point rises with size (the hot owner, ~30-50% of five-loop Apply seconds) *and* every guard-sensitive owner (the H frontier owner `1010011100` in four-loop).
  - Widening the non-hot owners while keeping the hot owner exact was measured at 1.204x the exact inspector-seconds on C-HOT-sub (§3.1): upstream cells inflate the hot owner's exact work by 1.89x. An allowlist has to be judged on closure growth downstream, not on per-owner cost.
  - Nothing in W1-W3 should depend on coarse cells. The moonshot's "10^4-10^6 cells" premise does not hold on these controls: under Apply-only G1 the five-loop natives fall only 1.22-1.36x, because Route natives (71-72% of the exact natives) do not fall; Route widening was not tested (plan: judged separately).
- **Dense/sparse level cells (W4.4 conditional, needs C2 and D2):** the oracle shows that their benefit is mostly the union-coverage part.
  - For the D-session: take the work-volume decision on **union-cover residuals / G2' (D2)**, with its bound from W0.7. Treat dense cells as an optional refinement with at most 0.33 → 0.23x of the inspections on C-HOT, bought with points outside the closure. Default: not launch-blocking.
  - If D2 is signed off, dense cells should be restricted to fully covered levels (θ ≥ 0.8-1.0, extra points ≤ 2%) or to owners without a superlinear top decade.
- **Controls for later waves:** C-HOT-sub (r1a12) drains in 14 min at W12 and C-HOT-sub2 (r2a11) in 16.5 min on the legacy engine. They are usable as the W2.5/W3.1/W4 gates. `CHOTSUB.md` has the exact definition. The knobs lane's `s2` (r2-a12 plus physics boxes) needed W48 on socket 1 to drain in 14 min; at W24 Ordered on 24 CPUs it does not drain in 20 min (`runs/exact-bce6772f-hotsub-r2a12`).
- **Instrument note:** at W12-W24 the legacy engine on these five-loop boxes runs 1,000-2,400 natives/s and is coordinator-bound, so the G1 wall-time ratios (0.83-1.10x) mostly reflect the coordinator. The gate metric is inspector-seconds, which is not affected by that.

## 6. Open issues

- One repeat each for the five-loop A/Bs; the four-loop has two. The G1 arms of repeat 1 X shared CPU 287 with the oracle.
- C-5F ran at W24, not W50, because socket 1 was held by another lane. Worker count does not enter the inspector-seconds ratio [E].
- The oracle's online view uses admission-ID order as the inspection order. It counts points of the *exact* closure only, and residual pieces are not boxes, so the piece count is unknown.
- Symbolica: no algebraic code was written. The widening uses the existing `DomainPowerSummary` projection. The oracle only counts lattice points (binomials in f64 for level sizes, in a standalone read-only tool). Upstream Symbolica (FETCH_HEAD 445b882d) has `numerica::Integer::binom`; vendored 953e26e2 has only private helpers. Neither was needed for exact algebra here.

## 7. Reproduction

On branch `fable_5_1-v3-widen`:
- `tools/research/w0_falsify/run_arm.py --binary TMP/w0/falsify/bin/rustred-bce6772f --family {fg,bmw,h,x,five-finite,hot} [--queries TMP/w0/falsify/inputs/hotsub-r1a12.json] --label L --cpus 264-287 --workers W --policy ordered [--env RUSTRED_W0_G1_WIDEN=all]`;
- `four_loop.sh` and `four_loop_pairs.sh` for C-4L;
- `walkstats <result.json> [--scale OWNER]`;
- `levelcells <result.json>`;
- `summarize.py`.

Build the tools with `cargo build --release --offline` in `tools/research/w0_falsify` (standalone crate).
