# Launching with the second five-loop frontier repair

The stopped rank-eleven diagonal case is repaired by 11 additional exact rules,
derived from the existing source equations with two free indices and no search
rank cutoff. No new master, dropped input, or topology-specific engine code is
introduced. See [the diagnosis](five_loop_second_frontier_2026-10-03.md).

The existing optimized engine already supports this repair. This delivery uses
that tested executable and newly frozen input/steering snapshots; it is not an
unnecessary rebuild or a claim of a new performance optimization. Executable
SHA256 is `8ef80b52da13866dbe09d817acd004ec3fc9ca2130d85411ac758398086cf0a7`.
Its engine source matches `8f6295f2`; subsequent commits through `a878e4fa` only
change documentation. Unrelated current FeynKit edits are not part of it.

## Important: this requires a fresh traversal

The old CP6 checkpoint binds the exact owner and overlay bytes. It also retains
the published `NativeFrontier`; simply changing the executable does not retry
that failure. Appending the repair therefore cannot safely use ordinary
`--resume` on either old input set. No checkpoint rebinding or conversion is
attempted.

Both prepared alternatives reuse their existing 67 saved owner bundles and
8,246 routes, preserve all 116 required queries and 67 auxiliary helpers, and
retain the October 1 repair. They add only the new 827,554-byte rule overlay.
No full source-generation campaign is repeated, but traversal progress must be
recomputed. The old checkpoints and the currently running campaign are untouched.

| Prepared alternative | Existing rules retained | Reserved CPUs |
| --- | --- | --- |
| `oldsavedpool` | The stopped older campaign's exact owner pool | 64–95 |
| `currentnew37pool` | The newer pool, including its experimental 37-term shortcut | 96–127 |

Each freezes 32 workers, the existing Epoch settings, a requested 600 GB RAM
ceiling, a 5% save-and-stop margin and a 150 GB host reserve. Actual memory
admission is rechecked at launch. There is no campaign time limit; checkpoints
are requested hourly and on cooperative interruption. Unknown frontiers still
stop and save rather than becoming terminals.

## Older campaign replacement

The delivery validation below is complete. To start the older-pool replacement:

```bash
cd /common/dev/rustred
nix develop /common/dev/rustred --command python -B \
  /common/dev/rustred/campaigns/five-loop-a1-oldsavedpool-frontier-repaired-20261003/steering/production_saved_owner_campaign.py \
  --campaign-directory /common/dev/rustred/campaigns/five-loop-a1-oldsavedpool-frontier-repaired-20261003 \
  --start
```

## Newer campaign replacement

This alternative uses the same CPUs as the currently running new-rule campaign.
First stop that campaign gracefully and wait for its checkpoint completion and
process exit; the assistant does not perform that stop. Then run:

```bash
cd /common/dev/rustred
nix develop /common/dev/rustred --command python -B \
  /common/dev/rustred/campaigns/five-loop-a1-currentnew37pool-frontier-repaired-20261003/steering/production_saved_owner_campaign.py \
  --campaign-directory /common/dev/rustred/campaigns/five-loop-a1-currentnew37pool-frontier-repaired-20261003 \
  --start
```

Use the Git-aware root path, **without `path:`**, so Nix does not copy ignored
campaigns and build caches. No nested release flake is needed. Use the existing
Symbolica license from the environment; none is saved in this delivery.

After either *new* campaign has started and later stopped cleanly, its own
checkpoint can be resumed with the same command plus `--resume`. Never import
the older campaign's checkpoint into the replacement directory. Rollback means
returning to the untouched original directory, binary and checkpoint, with the
understanding that the old stopped frontier is still unresolved by those inputs.

## Validation and limitations

- Source generation: 11 rules, zero residuals; exact source/guard replay passes.
- Separate cold load: source, guard and strict-descent checks pass; no generation.
- Finite applicability: 84/84 queries pass, including the original gap and
  diagonal ranks 21, 41, 101 and 201. All previously valid selections are unchanged.
- Actual stopped parent: its correlated four-point domain splits exactly into
  the existing rule and the repair, without a gap or unresolved predicate.
- The failing owner's 47 terminals are unchanged; no new terminal is declared.
- Generic staging tests: 26 pass. Executable-upgrade tests: 13 pass.

- Full-pool integration smoke: the latest CLI cold-loads all 67 owners, both
  overlays and 8,246 routes, then inspects the actual parent and two higher-rank
  diagonal roots. It schedules 2,000 domains, completes 73, routes 64 and reports
  zero frontiers. The deliberate scheduled-domain allowance causes exit 4 and
  one failed coordinator admission, with 1,926 obligations still pending.
  This is an incomplete smoke test, not a closed reduction. The parent retains
  five conditional successors through the existing conservative routing path;
  no successor is discarded. Inclusive time is 138.964 s, of which 133.947 s is
  native preparation; sampled peak process-tree RSS is 5.19 GB. Owned processes
  drain normally. There is no corresponding production work cap.
- Independent delivery audit: every staged owner/overlay hash matches the
  intended source variant, all query bytes and route objects are unchanged,
  and both frozen binaries and nine-module steering snapshots validate. No
  production run or checkpoint is created by preparation. The smoke uses the
  newer pool; its inspected prefix does not reach the shortcut owner, so it
  does not compare the performance of the two alternatives.

The broad, unbounded two-coordinate rectangular diagnostic retains one
unresolved classification. This does not affect the checked finite parent,
but no unrestricted face coverage or complete five-loop closure is claimed.
There may be further previously unvisited exceptional cases elsewhere.

Local receipts and repeatable preparation scripts are in
`TMP/frontier-repair-20261003/`. The full campaign payloads remain local and are
not committed. Owner and overlay paths within each selection are relative;
the frozen steering binds this installation's campaign directory.

The delivered new-overlay SHA256 is
`7e1f93615282130555f29a885d1bbfe7b5b0c08074d7c2d1e542378b4457a86a`.
Each campaign contains `prepared-launch-plan.json`, `repair-validation.json`,
`inputs/input-receipt.json`, `steering/source-receipt.json` and its frozen binary.
Preparation is reproducible locally through
`TMP/frontier-repair-20261003/prepare_fresh_pool.py`; its `--stage` mode freezes
inputs but never passes `--start`. The source search itself is reproduced in
the companion diagnosis document.
