# HANDOFF (Opus 5.5 session, 2026-09-26 15:25 UTC to 2026-09-27): the fable_5_1 "next push"

This document hands over everything from the Opus 5.5 session that followed the Fable 5.1 sessions on branch
`fable_5_1` of RustRed (`/common/dev/rustred`). It states the goal, the owner's rules, the current state, every
finding with its evidence, the plan in force, the status of the work in flight, and how to continue. It is meant
to be read in full before doing anything.

Owner: Valentin Hirschi (theoretical physicist; GammaLoop/Vakint author; git identity `ValentinHirschi
<valentin.hirschi@gmail.com>`). Labels used below: **[M]** measured (a run directory, receipt or commit is cited),
**[E]** estimate or interpretation. Nothing in this document is an ETA or a closure claim; `family_closure_claim`
stays false until a ledger drains and the oracles certify it.

---

## 0. Read this first (TL;DR)

1. **Both five-loop campaigns are STOPPED and must stay stopped** until the "ultimate" campaign is ready (owner
   decision, 2026-09-27). v2 (`campaigns/five-loop-qcd-feynman-d9d10-v2`) paused cleanly at generation 7, the
   interim (`campaigns/five-loop-qcd-feynman-d9d10`) at generation 8 (both exit 4). Their directories are evidence;
   do not modify them.
2. **The goal** (section 1): implement, with NO backward-compatibility constraints, all worthwhile improvements to
   the five-loop saved-owner walk (engine, algorithms, inputs, native algebra, memory), iterate until out of ideas
   for the bottleneck, then tell the owner to start the **ultimate five-loop campaign on 100 cores with a 600 GB RAM
   cap**; the owner launches it himself in Zellij tab `fable_5_1` with the command you give him.
3. **The governing plan** is `docs/research/fable51_next_push_master_plan_2026-09-27.md` (27-agent review, waves
   W0-W5, launch criteria (A)-(G)); the target engine is the v3 **epoch** engine of
   `docs/research/fable51_v3_engine_design_2026-09-27.md`. `GOAL.md` (last section) records the directive.
4. **Where the work stands**: branch `fable_5_1` is at `b15316b9` (pushed). Wave 2 (records sidecar, compact
   queue state, CSR edges, Ready gate, upgrade path) is merged and validated. **W0 (intel/oracles/harnesses) is
   partly done**: oracle, Symbolica audit and falsifier lanes complete; harness, baseline, knobs, intel, census and
   inputs lanes are partial (usage limit hit mid-flight); the combined four-loop run was built partially
   (section 7-8). No W1-W5 work has started. Two W0 results reshape priorities: in-process native inspection scales
   badly (3.75x CPU per native at 96 threads, gate 0.3 FAIL) and the residual-inspection lever is confirmed (gate 0.7
   PASS; 0.33x inspections on the drained hot-owner closure).
5. **Next**: finish W0 (section 9.1), bring the D1-D8 decision memo to the owner, then execute W1-W5.

### 0.1 Addendum 2026-09-27 ~21:20 UTC: directives from the Fable 5.1 audit (binding for every later agent)

The owner had Claude Fable 5.1 audit this handoff: `FABLE_5_1_CRITIQUE.md` (repo root; read its sections 1-5 and 7
before designing, gating or summarizing anything). Its lane-level [M] figures re-derive; its criticism targets the
summary layers and the inherited plan. The following are in force, overriding conflicting text below and in the master
plan / v3 note until those are amended:
1. **Native scaling (critique 2.1).** The remote share of memory fills tracks the slowdown exactly (about 0% at K<=24
   and in node-bound processes, 63% at K=96, 81% interleaved), so "NUMA placement does not help" is true only of
   interleaving. Untested and first in line: in-process per-node replication of read-shared data (owner programs: the
   hot owner's `.rrbin` is 817 MB, the 67 owners 1.28 GB, first-touched by the loading thread; 89.4% of the M1 heap
   sat on node 4). The within-node 1.46x at K=24 survives in p4 (1.42x). W2.0 records the process/placement decision
   in cost order: in-process per-node replicas; node-bound inspector processes; shared-memory snapshot segments. S2
   job/result types are byte-serialized regardless. No "layers interleaved over nodes 4-7" without measurement.
   3.75x is a plausible LOWER bound for an epoch inspector (it adds per-successor lookup work).
2. **Gates in useful-work units (THR-2, RAM-1, BASE-1, SOUND-2).** "Native work" = natives x per-class K=1 cost (or
   instructions in native frames), never raw inspector-thread CPU; add "inspector CPU per native <= 1.3-1.5x the
   harness K=1 value" and an IPC clause to P-IMP and launch (B). P-IMP ">= 5x" names its matched window and
   denominator (M1 run2 full window with its slice spread). Memory gates use the MARGINAL RSS/domain fitted over the
   second half of every >= 40-min pilot plus restore VmHWM within the guard, not the import-dominated average.
   "Closure trajectory no worse" means per-root open-cone counts at matched natives.
3. **Work volume is first-class (TERM-1/2).** Add launch criterion (H): pending growth per completion and discovered
   domains per native at matched discovered domains vs the comparator. Every lever gate reports scheduled domains,
   peak pending, pending growth per completion and new points per native, not only inspector-seconds. Schedule an
   early env-gated D-only G2' falsifier on the legacy engine (merged-Native anchors, residual re-inspected, anchor
   edges; D-only cuts need <= 2 pieces, no C2) on C-5F and C-HOT-sub r1a12 within the 1-h rule, and a Route-side
   coverage census (Route is 74% of domains, 72% of native-pending). Present D2 as a three-way choice (none / G2' /
   general union) with the census numbers (pending Apply fully covered: 55.4% by gen-7 natives vs 79.5% by all
   domains).
4. **Errata (use the corrected values).** Union-only residual inspections: C-5F 0.521x, C-HOT-sub r1a12 0.423x,
   r2a11 0.394x, C-HOT 0.328x (0.346x/0.288x are the theta=0.4 dense-cell rows). Gen-7 restore 567 s / 598 s incl.
   the 108 s verify (launch to restored 570/600 s). RAM: 0.3 KB/domain is unsupported; measured legacy marginals
   0.5-1.1 KB/domain (late window ~0.5), restore VmHWM 0.64 KB/domain; 600 GB holds ~0.75-1.2G domains. Native scaling
   WAS measured at K=96 (section 11 is stale). C-HOT is audit-only (CP3). ~1e8 positive inclusions (3.8e9 contains()
   calls). Successor events 25.08M/25.46M (28.4M counts admitted successors). There are two C-HOT-sub boxes: r1a12
   (falsify; 1.02M natives; drains at W12) and s2/r2a12 (knobs; 2.12M natives; drains at W48): every gate names its
   box. mimalloc compiled ratios 0.897/0.929/1.002/0.900. The restored-baseline "denser index" reading is refuted.
   The governing notes' 700 GB guard, CPUs 28-177, W150/136 inspectors and the unrescaled 5-20x band are stale.
5. **Inputs.** I1b is DROPPED (four-loop BMW I1b does not drain; five-loop I1b does more work per native; its
   reinterpreted gate cannot fail); it is not brought to the owner. Inputs v4 = I1 (L* owners; a domain-count lever
   bounded to ~16.5% of gen-7 Apply CPU) + I2 (a memory/index lever, ~0 CPU on current evidence) + I4 frontier stop,
   plus an in-flight stop on any Apply edge from an L* owner into a non-L* mask.
6. **Oracles.** Every gate script asserts `verdict == PASS` AND `roots_independently_verified == roots_total`; the
   oracle branch is merged before any W1 gate is accepted. Before any W4.2 code: an independent exact
   "Q subset of residual union anchors" predicate (lattice.rs, independent of C2) and G2' mutations; the F10 reference
   runs with N1/N4 off.
7. **Controls.** four-all is an ADDITION to the per-family FG/BMW/H/X controls, not a replacement: Ordered four-all is
   an identity oracle; Ready four-all is judged statistically (n >= 5), never as a binary drain gate. The five-loop
   controls invert the gen-7 cost mix (owner 011101110111000 is 0.3-1.3% of their Apply CPU vs 64% in gen 7): report
   lever gates per owner class and add aged (gen-7 resume) pilots. Replace the 10% void rule by a mandatory recorder
   (foreign busy CPUs, schedstat run delay, instructions per native) in every metrics file; state n and a noise
   floor for Ready gates.
8. **Epoch design items W2.0 must settle:** F7 vs A2 (amend the design); a per-ID attempt counter (a head that trips
   the RAM guard must not loop save/resume forever); a rolling replay oracle (event-count-only refresh; record
   snapshot versions per job and merge order per epoch); the CP6 cut while merges continue (chunked/COW sections or a
   pause; saves reuse the last completed refresher result); `--epoch-resolve merge` is a diagnostic, not a fallback;
   summaries for ALL IDs (~86 GB at 1G) or live-only plus recompute; first-found bias (prefer sealed/oldest verified
   containers); merge-helper count at 1G; the admission-share projection (18-47% at 1G before a 2.0-2.6x thread
   factor) already exceeds the 25% trigger, so W3.2 moves into W2.
9. **Governance.** D9 for the owner: an MVP ladder with time-boxes and abandon rules: MVP-A = legacy 4a17f9c7 +
   `[profile.campaign]` + mimalloc + I1/I2 inputs + frontier stop + 600 GB guard (still coordinator-bound);
   MVP-B = epoch S2-S4 lockstep + CP6 + kernel (no W3/W4); the full plan. The W2 comparator is the legacy engine with
   the SoA kernel on a 25-min gen-7 resume vs M1 run2 at a matched window; continue W2 only if the S2/S4 skeleton beats
   it by >= 1.5x. Ask the owner explicitly about the launch-(F) items (cpuset exclusivity for socket 1, an ARC cap,
   the host reserve): only the off-pool item was waived. Publish a merge order; "frozen legacy" = binary 4a17f9c7 plus
   strict identity. Write up results before starting new runs.
10. The interim campaign directory holds six refused fresh-start receipts (10:56 and 12:07 UTC, from the owner's
    Zellij session); its paused gen-8 state is intact.
11. **Owner answers (2026-09-27 ~21:35 UTC)** to the audit's governance items: socket 1 stays SHARED at launch (no
    cpuset; record foreign load); NO ZFS ARC cap; host MemAvailable save-and-stop floor 50 GB (was 20 GB); FULL
    PLAN, GATED: continue W2 only if the S2/S4 epoch skeleton beats the legacy + SoA-kernel comparator (25-min gen-7
    resume vs M1 run2, matched window) by >= 1.5x; otherwise fall back to MVP-B, then MVP-A, and report. Launch
    criterion (F) is thereby settled (shared host, no ARC cap, 600 GB cap, 50 GB host floor).
12. **Owner answers (2026-09-28 ~09:40 UTC):** G2' residual anchors ALLOWED (amend S7/plan 3.11: an anchor is any
    merged record whose domain is fully discharged, resolved in merge order; validators, audit and mutations check
    it; D2 settled as dispatch-time G2' in union form); Symbolica: per-CCX replicas only, no Symbolica patch (owner
    may raise the shared-refcount design upstream); I2: rebuild with one coordinate frame per owner and re-gate with a
    total-work cap; D7 always stop; D8 symbolic closure. D6 and D1(b) settled "no" by measurement. Binding
    orchestrator decisions and lane results: `TMP/progress/orchestrator_decisions.md`, `TMP/progress/*.report.json`.

---

## 1. The goal (owner directive, verbatim intent)

- **Implementation goal.** "Keep both campaigns stopped and implement, without backward compatibility constraints,
  ALL your suggestions and improvements. Only when you're absolutely out of ideas to improve the current bottleneck
  (and new ones if any shows up), then tell me to start the 'ultimate' campaign, on 100 cores and with ... RAM."
  RAM cap was then fixed at **600 GB** (the host cannot give 750 GB: MemAvailable 636-692 GB with ZFS ARC 204-349 GB
  and other users).
- **Ambition.** "Think ambitious ... beyond minute local optimizations." A big multi-agent review was done
  (section 5); its master plan is the goal of this push.
- **Deployment.** The final deliverable of this push is: a validated engine + inputs + an exact prepare/start
  command for the ultimate campaign, which the **owner** types into Zellij session `rustred`, tab `fable_5_1`
  (`XDG_RUNTIME_DIR=/run/user/1125`). Never launch, stop, signal or resume a campaign yourself.
- **Physics scope (frozen).** Complete five-loop QCD renormalization class in Feynman gauge: 67 saved owners, 183
  queries (116 physics roots + 67 helpers), D = A - R in {9, 10}; connected owners A <= 16 - V4, R <= 6 - V4 at
  D = 10 (A <= 14 - V4, R <= 5 - V4 at D = 9); 18 factorized owners with nested bounds A <= 24 - V4, R <= 15 - V4,
  D >= 9; 8 non-entry owners at the widest connected boxes. See
  `docs/research/five_loop_qcd_feynman_entry_class_2026-09-26.md`, `FABLE_5_1_five_loop_vacuum_plan.md` section 2.
  Input shapes (helpers, witnesses) may change (section 6.4); the physics class may not.
- **Soundness is non-negotiable.** The walk must remain a complete closure computation: every obligation (Apply and
  Route, every successor) is inspected or aliased to a domain that CONTAINS it; dependency edges are recorded for
  certification; frontiers and errors stay explicit and block certification; pause/resume and RAM-guard
  save-and-stop are supported.

## 2. Owner rules and preferences (all still in force)

Recorded in auto-memory (`~/.claude/projects/-common-dev-rustred/memory/`, especially
`feedback-v3-campaign-freedom.md`, `feedback-autonomy-fable51.md`, `rustred-campaign-conventions.md`,
`user-valentin-physicist.md`) and in `GOAL.md`.

1. Full autonomy for the work; do not ask for approval of engineering steps. Ask only for genuine owner decisions
   (the D1-D8 memo, physics scope) and host actions.
2. Campaigns: never start/stop/signal/resume one; give the owner the command. Both current campaigns stay stopped.
3. No backward-compatibility constraints for the push: new walk semantics (bump `WALK_SEMANTICS_VERSION`), new
   checkpoint format (CP6), new inputs; the `containment_checks` metric may change.
4. **Never implement an own computer-algebra solution.** Before writing any algebraic code, triple-check that the
   need is covered by Symbolica: the vendored copy (`vendor/symbolica` at `953e26e2`, v3.0.0+24, with
   `patches/symbolica/heap-pow-wide-radix.patch` applied in the working tree) AND the latest upstream (dev
   `445b882d`; also main `70375b9e`). The W0 Symbolica lane (section 7) already did this audit for the plan's needs.
5. **Validation**: every change is validated on the **complete four-loop run** as well as on five-loop pilots.
   The owner prefers **one combined run over all four-loop owners** (more representative of the five-loop
   campaign; section 8) in addition to the per-family FG/BMW/H/X runs.
6. **Pilots deliver their intel within ONE HOUR** (restore included); no 10-hour pilots. Extrapolate and label
   estimates.
7. Any CPU may be used and reserved (owner: "you can use any CPU you want, you don't need to ask"). Still never
   touch other users' processes. The host is shared (other users' jobs, e.g. `nfink` postgres/gammaboard, floated
   over 43-63 CPUs of socket 1 during W0).
8. The zroot pool's 2 permanent data errors are accepted as a non-issue (owner). No off-pool target exists.
9. Evidence discipline: measured vs estimated separated; run directories and binary sha256 cited; no ETA; no
   closure claims; small reviewable commits with `git -c user.name=ValentinHirschi -c
   user.email=valentin.hirschi@gmail.com commit`, messages ending with `Co-Authored-By: <model> <noreply@anthropic.com>`;
   push `fable_5_1` to origin after each green milestone.
10. Tooling: `python3` is not on PATH (use `nix develop --command python ...`); `SYMBOLICA_LICENSE` is in the
    environment (license-gated tests silently return without it; confirm with
    `nix develop --command bash -c 'test -n "$SYMBOLICA_LICENSE" && echo license-set'`).
11. Ultracode was on for this session: substantive tasks were orchestrated as Workflow scripts with adversarial
    verification. Keep doing that if available.

## 3. Current state snapshot (2026-09-27, ~19:30 UTC)

### 3.1 Campaigns (stopped; read-only evidence)
| Campaign | Binary | Inputs | Last run | Final checkpoint |
|---|---|---|---|---|
| `campaigns/five-loop-qcd-feynman-d9d10-v2` | 102adcc3 (`TMP/fable51-controls/bin/rustred-102adcc3`) | plan-v3 (`examples/input/five_loop_qcd_feynman_d9d10/queries.json`, sha 2c714860, 54 helpers bounded at A_max) | `runs/20260926T151353.794886Z`, W100 on 28-127, Ready, 18.6 h | CP5 gen 7, paused, 45,889,639 committed, 74,156,033 discovered, 8/67 roots closed, 0 frontiers |
| `campaigns/five-loop-qcd-feynman-d9d10` (interim) | 32fdec09 | plan-v2 (sha 0f7f0a04, 7 helpers bounded) | `runs/20260926T122852.894005Z`, W50 on 128-177, Ready | CP4 gen 8, paused, 35,494,771 committed, 12/67 roots, ~1,300 frontiers |
| `campaigns/five-loop-dependency-closure` (original) | 32fdec09 | A24/R15/D9 | stopped by the owner earlier | CP4 |

Read-only block clones for analysis: `TMP/v2-checkpoint-copy-gen3` (16.7 GB), `TMP/v2-checkpoint-copy-gen6` (41 GB),
`TMP/v2-checkpoint-copy-gen7` (43 GB) with provenance in the `.meta` siblings. Clone again (`cp -r`, instant on ZFS)
before any use; never write into them.

### 3.2 Branch `fable_5_1` (pushed, tip `b15316b9`)
Recent history: `66ede259` wave-2 merge; `1b13efa4` compare fix; `3717d751` profiling note; `8ea26917` handoff §8;
`260b49cc` v3 engine design; `4be9fdf6` root-blocker note; `b977b73e` master plan; `b15316b9` governance (GOAL.md
directive, plan decision log). Gates at 66ede259 (main tree): `cargo fmt --all -- --check` clean; `cargo test
--release --locked --offline -p rustred-app --lib` **777 passed / 0 failed / 6 ignored**; `--test
cli_routed_campaign` 6/6; Python `python -m unittest discover -s examples/python -p 'test_*.py'` **226 OK, 1 skip**
(needs >= 8 CPUs in the affinity for `test_shared_owner_campaign`).

Canonical current binary: `TMP/fable51-controls/bin/rustred-4a17f9c7` (sha256
`4a17f9c7c0447713370a1aed2e54c9a04251106a9183fd1b8a86dc5d1abb395e`, built from 66ede259; `rustred
walk-semantics-version` prints `{"walk_semantics_version":1,"checkpoint_format":"RUSTRED-WALK-CP5","checkpoint_schema":5}`).
`target/release/rustred` in the main tree is that binary. Other binaries in `TMP/fable51-controls/bin/` are
per-track gate builds (listed in the research notes).

### 3.3 Other branches and worktrees (not merged unless stated)
| Branch | Worktree | Content | Status |
|---|---|---|---|
| `fable_5_1-v3-oracle` (3 commits, tip 1453936a) | `.claude/worktrees/fable51-oracle` | W0.2 oracles: extended audit, `rustred walk-verify-closure`, mutation matrix | complete; verifier found gaps to fix (section 7.2) |
| `fable_5_1-v3-symbolica` (0dcd25b0) | `.claude/worktrees/fable51-symbolica` | Symbolica audit tools | complete; recommendation: no upgrade |
| `fable_5_1-v3-widen` (1457e08e) | `.claude/worktrees/agent-ade877816b107b1cf` | W0.9 falsifiers (throwaway G1 widening, dense-cell oracle) | complete; do not merge the engine change |
| `fable_5_1-v3-harness` (9 commits, f2844d5d) | `.claude/worktrees/fable51-csr` | W0.3 re-inspection harness (test-only) | partial |
| `fable_5_1-v3-baseline` (6e082daf) | `.claude/worktrees/fable51-fp` | W0.5 fp build + M1 tools | run1 done, run2 not |
| `fable_5_1-v3-knobs` (03fc9434) | `.claude/worktrees/agent-ab06981cd80007c75` | W0.8 dispatch-order env knob, `[profile.campaign]` | partial (socket-1 section open) |
| `fable_5_1-v3-intel` (43fdec96) | `.claude/worktrees/fable51-py` | W0.1 lens tools under `tools/research/`, W0.4 `idxreplay` | partial |
| `fable_5_1-v3-census` (1df38d59) | `.claude/worktrees/fable51-compact` | W0.7 census tools | partial |
| `fable_5_1-v3-inputs` (9e63088d) | `.claude/worktrees/fable51-inputs` | W0.6 input intel | partial |
| `fable_5_1-c4l-combined` (40dbccd3) | `.claude/worktrees/fable51-c4l` | combined four-loop run (Luthe A4 basis) | partial, unaudited |
| `fable_5_1-root-diagnostics` (886387aa) | (base of harness) | root-blocker diagnostic test (test-only) | complete; merge with the harness |
| `fable_5_1-coord-{1ab,hit,json}` | (reused worktrees) | WIP of the stopped result-identical coordinator tracks | superseded by the epoch design; WIP commits unreviewed |
| `fable_5_1-c2-sidecar`, `-c2-compact`, `-c2-csr`, `-b2`, `-wave2`, `-scale-restore`, `-py` | — | wave-2 tracks | merged into fable_5_1 |

Every worktree has `vendor/symbolica` initialized at 953e26e2 with the heap-pow patch applied (same as the main tree). (Superseded by LC2, merge f36ba878: `vendor/symbolica` is now clean upstream dev `ef0db494` and `patches/symbolica/` is removed; do not re-apply the heap-pow patch.)

### 3.4 Shared infrastructure created this session
- `TMP/fable51-controls/run_control.py`: control runner (families `fg`, `bmw`, `h`, `x`, `five-finite`); writes
  `TMP/fable51-controls/<label>/<family>/`; refuses to overwrite a label.
- `TMP/fable51-controls/resume_control.py`: interrupted-vs-uninterrupted resume control (first binary pauses at a
  committed-domain threshold via the stop file, second binary resumes; optional strict/multiset compare).
- `examples/python/compare_walk_records.py` (strict/multiset; ignores timing keys and, since 1b13efa4, the
  wall-clock closure refresh telemetry `refresh_count`, `refresh_scratch_estimate_bytes`,
  `retained_storage_estimate_bytes`).
- `examples/python/audit_owner_domain_walk.py` (helper-aliased queries accepted; resumed-walk carried attempts; the
  oracle branch adds `--require-closure` etc.).
- `examples/python/ready_resume_control.py` (Ready multi-prefix pause/resume harness).
- Locks: `TMP/locks/build-{0,1,2}.lock` (wrap every cargo build/test: `flock -w 14400 ...`), `TMP/locks/socket1.lock`
  (any >= 24-thread run on CPUs 128-255); memory guard: start heavy work only if `MemAvailable >= 150 GiB`.
- Session scratchpad (ephemeral!): `/tmp/claude-1125/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/scratchpad/`
  holds the review lenses' analysis tools (indexscan, rtool, perfskeptic*, meas/fits.py, inputs_lens, membench, ...);
  the intel lane started persisting them under `tools/research/` on `fable_5_1-v3-intel`. Workflow scripts of this
  session are under `.../scratchpad/mine/` and `~/.claude/projects/-common-dev-rustred/7dfabea8-.../workflows/scripts/`.

## 4. What happened in this session (chronological, with evidence)

1. **Live v2 monitoring (2026-09-26 15:25 → 2026-09-27 09:50).** v2 (W100, 67 inspectors / 32 helpers /
   1 coordinator) stayed frontier-free; roots closed 1 → 8 by 1.45 h, then flat; completions fell from ~4.5M/h to
   ~0.7M/h; pending growth per completion 0.3-1.9 per 30-min window, no trend; RSS reached ~297 GB. The first
   periodic CP5 save (gen 3, 19:17 UTC) was 16.7 GB in 110 s.
2. **Wave 2 of packages C and B** (workflow `wf_6afddf23-7b2`, 78 agents: implement → 3-lens review → 2-vote
   verify → fix per track; integration `wf_bfba6a36-b4d`). Merged at 66ede259:
   - C1 records sidecar: committed records stream to `records-<G>.jsonl` segments; restore cross-checks
     ledger/closure instead of scanning records (`docs/research/fable51_c1_records_sidecar_2026-09-26.md`).
   - C2 compact queue state: 96 B domains, 176 B summary slab, digest-keyed exact index verified on every hit
     (`fable51_c2_compact_state_2026-09-26.md`).
   - C3 u32 CSR dependency edges + append log; pre-save closure scan never cut (`fable51_c3_csr_edges_2026-09-26.md`).
   - B2 Ready multi-prefix resume gate + scheduler review follow-ups (`fable51_ready_multi_prefix_gate_2026-09-26.md`).
   - Production restore-at-scale test (`fable51_restore_at_scale_2026-09-27.md`).
   Results [M]: result-identical to 102adcc3 (four-loop and five-loop Ordered strict); peak RSS -57% (five-loop
   W50), -24..-78% (four-loop); five-loop Ordered traversal -6..-9%; the live gen-3 checkpoint restores in
   11.0 GB vs 142.8 GB live (164 s); CP5 byte-compatible in both directions (`fable51_wave2_profiling_2026-09-27.md`).
3. **Upgrade path** (`wf_5f3508b3-a57`): `rustred walk-semantics-version` probe and
   `production_saved_owner_campaign.py --resume --upgrade-executable NEW` (dry run by default; liveness, format and
   semantics checks; rollback history). Documented in `docs/shared_owner_campaign_driver.md`. Superseded for the
   ultimate campaign (fresh start), but still valid for performance-only binary swaps within a semantics version.
4. **Audit fix** for helper-aliased roots (`wf_0fc8ccc9-610`): the drained single-owner hot pilot audits PASS.
5. **Coordinator relief design** (`wf_f0d92bc5-af0`, `docs/research/fable51_coordinator_relief_design_2026-09-26.md`):
   result-identical levers ranked; later superseded by the epoch design (it optimizes a barrier epoch removes).
6. **Owner stopped both campaigns** (2026-09-27 ~09:50 UTC) and set the push rules (section 2).
7. **v3 engine design panel** (`wf_e71975ae-23a`, 14 agents, `docs/research/fable51_v3_engine_design_2026-09-27.md`):
   recommends the **epoch** policy (section 6.1).
8. **Root-blocker analysis** (`wf_75768a9e-6e9`, `docs/research/fable51_root_blockers_2026-09-27.md`): the four
   roots closed in the interim but not in v2 are blocked by queue order (pending leaves behind 3.8-14.4M
   obligations in id-ordered Ready reservation), not by helper absorption; the interim's earlier closure is not
   causally established.
9. **Grand review → master plan** (`wf_1a2beb24-dc5`, 27 agents: 8 survey lenses, 4 strategies, 12 skeptic votes,
   synthesis, completeness critic with 32 revisions): `docs/research/fable51_next_push_master_plan_2026-09-27.md`.
10. **Owner answers** (2026-09-27): 600 GB cap; CPUs reservable; zroot errors accepted; no own CAS (check latest
    Symbolica); validate on the complete four-loop run; prefers one combined four-loop run.
11. **W0** (`wf_a551d15c-475`) launched with 9 lanes; the usage limit stopped 6 lanes mid-way (section 7).
12. **Combined four-loop lane** (`wf_ad162918-3ac`): Luthe's A4 common basis found and verified; the builder
    committed inputs and an Ordered W24 reference before being cut off; the audit never ran (section 8).

## 5. Key measured findings (what drives the plan)

### 5.1 The bottleneck of the current engine
- **v2 was coordinator-bound [M]**: coordinator wall 66,896 s; ordered_commit 51.4%, preparation 38.0%, dispatch
  2.7%, poll 1.4%, progress_json 0.95%; mean computing inspectors 2.69 of 67 (0.37-0.67 after 9 h); process CPU
  averaged 10.8 of 100 reserved cores. (master plan §1)
- **At production scale the coordinator's CPU is the ordered admission search itself** [M, W0.5 M1 on the
  resumed gen-7 clone, 60-s fp call-graph window, `TMP/w0/baseline/RESULTS.md` §4]: `find_from` + `retire`
  70.9% inclusive, index block scan 51.0%, summary `contains` 20.3%, bit prefilter 9.3%; telemetry, serde_json and
  allocation are <= 6% each (they dominated only the small W50 control). Coordinator duty in that window: ordered
  commit 46.1%, preparation 45.6% (off-CPU waiting for helpers). Admission helpers are memory-latency bound (IPC
  0.64, 50% far-node DRAM fills); inspectors IPC 3.06. The process used ~9 of 100 CPUs.
- **Cost grows with the queue** [M-r, master plan §1]: helper forward checks per request 136 → 2,000-2,850
  (checks ∝ live^1.05); commit µs per record 2.8 → 16.7-31.7; prep µs per batch 199 → 1,866-2,852; cumulative
  wall ∝ N^1.59; W100 v2 and W50 interim converged to the same late throughput (~0.75M/h).
- **Restored state is cheaper than aged live state** [M, M1]: resuming gen 7 with the wave-2 binary ran 1.48x the
  natives/h and 1.75x the obligations/h of v2's last 12 min, with 0.39x commit µs per record and 0.41x checks per
  request (interpretation [E]: denser index rebuilt at restore, or a cheaper dispatch mix). Compare new engines
  against this restored baseline, not v2's late live rate.

### 5.2 Work volume and redundancy (termination is NOT established)
- Pending grows 0.3-2 per completion with no trend; max rank flat at 18; A <= 25 over all descendants [M-r].
  A 5-20x faster engine only reaches the RAM wall sooner (10-250 h at 600 GB [E], master plan §3.8).
- **Massive redundancy** [M]: in the drained hot-owner pilot (C-HOT) 71.1% of Apply inspections (69.3% of Apply
  native seconds) ran on domains already covered by the union of earlier records; only 3.4% of admitted Apply
  lattice points were new; Apply points overlap 19.5x. In v2, 36.9% of inspected domains were later contained by
  a newer admission (loose upper bound on avoidable work). 96.15% of admission requests are containment hits.
- **Union-cover residual inspection is the big lever** [M-off, W0.9 falsifier oracle on the drained C-HOT closure,
  `TMP/w0/falsify/`]: pure union-cover residuals need 1.000x the distinct points and **0.328x the inspections**
  with no out-of-closure points (C-5F 0.346x, C-HOT-sub 0.288x); dense cells add out-of-closure points and are
  worse. So "most of the gain belongs to union coverage (D2/G2'), not dense cells". Cost projection 0.06-0.43x of
  today's Apply seconds [E, wide].
- **Hull widening (G1) is rejected** [M, W0.9]: H leaves a guard frontier; C-5F 0.774x, C-HOT-sub 0.82x/0.70x, none
  reaches the 0.5x gate; the hot owner's cost per point is superlinear (177 → 1,196 µs/pt).
- Route is 78% of natives but <1% of native seconds; owner `011101110111000` carries 64% of native time [M-r].

### 5.3 Scheduling order
- Root closure under id-ordered (FIFO/breadth-first) Ready reservation waits for the global queue to pass each
  dependency level (root-blocker note). **W0.8 knobs [M, `TMP/w0/knobs/RESULTS.md`]**: no dispatch order is
  adopted: support-then-volume cuts mistakes per native 8-94% and natives up to 10.6-14.5% but raises peak pending
  +12..+193% and delays root certification; closure-boost is a no-op; depth-first is refuted (natives x1.9-3.2).
  Priority dispatch in the epoch engine must therefore be designed around peak pending (bounded enqueue depth,
  age bound) and re-measured there.

### 5.4 Native inspection and build
- **[profile.campaign] (fat LTO, codegen-units=1)**: inspector CPU per native x0.855-0.894 on all four four-loop
  families, strict Ordered identity [M, W0.8]. **mimalloc via malloc override** (LD_PRELOAD of nix
  `libmimalloc.so` 3.4.5, which also covers GMP): x0.87-0.93 (H x1.00), +3..+21% peak RSS; campaign+mimalloc
  x0.78-0.86. znver4: nothing; glibc tunables: worse; jemalloc: neutral. The offline cargo cache has `mimalloc
  0.1.52` / `libmimalloc-sys 0.1.49` with the `override` feature (not in Cargo.lock yet).
- GMP/MPFR make 2.27% of heap allocation calls (Rust 97.73%) [M, harness census]; ~7e6 allocation calls per native
  CPU-second.
- Cancellation of native inspections returns within 0.63 ms even for heavy heads (up to 8.8 s) [M, harness §8].
- **Symbolica** [M/src, W0 Symbolica lane, `TMP/w0/symbolica/RESULTS.md`]: do NOT upgrade. Upstream dev (445b882d)
  is the vendored copy + a C-API feature flag (result-identical, no speed change); upstream main drops Symbolica
  export format 5 (all owner programs are format 5) and changes native frames (76 RustRed persistence tests fail).
  Everything N1 needs is in the vendored copy: `Zp64`, `FiniteField<Mersenne64>`, `Integer::to_finite_field`,
  `evaluate_with_coeff_map` (polynomials and rational functions), `Integer::is_prime`, rational reconstruction/CRT;
  RustRed already uses this pattern in `crates/rustred-core/src/foundry/completion/frame/modular/sample.rs:412-441`.
  No Symbolica lock is on the walk's native path.

### 5.5 Memory and checkpoints
- Wave-2 binary: 0.38 KB/domain after restore (gen 3: 11 GB for 28.8M); M1 resume at 74M: 459-478 B/domain with a
  marginal 1,086 B per new domain [M]; old binary 3.7-4.5 KB/domain.
- Restore of gen 7 (74.2M domains, 1.19G edges): 458.8 s (verify 108 s, decode 121 s, validate 338 s incl. 128 s
  accepted-events derivation); VmHWM 47.7 GB [M, M1]. Gen-7 CP5 is 46 GB, 30 GB of it records JSONL.

### 5.6 The combined four-loop run
- Luthe's **A4 common basis** exists (thesis Table A.1 p.111, eq. (2.26)): {k1, k2, k3, k4, k1-k4, k2-k4, k3-k4,
  k1-k2, k1-k3, k1-k2-k3}; roots 1022 (H prism) and 511 (X, K3,3); FG = 1020 inside 1022, BMW = 510 = 1022 AND 511;
  RustRed's own verifier accepted all 108 maps from the Vakint bases [M]. H and BMW programs cannot be relabelled
  (affine ISP maps); regenerate both roots in A4 with `family-candidates` (~1 min per root with 6 workers [M]).
  16 class owners + 508 routes (492 transported); route-dominated like five loops. Details in section 8.

### 5.7 W0 findings so far (details in section 7)
- **Native inspection does not scale in one process** [M, harness session C]: CPU per native at K=96 is 3.75x K=1
  (gate 0.3 threshold 1.3x; four-loop controls 4.6-7.3x); NUMA interleaving has no effect, mimalloc ~3%; four
  node-bound 24-thread processes reach 1.42x. The counting-sink harness is proven identical to real inspections
  (all natives of FG/BMW/H/X and C-5F).
- **Work-volume gate 0.7 PASSES** [M, census]: 70.1% (D-only cuts) / 89.0% (hull) of gen-7 Apply CPU sits in domains
  whose residual against natives merged before dispatch is <= 10% of their points in <= 8 pieces.
- **Index kernel** [M, intel]: on gen-7 at 74M domains, SoA layouts cut CPU per tested candidate 4.8-9.5x; real
  streams resolve 76.6-88.9% of requests in the cheap tiers (k=1..64 MRU); first-found vs min-ID cuts candidates per
  hit 28-41% on BMW/C-5F; candidates per miss grow as live^0.44-0.53.
- **Oracles work** (gate 0.2 PASS with full re-inspection) but have verified gaps (section 7.1).
- **Inputs** [M]: L* = 40 owners; the I1 candidate is diagnostic-clean; I1b is doubtful (four-loop BMW I1b does not
  drain); I2 witnesses cut Route→Route edges 56% in a W6 pilot.
- **Build/order** [M, knobs]: `[profile.campaign]` x0.855-0.894 inspector CPU per native; no dispatch order adopted.
- **Combined four-loop run** [M]: four-all drains with audit PASS at W6/W24 (Ordered identity across widths) but one of
  two Ready W96 runs did not drain within 1 h (unexplained; schedule sensitivity).

## 6. The plan in force (summary of the master plan)

Read the full plan: `docs/research/fable51_next_push_master_plan_2026-09-27.md`. Summary:

### 6.1 Target engine: v3 "epoch" (walk semantics 3, CP6)
- Inspectors run whole native inspections and resolve each successor against an immutable, `Arc`-published index
  snapshot (job-local exact set → self-scope → Local sibling → per-job MRU targets → helpers/orthants → layers,
  first verified container wins → otherwise Miss). One coordinator merges whole inspections in bulk and is the only
  mutator (`EpochState`: append-only CompactDomain arena, immutable per-ID summaries, ExactIndex, 8-byte ledger6,
  live bitset). Serial work scales with inspections and misses, not successors.
- Amendments A1-A10 (verify chokepoint in production for every positive; failure classes; dynamic bucket interning;
  full reverse retirement with block-parallel reverse sets; SIMD kernel encoding; snapshot refresh for long heads;
  binding without workers/split so performance re-splits resume; restore validators; separate helper vs physics
  certification reporting; `--frontier-policy stop` default).
- Layers: 32-entry SoA blocks with u8 lanes, ordered by finite-upper pattern then lexicographic lower corner;
  offline replay showed first-found cuts candidates per hit by 51% and u8 lanes suffice (0 of 74.16M summaries escape).
- Priority dispatch with a bounded enqueue depth (a deep queue turns transfers into re-inspections).
- Closure monitor refreshed off the coordinator; per-root cones feed priority.
- CP6: append-only domains/edges/records; background saves at merge boundaries; the CP5 review follow-ups become CP6
  requirements (manifest self-digest, bounded segment count, change stamp, cleanup, staging sweep, install order,
  capped interval stretch).
- Records become typed binary; no `domains` JSON array in the deliverable (would be ~620 GB [E]); Rust streaming
  audit/verify.

### 6.2 Work-volume lane (from W0, launch-blocking if a signed-off lever projects >= 2x)
- **G2' residual inspection against merged native anchors** (generalizes `W/initial_overlap.rs` D-band anchors):
  a new Apply domain inspects only Q minus the union of Native anchors merged before its dispatch, with an edge to
  each anchor; needs the C2 power vocabulary (min_positive_power, rank lower bound). With an empty residual it is
  union-cover aliasing restricted to merged Native anchors (D2). **The W0.9 oracle strongly supports it** (0.33x
  inspections on C-HOT).
- Conditional: widening allowlist (rejected by W0.9), dense cells (worse than pure union residuals), P-anchors,
  head parallelism (Amdahl <= 2.07x at k=4), factor atlas.

### 6.3 Native levers
- N1 modular zero certificates for "coalesced group numerator not identically zero" using Symbolica's finite-field
  evaluation (section 5.4); exact path on any doubt. N2 allocation-free applied geometry. N3 allocator and build:
  `[profile.campaign]` (fat LTO, cgu 1) + mimalloc with malloc override. N4 cover-first (after the engine).

### 6.4 Inputs (v4; physics scope frozen)
- I1 hybrid helpers: interim shape (rank-bounded, A unbounded) only for owners in L* = L_static ∩ L_obs ∩ L_clean
  (cannot reach the 7 guard-sensitive owners); I1b envelope-sized bounded helpers elsewhere (82% of Apply domains
  in v2-only-bounded owners are A-escapes above the plan-v3 A_max; the blocked roots escape by rank); I2 route
  witnesses minimizing traffic-weighted cancellation support (7.56 → 5.56 [M-r]); I4 frontier policy stop.

### 6.5 Waves and gates (each pilot <= 1 h)
- **W0** intel, oracles, harnesses, work-volume bounds (partly done; section 7). Ends with the D-session.
- **W1** legacy-lane primitives and inputs in parallel with W2: SoA kernel (strict Ordered identity; >= 4x less CPU
  per candidate), native class A (N1, N2, N3), inputs v4 (clean matching diagnostic, 1-h L*-only walk with 0
  frontiers), operations (frontier stop, host-aware RAM guard, flaky-test hardening).
- **W2** the epoch engine: protocol note first (hazards, A1-A10, termination condition), S2 lockstep skeleton
  (byte-identical across W6/W12/W24), S3 CP6 + stop paths, S4 inspector resolution, S5 parallel merge, S6 rolling
  schedule, importer + deliverable + Rust tooling. Exit gate P-IMP (restore of gen-7 import <= 10 min, 40 min at
  W100): >= 60% of inspector threads in native work, merge duty <= 50%, lookup CPU <= 1x native CPU, >= 5x the
  restored-baseline obligations/h, RSS <= 0.6 KB/domain.
- **W3** priority dispatch + enqueue depth, miss-path index if needed, memory tiering/NUMA placement (socket 1 has
  564-594 GB local), refresher with per-root cones, Route micro-batching.
- **W4** collapse work volume: C2 vocabulary, G2' residual anchors (drain at <= 0.5x inspector-seconds, 0
  frontiers, verify-closure extended), N4, conditional levers.
- **W5** iterate until dry (profile → fix largest non-native cost → repeat), 60-min rehearsal (pause/resume, binary
  swap, re-split, RAM-guard stop, kill -9, frontier drill), freeze and hand the owner the launch command.

### 6.6 Owner decisions pending (D-session at the end of W0; defaults in brackets)
D1 what certifies a physics root [helper aliasing with I1/I1b; no piece-level certification]; D2 union coverage
[neither G2' nor general union cover, until W0 bounds; **the W0.9 oracle is strong evidence for G2'**]; D3 C2
vocabulary [follows D2]; D4 widening allowlist / P-anchors [no; widening now rejected]; D5 host [**resolved: 600 GB
cap, CPUs reservable, no off-pool target**]; D6 symmetry canonicalisation [no]; D7 frontier recovery [always stop];
D8 deliverable scope [symbolic closure].

### 6.7 Launch criteria (all required; adapted to the owner's answers)
(A) soundness battery green (oracles, mutation tests, verify-all, canary); (B) inspection-bound at scale on P-IMP
and P-FRESH arms (>= 70% of inspector-thread CPU in native work, merge duty <= 50%, lookup CPU <= 0.5x native,
admission share at N = 1G <= 25%); (C) memory: RSS <= 0.5 KB/domain at 74M+, save peak within the guard,
time-to-guard projected at the measured rate for the **600 GB** cap; (D) out of ideas: two consecutive W5
iterations change obligations/h or inspector-seconds per native by < 10%, no non-native symbol group > 15% of worker
CPU, no open lever >= 15% in <= 3 days; (E) rehearsal passes with 0 frontiers; (F) host: 100 CPUs on socket 1
reserved (owner allows), 600 GB cap (the off-pool copy and cpuset-exclusivity items are waived/optional); (G) every
signed-off work-volume lever projected >= 2x implemented and gated.

## 7. W0 status per lane

Three lanes finished (oracle, Symbolica audit, falsifiers); the other six were cut off by the usage limit and are summarized from their artifacts by read-only summarizer agents (workflow wf_0d0acddd-0a2). Every lane writes `TMP/w0/<lane>/RESULTS.md`; branches are listed in section 3.3. Items are quoted from the lane and summarizer reports (labels as given).

### 7.1 W0.2 oracles (COMPLETE, branch fable_5_1-v3-oracle, tip 1453936a)

**Summary.** Gate 0.2 passes. Both oracles pass on every current drained output and fail on every mutation.

What was built:
- **Extended audit (commit 067b4985).** `examples/python/audit_owner_domain_walk.py` now has:
  - a new `--require-closure` flag;
  - an exact integer-set check that every alias is inside its direct representative;
  - an exact check that every partial record's D>=cut slice is inside its anchor (a second pass covers Ready ordering);
  - a per-record check that `accepted_events == stats.events`;
  - frontier/error/refusal parity checks;
  - the F8 rule that no record carrying a frontier or error may be reported descendant-closed;
  - separate helper and physics certification reports.
- **Mutation harness (same commit).** `examples/python/oracle_mutation_matrix.py`.
- **New Rust CLI `rustred walk-verify-closure --command RUN/command.json` (commit ca0403eb).** It reads a CP5 generation raw and checks:
  - the request/queries digest binding and the owner digests; roots come from the query records;
  - that record images equal the saved domain images;
  - F8, re-derived from the records and compared with the saved seal flags;
  - exact alias and anchor inclusion, with their edges;
  - counter parity;
  - closure, re-derived from the saved edges and cross-checked by forward cones per root;
  - F10: every native is re-inspected with every walk lever off. Frontier, error and event parity must hold, and every successor must be contained in a recorded target of its parent or along its alias chain.
- **How inclusion is decided.** An independent interval predicate (`lattice.rs`), cross-checked by brute-force lattice enumeration on small cells. `--mutate` injects six defects: dropped edge, retargeted alias, dropped frontier record, seal with frontier, seal with error, injected false hit.
- **Symbolica check.** I searched vendored 953e26e2 and upstream dev 445b882d three times. Neither has integer-box inclusion or lattice-point tools, so no own algebra was needed beyond the existing reducer.

Calibration, all [M]. Fresh runs used 4a17f9c7 on CPUs 46-63:
- **Complete four-loop run.** FG, BMW, H and X, both Ordered and Ready at W6. Both oracles pass with `--require-closure` and full re-inspection. Every successor is covered, for example 4,714,148 of 4,714,148 on BMW. All helper and physics roots are certified and independently verified.
- **C-5F.** Ordered and Ready at W17. Both oracles pass. 967,621 and 981,125 natives were re-inspected, and 28.4M and 28.8M successors were all covered. The verifier took 315-336 s with 6.7 GB RSS.
- **C-HOT.** The extended audit passes with closure required, in 474 s, re-checking all 1,976,549 aliases exactly. The old FAIL in its `audit.json` came from an older audit.
- **Enumeration cross-check.** Brute-force enumeration found 0 disagreements over about 3.8e9 inclusions.

Mutation matrices (Rust oracle on FG and C-5F, audit on FG): every mutation fails with the expected class, and every baseline behaves as required. I rebuilt the binary from the committed source (f4d1870f) and got identical reports.

Test suites: lib 783 passed, 0 failed, 6 ignored. Python 230 OK, 1 skipped. `cargo fmt` is clean.

Results note: `/common/dev/rustred/TMP/w0/oracle/RESULTS.md`.

**Measurements.**
- [M] C-4L fresh runs, binary rustred-4a17f9c7 (sha256 4a17f9c7c0447713370a1aed2e54c9a04251106a9183fd1b8a86dc5d1abb395e), dirs /common/dev/rustred/TMP/w0/oracle/runs/c4l-{ordered,ready}/{fg,bmw,h,x}. All exit 0 with 0 frontiers. Ordered traversal FG 18.1 s, BMW 46.3 s, H 12.3 s, X 35.2 s; Ready 11.6 / 28.6 / 12.1 / 32.6 s. Natives, Ordered: 98,869 / 147,233 / 24,680 / 46,826
- [M] C-5F fresh runs, same binary, dirs /common/dev/rustred/TMP/w0/oracle/runs/c5f-{ordered,ready}/five-finite. Ordered W17: 283.1 s traversal, 967,621 natives, 1,273,376 records. Ready W17: 196.5 s, 981,125 natives, 1,275,621 records. Both exit 0 with 0 frontiers
- [M] Frontier fixture: FG with fg-unbounded.queries.json, dir /common/dev/rustred/TMP/w0/oracle/runs/frontier-fixture/fg. Exit 4, 124 natives, 85 frontiers. Both oracles give 60/124 roots closed, matching four_loop_helper_bounds_2026-09-25.md
- [M] Extended audit with --require-closure (reports /common/dev/rustred/TMP/w0/oracle/audits/*.json): PASS on all 8 C-4L runs (1.3-9.8 s each) and on C-5F Ordered (150.1 s) and Ready (95.8 s). C-5F re-checked 305,755 and 294,496 aliases exactly, with 0 exact-vs-enumeration disagreements over about 258K enumerated checks each
- [M] C-HOT extended audit (/common/dev/rustred/TMP/w0/oracle/audits/chot.json; result sha256 dd3f1dc12b6519f5b83b70a73079b06c5b349470b6b2f72d72f55ca1c0a2fd1e, binary 32fdec09): PASS in 474.5 s. 7,767,543 records, 1,976,549 aliases re-checked exactly. 1 helper root is certified and 2 physics queries are absorbed into it and certified. The unmodified pre-lane audit also PASSes (248 s, /common/dev/rustred/TMP/w0/oracle/chot-audit-baseline)
- [M] walk-verify-closure with full re-inspection and --require-closure, verifier binary rustred-89558210 (sha256 89558210dda816d38aad2c60052ccff87f4905122fd495bbd10fdc9d94e5a62f), 18 threads, reports /common/dev/rustred/TMP/w0/oracle/verify/*.json. PASS on all 10 drained outputs. Successors covered: FG 2,182,549/2,182,549; BMW 4,714,148/4,714,148; H 2,358,487/2,358,487; X 4,117,969/4,117,969; C-5F Ordered 28,406,786/28,406,786; C-5F Ready 28,823,703/28,823,703. Verifier wall time: 6.7 / 14.0 / 10.0 / 25.0 / 314.8 / 336.0 s. C-5F max RSS 6.7 GB
- [M] Exact inclusion predicate vs brute-force enumeration: 0 disagreements over 1,829,889,567 (C-5F Ordered) plus 1,901,533,293 (C-5F Ready) plus about 102M (C-4L) enumeration-checked inclusions
- [M] Rebuilt verifier from commit ca0403eb itself: rustred-f4d1870f (sha256 f4d1870f388d0054445dbb6dd49514c84af8bc8fd4cd095b7a8c96067423b2f4). All 11 reports in /common/dev/rustred/TMP/w0/oracle/verify-f4d1870f are identical to the 89558210 reports in every field except timing
- [M] Mutation matrix /common/dev/rustred/TMP/w0/oracle/mutations/matrix-fg.json, matrix-fg-f4d1870f.json and matrix-c5f.json, all_ok true. Rust oracle: dropped-edge -> successor_uncovered (1,242 on FG, 7 on C-5F); retargeted-alias -> alias_containment; dropped-frontier-record -> frontier_parity x2; seal-with-frontier -> seal_parity; seal-with-error -> seal_parity + error_parity + false_closure (1,319 nodes on C-5F); injected-false-hit -> successor_uncovered. Audit: the 5 applicable kinds all FAIL with the expected message
- [M] Test gates: rustred-app release lib suite 783 passed / 0 failed / 6 ignored (/common/dev/rustred/TMP/w0/oracle/lib-suite.log). Python suite 230 OK, 1 skipped. New unit tests: Rust 6, Python 4. cargo fmt clean
- [E] At gen-7 scale (74M domains, 1.19G edges) the in-memory verifier needs about 40-60 GB. Sample mode (--reinspect sample:N:SEED) covers the at-least-1e4-natives production check; a full run needs the W2.6 streaming port

**Gates.**
- PASS 0.2 PASS on all current drained outputs: complete four-loop run C-4L (FG/BMW/H/X, Ordered and Ready) — /common/dev/rustred/TMP/w0/oracle/audits/c4l-*.json (audit PASS with --require-closure) and /common/dev/rustred/TMP/w0/oracle/verify/c4l-*.json (walk-verify-closure PASS with --require-closure and full re-inspection, 0 uncovered successors, 0 brute-force disagreements)
- PASS 0.2 PASS on C-5F (1,324-tuple five-loop control) — /common/dev/rustred/TMP/w0/oracle/audits/c5f-{ordered,ready}.json and /common/dev/rustred/TMP/w0/oracle/verify/c5f-{ordered,ready}.json: both PASS, 967,621 and 981,125 natives re-inspected, all 28.4M and 28.8M successors covered
- PASS 0.2 PASS on C-HOT (existing drained pilot output, helper-aware re-audit) — /common/dev/rustred/TMP/w0/oracle/audits/chot.json: extended audit PASS with --require-closure; 1,976,549 aliases re-checked exactly; helper root certified; 2 physics queries absorbed and certified. The edge-based verifier cannot read its CP3 state (open issue)
- PASS 0.2 FAIL on every mutation (dropped edge, retargeted alias, dropped frontier record, seal with frontier, seal with error, injected false hit) — /common/dev/rustred/TMP/w0/oracle/mutations/matrix-fg.json, matrix-fg-f4d1870f.json and matrix-c5f.json, all_ok true: every mutation gives FAIL with the expected violation class; unmutated baselines PASS, and the frontier fixture FAILs once closure is required
- PASS Per-commit gates (cargo fmt check, release lib suite, Python suite) — fmt clean; lib 783/0/6 (/common/dev/rustred/TMP/w0/oracle/lib-suite.log); Python 230 OK, 1 skipped

**Implications.**
- W1.1, W1.2 and W2.1-2.5 gates can cite 'audit --require-closure' plus 'rustred walk-verify-closure --require-closure' with full re-inspection. Each takes at most about 6 min on C-5F and seconds on C-4L, well inside the 1-hour rule. oracle_mutation_matrix.py is the regression test for the oracles themselves.
- Checking closure alone is not a sufficient oracle. A dropped or retargeted load-bearing edge leaves the re-derived closure 'closed'; only the F10 re-inspection catches it. W2 gates should require F10 re-inspection on controls, not just closure counters.
- lattice::Cell (exact interval inclusion) agrees with the walker's DomainPowerSummary::contains and with enumeration everywhere tested. W1.1 kernel differential tests and W4.1 C2 property tests can use it as an independent reference.
- W2.2 (CP6) should add a distinct-edge count to each record. The verifier can then check out-degrees (F10 item not implemented here). W2.6 should port the verifier to streaming and add 'epoch' policy support.
- W4.2 (G2'): the partial-record path (anchor covers D>=cut exactly, residual re-inspected, anchor edge required) is the template for 'anchor scopes plus residual cover Q exactly'. General union cover will need multi-target point coverage, which is not implemented.
- C-HOT is only an audit baseline, not an edge-verified control, because its state is CP3. This supports gating on C-HOT-sub unless an epoch run drains C-HOT in 45 min or less.
- D-session: no new decision inputs. D1(b) piece-level certification would need a per-piece closure notion that neither oracle provides.

**Open issues.**
- C-HOT edge-based verification is unavailable. Its checkpoint is CP3 (state-*.bin, schema 3, written by 32fdec09) and the CP5 reader refuses it. A re-run does not fit the 1-hour rule: 3.7 h at W6 with coordinator duty 0.80.
- The verifier trusts the CP5 transport codecs and the native reducer. Its independence covers seal, inclusion, closure and coverage logic only; defects inside the native matcher are left to the N1/N2 multiset gates.
- Records carry no distinct-edge count, so the F10 out-degree check on restore is not implemented (CP6 record-schema change, W2.2).
- The Python audit cannot see edges. Its closure requirement re-checks engine annotations for consistency plus global exhaustion; edge-based claims need walk-verify-closure.
- Neither oracle supports the 'epoch' policy (semantics 3) yet; both accept ordered and ready.
- The verifier holds all domains and the edge CSR in memory (6.7 GB at 1.27M domains). Gen-7-scale use needs sample mode now and the W2.6 streaming port later.
- Calibration wall times were measured while other agents' cargo builds ran on the same CPUs 46-63. They are upper bounds, not benchmarks.

**Adversarial verification of the oracles (2 verifiers, both 'sound' for gate 0.2 with full re-inspection; the fix round was cut off, so these are OPEN):**

- Blocking for gate use, demonstrated [M]: the verifier reports PASS, exits 0 and marks every root 'certified' when `--require-closure` is used without full F10 re-inspection. Only `independently_verified` shows the gap, and nothing enforces it. Runs used binary rustred-f4d1870f on CPUs 46-63, reports in /tmp/claude-1125/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/scratchpad/oracle-review/. - `--reinspect none --require-closure --mutate dropped-edge` on C-4L FG gives verdict PASS; all 248 queries are certified; independently_verified is 0. - `--reinspect sample:10000:1 --require-closure --mutate injected-false-hit` on FG also gives PASS, with all 248 queries certified. - `--reinspect none --mutate dropped-frontier-record` on the frontier fixture gives oracle_closed 62/124 instead of 60. The FAIL comes only from the global counter. A frontier hidden in both the record and the counter would therefore give 2 falsely closed roots. - The lane recommends sample mode (at least 1e4 natives) for the gen-7 production check. At 74M domains that catches a single bad node with probability of about 1e-4. - Fix: `--require-closure` should require every root to be independently_verified, or give a distinct non-PASS verdict when re-inspection is partial.
- Mutation-matrix adequacy: a row passes when the expected violation class is merely present (`klass in classes` in oracle_mutation_matrix.py). - For dropped-frontier-record and seal-with-error, `frontier_parity` and `error_parity` are raised twice: once by the global counter check and once by the per-node F10 re-inspection check (e.g. matrix-fg/mutation-dropped-frontier-record.json). - The per-node check is the only one that catches a frontier or error hidden consistently in both record and counter. If that check broke, the matrix would still report all_ok. - No mutation models the consistent case (drop the frontier and also decrement the counter).
- No mutation asserts that a root's re-derived closure flips: - Every mutation run omits `--require-closure`. - The frontier-fixture baseline only requires at least one unclosed root, not the measured 60/124. - The transitive part of the closure derivation is guarded only by the graph.rs unit test and the internal forward-cone vs reverse-reachability cross-check. seal-with-error shows propagation (false_closure 1,319 on C-5F), but the matrix does not require it.
- Verifier paths with no mutation or real-data exercise: - **Alias-chain coverage:** `alias_chain_covers` is used 0 times in all 11 calibration reports and has no unit test. - **Rust checks with no mutation and no Rust unit test:**   - partial-anchor containment (Python has a synthetic unit test);   - query-to-root mapping;   - request/owner digest binding;   - per-node event_parity. - `cargo test` has no end-to-end test of `verify()`. Oracle regression depends on the external Python matrix and on run directories in TMP. - My reading of the code finds these paths sound, but a regression in them would go undetected.
- The Python audit's 'injected-false-hit' is a different defect. It retargets a query's inputs[].domain to an earlier non-containing initial record, and the existing query-preservation check catches it ('query ...: upper changed'). The audit cannot see edges, so it can detect neither successor-level false hits nor dropped edges. Together with dropped-edge being n/a, the headline 'Both oracles ... FAIL on every mutation' overstates the audit. Only the Rust oracle fails all six defects as the task defines them.
- The headline 'Both oracles pass on every current drained output' overstates C-HOT. - C-HOT was checked only by the Python audit, which re-checks engine descendant_closed annotations for consistency. - The edge verifier cannot read its CP3 state. - The C-HOT 'certified' counts (1 helper, 2 absorbed physics) are engine-claimed closure plus exact alias containment, not independently re-derived closure. RESULTS §6 says this, but the summary and the gate evidence line do not.
- Scope caveats. These are acknowledged, but they limit what PASS means: - The re-inspection uses the walker's own native visitor and the run's request options (including `route_joint_source_support_pruning`, off in all calibration runs). A lost successor inside the reducer, or an unsound pruning option, would be reproduced rather than caught. - Closure is coinductive: sealed cycles and self-edges count as closed. For example, FG helper roots such as record 124 have cone size 1. Termination and descent are not certified. - The verifier does not bind the checkpoint generation it reads to the result.json that the audit reads. Pairing the two oracles on one run assumes the final checkpoint equals the published result.
- Overstated cross-check figure. The reports say 'about 3.8e9 inclusions' and, in RESULTS section 5, '~3.8e9 real inclusions'. The counter actually counts every contains() call in the linear scan over a parent's targets. C-5F Ordered made 1.845e9 exact checks for 28.4M admits, about 65 per admit. The ratio brute_force_points / brute_force_checks is about 1.04, so roughly 98% of the enumerated checks are non-inclusions settled after one lattice point. Positive inclusions confirmed by enumeration are on the order of 1e8, not 3.8e9. The counts are measured correctly; the label claims more than they show.
- 'Successors covered' is mislabelled. C-5F reports 28,406,786 and 28,823,703, but the tally field is admitted_successors, which counts every Admit effect. That includes the Apply domains routed out of Route-phase natives (successor=false). The actual successor-event counts are 25,077,490 and 25,458,946. On C-4L the two numbers happen to coincide.
- RESULTS section 2 says F10 requires 'frontier, error, event and successor counts' to equal the record's. Successor-count parity is only enforced for Apply-phase natives (verify_closure.rs:748-758), because Route records have no successors stat. Route natives get event, frontier and error parity only.
- Scope of F10 independence. F10's 'reference reducer' is the engine's own inspect_native visitor (inspection.rs inspect_reference) with the reuse cache and pre-admitted orthants removed, run under the run's own request. Native-level policies stay on: --bounded-refinement-axes, and --apply-cell-refinement-max-cardinality when set. For a deterministic visitor, event and successor parity therefore hold by construction. What F10 tests independently is the graph bookkeeping (edges, semantic hits, aliases, partial anchors, orthant hits), not successor generation. Open issue 2 discloses this, but 'exact reducer with every lever off' should be read in that sense. Runs with physical-part subdivision are neither handled nor calibrated.
- The Python audit's 'injected-false-hit' mutation is not a successor false hit. It remaps a query to a non-containing initial record and is caught by the old query-preservation check ('upper changed'), not by the lane's new code; the audit cannot see edges. Only the Rust mutation is a real false hit (a covering edge retargeted to a non-container). The claim 'Audit: the 5 applicable kinds all FAIL' mixes two different defects.
- The C-5F mutation matrix is not all C-5F. Its dropped-frontier-record and seal-with-frontier rows run on the FG frontier fixture, because C-5F has no frontiers. RESULTS says 'same fixture', but the lane summary's 'Rust oracle on FG and C-5F: every mutation fails' overstates C-5F coverage.
- The [E] gen-7 memory estimate of 40-60 GB is probably low. Loaded keeps both the raw CompactDomains and the expanded Domain<N>, whose heap Vecs cost about 500 B or more per domain at arity 15. The raw edge list stays alongside the CSR, and Graph::closed builds a reversed CSR through a temporary Vec<(u32,u32)>: about 24 B per edge at peak, roughly 29 GB at 1.19G edges. My own [E] is about 70-90 GB plus the reducer. Separately, 'sample mode covers the >=1e4-natives production check' is untested: the verifier was never run on the gen-6/gen-7 clones (CP5, arity 15), sample mode does not reduce memory because it still loads everything, and load and preparation time at 74M domains is unmeasured.
- Provenance gaps. The audits (13:17) and the lib suite (13:59) ran on the uncommitted tree, before commits 067b4985 (13:19) and ca0403eb (14:02). The Python suite result '230 OK, 1 skipped' has no saved log. 'Built from commit ca0403eb itself' (f4d1870f) also includes the pre-existing uncommitted vendor/symbolica src/poly/polynomial.rs patch, which is identical to the main checkout's. My re-runs reproduce the claims (see reasoning).
- Minor numeric and presentation points. 'About 258K enumerated checks each': the Ready run had 247,182. In the RESULTS table the Natives column includes partials (records = natives + aliases), so the Natives and Partials columns overlap.


### 7.2 Symbolica capability and upgrade audit (COMPLETE, branch fable_5_1-v3-symbolica, tip 0dcd25b0)

**Summary.** Recommendation: do not upgrade Symbolica for W1. Keep the vendored 953e26e2 with patches/symbolica/heap-pow-wide-radix.patch.

Upstream state:
- Upstream dev (445b882d) is exactly one commit ahead of the vendored copy. That commit, "Move C API behind a feature flag", touches no code RustRed runs.
- The latest upstream code is on main (70375b9e, 2026-09-26). It diverged from v3.0.0 and lacks the 24 dev commits that add poly::reconstruction, which RustRed's semi_numerical discovery needs.
- No newer tag than v3.0.0 exists. The upstream GCD/factorisation speed-ups (branch dev_poly) are already inside v3.0.0, so nothing faster is available upstream.

Capability audit (all [src]):
- N1: everything it needs is already in the vendored copy and unchanged in dev and main: Zp64 and FiniteField<Mersenne64> (p = 2^61-1), Integer::to_finite_field, evaluate_with_coeff_map for polynomials and rational functions, Integer::is_prime, plus rational reconstruction and CRT if ever wanted.
- RustRed already uses exactly this evaluation pattern in foundry/completion/frame/modular/sample.rs:412-441. N1 needs no new CAS code; only its decision rule is RustRed logic.
- N2 is lattice geometry; Symbolica has no counterpart.
- N4 and specialization: no revision offers a multi-variable partial substitution. The existing specialization loop only moves data around Symbolica Integer arithmetic. clear() and reserve() allow scratch-buffer reuse.
- Guard factorisation: factor, gcd and to_multivariate_polynomial_list are unchanged upstream.
- N3: Symbolica's faster_alloc makes mimalloc the Rust global allocator but does not cover GMP's C malloc. The offline-cached mimalloc 0.1.52 has an `override` feature.
- Global state: no Symbolica lock sits on the walk's native path. The licence check is a relaxed atomic load. The global State write lock is taken only when symbols, parsed input or rational-function atoms are created, none of which the walk does.

Trial upgrade to dev 445b882d (the heap-pow patch is still needed there):
- Clean build (--locked --offline) with no API break.
- Suites: app lib 777/0/6 and cli_routed_campaign 6/6. Core lib gave 2838/1/32 on the first run; the failure is a symbol-table race between parallel tests, and two reruns gave 2839/0/32.
- Strict Ordered comparison against rustred-4a17f9c7 shows 0 differing records on the complete four-loop run (FG, BMW, H, X) and on the five-loop finite control C-5F, two repeats each.
- Inspector CPU per native is unchanged within noise.

Trial of the latest upstream (dev plus main, merged locally):
- RustRed compiles unchanged against it, but it cannot run any existing input. Every owner program is Symbolica export format 5, and main only imports format 6, so four-loop FG and C-5F both exit 4 within seconds.
- The core suite fails 76 tests, all in persistence code. RustRed's persistence/native.rs::preflight_native_frame requires a leading byte of 0, and main now writes a format byte of 1.
- Upstream's own heap_pow fix passes RustRed's heap-pow regression tests, so the local patch could be dropped then. Adopting main would also mean a persistence code change, regenerating or converting every native artifact, and new control baselines, for no speed-up on the walk.

Results note: /common/dev/rustred/TMP/w0/symbolica/RESULTS.md. Tools are committed under tools/research/symbolica_lane/ (commits a64aa267 and 0dcd25b0). The worktree submodule was restored to 953e26e2 with the patch. The trial merge commit is kept as local branch trial-devmain-3272a7fc in the submodule repository, and nothing was pushed.

**Measurements.**
- [M] upstream dev 445b882d is vendored 953e26e2 + 1 commit (Cargo.toml c_api feature, cfg-gated api::cpp); upstream main 70375b9e = v3.0.0 + 39 commits, divergent from dev (merge-base v3.0.0); no newer tag than v3.0.0 (git ls-remote --tags). Evidence: git -C .claude/worktrees/fable51-symbolica/vendor/symbolica log, refs/remotes/upstream/*
- [M] dev445 binary /common/dev/rustred/TMP/w0/symbolica/bin/rustred-dev445-9f2f4c4d sha256 9f2f4c4dfae3fa90b42ce3f496900c4500f89b8a859f16d6b4c48bda7b16cffd, 129,865,208 B, built in 483 s with --locked --offline and no RustRed source change; reference rustred-4a17f9c7 is 130,174,160 B
- [M] dev445 suites: rustred-app --lib 777 passed / 0 failed / 6 ignored; cli_routed_campaign 6/6; rustred core --lib first run 2838/1/32 (catalog roundtrip test race), then two reruns 2839/0/32. Logs: /common/dev/rustred/TMP/w0/symbolica/runs/build-dev445-test-*.log
- [M] Strict Ordered identity dev445 vs 4a17f9c7, 0 differing records and 0 top-level differences, for ref-r2, dev-r1 and dev-r2 each against ref-r1: FG 98,909 records / 98,869 natives; BMW 158,951 / 147,233; H 24,929 / 24,680; X 47,193 / 46,826 (W6, CPUs 128-151); C-5F 1,273,376 records / 967,621 natives (Apply 271,475 + Route 696,146; W50, CPUs 128-177). Evidence: /common/dev/rustred/TMP/w0/symbolica/runs/c4l/*/strict-ref-r1-vs-*.json, runs/c5f/strict-ref-r1-vs-*.json
- [M] Inspector ms per native, ref then dev (dev change vs ref mean): FG 0.3142/0.3139 vs 0.3140/0.3154 (+0.2%); BMW 0.466/0.453 vs 0.479/0.449 (+1.0%); H 1.416/1.499 vs 1.470/1.547 (+3.5%, within the ref repeat spread); X 1.951/1.938 vs 1.968/1.939 (+0.5%); C-5F 0.7975/0.7881 vs 0.7889/0.7856 (-0.7%). Process CPU per native on C-5F: 1.993/1.987 vs 1.970/1.993 ms (-0.4%). Evidence: runs/c4l/native-seconds.json, runs/c5f/native-seconds.json; binaries 4a17f9c7... and 9f2f4c4d...
- [M] Foreign load on the run CPU sets (/proc/stat busy minus own rusage share): C-4L 1.1-5.5% except FG ref-r1 11.5% and H ref-r1 21.3%; C-5F 12.6-31.1% in all four runs. Per plan §7 the C-5F timings and those two C-4L runs are void as timing evidence; identity results are unaffected. Evidence: runs/*/*/*/metrics.json (cpu_set_busy_share, rusage)
- [M] Socket-1 A/B session held socket1.lock 12:46-13:14 UTC (28 min, within the 1 h rule). Log: /common/dev/rustred/TMP/w0/symbolica/socket1_ab_session.log
- [M] devmain binary /common/dev/rustred/TMP/w0/symbolica/bin/rustred-devmain-9ad50abc sha256 9ad50abc4498eac380703edbf616121162a7920c6d57a871770331c34821b86c builds in 662 s, unchanged Cargo.lock, no compile error
- [M] devmain on existing inputs: FG exits 4 after 1.0 s and five-loop finite exits 4 after 7.0 s, both with 'Unsupported export format version 5'. Evidence: /common/dev/rustred/TMP/w0/symbolica/runs-devmain/probe/{fg,five-finite}/devmain-r1/stderr
- [M] Owner programs carry Symbolica magic 0x37871367 plus format 05 00 at byte 0x20: four-loop FG/BMW/H/X region-control owners, five-loop finite control owners, and campaigns/five-loop-qcd-feynman-d9d10-v2/inputs/owners (67 files, read only). Checked with xxd
- [M] devmain core --lib 2763 passed / 76 failed / 32 ignored. All 76 failures are persistence paths: foundry::artifact::persistence* 46, source_port 12, persistence::{atoms,family,compare,catalog} 13, artifact two_loop/scope/install 4, terminal_normalization 1. The log has 68 'invalid native atom frame' errors; native_heap_pow tests pass with upstream's fix. Evidence: /common/dev/rustred/TMP/w0/symbolica/runs-devmain/core-suite.log and core-suite-failures.txt
- [E] The 309 KB binary size difference dev445 vs 4a17f9c7 is consistent with the C API module no longer being compiled

**Gates.**
- PASS dev445 builds with --locked --offline, no API break — TMP/symbolica-lane/build-dev445-bin.log: cargo exit 0 after 483 s; Cargo.lock unchanged
- PASS dev445 rustred-app lib suite equals baseline 777/0/6 — TMP/w0/symbolica/runs/build-dev445-test-app.log: 777 passed; 0 failed; 6 ignored
- PASS dev445 cli_routed_campaign — runs/build-dev445-test-cli.log: 6 passed
- PASS dev445 core lib suite — First run 2838/1/32; the failure (catalog roundtrip) is a symbol-table race between parallel tests. It passes alone 3/3 and within persistence:: 77/77; two full reruns of the same binary gave 2839/0/32
- PASS Strict Ordered identity on the complete four-loop run (FG/BMW/H/X, saved-cover envelope) vs rustred-4a17f9c7 — runs/c4l/strict-summary.txt: 12/12 PASS, 0 differing records
- PASS Strict Ordered identity on five-loop C-5F vs rustred-4a17f9c7 — runs/c5f/strict-ref-r1-vs-{ref-r2,dev-r1,dev-r2}.json: PASS, 0 differing records of 1,273,376
- FAIL Timing A/B validity (foreign load <= 10%, plan §7) — C-5F foreign load 12.6-31.1% in all four runs; C-4L FG ref-r1 11.5%, H ref-r1 21.3%; the remaining C-4L runs are 1.1-5.5%
- PASS Every pilot run delivers within one hour, restore included — Socket-1 session 12:46-13:14 UTC; longest single run 380 s
- FAIL devmain (latest upstream main merged into dev) runs the existing four-loop and five-loop inputs — runs-devmain/probe/*/devmain-r1/stderr: Unsupported export format version 5, exit 4
- FAIL devmain core lib suite — runs-devmain/core-suite.log: 2763/76/32; all failures in native persistence ('invalid native atom frame')

**Implications.**
- W1.2 N1: implement it on the vendored API, as foundry/completion/frame/modular/sample.rs already does. Use Zp64 or FiniteField<Mersenne64>, Integer::to_finite_field, MultivariatePolynomial/RationalPolynomial::evaluate_with_coeff_map and Integer::is_prime. No Symbolica upgrade and no CAS code is needed; only the certificate rule and the memo are RustRed logic.
- W1.2 N2 is geometry and outside Symbolica. N4/specialization should keep the current Symbolica primitives and can reuse scratch polynomials via clear()/reserve(); no multi-variable partial-substitution API exists in any revision.
- Skipping from_num_den(..., true), the heuristic-GCD share of the profile, keeps Zero::Yes exact but can weaken Uniform to Conditional. This belongs in the N1/N4 identity gates; it is not a CAS gap.
- W1.2 N3 / W0.8: Symbolica's faster_alloc sets mimalloc as the Rust global allocator inside the symbolica crate and does not cover GMP's C malloc. Choose either it or a RustRed-level mimalloc with the `override` feature (mimalloc 0.1.52 is in the offline cache), not both.
- W1.1-W2 and the launch: stay on vendored 953e26e2 with the heap-pow patch. The dev bump is result-identical but useless. A main-based Symbolica needs a RustRed persistence change (native frame preflight), regeneration or string-based conversion of every native artifact (owner programs, candidate bundles, catalogs), and new control baselines.
- W0.3: Symbolica takes no lock on the walk's native path [src]. Still record futex wait at K=96. Any future inspector-side code that turns a RationalPolynomial into an atom would take the global State write lock (get_or_insert_variable_list, a linear scan).
- D-session: no Symbolica-side decision is needed. If a later upstream dev absorbs main, plan the artifact migration together with any input regeneration (e.g. v4 inputs) rather than separately.

**Open issues.**
- Flaky core test persistence::catalog::tests::arbitrary_exact_expressions_roundtrip_with_deduplicated_values: encode_native exports every symbol registered since an offset, so symbols registered concurrently by other tests leak into the second encoding (1 of 3 full runs at 32 threads). Add it to the W1.4 flaky-test hardening.
- Socket-1 timing A/Bs are contaminated by foreign load (13-31% on CPUs 128-177) even while socket1.lock is held, because runs with fewer than 24 threads may use socket 1. Timing gates need cpuset exclusivity (W0.10) or a foreign-load check.
- The C-5F performance comparison is void by the plan §7 foreign-load rule; only identity is established there. The C-4L timing from the low-load runs shows no change beyond repeat spread.
- Futex/lock wait and allocator share at K=96 were not measured here; they belong to W0.3.
- Upstream main drops format-5 import and changes the atom frame. When RustRed eventually upgrades, persistence/native.rs::preflight_native_frame and its framing tests must change, and all native artifacts must be regenerated or converted.
- The local branch trial-devmain-3272a7fc and scratch worktree /tmp/claude-1125/.../scratchpad/symlane/sym-merge hold the trial merge. The scratchpad is ephemeral; devmain_merge.sh reproduces the merge.

### 7.3 W0.9 falsifiers (COMPLETE, branch fable_5_1-v3-widen, tip 1457e08e)

**Summary.** G1 hull widening is rejected by the 0.9 gate on both conditions, and the pre-registered prediction ("fails on the hot owners") holds.

**(a) G1 widening.** Env-gated, throwaway, at the Apply successor arm only. W(S) is the owner box [0,∞)^N capped by the tight Rmax/Amax/Dmin/Dmax of DomainPowerSummary; no axis is tightened, and S ⊆ W(S) is asserted in release. Both arms of every A/B ran on one binary (bce6772f), with only the env var differing. With the flag off, counts equal 4a17f9c7.
- Complete four-loop run (FG/BMW/H/X, W6 Ordered, 2 repeats):
  - H leaves 1 guard frontier and 24/628 roots open, reproducibly. The frontier point lies outside every exact-closure domain.
  - FG/BMW/X close, but cost 1.48-2.72x the exact inspector record-seconds (H: 4.6x), despite 31-83x fewer Apply natives.
- Five-loop:
  - C-5F (W24): 0.774x.
  - C-HOT-sub (r1a12, W12): 0.820x.
  - C-HOT-sub2 (r2a11): 0.696x.
  - All drain with 0 frontiers, but none reaches 0.5x.
- The hot owner 000011001001011 gets 109-128x fewer inspections, yet its seconds change by 1.44x / 1.09x / 0.97x. Per point, its exact cost rises superlinearly above ~1e3 points (177 → 603 → 1,196 µs/pt).
- An exploratory allowlist (widen every owner except the hot one) costs 1.204x on C-HOT-sub. Widened upstream cells inflate the hot owner's exact work 1.89x: closure growth measured directly.

**C-HOT-sub.** Defined as a hot-owner full orthant with R≤1, A≤12, D free (drains in 14.1 min at W12 on the legacy engine). The secondary is R≤2, A≤11 (16.5 min; same closure as the knobs lane's s1). r3a12, r2a12, r3a11 and r1a13 do not drain in 20 min.

**(b) Dense/sparse level-cell oracle** on the drained C-HOT closure:
- It reproduces the lens's static figures exactly: θ=0.4 gives 590 cells + 4.80M sparse points = 1.245x distinct points.
- In admission order it needs 1.624x the distinct points and 542,005 inspections (0.228x of 2,372,220).
- Pure union-cover residuals with no cells need 1.000x the points and 0.328x the inspections, with no points outside the exact closure.
- The dense cells add 3.47M out-of-closure points, and cell sizes exceed the largest measured domain by up to 21.5x.
- Cost projection 0.05-0.39x [E, wide].

Conclusion: most of the gain belongs to union coverage (D2/G2'), not to dense cells.

**Measurements.**
- [M] Binary /common/dev/rustred/TMP/w0/falsify/bin/rustred-bce6772f sha256 bce6772f1b3a185dc33f12fbb851f87c64978ecddb683f2b786ad4bf43b2436b (source afdea4d4 = fable_5_1 b15316b9 + env-gated g1). Flag-off counts equal 4a17f9c7: FG 98,869 natives / 169,509,549 containment checks; BMW 147,233; H 24,680; X 46,826; C-5F 967,621 natives. Runs: /common/dev/rustred/TMP/w0/falsify/runs/exact-bce6772f/*
- [M] C-4L repeat 1 (G1/exact record-seconds): FG 65.3/33.0=1.98x, BMW 112.3/71.2=1.58x, H 176.5/38.7=4.56x, X 270.8/100.4=2.70x. Apply natives 98,869→1,196, 147,233→2,234, 24,680→2,978, 46,826→1,496. Runs: runs/exact-bce6772f and runs/widen-all-bce6772f/{fg,bmw,h,x}
- [M] C-4L repeat 2: FG 2.03x, BMW 1.48x, H 4.62x, X 2.72x; counts identical to repeat 1 (Ordered). Runs: runs/exact-bce6772f-rep2 and runs/widen-all-bce6772f-rep2
- [M] C-4L H under G1, both repeats: exit 4 'incomplete', 1 local_dispatch_frontier (Unresolved OriginalDenominator rule 64 term 2) at point (0,2,0,8,3,0,0,0,0,0) of owner 1010011100 (A=5, R=13, D=-8), 604/628 roots closed, 39 unresolved domains. None of the 8 exact-closure domains of that owner contains the point (runs/exact-bce6772f/h/result.json)
- [M] G1 raises the max scheduled rank: four-loop 13-14 → 14-16; five-loop 6 → 8
- [M] C-5F (W24 Ordered, CPUs 264-287): record-s exact 1,120.9 vs G1 867.5 = 0.774x; slot-busy 0.794x; traversal 405.3 vs 447.2 s; Apply natives 271,475 → 10,900; Route 696,146 → 784,023; hot owner 59,585 insp / 336.6 s → 465 / 484.5 s (1.44x). runs/{exact,widen-all}-bce6772f/five-finite
- [M] C-HOT-sub r1a12 (W12, CPUs 264-275): record-s 2,047.0 → 1,679.4 = 0.820x; hot owner 67,208 / 1,029.5 s → 547 / 1,120.3 s. runs/{exact,widen-all}-bce6772f-hotsub-r1a12-w12/hot
- [M] C-HOT-sub2 r2a11 (W12, CPUs 276-287): record-s 2,466.9 → 1,717.7 = 0.696x; hot owner 72,602 / 1,165.3 s → 666 / 1,133.0 s. runs/{exact,widen-all}-bce6772f-hotsub-r2a11-w12/hot
- [M] Allowlist arm (G1 on every owner but 000011001001011), C-HOT-sub: record-s 2,464.3 = 1.204x exact; hot owner 109,910 insp / 1,944.5 s (1.89x exact). runs/widen-exclhot-bce6772f-hotsub-r1a12-w12/hot
- [M] Hot-owner cost per point by size decade, C-HOT-sub exact: 221 / 191 / 177 / 603 / 1,196 µs/pt at ~5 / 22 / 330 / 2.5k / 13.7k points. G1: 80 cells at ~22k points cost 10.6 s each (848 of the owner's 1,120 s); 21 cells at 5.2e5 points cost 5.9 s each. runs/*hotsub-r1a12-w12/hot/scale-000011001001011.json
- [M] C-HOT-sub drain times (legacy engine): r1a12 846 s whole / 739 s traversal, 1,020,597 natives; r2a11 993 s / 880 s, 1,167,377 natives. Not drained in 20 min: r3a12 (W24, binary 4a17f9c7), r2a12 (W24), r3a11 (W12), r1a13 (W12). Definition: /common/dev/rustred/TMP/w0/falsify/CHOTSUB.md
- [M-off] Oracle on the drained C-HOT closure (TMP/qcd-feynman-d9d10-pilot-hot-owner/.../run/result.json, sha256 dd3f1dc1…): today 2,372,220 Apply inspections, 2.757e8 inspected points, 1.414e7 distinct (19.50x overlap), 10,232.3 Apply s. θ=0.4 static: 590 cells, 12.80M dense points (3.47M outside the exact closure), 4.80M sparse points, 1.245x distinct. θ=0.4 online: 542,005 inspections (0.228x), 22.95M points (1.624x distinct). θ=∞ (union residual only): 778,525 inspections (0.328x), 1.000x points. θ=0: 1,321 inspections, 5.86x points, 68.1M outside the closure. Output: /common/dev/rustred/TMP/w0/falsify/oracle/levelcells-chot-v2.txt
- [M-off] Same oracle on the exact closures: C-5F 3.60x overlap → θ=0.4 static 1.236x / online 1.586x, 0.346x inspections; C-HOT-sub 9.41x → 1.245x / 1.638x, 0.288x; C-HOT-sub2 10.56x → 1.244x / 1.633x, 0.270x
- [E] Dense-cell cost projection on C-HOT: 0.05-0.39x of today's Apply seconds at θ=0.4; 0.06-0.43x for the union residual alone. Extrapolates cells up to 21.5x beyond measured domain sizes; closure growth not modelled

**Gates.**
- FAIL 0.9 (a): C-4L closes with 0 frontiers — H under G1 leaves 1 frontier and 24/628 roots open in 2/2 repeats (runs/widen-all-bce6772f/h and runs/widen-all-bce6772f-rep2/h). FG, BMW and X close with 0 frontiers.
- FAIL 0.9 (a): C-5F drains at <=0.5x inspector-seconds — Drains with 0 frontiers, but at 0.774x record-seconds (867.5 vs 1,120.9 s) and 0.794x slot-busy (runs/widen-all-bce6772f/five-finite)
- FAIL 0.9 (a): C-HOT-sub drains at <=0.5x inspector-seconds — r1a12: 0.820x (1,679.4 vs 2,047.0 s); r2a11: 0.696x (1,717.7 vs 2,466.9 s); both 0 frontiers and root closed
- PASS Pre-registered prediction: G1 fails on the hot owners — Prediction confirmed: hot owner 000011001001011 seconds 1.44x / 1.09x / 0.97x of exact on C-5F / C-HOT-sub / C-HOT-sub2, with 109-128x fewer inspections; exact cost per point superlinear above 1e3 points
- PASS 0.9 (b): dense/sparse level-cell oracle on the drained C-HOT closure recorded — levelcells-chot-v2.txt: θ=0.4 static 1.245x, online 1.624x distinct points and 0.228x inspections vs 19.5x overlap today; union residual alone gives 1.000x and 0.328x

**Implications.**
- Widening allowlist (W4.4 conditional) and D4 widening: the default 'no' is confirmed. An allowlist cannot be judged owner by owner: excluding the hot owner made C-HOT-sub 1.204x worse than exact, because upstream widened cells inflate downstream exact work.
- Nothing in W1-W3 should rely on a coarse-cell collapse. Under Apply-only G1 the five-loop natives fall only 1.22-1.36x, because Route natives (71-72%) do not fall. The 10^4-10^6-cell premise does not hold on these controls. Route widening was not tested.
- D-session (D2/D3): the oracle puts most of the dense-cell benefit in union-cover residualization. On C-HOT it takes 19.5x overlap to 1.00x and inspections to 0.328x with no out-of-closure points. Dense cells at θ=0.4 only move inspections 0.328 → 0.228x and add 3.47M out-of-closure points. Decide on G2'/union cover with the W0.7 bound. Treat dense cells as optional and not launch-blocking; if they are ever built, restrict them to θ≥0.8-1.0 or to owners without a superlinear top decade.
- C-HOT-sub for W2.5/W3.1/W4 gates: r1a12 (14.1 min at W12) or r2a11 (16.5 min; same closure as knobs s1) per TMP/w0/falsify/CHOTSUB.md. The knobs lane's s2 (r2a12) drains only at W48 on socket 1; at W24 on 24 CPUs it does not drain in 20 min.
- G1-type extra points create real guard frontiers (four-loop H) and raise the max rank by 1-2. This is direct evidence for keeping the exact successor geometry in the epoch engine, and for the A10 frontier-stop policy.

**Open issues.**
- The five-loop A/Bs have one repeat each (the four-loop has two); the margins to the 0.5x gate (0.20-0.32) far exceed the observed ≤23% session drift, which moved both arms together.
- C-5F ran at W24 on CPUs 264-287 instead of W50, because socket 1 was held by another lane. Inspector-seconds do not depend on the worker count [E].
- The oracle's online view uses admission-ID order and counts points of the exact closure only. The piece count of the residuals is unknown, and the cost projection extrapolates cells up to 21.5x beyond measured domain sizes.
- In repeat 1 the G1 X arm shared CPU 287 with the oracle; repeat 2 ran clean and gives the same ratio (2.70x vs 2.72x).
- The worktree's vendor/symbolica has a pre-existing uncommitted src/poly/polynomial.rs change, identical to the main tree's working copy. It was left untouched and is in the build.
- No algebraic code was written. Checked Symbolica vendored 953e26e2 and upstream FETCH_HEAD 445b882d: upstream has numerica Integer::binom, which was not needed because the oracle only counts lattice points in a standalone tool.

### 7.4 W0.3 native re-inspection harness

**State.** Partial, but further along than RESULTS.md shows. Last committed tip: f2844d5d on fable_5_1-v3-harness (6 lane commits 8cd6fcda..f2844d5d, on top of fable_5_1-root-diagnostics 886387aa; not merged into fable_5_1). All harness code is committed; the worktree's only uncommitted changes are vendor/symbolica (byte-identical to the main tree, so not from this lane) and an untracked target-fp/. RESULTS.md (TMP/w0/harness/RESULTS.md, last written 15:14 UTC, untracked) has finished sections for the differential proof, the gen-7 sample, the C-allocation census, cancellation latency and the stream table. It still has 7 unfilled markers: STATUS_PLACEHOLDER, FP_BINARY_PLACEHOLDER, GEN7_CROSSCHECK_PLACEHOLDER, SOCKET1_PLACEHOLDER, PROFILE_PLACEHOLDER, STREAMS_PLACEHOLDER and IMPLICATIONS_PLACEHOLDER. The socket-1 scaling session C ran by itself after the agent stopped writing (lock held 16:45:26 to 17:42:56 UTC). It finished 26 of 27 planned runs (TMP/w0/harness/sessions/C/session.log), and nobody had written those results up. The numbers below marked "derived here" were computed in this summary from those receipts and perf files. Sessions A and B never got the socket-1 lock and were terminated (EXIT 143); their directories are empty.

**Key findings.**
- [M] Differential proof, finished. The counting/digest sink reproduces the real walk's streams exactly: FG 98,869, BMW 147,233, H 24,680 and X 46,826 natives identical with 0 mismatched; C-5F 980,945 identical (274,010 Apply + 706,935 Route). Negative control: 96,635 mismatched and 2,234 identical. Evidence: TMP/w0/harness/diff-A/{bmw,h,x,five-finite}/reinspect/receipt.json, diff-A/fg/reinspect-2/receipt.json (the first FG attempt diff-A/fg/reinspect failed on a missing tap path, fixed in 2f007352), diff-A/fg/negative-control-no-initial-orthants/receipt.json. Binary A sha256 e168c1f03382b6209d7b1cbd23230c223fcdbc58b60bb2b1c812fc8b7872034e.
- [M] Gen-7 stratified pending sample, finished: 20,572,079 native-pending IDs; 9,000 sampled (seed 20260927); restore 744 s at peak RSS 47.7 GB. Evidence: TMP/w0/harness/fixtures/gen7-pending-10k.json and gen7-pending-10k-extract-receipt.json.
- [M] W0.3 gate (CPU per native at K=96 <= 1.3x K=1) FAILS. Matched CPU per native, socket 1, glibc, first-touch, gen-7 sample: K=1 28.48 ms; K=8 1.018x; K=24 1.463x; K=48 2.235x; K=96 3.754x, repeat 3.753x. Counting only natives that ran with >=90% of K busy: K=8 1.005x, K=24 1.398x, K=48 2.144x, K=96 3.655x. Evidence: sessions/C/g-k{1,8,24,48,96}/receipt.json, natives.jsonl, g-k96-r2 (derived here).
- [M] Only Apply natives degrade. At K=96 Apply is 3.754x and Route 0.989x (Route natives average 0.04 ms at K=1). Hot-owner Apply strata are 3.43-3.93x and other Apply strata 3.39-3.52x. By K=1 cost quintile: 1.007, 2.578, 2.857, 3.071, 3.772 (cheapest to heaviest). Evidence: sessions/C/g-k1 and g-k96 natives.jsonl (derived here).
- [M] Four-loop controls are worse at K=96 vs K=1: FG 5.742x, BMW 4.621x, H 5.141x, X 7.343x (all Apply). Evidence: sessions/C/c4l-*-k1, c4l-*-k96 (derived here).
- [M] NUMA placement does not help, and separate processes largely do. Same 96 CPUs (24 per node on nodes 4-7): one first-touch process 3.861x / 3.824x (repeat); one interleaved process 3.827x / 3.838x; four node-bound processes of 24 threads each 1.417x / 1.416x. Wall throughput on those CPUs: 72,000 natives in 99.2 s (s-ft) vs 44.4 s (p4), about 2.2x. Evidence: sessions/C/s-ft, s-ft-r2, s-il, s-il-r2, p4/node*, p4-r2/node* (derived here).
- [M] Allocator at K=96: mimalloc via LD_PRELOAD (override) gives 3.614x / 3.670x vs glibc 3.754x / 3.753x, about 2-4% less CPU per native. Whole-process instructions fall about 10% (1.9248e13 vs 2.1462e13). A K=1 mimalloc run was never made. Evidence: sessions/C/g-k96-mi, g-k96-mi-r2, perfstat.csv.
- [M] The slowdown is stalled cycles, not extra work. Clock is 3.02-3.10 GHz in every run. Whole-process IPC: K=1 3.43, K=8 3.28, K=24 2.29, K=48 1.49, K=96 0.89, p4 2.45. Instructions per native are the same at every K: 2.843e8 +/- 0.02%, derived using a prepare cost of 9.92e11 instructions taken from (p4 - g-k96)/3. Evidence: sessions/C/*/perfstat.csv (derived here).
- [M] Cross-node cache-line transfers appear at K>=48. ls_any_fills_from_sys.far_cache: K=1 2.9e3, K=24 3.2e4, K=48 3.7e8, K=96 8.4e8, p4 1.0e5 (whole process). Far share of DRAM fills: K=96 55%, p4 0.1%. The L2-miss count per run grows about 2x. Evidence: sessions/C/*/perfstat.csv.
- [M] Futex census at K=96 (strace, 4 passes): 73 futex calls, 20 errors, 23.9 s total, so there is no blocking-lock contention. wall/CPU per native is 1.016-1.041 in every run. Evidence: sessions/C/g-k96-futex/strace-futex.txt, receipts.
- [M] Socket-1 CPUs 128-255 have no SMT siblings online, 8 cores per L3 and 32 cores per node, so SMT does not explain the scaling. Evidence: /sys/devices/system/cpu/cpu*/topology. [E] The pattern (1 CCD about 1.0x, 3 CCDs about 1.4x, cross-node about 3.7x, fixed by splitting processes but not by interleaving) points to writes to shared cache lines inside one process: true or false sharing, for example atomic refcounts or lock reader counts on shared reducer or Symbolica state. This was not identified: no perf c2c or call-graph profile at K=96 exists. [E] A small harness artifact cannot be fully excluded: per-thread slots holding the cancel flag and timestamps are packed contiguously in reinspection.rs, but they are written only once per native.
- [M] Robustness to background load: foreign load on the measured CPU set varied from 3% to 32% across runs, yet the K=96 repeats agree to 0.03% (g-k96 31.6% foreign vs g-k96-r2 8.6%). Evidence: procstat.before/after through analyze.foreign_load (derived here).
- [M] Heavy heads: the top 90 of the 9,000 natives carry 76.4% of K=1 native CPU, and the top 9 carry 33.8%. K=1 per-native CPU p50 0.7 ms, p99 0.48 s, max 13.04 s. Evidence: sessions/C/g-k1/natives.jsonl (derived here).
- [M] Heavy-head profile is only flat. Evidence: profile/heavy90-flat-A/flat-workers.txt (K=8, CPUs 64-71 on socket 0, 197,271 samples, binary A without frame pointers). Top self-time symbols: validate_polynomial_on_map 12.08%, Integer::partial_cmp 3.09%, MultivariatePolynomial::replace 2.84%, preflight_fixed_polynomial 2.80%, geometry::project<15> 2.52%, _int_malloc 2.22%. glibc allocator symbols among the listed rows sum to 8.08%, and GMP mpn rows to 3.67%. The dwarf profile (profile/heavy90-dwarf-A) is unusable: categories.txt reports 0 native samples out of 204 decoded, and script.txt contains only loader frames.
- [M] C (GMP/MPFR) share of heap calls, finished: 2.27% of allocation calls and 1.68% of requested bytes, with Rust at 97.73% and 98.32%. Evidence: TMP/w0/harness/census/runs/gen7-k8 and gen7-prepare-only.
- [M] Cancellation latency, finished (K=8 on socket 0): all 7,128 cancelled natives have p50 2.6 us, p99 53.8 us, max 1.20 ms. Natives cancelled after >=100 ms: max 631 us. Evidence: TMP/w0/harness/cancel/gen7-k8-half.
- [M] Successor streams, produced: gen7-pending-9k (9,000 natives), c4l-fg, c4l-bmw, c4l-h, c4l-x, in TMP/w0/harness/streams/ with README.md, STREAMS.md and read_streams.py. streams/summaries.txt holds saved verify output only for bmw, h and x. The 'verified' claim for gen7-9k and fg in RESULTS.md has no saved output.
- [M] Socket 0 vs socket 1 at K=1: 43.95 ms/native on CPU 80 (shared with campaigns) vs 28.48 ms on CPU 128. Evidence: socket0/g-k1-cpu80, sessions/C/g-k1. Socket-0 timings (census, cancel, profile, pilot 38.78 ms/native at K=16) are therefore not comparable with the socket-1 numbers.
- [E] Implication for the plan: the prescribed fallback when W0.3 fails ('NUMA replication and the allocator change') is not supported by this data. Interleaving changes nothing and mimalloc gives about 3%. Even four node-bound processes (1.42x) miss the 1.3x threshold. Whether the production W100 walker loses the same factor is not measured (W0.5 perf would show it).

**Artifacts.**
- Branch fable_5_1-v3-harness commits: 8cd6fcda (harness: stream tap, fixture extraction, K-thread sinks), 689adfba (run/session/differential scripts, malloc census shim, stream reader), a1ceb224 (strace futex mode), 2f007352 (differential.sh tap path fix), 37e5621c (analysis: cancel classes, profile categories, records cross-check), f2844d5d (session_tables.py); diff 886387aa..f2844d5d = 19 files, +2855/-1 (walking/reinspection.rs 1,243 lines, tools/research/harness/*)
- Binary A: /common/dev/rustred/TMP/w0/harness/bin/rustred_app-harness-A-e168c1f03382, sha256 e168c1f03382b6209d7b1cbd23230c223fcdbc58b60bb2b1c812fc8b7872034e (the only harness binary; no frame-pointer binary B exists)
- /common/dev/rustred/TMP/w0/harness/RESULTS.md (untracked, has 7 placeholders)
- /common/dev/rustred/TMP/w0/harness/sessions/C/ (26 finished socket-1 runs + session.log; g-k1-r2-h0 failed with timeout 124 at budget end); sessions/planA.txt, planB.txt, planC.txt
- /common/dev/rustred/TMP/w0/harness/diff-A/{fg,bmw,h,x,five-finite}/ (differential proof, tapped walks, fixtures)
- /common/dev/rustred/TMP/w0/harness/fixtures/gen7-pending-10k.json + extract receipt
- /common/dev/rustred/TMP/w0/harness/streams/ (5 stream datasets, README.md, STREAMS.md, read_streams.py, summaries.txt)
- /common/dev/rustred/TMP/w0/harness/census/, cancel/gen7-k8-half/, profile/heavy90-flat-A/ (usable flat), profile/heavy90-dwarf-A/ (decode failed), pilot/gen7-k16-A/, socket0/g-k1-cpu80/
- /common/dev/rustred/TMP/w0/harness/work/gen7-clone (5.9 GB work dir incl. gen-7 block clone and v2-inputs copy)
- Incomplete frame-pointer build: /common/dev/rustred/.claude/worktrees/fable51-csr/target-fp/ (only .rcgu.o objects, cgu.00 is 0 bytes, last write 15:32 UTC, no linked test binary)

**Gate status.**
- W0.3 gate (CPU/native at K=96 <= 1.3x K=1): FAIL [M]. 3.754x on all natives (3.655x at >=90% busy), sessions/C/g-k96 vs g-k1. Repeat 3.753x. Four-loop controls 4.6-7.3x. Best configuration measured: four node-bound 24-thread processes at 1.417x, which still fails.
- Plan consequence as written (NUMA replication + allocator before W2 exit) triggers, but the data shows interleaving has no effect and mimalloc gives about 3% [M]. This remedy needs re-deciding [E].
- Differential correctness of the harness: PASS [M] on 5 controls, and the negative control detects a wrong reconstruction.
- K=1 base has one run only. Its repeat (g-k1-r2-h0) timed out when the session budget ran out, so the base is unreplicated [M].

**Remaining work.**
- Write the session C results into RESULTS.md (SOCKET1_PLACEHOLDER; sections 4-6). The lane's tools/research/harness/session_tables.py is too heavy for the full session: I killed it after about 11 CPU-min at 8.8 GB RSS with no output. Either optimise its concurrency sweep or run it per run directory, pinned off the campaign CPUs (nice -n 19 taskset -c 100-127).
- Re-run the K=1 base on socket 1 (g-k1-r2-h0 and the unrun g-k1-mi-h0) under the socket-1 lock so the gate ratio and the mimalloc effect at K=1 are replicated. About 6 min each.
- Root-cause the in-process scaling loss: perf c2c (or perf record on a frame-pointer build) at K=24 and K=96 on socket 1, to find the shared written cache lines (Arc/RwLock counters, Symbolica global state, reducer caches, and as a control the harness's packed per-thread slots). Then test a per-CCD multi-process variant (12 x 8 threads) to see whether it reaches <=1.3x.
- Finish the frame-pointer binary B: the target-fp build in the worktree was interrupted. Then take the inclusive heavy-head profile (Q7) that fills PROFILE_PLACEHOLDER and FP_BINARY_PLACEHOLDER. The dwarf profile's perf script decode also needs fixing (0 native samples classified).
- GEN7_CROSSCHECK_PLACEHOLDER: extract an inspected-sample fixture (option added after binary A, so it needs a rebuild) and run records_crosscheck.py against the gen-7 CP5 records.
- Save read_streams.py verify output for gen7-pending-9k and c4l-fg (only bmw, h and x are in streams/summaries.txt), fill STREAMS_PLACEHOLDER, and hand streams/ to W0.4 and F10.
- Bandwidth in GB/s was not recorded: only DRAM fill counts and IPC were. Add uncore/DF bandwidth counters if the plan still needs them.
- Fill STATUS_PLACEHOLDER and IMPLICATIONS_PLACEHOLDER. State the gate FAIL and the finding that separate processes help while NUMA and the allocator do not. Move the conclusions into a docs/research note (RESULTS.md is untracked TMP), then decide with the user whether to merge fable_5_1-v3-harness (it also carries 3 root-diagnostics commits not on fable_5_1).
- Feed the gate outcome into the W3.3 split decision and the throughput-band rescaling: every W96 throughput projection assumes near-linear scaling, and it measures about 3.7x worse per native.

**Caveats.**
- Numbers marked 'derived here' were computed in this summary from the lane's raw receipt.json, natives.jsonl and perfstat.csv under sessions/C. The lane agent never reviewed or wrote them up.
- Instructions per native rest on one estimate: prepare costs 9.92e11 instructions, taken from the p4 minus g-k96 difference divided by 3. The constancy across K (2.843e8 +/- 0.02%) holds under that estimate.
- perf stat covers the whole process including the roughly 86 s prepare, so the IPC figures are whole-process values. The K=1 section IPC would be a little lower, and the K=96 ratio estimate from cycles is about 3.85x, consistent with the CPU-time ratio.
- The K=96 ratio includes pass tails. The >=90%-busy subset gives 3.655x, a little lower than all natives, because heavy natives are front-loaded by the cost order.
- Four-loop K=96 ratios compare 20 passes against a single K=1 pass. The natives are short (0.3-1.8 ms), and harness overheads per native were not separated.
- All harness measurements use the counting sink in isolation: no admission, queue or publication. Transfer to the production walker at W100 is not measured [E].
- Census, cancel, profile and pilot runs were on socket 0, which is shared with live campaigns (for example 21% foreign load in the cancel run). Their timings are not comparable with socket-1 timings.
- vendor/symbolica shows as modified in the worktree, but the file is byte-identical to the main tree's pre-existing modification. It is not lane work.
- I did not run, build or edit anything. I ran only read-only Python analyses, pinned and niced. I stopped my own run of session_tables.py because it grew heavy.

### 7.5 W0.5 production baseline M1

**State.** Partial, but most of the data exists. The branch tip is 6e082daf. It is local only, not pushed and not merged. The worktree is clean apart from the known vendor/symbolica working-tree patch. Finished: the fp build, C-4L validation, four-loop attribution, the M1 run1 analysis, and RESULTS.md (written 13:56Z). Gate 0.5 is recorded as PASS on run1. M1 run2 then ran to completion from 15:27:04Z to 16:04:28Z: a full 25-min traversal (1500.9 s), harness exit 0, stop_reason run_seconds_elapsed. It recorded early and late fp windows, a DWARF window, perf stat early and late, and a numa census. The agent stopped before analysing run2. There is no run2/analysis.json and no attr-*.json in run2. RESULTS.md section 7 still reads "run2 (queued) ... will be replaced by its results", so that section is out of date: run2 did run. The run2 numbers below come from this summary pass. I ran the lane's analyzer on a scratch copy with --skip-perf and two None-guards added. The TMP tree was not written.

**Key findings.**
- [M] fp binary: /common/dev/rustred/TMP/w0/baseline/bin/rustred-fp-7eed68fc, sha256 7eed68fcafed333afb7370b3c4561c9bb9b20fbc984ebe608b60e2a42213df7f, built from fable_5_1-v3-baseline @ b15316b9 with -C force-frame-pointers=yes and line-tables-only. The agent reports that crates/, Cargo.lock and vendor are identical to 66ede259, which built 4a17f9c7. Provenance: bin/rustred-fp-7eed68fc.provenance.json.
- [M] C-4L identity of fp vs 4a17f9c7 (TMP/w0/baseline/c4l/summary.json, c4l/compare/strict-*.json, verified in this pass): 8/8 Ordered strict comparisons PASS with 0 differing records; 24/24 audits PASS with 0 violations. Native Apply counts are identical: FG 98,869, BMW 147,233, H 24,680, X 46,826. Ready multisets: FG PASS; BMW, H and X FAIL on shapes. This is informational only, since Ready order is non-deterministic, and the native counts agree within 0.1%.
- [M] Frame-pointer overhead (c4l/summary.json timing): the fp/ref Ordered traversal ratios are FG 1.009, BMW 0.990, H 0.994, X 1.002. Repeat spreads are 0.7-12.4%, so the overhead is below the spread. [E] fp numbers therefore stand for 4a17f9c7 within that precision.
- [M] Four-loop coordinator attribution (c4l-perf/<family>-rdy-fp/attribution.json): named share is at least 99.95%, and 89.0-92.5% of samples inside the traversal loop map to duty buckets. Telemetry (set_parallel_lean and snapshot_lean) takes 15-27% of loop samples and is untimed on the Events path.
- [M] M1 run1 (TMP/w0/baseline/m1/run1/analysis.json, spot-checked in this pass): gen-7 block clone resumed at W100 (67 inspectors, 32 helpers, 1 coordinator) on CPUs 128-227. The measured window is only [T, T+720 s], because the harness crashed in pick() when the DWARF window started. The socket-1 lock was then lost, and the knobs lane's W48 run overlapped for about 2.6 min after the cut. Timeline: restored at 570 s, first traversal heartbeat at 680 s. Internal restore took 458.8 s (verify 108.4, decode 121.0, validate 337.8).
- [M] run1 coordinator duty over 720.4 s: ordered_commit 46.1%, preparation 45.6%, untimed 2.4%. Rates: 6.76 us per prepared record, 1,195 us per batch, 1,014 checks per request. Throughput: natives 1.20M/h, committed domains 2.06M/h, discovered 3.92M/h, native pending +237,934. Initial roots closed stayed at 8. RAM: 459 B/domain at 74.16M discovered, 478 B/domain at 74.94M; marginal 1,086 B/domain (fit, R2 0.94); VmHWM 47.7 GB. Foreign load on CPUs 128-227 averaged 43.3 busy CPUs (p90 59, max 63), so the CPUs were not exclusive.
- [M] run1 threads over 712.6 s: the coordinator used 0.53 CPU, the helpers 6.76 CPUs and the inspectors 1.66 CPUs, about 9 of 100 CPUs in all. perf stat (run1/perf-stat-early.csv) gives IPC of coordinator 1.06, helpers 0.64 and inspectors 3.06. Helper far-node DRAM share is 50.3%.
- [M] Gate 0.5 as scored on run1 (run1/attr-early2-coordinator.json, 12,283 samples, 60 s fp window at T+286 s): named share 1.0 (verified); duty buckets 94.8% (agent's figure); ordered_commit 88.8% of the coordinator's user samples. Inclusive shares: AggregateIndex retire 30.8%, find_from 23.2%, index block scan 51.0%. The agent recorded Gate 0.5 as PASS against the 80% threshold. [E] This perf attribution covers on-CPU user time only. The off-CPU preparation wait (about 45% of wall) is covered by the duty timers, which leave 2.4% untimed.
- [M] run1 vs v2's last 720 s before the gen-7 state (from v2 events.jsonl, as computed by the agent): run1 was 1.48x on natives/h and 1.75x on obligations/h, and 0.41x on checks/request. The interpretation 'restore rebuilds a denser index' is labelled [E, unverified] in RESULTS.md. [E] The run2 time series below weakens that interpretation, because checks/request climbs back toward v2's 2,465 during the run.
- [M] M1 run2 completed (TMP/w0/baseline/m1/run2/, launcher log m1/run2.launcher.log, same binary sha256, fresh gen-7 clone m1/checkpoint-run2). Launch to restored 600.4 s; to first traversal 710.5 s; stop request at 2211.4 s; exit 25.0 s later, after a gen-9 final save of 13.5 s. A gen-8 save of 5.65 s followed the restore. Internal restore took 490.1 s (verify 107.9, decode 124.3, validate 365.8). Exit code 4 is the expected paused/incomplete result.
- [M] run2 over [T, stop], 1500.9 s (computed in this pass from run2/heartbeats.jsonl with the lane analyzer, --skip-perf): coordinator duty ordered_commit 48.1%, preparation 43.6%, dispatch 2.3%, untimed 2.5%. Rates: 8.36 us per prepared record, 1,292 us per batch, 1,179 checks per request. Throughput: natives 1.30M/h, committed domains 2.27M/h, delegated 0.97M/h, discovered 3.61M/h, native pending +392,837. Initial roots closed stayed at 8 throughout.
- [M] run2 drift inside the run (5-min slices from T; the last slice is 297 s long): natives/h went 1.04M, 1.12M, 1.09M, 1.72M, 1.52M, and checks/request went 1,055, 1,205, 802, 1,742, 1,523. Window values: early (T+180 s) 0.98M natives/h at 993 checks/request; late (T+21 min) 1.88M natives/h at 1,650 checks/request. [E] Throughput is not stationary over 25 min, and a baseline for P-IMP comparison should quote the full-window figure together with its spread.
- [M] run2 RAM: 459 B/domain at 74.16M discovered, 479 B/domain at 75.66M. Marginal: 847 B/domain from the fit after T+60 s (R2 0.94, 1,429 points), or 1,469 B/domain from the endpoints. VmHWM 47.7 GB. Host MemAvailable never fell below 717 GB. numa_maps at T+23 min (run2/numa.json, 8.84M pages): 89.4% on node 4, 7.0% on node 6, 3.3% on node 5, and 99.97% on socket 1. [E] The heap is concentrated on one NUMA node, which fits the far-node DRAM shares below.
- [M] run2 threads over T to stop (1,495.9 s): the coordinator used 0.55 CPU, helpers 7.21 CPUs, inspectors 1.66 CPUs. perf stat early/late (run2/perf-stat-early.csv, perf-stat-late.csv): coordinator IPC 0.86/0.91, far-node share 40.7%/26.8%; helpers IPC 0.49/0.64, far-node 57.4%/55.5%; inspectors IPC 2.74/2.78.
- [M] run2 foreign load on CPUs 128-227 averaged 33.4 busy CPUs (p90 40.4, max 68.4; 32.4 before launch). The CPUs were not exclusive again. By the plan's A/B void rule (above 10%) neither run qualifies as an A/B arm.
- [M] run2 perf data exists but is unanalysed. fp windows: coordinator early 12,612 samples and late 12,636; helpers early 18,926 and late 22,668; inspectors early 2,340 and late 4,461. The DWARF window is thin: coordinator 912 samples, helpers 224, inspectors 3.

**Artifacts.**
- Branch fable_5_1-v3-baseline, local only (no remote contains it). Commits a8aef492 (tools: m1_resume_profile.py, m1_run.sh, m1_analyze.py, perf_attribution.py, c4l_matrix.sh, c4l_summary.py, c4l_perf.py, symnorm.py under tools/research/w0_baseline/), 15cb100c (harness hardening: lock held until the native exits, loop survives exceptions), 6e082daf (attribution classes, run1 --end override)
- /common/dev/rustred/TMP/w0/baseline/bin/rustred-fp-7eed68fc sha256 7eed68fcafed333afb7370b3c4561c9bb9b20fbc984ebe608b60e2a42213df7f (+ .provenance.json)
- /common/dev/rustred/TMP/w0/baseline/RESULTS.md: complete for C-4L and run1. Section 7 (run2) is stale and still says queued.
- /common/dev/rustred/TMP/w0/baseline/c4l/ (summary.json, compare/, 24 run dirs) and c4l-perf/<family>-rdy-fp/attribution.json
- /common/dev/rustred/TMP/w0/baseline/m1/run1/ (analysis.json, attr-early2-coordinator.json, attr-early2-inspector.json, attr-early-admission_helper.json, perf-stat-early.csv, timeline.json)
- /common/dev/rustred/TMP/w0/baseline/m1/run2/: raw data only (heartbeats.jsonl, events.jsonl, samples.jsonl, timeline.json, numa.json, perf-stat-early/late.csv, perf-early/late/dwarf-{coordinator,admission_helper,inspector}.data, sched-*.json, result.json); no analysis.json
- /common/dev/rustred/TMP/w0/baseline/m1/checkpoint and m1/checkpoint-run2: gen-7 block clones advanced to gen 9. Each is 45G apparent and 6.1G on disk. /common/dev/rustred/TMP/w0/baseline/m1/inputs: 1.3G apparent.
- /common/dev/rustred/.claude/worktrees/fable51-fp/TMP/baseline-progress.md: the agent's own progress note, which lists the run2 follow-up
- Ephemeral, from this summary pass: /tmp/claude-1125/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/scratchpad/run2copy/analysis.json (run2 analysis without perf, made with a patched copy of the analyzer in scratchpad/tools/)

**Gate status.**
- Gate 0.5 (baseline recorded, and at least 80% of coordinator time attributed to named callers): the lane recorded PASS on run1 [M], with named share 100% and duty buckets 94.8% in one 60 s early window (T+286 s) at 74.2-74.4M domains. It has not yet been re-checked on run2's early and late coordinator windows, which cover the full 25 min.
- Plan item 0.5 requirements: the 25-min resume at W100 was met by run2 (1500.9 s traversal) [M]. The requirement for 100 exclusive CPUs was not met in either run: foreign load averaged 43.3 busy CPUs in run1 and 33.4 in run2 [M]. perf record with a short DWARF window was captured in run2 but is thin and unanalysed. perf stat is done for run1 early and run2 early and late. RSS per domain above 74M is done for both runs.
- Engine per-commit gates: not applicable, because no engine code was changed; only tools were committed.

**Remaining work.**
- Fix tools/research/w0_baseline/m1_analyze.py on fable_5_1-v3-baseline so it survives samples without VmHWM/VmRSS. run2 crashes at the line `ram["peak_vmhwm_bytes"] = max(hwm) * 1024` with a TypeError on None. Filter None in the hwm list and guard the VmRSS read. Then commit.
- Run the analyzer with perf attribution on run2: `cd /common/dev/rustred/.claude/worktrees/fable51-fp && nix develop --command python tools/research/w0_baseline/m1_analyze.py /common/dev/rustred/TMP/w0/baseline/m1/run2 --perf /nix/store/gyp2si1k1w7jhw8z4xx1bwr2m0pr5445-perf-linux-7.2/bin/perf`. This writes run2/analysis.json and the coordinator, helper and inspector attributions for the early, late and dwarf windows.
- Re-score gate 0.5 on the run2 early and late coordinator windows (named share and duty-bucket share), and compare them with run1's early2 window.
- Replace RESULTS.md section 7 with the run2 results: timeline, restore, duty, throughput including the per-slice drift, RAM and NUMA, threads, perf stat early and late, and attribution. Update the section 0 verdict and the section 5 plan-metric table so they quote the 25-min run2 figures, with run1 as the 12-min cross-check.
- Decide whether the DWARF window (coordinator 912 samples, helper 224, inspector 3) is usable. If it is not, record that as a limitation. A retry would need another socket-1 slot, and the per-user perf mlock budget is 516 KB.
- Resolve or explicitly defer the open [E] question of why the restored process beats v2's late live throughput (1.48x natives/h in run1). run2 shows checks/request drifting from about 1,000 to about 1,650 over 25 min, which bears on the dense-restored-index hypothesis. P-IMP (W0.5 same-clone resume) should be compared against the full-window run2 figures with their spread.
- Persist the results out of TMP, for example as a docs/research note on the branch, and decide on merge or push. The branch is local only, and the plan named fable_5_1-v3-intel for this item.
- Housekeeping: once run2 is analysed and no later lane needs them, release or delete the two advanced clones m1/checkpoint and m1/checkpoint-run2 (6.1G each on disk). They are now at gen 9 and are no longer gen-7 fixtures.

**Caveats.**
- The run2 numbers here were computed by this summary pass, not by the lane agent. I copied the small run2 files to the scratchpad and ran a copy of the lane's m1_analyze.py with --skip-perf and two None-guards added. They are [M] from run2's raw files, but the lane has not reviewed them, and they carry no perf attribution.
- RESULTS.md in TMP is uncommitted. Its section 7 says run2 is queued, but run2 finished at 16:04:28Z with harness exit 0.
- Both M1 runs had 33-43 foreign busy CPUs on the 'exclusive' 100-CPU set (mostly nfink postgres and gammaboard per the agent). They are baselines with that load recorded, not A/B-grade arms.
- run1's measured window is cut at T+720 s by the harness crash. After that cut, the knobs lane's W48 run overlapped the native for about 2.6 min. The agent says the cut excludes this overlap, and the stop time is recorded as a manual override in run1/timeline.json.
- Throughput within run2 is not stationary: natives/h ranged 0.98M-1.88M across windows and slices. Any single-window figure, including run1's 12 min, understates the spread.
- The perf attribution uses cpu-clock:u with perf_event_paranoid 2, so it covers user-mode on-CPU time only. The coordinator is on-CPU about 53-55% of wall; the rest is covered by the duty timers.
- The plan assigns 0.5 to branch fable_5_1-v3-intel, but the lane used its own branch, fable_5_1-v3-baseline.
- No campaign was touched. The v2 campaign directory was only read (request.json and events.jsonl).

### 7.6 W0.8 legacy knobs (dispatch order, build)

**State.** PARTIAL. The last committed tip is 03fc9434 on the local branch fable_5_1-v3-knobs. It has no upstream and is not pushed. Its parent is 8b8a9e69, which sits on fable_5_1 b15316b9.
- The "tooling/results commit" that RESULTS.md mentions was never made. results_tables.py, socket1_session2.sh, strict_pairs.sh and lib_tests.sh are untracked, and ab_table.py and summarize.py have small uncommitted edits.
- /common/dev/rustred/TMP/w0/knobs/RESULTS.md has prose for sections 0-3 and the 4.x/5.x captions. It still contains 9 unfilled placeholder tokens: TABLE_C4L_W6_ORDER, TABLE_C4L_W6_AGE, TABLE_C5F_ORDER, GATE_ORDERS, TABLE_C4L_W6_BUILD_ENV, TABLE_C4L_W6_BUILD_COMPILED, GATE_BUILD, SECTION6 and SECTION7.
  - The first five tables exist as separate files: tables-c4l-w6-order.md, tables-c4l-w6-age.md, tables-c5f-w50-order.md, tables-c4l-w6-build-env.md and tables-c4l-w6-build-compiled.md.
  - The two gate paragraphs, section 6 (socket-1 sessions) and section 7 (tests) are not written.
- These socket-1 sessions have finished:
  - hot-sub selection (session A);
  - C-5F W50 order (session B);
  - C-HOT-sub W48 order (session C1: fifo, sv and cb, n=1 each);
  - C-4L W48 order (40 runs of session D).
- The C-4L W6 closure-boost blocker-cap sweep (ob6-*, 32 runs, 15:20-15:36Z) also finished, but no write-up references it.
- STILL RUNNING at 19:05Z: socket-1 session D-c4l-w48 (bash PID 987089, script socket1_session2.sh). It holds /common/dev/rustred/TMP/locks/socket1.lock, which it took at 18:43:26Z. It is working through the b48 build arms (repeat 2) and then 8 b48-campaignmi runs and 5 b50 C-5F W50 Ordered build arms.
  - Its budget is 3540 s, so it ends by about 19:41Z at the latest [E].
  - The last b50 arms may be skipped or cut short by the budget [E].
  - No agent is left to tabulate its output.

**Key findings.**
- [M] Implementation, in commits 8b8a9e69 and 03fc9434: - The env knob RUSTRED_WALK_DISPATCH_ORDER selects fifo (default), support-then-volume (SV), closure-boost (CB) or depth-first (DF), in walking/delegation/ledger.rs reserve_available plus ledger/order.rs. - RUSTRED_WALK_ORDER_AGE_BOUND adds an age bound; RUSTRED_WALK_BOOST_MAX_BLOCKERS and ..._MAX_CONE are CB overrides. - New dispatch_order telemetry, a RUSTRED_CLOSURE_REFRESH_MIN_INTERVAL_SECONDS override, [profile.campaign] (lto=fat, codegen-units=1) in Cargo.toml, 9 unit tests in ledger/order_tests.rs, and tools under tools/research/knobs/. - Non-FIFO orders are Ready-only and fresh-run-only, and their checkpoints fail restore by design.
- [M] The knob build matches the current engine. Its FIFO/Ordered output is identical to canonical 4a17f9c7: strict compare_walk_records PASS with 0 differing records on FG/BMW/H/X (TMP/w0/knobs/sessions/strict-b6.log, strict-c6.log). smoke-fifo FG reproduces 98,841 natives (sessions/smoke.log).
- [M] Lib suite at 03fc9434 (dirty tree): 784 passed, 2 FAILED, 6 ignored (/common/dev/rustred/.claude/worktrees/agent-ab06981cd80007c75/TMP/lib-tests.log). - Both failures come from the new 'dispatch_order' key breaking frozen event key sets: execution::tests::observe_attaches_the_lean_pool_tier_to_per_domain_events_only (domain_started keys) and execution::tests::per_domain_event_key_sets_are_frozen_and_heartbeats_only_add_pinned_telemetry. - RESULTS.md section 7 does not record this. The order_tests are not among the failures, but the log is a tail -40, so they are not listed individually.
- [M] C-HOT-sub was chosen as queries-s2-a12-r2.json (sha256 49f70c556165481bd2687336c3a5db0e01471e6ed85ecf1f95a8aa7757cb3c14; anchor R<=2 A<=12, plus phys-d10-a12-r2 and phys-d9-a11-r2, one root). - On 4a17f9c7, Ready W48 on CPUs 128-175, it drains in 841.6 s whole with 2,120,746 natives, 0 frontiers, root closed 1/1 and foreign load 0.30 (sessions/A-hotsub-select.log, runs/select-s2-4a17). - The fallback S1 drains in 507.7 s with 1,188,906 natives (runs/select-s1-4a17). - S3 (R<=3) was not run. That it would take over 20 min is [E].
- [M] C-4L W6 orders (tables-c4l-w6-order.md, runs/o6-*, binary b1506cf7, n=2), ratios against FIFO: - SV: mistakes/native x0.920 (FG), x0.101 (BMW), x0.610 (H), x0.065 (X). Peak Unreserved x1.12-2.93. Root AUC x0.138 (H) and x0.314 (X). Natives x0.894 on BMW. - CB: mistakes x0.91-0.98, root AUC x0.99-1.09, effectively a no-op. - DF: natives x3.158 on BMW, mistakes x1.4-12.9. - All runs had 0 frontiers. Audits of the repeat-1 runs PASS (sessions/audit-o6-r1.log).
- [M] C-4L W6, SV with age bound (tables-c4l-w6-age.md, runs/oa6-*, binary 4e40bfdc znver4, n=2): sva2k and sva8k lose SV's mistake cut (FG x1.855 and x5.498, BMW x0.866 and x1.000). Root AUC on H/X stays at x0.093-0.261.
- [M] C-5F W50 orders (tables-c5f-w50-order.md, runs/o50-*, n=2): - SV: natives x0.855, mistakes x0.548, peak Unreserved x2.098, inspector CPU x1.215. - CB: identical to FIFO within 0.2%. - DF: natives x1.936, inspector CPU x1.775. - Audits PASS for fifo, sv and cb repeat 1. The DF audit reads FAIL only because result.json (>2 GB) had been deleted (FileNotFoundError in runs/o50-df-r1/five-finite/audit.json), so DF is effectively unaudited.
- [M] C-4L W6 build A/B, allocator and tunables arms (tables-c4l-w6-build-env.md, runs/b6-*, n=3), inspector CPU ms/native ratio against base: - mimalloc: x0.935 (BMW), x0.922 (FG), x0.899 (H), x0.971 (X). - jemalloc: x0.985-1.056. - tcache: x1.006-1.181. tcache-thresh: x0.962-1.195. - Strict identity PASS for every arm and family.
- [M] C-4L W6 build A/B, compiled arms (tables-c4l-w6-build-compiled.md, runs/c6-*), ratios against release (n=10): - campaign (n=4): x0.855 (BMW), x0.871 (FG), x0.894 (H), x0.889 (X). The FG and BMW min..max ranges are disjoint from release. - mimalloc (n=6): x0.897, x0.929, x1.002, x0.900, with peak RSS +3..+21%. - campaign+mimalloc (n=2): x0.783, x0.817, x0.863, x0.829. - znver4 (n=3): x1.048, x1.058, x1.084, x1.002. - Strict identity PASS for all compiled arms (sessions/strict-c6.log). - By the 0.8 build gate (>=5% less CPU with strict identity), campaign passes on all 4 families at W6. mimalloc passes on 3 of 4 in each session, but H and X disagree between the two sessions.
- [M] C-HOT-sub W48 orders (runs/oh48-*, sessions/C1-hotsub-order.log, binary 4e40bfdc znver4, n=1 per arm). No DF arm and no second repeat were run. I tabulated these with the lane's results_tables.py; the lane agent never wrote them up. - FIFO: 2,119,687 natives, mistakes/native 0.458, peak Unreserved 218,977, inspector CPU 3692 s, wall 797 s. - SV: natives x0.650, mistakes x0.362, peak Unreserved x1.844, inspector CPU x1.046 (ms/native 2.80 vs 1.74), wall x1.017. - CB: natives x0.994, mistakes x0.993, CPU x1.002, a no-op. - All runs had 0 frontiers and root closed 1/1. SV fails the peak-pending condition, and with one root, roots per CPU-hour is x0.956.
- [M] C-4L W48 orders (runs/o48-*, binary 1a8358fc, n=2), which RESULTS.md does not report. I tabulated them with the lane's tool. - On BMW, FIFO at W48 does far more work than at W6: 260,172 natives (259.1k-261.3k), mistakes/native 0.496, against 148k and 0.117 at W6. - On BMW, SV gives natives x0.508, mistakes x0.018, peak Unreserved x0.630, inspector CPU x0.551, root AUC x1.109 and wall x0.526. That passes gate 0.8 on BMW. - On BMW, CB gives natives x0.714, mistakes x0.560, peak x0.643 and CPU x0.743, but its repeats spread from 157.7k to 213.7k natives. - Elsewhere SV fails on peak Unreserved: FG x1.732, H x1.795, X x2.901. CB is a no-op on FG and X (X CPU x1.164) and gives H mistakes x0.724 at peak x0.996. - sva2k is about x0.99 on BMW, so the age bound erases the SV gain. - DF: BMW natives x1.486, mistakes x1.3-4.5. - No o48 run has been audited.
- [M] Closure-boost blocker-cap sweep at C-4L W6 (runs/ob6-*, session c4l-w6-cb, binary 1a8358fc, n=2): default (4096), 512 and 64 caps are all no-ops. Mistakes x0.875-1.450, natives x0.995-1.002, peak x0.94-1.09. Nothing reports this.
- [M] Partial C-4L W48 Ordered build arms from the live session (runs/b48-*, mostly n=1, snapshot at 19:04Z), inspector CPU ms/native ratios against release: - campaign: x0.926 (BMW), x0.948 (FG), x1.167 (H), x0.766 (X). - mimalloc: x0.930, x0.992, x0.892, x0.692, with peak RSS +29..+73%. - znver4: x0.974, x1.028, x1.160, x0.713. - These are too noisy to use. Runs last 5-30 s, and the X release point (3.23 ms) looks like an outlier. There is no strict-identity check for b48 yet, and no b50 (C-5F build) data yet.
- [E] The lane agent's verdict in RESULTS.md section 0 is based only on W6 and C-5F data: no order class is adopted, campaign profile is kept, mimalloc is a second step. The W48 BMW data above is new evidence that FIFO's waste grows with width and that SV or CB can halve natives on some families, while SV's peak-pending excess persists on FG, H, X, C-HOT-sub and C-5F. Under the lane's 'passes on all controls' rule the verdict still holds. That is my reading, not a result the lane produced.
- [M] Some section-0 prose ranges do not match the tables exactly: - DF mistakes are given as 'x1.4-13.9'; the table maximum is x12.854 (FG). - mimalloc is given as 'x0.87-0.93 on FG/BMW/X'; the tables show 0.897-0.929 (compiled) and 0.922-0.971 (env session), with H at 0.899 in the env session. - znver4 is given as 'x0.98-1.08'; the compiled table shows x1.002-1.084. - The tunables are given as 'x1.00-1.20'; the table shows x0.962-1.195.

**Artifacts.**
- Branch fable_5_1-v3-knobs (local only, no upstream): 8b8a9e69 'Ready dispatch-order env knob, order telemetry, campaign profile' (15 files, +1533) and 03fc9434 'starvation age bound for the priority dispatch orders'
- Uncommitted in worktree /common/dev/rustred/.claude/worktrees/agent-ab06981cd80007c75: - untracked: tools/research/knobs/results_tables.py, socket1_session2.sh, strict_pairs.sh, lib_tests.sh; - modified: ab_table.py and summarize.py (peak_unreserved and aged_reservations columns); - untracked build dir target-_C_target_cpu_znver4/ (do not commit); - vendor/symbolica src/poly/polynomial.rs modification. This is the pre-existing local heap-power patch, identical to the main tree's (diff sha256 629752e727eccc3dbb0e53bea1d02ad0ef0eb57c19c206be64cb3d4c3d15e6a4, file mtime 2026-09-26), and predates this lane.
- Binaries in /common/dev/rustred/TMP/w0/knobs/bin/: - rustred-knobs-b1506cf7 (release, sources = 8b8a9e69): b1506cf7bf04509663eddb75ffa7f34dc4c148506bd1c8f2fc297992cd713d0d - rustred-knobs-release-1a8358fc (03fc9434): 1a8358fcff8b2649f5e673e881ca1ad25b35e94a6ccd4b6047b3d2564ac97c4c - rustred-znver4-4e40bfdc: 4e40bfdc274b6f84c3ed0d9b32944412c180bfeb407bc71cdda2fdf1ff6b6f56 - rustred-campaign-b9335604: b933560479af4e565e63fec87a7d1c5a6aed71f89f4eb7d0ff8b784830f45798 - rustred-campaign-znver4-aaf84129 (campaign profile + znver4, built 17:04Z, not in RESULTS section 1 and used by no run): aaf841291600611908507604da319271d75125fceaacdaac7ae3dc452c049446 - rustred-release-knobs-wip-b1506cf7: same bytes as rustred-knobs-b1506cf7. Sidecar .json files record commit and profile.
- Preloaded allocator hashes in /common/dev/rustred/TMP/w0/knobs/preload-libs.sha256: mimalloc 3.4.5 9fc37bbb..., jemalloc 5.3.1 4e7a4582...
- Hot-sub query files in /common/dev/rustred/TMP/w0/knobs/hot-sub/: - queries-s1-a11-r2.json: sha256 40731c2e5141d9e4770e06b95c0d456872d14f7279b87900dad411308a02165e - queries-s2-a12-r2.json: sha256 49f70c556165481bd2687336c3a5db0e01471e6ed85ecf1f95a8aa7757cb3c14 - queries-s3-a12-r3.json: sha256 0fa441114a09bf00c10eb8e614526fd24dd19814393ee459273f12f34e6148ee (never run)
- /common/dev/rustred/TMP/w0/knobs/RESULTS.md (partial; 9 placeholder tokens) and tables-c4l-w6-order.md, tables-c4l-w6-age.md, tables-c5f-w50-order.md, tables-c4l-w6-build-env.md, tables-c4l-w6-build-compiled.md
- /common/dev/rustred/TMP/w0/knobs/runs/ (8.8 GB, 99 run directories): o6, oa6, ob6, o50, o48, oh48, b6, c6, b48 (in progress), select-s1/s2-4a17, smoke, canon-4a17-r1, test-runner-4a17
- /common/dev/rustred/TMP/w0/knobs/sessions/: - session plans and logs: A-hotsub-select, B-c5f-order, C1-hotsub-order, D-c4l-w48 (live), c4l-w6-*; - strict-b6.log and strict-c6.log; - audit-o6-r1.log and audit-o50-r1.log.
- /common/dev/rustred/.claude/worktrees/agent-ab06981cd80007c75/TMP/lib-tests.log (784 pass, 2 fail, 6 ignored) and TMP/build-arms.log

**Gate status.**
- Gate 0.8, orders: NOT ADOPTED on the lane's own criterion (an order must pass on all controls). The GATE_ORDERS paragraph in RESULTS.md is an unfilled placeholder. - SV fails the peak-pending (+10%) condition on C-4L W6 (all 4 families), C-5F W50 (x2.098) and C-HOT-sub W48 (x1.844, n=1). - At W48, SV and CB pass on BMW, and CB also on H (my tabulation, n=2, CB noisy). This is not reflected in RESULTS.md. - DF is refuted everywhere, and the age bound does not repair SV.
- Gate 0.8, build: [profile.campaign] PASSES at C-4L W6, with inspector CPU per native x0.855-0.894 (at least 5% lower) and strict Ordered identity PASS [M]. The GATE_BUILD paragraph in RESULTS.md is an unfilled placeholder. - The W48 C-4L and W50 C-5F Ordered confirmations (b48/b50) are still running or not yet run, and have no strict-identity check yet. - mimalloc passes on 3 of 4 families in each W6 session, but which families differ between sessions. It is pending the W48/W50 confirmation. - znver4, glibc tunables and jemalloc fail.
- Unit/lib tests: FAIL. 2 frozen event-key-set tests break on the new dispatch_order telemetry key (the measurement branch is not proposed for merge, but this must be fixed or recorded).

**Remaining work.**
- 1. Leave session D-c4l-w48 running (PID 987089; it holds socket1.lock and ends by about 19:41Z [E]). Then check its log tail at /common/dev/rustred/TMP/w0/knobs/sessions/D-c4l-w48.log for 'budget exhausted, skipping' lines, which mark b50 arms that were not run.
- 2. Tabulate the finished sessions to stdout with the worktree tools (PYTHONDONTWRITEBYTECODE=1 nix develop /common/dev/rustred --command python tools/research/knobs/results_tables.py ...): - order tables with --reference o48-fifo 'o48-*', --reference oh48-fifo 'oh48-*' and --reference ob6-fifo 'ob6-*'; - build tables with --reference b48-release 'b48-*' and --reference b50-release 'b50-*'.
- 3. Run the strict Ordered identity check (tools/research/knobs/strict_pairs.sh, or compare_walk_records.py) for b48-{znver4,campaign,mimalloc,campaignmi} against b48-release-r1 and for b50-* against b50-release-r1. Also compare b50-release against a canonical 4a17f9c7 C-5F Ordered output if one exists. The build gate requires identity.
- 4. Fill the RESULTS.md placeholders: - splice the 5 tables-*.md files into TABLE_C4L_W6_ORDER, TABLE_C4L_W6_AGE, TABLE_C5F_ORDER, TABLE_C4L_W6_BUILD_ENV and TABLE_C4L_W6_BUILD_COMPILED; - write GATE_ORDERS and GATE_BUILD; - write SECTION6: o48, oh48, b48 and b50 results with n and foreign load; - write SECTION7: lib suite 784/2/6 with the two named failures, the ob6 CB-cap sweep, the DF audit artefact and the unused campaign-znver4 binary. - Correct the section-0 ranges listed in key_findings, and revise section 0 for the W48 BMW result.
- 5. Handle the 2 failing lib tests. Either add 'dispatch_order' to the frozen key lists in crates/rustred-app/src/application/routed_campaign/walking/execution/tests.rs (around lines 642 and 745), or move the telemetry out of per-domain events. Then rerun lib_tests.sh under build-0.lock.
- 6. Optional, if socket-1 time allows: - a second repeat of the C-HOT-sub order arms, plus a DF arm; - audits (audit_run.sh) of the o48 and oh48 repeat-1 runs; - a third repeat of o48 BMW CB to settle its spread (157.7k-213.7k natives).
- 7. Commit the tooling (results_tables.py, socket1_session2.sh, strict_pairs.sh, lib_tests.sh, and the ab_table.py/summarize.py edits) on fable_5_1-v3-knobs. Exclude target-_C_target_cpu_znver4/ and the vendor/symbolica working-tree change. Push only if the orchestration calls for it. Per the W0 rule, nothing from this branch goes into the legacy engine.
- 8. Housekeeping: runs/ holds 8.8 GB (hot-sub checkpoints are about 250 MB each and result.json up to 225 MB). Prune after the write-up if space matters.

**Caveats.**
- Host noise was 7-45% foreign load on every run (another user's postgres and gammaboard processes, plus other lanes' rustc). The plan's 10% void rule was deliberately not applied. CPU numbers come from schedstat on-CPU time of the owner-domain-* threads, and counts are the primary order metrics.
- Arms were compared across different binaries: o6 and o50 used b1506cf7; oa6 and oh48 used the znver4 build 4e40bfdc; o48 and ob6 used 1a8358fc. Each table's reference arm uses the same binary as its arms, but comparing tables (for example BMW FIFO at W6 against W48) crosses binaries. Ready runs are nondeterministic anyway.
- The C-HOT-sub order results are n=1. The C-4L W48 build results are mostly n=1, and the session was still producing data when I read it.
- The W48 order, C-HOT-sub order, CB-cap sweep and partial b48 build figures were never written up by the lane agent. I produced them from its run directories with its own results_tables.py and ab_table.py, printing to stdout only. The measurements are [M]; the gate readings of them are mine.
- Root AUC is 0.000 on the one-root controls (C-5F, C-HOT-sub) because the root certifies only at drain. There, 'roots per CPU-hour' reduces to 1/inspector CPU. C-5F CB shows 'root AUC x5.762' against a FIFO AUC of 0.005; this is an artefact and should be ignored.
- The C-5F DF audit 'FAIL, 1 violation' is FileNotFoundError on a result.json deleted for size. It is not a correctness finding, but DF on C-5F is unaudited. The o48, oh48, oa6 and ob6 runs have no audits.
- RESULTS.md's header marks sections 1-5 and 7 as final, but section 7 and the gate paragraphs in 4.5 and 5.3 are placeholders. The 'tooling/results commit' it cites does not exist.
- The worktree's vendor/symbolica carries an uncommitted patch. It is byte-identical to the main tree's pre-existing patch and not the lane's work, so 'nothing touches Symbolica' holds for the lane's commits. The binaries were built with that patch in place, and strict identity against 4a17f9c7 still PASSES.
- Session D holds the socket-1 lock until about 19:41Z [E], so other W0 lanes queued on socket1.lock are blocked until then.

### 7.7 W0.1 lens tools + W0.4 offline index replay

**State.** Partial. The measurements are complete but the write-up is not. Last committed tip: 43fdec96 on fable_5_1-v3-intel, on top of 055986ba and b15316b9. The branch has no upstream (not pushed) and is not merged. The worktree has uncommitted edits to tools/research/idxreplay/src/{dynamic.rs,stat.rs,util.rs} and an untracked tools/research/idxreplay/project.py. These edits were never built: build-idxreplay-v4.log is empty and target/release/idxreplay is still v3 (sha c39727e0…). /common/dev/rustred/TMP/w0/intel/RESULTS.md was last edited at 14:48Z. It still contains six unfilled markers (DYNAMIC_TABLE, PIPELINE_TABLE, STATIC_TABLE, THIN_TABLE, THR18_TABLE, LAG_TABLE) and two "PENDING" paragraphs (real successor streams; 48/90 threads), although the data for both now exists. The "open issues" section it refers to and a gate-0.4 verdict section are both missing. W0.1 is effectively done. For W0.4, all planned runs finished, but the results are not written up and the gate is not decided.

**Key findings.**
- [M] W0.1 commit 43fdec96 (162 files, +41,689 lines). It adds under tools/research/: lens-rs (18 std-only bins); rtool (cover/union/sum/scale, plus the rtool_owner, deleg and keys bins); 70 Python lenses under py/, including meas/fits.py, inputs_lens/* and perfskeptic/*; membench; the new idxreplay; small receipts under outputs/; and a README mapping each tool to the plan figure it reproduces. Evidence: git show --stat 43fdec96.
- [M] Builds: lens-rs rc=0 (TMP/w0/intel/build-lens-rs.log); rtool rc=0 (build-rtool.log, 14:52Z); idxreplay rc=0 (build-idxreplay.log). The rtool build finished after RESULTS.md was last edited, so RESULTS.md's line 'rtool: see open issues if not yet confirmed' is stale.
- [M] Reproduction. The rebuilt scan2 on the gen-7 clone matches tools/research/outputs/perfskeptic_cwe/scan2_7.out on every count and percentile; only the wall-clock fields differ (I checked with diff). Evidence: TMP/w0/intel/lens-repro/scan2_7.out. Examples: miss 11,200 tested per query with p99 95,756; hit min-ID 7,374 vs first-found 3,633. indexscan gives 8,040 buckets, 37,907,667 live, 1,993,524 blocks, fill 0.594, mean ID gap 696,417 and top bucket 2.9% (lens-repro/indexscan7.out); deg also ran (deg7.out). The other tools (rtool cover/union, fits.py, membench) were not re-run against their committed outputs.
- [M] Commit 055986ba adds a research-only cargo feature, admission-trace (crates/rustred-app, walking/trace.rs), off by default. The trace binary is TMP/w0/intel/bin/rustred-trace-edbe2729 (sha256 edbe2729…). It was built with the same modified vendor/symbolica src/poly/polynomial.rs as the main tree (identical sha 462b4e03…), so there is no lane-specific drift.
- [M] Dynamic validation (replay/dynamic-4l.jsonl, dynamic-5f.jsonl; idxreplay-v3). Six traces were replayed: four-loop FG/BMW/H/X at W6, FG at W1, and C-5F at W18. Across 26,748,231 admissions there are 0 outcome, 0 maintenance and 0 retirement mismatches. RESULTS.md states 27.34M admissions, which is wrong; the sum of the jsonl rows is 26.75M, or 26.32M without the duplicate FG W1 run.
- [M] Forward checks. At W1 the engine and replay agree exactly (FG 19,776,723 = 19,776,723). At W6/W18 the engine totals are 0.04-0.65% below the serial replay (prepared lookups are charged on a snapshot); for C-5F 634,145 records differ. FG W6 containment_checks = 169,509,549, equal to the frozen baseline (TMP/fable51-controls/baseline-32fdec-fg.log). The traced C-5F at W18 gave 986,213 natives, 1,284,040 domains, 0 frontiers, 431 s (TMP/w0/intel/trace-5f.metrics.txt).
- [M] Evolving index, candidates per hit, first-found vs min-ID (replay/t_dyn.md): C-5F 128.4 vs 216.2 (−41%); BMW 170.4 vs 238.0 (−28%); FG/H/X change by less than 3%. The exponent of candidates per request against bucket live size is 0.51-0.82 (R² 0.88-0.99). C-5F forward candidates: 4.374G today → 2.305G after the pipeline (min-ID) → 1.536G (first-found), about 2.85x fewer.
- [M] Resolution pipeline on C-5F and the four-loop runs (replay/t_pipe.md). C-5F cheap-tier share is 55.3/64.6/70.1/72.0% at k=1/4/16/64; four-loop is 73.2-98.7%. C-5F route jobs resolve 56.5%, all via the exact-store probe.
- [M] Pipeline on real gen-7 streams (replay/streams-g7.jsonl, not yet in RESULTS.md). 273,070 joined jobs, 61.6M requests, 80 jobs unjoined. Cheap-tier share is 76.6/84.7/87.5/88.9% at k=1/4/16/64. At k=16 the split is: exact-job 17.0%, self 14.0%, Local 0.9%, MRU 52.4%, exact-store 0.5%, helper 2.6%, layer-hit 11.1%, miss 1.4%. Apply jobs (56,383 jobs, 94.4% of requests) are 92.1% cheap at k=16. Route jobs (216,687 jobs, 5.6% of requests) are 10.1% cheap and 84.7% layer hits. MRU tests per request are 0.59/0.96/1.33/1.80; Local tests per request 1.23.
- [M] Static layouts at 74M, 1 thread (replay/static-g7-1t.jsonl, t_static.md). CPU ns per tested candidate for miss / hit min-ID / hit first-found / reverse / word-only: today's layout (l0-stored) 51.9/55.2/51.7/47.9/29.1; SoA-id 6.6/6.9/6.3/5.0/3.6; SoA-pattern 8.2/7.4/8.1/10.1/5.1. That is 7.9-9.5x less for SoA-id and 4.8-7.5x less for SoA-pattern; RESULTS.md's '6.4-7.9x' is the miss row only. Candidates per request, SoA-pattern vs today: miss 5,133 vs 11,200; hit first-found 2,606 vs 11,468 min-ID; reverse 2,317 vs 11,133.
- [M] Real-stream layer requests at 74M (streams-g7.jsonl, sampled 167,927 hits and 25,679 misses). CPU ns per tested candidate: today 85.3 (hit min-ID) / 71.4 (hit first-found) / 75.2 (miss) / 68.1 (reverse); SoA-id 12.9/8.9/10.1/7.1; SoA-pattern 9.1/10.4/10.1/13.5. Candidates per request for today / SoA-id / SoA-pattern: hit min-ID 2,361/2,129/6,712; hit first-found 1,148/920/584; miss 4,483/3,826/2,585; reverse 12,272/9,939/2,884. CPU µs per request, today min-ID vs SoA-pattern first-found: hit 201.3 vs 6.1, miss 336.9 vs 26.1, reverse 835.5 vs 38.8. Real hits are about 5x easier than the static hit proxies (2,361 vs 11,468 min-ID).
- [M] New and not recorded by the lane: pattern ordering removes min-ID early exit. Under min-ID, SoA-pattern needs 2.8x more candidates per hit than today's layout (6,712 vs 2,361), while SoA-id stays at 2,129. The layout choice therefore depends on whether canonical (min-ID) resolution must be served by the layers (relevant to the W2.3 canonical-mode gate).
- [M] Thinning, 25/50/100% live (t_thin.md). Candidates per miss scale as live^0.44 (SoA-pattern), 0.51 (today, rebuilt) and 0.53 (SoA-id); reverse checks as live^0.45/0.60/0.61; CPU per miss as live^0.46/0.55/0.65. Per-bucket regression over 797 buckets: 0.78 today, 0.78 SoA-id, 0.65 SoA-pattern.
- [M] Threads. 18 threads on CPUs 10-27 (t_thr18.md): today 54.6-86.4 ns per tested candidate, SoA-pattern 9.3-18.7 ns. Per request set, SoA-pattern is 4.0-7.1x cheaper and SoA-id 7.0-7.9x; RESULTS.md's '4.6-7.6x' does not match the per-set ratios. 48 and 90 threads on socket 1 (replay/t_thr_s1.md, static-g7-threads.jsonl; numactl interleave 4-7, CPUs 128-217, 15:18-15:27Z), miss cost for 1/48/90 threads: today 56.3/91.7/109.3 ns, SoA-id 7.4/15.3/19.3, SoA-pattern 9.1/18.4/21.5. At 48/90 threads SoA-id is 5.2-7.2x cheaper on every set. SoA-pattern is 4.5-5.6x cheaper on forward checks, but on reverse checks only 3.7x (48 threads) and 3.3x (90 threads).
- [M] Foreign load, which I derived from TMP/w0/intel/socket1-load.txt (CPUs 128-227). About 41 of 100 CPUs were busy with foreign work during the 48/90-thread sweep and about 52 during the traced resume. The sweep is a single pass with no interleaved repeat and no schedstat, so the 48/90-thread numbers are contaminated and do not meet the §5 A/B protocol.
- [M] Stale misses vs snapshot lag (replay/lag-g7.jsonl, t_lag.md). Over 1,192,281,291 committed edges, in domain-ID units: lag <1,024 misses 1.6-2.0% of hit edges in generations 5-7; lag <65,536 about 3.1%; lag <1M 4.1-4.5%. Over all generations the figures are 1.83/4.18/7.40%. In the stream window, 222,357 of 5,987,195 layer hits (3.7%) target containers admitted after the snapshot (877,287 new IDs).
- [E] From RESULTS.md: at 8-80M discovered domains per hour, a 1 s snapshot lag gives about 2-3% stale misses, and a 179 s heavy head without refresh gives about 4-6%.
- [M] Traced W100 resume of a gen-7 clone (TMP/w0/intel/g7-trace-resume/result.json, run.log, stderr), socket 1 under the lock, 14:49-15:18Z. elapsed_seconds 1,722, of which about 11.5 min was restore and preparation according to the note in g7-manual-stop.sh. It was stopped manually and cooperatively; the scripted stop at 15:04Z did not take effect. Exit status was 4 (paused). run.log's 'exit 0' is a script artefact: $? is read after $(date). Natives went from 27,465,422 to 27,738,371 (+272,949) and admitted domains from 74,156,033 to 75,033,320 (+877,287), with 0 frontiers and peak RSS 46.6 GB. Coordinator: ordered_commit 461 s plus preparation 500 s out of 1,046 s. An earlier attempt at 13:13Z was refused because 100 cores were requested on an 18-CPU mask (attempt1-refused/).
- [M] Engine counters in the stream window (streams-input row of streams-g7.jsonl): 54,831,235 admissions; 53.98G forward checks (49.77G on contained admissions, 4.21G on new ones); 45.43G maintenance checks; 273,150 natives. That is about 198k forward and 166k maintenance checks per native.
- [M] I produced these numbers myself, not the lane agent, by running the lane's own replay/jobsecs.py over the trace job headers. Native seconds: 1,442.5 s over 273,150 records. Apply: 56,427 records at 25.3 ms each; route: 216,723 records at 0.061 ms; overall 5.28 ms per native. That averages about 1.4 busy native workers out of 100. The lane's processes used about 8 CPUs. The traced run was coordinator-bound and ran under foreign load, so its throughput is not a baseline; its request mix is real.
- [E] My provisional gate-0.4(b) projection, not reviewed by the lane. I ran the lane's untracked project.py on streams-g7.jsonl with k=16, native 5.28 ms per native, thread factor 1 and an assumed 100 ns per cheap-tier request. Share of worker CPU spent on admission: SoA-pattern first-found 6.7% at 74M, and 17.9/27.3/36.7% at N=1G for α=0.44/0.65/0.82; SoA-id first-found 9.8% at 74M, 25.0/36.5/47.1% at 1G; today's layout behind the pipeline (first-found) 52.5% at 74M, 77.6-90.3% at 1G. Multi-thread per-candidate costs are 2.0-2.6x the 1-thread values, which would push these shares higher. So the >25% trigger (move W3.2 into W2) is likely unless α≈0.44 holds.
- [M] Gate 0.4(c): the pipeline tiers resolve at least 50% of requests on real gen-7 streams (76.6-88.9%) and on C-5F (55.3-72.0%), so 3.2 does not need a redesign. Gate 0.4(a): SoA-id is at least 4x cheaper per tested candidate in every measured condition (5.2-9.6x). SoA-pattern meets 4x on forward checks everywhere but not on reverse checks at 48/90 threads (3.3-3.7x, under contaminated load). The lane wrote no verdict.

**Artifacts.**
- commit 055986ba (fable_5_1-v3-intel): admission-trace research feature
- commit 43fdec96 (fable_5_1-v3-intel): tools/research/ lens tools + idxreplay; branch not pushed
- uncommitted: /common/dev/rustred/.claude/worktrees/fable51-py/tools/research/idxreplay/src/{dynamic.rs,stat.rs,util.rs} (adds LoadMon foreign-load fields, native_seconds/native_ms_per_job per pipeline row, stale-lag-requests at k=16); untracked tools/research/idxreplay/project.py (gate-0.4 [E] projection); never built
- /common/dev/rustred/TMP/w0/intel/bin/rustred-trace-edbe2729 sha256 edbe272967c2c9fe9d4cbbefd465c1c51a250610ac019ac1815fef2854bc4312
- /common/dev/rustred/TMP/w0/intel/bin/idxreplay-v1 sha256 d39a918301cf0b93c97c47b9d4b0e4061630a08f27c03233c70b1b93a0c9d6f2 (static, lag, thread sweeps)
- /common/dev/rustred/TMP/w0/intel/bin/idxreplay-v2 sha256 28d2f03e2f2bfd48006b86ef8095a300446f1ce2b2273e14616c10cfeb3ee758 (superseded, unused in results)
- /common/dev/rustred/TMP/w0/intel/bin/idxreplay-v3 sha256 c39727e04f93be32c579fb2d3162191ba8f6c21e689f4917b8008e69b7281b01 (dynamic, streams; = worktree target/release/idxreplay)
- /common/dev/rustred/TMP/w0/intel/RESULTS.md (incomplete; 6 *_TABLE markers, 2 PENDING paragraphs)
- /common/dev/rustred/TMP/w0/intel/replay/: dynamic-4l.jsonl, dynamic-5f.jsonl, static-g7-1t.jsonl, static-g7-lane18.jsonl, static-g7-threads.jsonl, streams-g7.jsonl, lag-g7.jsonl, tables t_dyn/t_pipe/t_static/t_thin/t_thr1/t_thr18/t_thr_s1/t_lag.md, fill_results.py (not run), jobsecs.py, pipe_summary.py
- /common/dev/rustred/TMP/w0/intel/lens-repro/ (scan2_7.out, indexscan7.out, deg7.out)
- /common/dev/rustred/TMP/w0/intel/lens-scratch-snapshot/ (byte copy of the review scratchpad)
- traces: /common/dev/rustred/TMP/w0/intel/trace-4l/{fg,bmw,h,x}, trace-4l-w1/fg, trace-5f (~2.8 GB coord + 9 job files), g7-trace-resume/trace (6.7 GB coord + 67 job files); run dirs /common/dev/rustred/TMP/fable51-controls/w0-intel-trace-edbe2729{,-w1,-w18}
- /common/dev/rustred/TMP/w0/intel/g7-trace-resume/{result.json,events.jsonl,run.log,stderr} (traced W100 resume receipt)
- /common/dev/rustred/TMP/w0/intel/socket1-load.txt, socket1-session.log (socket-1 lock session 14:49-15:27Z, 2,274 s)
- fixtures: /common/dev/rustred/.claude/worktrees/fable51-py/TMP/gen7 (read-only gen-7 block clone); .../TMP/gen7-resume (clone mutated by the resume, now CP5 generation 9)

**Gate status.**
- W0.1: done in substance (tools committed, builds rc=0, scan2 and indexscan reproduced exactly). Not every tool was re-run against its committed output.
- Gate 0.4(a), SoA kernel ≥4x less CPU per tested candidate at 74M: [M] met by SoA-id in all measured conditions (5.2-9.6x). SoA-pattern meets it on forward checks; on reverse checks it gives 3.3-3.7x at 48/90 threads under contaminated load. Layout by candidates: SoA-pattern first-found is best on real streams, but SoA-pattern is worst under min-ID. No verdict recorded by the lane.
- Gate 0.4(b), projected admission share at N=1G >25% moves W3.2 into W2: NOT evaluated by the lane. My provisional [E] run gives 17.9-36.7% (SoA-pattern first-found) and 25.0-47.1% (SoA-id), so the trigger is likely. Pending a reviewed run with a thread factor.
- Gate 0.4(c), pipeline resolving <50% forces a redesign of 3.2: [M] not triggered (76.6-88.9% on real gen-7 streams, 55.3-72.0% on C-5F).
- Plan item 0.4, 1/48/90 threads: measured once, under about 41 foreign CPUs on socket 1, without the ≥2 interleaved repeats or schedstat the §5 protocol requires.

**Remaining work.**
- Fill RESULTS.md: nix develop /common/dev/rustred --command python /common/dev/rustred/TMP/w0/intel/replay/fill_results.py. This replaces the six *_TABLE markers with replay/t_*.md.
- Replace the two PENDING paragraphs in RESULTS.md with the real-stream results (replay/streams-g7.jsonl: pipeline rows plus the streams-layer-cost and streams-layer-requests rows) and the socket-1 sweep (replay/t_thr_s1.md), including the foreign-load caveat of about 41 of 100 CPUs.
- Correct RESULTS.md: '27.34M admissions' should be 26,748,231 (26.32M distinct runs); '6.4-7.9x' is the miss row only (7.9-9.5x SoA-id, 4.8-7.5x SoA-pattern across all request sets); the 18-thread ratio is 4.0-7.1x (SoA-pattern), not 4.6-7.6x; remove the stale rtool line. Add the missing open-issues section and a gate-0.4 verdict section, including the SoA-pattern min-ID finding.
- Decide on the uncommitted idxreplay edits. Either build them (bash /common/dev/rustred/TMP/w0/intel/build-tool.sh /common/dev/rustred/.claude/worktrees/fable51-py/tools/research/idxreplay), save the binary as idxreplay-v4 with its sha256, re-run `idxreplay streams --ckpt .../TMP/gen7 --gen 00000000000000000007 --trace TMP/w0/intel/g7-trace-resume/trace --initial 67 --layer-sample 200000` (about 8 min, about 58 GB RSS) so the jsonl carries native_ms_per_job and stale-lag-requests, then commit together with project.py; or revert them and use jobsecs.py for the native ms.
- Gate 0.4(b): run project.py with a thread factor taken from t_thr_s1.md (SoA-pattern 90-thread/1-thread about 2.4, SoA-id about 2.6) and α ∈ {0.44 static, 0.65 per-bucket, 0.69-0.82 dynamic}. Record the result as [E] and decide whether W3.2 moves into W2.
- Optionally repeat the 48/90-thread static sweep on socket 1 with ≥2 interleaved repeats, the LoadMon foreign-load fields (v4 binary) and schedstat run delay, to meet the §5 protocol, and resolve SoA-pattern reverse at 3.3-3.7x against the 4x threshold.
- Persist the W0.1/W0.4 results in git: RESULTS.md lives under TMP and is not tracked. Write a docs/research note with the tables and the gate verdict, commit it on fable_5_1-v3-intel, push the branch (it has no upstream) and merge it into fable_5_1 per the wave process.
- Cleanup once the results are frozen: /common/dev/rustred/.claude/worktrees/fable51-py/TMP/gen7-resume (mutated to generation 9; must not be reused as a gen-7 fixture); optionally the trace directories (g7 about 8 GB logical, 5f about 5 GB logical). Keep TMP/gen7 read-only if other lanes use it.
- Out of scope for this summary but on the same branch per the plan: W0.5 (production baseline M1) and W0.7 (census). They have their own TMP/w0/baseline and TMP/w0/census directories and were not reviewed here.

**Caveats.**
- I produced two sets of numbers myself, not the lane agent, by running the lane's own scripts on its outputs: native seconds per record (replay/jobsecs.py on g7-trace-resume/trace) and the gate-0.4(b) [E] projection (project.py). They are unreviewed, and project.py's model (cheap tier at 100 ns, 1-thread costs, α choices) is the lane's own untested design.
- The foreign-load figures (about 52 CPUs during the traced resume, about 41 during the 48/90-thread sweep) are my own derivation from socket1-load.txt, assuming the sampler covered CPUs 128-227. The jiffy rate is consistent with 100 CPUs.
- All timings ran under a foreign workload (user nfink, load average 180-240, floating over all CPUs). Per-candidate ratios were compared within one process and one session, but the absolute ns values vary about 2x between sessions (e.g. today's miss cost is 51.9 ns in one 1-thread session and 114 ns in another).
- The real-stream layer costs use the static gen-7 snapshot index, not an evolving one. The 3.7% of hits whose container was admitted after the snapshot are reported separately. 80 jobs (46,947 event records) could not be joined and were skipped.
- The traced W100 resume is not a throughput baseline. It averaged about 1.4 busy native workers, was coordinator-bound with trace I/O (6.7 GB coordinator trace), and ran under foreign load. Only its request mix and counters are representative. The window's mix was 79% route jobs by count.
- C-5F was traced at W18 on CPUs 10-27, not at the plan's W50.
- Static hit and miss proxies are harder than real stream requests (about 5x more candidates per hit), so the static per-request speedups (13.9-30x) are not the production speedups. Per tested candidate, the ratios are similar between static and real streams.
- The lag table's 'admissions' unit is domain-ID distance (new IDs), per lag.rs, not total admissions. There are about 62 admissions per new ID in the stream window.

### 7.8 W0.7 work-volume census

**State.** Mostly done, not closed out. The last committed tip is 1df38d59 ("census: binned cost law, per-query candidate grid, batch and summaries") on local branch fable_5_1-v3-census, which has 3 commits on top of fable_5_1 b15316b9. The branch has no upstream and has not been pushed. The full receipt batch (w0_batch.sh with binary TMP/census-v3) finished with exit 0 at about 15:20 (receipt/batch.log). All seven W0.7 sub-items have machine outputs, and Q3 was run by hand into out/q3. Three things are still open: (1) an uncommitted change to tools/research/census/src/cover.rs (mtime 15:20:27, saved after the batch finished) that counts unevaluated or infinite queries conservatively; the receipt was NOT re-run with it. (2) No results note or RESULTS.md exists anywhere; the only tables are the JSON outputs plus summarize.py and q3_summary.py. (3) Q3 is not part of w0_batch.sh or the receipt dir. No file has a PLACEHOLDER marker; the receipt numbers below come from finished runs.

**Key findings.**
- [M] GATE 0.7 PASSES (threshold ≥30% of Apply CPU with residual against merged natives ≤10% of points in ≤8 pieces). Sample: gen7 hist_pps_seconds / all_owners / natives_before_dispatch, wait 30 s, 6000 PPS draws (5123 distinct). Gate share by vocabulary: D-only 70.1%, hull 89.0%, A/R (C2) 84.8%, exact pointwise 91.9%. Fully covered 61.6%, mean uncovered fraction 3.6%, unevaluated 0.12%. Evidence: /common/dev/rustred/TMP/w0/census/receipt/gen7/cover-w30.json
- [M] The pass holds across dispatch-wait and generation. Wait 300 s: D-only 68.0%, hull 87.0%, fully covered 59.5% (receipt/gen7/cover-w300.json). gen6: 69.5% / 89.1%, fully 61.3% (receipt/gen6/cover-w30.json). gen3: 57.4% / 79.9%, fully 55.4% (receipt/gen3/cover-w30.json).
- [M] Hot owner 011101110111000 at gen7 (64.5% of the PPS draws, so about 64% of Apply CPU), natives_before_dispatch: fully covered 63.8%, gate D-only 72.5%, gate hull 92.5%, mean uncovered 2.2%. Other large owners' D-only gate is 51-83%. Worst listed owner is 000011001001011 (1.6% of draws): 18.9% D-only, 36.8% hull. Evidence: receipt/gen7/cover-w30-rows.jsonl via `summarize.py owners ... hist_pps_seconds natives_before_dispatch`
- [E] (model on [M] inputs) Projected relative Apply cost of residual-only inspection, gen7 hist PPS natives_before_dispatch. Binned per-owner cost law: D-only 0.099, hull 0.056, A/R 0.056. OLS law: 0.162 / 0.092 / 0.105. Hot owner, binned law: D-only 0.058, hull 0.025. Evidence: receipt/gen7/cover-w30.json, fields projected_relative_cost_*[E]
- [M] Per-inspection (uniform) weighting gives lower shares. gen7 hist_uniform natives_before_dispatch: fully 57.1%, D-only gate 59.8%, hull 72.7% (receipt/gen7/cover-w30.json).
- [M] G2' at resume (pending Apply covered by all gen7 natives). pending_pps_predicted: fully 55.4%, D-only gate 63.8%, hull 85.7%. pending_uniform: 56.7% / 61.1% / 76.2%. For the general union cover (D2, anchor set all_other_domains), pending_pps_predicted gives fully 79.5%, D-only 85.0%, hull 96.4%. Evidence: receipt/gen7/cover-w30.json
- [M] Piece counts of partial residuals (gen7 hist PPS natives_before_dispatch). D-only: {1: 2284, 2: 14}. A/R: {1: 1966, 2: 292, 3: 29, 4-8: 7, 13-19: 4}. Pending vs all natives, D-only: {1: 2673, 2: 4, 3: 1}. Fragmentation risk R9 is negligible for D-only cuts, with a small tail for A/R. Evidence: receipt/gen7/cover-w30-rows.jsonl via summarize.py cover
- [M] Cross-check against the plan's C-HOT bound (71.1% of inspections, 69.3% of seconds). Drained pilot, fully covered by all_earlier_ids: 71.5% uniform, 68.4% PPS, which agrees. Pilot natives_before_dispatch PPS: fully 53.6%, D-only gate 66.0%, hull 79.2%. Evidence: receipt/pilot/pilot-cover.json
- [M] Controls. C-5F natives_before_dispatch PPS: fully 44.2%, D-only gate 44.4%, hull 45.4% (receipt/c5f/cover.json). Four-loop controls (PPS): unevaluated (infinite) queries are 15.2% of the weight for BMW, 22.4% for FG, 51.8% for H and 69.9% for X. FG, H and X gate at ≤1.6%. BMW gates at 40.5% D-only and 67.7% hull. The four-loop cover numbers cannot be interpreted before a re-run with the conservative accounting (receipt/four-loop/*-cover.json).
- [M] Cost laws at gen7 (receipt/gen7/cost.json): 241,443 Apply native seconds over 5,949,328 Apply natives. Hot owner: 64.1% of Apply seconds, OLS cost exponent 0.659 (r2 0.38), successor exponent 0.673, top-decade exponent 1.37. By generation its exponent is 0.77 (g3), 0.68 (g4), 0.58 (g5), 0.61 (g6), 0.67 (g7). The next five owners (1.8-5.2% share) have b 0.51-0.63. 55 of 67 owners were fitted, b range 0.17-0.67. Pooled b 0.351 (r2 0.34). These are below the plan's 'points^0.74-0.90' (plan line 74); the gap is not reconciled.
- [M] Composition of the 20,572,079 native-pending at gen7 (receipt/gen7/compose.json). Apply 5,847,445 (31 infinite), Route 14,724,634 (36 infinite); all but 262 are unreserved. By admission generation, Apply is 2.43M (g5), 2.23M (g6), 1.19M (g7) and Route is 6.16M / 5.60M / 2.97M. Pending points are 8.12e11 for Apply and 4.29e12 for Route. 3.93M Apply pending sit ≥1e7 behind the ledger cursor. Predicted Apply pending CPU [E, binned law] is 115,809 s. The hot owner holds 317,309 of those domains and 62,254 s [E] (53.8%). Owner 111001100111001 holds 785,629 domains and 8,570 s [E].
- [M] Pending vs committed envelope at gen7 (receipt/gen7/compose.json). Of 8.12e11 pending Apply points, 6.65e7 lie outside the committed per-owner level set (about 8e-5), and 7,983 of 5.85M domains touch it. Apply pending domains that exceed the committed extrema: Amax 5,304, Rmax 66, Pmax 77, Dmin 11. Route: 40,655 / 16,789 / 5,183 / 9,347 of 14.72M.
- [M] Global-potential check (observational only, per potential.rs). On gen3, gen6 and gen7 (8040 nodes; 72,456 creator pairs at gen7), the creator|P graph has no positive cycle, with max potential 19. The creator|A and creator|R graphs, and all three all_transitions graphs (A, P, R), have positive cycles, which the tool reports as 'unresolved', not as nontermination. C-5F: creator|P acyclic, max potential 9. Four-loop controls: creator|P acyclic, max potential 2-3. Evidence: receipt/gen{3,6,7}/potential.json, receipt/c5f/potential.json, receipt/four-loop/*-potential.json
- [M] Point-space saturation (receipt/gen7/saturation.json, 2000 draws per window). Estimated new points from the union of all natives: 5.54e11 through g3, then +6.4e9 (g4), +1.3e9 (g5), +1.14e10 (g6), +3.3e6 (g7; 448,938 natives, 7.3 new points per native). Hot-owner new-point fraction, CPU-weighted: 2.1% (g3), 0.7% (g4), 0.4% (g5), 1.1% (g6), 0.3% (g7); points-weighted 0.9% at g7. The census computes this itself; `rtool union` was not used.
- [M] Hot-owner closure import (feeds W0.11). v2 gen7 has 879,860 Apply natives (169,606 s) in pilot buckets. Pilot natives fully cover only 0.4% of them, mean uncovered fraction 72.3%, projected hull cost 0.866 [E]. For v2 pending Apply in pilot buckets (841,457 domains, 77,768 s predicted [E]), 0.0% are fully covered under PPS and 2.2% under uniform draws. In the other direction, v2 covers 100% of the pilot's natives. Exactly identical domains are rare (862 of 273,737 hot natives). Pilot: 7,767,543 records, 801 buckets. Evidence: receipt/pilot/v2g7-vs-pilot.json
- [M] Q3 guard/coefficient factor census on the v2 selection: 67 owners, 17,975 rules, 1,667,335 rhs terms, 242 affine cases, 0 skipped and 0 failed factorizations. Equality guards: 258 occurrences, 129 distinct, 100% affine in n. Excluded conjunctions: 32,037 occurrences, 2,122 distinct, 98.6% affine in n (91.4% weighted by gen7 CPU share). Denominators: 35,567 distinct. 38.6% of occurrences have all factors affine with integer coefficients (24.8% CPU-weighted); 81.6% are at most affine in n (66.3% CPU-weighted). Denominator factor classes by occurrence: constant 24.0%, base-only 3.0%, affine index 34.1%, affine plus base offset 31.4%, base slope 0.4%, nonlinear 7.1%. Numerators: 496,669 distinct polynomials, 54.5% / 71.1%, nonlinear-in-n factors 18.0%. Evidence: /common/dev/rustred/TMP/w0/census/out/q3/v2-factor-census-plain.json and v2-factor-census-numerators.json via q3_summary.py with receipt/gen7/cost.json
- [M] Checkpoint facts at gen7 (receipt/gen7/stats.json): 74,156,033 domains (Apply 19,366,066, Route 54,789,967), 45,889,639 records, 37,907,667 live candidates, 8040 buckets. Apply native seconds 241,437 vs Route 1,951. Native ID lag quantiles: p50 330, p90 2089, p99 9455, max 87,084. 131,316 native records are out of ID order.

**Artifacts.**
- Commit 7d6dd2f1: tools/research/census standalone crate (CP5 decoders, cover/cost/compose/potential/saturation/pilot subcommands, brute-force unit tests), README.md, run_all.sh, summarize.py
- Commit 8cc2aed5: owner-domain-scan --factor-census, --factor-census-numerators and --factor-census-max-terms (crates/rustred-core/src/solver/candidate_reduction/owners/factor_census.rs plus CLI wiring). Its commit message reports owner_domain lib tests 33/0.
- Commit 1df38d59 (tip): binned cost law, per-query candidate grid, tools/research/census/w0_batch.sh, q3_summary.py
- Uncommitted: /common/dev/rustred/.claude/worktrees/fable51-compact/tools/research/census/src/cover.rs (+10 lines: unevaluated or infinite queries count as uncovered, full residual, full cost, gate failed). It was saved after the receipt run.
- Receipt binary: /common/dev/rustred/.claude/worktrees/fable51-compact/TMP/census-v3, sha256 dc0a4a6f2a157b0bc55138f4a23070c2051d5c6d7c660c02e8e2a8d3a5b04d5e (recorded in /common/dev/rustred/TMP/w0/census/receipt/census-bin.sha256)
- Q3 binaries: /common/dev/rustred/.claude/worktrees/fable51-compact/TMP/rustred-q3census (13:28, first run) and TMP/rustred-q3census-b (14:06, plain and numerators runs). No sha256 is recorded for either.
- Receipt outputs: /common/dev/rustred/TMP/w0/census/receipt/{gen3,gen6,gen7,four-loop,c5f,pilot}/*.json|.err|-rows.jsonl, v2-series.txt, batch.log (exit 0)
- Q3 outputs: /common/dev/rustred/TMP/w0/census/out/q3/v2-factor-census{,-plain,-numerators}.json with events and stderr
- Superseded exploratory runs from binaries census-v1 and census-v2: /common/dev/rustred/TMP/w0/census/out/{gen3,gen6,gen7,four-loop,c5f,pilot,*.json}. Do not cite them; use receipt/.
- Inputs, read-only block clones: /common/dev/rustred/.claude/worktrees/fable51-compact/TMP/gen{3,6,7} (the gen3 records file alone is 10.5 GB)

**Gate status.**
- W0.7 gate [thresholds E]: PASS [M]. 70.1% (D-only) and 89.0% (hull) of gen7 Apply CPU, measured with PPS draws weighted by native seconds, have a residual against natives merged before dispatch that is ≤10% of points in ≤8 pieces; the threshold is ≥30%. W4.2 may proceed on this criterion. Evidence: receipt/gen7/cover-w30.json
- The pass still holds under the conservative fix that has not been applied: unevaluated weight is 0.12% at gen7, and the fix changes gate shares by ≤0.12 pp.
- No gate is defined for the Q3 factor census; it is input to the conditional 'factor atlas' (plan line 735).

**Remaining work.**
- Review the uncommitted cover.rs change (conservative accounting of unevaluated queries), then commit it, rebuild census (see README build command, CARGO_TARGET_DIR=TMP/census-target) and re-run `w0_batch.sh <bin> <new receipt dir>`. The last batch took about 35 min of summed step time (batch.log); the gen7 pilot overlap alone took 487 s. Five-loop shares should move by ≤0.23 pp. The four-loop cover numbers will change materially and must be re-read.
- Add the Q3 runs (owner-domain-scan --factor-census, plain and --factor-census-numerators, on campaigns/five-loop-qcd-feynman-d9d10-v2/inputs/selection.json) to w0_batch.sh or the receipt dir. Record the binary sha256 and build from the committed tip 8cc2aed5 or later. The existing Q3 outputs were built from a dirty tree (build log: 'Git tree ... is dirty').
- Check why coefficient_denominator and term_denominator have identical rows in every Q3 output (35,567 distinct, same factor classes): either they coincide by construction or the census counts one role twice.
- Check the hot-owner mask 011101110111000 against the pilot's legacy CP4 result.json and against C-5F. Only 53-59 of 4000 PPS draws land on the hot owner in the pilot, which is odd for a 'hot-owner pilot'.
- Reconcile the gen7 per-owner OLS cost exponents (0.51-0.66 for the large owners, r2 0.36-0.65) with the plan's 'cost ∝ points^0.74-0.90' (plan line 74).
- Optionally raise --cap or --samples for the ≥2M-point queries. At gen7, 1.7% of PPS draws use sampled points, so 'fully covered' may be overestimated for them.
- Write the W0.7 results note (for example docs/research/, using summarize.py and q3_summary.py tables) with [M]/[E] labels. No results document exists yet.
- Feed W0.11 and the D-session: the G2' bound (gate PASS), D-only vs A/R piece counts, the pending G2' at resume, and a hot-owner closure import projected at ≈0 benefit.
- Push fable_5_1-v3-census, or merge it into the lane branch the plan names (fable_5_1-v3-intel), once the orchestrator decides.

**Caveats.**
- The receipt was produced by TMP/census-v3, built at 14:48:54, 9 s before commit 1df38d59 at 14:49:03. It is likely the same source but that was not verified. The later uncommitted cover.rs edit is not in the receipt.
- In the receipt, unevaluated (infinite) queries count as gate-failed and not fully covered, which is conservative. They add 0 to residual fraction, pieces and projected cost, which is optimistic. At gen7 this is ≤0.12% of the weight; in the four-loop controls it is 15-70%.
- natives_before_dispatch is a proxy: the dispatch time is reconstructed as commit time − native seconds − wait, on the heartbeat series (v2-series.txt). Moving the wait from 30 s to 300 s moves the D-only gate from 70.1% to 68.0%.
- The CPU shares come from 6000 PPS draws. The 1-sigma binomial error at 70% is about ±0.6 pp [E]. The uniform hot-owner subsamples are small (212-345 draws).
- All projected_relative_cost values are [E] (per-owner binned or OLS cost-law extrapolation), and so are all predicted pending seconds.
- The global-potential result is observational. An acyclic creator|P graph does not prove termination, and a positive cycle does not prove nontermination (potential.rs header).
- vendor/symbolica has an uncommitted src/poly/polynomial.rs change (heap-power encoding, dated 2026-09-26) in both the main checkout and this worktree. The Q3 binaries were built against that dirty submodule. The change was already there before this lane.
- The lane used branch fable_5_1-v3-census, not the fable_5_1-v3-intel named in plan §5 0.7.
- Everything above was read from existing files. Nothing was re-run for this summary except summarize.py, q3_summary.py and small python readers.

### 7.9 W0.6 input intel

**State.** Partial, but most of it is done. Last committed tip is 9e63088d ("run_four: C-5F family, manifest/worker overrides, output root", 2026-09-27 15:09 UTC), 7 commits on top of b15316b9. The branch is not merged into fable_5_1. The write-up is only in /common/dev/rustred/TMP/w0/inputs/RESULTS.md: it is uncommitted, its header says "DRAFT, being filled", and its §4 points to a "§4.1" plan-v3 control probe that was never written. It has no *_PLACEHOLDER markers. Finished: L_static, the interim-frontier ancestors, L_obs, L_clean, L*, the four candidate helper sets with their matching diagnostics, the I1b 60-min probe, four-loop static and dynamic validation, the I2 rewrite with load check, and the I2 W6 pilot A/B. Not finished: (a) the same-CPU plan-v3 control probe (probe-v3) was killed at about 2,071 s of its planned 3,240 s. It left only heartbeat files: no result.json, receipt.json or census. (b) The queued probe-I1 (60-min five-loop I1 probe) never started. (c) The queued C-5F I2 A/B with audit never started; TMP/w0/inputs/i2/c5f/ is empty. (b) and (c) were chained behind (a) by i1probe_after.sh and c5f_after.sh, which wait for receipt files that were never written. No lane process is running now (checked 19:02 UTC).

**Key findings.**
- [M: TMP/w0/inputs/static-reach.json] Geometric L_static is the empty set. Keeping every scan region, all 67 owners reach all 67 owners. The cause is the unsupported-support-change regions, which the scan does not test on coefficients (73,781 such groups in static-reach.json; RESULTS gives 1,958,316 regions). Scan in scan5/ (result.json sha256 84842e9f...): 4,758,436 sign regions in 87,179 groups, 103.6 s, 6.0 GB.
- [M: TMP/w0/inputs/static-reach-r24.json, static-reach-r32.json, static-probe/] After replacing the scan targets of 40 owners with native one-hop successors of their full orthants, L_static at rank ≤24 and at rank ≤32 is the same 40 owners, with 0 violations. The matching-only diagnostics are clean: r24 has 133,131 selected / 413 terminal / 0 unresolved (125 s); r32 has 158,157 / 413 / 0 (136 s). The one-hop runs had 40/40 initial records and 0 bad records (hop-r24.log, hop-r32.log). RESULTS reports 506 distinct targets and 0 targets outside the source mask. Rank infinity is not covered: at rank None the first owner has 31 unresolved pieces, exit 4 (static-probe/rnull.log). The soundness argument that the full-orthant one-hop contains every domain of rank ≤R is the agent's; I did not verify it independently.
- [M: TMP/w0/inputs/interim-gen8/summary.json, frontiers.tsv, ancestors.tsv] Read-only parse of the interim gen-8 CP4 state (276 GB) found 53,228,053 domains, 4,205,183,439 edges, 1,299 local_dispatch_frontier records on 397 Apply nodes, and 7,762,250 ancestor nodes. RESULTS adds: the frontiers are all unbounded-A at ranks 4-7 in 6 guard owners; the ancestors include 3,048,864 Apply nodes in 27 owners (the 7 guard owners plus 20 others); no L owner is an ancestor. Owner-level closure of gen 8 gives the same 40 owners (L_obs).
- [M: TMP/w0/inputs/lstar-candidate.json sha256 f8418350d272...; candidates/lstar.json holds the same owner set] L* = L_static ∩ L_obs ∩ L_clean has 40 owners: the 13 owners with t ≤ 7 plus 27 with t = 8-10. The 27 owners outside L* are the 7 guard owners G and 20 U owners that reach them. Caveat from RESULTS: the static part holds only up to rank 32, so a rank escape above 32 in an L* owner has to stay a monitored stop condition.
- [M: TMP/w0/inputs/candidates/match-*.log] The four candidate query sets all pass the checker, and the matching-only diagnostic is clean on 183/183 queries for each. plan-v3 (queries sha256 2c714860...) equals match-v3. I1 (e6cbc0c1...): 398,403 selected / 1,065 terminal / 0 unresolved, gap or invalid, 143 s. I1b (80f3e54d...): 431,853 / 1,096 / 0, 184 s. I1+I1b (8aa85f23...): 432,703 / 1,096 / 0, 177 s. Physics roots are identical in all sets (116 roots). I1b bounds are the v2 gen-7 envelope (v2g7-envelope/envelope.tsv) plus 2 on both rank and A. The +2 margin is a choice, not a derived bound.
- [M: commit fe0879b8] Planner option --helper-bounds-from (schema rustred.helper-bounds.json.v1) is also re-derived by the checker. RESULTS reports 48 planner and checker tests OK, with 1 opt-in skip. Without overrides the output is byte-identical to plan-v3. I did not re-run the tests.
- [M: TMP/w0/inputs/probe-I1b/receipt.json, analysis.json, probe-I1b.log] I1b five-loop closure probe on binary 4a17f9c7, W24 on CPUs 100-117 plus 362-367, Ready policy, all 183 queries. Cooperative stop at 3,240 s, 3,266 s wall, exit 4. Frontiers stayed at 0 throughout. At the stop: 2,593,248 natives, 11,390,857 scheduled, 5,374,792 queued, 180,945,561 edges, max rank 16, 3 of 67 roots closed, RSS 12.1 GB. There were 258,624 Apply domains in the 7 guard owners (35,093 inspected), none with unbounded A. Per guard owner the max finite A was 25-29 and the max rank 8-14.
- [M: TMP/w0/inputs/probe-v3/timeseries.jsonl; interrupted run, comparison computed by this summary, not by the lane] Same-CPU, same-binary plan-v3 control against I1b at matched natives. At 2.5M natives: discovered 10.97M (I1b) vs 9.11M (v3), 1.20x; pending 5.10M vs 3.44M, 1.49x; edges 176.1M vs 132.9M, 1.32x; roots closed 3 vs 6; wall to reach 2.5M natives 3,131 s vs 2,039 s, 1.54x. At 1.0M natives: discovered 4.75M vs 4.88M; pending 2.38M vs 2.11M. v3 frontiers 0 up to its last heartbeat (2,071 s, 2,514,566 natives, max rank 17). Early phase only, and foreign load was not recorded.
- [M: TMP/w0/inputs/four/ab_table.txt, four/runs/*/audit.json] Four-loop A/B, W6 on CPUs 100-105, two repeats. FG, H and X have no guard owner, so their inputs do not change. BMW has exactly one guard owner, 0101111100, and L_static(BMW, R≤20) = L_obs = 130 owners (four/static-reach-bmw-r20.json). BMW I1 drains in both repeats: 9,389 natives vs baseline 147,233 (15.7x fewer); traversal 8.6 s vs 37.4 s (repeat 1) and 10.0 s vs 44.5 s (repeat 2); 268/268 roots; audit PASS. BMW I1b and I1+I1b do not drain within the caps: I1b reached 2,684,469 natives at 600 s (repeat 1) and 1,417,002 at 300 s (repeat 2), and I1+I1b 3,288,383 at 734 s and 1,617,250 at 300 s, still growing, with 0 frontiers. Audit PASS: base BMW/FG/H/X (repeat 1) and BMW I1 (both repeats).
- [M: TMP/w0/inputs/i2/summary.json, rewrite.stdout; tool sha256 fa623da4...] I2 witness rewrite: 8,179 routes evaluated, 7,467 changed. Traffic-weighted cancellation support goes from 7.559 to 5.562, the unweighted mean from 6.39 to 4.81, over 557,718,765 route-traffic edges (route-traffic-v2g7.tsv). This reproduces the lens estimate with exact Symbolica arithmetic. selection-i2.json (sha256 d9760837...) loads through symmetry::verify and integral_transport::compile. Per RESULTS, the plan-v3 matching diagnostic on it (i2/match-v3-i2) is clean and identical to match-v3 at 397,553 / 1,065 / 0.
- [M: TMP/w0/inputs/i2/pilot-orig, pilot-i2 (census/transitions.tsv recomputed by this summary)] I2 pilot: W6, 900 s each, run at the same time on CPUs 106-111 and 112-117, plan-v3 queries. Natives: 833,541 (original) vs 1,162,863 (I2). Route→Route edges per inspected Route: 35.78 vs 15.61 (−56%; recomputed and confirmed). RESULTS reports, at 800k natives: discovered 3.86M vs 2.92M, pending 2.01M vs 1.39M, edges 35.8M vs 17.8M. Frontiers 0 in both runs. The phase is early and route-heavy, and the long-run effect is not measured [E].
- [M: TMP/w0/inputs/v2g7-escapes-v3/escapes.tsv] 19,365,999 of the 19,366,066 v2 gen-7 Apply domains lie outside their plan-v3 helper box (by A: 17,674,003; by rank: 5,574,107). So 'outside the helper' is the normal state and not a risk signal; the risk signal is a frontier.
- [E] Gate 0.6 for I1: the static and diagnostic parts pass (L* = 40, diagnostic clean, four-loop BMW I1 drains with audit PASS). There is no dynamic five-loop I1 evidence, because probe-I1 never ran.
- [E] Gate 0.6 for I1b: diagnostic clean and 0 frontiers in the 60-min probe. It passes only under the lane's reinterpretation that 'no domain in a guard mask' means no unbounded-A domain in a guard mask (count 0). Read literally, it fails: there were 258,624 Apply domains in guard owners. It also fails to drain on four-loop BMW and, against the partial same-CPU plan-v3 control, does more work per native. Taken together, the evidence argues against shipping I1b.
- [E] Gate 0.6 for I2: rewrite and load verification are done, and the W6 pilot is above the ≥20% threshold. The formal W1.3 gate (C-5F A/B with audit) was not run.

**Artifacts.**
- Branch fable_5_1-v3-inputs commits: fe0879b8 (planner --helper-bounds-from, checker, tests), 02db11dc (input-intel tools: cp4scan.rs, cp5hop.rs, static_reach.py, one_hop.py, make_candidates.py, probe.py, run_match.py, rewrite_anchors.py), daba23d6 + b949e064 (crates/rustred-app/examples/route_witness_rewrite.rs), c9731195 (analyze_probe.py, phase census, run_four.py), d3f2ec80 (compare_probes.py), 9e63088d (run_four C-5F family). 16 files, +2966/-5 vs b15316b9. Not merged.
- Tools dir: /common/dev/rustred/.claude/worktrees/fable51-inputs/tools/research/inputs/
- Binaries: /common/dev/rustred/.claude/worktrees/fable51-inputs/TMP/bin/cp4scan sha256 e44815d02d56b146a018b055af0b16c2fa7293624a2f159dc50fec1ff75922c6; .../TMP/bin/cp5hop sha256 b215cf03a40b4039b919e22d250915ccb2c04d060bb09886f6aa6bb9fb5c3b05; /common/dev/rustred/TMP/w0/inputs/i2/route_witness_rewrite sha256 fa623da4be6fa3e0a5c5e01c5651f88c0fb7f70b7911d855038c4ff8ccbfceee; walk binary /common/dev/rustred/TMP/fable51-controls/bin/rustred-4a17f9c7 sha256 4a17f9c7c0447713370a1aed2e54c9a04251106a9183fd1b8a86dc5d1abb395e
- Draft write-up (uncommitted): /common/dev/rustred/TMP/w0/inputs/RESULTS.md
- L*: /common/dev/rustred/TMP/w0/inputs/lstar-candidate.json sha256 f8418350d27272612e0c7e8dd6eb6f58203d9192dab6a460321e1135af51f1d4 (same set in candidates/lstar.json)
- Candidate queries: /common/dev/rustred/TMP/w0/inputs/candidates/plan-v3/queries.json 2c7148601436ebd1e50f7c13d922859cdb5d296f5584f33d266f0f866d27204f; plan-I1 e6cbc0c159246d117a32468cbc2626851f3f3d9ca860f823511bea19982320e7; plan-I1b 80f3e54d3c3620b0668f528036e66adeb9b3515aec63af9b1cfebe9dc1bf6a14; plan-I1-I1b 8aa85f2365a669f4bd51bd0e47837b12774c5597d9851697ae14abd73318727f; helper-bounds JSON/TSV alongside
- I2 selection: /common/dev/rustred/TMP/w0/inputs/i2/selection-i2.json sha256 d976083795bff7ffe48f4418476439626730533df4c4ea43e71723248d32dd25; report.tsv, summary.json, match-v3-i2/, pilot-orig/, pilot-i2/
- Static reach: /common/dev/rustred/TMP/w0/inputs/static-reach.json, static-reach-r24.json, static-reach-r32.json, static-probe/, scan5/
- Interim gen-8 census: /common/dev/rustred/TMP/w0/inputs/interim-gen8/ (summary.json, frontiers.tsv, ancestors.tsv, seed_ancestors.tsv, transitions.tsv, apply_census.tsv)
- v2 gen-7 census: /common/dev/rustred/TMP/w0/inputs/v2g7-envelope/, v2g7-census/, v2g7-escapes-v3/, route-traffic-v2g7.tsv
- I1b probe (complete): /common/dev/rustred/TMP/w0/inputs/probe-I1b/ (receipt.json, result.json, analysis.json, census/, checkpoint/)
- plan-v3 control probe (interrupted, heartbeats only): /common/dev/rustred/TMP/w0/inputs/probe-v3/ (timeseries.jsonl 66 rows to t=2071 s; checkpoint/records-...03.jsonl 3.2 GB)
- Four-loop: /common/dev/rustred/TMP/w0/inputs/four/ (ab_table.txt, runs/ab-r{1,2}-{base,cand,i1,i1b}/<fam>/, match-*/, static-reach-bmw-r20.json, queries-bmw-I1.json 26f3bb6e..., queries-bmw-I1b.json 7ffd4624..., queries-bmw-I1-I1b.json 7aea92a5...)
- Queued-but-never-run scripts: /common/dev/rustred/TMP/w0/inputs/v3probe_after.sh (ran, killed), i1probe_after.sh (probe-I1, never started), i2/c5f_after.sh (C-5F I2 A/B, never started)

**Gate status.**
- 0.6 I1 (ships only for L* owners): L* = 40 owners [M]; the I1 candidate changes only L* owners, and its diagnostic is clean [M]; four-loop BMW I1 drains with audit PASS [M]. There is no five-loop dynamic evidence yet, because probe-I1 never ran. Status: static and diagnostic parts PASS; dynamic part OPEN.
- 0.6 I1b (diagnostic clean + 60-min probe 0 frontiers + no domain in a guard mask): diagnostic clean [M]; 0 frontiers in the 3,240 s probe [M]. 'No domain in a guard mask' fails if read literally (258,624 Apply domains in guard owners) and passes only as 'no unbounded-A domain' (0) [M]. Status: needs a decision on how to read the gate. The supporting evidence is negative: four-loop BMW I1b does not drain [M], and I1b does more work per native than the same-CPU plan-v3 partial control [M, partial].
- 0.6 I2 (witness rewrite and load verification): DONE [M]. The W1.3 gate (≥20% fewer Route→Route edges per route native on C-5F, with audit PASS) was NOT RUN. The W6 five-loop pilot shows −56% [M], but that pilot is not the gate.

**Remaining work.**
- Re-run the same-CPU plan-v3 control probe for the full 3,240 s, using the command in /common/dev/rustred/TMP/w0/inputs/v3probe_after.sh (drop its wait-loop). Write to a fresh --out directory, or delete the stale probe-v3/ first: its checkpoint is from a killed run and its records file is 3.2 GB. Then run analyze_probe.py and compare_probes.py (tools/research/inputs/) against probe-I1b, and fill the dangling §4.1 of RESULTS.md.
- Run probe-I1, the 60-min five-loop I1 closure probe, using the command in /common/dev/rustred/TMP/w0/inputs/i1probe_after.sh (queries candidates/plan-I1/queries.json, bounds candidates/bounds-I1.tsv, W24 on CPUs 100-117 and 362-367, --stop-after 3240). Check for 0 frontiers and for unbounded-A domains in guard masks, and compare with probe-v3 at matched natives.
- Run the C-5F I2 A/B with audit, using the commands in /common/dev/rustred/TMP/w0/inputs/i2/c5f_after.sh: run_four.py --family five-finite, labels orig and i2 (manifest i2/selection-i2.json), W18 on CPUs 100-117, --max-seconds 1500, then audit_owner_domain_walk.py on each. Compute Route→Route edges per route native from each census. This is the W1.3 witness gate (≥20% reduction with audit PASS).
- Get a D-session decision on the I1b gate wording (literal 'no domain in a guard mask' vs 'no unbounded-A domain'), and on whether to drop I1b given the four-loop BMW non-drain and the partial same-CPU control.
- Finalize RESULTS.md: remove the DRAFT status, add the §4.1 control and the probe-I1 results, and write the gate verdicts. Then commit it as a research note on fable_5_1-v3-inputs (it currently exists only in TMP), and merge or PR the branch (7 commits, not merged into fable_5_1).
- Optionally re-run the planner and checker test suites (RESULTS claims 48 OK with 1 skip; this summary did not re-run them).
- Cleanup, only after the results are frozen: the gen-7 block clone at /common/dev/rustred/.claude/worktrees/fable51-inputs/TMP/v2-gen7 (43 GB apparent size), the probe checkpoints under TMP/w0/inputs/probe-*/checkpoint and the i2 pilot and four-loop run checkpoints, and the stray nix-develop and nix-shell temp dirs in the worktree TMP.
- W1.3 follow-up, outside W0: ship examples/input/five_loop_qcd_feynman_d9d10/queries.json v4 (I1 for L* owners, plus I1b only if the decision allows it) and the rewritten selection. Then run the 1-h L*-only legacy walk gate and, after W2.5, the epoch-engine L*-only walk.

**Caveats.**
- probe-v3 was killed around 15:30 UTC, about 2,071 s into its planned 3,240 s, presumably by the usage-limit cut-off; stderr is empty and there is no result.json or receipt. The same-CPU I1b vs plan-v3 numbers in key_findings are my own interpolation over its heartbeat timeseries, not a result from the lane agent. Foreign load on the probe CPUs was not recorded (the knobs lane may have been active on the same socket).
- The I1b vs v2 trajectory table in RESULTS §4 compares runs with different binaries (4a17f9c7 vs 102adcc3) and worker counts (W24 vs W100). It is labelled [M-r] and is not like-for-like.
- In the I2 pilot the two arms ran at the same time on separate CPU sets and were stopped at 15 min. 95-97% of natives were Route, so this says nothing about long-horizon effects. It also used plan-v3 queries, not C-5F.
- L_static is proved only for domains of rank ≤32. The observed max rank is 18 in v2 gen 7 and in interim gen 8, and rank 17 was reached in probe-v3. Rank infinity cannot be checked (guard algebra limit refusal).
- The I1b reading of the gate phrase 'no domain in a guard mask' is the lane agent's own interpretation, not something the plan says.
- RESULTS.md is labelled DRAFT and is uncommitted, in TMP. I spot-checked hashes, matching logs, audit verdicts, ab_table, the I2 summary, the Route→Route ratios, the gen-7 escape totals, the static-reach counts and the interim-gen8 summary; all matched RESULTS. I did not check every number, such as the 1,958,316 unsupported regions, the 506 one-hop pairs or the 3,048,864 ancestor Apply nodes.
- The worktree shows vendor/symbolica as modified (src/poly/polynomial.rs). The main tree has the same diff, so it was already there and is not this lane's change. The lane also fetched upstream Symbolica dev 445b882d into the submodule's FETCH_HEAD without checking it out.

## 8. The combined four-loop run (owner request)

Owner request: validate on one combined run over all four-loop owners. The lane (workflow wf_ad162918-3ac) found Luthe's A4 common basis (section 5.6), built the combined inputs (branch `fable_5_1-c4l-combined`, family `four-all` in `run_control.py` on that branch; variants `four-all-a19` (all helpers at A19) and `four-all-physics`), ran W6/W24 walks with audit PASS and Ordered identity across widths, and queued W96 runs that completed unattended. The independent audit never ran. Summary of the artifacts (read-only summarizer):

**State.** Partial, but most of the build is done. Last committed tip is 40dbccd3 on fable_5_1-c4l-combined (two commits on top of b15316b9: e3dc9995 and 40dbccd3). The branch is not pushed and not merged. The working tree is clean apart from the vendor/symbolica submodule, which carries the heap-pow patch. Both investigations (basis, engine) finished. The build agent committed inputs, receipts, a research note and the W6/W24 results, then hit the usage limit at 15:31Z while waiting for the socket-1 lock. The W96 socket-1 session it had queued ran unattended at 17:42-18:43Z. Its results were never evaluated, audited or committed, and the committed README still shows W96 as 'queued'. One of those W96 runs contradicts the committed claim that four-all 'drains robustly'. The workflow's Audit phase never ran: agent a60764648b1c36586 stopped on the weekly limit before doing anything.

**Key findings.**
- [M] Basis. Luthe's A4 = {k1,k2,k3,k4,k1-k4,k2-k4,k3-k4,k1-k2,k1-k3,k1-k2-k3} (thesis Table A.1 p.111, eq. 2.26) holds both cubic 9-line roots: 1022 (prism, the Vakint H parent) and 511 (K3,3, the X parent). FG = 1020 lies inside 1022. BMW = 510 = 1022 AND 511. RustRed's verifier accepted all 108 Vakint-to-A4 maps. Premise correction: FG and BMW are 8-line graphs with 2 ISPs, not roots. Evidence: journal wf_ad162918-3ac line 4 (basis result); scratch in /common/dev/rustred/TMP/c4l/.
- [M, scratch Python only; E as a general proof] Uniqueness: 216 embeddings give 3 tenth forms, each GL(4,Z)-equivalent to A4. No common basis makes H or BMW a slot permutation, so their saved programs cannot be relabelled. X is A4 with slot 1 moved to the end. The search was not replayed natively (/common/dev/rustred/TMP/c4l/uniqueness.py, search.py).
- [M] Engine investigation: a common-basis run needs no engine change. The owner loader already takes a union of roots. The binding constraints are one family fingerprint, one saved solver policy, unique masks and single-sector bundles. The per-family C-4L controls have zero Route records. Summed per-family baseline (census-4l-4a17f9c7, W6 Ordered): 317,608 natives, 13.37M successors, 284.6 s inspector busy. Journal line 3. [E] A multi-family harness would cost 5-7 agent-days on the legacy engine, or +1-2 agent-days in v3 at W2.0/S2. It was rejected.
- [M] Owner generation with release binary 4a17f9c7 (sha256 4a17f9c7c044...395e), Vakint package policy (sparse, numerical-depth 2, finite-case search, no rank scope), --checkpoint-dir. Root 1022: 314 sectors, 22,715 rules, 386 residuals, 58.6 s, 0.61 GB. Root 511: 328 sectors, 22,228 rules, 445 residuals, 25.3 s, 0.88 GB. Each ran on 16 cores, concurrently, on a shared host. 281 zero sectors, matching thesis Table 9.1. Evidence: /common/dev/rustred/TMP/c4l-build.wBU9zC/gen/root{1022,511}.{time,generation.toml}.
- [M] Selection: 16 class owners (Table B.1; 12 from root 511, 4 from root 1022) and 508 labelled routes, 492 of them transported. Owner payloads total 6,562,373 bytes and stay uncommitted in TMP, pinned by SHA-256 in selection.json. Evidence: examples/input/four_loop_combined/selection.json on the branch; TMP/c4l-build.wBU9zC/inputs/.
- [M] Routing validation all PASS. The census reproduced 16/16 forward and 508/508 reverse witnesses. The independent Symbolica replay passed with 28/28 corruption controls rejected (2,754 reverse and 105 forward signed equalities; 281 rank-deficient masks in the family, 260 in the union). The native load verified all 508 routes and refused all 8 corrupted selections (NonUnitJacobian, ActiveBijection, NonUnitActiveRow, missing/duplicate source, owner mask mismatch, transport flag). The good finite trace completed in 2.07 s. Evidence: TMP/c4l-build.wBU9zC/audit/run.stdout and TMP/c4l-build.wBU9zC/loadcheck/*/{status,stderr}, good/result.json.
- [M] Queries. The planner (--loops 4 --gauge feynman --difference-set 7,8) classifies 9 connected, 6 factorized and 1 non-entry owner (841, the banana). It gives 42 queries (16 helpers, 26 roots) and 61,683,424 starting tuples. The checker passes (5,200 probes, 0 disagreements, Rust counts verified). The historical A<=19/R<=12/D>=7 envelope has 32 queries and 362,692,824 tuples. Matching-only diagnostics find 0 unresolved, 0 gap and 0 invalid pieces for physics (7,024 pieces), envelope (7,510), four-all (11,495) and r6anchors (10,570). Evidence: TMP/c4l-build.wBU9zC/plan-v1.check.json, match-*/.
- [M] four-all (42 physics rows plus 16 rank-12 owner orthants = 58 rows, 32 initial records after aliasing) drained with audit PASS in all 11 audited W6/W24 runs. Ready W24 x5: 22,290-24,425 natives. Ready W6 x3: 24,482-26,065. Ordered W6 x2 and Ordered W24 x1: 30,159 natives each, and strict record comparison PASS both W6-vs-W6 and W6-vs-W24. Zero frontiers, max scheduled finite rank 14, 0 partial initial inspections. Evidence: /common/dev/rustred/TMP/fable51-controls/c4l-4a17f9c7-{ready,ordered}-w{6,24}*/four-all{,-r12anchors}/{result,audit}.json; TMP/c4l-build.wBU9zC/compare-ordered-w6{,-w24}-strict.json.
- [M] NEW, not in the committed docs. Two Ready W96 runs of four-all on CPUs 128-223, identical input bytes: (a) c4l-4a17f9c7-ready-w96/four-all drained with 21,730 natives in 10.0 s (traversal 8.39 s), 0 frontiers. (b) c4l-4a17f9c7-ready-w96-rep2/four-all did NOT drain. It was cooperatively stopped at 3,605 s with 5,186,115 natives, 9,151,149 scheduled domains, 29,590 pending and 6,789,986 unresolved, 20/32 initial records closed, 0 frontiers, peak RSS 3.52 GB, and 2,007 s of ordered-commit wall. Neither W96 run was audited (no audit.json). Evidence: TMP/c4l-build.wBU9zC/run-four-all-c4l-4a17f9c7-ready-w96{,-rep2}.log, socket1_session.log, TMP/fable51-controls/c4l-4a17f9c7-ready-w96{,-rep2}/four-all/result.json.
- [M, tail sample only] In the last ~60 MB of the 6.3 GB rep2 records sidecar (90,620 records, 46,160 natives), Apply on owner 0111110010 (class 993; connected, V4min 2) accounts for 4.3 of 5.1 native-seconds. All 14,735 of its Apply natives are at rank 13, above the rank-12 anchor, fed by Route records from 0111010010, 0111100010, 0110110010 and 0101110010. The in-flight inspection cancelled at the stop was also this owner at rank 13. [E] This looks like the same rank-escape flood as four-all-physics, occurring even with rank-12 anchors. Because rep2 had 0 partial initial inspections, the committed 'engine note' (partial-overlap timing) does not explain it. The full per-owner breakdown was not computed. Evidence: TMP/fable51-controls/c4l-4a17f9c7-ready-w96-rep2/four-all/checkpoint/records-00000000000000000003.jsonl.
- [M] four-all-physics (the 42 planner rows alone) did not drain at Ready W24. At 3,600 s the last heartbeat shows 2,367,249 natives, 2,532,735 discovered domains, 11/16 initial records closed, 1,400,991 unresolved, 0 frontiers and 1.36 GB RSS. The run was stopped by SIGINT from the old runner, so no result.json exists. Per the README (not re-verified by me), 96.3% of native seconds went to owners 0111110010 (74.0%) and 0111111001 (22.3%), both with helper rank 3, mostly at rank 5. Evidence: TMP/fable51-controls/c4l-4a17f9c7-ready-w24/four-all-physics/{events.jsonl,RENAMED.txt,checkpoint/}.
- [M] four-all-r6anchors (physics plus rank-6 orthants) drained 3/3 at Ready W24: 12,527-13,370 natives, audit PASS. It was not run at W6 or W96. Evidence: TMP/fable51-controls/c4l-4a17f9c7-ready-w24*/four-all-r6anchors/.
- [M] four-all-a19 (box-first envelope) is schedule-sensitive. At Ready W24, 2 runs drained (22,846 and 22,847 natives, audit PASS) and the 3rd was killed at 600 s with >=1,150,778 natives (exit 124, no result.json). Ready W6 stopped at 2,660,400 natives after 2,540 s, and Ordered W6 at 3,168,826 after 3,141 s; both audits FAIL (not exhausted), 20/32 initial records closed. Ready W96 (1 run, unaudited) drained with 23,872 natives in 10.0 s. It had 31-74 partial initial inspections, against 0 for four-all.
- [E] In the committed note, the claims that four-all 'drains robustly' and that helper-first orders 'drained in all 13 runs' are refuted by W96 rep2. four-all is a reliable width-invariant oracle only under Ordered at W6/W24, as measured. Its Ready behaviour at W96 is schedule-sensitive. The note's input-lever and engine-note interpretations remain [E] and need revision.

**Artifacts.**
- Branch fable_5_1-c4l-combined, commits e3dc9995 (inputs, receipts, results, research note) and 40dbccd3 (four-all query document plus the Ordered W24 reference); based on b15316b9, not pushed
- /common/dev/rustred/.claude/worktrees/fable51-c4l/docs/research/four_loop_combined_run_2026-09-27.md (stale: no W96 results; its robustness claim is contradicted)
- /common/dev/rustred/.claude/worktrees/fable51-c4l/examples/input/four_loop_combined/ (README.md, selection.json sha256 b228ac95..., four_loop_common_basis.toml, manifest, parent vertices, routing witnesses, labelled_routes.jsonl, physics/, envelope-a19r12d7/, four-all/queries.json sha256 d1ac816e..., identical to the staged file used in every four-all run)
- /common/dev/rustred/TMP/c4l-build.wBU9zC/ (bin/rustred-4a17f9c7 sha256 4a17f9c7c0447713370a1aed2e54c9a04251106a9183fd1b8a86dc5d1abb395e, gen/, census/, audit/, loadcheck/, plan-v1/, envelope/, staged-*/, match-*/, commands/, hot_owners*.py, compare-*.json, socket1_session.{sh,log}, run-*.log, RESULTS.md, run_control.py.orig)
- /common/dev/rustred/TMP/fable51-controls/c4l-4a17f9c7-{ready-w24,ready-w24-rep1..3,ready-w6,ready-w6-rep1..2,ordered-w6,ordered-w6-rep1,ordered-w24,ready-w96,ready-w96-rep2}/<family>/ (walk dirs; ready-w96-rep2 holds a 7.4 GB-logical CP5 checkpoint that is resumable)
- /common/dev/rustred/TMP/fable51-controls/run_control.py (shared runner edited by this lane: five four-all families registered, --timeout-seconds cooperative stop, start_new_session; sha256 8dace33c...; the original is saved at TMP/c4l-build.wBU9zC/run_control.py.orig)
- /common/dev/rustred/TMP/c4l/ (basis investigation scratch: search.py, uniqueness.py, verify/main.rs, gen/, combined_a4/, combined_xbasis/, routed2_*/, plan_a4/)
- Workflow journal: /home/codex-2/.claude/projects/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/subagents/workflows/wf_ad162918-3ac/journal.jsonl (line 3 = engine result, line 4 = basis result; build and audit entries show 'failed')

**Gate status.**
- Routing census reproduction: PASS [M] (16/16 forward, 508/508 reverse)
- Independent Symbolica replay with 28 corruption controls: PASS [M] (TMP/c4l-build.wBU9zC/audit/run.stdout)
- Native load of 508 routes: PASS; 8/8 corrupted selections refused [M] (loadcheck/)
- Planner checker: PASS [M] (plan-v1.check.json)
- Matching-only diagnostics: clean for all 4 variants [M]
- four-all walk audit at W6/W24: PASS 11/11 [M]; Ordered strict record identity across repeats and W6-vs-W24: PASS [M]
- four-all at Ready W96: 1/2 drained (unaudited), 1/2 NOT drained within 3,600 s [M]. Drain robustness at W96 fails
- four-all-a19: audit PASS 2, FAIL 2 (not exhausted), 1 killed without a result; drained at W96 once (unaudited)
- four-all-physics: not drained within 1 h at Ready W24 (by design a stress case), no result.json
- Workflow independent adversarial Audit phase: NOT RUN (the agent hit the usage limit immediately)
- Merge to fable_5_1: not done; branch not pushed

**Remaining work.**
- 1. Run examples/python/audit_owner_domain_walk.py on the two drained W96 runs (TMP/fable51-controls/c4l-4a17f9c7-ready-w96/{four-all,four-all-a19}) and on the non-drained rep2 (expect FAIL: not exhausted).
- 2. Compute the full per-owner and per-rank breakdown of W96 rep2 with TMP/c4l-build.wBU9zC/hot_owners_jsonl.py <records-00000000000000000003.jsonl> <four-all/queries.json>. The sidecar is 6.3 GB, so run it off the busy CPUs. This confirms or refutes the class-993 rank-13 flood seen in the tail sample.
- 3. Repeat four-all at Ready W96 (>=3 runs, each capped at 1 h, socket 1 under the lock) and also at Ordered W96. This establishes whether the control is schedule-sensitive at Ready and whether Ordered stays at 30,159 natives across widths.
- 4. Update the README §5 table and the research note on the branch: add the W96 rows, withdraw 'drains robustly' and 'all 13 runs', and revise the engine note. The fragmenting W96 run had 0 partial initial inspections, so partial-overlap timing does not explain it. Decide whether the C-4L replacement should be Ordered four-all only, or four-all with anchors at rank >= 14 (the max scheduled finite rank), re-tested at W96.
- 5. Run the independent adversarial audit of the lane: inputs, digests, claims, the owner rule (12/16 owners from root 511; the 134 shared sectors have byte-different programs) and the generation-policy mismatch with five-loop (--max-numerator-rank 10, sparse-factorized, depth 0 vs the Vakint policy used here).
- 6. Decide on the owner payloads (6.56 MB, TMP only): commit them, or document the 1.5-min regeneration as the supported path.
- 7. After the audit, push fable_5_1-c4l-combined and merge it into fable_5_1. Update the master plan's C-4L row (section 5 table), which still points at the per-family FG/BMW/H/X controls.
- 8. Optional follow-ups named by the investigations: the all-A19-helper stress arm (~2.5M natives [M, per-family sum]); an X-basis variant that reuses the 328 saved X programs; reconciling the q^2-1 vs thesis q^2+1 sign convention; a native replay of the uniqueness search.

**Caveats.**
- The W96 runs ran unattended after the build agent was cut off. No agent inspected them before this summary. My W96 per-owner evidence is a tail sample (last ~60 MB of 6.3 GB, ~90k of ~9.1M records), not the full record set.
- All timings are single observations on a shared host. W6/W24 ran on CPUs 320-383, SMT siblings of 64-127 with foreign load. No speed ratio against the per-family C-4L controls is valid: 16 owners vs 900, fresh A4 programs vs Vakint, different queries.
- The shared runner TMP/fable51-controls/run_control.py was edited by this lane. Other lanes using it get the new families and --timeout-seconds behaviour; the original is saved at TMP/c4l-build.wBU9zC/run_control.py.orig.
- Naming clash: a separate knobs-lane socket-1 session named 'D-c4l-w48' (tools/research/knobs/socket1_session2.sh in worktree agent-ab06981cd80007c75) is queued or running with fg,bmw,h,x per-family controls. It is not part of this lane.
- The basis investigation's regeneration timings (~1 min/root) used binary 5746feb with 6 workers. The committed build used 4a17f9c7 with 16 cores. Rule counts agree.
- The hot-owner percentages for four-all-physics (74.0% / 22.3%) are taken from the committed README and were not recomputed here. The heartbeat totals were re-checked and match.
- Nothing here is a family-closure, termination or minimal-master claim. family_closure_claim is false in every artifact.

## 9. What to do next (in order)

### 9.1 Finish W0 (priority order)
Still running at hand-over: the knobs lane's unattended socket-1 session `D-c4l-w48` (script
`tools/research/knobs/socket1_session2.sh`, PID 987089 at 19:24 UTC, holds `TMP/locks/socket1.lock`, expected to end
around 19:41 UTC; log `TMP/w0/knobs/sessions/D-c4l-w48.log`). Nothing else from this session is running.

1. **Native in-process scaling (harness, section 7.4) — highest priority.** Gate 0.3 FAILS: CPU per native at 96
   threads is 3.75x the single-thread value (four-loop controls 4.6-7.3x); NUMA interleaving has no effect, mimalloc
   ~3%, four node-bound 24-thread processes 1.42x. Root-cause it (perf c2c / fp profile at K=24 and K=96 on socket 1:
   shared written cache lines such as Arc/RwLock counters, reducer caches, Symbolica global state; check the
   harness's own per-thread slots as a control), test a per-CCD multi-process variant (12 x 8 threads), replicate the
   K=1 base. The outcome decides whether the epoch engine uses one process with ~90 inspector threads or several
   inspector processes, and every W96 throughput projection must be rescaled by it.
2. **Oracle fixes (section 7.1 verifier findings)**: `--require-closure` must require `independently_verified` (or
   emit a distinct non-PASS verdict) when re-inspection is partial; mutation rows must assert the exact class and the
   per-node check; add a consistently-hidden-frontier mutation and closure-flip assertions; unit/end-to-end tests for
   alias-chain coverage, partial-anchor containment, digest binding, per-node event parity; measure gen-7 memory
   (verifier estimate 70-90 GB); fix the mislabels (successor counts, 3.8e9 inclusions). Then merge the oracle
   branch: every later gate depends on it.
3. **Census (7.8)**: review/commit the conservative `cover.rs` change, re-run the batch, write the W0.7 note.
   Gate 0.7 PASSES (70.1% D-only / 89.0% hull of gen-7 Apply CPU has a residual <= 10% of points in <= 8 pieces).
4. **Inputs (7.9)**: re-run the plan-v3 control probe, run probe-I1 (60 min), run the C-5F I2 A/B with audit (the W1.3
   witness gate); bring the I1b gate wording and the I1b go/no-go (four-loop BMW I1b does not drain) to the D-session.
5. **Knobs (7.6)**: tabulate session D, strict identity for b48/b50 build arms, fill RESULTS.md placeholders, fix the
   2 frozen event-key-set tests broken by the `dispatch_order` telemetry key on that branch, commit the tooling.
6. **Baseline (7.5)**: fix `m1_analyze.py` (None VmHWM), analyse the completed 25-min run2 (checks/request drifted
   ~1,000 → ~1,650), re-score gate 0.5 on run2, update RESULTS.md section 7.
7. **Intel (7.7)**: fill the RESULTS.md tables, write the gate 0.4 verdict (the SoA kernel passes the >= 4x criterion:
   SoA-id 7.9-9.5x less CPU per tested candidate, SoA-pattern 4.8-7.5x; note that pattern ordering loses min-ID early
   exit, so canonical mode favours SoA-id), build the uncommitted idxreplay v4 edits or drop them.
8. **Combined four-loop run (section 8)**: audit the W96 runs, repeat four-all at Ready and Ordered W96 (>= 3 x 1 h),
   explain the non-drained W96 Ready repeat (rank-13 flood in class 993?), run the independent adversarial audit,
   then push/merge `fable_5_1-c4l-combined` and make four-all (at least Ordered) the C-4L gate of the master plan.
9. **W0 gates + 0.11 work-volume synthesis + D-session memo** (the gatekeeper step that never ran): evaluate every
   gate, give a projected work factor per lever, and present D1-D8 to the owner in plain language (D5 is resolved:
   600 GB, CPUs reservable; G2' has strong evidence; I1b and I1b-gate wording need a decision).
10. Persist every W0 `RESULTS.md` as a `docs/research/fable51_w0_<lane>_2026-09-27.md` note and push the branches
    (tools only; no legacy-engine changes are merged from W0 except the oracles and harness tests).

### 9.2 Adjustments W0 already forces on W1-W5
- **Native scaling**: not ~linear in one process (3.75x per-native cost at 96 threads). Before W2 exit either fix the
  contention or design the epoch engine with multiple inspector processes (the harness showed separate processes
  help while NUMA placement and the allocator do not). Rescale every throughput band.
- **Kernel**: adopt the SoA compare kernel (gate 0.4 criterion met); choose SoA-id vs SoA-pattern with the canonical
  (min-ID) mode in mind.
- **Build**: adopt `[profile.campaign]` (fat LTO, codegen-units 1) in W1.2/N3; mimalloc with malloc override is
  likely (x0.87-0.93) pending the W48/W50 confirmation; drop znver4 and glibc tunables.
- **Dispatch order**: none of FIFO alternatives is adopted on the legacy engine; design epoch priority around peak
  pending (bounded enqueue depth, age bound) and re-measure there.
- **Work volume**: G2' residual anchors promoted (census gate PASS; falsifier oracle 0.33x inspections at 1.000x
  points on C-HOT). G1 widening rejected; dense cells deprioritized.
- **Symbolica**: no upgrade; implement N1 on the vendored finite-field API.
- **Inputs**: I1 for the 40 L* owners likely; I1b doubtful; I2 promising (-56% Route→Route edges in a W6 pilot).
- **Four-loop validation**: four-all (A4 basis, 16 owners, 508 routes) replaces the per-family C-4L gate once its
  W96 drain robustness is understood; keep FG/BMW/H/X for byte-identity checks against history.

### 9.3 Then W1-W5 per the master plan (section 6.5), ending with the launch hand-over
When the launch criteria (section 6.7) hold, freeze the binary and inputs, prepare the campaign directory, and give
the owner the exact `--start` command for Zellij tab `fable_5_1` (template in section 10.3, with the epoch policy,
100 CPUs on socket 1, `--max-memory-bytes 600000000000`). Never start it yourself.

## 10. Operational handbook

### 10.1 Builds, tests, gates
```sh
cd /common/dev/rustred            # or a worktree
export TMPDIR=$PWD/TMP
flock -w 14400 /common/dev/rustred/TMP/locks/build-0.lock \
  nice -n 5 taskset -c <cpus> nix develop --command cargo build --release --locked --offline -p rustred-app
flock -w 14400 /common/dev/rustred/TMP/locks/build-0.lock \
  nice -n 5 taskset -c <cpus> nix develop --command cargo test --release --locked --offline -p rustred-app --lib
nix develop --command cargo fmt --all -- --check
nix develop --command cargo test --release --locked --offline -p rustred-app --test cli_routed_campaign
nice -n 5 taskset -c <>=8 cpus> nix develop --command python -m unittest discover -s examples/python -p 'test_*.py'
```
A release rustc of `rustred_app` peaks at 35-49 GB; check `MemAvailable >= 150 GiB` first. Full release build from
cold ~25 min; incremental rustred-app ~5-6 min. Known timing-flaky tests (pass alone; fail ~1 in 3-7 full runs under
load): `cli::shards::supervisor::tests::orphan_child_retains_campaign_lock_until_exit` (a forked sibling test child
briefly inherits the lock fd), `walking::execution::ready_tests::ready_late_native_fault_after_cancellation_disallows_pause`,
and in the core crate `persistence::catalog::tests::arbitrary_exact_expressions_roundtrip_with_deduplicated_values`
(symbol-table race). Harden them in W1.4.

### 10.2 Controls
```sh
# per-family complete four-loop run (A<=19, R<=12, D>=7) and the five-loop 1,324-tuple finite control
nix develop --command python TMP/fable51-controls/run_control.py --binary BIN --family fg|bmw|h|x|five-finite \
  --label LABEL --cpus 250-255 --policy ordered|ready [--workers N]
nix develop --command python examples/python/compare_walk_records.py --mode strict REF/result.json NEW/result.json
nix develop --command python examples/python/audit_owner_domain_walk.py RUN_DIR [--require-closure]   # oracle branch
nix develop --command python TMP/fable51-controls/resume_control.py --first BIN_A --second BIN_B --family fg \
  --label L --cpus C6 --policy ordered --stop-at-committed 40000 --reference REF/result.json --mode strict
```
Baselines: `TMP/fable51-controls/RESULTS.md` (32fdec), `TMP/fable51-controls/wave2-*` (102adcc3 vs 53e672fc),
`TMP/w0/oracle/runs/` (fresh 4a17f9c7 C-4L and C-5F), `TMP/w0/falsify/CHOTSUB.md` (C-HOT-sub definition: hot-owner
full orthant R <= 1, A <= 12, drains in 14.1 min at W12 on the legacy engine; secondary R <= 2, A <= 11). Five-loop
Ready multisets are not reproducible run to run (two 102adcc3 runs differ by ~700 natives): judge Ready by audit.
Timings on this host are only valid when foreign load on the run's CPUs is <= 10% (plan §7); record it.

### 10.3 Campaign preparation and launch (for the final step; the OWNER launches)
Today's launcher (to be updated for the epoch policy/CP6/v4 inputs in W2/W5):
```sh
cd /common/dev/rustred && export TMPDIR=$PWD/TMP
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --prepare-from campaigns/five-loop-dependency-closure --queries <v4 queries.json> \
  --attach <entry-plan-receipt.json> --attach <skeleton-classification.json> \
  --campaign-directory campaigns/<new-name> --executable <frozen binary> --workers 100 --cpus 128-227 \
  --publication-policy <epoch> --checkpoint-interval-seconds 14400 \
  --max-memory-bytes 600000000000 --ram-guard-margin-percent 5
# the owner then types, in Zellij session rustred, tab fable_5_1:
env TMPDIR=/common/dev/rustred/TMP nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/<new-name> --start
```
Monitor read-only: `nix develop --command python examples/python/campaign_monitor.py campaigns/<name>/runs/<run> --once`
(`status.json` has a `derived` block). Check `progress.work.frontiers == 0` in the first minutes. The v2 supervisor
stops a campaign when host MemAvailable <= 20 GB (host reserve) and hard-stops at <= 5 GB: keep builds away from a
running campaign's memory. Do not pin the preparing shell with taskset (the launcher checks affinity). Placement
lesson: `zellij action new-pane` opens in the tab focused by an attached client (see `FABLE_HANDOFF.md` §2b).

### 10.4 Host
2x EPYC 9754 (Zen 4c, AVX-512), 384 logical CPUs: socket 0 = NUMA nodes 0-3 (CPUs 0-127 + SMT siblings 256-383),
socket 1 = nodes 4-7 (CPUs 128-255, no SMT, 564-594 GB local memory). ~1.13 TB RAM; ZFS ARC 200-350 GB counted as
used; zram swap 500 GB; THP effectively unavailable. Single NVMe pool `zroot` (2 data errors; accepted).

### 10.5 Workflow tips
- The Workflow tool's pre-launch hook timed out several times ("host client may be unreachable"); writing the
  script to a file and launching with `scriptPath` worked; the Agent tool also worked as a fallback.
- Scripts must be plain JavaScript; an apostrophe inside a single-quoted string breaks parsing.
- Subagents hit a weekly usage limit at the end of this session: prefer workflows that commit progress and write
  `TMP/<lane>-progress.md` continuation notes (the W0 lanes did).

## 11. Open issues, risks and gotchas
- Termination of the five-loop walk is not established (R1); RAM wall 10-250 h at the measured discovery rate for
  a 5-20x faster engine [E]; the work-volume lane is therefore launch-blocking.
- Native scaling to ~90 concurrent inspectors was never measured beyond ~6.5 (W0.3 scaling sessions open).
- Oracle gaps found by its verifier (section 7): `--require-closure` without full re-inspection reports PASS; the
  mutation matrix accepts a class if merely present; no mutation for a consistently hidden frontier; untested
  verifier paths (alias-chain coverage, partial-anchor containment, digest binding, per-node event parity).
- Measurement noise: 43-63 foreign busy CPUs on socket 1 during W0; timing A/Bs need a foreign-load check.
- The legacy engine (Ordered/Ready, CP5) is kept read-only as the oracle lane until after launch (R15).
- The v2 status.json's "3.7 KB/domain" is an artefact of its RSS plateau; the real figure is 0.38-0.48 KB/domain on
  the wave-2 binary.
- `result.json` `domains` arrays do not scale (typed records + Rust tooling needed, W2.6).

## 12. Document index
- Governing: `GOAL.md` (last section), `FABLE_5_1_five_loop_vacuum_plan.md` (§6 decision log),
  `docs/research/fable51_next_push_master_plan_2026-09-27.md`, `docs/research/fable51_v3_engine_design_2026-09-27.md`.
- Earlier handoff (Fable 5.1 + this session's §8): `FABLE_HANDOFF.md`.
- This session's research notes: `fable51_c1_records_sidecar_2026-09-26.md`, `fable51_c2_compact_state_2026-09-26.md`,
  `fable51_c3_csr_edges_2026-09-26.md`, `fable51_ready_multi_prefix_gate_2026-09-26.md`,
  `fable51_restore_at_scale_2026-09-27.md`, `fable51_wave2_profiling_2026-09-27.md`,
  `fable51_coordinator_relief_design_2026-09-26.md`, `fable51_root_blockers_2026-09-27.md`.
- Designs and reviews from Fable 5.1: `fable51_design_{checkpoint_memory,scheduler_admission,inputs_pilots_launch}_2026-09-26.md`,
  `fable51_review_{checkpoint,scheduler}_branch_2026-09-26.md`, `five_loop_completion_levers_2026-09-26.md`,
  `five_loop_qcd_feynman_entry_class_2026-09-26.md`.
- W0 results notes (not yet committed to docs/): `TMP/w0/<lane>/RESULTS.md`.
- Older notes the master plan relies on: `four_loop_saved_cover_control_2026-09-24.md`,
  `four_loop_helper_bounds_2026-09-25.md`, `guard_obstruction_triage_2026-09-22.md`,
  `radical_parallel_architecture_2026-09-24.md`, `five_loop_slow_inspection_parallelism_2026-09-24.md`,
  `domain_admission_index_next_step_2026-09-22.md`, `five_loop_auxiliary_scope_2026-09-25.md`.
