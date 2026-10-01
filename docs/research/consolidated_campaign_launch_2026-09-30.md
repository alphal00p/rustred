# Consolidated five-loop build: decision and owner-operated launch

Status: **tested experimental Ready/Union launch configuration**. Final isolated
strategy pairs and exact-input preparation checks passed independent review.
Current Epoch settings are not performance-qualified; no production launch or
full-five-loop closure is claimed.

## Decision

Use **Ready + G2 Union**, with **16 reserved physical cores**, for the next
experimental full-scope campaign. Do not deploy Epoch as a faster replacement:
its tested settings have failed the performance gate. This is the consolidated
build, not a claim that the parallel architecture achieved its intended speedup.

The setup preserves all67 existing saved owners and every one of the183 ordered
starting queries:116 physical/convenience queries are required and67 original
helpers are auxiliary. It enables explicit query roles and bounded automatic
rescue. No numerator/propagator bounds or descendants are dropped. LC2's current
steering is Ready/W100 with G2 Off; a compatible binary upgrade alone did not
enable Union or migrate its roles. Thus this fresh setup is not simply LC2 with
a different executable name.

Important architecture boundary: the original S5 throughput roadmap remains
partial. Inspector-side lookup, durable CP6 and bounded rolling execution are
implemented, but P2 bucket planning and P3 publication still run serially, records
remain JSON, and tracker updates remain per-edge. Ledger6 and edge hash-feed
batching are not the promised parallel bucket/typed-record/bulk-update stage.
Rejecting the measured Epoch settings is not proof that completing that larger
architecture cannot help.

Why this remains promising: the earlier independently verified G2 comparison
reduced scheduled work by24–28% in both finite/hot five-loop pairs. The latest
`1b33ad29` Ready/Union build again closes and cold-verifies the finite control;
the hot control also passed on the earlier consolidated `3428b519` build.
Those are not full183-query timings, a remaining-time estimate, or proof that a
restart will repay LC2's accumulated progress. See
[the G2 measurements](codex_g2_ready_deployment_2026-09-29.md).

### What the experiments decided

| Avenue | Observed result | Deployment choice |
|---|---|---|
| Epoch publication/window rescue | Best completed new4L setting25.911s versus Ready21.939s native+cold; oldest-ready creates severe extra work | Retain opt-in, do not select |
| True iterative deepening |4L staged30.038s versus identical final one-shot16.330s; both cold-verified | Do not use this curriculum;5L transfer deferred |
| Coordinate reindex heuristic | Small5L generation+walk+cold42.861s versus natural25.763s | Preserve natural coordinates |
| Source-row discovery recipes | Two isolated4L pairs improve generation+walk+cold17.45%/18.73%, walk+cold36.51%/39.23%; small5L scout24.911→17.670s | Available for generation; not silently applied to saved owners |
| Ready width |4L W16 closes in24.742s including cold; W50 stopped after250.373s native with52.93times as many inspections | Use tested W16; do not extrapolate to200 |

The new source-row API selects finite source/sector visit order; it is not the
proposed full integral-comparator language. Changing recipes needs no Rust
rebuild, but it does require regenerated rules and a fresh owner campaign. The
current67 saved owners are deliberately unchanged. Stronger source-artifact
certification attempts in the4L portfolio reached existing proof/input limits;
successful scoped cold traversal is not represented as successful source replay.

Evidence: [Epoch](epoch_ready_publication_results_2026-09-30.md),
[deepening](true_iterative_deepening_2026-09-30.md),
[coordinate transfer](five_loop_coordinate_pilot_2026-09-30.md),
[source-order portfolio](runtime_source_strategy_portfolio_2026-09-30.md),
[width scaling](ready_width_scaling_2026-09-30.md), and
[the current experiment ledger](../../CODEX_PROGRESS.md).

### What about the planned200 cores?

Native tests exercise a synthetic200-budget Epoch inspector-pool lifecycle,
cancellation/join and200-worker restore-request validation. This is not an
actual Ready/W200 campaign, checkpoint drill or physical-core performance test.
On the latest W16 finite
five-loop control, the native average was3.21 CPU-seconds per wall-second, with
roughly6.1–6.6 busy cores in its main traversal. Preparation was substantially
serial. The W50 four-loop run averaged6.72 but did much more total work.
No measured result supports promising200-core saturation or a speedup.

`--workers 200 --cpus 0-199` is an available experimental configuration, not the
recommended one. It requires a different fresh destination and must not overlap
LC2's128–227 reservation. Do not change a campaign's frozen worker/policy options
on resume. There is no hidden200-to16 fallback in the command below: it explicitly
reserves16 cores. Only the owner decides whether to pause LC2 and launch.

## Frozen executable, Python and environment

No rebuild is needed on this machine:

```text
Native source: 1b33ad2956d87f29db6ef9987542f3364f443814
Native source directory: /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/repo
Executable: /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-1b33ad29
SHA256: 73253922552ef341e3e97522d9481a4e369d388e9f8612c4ac92468c3c584a14
Python tree: /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/python-delivery-62c763cd
Python revision: 62c763cd974fb662458d5dc2d281e0306aa887b7
Symbolica: ef0db494533c87adb40356c996241680dc5a7bff
Build: campaign profile, opt3, fat LTO, one codegen unit
```

The full consolidated app test result is1,139 passed/one stale diagnostic-string
assertion failed/12 ignored. The expected string was corrected in the source,
but that Rust test binary was not rebuilt. Its independently audited ten-phase
public-CLI equivalent passed against this exact optimized executable. All new
native rolling/publication tests passed; reports preserve the raw failure rather
than claiming a wholly green suite. The unchanged A core tree previously passed
2,853 tests. Python and metadata checks are recorded in the progress ledger.

If a rebuild is desired, use the clean native source directory above, not dirty
main or the separate Python worktree:

```bash
cd /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/repo
env TMPDIR=/common/dev/rustred/TMP CARGO_INCREMENTAL=0 \
  nix develop --command cargo build --profile campaign --locked --offline \
  -j8 -p rustred-app --bin rustred
```

This is optional and can take about an hour. The frozen executable above remains
the measured delivery; rebuilding does not replace it automatically. Keep the
Symbolica license inherited in the environment, never written into these files.

## Exact owner commands

The same effective preparation policy passed against all67 real input payloads in
`TMP/codex-ready-launch-preflight.H9eIUf/`:10.167s for preparation and2.157s for
re-reading the frozen plan. The read-only census confirms all183 unchanged rows,
116/67 roles,8246 routes, identical owner receipts and the frozen executable.
No native solver, run directory, checkpoint or production mutation occurred.
The census initially expected `32-47` rather than the launcher's equivalent
canonical CPU list; its representation-only correction is retained in the log.
This operational check is not an all183-query solve or a production-scale resume.

The check used the pinned Python executable and explicitly spelled the unchanged
`owner-anchor-` helper-ID prefix; the commands below obtain Python from the frozen
Nix environment and use that default. The disposable destination is the only
campaign-location difference. Both plans preserve the original ordered inputs.
The final read-only receipt census was repeated successfully, and the frozen
tree's actual `nix develop --command python -B
examples/python/production_saved_owner_campaign.py --help` exited successfully.
These operational checks do not start production or certify its eventual result.

First, if choosing to switch, gracefully pause LC2 in its existing terminal and
wait for its durable checkpoint message and all native workers to exit. Preserve
that campaign and its frozen binary for rollback. No agent has paused it.

Create `codex_astra` only if it does not already exist:

```bash
env XDG_RUNTIME_DIR=/run/user/1125 zellij --session rustred action new-tab \
  --name codex_astra --cwd /common/dev/rustred -- bash
```

In that tab:

```bash
cd /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/python-delivery-62c763cd
nix develop --command bash
```

Then prepare once. The destination must not already exist; do not erase a
campaign to make this succeed. This copies immutable inputs and freezes the
executable and steering, **without launching the solver**:

```bash
PARALLEL_TREE=/common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/python-delivery-62c763cd
PARALLEL_BINARY=/common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-1b33ad29
PARALLEL_CAMPAIGN=/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-ready-union-1b33-w16
export TMPDIR=/common/dev/rustred/TMP

python -B "$PARALLEL_TREE/examples/python/production_saved_owner_campaign.py" \
  --campaign-directory "$PARALLEL_CAMPAIGN" \
  --prepare-from /common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-lc2 \
  --queries "$PARALLEL_TREE/examples/input/five_loop_qcd_feynman_d9d10/queries.json" \
  --attach "$PARALLEL_TREE/examples/input/five_loop_qcd_feynman_d9d10/entry-plan-receipt.json" \
  --query-order preserve --executable "$PARALLEL_BINARY" \
  --workers 16 --cpus 32-47 --publication-policy ready \
  --transfer-unreserved-lookahead 256 --g2-residual-anchors union \
  --frontier-policy stop --auto-rescue --max-rescues 32 \
  --checkpoint-interval-seconds 3600 \
  --max-memory-bytes 600000000000 --ram-guard-margin-percent 5 \
  --host-memory-reserve-bytes 150000000000 \
  --swap-growth-stop-bytes-per-second 33554432 --swap-growth-stop-seconds 120 \
  --json
```

The requested600GB ceiling is decimal bytes and is re-clamped to available
memory minus the150GB host floor. Cooperative save/stop starts at95% of the
effective ceiling or at the floor; a hard emergency guard remains. Checkpoints
target hourly intervals, throttled if prior saves are expensive. There is no
production wall-time or domain-count cap. The32-rescue bound applies only to
recognized recovery attempts, not ordinary inspections; unknown failures stop.

Start:

```bash
python -B "$PARALLEL_TREE/examples/python/production_saved_owner_campaign.py" \
  --campaign-directory "$PARALLEL_CAMPAIGN" --start
```

One ordinary Ctrl-C requests a checkpoint. Wait for the saved message and worker
drain. Resume later in the same frozen environment, re-establishing the variables:

```bash
python -B "$PARALLEL_TREE/examples/python/production_saved_owner_campaign.py" \
  --campaign-directory "$PARALLEL_CAMPAIGN" --resume --start
```

Fresh roles/G2 settings change the request binding: this does not resume LC2's
progress. Do not copy its checkpoint or edit a manifest to imitate a migration.
For rollback, stop/save the new run first, then use LC2's own Stage A launcher:

```bash
python -B /common/dev/rustred/TMP/codex-stage-a.2RU3AX/repo/examples/python/production_saved_owner_campaign.py \
  --campaign-directory /common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-lc2 \
  --resume --start
```

## Monitoring and final mathematical acceptance

The terminal dashboard shows measured busy cores, not just reservations.
The current standalone monitor shows `Discovery/closure` as newly discovered
divided by newly recursively closed domains in the same trailing-hour window,
recorded under the new `rates.discovery_per_recursive_closure_1h` field.
This ratio and unchanged pending growth per completion are red strictly above
2, yellow strictly above 1 through 2, and green otherwise; CPU colors are
unchanged. Positive discoveries with no closures show `∞`/red; empty, invalid,
unavailable or reset windows show `unknown`. Scan-batched and stale-snapshot
qualifications remain visible. Historical normalized balance values and frozen
launchers are not rewritten. Generic data production and dashboard rendering
are separate modules.
Each actual run prints its directory, containing `status.json` and
`telemetry.jsonl`. The read-only monitor accepts that directory with
`campaign_monitor.py DIRECTORY --once --json`; the optional plotting utility is
`plot_campaign_rates.py TELEMETRY --output FIGURE.svg`. The plot has unresolved
domains on the left axis and raw discovery-minus-closure rate on the right.
These paths refer to the printed run, not a guessed timestamp. `NO_COLOR=1`
disables colours. Progress rates and cached root counts are not completion proofs.

When the full run finishes, independently cold-load its actual request and
checkpoint with `walk-verify-closure --no-result --reinspect all
--reference-levers off --certification-scope physics-queries --require-closure`.
All116 required queries and their live dependencies must pass. Failed auxiliary
helpers are not counted closed. Neither these control tests nor this launch
handoff claim that the full five-loop scope is already closed.
