# FABLE_HANDOFF — state of the `fable_5_1` effort (2026-09-26, ~12:35 UTC)

Governing goal: [`FABLE_5_1_five_loop_vacuum_plan.md`](FABLE_5_1_five_loop_vacuum_plan.md)
(sections 2 and 6 hold the user decisions and the decision log). Evidence
discipline is unchanged: numbers below cite a run directory or receipt; no
ETA and no closure is claimed anywhere.

## 1. What happened, in order

1. Read-only audit of the live campaign, the code and the literature (53-agent
   workflow): [`docs/research/five_loop_completion_levers_2026-09-26.md`](docs/research/five_loop_completion_levers_2026-09-26.md).
   Findings: the Ordered coordinator serializes heavy Apply heads (one owner
   holds the head ~62% of wall; ~1.8 of 10 cores busy); the real roots are the
   R<=15 orthant helpers and 99% of pending Apply work sits above them; RSS is
   ~4.5 KB per domain, 60% of it write-only report records; saves are
   single-threaded JSON rewrites that grow ~30 s per generation.
2. User decisions (all recorded in the plan): Feynman gauge, D in {9,10},
   factorized owners kept with nested bounds, all 67 roots kept, relaxed
   semantics-version checkpoint binding, implement everything before the
   "real" launch with pilots along the way, CPUs 128-177 (more if utilization
   saturates), 700 GB guard, user stops the old campaign themselves. Then:
   full permissions, no more approval questions.
3. Branch `fable_5_1` created from `main` (100e990f). Baseline tests: 687/0/4
   Rust release lib tests, 110 Python tests.
4. Package A (planner) delivered and committed: generic skeleton enumeration
   and 2-isomorphism classification, per-owner physics roots, rank-reduced
   orthant helpers, independent checker, match summarizer, 44 tests, fixture.
5. Packages D and E delivered and committed: production steering v2 (Ready
   default, frozen policy/lookahead/worker split/interval, CPU ranges, worker
   cap 256, `--prepare-from --queries --attach`), supervisor derived metrics,
   monitor lines, control-matrix harness, walk audit, record comparison
   (Python suite now 185 passing).
6. Validation ladder for the new inputs: planner + checker pass; matching-only
   diagnostic v1 found 49 unresolved guard pieces on 7 helper anchors at
   unbounded positive power (the same 7 owners as the September 24 recipe);
   v2 with those 7 helpers bounded at their owner's largest root A_max: all
   183 queries locally applicable, zero unresolved / gaps / invalid.
   Evidence: `TMP/qcd-feynman-d9d10-input.dcgP73/{plan-v1,match-v1,plan-v2,match-v2,RESULTS.md}`.
   Committed copies of the final inputs: `examples/input/five_loop_qcd_feynman_d9d10/`.
7. Single-owner pilot (hottest owner `011101110111000`, physics box, frozen
   binary, Ordered W6 on CPUs 192-197, then Ready) is running:
   `TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/`. At 111 min: 3.0M
   inspections, pending 595K and shrinking (growth per completion -0.03), max
   scheduled rank 8 (old envelope: 21), 40 GB RSS, zero frontiers.
8. Wave-1 Rust packages run as background implementers in isolated worktrees:
   scheduler/admission on branch `fable_5_1-sched` (9 commits, tree clean at
   83a66462, doing final control measurements), CP5 checkpoint core on branch
   `fable_5_1-ckpt` (5+ commits, asked to commit WIP frequently). Both branches
   are pushed to origin. An adversarial review workflow of `fable_5_1-sched`
   is running (4 lenses, 2 verifiers per finding).
9. Interim campaign launched at the user's request (credits running out)
   with the validated inputs, the existing Ready policy and the current
   frozen binary (see section 2).

## 2. The running interim campaign (this is what to watch)

- Directory `campaigns/five-loop-qcd-feynman-d9d10` (ignored by git except
  nothing; inputs are byte-identical to `examples/input/five_loop_qcd_feynman_d9d10/queries.json`,
  sha256 `0f7f0a043de8a896725d3bd1fcb42a7948d2aeadc68cbd78bf2fa549d80d97b8`).
- Run `runs/20260926T122852.894005Z`; native PID 321972, supervisor PID
  321704 (boot id `02fd9278-...`); launched 2026-09-26 12:28:52 UTC in a new
  command pane of Zellij tab `fable_5_1` (session `rustred`,
  `XDG_RUNTIME_DIR=/run/user/1125`), which shows the live monitor.
- Frozen policy (`bin/steering.json`, schema v2): executable
  `rustred-32fdec09...` (the live campaign's binary), 50 workers on CPUs
  128-177 (25 inspectors / 24 helpers / 1 coordinator), `--publication-policy
  ready`, lookahead 256, `--route-domain-overcover --reuse-initial-d-bands
  --bounded-refinement-axes finite-axes --max-guard-univariate-degree 64
  --unbounded-work`, 700 GB requested RAM ceiling (save-and-stop at 665 GB
  minus host headroom), checkpoints every 14,400 s (CP4 format of the old
  binary; a new CP5 binary cannot resume it).
- First minutes: all 67 initial roots published by 2.3 min, 1 root already
  closed, 2.4 busy cores while the queue fills, zero frontiers.
- Read-only monitor:
  `nix develop --command python examples/python/campaign_monitor.py campaigns/five-loop-qcd-feynman-d9d10/runs/20260926T122852.894005Z --once`
  (add `--json` for machine output; `status.json` has a `derived` block with
  completions/h, stall shares, pending growth per completion, RSS per domain).
- Pause: Ctrl-C in its pane (cooperative saved pause, exit 4). Resume:
  `nix develop --command python examples/python/production_saved_owner_campaign.py --campaign-directory campaigns/five-loop-qcd-feynman-d9d10 --resume --start`
  (RAM options may be overridden at resume; nothing else).
- Thresholds to call "better odds than the old run" (plan section 3.F): roots
  closed strictly increasing, pending per completion falling, rank cap flat,
  RSS trajectory to the stop point > 7 days. Escalate (Ctrl-C, keep the
  checkpoint) if pending growth per completion exceeds the old run's for 3 h
  or the max scheduled rank climbs past helper rank + 7.
- Housekeeping: a launch command was first typed into the tab's fish pane
  and did not execute; if that pane still shows the pending line, clear it
  (a second `--start` would be refused anyway because the checkpoint
  directory is not empty).
- The old campaign (`five-loop-dependency-closure`, CPUs 0-9) is untouched:
  20.1 h, 11.8M completed, 20.9M pending, 186 GB RSS, 75 GB checkpoints in
  ~9 min. The host cannot hold both at their ceilings; the user stops the old
  one.

## 3. Reproducing the campaign inputs from scratch

```sh
cd /common/dev/rustred && export TMPDIR="$PWD/TMP" TMP="$PWD/TMP" TEMP="$PWD/TMP"
# 1. plan (fixture makes it seconds; drop --classification to re-enumerate, ~90 s)
nix develop --command python examples/python/plan_renormalization_entry_queries.py \
  --loops 5 --manifest campaigns/five-loop-dependency-closure/inputs/selection.json \
  --momenta examples/input/tide_five_loop_manifest.json \
  --parent-witnesses examples/input/tide_five_loop_parent_vertices.json \
  --gauge feynman --difference-set 9,10 \
  --classification examples/python/fixtures/tide_five_loop_skeleton_classification.json \
  --helper-positive-power-owners-from examples/input/five_loop_qcd_feynman_d9d10/matching-summary-v1-unbounded-helpers.json \
  --executable target/release/rustred --output-directory TMP/qcd-replan
# 2. check
nix develop --command python examples/python/check_renormalization_entry_queries.py \
  --queries TMP/qcd-replan/queries.json --receipt TMP/qcd-replan/entry-plan-receipt.json
# 3. matching-only diagnostic (~150 s, ~8 GB): match_shared_owner_domains.py without --follow-successors,
#    exact argv in TMP/qcd-feynman-d9d10-input.dcgP73/match-v2/command.json; then
nix develop --command python examples/python/summarize_owner_domain_match.py --result RESULT.json --output matching-summary.json
# 4. prepare + start (do not pin the preparing shell with taskset: it checks affinity)
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --prepare-from campaigns/five-loop-dependency-closure --queries TMP/qcd-replan/queries.json \
  --attach TMP/qcd-replan/entry-plan-receipt.json --attach TMP/qcd-replan/skeleton-classification.json \
  --campaign-directory campaigns/<new-name> --executable <binary> --workers 50 --cpus 128-177 \
  --publication-policy ready --transfer-unreserved-lookahead 256 --checkpoint-interval-seconds 14400 \
  --max-memory-bytes 700000000000 --ram-guard-margin-percent 5
nix develop --command python examples/python/production_saved_owner_campaign.py --campaign-directory campaigns/<new-name> --start
```

Known quirks: the launcher stages inputs before checking CPU affinity (a
failed affinity check leaves a half-prepared directory without
`bin/steering.json`; delete and redo); the checker looks for
`entry-plans/*.json` next to the receipt (run it on the planner output
directory, or copy `entry-plans/` beside a staged receipt).

## 4. Branches, worktrees, what remains

- `fable_5_1` (main line, pushed): plan, audit, packages A, D, E, inputs,
  this handoff. Suites: `nix develop --command cargo test --release --locked
  --offline -p rustred-app --lib` (687/0/4 at branch base) and `nix develop
  --command python -m unittest discover -s examples/python -p 'test_*.py'` (185).
- `fable_5_1-sched` (pushed; worktree `.claude/worktrees/agent-ab06981cd80007c75`):
  bit-signature containment pre-filter, helper-prepared reverse retirement,
  refresh duty 1%, worker cap 256 + Ready helper cap 32, Ready slot recycling
  mid-commit, coordinator/slot telemetry, tests. Author was measuring FG/BMW
  controls (`TMP/fable51-controls/sched-wave1*`) when this handoff was written.
  Merge gate: release suite green, `cargo fmt --all -- --check`, FG/BMW/H/X
  `result.json` counters identical to `TMP/fable51-controls/baseline-32fdec/`
  (strict mode of `compare_walk_records.py`), review findings fixed. The
  review workflow's results are in the session transcript directory
  `.../subagents/workflows/wf_1796530b-36e/journal.jsonl` (may be partial).
- `fable_5_1-ckpt` (pushed; worktree `.claude/worktrees/agent-ade877816b107b1cf`):
  CP5 sectioned checkpoint store with today's in-memory types, semantics
  version binding, change-stamp saves. Wave 2 still to do (plan section
  3.C): records sidecar, u32 CSR edges, compact domains/summaries, scale
  tests. Merge after review + FG stop/resume equivalence.
- Wave 2 (plan section 4): B3 Ready multi-prefix resume gate, B5 pipelining
  and B6 inner parallelism (gated on coordinator duty), C1-C3, C7; profiling
  matrix old vs new binary (`walk_control_matrix.py`; baselines in
  `TMP/fable51-controls/RESULTS.md`); multi-owner Ready pilot at W24; then a
  second campaign with the new binary (the interim campaign's CP4 checkpoint
  cannot be migrated; it can keep running on CPUs 128-177 while a new one is
  prepared on other cores, RAM permitting).
- To resume the effort in a new session: read the plan's decision log, `git
  fetch`, inspect the two branch tips and their worktrees, rerun the two test
  suites, then continue with the merge gates above.

## 5. Baselines and controls collected

`TMP/fable51-controls/RESULTS.md`: frozen binary, W6 on CPUs 192-197
(Ordered / Ready whole-command seconds): FG 18.2 / 19.6, BMW 40.2 / 45.3,
H 17.5 / 17.5, X 41.0 / 38.2; five-loop finite control (1,324 tuples) at W50
on 192-241: Ordered 425.6 s (traversal 321.5 s, 967,621 inspections), Ready
339.4 s (236.2 s, 981,183). Runner: `TMP/fable51-controls/run_control.py`.

## 6. Risks

- Termination of the symbolic walk is not established even for the smaller
  class; the pilot's shrinking queue is the first positive signal, not a proof.
- The interim campaign runs the old binary: ~4.5 KB RSS per discovered
  domain and JSON checkpoints; the 665 GB stop point is real. A restore of a
  large CP4 checkpoint has never been executed.
- Ready has never been drained at five loops; the multi-prefix resume gate
  is open (plan B3).
- Shared host: another user's job floats over all cores; `zpool status`
  reports 2 data errors on the single NVMe pool (file list needs root).

## 7. Remaining work, with the exact plans to follow (for the follow-up model)

Everything below has a verbatim file-level design already written; do not
re-plan, execute. Read in this order: `FABLE_5_1_five_loop_vacuum_plan.md`
(scope, gates, decision log), then the design of the package you touch, then
its review report (findings are verified, with file:line at the branch tips of
2026-09-26; lines shift after the merge, titles and symbols do not).

| Package | Design (verbatim) | Review (verified findings) | Status at handoff |
|---|---|---|---|
| B scheduler/admission | `docs/research/fable51_design_scheduler_admission_2026-09-26.md` | `docs/research/fable51_review_scheduler_branch_2026-09-26.md` | items 1(d-f), 2, 4, 5, 7, 8 merged; review must-fix #1, #4, #5, #7 applied |
| C checkpoint/memory | `docs/research/fable51_design_checkpoint_memory_2026-09-26.md` | `docs/research/fable51_review_checkpoint_branch_2026-09-26.md` | items 4, 5, 6 merged (CP5 store, semantics binding, change-stamp saves); review must-fix #1-#6 applied |
| A/D/E/F inputs, launcher, harness, pilots, launch | `docs/research/fable51_design_inputs_pilots_launch_2026-09-26.md` | (no separate review; Python suite 185 green) | planner/checker/launcher/monitor/harness merged; pilots partly run |

### 7.1 Package C, wave 2 (largest remaining value: the RAM wall)
Follow design sections 1, 2, 3, 7 in that order; each is self-contained.
1. Records sidecar (design §1): `RecordSink::{Memory, Sidecar}` in
   `walking/execution/records.rs`; `State.records` becomes `RefCell<RecordSink>`
   plus a `records_accepted_events` counter; records are streamed to
   `records-<S>.jsonl` at commit (`execution.rs` commit_result and
   `execution/delegation.rs` delegated records) and sealed per generation; the
   CP5 store already writes `records-<S>.jsonl` segments (wave 1 kept the
   in-RAM `Vec<Value>` as the source), so the change is to stop retaining them
   and to replace `validate_closure_records` by the ledger/closure cross-check
   of §1.4; `OwnerDomainWalkResult::write_json` streams `domains` into
   `result.json` (CLI writer in `cli/owner_match.rs`); tests listed in §1.5,
   including the test-facing `snapshot()` accessor to migrate the ~10 tests
   that index `state.records`. Gate: RSS growth per committed domain drops
   from ~6 KB to ~0 on the synthetic state (§7.3) and the FG control's
   `domains` array is unchanged.
2. Compact in-RAM state (§2): `CompactDomain<N>` (u16 coordinates, ~96 B) and
   `CompactSummary<N>` (~176 B) with retired-slot reclamation and a
   `HashMap<u128, u32>` exact map; keep `Domain<N>` as the transport type;
   differential tests against `DomainPowerSummary::contains` on random boxes;
   the only new refusal is a coordinate above 65534. Gate: <= 0.8 KB per
   discovered domain on the synthetic state.
3. CSR edges (§3): u32 CSR-by-target plus a bounded append log folded after
   each save; node flags as bytes; restore rebuilds heads from the edge
   segments; gate: refresh time <= 1/3 of the linked-list time and <= 5 B/edge.
4. Scale tests (§7): timed restore of a copied production CP5 checkpoint on
   CPUs 192-241, FG interrupted-vs-uninterrupted equivalence (Ordered and
   Ready), synthetic 20M-domain save/restore benchmark.
5. Review follow-ups not yet applied (report #7-#14): meta-section closure
   `unavailable` bypass, `previous.json` ordering, `effective_interval`
   reporting, manifest segment-list compaction (currently only the byte cap
   was raised), monitor rendering of the new events.

### 7.2 Package B, wave 2
1. Ready multi-prefix resume-to-exhaustion gate (design §1(a)): env seam
   `RUSTRED_WALK_DIAGNOSTIC_PAUSE=ready-multi-prefix` in the `maybe_save`
   closure of `walking/mod.rs` that force-saves and cancels when
   `ready_accepted_source_prefixes >= 2 && published_count > queue.next`; the
   in-process W=4 test with scripted inspectors (`ready_native_multi_tests.rs`)
   and the fresh-process pause/resume harness on the four-loop X control and
   the restricted five-loop pilot; pass criteria in §1(a).
2. Review follow-ups (report #2, #3, #6, #8-#13): disjoint duty buckets,
   deferred Delegates in the mid-commit service step (the branch currently
   `break`s at the first unpublished Delegate, which caps the benefit of slot
   recycling in retirement-heavy regimes), third snapshot tier for per-slot
   arrays, ungated `reclaim_all_finished` test, frozen key-set test.
3. Prep/commit pipelining (§4) only if, after the above, prep + commit exceed
   15% of coordinator wall on the five-loop control; inner parallelism (§6)
   off by default, only if pilots show coordinator duty < 60% with idle
   inspectors. Both are fully specified in the design.

### 7.3 Pilots, profiling and the "real" launch
1. Profiling matrix (design 3 §3): `examples/python/walk_control_matrix.py`
   over FG/BMW/H/X (W6, CPUs 192-197, Ordered and Ready) and the 1,324-tuple
   five-loop control (W50, CPUs 192-241), old binary `32fdec09...` vs new;
   `audit_owner_domain_walk.py` on every case; `compare_walk_records.py
   --mode strict` for Ordered pairs. Baselines: `TMP/fable51-controls/RESULTS.md`.
2. Multi-owner Ready pilot at W24 on CPUs 200-223 (design 3 §2(d)) with the
   matched-metric decision thresholds of plan section 3.F.
3. Restore-at-scale before trusting any multi-day run (C §7.1).
4. If a third campaign is launched: prepare with the current binary and
   `examples/input/five_loop_qcd_feynman_d9d10/queries.json` (v3, frontier-free
   in the matching diagnostic), Ready, W50 on free cores, 700 GB guard, 4 h
   interval, exactly as in section 3 of this file, and launch it in the
   `fable_5_1` tab with `XDG_RUNTIME_DIR=/run/user/1125 zellij --session rustred
   action go-to-tab-name fable_5_1 && zellij --session rustred action
   focus-pane-id terminal_1 && zellij --session rustred action new-pane --cwd
   /common/dev/rustred --name <campaign> -- env TMPDIR=... nix develop
   --command python examples/python/production_saved_owner_campaign.py
   --campaign-directory campaigns/<campaign> --start` (verify with
   `dump-layout` that the pane landed in `fable_5_1`; a headless
   `go-to-tab-name` alone once failed to move focus).

### 7.4 Physics follow-ups (user decisions needed)
- Linear-xi gauge terms (`--gauge linear-xi --gauge-parameter-powers 1`)
  would add one dot and one scalar product per owner; not requested.
- The eight non-entry owners keep convenience roots at the widest connected
  box; if the community tool needs more than that (e.g. the six-line banana
  with higher powers), widen those roots explicitly and re-run the matching
  diagnostic.
- Frontiers: any campaign must be checked for `frontiers == 0` in its first
  minutes (`status.json` -> `progress.work.frontiers`); a nonzero count with
  `local_dispatch_frontier` records means a helper with unbounded positive
  power on an owner with >= 8 active lines (see section 2 of
  `TMP/qcd-feynman-d9d10-input.dcgP73/RESULTS.md`).
