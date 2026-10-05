# Second five-loop campaign: complementary banana rules

Status: release controls passed; separate inputs, executable and steering are
frozen. The user launched this campaign on October4; the run was observed
active on October5. The commands below are the launch/resume reference, not
an instruction to start a duplicate process. Both production campaigns and
their checkpoints remain user-controlled and untouched by development work.

## Why this variant, and what is not established

The research has found useful local identities, but **not a demonstrated
material speedup for the complete five-loop campaign**. The best tested rule
pool keeps the current new37 shortcut and both frontier repairs, and adds two
complementary source-derived rules to its existing banana subtopology owner.
These are ordinary exact rules consumed by the generic engine, not a
topology-specific solver implementation.

| Completed comparison | Result | Interpretation |
| --- | --- | --- |
| Focused banana controls | Approximately20–26% fewer local domains in the relevant comparisons | Repeated local benefit; not a prediction for every input |
| Broader260-input control |69,772 →69,682 operational states;0.129% less | Both cold-verified; no material whole-cohort speedup |
| Depth-four composition, original4L90 | Shared boundary235 →235 | Park fixed composition |
| Depth-four composition, original5L29 | Shared boundary5,352 →5,263;1.663% less | Exact same-cut cancellation; no avoided-work/closure claim |

The two banana rules are complementary: each retains the original rule stack
as a fallback outside its proved case. Their83 saved terminal records are
unchanged. A hard owner31 cohort still did not finish within its registered
pilot envelope. The unsuccessful source-bank expansion and weighted-cut
experiments remain documented rather than being enabled in production.

The user requests this second experiment after research wind-down even though
the original20% whole-campaign improvement target has not been met. Running
it alongside the old pool is therefore an experiment with a validated exact
alternative, **not a recommendation to discard the existing progress**. We
cannot currently give a reliable five-loop completion ETA or guarantee that
either symbolic worklist terminates in a practical time.

## Preserved scope and changed inputs

- Preserve all67 saved owners,8,246 routes,116 required queries and67 helpers.
- Preserve the existing new37 rule and the11-rule second-frontier repair.
- Replace only owner0's483-rule payload by its485-rule payload, and rebind its
  existing two-rule repair to that exact owner context.
- Keep existing Epoch, G2 union reuse, helper order and correlated input caps.
- Leave finite replay-summary and weighted-cut research modes off.
- Start a **fresh traversal**. The old checkpoint binds different owner bytes;
  it is neither rewritten nor imported. New saved checkpoints belong only to
  the new campaign.

Candidate owner SHA256:
`ebd0def4ae1c77cab74c2f377dc706ad5a9c03af4d0155781b022f3ed856b397`.
Rebound two-rule overlay SHA256:
`f330f7082f102a1dfd056be4904ca19ac90300ec0199e52620da73d1e4be38eb`.
Unchanged second-frontier overlay SHA256:
`7e1f93615282130555f29a885d1bbfe7b5b0c08074d7c2d1e542378b4457a86a`.

## Resource isolation

The intended allocation is32 workers on CPUs64–95. The existing campaign uses
CPUs96–127. A reservation is not a promise of32 busy cores: dependency and
ordered-publication bottlenecks remain. The new campaign requests a600GB RAM
ceiling,5% save-and-stop margin,150GB host reserve and hourly checkpoints.
It has no arbitrary campaign runtime or domain-count cap.

The supervisor re-evaluates available memory at launch and can admit a lower
effective ceiling. Two600GB ceilings are not a guarantee that1.2TB can be used
simultaneously with the OS and other workloads. Read the printed admission
and retain the host-reserve guard. Neither campaign is stopped to make room.

## PR consolidation

[PR2](https://github.com/alphal00p/rustred/pull/2) adds the HepKit bridge's
tensor invariants, cuts, preferred-master basis changes and optional reduction
checks/master counting. Its new checks do not become production campaign
closure authority. Numerical master counting remains probabilistic; exact
reduction replay does not prove that a terminal basis is minimal. Rust bridge
signatures change; Python defaults leave the new features opt-in.

Independent review identified a public certificate input-validation gap:
symbolic flags could be ignored and malformed key lengths could reach numeric
ordering. Integration fixes this at the certificate boundary and adds negative
tests. The PR does not replace the routed campaign engine. Unrelated local
collaborator edits are preserved separately and are not part of the release.

## Final validation and launch

The optimized CLI is built from reviewed merge `3d0b08fb483fc3b02a094998433862e6a8b2479e`,
with SHA256 `05386e852cff135cae040c3c53861ed6b15fa6d84081fb36d98b6b82dd26e6c1`.
The later arity and native-notebook interfaces are separate library changes,
not silently attributed to this frozen campaign executable. Thirty-two focused
native PR-integration tests and the FeynKit Rust check passed before freezing.

| Release control | Result | Observed inclusive time |
| --- | --- | --- |
| Whole four-family/58-query campaign, independent cold reinspection | PASS;26,025 saved records,17,957 native inspections,872,486 events,495,898 edges;58 queries checked,zero frontiers |17.295s |
| Finite five-loop260-input control, independent cold reinspection | PASS;69,682 operational states,69,422 physical keys,33,561 applications,23,338 transports,66 declared terminals,12,457 zeros |183.027s |

All recorded work counts match their archived references exactly. Both controls
used the same frozen executable with unchanged inputs and bounds. Processes
drained cleanly. The five-loop canary enables the same finite-replay mode as its
reference; that research mode is **not enabled in production**. It validates
those260 concrete seeds, not the entire symbolic campaign: its outer symbolic
walk still reports the historical `incomplete` status/exit4; the separately
exhausted finite trace and independent cold check are what passed. Timings exclude
compilation, include preparation and cold verification, and are release checks
under host contention—not matched performance improvements. Receipts are under
`TMP/rule-optimizer-20261003/profiles/release-v4-controls-20261004/`.

For an initial launch in a **new** tab, for example `codex_banana485` in the
`rustred` Zellij session, the command is below. This campaign is already
running as of October5: do not invoke it again while that run is active.
Leave the other campaign's tab alone.

```bash
cd /common/dev/rustred
nix develop /common/dev/rustred --command python -B \
  /common/dev/rustred/campaigns/five-loop-a1-banana485-20261004/steering/production_saved_owner_campaign.py \
  --campaign-directory /common/dev/rustred/campaigns/five-loop-a1-banana485-20261004 \
  --start
```

No compilation is needed; it selects the frozen binary and32-worker CPU64–95
policy. Use the repository's Git-backed flake as shown, **not** `path:`, which
would import the large ignored workspace into the Nix store. A first launch
must not include `--resume`. After a clean interruption and saved checkpoint,
use the same command with `--resume --start`. Ctrl+C requests an orderly save;
allow it to finish and report the checkpoint path before closing the tab.

To inspect preparation without launching, omit `--start` and add `--json`.
The staged manifest SHA256 is
`90245dd3babf801915f786b273988d351e3d843487c25ee3671842853d0c3dc6`;
queries are byte-identical to the existing campaign, SHA256
`42a0c62771b6e7c53cc937d46ad9505e33c282db31a8d6f64846ecbca749ef64`.
The frozen steering and its module hashes live inside the new campaign. Changes
to the root build or Python sources do not replace them. Scope is still116
required queries and67 auxiliary helpers. The old checkpoint is not compatible
with changed owner payloads; this experiment recomputes traversal from scratch.

Rollback is simply to stop this new campaign gracefully and leave the old one
running. Never copy its checkpoints over the old campaign's files. The600GB RAM
request remains subject to live host admission and the150GB reserve described
above. There is no reliable closure ETA and no assertion of a substantial
whole-campaign speedup.

Research details: [conceptual search](conceptual_rule_search_2026-10-04.md),
[banana nomination and controls](banana_moment_nomination_2026-10-04.md), and
[continuous progress](../../CODEX_PROGRESS.md).
