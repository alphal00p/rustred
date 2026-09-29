# Codex progress: five-loop optimization and controlled deployment

Authoritative plan: [CODEX_PROGRESS_PLAN.md](CODEX_PROGRESS_PLAN.md).
Root orchestrator owns this log; agents report evidence for integration here.
`[M]` denotes observed/measured evidence; `[E]` denotes interpretation or estimate.

## Current state — 2026-09-29 00:30 UTC

- [M] Main checkout: `fable_5_1`, starting tip `2255bc06`, synchronized with
  `origin/fable_5_1` at the preceding audit. Symbolica gitlink `ef0db494`,
  patch-free upstream dev; native artifact format 6.
- [M] LC2 is the live campaign: `campaigns/five-loop-qcd-feynman-d9d10-lc2`,
  run `20260928T234230.071605Z`; frozen binary `rustred-lc2-fd9b9ac9`.
  At the preceding read-only snapshot (2026-09-28 23:55:33 UTC): 5,379,517
  discovered, 1,247,355 native completions, 2,125,114 pending, 2/67 initial
  obligations recursively closed, zero frontiers, 9.96 GB RSS. This is a
  historical early snapshot, not a current rate or completion forecast.
- LC1 is paused and recoverable with its own frozen format-5 binary. Do not
  alter either campaign. LC2 owns CPUs 128–227 and up to 600 GB; development
  must preserve host headroom and existing build/heavy-job locks.
- [M] Frozen input census: 116 required physical/convenience queries plus
  67 helpers; descendants are not clipped. Existing helpers: 54 finite-A,
  13 unbounded-A. Physics assumptions remain the approved Feynman-gauge QCD
  auxiliary-mass scope, not an arbitrary-theory coverage theorem.
- [M] Existing unrelated work preserved: `crates/rustred-feynkit/src/lib.rs`,
  `EPSILON.md`, `Janet_Ore_reference_code_usefulness_study.md`,
  `tools/research/symbolica_arc_mre/`, `vendor/OLD_vendored_symbolica/`.

## Baseline validation and measurements (prior receipts, not rerun here)

- [M] LC2: core 2844 passed / 32 ignored; app 822 / 12 ignored; CLI 6;
  Python 260 OK. Seven strict Ordered identity and independent full-reinspection
  controls passed. Sources: `docs/research/fable51_lc2_patchfree_2026-09-29.md`,
  `TMP/lc2/gates/gates.tsv`, `TMP/lc2/gates/oracle.tsv`.
- [M] Thread-owned Symbolica contexts: K96/K1 inspection CPU ratio 1.034–1.044
  in the recorded harness; this is not whole-walk scaling. Source:
  `TMP/progress/symeval-dev.report.json`.
- [M] G2 production-branch pilot: C-5F scheduled domains 0.756–0.761x,
  CPU 0.514–0.548x; hot-sub control scheduled 0.721x, CPU 0.271x. Two repeats;
  not a complete production campaign. Evidence:
  `.claude/worktrees/fable51-g2f/docs/research/fable51_w1_g2prod_2026-09-28.md`.
- [M] Epoch fix-round gates did run despite stale handoff text:
  `TMP/epoch-s2/runs/fixround-r2.log`; seven oracle passes and cross-width
  identities. C-5F W24 traversal 565.63 s. S2 is not the completed performance
  architecture and has not met the deployment speed gate.

## Work ownership and backlog

| Work | Status | Responsible lane | Next step / reopening condition |
|---|---|---|---|
| Plan, goal, progress bootstrap | delivered | root | Goal active; independently audited documentation milestone |
| G2′ integration | active | joint_support_pruning | Bring existing branch onto LC2; reconcile final-tip gates |
| Rescue and explicit query roles | active | bounded_helpers_bmw | Preserve 116 required queries; remove substring authority |
| Independent math/code audit | active | checkpoint_final_audit | Review both lanes and combined quarantine/replay invariants |
| Combined G2′ + rescue | pending | root + independent auditor | Integrate after separate slices; paired pilot and restart decision |
| Coordinator latency / telemetry | pending | profiling lane | Select narrow measured bottleneck after first integration |
| Epoch S3–S6 | pending | implementation + independent audit | Durable checkpoints, inspector lookup, merge, rolling replay |
| N2 allocation-free geometry | pending | profiling/implementation | Register opportunity and falsifier |
| N1 modular witnesses | pending | research/implementation | Audit Symbolica and exact decision semantics |
| N4 coverage-first work | pending | research/implementation | Preserve denominator/guard/frontier obligations |
| Scheduling / ordering | pending | research/measurement | Compare work volume and censored Ready outcomes |
| Memory / checkpoint / NUMA | pending | profiling lane | Measure process-local opportunity without host-wide changes |
| New algorithms / literature | pending | rotating research lane | Falsifiable hypothesis based on observed bottleneck |
| Earlier rejected levers | deferred | root | New evidence required; retain prior negative results |

## Decisions in force

1. The approved plan supersedes stale stop/launch directives; production is
   still owner-controlled. Never start/stop/signal/resume it from an agent.
2. Leave pending-growth calculation, name, and rendering unchanged.
3. Resume compatibility is a convenience, not a veto or a migration project.
   Recommend fresh runs when necessary; preserve old campaigns for rollback.
4. A passing internal microbenchmark is not sufficient deployment evidence.
   Use matched controls, two pairs for a switch recommendation, and the
   approved 20% benefit / epoch 1.5x gates.
5. Pilots <=30 minutes including restore, preparation and safe shutdown.
   Separate compilation; report unfinished runs as censored.
6. No extra terminal minimization, numerical-master or Vakint work in this stage.

## Event log

### 2026-09-29 00:30 UTC — execution bootstrap

- User approved the complete plan. Recorded it verbatim and added the
  authoritative directive to `GOAL.md` before implementation edits.
- Checked the working tree and lane worktrees; no unrelated changes touched.
- Initial implementation lanes start from existing G2 `80e80b5a`, rescue
  `7ec2d16f`, and main LC2 `2255bc06`. Epoch `5d166910` remains deferred until
  the near-ready work is integrated and measured.
- [M] Root tool-managed goal created at 2026-09-29 00:32 UTC, status active.
  Subagent threads do not share the root's goal tool state.
- Dispatched separate G2′ and rescue implementation agents on isolated
  `codex/*-lc2-integration` branches and an independent auditor. Builds use
  CPUs 0–15 / 16–31 respectively, capped at eight workers, with heavy/build
  locks. No production CPUs or campaign writes authorized.
- Independent bootstrap audit found no substantive scope conflict.
- Read-only LC2 update at 00:33 UTC: 13,148,077 discovered, 3,730,934 native
  completions, 5,469,700 pending, 6/67 closed initial obligations, zero
  frontiers, approximately 14.4 GB RSS. No completion estimate inferred.
- Next: commit/push this documentation milestone; reconcile source integration
  and test receipts before any performance pilot.
