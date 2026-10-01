# Restart with the October 1 source-derived frontier repair

The optimized repair build is frozen from commit `4753995f`. It reuses the
existing 67 owner payloads and 8,246 routes, adds two independently replayed
rules, and preserves all 116 required queries, 67 auxiliary queries and 939
saved terminals. It does **not** regenerate owners or claim five-loop closure.

The new rules cover the exceptional ray containing the stopped rank-eleven
point. Cold loading checks original-source identities, guards and strict
descent. Neighboring concrete and symbolic campaign-path checks found no new
frontier before their diagnostic resource allowances. Future gaps remain
possible; the production policy still saves and stops at a frontier.

## Start the prepared fresh walk

Use the `codex_astra` tab of the `rustred` Zellij session. The preparation is
metadata-only; **the user runs the following command to start the calculation**:

```bash
cd /common/dev/rustred/TMP/releases/20261001-frontier-repair
nix develop "path:$PWD" --command python -B examples/python/production_saved_owner_campaign.py \
  --campaign-directory /common/dev/rustred/campaigns/five-loop-a1-epoch-frontier-repaired-20261001 \
  --start
```

The explicit `path:` is required: this frozen release is intentionally untracked
inside the parent Git repository. Without it, Nix tries to resolve the release's
flake through that repository and rejects it. Do not add `TMP/` to Git.

Keep the current Symbolica license in the shell environment. No rebuild is
needed. Frozen settings are 32 physical cores (64–95), 31 inspectors, a 600 GB
RAM ceiling with a 5% save/stop margin, 150 GB host-memory reserve and hourly
checkpoints. The other Epoch, G2 and publication settings are unchanged.

The added rules change the immutable inputs bound to a checkpoint, so the old
walk cannot resume with this repair. This is a **fresh traversal of saved
rules**, not a fresh IBP-generation campaign. Both old campaign directories and
their checkpoints remain untouched.

## Resume the new walk after an interruption

Use the same frozen release and directory, adding `--resume`:

```bash
cd /common/dev/rustred/TMP/releases/20261001-frontier-repair
nix develop "path:$PWD" --command python -B examples/python/production_saved_owner_campaign.py \
  --campaign-directory /common/dev/rustred/campaigns/five-loop-a1-epoch-frontier-repaired-20261001 \
  --resume --start
```

Rollback means returning to the original campaign with its original frozen
binary, inputs and checkpoint—not substituting an old binary into the repaired
campaign. Moving development to `main` does not change this frozen release.

## Updated dashboard without restarting the calculation

The frozen supervisor retains its originally imported presentation code. To
see the updated discoveries-per-recursive-closure ratio now, run the current
read-only monitor in a **separate terminal or Zellij pane**:

```bash
cd /common/dev/rustred
nix develop --command python -B examples/python/campaign_monitor.py \
  campaigns/five-loop-a1-epoch-frontier-repaired-20261001/runs/20261001T091314.583624Z
```

This reads the live status and process identities only; it does not launch,
stop, resume, signal or modify the calculation. No Rust compilation is needed.
Ctrl-C in this **separate monitor** exits only that viewer. Do not press Ctrl-C
in the existing calculation's terminal merely to refresh its display. After
a future user-operated resume, use the new run directory instead.

`Discovery/closure` is `D/C`, discoveries divided by recursively closed domains
observed over the same trailing hour, not `(D-C)/(D+C)`. Both this indicator and
the unchanged pending-per-completion metric are red above2, yellow above1
through2, and green at1 or below. Positive discoveries with zero observed
closures show red `∞`;0/0 or unavailable data show neutral `unknown`.
Closure scans are batched, so check the separate snapshot age/staleness line:
`∞` during a stale scan does not prove that no additional domains have closed.
The new monitor derives the ratio from the old supervisor's raw paired counts;
it does not reinterpret the old normalized-balance value.

## Validation and exact release identities

- Core public regression tests: 4/4 pass.
- App public integration tests: 5/5 pass, including same-input Epoch resume and
  rejection of a different valid same-length repair payload.
- Python staging and upgrade tests: 39/39 pass.
- Actual ray export: 24.883 s; independent cold load: 16.736 s. No added masters.
- Full-routing symbolic band: 2,067 completed inspections, zero frontiers before
  the explicit 20,000-domain diagnostic allowance. This is not completed
  recursive closure.

CLI SHA256:
`4e76707baf8419b4b072ea58af0091272446138b33e0390fe23c10ce2f6e1f58`.
Partial-rule payload (220,735 bytes) SHA256:
`77941434f7d2bd6d161edce32946d5e70608b80fec518c543eee9020a6787e7a`.

The preparation receipt and frozen executable are under the new campaign's
`inputs/` and `bin/`; the release manifest is beside the frozen release.
Local diagnostic evidence is under
`TMP/postlaunch-20260930/production-frontier-20261001/`. See the
[mathematical investigation](research/five_loop_a1_frontier_2026-10-01.md) for
the actual exceptional guard and the distinction between local applicability
and full reachable-obligation closure.
