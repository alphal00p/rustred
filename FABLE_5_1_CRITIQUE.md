# Critical appraisal of `HANDOFF_opus_5_5.md` and the plan it hands over

Auditor: Claude Fable 5.1, 2026-09-27 (~19:45-21:00 UTC). Pure read-only audit: no file under the repository was
modified except this one; no build, test, binary, lock or campaign was touched. Another agent (Opus 5.5) was working
concurrently and opened new lanes during the audit; this critique is scoped to the handoff **as committed at e74d31ce
(19:26 UTC)** and to the two documents it names as governing: `docs/research/fable51_next_push_master_plan_2026-09-27.md`
and `docs/research/fable51_v3_engine_design_2026-09-27.md`.

Method: the handoff, master plan, v3 design, root-blocker note and the raw W0 receipts under `TMP/w0/` were read in
full. Nine independent critique lenses (throughput/native scaling, termination/RAM/work volume, soundness/oracles,
epoch-engine design, controls/measurement, inputs/decisions, execution/governance, numeric consistency, devil's
advocate) produced 96 findings; every critical/high finding was then handed to an adversarial skeptic instructed to
refute it against the receipts (27 of 29 verdicts returned before the audit was stopped to save credits). The auditor
independently re-derived the numbers the load-bearing critiques rest on (section 9).

Labels: **[M]** re-derived by the auditor or a skeptic from a cited receipt/file; **[D]** stated in the reviewed
documents and not independently checked; **[E]** estimate or interpretation. Verification status of each finding:
**confirmed**, **weakened** (the skeptic reduced or sharpened the claim; the corrected form is what is stated here), or
**unverified** (medium/low findings and the one high finding the skeptic round did not reach).

---

## 1. Verdict in brief

1. **The handoff is an unusually honest and well-evidenced document.** Roughly 30 lane-level [M] figures were
   re-derived from receipts and all matched; the negative results (G1 widening rejected, dispatch orders not
   adopted, Symbolica no-upgrade, I1b doubtful, oracle gaps) are reported rather than buried. Its defects are
   concentrated in the summary layers (TL;DR, sections 5 and 9) and in the plan it inherits, not in the lane data.
2. **The plan optimizes throughput while its own analysis says throughput is not what finishes the campaign.**
   Pending grew in every hour of v2 (0.35-1.76 per completion) [M]; 74% of all Apply natives (generations 4-7) added
   3.5% to the union of explored lattice points [M]. The plan states that a 5-20x engine "only reaches the RAM wall
   sooner", yet schedules the only mechanism that acts on fragment volume (union-cover / G2' residual inspection)
   last (W4) and defaults it off (D2 "neither"), and no gate before W4.2 measures pending growth or discovered domains
   per native under any lever. (TERM-1/2, DEVIL-2, PLAN-1, LAUNCH-1: weakened to medium, core confirmed.)
3. **The 3.75x in-process native-scaling loss is misread and mis-sequenced.** The harness data [M] shows the remote
   share of memory fills tracks the slowdown exactly (0% at K<=24 and in node-bound processes, 63% at K=96, 81%
   interleaved); "NUMA placement does not help" is true only of interleaving, which is the wrong remedy. In-process
   per-node replication of the read-shared owner programs (the hot owner's program is 817 MB [M]) was never tested,
   and the process model is deferred to "before W2 exit" although the v3 design is single-process throughout.
   (THR-1/3, EPOCH-1, DEVIL-3: weakened to medium; the numbers are confirmed, the diagnosis is open.)
4. **Several gates are written in units that cannot fail.** P-IMP and launch criterion (B) measure "native work" as
   inspector-thread CPU, which counts the 3.75x of stalled cycles as native work (THR-2: **confirmed high**).
   Criterion (C) gates an average RSS/domain that the 74M import dominates, while the wall is set by the marginal
   (RAM-1). The P-IMP ">=5x the baseline" omits the matched-window definition the plan uses elsewhere and its
   denominator has ~1.5x internal spread (BASE-1). The W2.3 "closure trajectory no worse" gate has no metric and no
   control on which roots move within an hour (SOUND-2).
5. **The soundness instrument is not yet a gate.** The committed verifier certifies PASS under `--require-closure`
   with zero re-inspection [M]; the fix exists only as an uncommitted, still-changing diff in the oracle worktree that
   the handoff does not mention; no multi-target coverage predicate exists although G2' needs one; and the F10
   "independence" covers bookkeeping, not successor generation. (SOUND-1/3/6, GATE-1: weakened to medium; every W1/W2
   gate cites this oracle.)
6. **Inputs v4 should shrink to I1 + I2 + I4.** Every measured signal on I1b is negative and its reinterpreted gate
   is non-discriminating by construction (INPUTS-1: **confirmed**). I1 is safe but bounded to 16.5% of Apply CPU.
   The "19,365,999 of 19,366,066 Apply domains lie outside their helper" statistic is a tautology.
7. **Governance gaps.** No minimum viable path, time-box or defined abandon rule (EXEC-1: weakened but still
   **high**); the launch criterion (F) waivers (cpuset exclusivity, ARC cap) have no recorded owner statement
   (MEAS-1: **confirmed high**); 14 unpushed branches with real file overlaps and no merge order; W0 results in
   untracked TMP files with placeholders; six refused fresh-start attempts written into the stopped interim campaign
   directory after the stop, unmentioned (GOV-1: attributed by the skeptic to the owner's own Zellij session, so
   low severity, but the handoff should say so).
8. **Numeric errata that change decisions**: the "pure union residual" figures for C-5F (0.346x) and C-HOT-sub
   (0.288x) are the dense-cell rows; union-only is 0.521x / 0.423x [M]. The gen-7 restore is 567-600 s, not 458.8 s
   (the verify phase is omitted) [M]. The 5-20x band, the 10-250 h RAM wall and the 700 GB guard in the governing
   notes were never rescaled to 90 inspectors, the measured scaling factor, the measured marginals or the 600 GB cap.

---

## 2. Load-bearing critiques

### 2.1 Native in-process scaling: the data supports a placement/sharing diagnosis, not a process-count decision

**What the handoff says** (5.7, 7.4, 9.1 item 1, 9.2): CPU per native at K=96 is 3.75x K=1; "NUMA placement does
not help, and separate processes largely do"; the outcome "decides whether the epoch engine uses one process with
~90 inspector threads or several inspector processes"; decide "before W2 exit".

**What the receipts show** [M, `TMP/w0/harness/sessions/C/*/perfstat.csv`, receipts]:

| run | CPU/native vs K=1 | near DRAM (G) | far DRAM (G) | far cache (G) | remote share |
|---|---|---|---|---|---|
| g-k8 (1 CCD) | 1.018x | 0.58 | 0.00 | 0.00 | 0.1% |
| g-k24 (1 node) | 1.463x | 1.15 | 0.00 | 0.00 | 0.1% |
| g-k48 (2 nodes) | 2.235x | 1.94 | 0.89 | 0.37 | 39% |
| g-k96 (3 nodes) | 3.754x | 1.72 | 2.12 | 0.84 | 63% |
| s-ft (4x24, first touch) | 3.86x | 1.53 | 2.28 | 0.95 | 68% |
| s-il (4x24, interleave) | 3.83x | 0.85 | 2.74 | 0.97 | 81% |
| p4 (4 node-bound processes) | 1.42x | 2.70 | 0.00 | 0.00 | 0.1% |

Instructions per native are constant across K (2.843e8) and futex waits are negligible [D 7.4], so the loss is
memory-system stalls. Interleaving raises the remote share to 81% (it is the opposite of a placement fix), so the
interleave arm cannot distinguish "remote placement" from "shared written lines". The p4 arm bundles three
remedies (per-process owner programs and reducer, private heaps, no cross-process sharing). The within-node 1.46x
at K=24 survives in p4 (1.42x), so a process split removes only the cross-node component and does not reach the
1.3x gate either. The read-shared data is large: the hot owner's program file is 817,327,375 bytes apparent (the 67
owners total 1.28 GB) [M], first-touched by the loading thread; M1 run2 shows 89.4% of the production heap on node 4
[M `numa.json`]. **Skeptic (THR-1, weakened):** the shorthand should read "interleaving has no effect; in-process
replication was not tested; node-bound processes reach 1.42x; root cause unidentified". The lane's own queued
`planD.txt` already schedules the in-process replica arms and per-CCD 12x8; those should run before any decision.

**Consequences for the plan.** (a) The epoch design is single-process everywhere (Arc snapshot per job, P2 in-process,
one W150/W100 split) and W2.0's scope (v3 §9 hazards) has no home for the process/placement decision; deciding at
"W2 exit" risks re-plumbing S3-S5 (THR-3, EPOCH-1, weakened). W2.0 should record the decision among, in cost order:
in-process per-node replication of owner programs/reducer; node-bound inspector processes with `--epoch-resolve
merge`; shared-memory snapshot segments only if both fail. S2 job/result types should be byte-serialized regardless.
(b) W3.5's "layers interleaved over nodes 4-7" would recreate the remote-access pattern for lookups; the memory
budget has no replication term (THR-6, unverified). (c) The harness measured algebra only (counting sink); the
epoch inspector adds per-successor image/digest work and latency-bound lookups, so 3.75x is a plausible lower bound
for a single-process epoch inspector, not an upper bound (THR-7, unverified). (d) The K=1 base is one run
(the repeat timed out) and socket-0 K=1 differs by 1.54x (SCALE-1, unverified).

### 2.2 Work volume and termination: the decisive lever is scheduled last and defaults off

**Evidence** [M]: v2 heartbeats, hourly pending growth per completion 0.35-1.76 (1.03 overall), queued rose in every
hour, roots 6->8 in hour 1 then flat for 17 h, rank 18 from hour 2. Census `saturation.json`: Apply natives per
generation 1.53M/1.44M/1.40M/1.13M/0.45M (g3-g7); union of explored points 5.540e11 after g3 -> 5.731e11 after g7
(+3.45%); new points per native 361,398 / 4,431 / 931 / 10,127 / 7.25. Census `cover-w30.json`: 55.4% (PPS by
predicted CPU) of pending Apply is fully covered by gen-7 natives, 79.5% by all other domains; D-only residual cuts
need <= 2 pieces in 2,298 of 2,298 sampled partials, so a D-only G2' needs no C2 vocabulary. `initial_overlap.rs`
(339 lines) already implements exact D-band residual reuse for initial anchors; `DomainPowerBounds` has no lower
bounds (confirms the C2 gap).

**Critique.** The plan's own text (§0.2, §3.8, v3 §8) says the engine does not change pending growth. The W1-W3
levers are CPU levers by construction; G2'/union cover is in W4 (after 45-55 agent-days of W2), D2 defaults to
"neither", and criterion (G) is then vacuously satisfied. **Skeptic (TERM-1, weakened):** "the only lever" is too
strong: I2 is a termination-relevant W1 lever (discovered -24%, pending -31% at matched natives in a 15-min W6 pilot
[M]) and G2' is Apply-only while Route is 74% of domains and 72% of native-pending. The genuine gap is narrower and
stands: **no gate before W4.2 measures pending growth per completion or discovered domains per native at matched
natives under any lever**, and G2'/D2 evidence is entirely in inspection and Apply-CPU units (TERM-2, weakened to
medium). The one drained-control lever with a measured volume effect, G1, cut Apply inspections 25-109x but left
scheduled domains at 0.9998x (C-5F) and 0.911x (C-HOT-sub) [M metrics.json], which the falsify write-up never
surfaced. Route pending (14.7M) has no coverage measurement at all.

**Erratum [M].** Handoff 5.2/7.3: "pure union-cover residuals need ... 0.328x the inspections (C-5F 0.346x, C-HOT-sub
0.288x)". The bracketed figures are the theta=0.4 dense-cell rows of `levelcells-*.txt`; the union-only rows are
C-5F 0.521x, C-HOT-sub r1a12 0.423x, r2a11 0.394x, C-HOT 0.328x. The lever's cut scales with overlap (3.6x overlap
-> 1.9x cut; 19.5x -> 3.0x); the 0.33x headline is C-HOT only.

**Recommendations.** Run a W0.9-style env-gated falsifier of D-only G2' (merged-Native anchors, residual
re-inspected, anchor edges) on the legacy engine on C-5F and C-HOT-sub within the 1-h rule, measuring scheduled
domains, peak pending, pending growth per completion and new points per native, not only inspector-seconds; add a
Route-side coverage census; redefine the "work factor" and criterion (G) in domain-volume terms; present D2 to the
owner as a three-way choice (none / G2' / general union) with the census numbers (G2' 55.4% vs general 79.5% of
pending Apply covered; projected residual cost 0.129 vs 0.045 [E]); add a launch criterion on pending dynamics at
matched discovered domains (the predecessor plan had one; the master plan demoted it to in-flight monitoring).

### 2.3 Throughput gates and bands

- **P-IMP and launch (B) count stalled cycles as native work (THR-2, confirmed, high).** "≥60/70% of
  inspector-thread CPU in native work" and "lookup CPU ≤1x/0.5x native CPU" are defined on thread CPU; the harness
  shows the same natives consume 3.75x the thread CPU at K=96 with identical instructions, on-CPU (wall/CPU per native
  1.016-1.030). A single-process engine with the measured contention passes both clauses while delivering ~27% of the
  native throughput the clause is meant to certify. Fix: define useful native CPU as natives x per-class K=1 cost (or
  retired instructions in native frames) and gate on useful/thread CPU; add "inspector CPU per native ≤1.3-1.5x the
  harness K=1 value" and an IPC clause.
- **Denominators disagree (THR-4, unverified; BASE-1, weakened).** The 5-20x band is "x v2 at matched D" (v2 late
  0.61-0.76M natives/h); P-IMP is "≥5x the restored baseline" (run2 1.30M natives/h, 2.27M obligations/h, 5-min
  slices 1.76-3.08M/h, 33-43 foreign busy CPUs); the two differ by 1.5-1.75x, so the band's own floor (3x v2) fails
  P-IMP. The restored baseline's "[E] denser index" reading is refuted by the lane's later close-out (candidate count
  carries over exactly; the 1.48x is ID-stretch mix). Rescaled to 90 inspectors and the measured factor, a single
  process projects 3.6-7.6k natives/s (10-21x restored) before lookup and merge costs [E]; the plan and design still
  print 136 inspectors, 5-20x, a 700 GB guard and CPUs 28-177.
- **Merge helpers (THR-5, EPOCH-7, unverified).** 6-8 helpers fit only the 74M / single-process / SoA-id cell; at 1G
  or at multi-process inspector rates the reverse-set work projects to 15-40 helper CPUs out of the same 100. Two
  bucket-skew figures (30% vs 2.9-4.8%) are in play; the per-epoch miss distribution across buckets is measurable
  offline on `streams-g7.jsonl` and should be.
- **Launch (B) admission share (THR-8, unverified).** The lane's own provisional projection (18-47% at 1G before a
  2.0-2.6x thread factor) already exceeds the 25% trigger that moves W3.2 into W2; the handoff records it as "likely"
  and leaves W2 scope unchanged.

### 2.4 Memory, restore and host

- **Average vs marginal (RAM-1, weakened).** Criterion (C), the W2 exit and W3.3 gates are averages (RSS per
  discovered domain); the wall depends on the marginal. Measured legacy marginals after a gen-7 restore decay with
  time since restore: run2 quarters 1,714 -> 850 -> 740 -> 506 B/domain, last 405 s 493 B/domain (R2 0.995), full
  window 844-847; run1 (11 min) 1,073-1,086 [M]. So the late-window legacy marginal sits at the 0.5 KB gate, not
  clearly above it; the §3.8 lower end of 0.3 KB/domain (hence 2.0G domains and 250 h) is supported by no
  measurement; at 0.5-0.84 KB, 600 GB holds 0.75-1.2G domains. State the gate as a marginal fitted over the second
  half of every ≥40-min pilot.
- **Per-ID summaries (EPOCH-3, unverified).** The design stores 176 B summaries for every ID and never recycles them;
  the budget counts them for live IDs only (~86 GB omitted at 1G). Either store live-only and recompute for retired
  targets in `verify`, or correct the budget and gates.
- **Restore peak ungated (EPOCH-2, weakened).** CP5 restore VmHWM is 1.67x the restored resident (gen 3 and gen 7)
  but only 0.13-0.16x the live resident at save; the concern is real only if v3's live footprint approaches the
  restored 0.38 KB/ID as §3.8 models. Add "restore peak within the guard" to (C) and record VmHWM in W2.2/W3.3.
- **Restore time (NUM-1, unverified but arithmetic checked).** 458.8 s excludes the 108 s verify phase; launch to
  restored was 570 s (run1) and 600 s (run2), at the P-IMP 10-min limit; linear extrapolation to 2-3e8 domains gives
  27-40 min against the W3.3 10-min gate.
- **ARC cap (RAM-2, weakened; MEAS-1, confirmed).** The 600 GB cap fits the host on measured evidence: two campaigns
  held 646 GB of joint RSS on 2026-09-27 10:03Z with 155 GB MemAvailable spare, and the ARC shrinks under anonymous
  pressure (c_min 38 GB). What is wrong is procedural: the master plan's ARC-cap and cpuset-exclusivity items in D5,
  (F) and W0.10 were dropped in the handoff without any recorded owner statement (the owner's answers waive only the
  off-pool item), and the handoff's own MemAvailable range (636-692 GB) leaves 16-72 GB before the supervisor's 20 GB
  save-and-stop. Ask the owner explicitly; raise the host reserve; record ARC size and MemAvailable in every receipt.

### 2.5 Soundness and oracles

- **PASS without re-inspection (SOUND-1, GATE-1, NUM-4; weakened to medium, core confirmed).** At committed tip
  1453936a `walk-verify-closure --require-closure --reinspect none --mutate dropped-edge` returns PASS with every root
  certified and `independently_verified` 0 [M]; the default `--reinspect all` re-inspects fully, so the exposure is
  explicit none/sample runs, including the planned production sample-mode F10 (which catches one bad native with
  probability ~1e-4 at 74M). The fix (INCOMPLETE verdict, `roots_independently_verified`, e2e tests, result binding)
  exists only as an uncommitted, actively changing diff in `.claude/worktrees/fable51-oracle` (10 files, +1746/-512
  at 20:20 UTC) that the handoff never mentions; §3.3 calls the lane "complete" and mis-references the gaps to 7.2.
  Every gate script should assert `verdict == PASS && roots_independently_verified == roots_total`; merge the oracle
  branch before any W1 gate is accepted. The TL;DR/gate lines should not repeat "both oracles pass on every drained
  output" (C-HOT is audit-only, CP3), "3.8e9 inclusions" (~1e8 positive inclusions; 3.8e9 `contains()` calls) or
  "successors covered 28.4M" (the tally is admitted successors; successor events are 25.08M/25.46M).
- **No G2' coverage oracle (SOUND-3, weakened).** The verifier checks single-target-or-alias-chain coverage;
  brute-force enumeration is capped at 4,096 points; the census coverage statistic samples points above 2M. An
  independent exact "Q ⊆ residual ∪ anchors" predicate (in `lattice.rs`, independent of the C2 code) plus G2'
  mutations (residual shrunk by one point, anchor stamp ≥ dispatch epoch, anchor not Native) must exist before W4.2
  code, and D2 must be decided before W4.
- **First-found bias (SOUND-2, weakened).** Newest-first, first-verified-container-wins biases coverage edges to
  queue-tail unsealed containers, the exact mechanism that blocked roots 25/35/42/43 in v2; the reassuring "86.3% of
  first-hit containers are ≥4.2M admissions old" was measured under the legacy ID-ordered scan. The 2.3 gate names no
  metric and no control on which roots move within an hour. Prefer sealed/oldest verified containers by default
  (performance-only under the bump contract) and gate on per-root open-cone counts at matched natives.
- **F10 independence and the mutation matrix (SOUND-6, unverified).** Rows pass when a class is merely present; no
  consistently-hidden-frontier mutation; alias-chain coverage exercised 0 times; the F10 reference is the engine's own
  visitor with walk levers off, so it will not catch an N1/N4 reducer regression; run the reference with N1/N4 off.
- **I1 safety argument (SOUND-4, INPUTS-5; unverified).** L_static is geometrically empty; the 40-owner L* rests on
  native one-hop closure at rank ≤32, an argument the summarizer did not verify. Observational support is strong
  (0 L* owners among 7.76M interim frontier ancestors; 0 support-increasing Apply edges over 1.7G edges [M]); an
  independent check of the one-hop containment argument and an in-flight stop on "Apply edge from an L* owner into a
  non-L* mask" are cheap insurance. Rename L_static to what it is.
- **N1 (SOUND-7, praise).** The certificate is one-sided (a nonzero residue is exact; zero falls back), so there is no
  false-zero exposure; C-5F and C-HOT carry the refusal-count gate (four-loop families have 0-17 refusals).
- **Test discipline (SOUND-8, unverified).** Seven silent license skips (the plan says five); three flaky tests (the
  plan hardens two); the knobs 784/2/6 figure has no saved summary line.

### 2.6 Controls and measurement validity

- **four-all (C4L-1, weakened; SCOPE-1, C4L-2 unverified).** The handoff is internally inconsistent: §2 rule 5 records
  the owner wants the combined run in addition to FG/BMW/H/X; §9.1(8)/§9.2 propose it replace the C-4L gate. four-all
  is the planner physics class (D in {7,8}) plus 16 rank-12 orthants, not the owner's A≤19/R≤12/D≥7 envelope; the
  envelope variant (four-all-a19) fails to drain within 1 h in 2 of 5 runs. Ordered four-all is a sound identity
  oracle (30,159 natives at W6, W24, W96). Ready four-all is schedule-sensitive (one W96 run did 239x the work and
  did not drain), so a binary drain gate on it is unsound for a rolling engine; use it statistically or Ordered
  only. Its programs use a different generation policy (sparse, depth 2) than the five-loop owners, its payloads live
  only in TMP, and the independent audit never ran.
- **Protocol (CTRL-2, weakened).** The literal 10% void rule was not met by the knobs build A/B (15-28%), the intel
  socket-1 sweep (~41 foreign CPUs, one pass) or M1 (33-43 CPUs), and §9.2 adopts `[profile.campaign]` and the SoA
  kernel without restating it; but the rule was met in several sessions (symbolica C-4L 1.1-5.5%, harness K=96
  2.7-9.4%), the C-5F build confirmation exists (b50, campaign x0.897 both repeats, strict identity), and harness
  repeats show CPU per native drifting ≤1.5% between 5% and 32% foreign load. Replace the void rule with a mandatory
  recorder (foreign busy CPUs, schedstat run delay, instructions per native) stamped into `metrics.json`; express
  algorithmic levers in counts, CPU levers in instructions per native, layout levers in cycles/IPC; keep wall-rate
  gates such as (D) labelled load-sensitive. `run_control.py` records no load field and was edited mid-W0 (PROV-1).
- **Controls invert the campaign's cost mix (CTRL-1, weakened).** All five-loop controls are boxes on hot owner
  011101110111000, yet that owner carries 0.3-1.3% of their Apply CPU (C-HOT 127 of 10,232 s) against 64% in gen 7
  [M]; none reaches the aged-queue regime (rank ≤8 vs 18; ≤1.5M domains vs 74M). Report every five-loop lever gate per
  owner class and add chained-resume pilots for aging.
- **Two C-HOT-sub definitions (NUM-6, CHOT-1).** Falsify: r1a12 (1.02M natives, drains at W12); knobs: s2/r2a12
  (2.12M natives, drains only at W48). Gates citing "C-HOT-sub" must name the box. C-HOT is admissible "only once
  epoch drains it in ≤45 min", a circular rule, and its CP3 state cannot be edge-verified.
- **Ready non-reproducibility (READY-1).** ±5%/+10% thresholds in gates 2.3/2.5/3.1 sit at or below the measured
  schedule noise (four-all W24 spread 9.6%; knobs CB repeats 36%); state n ≥ 3-5 and a noise floor per control.

### 2.7 Inputs and owner decisions

- **Drop I1b (INPUTS-1, confirmed).** Four-loop BMW I1b does not drain in either repeat (≥2.68M natives vs 147k
  baseline); two same-binary same-CPU five-loop controls show I1b at 1.19-1.20x scheduled, 1.45-1.49x pending,
  1.54-1.73x wall per native and half the roots; unbounded-A domains can only descend from A-null helpers, whose set
  is identical in plan-v3 and I1b, so the reinterpreted gate "0 unbounded-A domains in guard masks" cannot fail.
  Remove I1b from §6.4, the D1 default and W1.3; do not bring gate wording to the owner.
- **I1 (INPUTS-2, weakened).** Safe (byte-identical to the interim's helpers on those owners, 22 h without an L*
  frontier ancestor) but bounded: L* owners carry 16.5% of gen-7 Apply CPU and 39% of Apply domains; the four hottest
  owners (78% of Apply CPU) are outside L*. Present I1 as a domain-count lever, not a ≥2x work lever.
- **Tautology (INPUTS-3).** 19,366,066 - 19,365,999 = 67 = the helpers themselves; every non-helper Apply domain is
  outside its helper by construction. The master plan's "82% are A-escapes" supports unbounded-A absorption (unsafe
  for U owners), not finite envelopes; it should not be cited for I1b.
- **I2 (INPUTS-4).** -56% Route→Route edges per Route native reproduces [M]; Route→Apply per Route native is
  unchanged and Route is <1% of native seconds, so I2 is a memory/index lever (~-18% total domains [E]) with ~0 CPU
  effect on current evidence; its formal C-5F gate never ran.
- **D1-D8 (DECISIONS-1/2, SCOPE-1; weakened).** The memo is defaults, not quantified choices, and the W0.11 synthesis
  it presupposes never ran. Settled by evidence: D4 (widening rejected; P-anchors moot, 8e-5 of pending points lie
  outside the committed envelope), D5. Quantifiable but not quantified: D2 (G2' vs general union, above), D7 (54 of
  116 physics roots sit in the 27 non-L* owners), D8 (what is deliverable at save-stop). Not measurable yet: D1(b)
  and D6 (their ≤1 h W0.11 measurements never ran). I3's "≤1.2% trimming" is correct for root trimming under
  full-orthant helpers and therefore says that any scope stage must be a helper-shape stage; give the owner an
  at-wall ladder now, not at the guard.

### 2.8 Execution and governance

- **No MVP, time-box or abandon rule (EXEC-1, weakened but high).** R14 cites a "W2 abandon rule" defined nowhere;
  the W2 exit's only failure path is a loop ("below 3x: fix the limiter"); criterion (D) is open-ended. A deployable
  fallback exists today and is never named: 4a17f9c7 + `[profile.campaign]` + mimalloc override (x0.78-0.86 inspector
  CPU per native at W6 with strict identity [M]) + plan-v3 or I1(L*)+I2 inputs + frontier stop + 600 GB guard; it
  stays coordinator-bound (~9 of 100 CPUs) and hits the same wall, but it is the honest "launch now" arm. Define
  MVP-A (legacy+levers), MVP-B (epoch S2-S4 lockstep + CP6 + kernel, no W3/W4) and the full plan, each with a
  launch criterion and a time-box, and bring it to the owner as D9.
- **Lane count vs the single socket-1 resource and the usage budget (EXEC-2, weakened).** W0 ran 10 lanes; the
  socket-1 lock was held nearly continuously 12:46-19:36 UTC; two harness sessions died waiting; the knobs session
  skipped an arm on budget; six lanes ended with results in untracked TMP files with placeholders. The pattern
  recurred within 30 minutes of the handoff (three new W1 worktrees at 19:52-19:54 UTC before any W0 write-up was
  persisted [M]). Add a concurrent-lane cap, a write-up-before-new-runs gate and an automatic results writer.
- **Merge train (MERGE-1, weakened).** 14 branches, no upstreams, real file overlaps (harness × oracle on
  `checkpoint.rs`/`inspection.rs`/`mod.rs`; harness/intel/knobs on `execution.rs`/`queue.rs`); `[profile.campaign]`
  is 8 lines inside a +1,533-line knobs commit that breaks two frozen key-set tests; intel's `admission-trace` is an
  engine change on a "tools" branch; "no legacy-engine changes from W0" and W1.1/W1.2 "on the legacy engine" contradict
  R15 "legacy lanes frozen". Publish a merge order; define "frozen legacy" as binary 4a17f9c7 plus strict identity.
- **Provenance (PROV-1, NUM-9).** The heap-pow Symbolica patch is committed under `patches/` and byte-identical in all
  12 worktrees [M] but nothing applies or checks it at build time; 5 of 6 knobs binary sidecars record `dirty:false`
  although the submodule was modified. The verifier's blocking reports and the run2 analysis lived only in an
  ephemeral scratchpad at handoff time.
- **Campaign directory (GOV-1, weakened to low).** Six refused fresh-mode starts (10:56 and 12:07 UTC) were written
  under the stopped interim campaign and `active-run.json` was rewritten; the paused state is intact [M]. The skeptic
  traced them to the owner's own Zellij session; the handoff should still record them since §0.1 declares the
  directory frozen evidence.
- **Document (DOC-1, NUM-10/11).** No one-page resumption card; labels [M-r]/[M-off]/[src] undefined; §3.3 stale
  within 30 min (branch tips and worktree assignments moved); the v3 note and master plan still say 700 GB, CPUs
  28-177 and per-family C-4L with no errata list.

---

## 3. Epoch-engine design findings (all medium/low unless stated; EPOCH-1 weakened)

- **F7 vs A2 unreconciled (EPOCH-4).** The design note (the document W2 will implement from) says errored results
  never merge; A2 merges deterministic errors as `Native{error}`. The plan records the contradiction but the design
  is unamended; resume behaviour after a persisted error is undefined (with D7 "always stop" a single deterministic
  error could make the campaign unresumable without a new flag).
- **Liveness (EPOCH-5).** No attempt counter: a head whose inspection alone trips the RAM guard is saved Reserved,
  dispatched first on resume, trips it again; infinite save-stop/resume with zero progress. Add a per-ID attempt map,
  escalate after N transient failures, dispatch repeat offenders last, add a W2.2 drill.
- **Rolling has no oracle (EPOCH-6).** Lockstep depth 1 never exercises delta checks, stale misses, A6 refresh or
  background-save queues; A6's time-based refresh makes lockstep-2 non-deterministic. Make refresh event-count-only,
  record snapshot versions per job and the merge order per epoch, so a rolling run is replayable; add a chaos mode.
- **CP6 cut and F15 (EPOCH-8).** A consistent cut of the rewritten sections (≈9 GB at 1G) while merges continue needs
  chunked/COW sections or a pause; a forced pre-save refresh over 16G edges (~200 s [E]) alone consumes the 2% stall
  budget. Saves should consume the last completed refresher result.
- **Merge fallback (EPOCH-7).** `--epoch-resolve merge` moves all ~201 requests/native onto 6-8 threads and needs an
  O(m²) antichain per bucket per epoch; it is a diagnostic, not a production fallback.
- **Unsafe claim (EPOCH-9).** "No unsafe except SIMD" does not survive a shared-memory design; `retired_at` in an
  immutable layer is either dead weight or a shared written line.
- **Bands not rescaled (EPOCH-10).** Design §8 still assumes 136 inspectors and linear scaling.
- **What checks out (EPOCH-11, praise).** `CompactSummary::contains` does accept EMPTY before the owner test and never
  compares phase [M compact.rs:513-543], so the A1 chokepoint is necessary; the cited source anchors (semantics
  version, LOG_CAP, MAX_KEYS, `binding()`) hold; Reserved-never-transfers and the ID-increasing alias rule make chains
  acyclic; the termination condition is checkable by the restore validators.

---

## 4. Diagnosis: is the rewrite the right bet? (DEVIL-1/4/5/10, weakened; unverified items marked)

- **"Coordinator-bound at 97% duty" is a duty-timer reading.** M1 run1/run2 [M]: coordinator 0.53-0.55 CPU (46-48%
  ordered_commit on-CPU, 44-46% waiting for helper batches), 32 helpers at 6.8-7.2 CPUs (IPC 0.49-0.64, 55-57%
  far-node fills), 67 inspectors at 1.66 CPUs, ~9 of 100 CPUs. It is a latency-bound serial admission pipeline, not
  a saturated thread. The epoch design attacks the right term (moving 96% containment hits off the serial path), and
  the skeptic confirmed the caveat is stated in the handoff; but no measurement separates "rewrite required" from
  "prep/commit overlap + SoA kernel on the legacy engine suffices" (perfect overlap ≤2x; overlap + SoA on commit and
  helper checks ~4-6x at 74M [E]), which is the same order as the W2 exit gate. A 25-min gen-7 resume of the legacy
  engine with the SoA kernel would settle it and give W2 a comparator.
- **The 27-agent review could not question the rewrite (DEVIL-4).** Its shared context stated the diagnosis as fact;
  all four strategies on the ballot included a rewrite; the work-reduction-first submission was a 394-byte
  placeholder [M journal]; the per-owner cost exponent (0.74-0.90) that the plan cites as the sole basis for refusing
  cells-first is contradicted by the census (hot owner 0.659, others 0.51-0.63) and flagged "not reconciled".
- **Some §9 rejections cite the wrong number (DEVIL-5, unverified).** "Independent owner shards 2x slower" is a
  startup-dominated 2.5-minute control; the evidence-based objection is inspection inflation (3.95x). "Inline Route
  saves no IDs" cites no measurement (parents per Route domain is unreported).
- **Plans the owner should see side by side (DEVIL-10, unverified).** A: as written (decisive pilot P-IMP ≥5x with
  a matched-window definition). B: legacy engine + result-identical levers (SoA kernel, prep-ahead, campaign profile,
  mimalloc, I2, I1) with node-bound inspector processes if the 3.75x is not fixed in-process; decisive pilot: 25-min
  gen-7 resume ≥3x run2. C: union coverage first (D-only G2' on the legacy engine + an oracle-verified offline
  pending trim at resume); decisive pilots: C-HOT-sub ≤0.5x with 0 frontiers and verify-closure PASS, and a 45-min
  fresh run with pending/completion below v2 at matched domains. D: scope (D1(b)/D8) with the W0.11 falsifier and a
  cone census. B and C are built from items the plan already schedules; they only need to be measured as end states.

---

## 5. Numeric and consistency errata (NUM lens; all reproduce from receipts)

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

---

## 6. What the plan and handoff get right (preserve)

- Soundness posture: single mutator, whole-inspection commit, verify-every-positive chokepoint grounded in source,
  frontier-stop default, explicit two-level claim wording, oracles with a mutation matrix and 0 disagreements over
  3.7e9 brute-force inclusion checks, and an adversarial verification of the oracles carried into the handoff
  verbatim.
- Measure-first W0 with real receipts (M1 thread/duty/perf data, the K-sweep harness with a differential proof on
  five controls, the census, the level-cell oracle), pre-registered falsifiers (G1) allowed to reject a favoured
  idea, and negative results reported.
- The diagnosis that admission, not algebra, consumes the legacy engine is supported by M1 (helpers 7.2 CPUs vs
  inspectors 1.66) and the SoA kernel decision rests on measured per-candidate costs.
- Owner rules are respected: no own CAS (lattice.rs, census, C2 are geometry/decision logic; N1 uses the vendored
  finite-field API), campaigns not launched or stopped by agents, 1-h pilots, evidence labels, no ETA or closure
  claims; the falsify and inputs lanes are templates for A/B design (same binary + flag, concurrent same-socket arms,
  two repeats, hashes).
- The plan already anticipates multi-anchor bookkeeping in CP6 (§3.11), so adopting G2' later does not force a
  checkpoint redesign, and it correctly separates semantics from performance in the bump contract.

---

## 7. Recommended re-sequencing (concrete)

**Track A, no socket-1 time, first:** (1) push all lane branches; persist every `RESULTS.md` as a docs/research note;
copy the ephemeral verifier reports into TMP/w0. (2) Commit or discard the oracle worktree diff; make PASS require
full re-inspection; assert it in every gate script; merge the oracle branch. (3) Write the errata list (section 5)
into the handoff, plan and v3 note. (4) Write the D-memo now with present evidence: settled (D4, D5), quantified
choices (D2 three-way with census numbers; D7 with 54/116 exposure; D8/at-wall ladder), not-yet-measurable (D1(b),
D6) with their ≤1 h measurements scheduled, plus **D9: MVP ladder, time-box, abandon rule, and the (F) waivers
(cpuset, ARC) put to the owner explicitly**. Strike I1b.

**Track B, one serialized socket-1 queue (≤1 h slots):** (5) harness planD arms: in-process per-node replicas,
per-CCD 12x8, K=1 replicate, perf c2c at K=24/96, near-cache counters. (6) Legacy engine + SoA kernel (+ prep-ahead
if cheap) 25-min gen-7 resume vs run2, matched window: the W2 comparator. (7) D-only G2' falsifier on the legacy
engine (C-5F, C-HOT-sub r1a12) with domain-volume metrics. (8) I2 C-5F A/B with audit; probe-I1 vs probe-v3b.
(9) four-all: Ordered W96 reference; Ready statistics (≥5 repeats); audit.

**Then W2.0** with: the process/placement decision (three options, cost order); F7/A2 reconciled; attempt caps; the
rolling replay oracle; gates rewritten in K=1-equivalent native work, marginal RSS, matched-window baseline, and a
pending-dynamics launch criterion (H). Continue W2 only if the S2/S4 skeleton beats the legacy+SoA comparator by
≥1.5x on the same protocol; otherwise launch MVP-A/B.

---

## 8. Findings register (id, severity as issued → after verification, status)

Critical/high after verification: THR-2 (high, confirmed), MEAS-1 (high, confirmed), EXEC-1 (high, weakened).
Confirmed at medium: INPUTS-1. Weakened to medium (corrected forms used above): SOUND-1, SOUND-2, SOUND-3, TERM-1,
TERM-2, TERM-3, RAM-1, RAM-2, EPOCH-1, EPOCH-2, CTRL-1, CTRL-2, C4L-1, BASE-1, THR-1, THR-3, MERGE-1, EXEC-2,
DECISIONS-1, INPUTS-2, DEVIL-1, DEVIL-2, DEVIL-3. Weakened to low: GOV-1. Not reached by the skeptic round: DEVIL-4
(high as issued). Unverified medium/low findings cited above: THR-4..9, EPOCH-3..10, SOUND-4..9, TERM-4, METRIC-1,
PLAN-1, LAUNCH-1, READY-1, CHOT-1, C4L-2, SCALE-1, PROV-1, GATE-1, EVID-1, INPUTS-3..5, DECISIONS-2, SCOPE-1, SEQ-1,
DOC-1, RULES-1, NUM-1..12, DEVIL-5..10. Praise items: SOUND-7/10, EPOCH-11, THR-10, NUM-12, DEVIL-11, PRAISE-1 (x4).
Full lens reports and skeptic verdicts are in the auditor's session scratchpad
(`/tmp/claude-1125/-common-dev-rustred/aeb0f8d4-d55f-4f58-a891-835d003dae7b/scratchpad/synth/`), ephemeral.

## 9. Auditor's independent checks (all read-only)

- Harness perf counters per run (table in 2.1) from `sessions/C/*/perfstat.csv`; passes and threads from receipts.
- Owner program sizes: `campaigns/five-loop-qcd-feynman-d9d10-v2/inputs/owners/0066-011101110111000.rrbin`
  817,327,375 B apparent; 67 files 1.28 GB apparent (133 MB on disk, ZFS-compressed).
- M1 run2 `numa.json`: 7,902,105 of 8,838,644 pages on node 4 (89.4%).
- Census `saturation.json` totals_natives and `cover-w30.json` pending coverage (numbers in 2.2).
- Falsifier oracle rows (erratum 5) from `TMP/w0/falsify/oracle/levelcells-*.txt`.
- `initial_overlap.rs` scope (initial Apply domains only, MAX_INITIAL_DOMAINS 4096); `DomainPowerBounds` fields.
- Host at 19:45 UTC: MemTotal 1,188 GB, MemAvailable 838 GB, ARC 203 GB (c_max 1,216 GB, uncapped), nodes 4-7
  148.6 GB each, zroot 6.6 TB free, /common 3.1 TB free.
- Interim campaign directory: six refused fresh-start receipts (10:56, 12:07 UTC), `active-run.json` rewritten 12:07,
  checkpoint state files unchanged since 09:59/10:29.
- Live activity of the other agent at 19:52-20:03 UTC: worktrees for v3-kernel, v3-ops, v3-wv created at e74d31ce;
  knobs session E-c5f-build-r2 holding `socket1.lock`; a release cargo build on `build-0.lock`. Nothing was touched.
