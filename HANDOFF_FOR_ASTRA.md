# HANDOFF FOR CODEX ASTRA (from Claude Opus 5.5, 2026-09-28 ~23:30 UTC)

RustRed, repo `/common/dev/rustred`, official branch **`fable_5_1`** (origin `github.com:alphal00p/rustred`).
Owner: Valentin Hirschi (theoretical physicist; GammaLoop/Vakint author; git identity
`ValentinHirschi <valentin.hirschi@gmail.com>`). This file is self-contained: read it fully before acting. Older
context: `HANDOFF_opus_5_5.md` (sections 0, 0.1 = binding audit directives and owner answers), `FABLE_5_1_CRITIQUE.md`,
`TMP/progress/orchestrator_decisions.md` (numbered binding decisions 1-19). Labels: **[M]** measured (file cited),
**[E]** estimate. Never present anything as an ETA or a closure claim (`family_closure_claim` stays false).

---

## 0. TL;DR (do these first, in order)

1. **A campaign is RUNNING: LC1** (`campaigns/five-loop-qcd-feynman-d9d10-lc1`, run
   `runs/20260928T215834.643394Z`, started by the owner ~21:58 UTC 2026-09-28 in Zellij session `rustred`, tab
   `fable_5_1`). Never start/stop/signal/resume it yourself. Monitor it read-only (section 9.4). At 81 min [M]:
   18.0M discovered, 5.53M completed, 7.37M pending, 8/67 roots closed, 0 frontiers, RSS 17.4 GB, 3.5M
   completions/h (1 h window), coordinator duty 79%, max rank 17, pending growth per completion 1.29.
2. **LC2 is DONE** (update 2026-09-29 ~00:40 UTC): the Symbolica-PATCH-FREE reference version is merged into
   `fable_5_1` (`f36ba878`, tip `46baa0f5`, pushed). vendor/symbolica = plain upstream dev `ef0db494` (no patch;
   `patches/` deleted); format-6 preflight; `examples/python/convert_native_v5_to_v6.py`; thread-owned contexts.
   Gates [M]: core 2844/0/32, app 822/0/12, cli 6/6, Python 260 OK; strict Ordered identity vs 4a17f9c7 (records and
   containment_checks) on FG/BMW/H/X, four-all, four-all-p5, C-5F; oracle 7/7; smoke (start, periodic save, SIGINT
   pause, resume, stop, 0 frontiers). Launch binary `TMP/fable51-controls/bin/rustred-lc2-fd9b9ac9`.
   `campaigns/five-loop-qcd-feynman-d9d10-lc2` is PREPARED (not started). The owner switches when he decides:
   Ctrl-C once in the LC1 pane (wait for the durable checkpoint and exit 4), then in a plain shell:
   `cd /common/dev/rustred && env TMPDIR=/common/dev/rustred/TMP nix develop --command python examples/python/production_saved_owner_campaign.py --campaign-directory campaigns/five-loop-qcd-feynman-d9d10-lc2 --start`.
   IMPORTANT: `fable_5_1` now reads only format-6 artifacts; LC1 can only be resumed with its own frozen binary in
   `campaigns/five-loop-qcd-feynman-d9d10-lc1/bin/` (never rebuild an LC1-compatible binary from `fable_5_1`).
   Section 5.1 below is kept for provenance; its steps are complete.
3. Then continue the upgrade lanes (section 6): production G2' residual anchors (biggest measured work-volume lever;
   can be activated on a paused campaign at resume), the frontier rescue (owner requirement), and the epoch engine
   (W2, the big throughput lever; owner gate: must beat the legacy comparator runC by >= 1.5x).

## 1. Owner rules and preferences (binding)

1. **Campaigns:** never start, stop, signal or resume a campaign; never write into existing campaign directories
   (`five-loop-dependency-closure`, `five-loop-qcd-feynman-d9d10`, `five-loop-qcd-feynman-d9d10-v2`,
   `five-loop-qcd-feynman-d9d10-lc1` are read-only evidence; you may CREATE a new prepared campaign directory). Give
   the owner the exact command; he types it in Zellij session `rustred`, tab `fable_5_1`
   (`XDG_RUNTIME_DIR=/run/user/1125`). The shell must not be pinned with `taskset` (the launcher checks affinity).
2. **Launch criterion (owner, 2026-09-28):** launch as soon as a stable, reasonably tested version exists and the
   campaign has a realistic shot at finishing; then monitor and let the running campaign's performance decide the next
   improvement. Do not over-validate or over-review; iterate.
3. **Frontiers:** a frontier for which a known rescue exists must NEVER end the campaign. A frontier stop is a pause
   (checkpoint kept); the rescue must be applicable at resume without losing certified progress (section 6.2).
4. **No own computer algebra.** Use Symbolica (now plain upstream dev `ef0db494` in LC2; section 5.1). Lattice/box
   geometry and decision logic are fine.
5. **Validation:** every engine-affecting change is validated on the complete four-loop run (FG, BMW, H, X at the
   A<=19 / R<=12 / D>=7 saved-cover envelope) plus the combined four-loop control (four-all, four-all-p5) plus the
   five-loop control C-5F (and C-HOT-sub r1a12 for G2'-type levers). Oracle gate contract in section 9.2.
6. **Pilots <= 1 hour**, restore included; extrapolate and label estimates.
7. **Host:** shared (users nfink/postgres+gammaboard, ben, codex-3 run jobs). Never touch other users' processes or
   files (e.g. user ben's uncommitted edit `crates/rustred-feynkit/src/lib.rs` in the main tree and his files under
   `tools/research/symbolica_arc_mre/`: never add/commit/stash/restore them; stage explicit paths only).
8. **Owner decisions in force** (details in `TMP/progress/orchestrator_decisions.md`): 600 GB RAM cap; host
   MemAvailable save-and-stop floor 50 GB; socket 1 shared (no cpuset); no ZFS ARC cap; at the RAM wall: stop and
   deliver (symbolic closure of certified roots + open cones); D7 frontier policy stop (plus rescue, rule 3); D8
   deliverable = symbolic closure; D2 = G2' residual anchors in UNION form (an anchor may be any merged record whose
   domain is fully discharged: Native or a validated merged G2' residual record; well-founded in merge order);
   D6 symmetry canonicalisation and D1(b) piece certification: no (measured < 2x); widening: no; I1b dropped;
   I1 not shipped (its only rescue needs a fresh campaign; revisit with in-run rescue); I2 rebuild failed its gate
   (not shipped); per-CCX replicas superseded by Symbolica's thread-owned context API (no Symbolica patching);
   epoch engine continues only if its skeleton beats the legacy comparator by >= 1.5x (interleaved, load-matched,
   not overly strict), else fall back to MVP-B (epoch core) / MVP-A (legacy + levers).
9. **Evidence discipline:** [M] with run dir / file / sha256, [E] otherwise; separate measured from inferred; small
   reviewable commits with `git -c user.name=ValentinHirschi -c user.email=valentin.hirschi@gmail.com commit`, message
   ending with a blank line and a `Co-Authored-By:` trailer for your model; push `fable_5_1` after each green
   milestone.
10. **Tooling:** `python3` is not on PATH: `nix develop --command python ...` (or
    `/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python` for stdlib scripts);
    `export TMPDIR=/common/dev/rustred/TMP`; `SYMBOLICA_LICENSE` is set; run tests with
    `RUSTRED_TESTS_REQUIRE_LICENSE=1` (license-gated tests fail instead of skipping).

## 2. Host, CPUs, memory and locks WHILE LC1 RUNS (binding, decision 19)

- Host: 2x AMD EPYC 9754 (Zen 4c, 8 cores per CCX/L3), 384 logical CPUs. Socket 0 = CPUs 0-127 + SMT siblings
  256-383 (NUMA 0-3); socket 1 = CPUs 128-255 (NUMA 4-7, no SMT). ~1.13 TB RAM; ZFS ARC 90-350 GB counted as used;
  MemAvailable 600-820 GB observed. `perf_event_paranoid=2` (user-mode only; perf at
  `/nix/store/wizn21b9virxqcnm4n89b09pgqkaxfn3-perf-linux-7.2.5/bin/perf`).
- **LC1 owns CPUs 128-227 and up to 600 GB.** Never run anything on CPUs 128-227. Use CPUs 0-127, 256-383 and at most
  228-255 for < 24 threads.
- The orchestrator HOLDS `TMP/locks/socket1.lock` (process `sleep infinity`, pid 1893886; note in
  `TMP/locks/socket1.holder`) so no agent can start a socket-1 session. Keep it while any campaign runs on socket 1.
  Release with `kill 1893886` only when socket 1 is free.
- Memory: start a heavy job (release build 35-60 GB, gen-7 clone analysis 40-60 GB, verifier) only if MemAvailable
  >= 250 GiB, under `flock -w 14400 TMP/locks/heavy.lock` (one heavy job at a time); abort if MemAvailable < 150 GiB.
  LC1's supervisor save-and-stops when host MemAvailable <= 50 GB: never push the host there.
- Build locks: `TMP/locks/build-{0..7}.lock` (wrap every cargo build/test: `flock -w 14400 TMP/locks/build-N.lock`).
- A read-only watcher may be running: `TMP/lc1-monitor/watch.py <run dir> <seconds>` appends 5-min samples to
  `TMP/lc1-monitor/series.jsonl` (restart it if not running; it exits on anomalies).

## 3. Campaigns (state)

| Campaign | State | Binary | Notes |
|---|---|---|---|
| `five-loop-qcd-feynman-d9d10-lc1` | **RUNNING** since 21:58Z 09-28 | `bin/rustred-4606cc4b...` = `TMP/fable51-controls/bin/rustred-lc1-4606cc4b` (sha256 4606cc4b4a54..., `[profile.campaign]` fat LTO, walk semantics 1, CP5; vendored Symbolica 953e26e2 + old heap-pow patch) | plan-v3 inputs identical to v2 (queries sha 2c714860, selection d2667dc9, 67 owners 1.28 GB format 5), W100 on CPUs 128-227 (67 inspectors / 32 admission helpers / 1 coordinator), Ready, lookahead 256, frontier policy stop, 600 GB cap (soft 570), host floor 50 GB, swap guard, checkpoints every 4 h |
| `five-loop-qcd-feynman-d9d10-lc2` | PREPARED, not started | `rustred-lc2-fd9b9ac9` (patch-free Symbolica ef0db494, format 6) | same config as LC1, owners converted v5->v6 (section 5.1) |
| `five-loop-qcd-feynman-d9d10-v2` | stopped (paused gen 7) | 102adcc3 | evidence: 74.16M discovered, 8/67 roots, 0 frontiers in 18.6 h |
| `five-loop-qcd-feynman-d9d10` (interim) | stopped (gen 8) | 32fdec09 | unbounded-A helpers -> 1,299 frontiers; six refused fresh-start receipts from 10:56/12:07Z 09-27 |
| `five-loop-dependency-closure` | stopped (paused) | 32fdec09 | source for `--prepare-from` |

Frontier risk of LC1: same inputs as v2 (0 frontiers over 74M domains); the only past frontiers came from
unbounded-A helpers reaching the 7 guard-sensitive owners, which plan-v3's bounded helpers exclude. LC1 explores
beyond v2's reach, so zero cannot be proven; a frontier pauses the run (exit 4, stop reason `frontier_policy`).

## 4. Code status: what is on `fable_5_1` (tip `d9271082`, pushed)

- Wave 2 (CP5 records sidecar, compact queue state, CSR edges, Ready multi-prefix gate, upgrade path).
- W0 (all merged): oracles (`rustred walk-verify-closure`, extended `examples/python/audit_owner_domain_walk.py`,
  `examples/python/oracle_mutation_matrix.py`, `examples/python/assert_oracle_pass.py`, exact `covered_by_union`
  predicate in `lattice.rs`), native re-inspection harness (test-only, `walking/reinspection.rs`,
  `tools/research/harness/`), census/route-census/intel/baseline/wv/symbolica tools under `tools/research/`,
  combined four-loop inputs (`examples/input/four_loop_combined/`), planner `--helper-bounds-from`, W0 notes
  `docs/research/fable51_w0_*_2026-09-27.md`, W0 memo `docs/research/fable51_w0_results_2026-09-27.md`, W2.0 epoch
  protocol note rev 2 `docs/research/fable51_w2_epoch_protocol_2026-09-28.md` (2,175 lines; the epoch spec),
  ERRATA sections in the master plan and the v3 design note.
- W1 ops (merged): `--frontier-policy record|stop` (+ production launcher default stop), host-aware RAM guard
  (50 GB floor default, swap guard, ARC telemetry, refuses resume after 2 zero-progress RAM-guard stops), flaky-test
  hardening, license skip markers + `RUSTRED_TESTS_REQUIRE_LICENSE`, `[profile.campaign]` (fat LTO, cgu 1) and an
  opt-in `mimalloc` feature (with C malloc override; not used in LC1: +3-20% RSS).
- W1.1 SoA admission kernel + O(1) storage telemetry + kernel2 (retire compaction moves rows only after the first
  empty row; watermark shortcuts in `find_controlled` and the prepared split). Strictly result-identical to
  4a17f9c7 (records AND containment_checks) on FG/BMW/H/X, four-all, four-all-p5, C-5F; gen-7 differential 0
  mismatches (169M fwd / 903M rev lookup pairs, 1.5e9 sweeps).
- LC1 ship/launch notes: `docs/research/fable51_lc1_ship_2026-09-28.md`, `docs/research/fable51_lc1_launch_2026-09-28.md`.
- Suites at `d9271082`: rustred-app lib 822/0/12, cli_routed_campaign 6/6, Python 257 OK / 1 skip, fmt clean.
- Reference binaries in `TMP/fable51-controls/bin/`: `rustred-4a17f9c7` (canonical legacy reference for all
  identity gates), `rustred-8da58390` (W0 integration), `rustred-lc1-4606cc4b` (LC1), `rustred-lc2-fd9b9ac9` (LC2).
- NOTE: the Symbolica submodule on `fable_5_1` is still 953e26e2 + the heap-pow working-tree patch until LC2 is
  merged (section 5.1).

## 5. Branch inventory (all other branches were retired)

| Branch (worktree) | Content | State |
|---|---|---|
| `fable_5_1` (main tree) | official reference | pushed `d9271082` |
| `fable_5_1-lc2` (`.claude/worktrees/fable51-lc2`, MERGED into fable_5_1 at f36ba878; retire) | patch-free Symbolica: `6078492d` gitlink ef0db494 + heap-pow patch removed; `d3732591`+`1ee36253` thread-owned contexts/seals; `5e9f200d` preflight accepts the linked atom-format byte, rejects v5 naming the converter; `b09a602a` `examples/python/convert_native_v5_to_v6.py` + tests | local; finish + merge (5.1) |
| `fable_5_1-symeval` (`.claude/worktrees/fable51-symeval`) | evaluation of dev+939c4de8+7b31114c; source of the thread-owned-context commits (c50d838f, bf6195c7) | pushed; retire after LC2 merge |
| `fable_5_1-v3-g2prod` (`.claude/worktrees/fable51-g2f`) | production G2' residual anchors on the legacy engine | pushed; paused (6.1) |
| `fable_5_1-v3-rescue` (`.claude/worktrees/agent-ade877816b107b1cf`) | frontier rescue + supervisor auto-rescue | pushed; paused (6.2) |
| `fable_5_1-v3-epoch` (`.claude/worktrees/fable51-epoch`) | epoch engine W2 stage S2 + fix round | pushed; paused (6.3) |
| tags `archive/fable_5_1-{coord-1ab,coord-hit,coord-json,v3-g2falsify,v3-i2,v3-intel,v3-kernel2,v3-knobs,v3-widen}` | retired unmerged work (G2' falsifier prototype, I2 rebuild note, admission-trace feature, kernel2 note 65d2cc25, dispatch-order knob, G1 widening falsifier, old coordinator WIP) | pushed tags |

Retired worktrees' `TMP/` evidence was moved to `TMP/archive-worktrees/<worktree>/TMP` (list and heads in
`TMP/archive-worktrees/HEADS.txt`, log `cleanup.log`). Each worktree has its own `vendor/symbolica` checkout
(953e26e2 + heap-pow patch, except fable51-lc2 = clean ef0db494).

### 5.1 LC2 (COMPLETED 2026-09-29 ~00:40 UTC; kept for provenance)

Evidence so far (`TMP/progress/lc2.md`, `TMP/lc2/`): build rc 0 (campaign binary 3,011 s fat LTO); launch binary
`TMP/fable51-controls/bin/rustred-lc2-fd9b9ac9` (sha256 fd9b9ac96a50513d8119a43938313e8a729110d6a3815b0f949cec4acb1d9d60,
built at 5e9f200d); suites core 2844/0/32 (incl. both native_heap_pow regressions WITHOUT any patch), app 822/0/12,
cli 6/6; strict Ordered identity of LC2 on v6 inputs vs 4a17f9c7 on v5 inputs: FG 98,909 / BMW 158,951 / H 24,929 /
X 47,193 / four-all 65,444 / four-all-p5 68,483 / C-5F 1,273,376 records, 0 differing, containment_checks equal
(`TMP/lc2/gates/gates.tsv`); v5 input is refused with a message naming the converter; converted inputs in
`TMP/inputs-v6/{c5f,region-control/{fg,bmw,h,x},four-all,four-all-p5,five-loop/inputs,five-loop-source/inputs}`
(converter output byte-identical to owners regenerated under the new Symbolica); `campaigns/five-loop-qcd-feynman-d9d10-lc2`
prepared (steering options == LC1; owner sha256 = converted LC1 owners; steering sha256 f3330f58...; validation mode).
The LC2 workflow may have finished some of the steps below after this file was written: check
`TMP/progress/lc2.md` and `TMP/progress/lc2.report.json` and `git log fable_5_1` first.

Remaining steps:
1. Oracle gate on the LC2 outputs `TMP/lc2/runs/lc2-new/*` (all 7): `rustred walk-verify-closure --command <run>/command.json --require-closure --reinspect all --threads 24` with the LC2 binary, then
   `nix develop --command python examples/python/assert_oracle_pass.py <report>`; pair with the Python audit.
2. Smoke (~8 min) of the SAME preparation on socket-0 CPUs in a TMP copy (prepare with `--campaign-directory
   TMP/lc2-smoke/campaign --cpus 32-63 --workers 32 --checkpoint-interval-seconds 300` or the launcher's equivalent):
   start, SIGINT pause (exit 4), `--resume --start`, stop file; 0 frontiers, restore OK.
3. `docs/research/fable51_lc2_patchfree_2026-09-29.md` (what changed, gates, binary sha256, input digests, launch
   procedure) on `fable_5_1-lc2`.
4. Merge into `fable_5_1` in the main tree (`git merge --no-ff fable_5_1-lc2`; stage explicit paths only). The main
   tree's `vendor/symbolica` carries the OBSOLETE heap-pow working-tree patch: after the merge run
   `git -C vendor/symbolica checkout -- . && git -C vendor/symbolica checkout ef0db494 && git submodule status`.
   Re-run in the main tree: `cargo fmt --all -- --check`, the Python suite; push `fable_5_1` and `fable_5_1-lc2`.
   Update all docs/scripts that still tell agents to apply `patches/symbolica/heap-pow-wide-radix.patch` (the LC2
   commit removed the file; grep again after the merge, e.g. HANDOFF_opus_5_5.md section 10, worktree setup notes).
5. Tell the owner: (a) in the LC1 pane press Ctrl-C once (save + exit 4); (b) then start LC2:
   ```sh
   cd /common/dev/rustred && env TMPDIR=/common/dev/rustred/TMP nix develop --command python examples/python/production_saved_owner_campaign.py --campaign-directory campaigns/five-loop-qcd-feynman-d9d10-lc2 --start
   ```
   (verify the exact flags against the prepared steering and the launcher's `--help` first). Keep the socket1.lock
   hold. LC1's final checkpoint stays as evidence.
6. Symbolica state after LC2: upstream dev `ef0db494` = Ben Ruijl's dev rebased on main: contains 939c4de8
   (multithreaded polynomial context handling + public `zero_with_new_context`/`clone_with_context_of` +
   thread-local ahash state), 7b31114c (heap_pow overflow fix; replaces our old local patch) and the
   `poly::reconstruction` module RustRed uses. It refuses export format 5: all native artifacts must be format 6
   (`examples/python/convert_native_v5_to_v6.py` rewrites only the state-version field and per-frame format byte of
   RustRed's Num-only artifacts; verified byte-identical to regeneration). Artifacts that still need conversion when
   used: any owner programs outside `TMP/inputs-v6/` (four-loop Vakint owners in other TMP dirs, candidate bundles,
   catalogs): convert with the tool, never edit by hand.

## 6. Remaining work (precise specifications)

Priorities: (1) LC2 (5.1); (2) G2' production (6.1) and frontier rescue (6.2), both deliverable to the RUNNING
campaign at a pause via the upgrade path if their bindings allow (6.5); (3) epoch engine (6.3); (4) native and
scheduling levers (6.4). After each lane: rebase/merge onto `fable_5_1` (post-LC2, so rebuild against Symbolica
ef0db494 and use `TMP/inputs-v6/` inputs), re-run the gates, merge, push.

### 6.1 Production G2' residual anchors (legacy engine) - branch `fable_5_1-v3-g2prod`

What it is: when a newly admitted Apply domain Q is dispatched, find anchors in the same (phase, owner) bucket that
were merged strictly before Q's dispatch and fully discharged (a Native record, or a merged validated G2' residual
record = its residual plus its own anchors, resolved in merge order); if an anchor covers Q restricted to D >= c
(D-only cut; the census shows D-only residuals are almost always 1 piece, <= 2), inspect only the residual Q|D<c and
record an edge Q -> anchor (reusing the partial-anchor mechanism of `walking/initial_overlap.rs`). Measured value
[M, falsifier arm u, `docs/research/fable51_w0_g2falsify_2026-09-28.md`, `TMP/w0/g2falsify/gate-table-2.md`]:
0.50-0.54x inspector record-seconds, 0.79-0.80x scheduled domains, 0.82-0.86x peak pending on C-5F / C-HOT-sub
r1a12 (anchor = native only: 0.68-0.77x). It is the only measured domain-volume lever.

State (`TMP/progress/g2prod.md`, `TMP/w1/g2prod/`, note draft `docs/research/fable51_w1_g2prod_2026-09-28.md` on the
branch): commits c3140a67 (flag `--g2-residual-anchors off|union` bound into the request digest), 51f30109 (Python
oracle checks for G2' records, mutation rows), 84f6f8e4 (lane tools), 8c84fd35 (**activation on resume**:
`--g2-activate-on-resume` switches a paused `off` campaign to `union`, recorded as manifest metadata
`g2_activation {from, to, generation, bindings}`; pin fixes; precise verifier mutations), c9874f9c, 97563d36, 0db0f01d
(oracle fixes: audit stream positions, dropped-anchor-edge mutation), 80e80b5a (audit union-cover test vs
enumeration). Binaries: B1 `TMP/w1/g2prod/bin/rustred-87be7311`, B2 `rustred-4e35aa54`, B3 (build log
`TMP/w1/g2prod/logs/build-test-norun-b3.log`). [M]: FG union drains, verify PASS 248/248; FG activation drill
(flag-off W6 paused at 60,843 committed, resumed with activation, drained; verify/gate/audit PASS); C-5F Ready
activation drill drained (70,611 G2' records); all post-main oracles PASS on c5f-{ord,rdy}-union-r{1,2},
hotsub-union-r{1,2}; mutation matrix 59/61 with the 2 misses fixed in 0db0f01d; lib suite green; Python 257 OK.

To finish:
1. Rebuild at the branch tip (B3) and re-run: the mutation matrix (G2' rows must FAIL: residual shrunk by one point,
   anchor stamp >= dispatch, anchor neither Native nor validated G2', dropped anchor edge, cyclic anchor), flag-off
   strict identity vs 4a17f9c7 (FG/BMW/H/X, four-all, C-5F), flag-on drains + oracle gate (C-5F Ordered+Ready,
   C-HOT-sub r1a12 Ready W12, FG/BMW/H/X, four-all), the activation drill, lib/cli/Python suites. Report the
   domain-volume table vs flag-off (scheduled domains, peak pending, pending growth per completion, natives,
   inspector-seconds) and compare with the falsifier's arm u. n=1 per arm is acceptable (owner).
2. Rebase onto post-LC2 `fable_5_1` (Symbolica ef0db494, v6 inputs), rebuild, re-run step 1's identity + one union
   drain + oracle. Merge, push.
3. Deployment decision (owner): (a) activate on the running campaign at its next pause via
   `--resume --upgrade-executable <G2' binary> --g2-activate-on-resume` if the upgrade probe accepts it (walk
   semantics must allow it: check `rustred walk-semantics-version`; the activation is recorded in the manifest and
   audited), or (b) enable from the start of the next campaign. Prefer (a) if the drill on a paused C-5F copy passes.
4. Known performance item: the anchor search must stay indexed (the prototype's linear scan visited 1.7-3.1e9
   anchors per control).

### 6.2 Frontier rescue (owner requirement) - branch `fable_5_1-v3-rescue`

Design (implemented): a frontier stop is a pause; at resume an append-only, digest-chained input amendment adds
protected bounded helper queries covering the physics region of the frontier-bearing helper; physics queries are
certified PER QUERY through any closed containing node (helper roots and physics queries reported separately), so a
frontier-bearing unbounded helper no longer blocks physics certification; the checkpoint binding accepts only
amendments whose digests chain from the original request; the supervisor classifies frontiers after a
frontier-policy stop and, for a known class (unbounded-A helper in a guard-sensitive owner: rescue = bounded
plan-v3-style helper for the owner/region), generates the amendment and resumes automatically (bounded attempts;
unknown class = stop and wait for the owner). Commits: 34becd40 (digest-chained amendments, taint quarantine,
per-query certification), 048324af (supervisor auto-rescue, launcher steering v4), 1c51a000 (audit of amended walks),
24186e06 (class scope, drain trigger), 3abc3c20 (non-monotone lookup quarantine, dead marks, taint and liveness
passes), 9a15b556 (verifier: rescue-abandoned records; linear coverage scan fix for a quadratic case), 81f5e8d3
(dead-cone abandonment, worker-view epochs, class-scope planner), 319f2683, 7ec2d16f (verifier indexed coverage
candidates for wide nodes). State (`TMP/progress/rescue.md`, `TMP/w1-rescue/`): dev9 `964bf4a7` identity FG/BMW/H/X
PASS; FG frontier-fixture drill (85 frontiers with unbounded helpers) rescued, gate PASS 124/124 + audit PASS; pause
drill 0 differing records vs unpaused; induced C-5F frontier drill drained with the physics query certified; the C-5F
oracle verification stalled on a quadratic verifier scan (fixed in 9a15b556/7ec2d16f). Binaries/drills in
`TMP/w1-rescue/` (`drill_fg.sh`, `drill_c5f.sh`, `verify_run.sh`, `identity.sh`, `c5f-inputs/queries-induced.json`).

To finish: rebuild at the tip; C-5F strict identity vs 4a17f9c7 (feature present, unused); oracle gate on the FG and
C-5F drills (the verifier must accept amended walks and rescue-abandoned records and must still FAIL the mutations);
a no-frontier run is byte-identical with the feature unused; lib/cli/Python suites; short note
`docs/research/fable51_w1_rescue_2026-09-28.md` (a draft with placeholders exists); rebase onto post-LC2
`fable_5_1`; merge; push. Then check whether a binary with rescue can resume LC1/LC2 checkpoints (upgrade probe);
if yes, it can be swapped in at the next pause so a future frontier is rescued in-run.

### 6.3 Epoch engine (W2) - branch `fable_5_1-v3-epoch`

Spec: `docs/research/fable51_w2_epoch_protocol_2026-09-28.md` (rev 2; read fully) and the master plan section 3/5.
Idea: inspectors run whole native inspections and resolve successors against immutable, Arc-published index
snapshots (first verified container wins; canonical min-ID mode for controls); one coordinator merges whole
inspections (P1 checks, P2 parallel per bucket, P3 serial apply preflighted, P4 layers) and is the only mutator;
walk semantics 3, checkpoint CP6. Serial work then scales with inspections and misses, not successors.

State: S2 lockstep skeleton done (`docs/research/fable51_w2_s2_2026-09-28.md` on the branch; commits 358a7da1 ...
ff1ed628, fix round f747dd59/fe7fbf47/3c7955b6/e6b107a4/5d166910; tools `tools/research/epoch_s2/`; gate binary
`TMP/epoch-s2/bin/rustred-582e7b38`; runs `TMP/epoch-s2/runs/`). [M] before the fix round (binary da364dc6):
W6/W12/W24 byte-identical on FG/BMW/H/X, four-all, four-all-p5; four-all/-p5 W96 identical to W6; C-5F W12 = W24 =
W50 identical (690 s traversal at W24: inspect batches 531 s, P2 126 s, P3 26 s; 960,538 natives vs legacy Ordered
967,621); oracle PASS on all (C-5F 1/1, 28.2M admits covered, 0 uncovered); legacy strict identity untouched;
verifier mutations FAIL as required. Finding: lockstep batch B >= 32 makes four-all flood (the same rank-13 flood on
owner 0111110010 seen in legacy Ready W96); B = 16 chosen. The fix round (reviewers: G2'-ready anchors v2, anchor
tokens, admit_initial verify, C5 panic/poison paths, parity stats_events, T8 limits, verifier bounds, exact-index
hash, error classes) was committed but its gates were NOT re-run (usage limit).

To finish W2 (each stage: implement per the protocol note, gates as listed there; owner allows lean validation):
1. Re-run the S2 gates with the fix-round binary (identity across W6/W12/W24; oracle on FG/BMW/H/X, four-all, C-5F;
   legacy identity; suites). Rebase onto post-LC2 `fable_5_1` (Symbolica ef0db494, v6 inputs).
2. S3 CP6 + stop paths (pause at epoch k byte-identical; kill -9 resume; RAM-guard stop; frontier stop;
   deterministic-error stop then resume without re-dispatch; corruption refused; per-ID attempt counter so a head
   that trips the RAM guard cannot loop).
3. S4 inspector-side resolution with the SoA kernel layers (>= 4x cheaper per candidate than today's layout is the
   target here; the legacy kernel reached ~3x only because of its historical block-boundary constraint) and
   Symbolica **thread-owned contexts** (`zero_with_new_context`/`clone_with_context_of`, as in LC2's hot path) — not
   per-CCX replicas; S4b miss path (the W0.4 projection put admission share at 1G above 25%).
4. **Owner gate** after S2/S4: interleaved, load-matched comparison vs the legacy comparator runC
   (`TMP/w1/comparator/`, recipe `TMP/w1/comparator/launch_run.sh`: 25-min gen-7 resume at W100 on socket 1, analysed
   with `tools/research/w0_baseline/m1_analyze.py` and `w2_compare.py`); runC = 3.415M obligations/h over
   [T, T+1500 s] (1.504x M1 run2, 1.611x on the matched committed range); 1.5x threshold = 5.12M obligations/h.
   Needs socket 1 -> only when no campaign runs there, or on CPUs 228-255 + socket 0 at reduced width (label it).
   Import of gen-7 into CP6 (W2.6 importer) is needed for P-IMP; a P-FRESH comparison (fresh W100 45-60 min vs LC
   early phase) is an acceptable alternative (owner: not overly strict).
5. S5 parallel merge + typed records + bulk edges + ledger6; S6 rolling schedule with a replay oracle
   (event-count-only snapshot refresh, recorded snapshot versions and merge order); W2.6 importer, deliverable and a
   streaming Rust verifier (full gen-7 re-inspection ~15 h in-memory [E]: shard it).
6. Only if the gate passes: prepare a fresh epoch campaign with the owner.

### 6.4 Native, scheduling and memory levers (after the above; each gated on controls)

- Thread-owned contexts: done in LC2 for the legacy hot path ([M] symeval-dev: K=96 CPU per native 1.03-1.04x of
  K=1 vs 3.75x before, results byte-identical; `TMP/progress/symeval-dev.report.json`). Keep the design rule: no
  per-operation refcount or shared-counter writes on shared data in the inspector hot path.
- N1 modular zero certificates (master plan 3.9): decide "coalesced group numerator not identically zero" by
  evaluating at a lattice point inside the cell with random base symbols over `Zp64`/`FiniteField<Mersenne64>`
  (`Integer::to_finite_field`, `evaluate_with_coeff_map`; pattern already used in
  `crates/rustred-core/src/foundry/completion/frame/modular/sample.rs:412-441`); nonzero residue = certified nonzero;
  zero residue or zero denominator residue = fall back to the exact path. Gate: identical successor/frontier/problem
  multisets on C-4L, C-5F and >= 1e6 sampled term visits; records identical except conditional-classification
  counters and optional-coefficient-refusal events. N2 allocation-free applied geometry
  (`C/.../applied/engine.rs` 312-320). N4 cover-first (skip numerator specialization on covered images; keep
  denominator specialization). Gate: >= 20% less inspector CPU per native with identical multisets.
- Legacy coordinator (matters while campaigns run on the legacy engine; LC1 coordinator duty ~79% at 81 min):
  helper-batch latency (preparation wait was 32% of coordinator wall in runC); `set_parallel_lean` builds a JSON pool
  snapshot per commit (2.5-5% of coordinator samples: make it lazy, prove identity); optional kernel items (hot/cold
  Meta split, confined prefetch) for the remaining reverse scan. Deliver as performance-only binary swaps (strict
  identity vs 4a17f9c7, then `--resume --upgrade-executable` at a pause).
- Priority dispatch (W3.1): FIFO alternatives were not adopted on the legacy engine (support-then-volume raised peak
  pending 1.1-2.9x); design around a bounded enqueue depth and a pending cap; judge by roots per native CPU-hour and
  pending at matched discovered domains.
- Memory tiering / NUMA (W3.3/3.5): legacy marginal RSS 0.5-1.1 KB/domain; LC1 ~0.97 KB/domain at 18M; 600 GB holds
  ~0.6-1.2G domains [E]. Restore of a 74M-domain CP5 state takes 567-600 s.
- Not adopted (measured): G1 hull widening, dense cells, D6 symmetry, D1(b) piece certification, I1b, I2 (both
  rebuilds), admission-time general union cover (net RSS 0.3-3.1x unresolved, 2.2-7.7x more union tests on the
  bottleneck), depth-first dispatch, hot-owner closure import.

### 6.5 Upgrading a running campaign

Performance-only binary swaps within the same walk semantics and checkpoint format:
`production_saved_owner_campaign.py --campaign-directory <dir> --resume --upgrade-executable <new binary>` (dry run
by default; liveness, format, semantics checks; rollback history; see `docs/shared_owner_campaign_driver.md`). The
owner pauses (Ctrl-C), you verify offline on a block-clone of the checkpoint (`cp -r`, instant on ZFS; never write
into the campaign directory), then give him the exact resume command. Gate for any swap: strict Ordered identity vs
the campaign's binary on FG/BMW/H/X, four-all, C-5F and a resume check from a clone of the campaign checkpoint.
LC1's Symbolica (953e26e2 + patch, format 5) differs from LC2's (ef0db494, format 6): an LC2-line binary cannot
resume LC1 checkpoints (input digests are format 5). Swaps into LC2 are fine once it runs.

## 7. Key measured findings (for orientation)

- v2 (legacy engine, 102adcc3) was coordinator-bound: ~9-15 of 100 CPUs busy; completions fell from 4.5M/h to
  ~0.7M/h; pending grew 0.3-2 per completion; 8/67 roots, 0 frontiers in 18.6 h [M].
- Admission cost dominates the coordinator (index block scan, find_from, retire) [M, W0.5 M1 perf]; the SoA kernel +
  kernel2 give runC 3.415M obligations/h vs M1 run2 2.27M (1.5x) [M]. Termination is not established: pending
  growth per completion at gen 7 ~0.59; LC1 early 1.29 (1 h window, 81 min) [M].
- Native in-process scaling loss 3.75x at 96 threads was cross-CCX cache-line ping-pong on Symbolica's shared
  `Arc<PolynomialContext>` reference count; fixed upstream (939c4de8) + our hot-path adoption of thread-owned contexts:
  1.03-1.04x [M]. The MRE is archived at `TMP/symbolica-main/mre-archive`; Ben's report
  `tools/research/symbolica_arc_mre/results/combined-20260928T140127Z/REPORT.md` (his files); our analysis
  `TMP/symbolica-main/UPSTREAM_REQUEST.md`.
- Work volume: census gate 0.7 PASS (70.1% D-only / 89.0% hull of gen-7 Apply CPU has a residual <= 10% in <= 8
  pieces; 61.6% fully covered by merged natives) [M]; G2' live falsifier as in 6.1; Route union cover not recommended
  (route census) [M].
- Combined four-loop control: four-all Ordered is an identity oracle (30,159 natives at W6/W24/W96); Ready four-all is
  schedule-sensitive (drained 5/5 W6, 17/18 W24, 7/13 W96; failures = rank-13 flood on owner 0111110010): judge Ready
  statistically, never as a binary drain gate [M].
- Oracles: `walk-verify-closure --require-closure --reinspect all` + `assert_oracle_pass.py` (verdict == PASS AND
  roots_independently_verified == roots_total); partial re-inspection gives INCOMPLETE (exit 9); gen-7 sample mode
  10k natives: 16 min, 33.6 GB [M].

## 8. Documents

Governing: `GOAL.md` (last sections), `FABLE_5_1_five_loop_vacuum_plan.md` section 6 (decision log),
`docs/research/fable51_next_push_master_plan_2026-09-27.md` (+ ERRATA section 11), `docs/research/fable51_v3_engine_design_2026-09-27.md`
(+ ERRATA), `docs/research/fable51_w2_epoch_protocol_2026-09-28.md`, `docs/research/fable51_w0_results_2026-09-27.md`,
`HANDOFF_opus_5_5.md` (0.1), `FABLE_5_1_CRITIQUE.md`, `TMP/progress/orchestrator_decisions.md`,
`TMP/progress/*.report.json` (every lane's final report), `docs/research/fable51_w0_*`, `fable51_w1_*`,
`fable51_lc1_*`. Symbolica: `TMP/symbolica-main/{UPSTREAM_REQUEST.md,MESSAGE_TO_BEN_zulip.md}`.

## 9. Operational handbook

### 9.1 Builds and tests
```sh
cd <worktree>; export TMPDIR=$PWD/TMP
flock -w 14400 /common/dev/rustred/TMP/locks/heavy.lock flock -w 14400 /common/dev/rustred/TMP/locks/build-N.lock \
  nice -n 5 taskset -c <CPUs outside 128-227> nix develop --command cargo build --release --locked --offline -p rustred-app
RUSTRED_TESTS_REQUIRE_LICENSE=1 ... cargo test --release --locked --offline -p rustred-app --lib   # also -p rustred --lib, --test cli_routed_campaign
nix develop --command cargo fmt --all -- --check
nice -n 5 taskset -c <>=8 CPUs> nix develop --command python -m unittest discover -s examples/python -p 'test_*.py'
# campaign binary: cargo build --profile campaign ... (fat LTO: ~50-60 min)
```
New worktrees: `git worktree add .claude/worktrees/<name> -b <branch> fable_5_1`, then
`git -C <wt> submodule update --init vendor/symbolica` (after LC2: ef0db494, no patch; before LC2: apply the old
patch from git history if you must build the old line).

### 9.2 Controls and gates
```sh
nix develop --command python TMP/fable51-controls/run_control.py --binary BIN --family fg|bmw|h|x|five-finite|four-all|four-all-p5 \
  --label LABEL --cpus <cpus> --policy ordered|ready [--workers N]      # runner lives in TMP (untracked)
nix develop --command python examples/python/compare_walk_records.py --mode strict REF/result.json NEW/result.json
BIN walk-verify-closure --command RUN/command.json --require-closure --reinspect all --threads 24 > report.json
nix develop --command python examples/python/assert_oracle_pass.py report.json
nix develop --command python examples/python/audit_owner_domain_walk.py RUN_DIR --require-closure
```
LC2-line binaries need v6 inputs: use `TMP/inputs-v6/...` (the LC2 identity script `TMP/lc2/identity.sh` shows how
the reference runs on v5 inputs and the new binary on v6 inputs). Reference outputs of 4a17f9c7:
`TMP/fable51-controls/k2-ref-fp/`. C-HOT-sub r1a12 definition: `TMP/w0/falsify/CHOTSUB.md`.

### 9.3 Campaign preparation
`examples/python/production_saved_owner_campaign.py --prepare-from <source campaign> --queries <queries.json>
--attach ... --campaign-directory campaigns/<new> --executable <bin> --workers 100 --cpus 128-227
--publication-policy ready --frontier-policy stop --checkpoint-interval-seconds 14400 --max-memory-bytes 600000000000
--ram-guard-margin-percent 5` (exact LC1 command: `docs/research/fable51_lc1_launch_2026-09-28.md` section 4; LC2
used `TMP/inputs-v6/five-loop-source/inputs` as the converted source). Validation mode = the same without `--start`.

### 9.4 Monitoring (read-only)
`nix develop --command python examples/python/campaign_monitor.py campaigns/<c>/runs/<run> --once` or read
`status.json` (`derived` block) / `events.jsonl` heartbeats. Watch: frontiers == 0; roots closed rising; pending growth
per completion trending down (must fall below 1 for termination); max scheduled rank levelling off (v2 plateaued at
18; escalate if > helper rank + 7); RSS per discovered domain settling (~0.5-1 KB); coordinator duty; host
MemAvailable > 50 GB; checkpoint saves every 4 h succeeding. Escalate to the owner (never act on the campaign
yourself) on: a frontier stop, an error, a RAM-guard stop, stalled progress, or pending growth above v2's for 3 h at
matched discovered domains.

## 10. Gotchas

- Controls runner, reference outputs and many tools live in untracked `TMP/`; do not delete `TMP/fable51-controls`,
  `TMP/w0`, `TMP/w1*`, `TMP/lc2`, `TMP/inputs-v6`, `TMP/v2-checkpoint-copy-gen{3,6,7}` (read-only clones: `cp -r` them
  before use; never write into them).
- Ready runs are not reproducible run to run; judge Ready by audit/oracle and statistics.
- Timings on this host are contaminated by foreign load (25-50 busy CPUs typical): record foreign busy CPUs and
  instructions per native; count-based metrics are reliable.
- A release build of rustred-app peaks at 35-60 GB; the fat-LTO campaign build takes ~50-60 min.
- `git worktree remove` refuses worktrees with submodules: remove the directory then `git worktree prune`.
- The heap-pow patch is obsolete after LC2; do not re-apply it to ef0db494.
